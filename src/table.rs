//! Table commands: list tables and set a numeric cell.
use prost::Message;

use crate::{tsp, tst, write};

fn tables_from(docs: &[write::Doc]) -> Vec<(u64, tst::TableModelArchive)> {
    let mut out = Vec::new();
    let map = write::ident_map(docs);
    for (id, (_t, b)) in map {
        if let Ok(tm) = <tst::TableModelArchive as Message>::decode(b.as_slice()) {
            if tm.number_of_rows > 0 && tm.number_of_columns > 0 {
                out.push((id, tm));
            }
        }
    }
    out.sort_by_key(|(id, _)| *id);
    out
}

pub fn list(path: &str) -> Result<(), String> {
    let docs = write::load_docs(path)?;
    let tables = tables_from(&docs);
    if tables.is_empty() {
        return Err("文档中没有表格".into());
    }
    println!("共 {} 个表格：", tables.len());
    for (id, tm) in &tables {
        println!("  ident={id:<12} {} 行 × {} 列   {:?}", tm.number_of_rows, tm.number_of_columns, tm.table_name);
    }
    println!("\n用 set-cell <src> <dst> <ident> <行> <列> <新值> 改数值。");
    Ok(())
}

pub fn set_cell(src: &str, dst: &str, ident: u64, row: u32, col: u32, value: &str) -> Result<(), String> {
    let docs = write::load_docs(src)?;
    let map = write::ident_map(&docs);
    let (_, tb) = map.get(&ident).ok_or(format!("找不到表格 {ident}"))?;
    let tm = <tst::TableModelArchive as Message>::decode(tb.as_slice())
        .map_err(|_| format!("ident {ident} 不是表格"))?;
    if row == 0 || col == 0 || row > tm.number_of_rows || col > tm.number_of_columns {
        return Err(format!("越界：表格为 {} 行 × {} 列", tm.number_of_rows, tm.number_of_columns));
    }
    let tile_id = tm.base_data_store.tiles.tiles.first()
        .ok_or("表格没有数据块")?.tile.identifier;
    let (di, si, oi) = write::find_object(&docs, tile_id, "tile").ok_or("未定位到 Tile 对象")?;
    let mut tile = <tst::Tile as Message>::decode(docs[di].segs[si].objs[oi].as_slice())
        .map_err(|e| e.to_string())?;
    let ri = tile.row_infos.iter_mut()
        .find(|r| r.tile_row_index == row - 1)
        .ok_or("未找到目标行")?;
    let cell_count = ri.cell_count as usize;
    let offsets = ri.cell_offsets.as_ref().unwrap_or(&ri.cell_offsets_pre_bnc);
    if offsets.len() < cell_count * 2 || (col as usize) > cell_count {
        return Err("单元格超出范围".into());
    }
    let start = u16::from_le_bytes([offsets[(col as usize - 1) * 2], offsets[(col as usize - 1) * 2 + 1]]) as usize;
    let mut buf = ri.cell_storage_buffer.as_ref()
        .unwrap_or(&ri.cell_storage_buffer_pre_bnc)
        .clone();
    let end = if (col as usize) < cell_count {
        u16::from_le_bytes([offsets[col as usize * 2], offsets[col as usize * 2 + 1]]) as usize
    } else {
        buf.len()
    };
    let size = end.saturating_sub(start);
    if size < 36 {
        return Err(format!("第 {row} 行第 {col} 列是文本格（{size} 字节），本命令只改数字格"));
    }
    if start + 16 > buf.len() {
        return Err("单元格偏移越界".into());
    }
    let old = i32::from_le_bytes([buf[start + 12], buf[start + 13], buf[start + 14], buf[start + 15]]);
    let new: i32 = value.parse::<f64>().map_err(|_| "新值不是数字")?.round() as i32;
    buf[start + 12..start + 16].copy_from_slice(&new.to_le_bytes());
    ri.cell_storage_buffer = Some(buf);
    let encoded = tile.encode_to_vec();
    write::save_rewrite(src, dst, &docs[di].name, si, oi, &encoded)?;
    println!("表格 {ident} 第 {row} 行第 {col} 列：{old} → {new}");
    Ok(())
}


