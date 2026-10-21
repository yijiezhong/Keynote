//! Table commands: list tables and set a numeric cell.
use prost::Message;

use crate::{schema, tsce, tsd, tsp, tss, tst, write};
use crate::iwa::{Node, NodeValue};

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


fn max_ident(docs: &[write::Doc]) -> u64 {
    let mut top = 0;
    for d in docs { for s in &d.segs { if let Some(id) = crate::iwa::ident_of(s) { top = top.max(id); } } }
    top
}

fn stroke(r: f32, g: f32, b: f32, width: f32) -> tsd::StrokeArchive {
    tsd::StrokeArchive {
        color: Some(tsp::Color { model: 1, r: Some(r), g: Some(g), b: Some(b), a: Some(1.0), rgbspace: Some(1), ..Default::default() }),
        width: Some(width),
        ..Default::default()
    }
}

pub fn set_border(src: &str, dst: &str, ident: u64, color: Option<&str>, width: Option<f32>, header: bool, body: bool, footer: bool) -> Result<(), String> {
    let (r, g, b) = match color {
        Some(c) => {
            let s = c.trim().trim_start_matches('#');
            if s.len() != 6 { return Err("颜色格式应为 #RRGGBB".into()); }
            (u8::from_str_radix(&s[0..2], 16).unwrap() as f32 / 255.0,
             u8::from_str_radix(&s[2..4], 16).unwrap() as f32 / 255.0,
             u8::from_str_radix(&s[4..6], 16).unwrap() as f32 / 255.0)
        }
        None => (0.932, 0.135, 0.047),
    };
    let w = width.unwrap_or(4.0);
    let mut docs = write::load_docs(src)?;
    let map = write::ident_map(&docs);
    let (_, tb) = map.get(&ident).ok_or(format!("找不到表格 {ident}"))?;
    let mut tm = <tst::TableModelArchive as Message>::decode(tb.as_slice()).map_err(|_| "不是表格")?;
    let base_id = tm.table_style.identifier;
    let (_, base_b) = map.get(&base_id).ok_or("该表格没有 table_style")?;
    let base = <tst::TableStyleArchive as Message>::decode(base_b.as_slice()).map_err(|_| "table_style 不是 TableStyleArchive")?;
    let new_id = max_ident(&docs) + 1;
    let mut sup = tss::StyleArchive::default();
    sup.parent = Some(tsp::Reference { identifier: base_id, ..Default::default() });
    sup.is_variation = Some(true);
    sup.stylesheet = base.super_.stylesheet.clone();
    let mut props = tst::TableStylePropertiesArchive::default();
    props.table_border_visible = Some(true);
    let st = stroke(r, g, b, w);
    if header {
        props.header_row_border_stroke = Some(st.clone());
        props.header_column_border_stroke = Some(st.clone());
    }
    if body {
        props.table_body_horizontal_border_stroke = Some(st.clone());
        props.table_body_vertical_border_stroke = Some(st.clone());
    }
    if footer { props.footer_row_border_stroke = Some(st); }
    let archive = tst::TableStyleArchive { super_: sup, override_count: Some(6), table_properties: Some(props) };
    let blob = archive.encode_to_vec();
    tm.table_style.identifier = new_id;
    let (di, si, oi) = write::find_object(&docs, ident, "").ok_or("未定位表格对象")?;
    docs[di].segs[si].objs[oi] = tm.encode_to_vec();
    let type_id = schema::type_id("TST.TableStyleArchive").ok_or("registry 缺少 TableStyleArchive")?;
    let mi = vec![
        Node { field: 1, value: NodeValue::Varint(type_id) },
        Node { field: 2, value: NodeValue::Bytes(vec![1, 0, 5]) },
        Node { field: 3, value: NodeValue::Varint(blob.len() as u64) },
    ];
    let ainfo = vec![
        Node { field: 1, value: NodeValue::Varint(new_id) },
        Node { field: 2, value: NodeValue::Bytes(crate::iwa::pb_serialize_nodes(&mi)) },
    ];
    docs[di].segs.push(crate::iwa::Segment { ainfo, objs: vec![blob], meta: vec![(type_id, 0)] });
    write::save_docs(src, dst, &docs)?;
    println!("表格 {ident}：边框已更新（新样式 ident={new_id}）");
    Ok(())
}


fn cfuuid_from_string(s: &str) -> Option<tsp::CfuuidArchive> {
    let hex: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() != 32 { return None; }
    let b: Vec<u8> = (0..16).filter_map(|i| u8::from_str_radix(&hex[i*2..i*2+2], 16).ok()).collect();
    if b.len() != 16 { return None; }
    Some(tsp::CfuuidArchive {
        uuid_w0: Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        uuid_w1: Some(u32::from_le_bytes([b[4], b[5], b[6], b[7]])),
        uuid_w2: Some(u32::from_le_bytes([b[8], b[9], b[10], b[11]])),
        uuid_w3: Some(u32::from_le_bytes([b[12], b[13], b[14], b[15]])),
        ..Default::default()
    })
}

