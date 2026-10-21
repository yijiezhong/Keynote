//! Chart grid read/write via the TSCH.ChartDrawableArchive extension field 10000.
use prost::Message;

use crate::iwa::{self, NodeValue};
use crate::{schema, tsch, write};

fn chart_bytes(b: &[u8]) -> Option<tsch::ChartArchive> {
    let nodes = iwa::pb_parse_nodes(b).ok()?;
    for n in &nodes {
        if n.field == 10000 {
            if let NodeValue::Bytes(v) = &n.value {
                return <tsch::ChartArchive as Message>::decode(v.as_slice()).ok();
            }
        }
    }
    None
}

fn charts_of(map: &std::collections::HashMap<u64, (u64, Vec<u8>)>) -> Vec<(u64, tsch::ChartArchive)> {
    let mut out = Vec::new();
    for (id, (t, b)) in map {
        if schema::type_name(*t) != Some("TSCH.ChartDrawableArchive") { continue; }
        if let Some(c) = chart_bytes(b) {
            if c.grid.is_some() { out.push((*id, c)); }
        }
    }
    out.sort_by_key(|(id, _)| *id);
    out
}

pub fn list(path: &str) -> Result<(), String> {
    let docs = write::load_docs(path)?;
    let map = write::ident_map(&docs);
    let charts = charts_of(&map);
    for (id, c) in &charts {
        let g = c.grid.as_ref().ok_or("图表没有 grid")?;
        println!("--- 图表 {id}（{} 行 × {} 列）", g.grid_row.len(), g.column_name.len());
        println!("   行: {:?}", g.row_name);
        println!("   列: {:?}", g.column_name);
        for (ri, row) in g.grid_row.iter().enumerate() {
            let vals: Vec<String> = row.value.iter().map(|v| v.numeric_value.map(|x| x.to_string()).unwrap_or_else(|| "?".into())).collect();
            println!("   行{ri}: {:?}", vals);
        }
    }
    println!("共 {} 个图表", charts.len());
    Ok(())
}

pub fn set_chart(src: &str, dst: &str, id: u64, row: usize, col: usize, value: f64) -> Result<(), String> {
    let docs = write::load_docs(src)?;
    let map = write::ident_map(&docs);
    let (t, b) = map.get(&id).ok_or("找不到图表")?;
    if schema::type_name(*t) != Some("TSCH.ChartDrawableArchive") { return Err("该 ident 不是图表".into()); }
    let mut nodes = iwa::pb_parse_nodes(b).map_err(|e| e.to_string())?;
    let mut chart = None;
    for n in &mut nodes {
        if n.field == 10000 {
            if let NodeValue::Bytes(v) = &n.value {
                chart = <tsch::ChartArchive as Message>::decode(v.as_slice()).ok();
            }
        }
    }
    let mut chart = chart.ok_or("图表没有 ChartArchive 扩展字段")?;
    let g = chart.grid.as_mut().ok_or("图表没有 grid")?;
    let old = g.grid_row.get(row).and_then(|r| r.value.get(col)).and_then(|v| v.numeric_value);
    if old.is_none() { return Err("行列越界或不是数值格".into()); }
    g.grid_row[row].value[col].numeric_value = Some(value);
    chart.is_dirty = Some(true);
    let new_chart = chart.encode_to_vec();
    for n in &mut nodes {
        if n.field == 10000 { n.value = NodeValue::Bytes(new_chart.clone()); }
    }
    let (di, si, oi) = write::find_object(&docs, id, "").ok_or("找不到图表对象")?;
    write::save_rewrite(src, dst, &docs[di].name, si, oi, &iwa::pb_serialize_nodes(&nodes))?;
    println!("图表 {id}：[{row},{col}] {old:?} → {value}");
    Ok(())
}