pub fn set_size(
    src: &str,
    dst: &str,
    ident: u64,
    row_height: Option<f32>,
    col_width: Option<f32>,
) -> Result<(), String> {
    let mut docs = write::load_docs(src)?;
    let map = write::ident_map(&docs);
    let (_, tb) = map.get(&ident).ok_or(format!("找不到表格 {ident}"))?;
    let mut tm = <tst::TableModelArchive as Message>::decode(tb.as_slice())
        .map_err(|_| format!("ident {ident} 不是表格"))?;
    if let Some(h) = row_height { tm.default_row_height = h as f64; }
    if let Some(w) = col_width { tm.default_column_width = w as f64; }

    let col_id = tm.base_data_store.column_headers.identifier;
    let row_ids: Vec<u64> = tm.base_data_store.row_headers.buckets.iter().map(|r| r.identifier).collect();
    let (ti, tsi, toi) = write::find_object(&docs, ident, "").ok_or("未定位到表格对象")?;
    docs[ti].segs[tsi].objs[toi] = tm.encode_to_vec();

    let mut update_header = |id: u64, value: f32| -> Result<(), String> {
        let Some((di, si, oi)) = write::find_object(&docs, id, "header") else { return Ok(()) };
        let mut hb = <tst::HeaderStorageBucket as Message>::decode(docs[di].segs[si].objs[oi].as_slice())
            .map_err(|e| e.to_string())?;
        for h in &mut hb.headers { h.size = value; }
        docs[di].segs[si].objs[oi] = hb.encode_to_vec();
        Ok(())
    };
    if let Some(w) = col_width { update_header(col_id, w)?; }
    if let Some(h) = row_height {
        for rid in row_ids { update_header(rid, h)?; }
    }
    write::save_docs(src, dst, &docs)?;
    let mut parts = Vec::new();
    if let Some(h) = row_height { parts.push(format!("行高 {h}")); }
    if let Some(w) = col_width { parts.push(format!("列宽 {w}")); }
    println!("表格 {ident}：{}", parts.join("、"));
    Ok(())
}


fn fresh_uid(taken: &[tsp::Uuid]) -> (u64, u64) {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let mut seed = t.as_nanos() as u64;
    loop {
        let lo = seed ^ 0x9e3779b97f4a7c15;
        let up = seed.wrapping_mul(0xbf58476d1ce4e5b9).rotate_left(17);
        if !taken.iter().any(|u| u.lower == lo && u.upper == up) {
            return (lo, up);
        }
        seed = seed.wrapping_add(1);
    }
}

pub fn add_row(src: &str, dst: &str, ident: u64, source_row: u32, at_row: Option<u32>) -> Result<(), String> {
    let mut docs = write::load_docs(src)?;
    let map = write::ident_map(&docs);
    let (_, tb) = map.get(&ident).ok_or(format!("找不到表格 {ident}"))?;
    let mut tm = <tst::TableModelArchive as Message>::decode(tb.as_slice()).map_err(|_| "不是表格")?;
    let n = tm.number_of_rows;
    if source_row == 0 || source_row > n { return Err(format!("源行越界：1..{n}")); }
    let at = at_row.unwrap_or(source_row + 1);
    if at == 0 || at > n + 1 { return Err(format!("插入位置越界：1..{}", n + 1)); }
    let insert_pos = (at - 1) as usize;
    let tile_id = tm.base_data_store.tiles.tiles.first().ok_or("表格没有数据块")?.tile.identifier;
    let uid_id = tm.base_column_row_uids.as_ref().ok_or("表格缺少 UID 映射")?.identifier;

    let uid_loc = write::find_object(&docs, uid_id, "uid").ok_or("未定位 UID 映射")?;
    let mut u = <tst::ColumnRowUidMapArchive as Message>::decode(
        docs[uid_loc.0].segs[uid_loc.1].objs[uid_loc.2].as_slice()).map_err(|e| e.to_string())?;
    let (lo, up) = fresh_uid(&u.sorted_row_uids);
    let ins_idx = u.sorted_row_uids.iter().position(|x| (x.upper, x.lower) > (up, lo)).unwrap_or(u.sorted_row_uids.len());
    u.sorted_row_uids.insert(ins_idx, tsp::Uuid { lower: lo, upper: up });
    let mut idxs: Vec<u32> = u.row_index_for_uid.iter().map(|v| if *v >= insert_pos as u32 { v + 1 } else { *v }).collect();
    idxs.insert(ins_idx, insert_pos as u32);
    u.row_index_for_uid = idxs;
    u.row_uid_for_index.insert(insert_pos, ins_idx as u32);
    tm.number_of_rows = n + 1;

    let tile_loc = write::find_object(&docs, tile_id, "tile").ok_or("未定位 Tile")?;
    let mut tile = <tst::Tile as Message>::decode(
        docs[tile_loc.0].segs[tile_loc.1].objs[tile_loc.2].as_slice()).map_err(|e| e.to_string())?;
    let src_ri = tile.row_infos.get((source_row - 1) as usize).cloned().ok_or("源行不存在")?;
    tile.row_infos.insert(insert_pos, src_ri);
    for (i, ri) in tile.row_infos.iter_mut().enumerate() { ri.tile_row_index = i as u32; }
    tile.numrows = tile.row_infos.len() as u32;

    let table_loc = write::find_object(&docs, ident, "").ok_or("未定位表格对象")?;
    docs[table_loc.0].segs[table_loc.1].objs[table_loc.2] = tm.encode_to_vec();
    docs[tile_loc.0].segs[tile_loc.1].objs[tile_loc.2] = tile.encode_to_vec();
    docs[uid_loc.0].segs[uid_loc.1].objs[uid_loc.2] = u.encode_to_vec();
    write::save_docs(src, dst, &docs)?;
    println!("表格 {ident}：在第 {at} 行插入一行（复制第 {source_row} 行），行数 {n} → {}", n + 1);
    Ok(())
}