fn merge_formula(table_id: &str, r1: u32, c1: u32, r2: u32, c2: u32) -> tsce::FormulaArchive {
    let table_uuid = cfuuid_from_string(table_id).unwrap_or_default();
    let sticky = tsce::ast_node_array_archive::AstStickyBits {
        begin_row_is_absolute: true, begin_column_is_absolute: true,
        end_row_is_absolute: true, end_column_is_absolute: true,
    };
    let range = tsce::ast_node_array_archive::AstColonTractArchive {
        absolute_column: vec![tsce::ast_node_array_archive::ast_colon_tract_archive::AstColonTractAbsoluteRangeArchive {
            range_begin: c1.min(c2), range_end: Some(c1.max(c2)),
        }],
        absolute_row: vec![tsce::ast_node_array_archive::ast_colon_tract_archive::AstColonTractAbsoluteRangeArchive {
            range_begin: r1.min(r2), range_end: Some(r1.max(r2)),
        }],
        preserve_rectangular: Some(true),
        ..Default::default()
    };
    let node1 = tsce::ast_node_array_archive::AstNodeArchive {
        ast_node_type: tsce::ast_node_array_archive::AstNodeType::ColonTractNode as i32,
        ast_cross_table_reference_extra_info: Some(tsce::ast_node_array_archive::AstCrossTableReferenceExtraInfoArchive {
            table_id: table_uuid, ..Default::default()
        }),
        ast_sticky_bits: Some(sticky),
        ast_colon_tract: Some(range),
        ..Default::default()
    };
    let node2 = tsce::ast_node_array_archive::AstNodeArchive {
        ast_node_type: tsce::ast_node_array_archive::AstNodeType::FunctionNode as i32,
        ast_function_node_index: Some(168),
        ast_function_node_num_args: Some(1),
        ..Default::default()
    };
    tsce::FormulaArchive {
        ast_node_array: tsce::AstNodeArrayArchive { ast_node: vec![node1, node2] },
        ..Default::default()
    }
}

fn split_cells(buf: &[u8], offsets: &[u8], count: u32) -> Vec<Option<Vec<u8>>> {
    let n = (offsets.len() / 2).min(count as usize);
    let offs: Vec<u16> = (0..n).map(|i| u16::from_le_bytes([offsets[i*2], offsets[i*2+1]])).collect();
    let mut cells = Vec::with_capacity(n);
    for k in 0..n {
        if offs[k] == 0xFFFF { cells.push(None); continue; }
        let st = offs[k] as usize;
        let mut en = buf.len();
        for j in k+1..n { if offs[j] != 0xFFFF { en = offs[j] as usize; break; } }
        if st > buf.len() || en > buf.len() || st > en { cells.push(Some(Vec::new())); }
        else { cells.push(Some(buf[st..en].to_vec())); }
    }
    cells
}

fn rebuild_cells(cells: &[Option<Vec<u8>>]) -> (Vec<u8>, Vec<u8>, u32) {
    let mut offs = vec![0xFFFFu16; cells.len()];
    let mut buf = Vec::new();
    let mut n = 0u32;
    for (i, cell) in cells.iter().enumerate() {
        let Some(c) = cell else { continue };
        offs[i] = buf.len() as u16;
        buf.extend_from_slice(c);
        n += 1;
    }
    let mut ob = Vec::with_capacity(offs.len() * 2);
    for v in offs { ob.extend_from_slice(&v.to_le_bytes()); }
    (buf, ob, n)
}

fn drop_cells(buf: &[u8], offs: &[u8], count: u32, drop: &[usize]) -> (Vec<u8>, Vec<u8>, u32) {
    let mut cells = split_cells(buf, offs, count);
    for i in drop { if *i < cells.len() { cells[*i] = None; } }
    rebuild_cells(&cells)
}

fn cell_text(rec: &Option<Vec<u8>>, strings: &std::collections::HashMap<u32, String>) -> String {
    let Some(b) = rec else { return String::new(); };
    if b.len() < 16 { return String::new(); }
    let val = i32::from_le_bytes([b[12], b[13], b[14], b[15]]);
    match b[1] {
        3 => strings.get(&(val as u32)).cloned().unwrap_or_default(),
        2 => val.to_string(),
        _ => String::new(),
    }
}

