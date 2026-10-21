//! Table commands: list tables and set a numeric cell.
use prost::Message;

use crate::{tst, write};

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