pub fn del_row(src: &str, dst: &str, ident: u64, row: u32) -> Result<(), String> {
    let mut docs = write::load_docs(src)?;
    let map = write::ident_map(&docs);
    let (_, tb) = map.get(&ident).ok_or(format!("找不到表格 {ident}"))?;
    let mut tm = <tst::TableModelArchive as Message>::decode(tb.as_slice()).map_err(|_| "不是表格")?;
    let n = tm.number_of_rows;
    if row == 0 || row > n { return Err(format!("行号越界：1..{n}")); }
    if n <= 1 { return Err("表格只剩一行，不能再删".into()); }
    let pos = (row - 1) as usize;
    let tile_id = tm.base_data_store.tiles.tiles.first().ok_or("表格没有数据块")?.tile.identifier;
    let uid_id = tm.base_column_row_uids.as_ref().ok_or("表格缺少 UID 映射")?.identifier;

    let uid_loc = write::find_object(&docs, uid_id, "uid").ok_or("未定位 UID 映射")?;
    let mut u = <tst::ColumnRowUidMapArchive as Message>::decode(
        docs[uid_loc.0].segs[uid_loc.1].objs[uid_loc.2].as_slice()).map_err(|e| e.to_string())?;
    u.row_index_for_uid.retain(|v| *v != pos as u32);
    for v in &mut u.row_index_for_uid { if *v > pos as u32 { *v -= 1; } }
    if pos < u.row_uid_for_index.len() { u.row_uid_for_index.remove(pos); }
    if pos < u.sorted_row_uids.len() { u.sorted_row_uids.remove(pos); }
    tm.number_of_rows = n - 1;

    let tile_loc = write::find_object(&docs, tile_id, "tile").ok_or("未定位 Tile")?;
    let mut tile = <tst::Tile as Message>::decode(
        docs[tile_loc.0].segs[tile_loc.1].objs[tile_loc.2].as_slice()).map_err(|e| e.to_string())?;
    if pos < tile.row_infos.len() { tile.row_infos.remove(pos); }
    for (i, ri) in tile.row_infos.iter_mut().enumerate() { ri.tile_row_index = i as u32; }
    tile.numrows = tile.row_infos.len() as u32;

    let table_loc = write::find_object(&docs, ident, "").ok_or("未定位表格对象")?;
    docs[table_loc.0].segs[table_loc.1].objs[table_loc.2] = tm.encode_to_vec();
    docs[tile_loc.0].segs[tile_loc.1].objs[tile_loc.2] = tile.encode_to_vec();
    docs[uid_loc.0].segs[uid_loc.1].objs[uid_loc.2] = u.encode_to_vec();
    write::save_docs(src, dst, &docs)?;
    println!("表格 {ident}：已删除第 {row} 行，行数 {n} → {}", n - 1);
    Ok(())
}