pub fn merge_cells(src: &str, dst: &str, ident: u64, r1: u32, c1: u32, r2: u32, c2: u32) -> Result<(), String> {
    let mut docs = write::load_docs(src)?;
    let map = write::ident_map(&docs);
    let (_, tb) = map.get(&ident).ok_or(format!("找不到表格 {ident}"))?;
    let mut tm = <tst::TableModelArchive as Message>::decode(tb.as_slice()).map_err(|_| format!("ident {ident} 不是表格"))?;
    let rr1 = r1.min(r2); let rr2 = r1.max(r2);
    let cc1 = c1.min(c2); let cc2 = c1.max(c2);
    if cc2 >= tm.number_of_columns || rr2 >= tm.number_of_rows {
        return Err(format!("合并范围越界：表为 {} 行 × {} 列", tm.number_of_rows, tm.number_of_columns));
    }
    let tdl_ref = tm.base_data_store.string_table.identifier;
    let tile_ref = tm.base_data_store.tiles.tiles.first().map(|t| t.tile.identifier).ok_or("表格没有 Tile")?;
    let mut tdl = map.get(&tdl_ref).and_then(|(_, b)| <tst::TableDataList as Message>::decode(b.as_slice()).ok());
    let mut tile = map.get(&tile_ref).and_then(|(_, b)| <tst::Tile as Message>::decode(b.as_slice()).ok()).ok_or("找不到表格 Tile")?;
    let strings: std::collections::HashMap<u32, String> = tdl.as_ref().map(|t| t.entries.iter().map(|e| (e.key, e.string.clone().unwrap_or_default())).collect()).unwrap_or_default();
    let mut merged_text = String::new();
    if tdl.is_some() {
        let mut lines = Vec::new();
        for ri in rr1..=rr2 {
            let Some(row) = tile.row_infos.get(ri as usize) else { continue; };
            let cells = split_cells(row.cell_storage_buffer.as_deref().unwrap_or(&[]), row.cell_offsets.as_deref().unwrap_or(&[]), row.cell_count);
            let mut line = Vec::new();
            for ci in cc1..=cc2 {
                if let Some(c) = cells.get(ci as usize) { line.push(cell_text(c, &strings)); }
            }
            lines.push(line.join("\t"));
        }
        merged_text = lines.join("\n");
        if !merged_text.is_empty() {
            if let Some(t) = tdl.as_mut() {
                let key = t.next_list_id;
                t.entries.push(tst::table_data_list::ListEntry { key, refcount: 1, string: Some(merged_text.clone()), ..Default::default() });
                t.next_list_id = key + 1;
                // Point the top-left cell at the new string entry.
                if let Some(row) = tile.row_infos.get_mut(rr1 as usize) {
                    let mut cells = split_cells(row.cell_storage_buffer.as_deref().unwrap_or(&[]), row.cell_offsets.as_deref().unwrap_or(&[]), row.cell_count);
                    if let Some(Some(recv)) = cells.get_mut(cc1 as usize) {
                        if recv.len() >= 16 {
                            recv[12..16].copy_from_slice(&(key as i32).to_le_bytes());
                        }
                        let (b, o, _) = rebuild_cells(&cells);
                        row.cell_storage_buffer = Some(b);
                        row.cell_offsets = Some(o);
                    }
                }
            }
        }
    }
    // Remove the merged cells from every row, preserving the slot count.
    for ri in rr1..=rr2 {
        let Some(row) = tile.row_infos.get_mut(ri as usize) else { continue; };
        let drop = if ri == rr1 { (cc1+1..=cc2).map(|x| x as usize).collect::<Vec<_>>() } else { (cc1..=cc2).map(|x| x as usize).collect::<Vec<_>>() };
        let (b, o, n) = drop_cells(row.cell_storage_buffer.as_deref().unwrap_or(&[]), row.cell_offsets.as_deref().unwrap_or(&[]), row.cell_count, &drop);
        row.cell_storage_buffer = Some(b); row.cell_offsets = Some(o);
        let (pb, po, _) = drop_cells(&row.cell_storage_buffer_pre_bnc, &row.cell_offsets_pre_bnc, row.cell_count, &drop);
        row.cell_storage_buffer_pre_bnc = pb; row.cell_offsets_pre_bnc = po;
        row.cell_count = n;
    }
    let formula = merge_formula(&tm.table_id, rr1, cc1, rr2, cc2);
    let owner = tm.merge_owner.get_or_insert_with(|| tst::MergeOwnerArchive {
        owner_id: cfuuid_from_string(&tm.table_id).unwrap_or_default(),
        formula_store: Some(Default::default()),
    });
    let fs = owner.formula_store.get_or_insert_with(Default::default);
    let fi = fs.next_formula_index;
    fs.formulas.push(tst::formula_store_archive::FormulaStorePair { formula_index: fi, formula });
    fs.next_formula_index = fi + 1;
    // Write table model, tile, and string table back into their own documents.
    let (di, si, oi) = write::find_object(&docs, ident, "").ok_or("找不到表格对象")?;
    docs[di].segs[si].objs[oi] = tm.encode_to_vec();
    let (di, si, oi) = write::find_object(&docs, tile_ref, "").ok_or("找不到 Tile 对象")?;
    docs[di].segs[si].objs[oi] = tile.encode_to_vec();
    if let Some(t) = tdl {
        let (di, si, oi) = write::find_object(&docs, tdl_ref, "").ok_or("找不到字符串表")?;
        docs[di].segs[si].objs[oi] = t.encode_to_vec();
    }
    write::save_docs(src, dst, &docs)?;
    println!("表格 {ident}：已合并 R{}C{}:R{}C{}", rr1+1, cc1+1, rr2+1, cc2+1);
    if !merged_text.is_empty() { println!("  文字串接：{merged_text:?}"); }
    Ok(())
}