pub fn add_col(src: &str, dst: &str, ident: u64, at_col: Option<u32>) -> Result<(), String> {
    let mut docs = write::load_docs(src)?;
    let map = write::ident_map(&docs);
    let (_, tb) = map.get(&ident).ok_or(format!("找不到表格 {ident}"))?;
    let mut tm = <tst::TableModelArchive as Message>::decode(tb.as_slice()).map_err(|_| "不是表格")?;
    let n_cols = tm.number_of_columns;
    let at = at_col.unwrap_or(n_cols + 1);
    if at == 0 || at > n_cols + 1 { return Err(format!("列位置越界：1..{}", n_cols + 1)); }
    let ins_col = (at - 1) as usize;
    let tile_id = tm.base_data_store.tiles.tiles.first().ok_or("表格没有数据块")?.tile.identifier;
    let uid_id = tm.base_column_row_uids.as_ref().ok_or("表格缺少 UID 映射")?.identifier;

    let uid_loc = write::find_object(&docs, uid_id, "uid").ok_or("未定位 UID 映射")?;
    let mut u = <tst::ColumnRowUidMapArchive as Message>::decode(
        docs[uid_loc.0].segs[uid_loc.1].objs[uid_loc.2].as_slice()).map_err(|e| e.to_string())?;
    let (lo, up) = fresh_uid(&u.sorted_column_uids);
    let ins = u.sorted_column_uids.iter().position(|x| (up, lo) < (x.upper, x.lower)).unwrap_or(u.sorted_column_uids.len());
    u.sorted_column_uids.insert(ins, tsp::Uuid { lower: lo, upper: up });
    let mut idxs: Vec<u32> = u.column_index_for_uid.iter().map(|v| if *v >= ins_col as u32 { v + 1 } else { *v }).collect();
    idxs.insert(ins, ins_col as u32);
    u.column_index_for_uid = idxs;
    u.column_uid_for_index.insert(ins_col, ins as u32);
    tm.number_of_columns = n_cols + 1;

    let tile_loc = write::find_object(&docs, tile_id, "tile").ok_or("未定位 Tile")?;
    let mut tile = <tst::Tile as Message>::decode(
        docs[tile_loc.0].segs[tile_loc.1].objs[tile_loc.2].as_slice()).map_err(|e| e.to_string())?;
    let mut nid: u32 = 100;
    for ri in &mut tile.row_infos {
        nid += 1;
        let mut cell = vec![0x05, 0x03];
        cell.extend_from_slice(&[0u8; 6]);
        cell.extend_from_slice(&[0x08, 0x10, 0x02, 0x00]);
        cell.extend_from_slice(&nid.to_le_bytes());
        cell.extend_from_slice(&[0x05, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00]);
        let mut buf = ri.cell_storage_buffer.clone().unwrap_or_default();
        buf.extend_from_slice(&cell);
        ri.cell_storage_buffer = Some(buf);
        ri.cell_count += 1;
    }
    tile.numrows = tile.row_infos.len() as u32;

    let table_loc = write::find_object(&docs, ident, "").ok_or("未定位表格对象")?;
    docs[table_loc.0].segs[table_loc.1].objs[table_loc.2] = tm.encode_to_vec();
    docs[tile_loc.0].segs[tile_loc.1].objs[tile_loc.2] = tile.encode_to_vec();
    docs[uid_loc.0].segs[uid_loc.1].objs[uid_loc.2] = u.encode_to_vec();
    write::save_docs(src, dst, &docs)?;
    println!("表格 {ident}：已加一列（第 {at} 列，空文本格），列数 {n_cols} → {}", n_cols + 1);
    Ok(())
}


pub fn del_col(src: &str, dst: &str, ident: u64, col: u32) -> Result<(), String> {
    let mut docs = write::load_docs(src)?;
    let map = write::ident_map(&docs);
    let (_, tb) = map.get(&ident).ok_or(format!("找不到表格 {ident}"))?;
    let mut tm = <tst::TableModelArchive as Message>::decode(tb.as_slice()).map_err(|_| "不是表格")?;
    let n = tm.number_of_columns;
    if n <= 1 { return Err("表格只剩一列".into()); }
    if col == 0 || col > n { return Err(format!("列号越界：1..{n}")); }
    let pos = (col - 1) as usize;
    let tile_id = tm.base_data_store.tiles.tiles.first().ok_or("表格没有数据块")?.tile.identifier;
    let uid_id = tm.base_column_row_uids.as_ref().ok_or("表格缺少 UID 映射")?.identifier;

    let uid_loc = write::find_object(&docs, uid_id, "uid").ok_or("未定位 UID 映射")?;
    let mut u = <tst::ColumnRowUidMapArchive as Message>::decode(docs[uid_loc.0].segs[uid_loc.1].objs[uid_loc.2].as_slice()).map_err(|e| e.to_string())?;
    u.column_index_for_uid.retain(|v| *v != pos as u32);
    for v in &mut u.column_index_for_uid { if *v > pos as u32 { *v -= 1; } }
    if pos < u.sorted_column_uids.len() { u.sorted_column_uids.remove(pos); }
    let mut new_uid_idx = Vec::new();
    for (i, v) in u.column_uid_for_index.iter().enumerate() {
        if i == pos { continue; }
        new_uid_idx.push(if *v > pos as u32 { v - 1 } else { *v });
    }
    u.column_uid_for_index = new_uid_idx;
    tm.number_of_columns = n - 1;

    let tile_loc = write::find_object(&docs, tile_id, "tile").ok_or("未定位 Tile")?;
    let mut tile = <tst::Tile as Message>::decode(docs[tile_loc.0].segs[tile_loc.1].objs[tile_loc.2].as_slice()).map_err(|e| e.to_string())?;
    for ri in &mut tile.row_infos {
        let count = ri.cell_count as usize;
        let offs = ri.cell_offsets.as_ref().unwrap_or(&ri.cell_offsets_pre_bnc);
        if pos >= count || offs.len() < count * 2 { continue; }
        let buf = ri.cell_storage_buffer.clone().unwrap_or_else(|| ri.cell_storage_buffer_pre_bnc.clone());
        let mut cells = Vec::new();
        for i in 0..count {
            let s = u16::from_le_bytes([offs[i * 2], offs[i * 2 + 1]]) as usize;
            let e = if i + 1 < count { u16::from_le_bytes([offs[(i + 1) * 2], offs[(i + 1) * 2 + 1]]) as usize } else { buf.len() };
            cells.push(buf.get(s..e).unwrap_or(&[]).to_vec());
        }
        if pos >= cells.len() { continue; }
        cells.remove(pos);
        let mut new_buf = Vec::new();
        let mut new_offs = Vec::new();
        for c in &cells { new_offs.push(new_buf.len() as u16); new_buf.extend_from_slice(c); }
        ri.cell_storage_buffer = Some(new_buf);
        let mut off_bytes = vec![0xFFu8; offs.len()];
        for (i, v) in new_offs.iter().enumerate() {
            if i * 2 + 1 < off_bytes.len() { off_bytes[i * 2..i * 2 + 2].copy_from_slice(&v.to_le_bytes()); }
        }
        ri.cell_offsets = Some(off_bytes);
        ri.cell_count = cells.len() as u32;
    }
    tile.numrows = tile.row_infos.len() as u32;

    let hdr_id = tm.base_data_store.column_headers.identifier;
    let mut hdr: Option<(usize, usize, usize, tst::HeaderStorageBucket)> = None;
    if let Some((di, si, oi)) = write::find_object(&docs, hdr_id, "header") {
        let mut hb = <tst::HeaderStorageBucket as Message>::decode(docs[di].segs[si].objs[oi].as_slice()).map_err(|e| e.to_string())?;
        if pos < hb.headers.len() {
            let total: f32 = hb.headers.iter().map(|h| h.size).sum();
            hb.headers.remove(pos);
            if !hb.headers.is_empty() {
                let each = total / hb.headers.len() as f32;
                for (i, h) in hb.headers.iter_mut().enumerate() { h.size = each; h.index = i as u32; }
            }
            hdr = Some((di, si, oi, hb));
        }
    }

    let table_loc = write::find_object(&docs, ident, "").ok_or("未定位表格对象")?;
    docs[table_loc.0].segs[table_loc.1].objs[table_loc.2] = tm.encode_to_vec();
    docs[tile_loc.0].segs[tile_loc.1].objs[tile_loc.2] = tile.encode_to_vec();
    docs[uid_loc.0].segs[uid_loc.1].objs[uid_loc.2] = u.encode_to_vec();
    if let Some((di, si, oi, hb)) = hdr { docs[di].segs[si].objs[oi] = hb.encode_to_vec(); }
    write::save_docs(src, dst, &docs)?;
    println!("表格 {ident}：已删除第 {col} 列，列数 {n} → {}", n - 1);
    Ok(())
}
