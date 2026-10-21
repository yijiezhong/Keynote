//! Character-level text style/color overrides (title/body placeholders).
use std::collections::BTreeMap;
use prost::Message;

use crate::iwa::{self, Node, NodeValue, Segment};
use crate::{schema, tsp, tss, tswp, write};

pub fn rgb_pub(s: &str) -> Result<(f32, f32, f32), String> { rgb(s) }

fn rgb(s: &str) -> Result<(f32, f32, f32), String> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 { return Err("颜色格式应为 #RRGGBB".into()); }
    let r = u8::from_str_radix(&s[0..2], 16).map_err(|_| "颜色格式错误")?;
    let g = u8::from_str_radix(&s[2..4], 16).map_err(|_| "颜色格式错误")?;
    let b = u8::from_str_radix(&s[4..6], 16).map_err(|_| "颜色格式错误")?;
    Ok((r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0))
}

fn max_ident(docs: &[write::Doc]) -> u64 {
    let mut top = 0;
    for d in docs { for s in &d.segs { if let Some(id) = iwa::ident_of(s) { top = top.max(id); } } }
    top
}

fn style_of(map: &std::collections::HashMap<u64, (u64, Vec<u8>)>, id: u64) -> Option<tswp::ParagraphStyleArchive> {
    let (_t, b) = map.get(&id)?;
    <tswp::ParagraphStyleArchive as Message>::decode(b.as_slice()).ok()
}

fn merged_props(map: &std::collections::HashMap<u64, (u64, Vec<u8>)>, ps: Option<&tswp::ParagraphStyleArchive>) -> tswp::CharacterStylePropertiesArchive {
    let mut merged = tswp::CharacterStylePropertiesArchive::default();
    let mut cur = ps.cloned();
    let mut depth = 0;
    let mut chain = Vec::new();
    while let Some(p) = cur {
        chain.push(p.clone());
        depth += 1;
        if depth >= 6 { break; }
        let pid = p.super_.parent.as_ref().map(|r| r.identifier).unwrap_or(0);
        if pid == 0 { break; }
        cur = style_of(map, pid);
    }
    for p in chain.iter().rev() {
        if let Some(cp) = p.char_properties.as_ref() { let b = cp.encode_to_vec(); let _ = merged.merge(b.as_slice()); }
    }
    merged
}

fn base_char_style(map: &std::collections::HashMap<u64, (u64, Vec<u8>)>) -> Option<u64> {
    let mut first = None;
    for (id, (t, b)) in map {
        if schema::type_name(*t) != Some("TSWP.CharacterStyleArchive") { continue; }
        let m = <tswp::CharacterStyleArchive as Message>::decode(b.as_slice()).ok()?;
        if first.is_none() { first = Some(*id); }
        if m.super_.name.as_deref() == Some("无") || m.super_.name.as_deref() == Some("None") { return Some(*id); }
    }
    first
}

fn apply_range(tbl: &mut tswp::ObjectAttributeTable, start: usize, end: usize, style_id: u64, text_len: usize) {
    let mut cur: Vec<(u32, u64)> = tbl.entries.iter().map(|e| (e.character_index, e.object.as_ref().map(|r| r.identifier).unwrap_or(0))).collect();
    cur.sort();
    let mut default = 0u64;
    for (idx, sid) in &cur { if (*idx as usize) <= start { default = *sid; } }
    let mut out: BTreeMap<u32, u64> = BTreeMap::new();
    for (i, s) in &cur { if (*i as usize) < start { out.insert(*i, *s); } }
    if start > 0 && !out.contains_key(&0) { out.insert(0, default); }
    out.insert(start as u32, style_id);
    if end < text_len {
        let existing = cur.iter().find(|(i, _)| *i as usize == end).map(|(_, s)| *s);
        out.insert(end as u32, existing.unwrap_or(default));
    }
    for (i, s) in &cur { if (*i as usize) > end { out.insert(*i, *s); } }
    tbl.entries = out.into_iter().map(|(i, s)| tswp::object_attribute_table::ObjectAttribute {
        character_index: i,
        object: Some(tsp::Reference { identifier: s, ..Default::default() }),
    }).collect();
}

#[allow(clippy::too_many_arguments)]
pub fn apply(
    src: &str,
    dst: &str,
    page: usize,
    which: &str,
    size: Option<f32>,
    bold: Option<bool>,
    italic: Option<bool>,
    color: Option<(f32, f32, f32)>,
    substr: Option<&str>,
) -> Result<(), String> {
    let mut docs = write::load_docs(src)?;
    let map = write::ident_map(&docs);
    let slides = write::slides(&map);
    if page == 0 || page > slides.len() { return Err(format!("页码越界：1..{}", slides.len())); }
    let (title, body) = write::title_body_storage_ids(&map, &slides[page - 1]);
    let sid = match which { "title" => title, "body" => body, _ => return Err("which 必须是 title 或 body".into()) }
        .ok_or_else(|| format!("第 {page} 页没有 {which} 文本"))?;
    let (di, si, oi) = write::find_object(&docs, sid, "storage").ok_or("找不到文本容器")?;
    let mut st = <tswp::StorageArchive as Message>::decode(docs[di].segs[si].objs[oi].as_slice()).map_err(|e| e.to_string())?;
    let para_id = st.table_para_style.as_ref()
        .and_then(|t| t.entries.first())
        .and_then(|e| e.object.as_ref()).map(|r| r.identifier).ok_or("该文本没有段落样式记录")?;
    let ps = style_of(&map, para_id);
    let stylesheet = ps.as_ref().and_then(|p| p.super_.stylesheet.as_ref()).map(|r| r.identifier);
    let mut cp = merged_props(&map, ps.as_ref());
    if let Some(v) = size { cp.font_size = Some(v); }
    if let Some(v) = bold { cp.bold = Some(v); }
    if let Some(v) = italic { cp.italic = Some(v); }
    if let Some((r, g, b)) = color {
        cp.font_color = Some(tsp::Color { model: 1, r: Some(r), g: Some(g), b: Some(b), a: Some(1.0), rgbspace: Some(1), ..Default::default() });
    }
    let base = base_char_style(&map);
    let new_id = max_ident(&docs) + 1;
    let mut sup = tss::StyleArchive::default();
    sup.name = Some(format!("自定义格式 {}#{:02X}{:02X}{:02X}", new_id, (color.map(|c| c.0).unwrap_or(0.0) * 255.0) as u8, (color.map(|c| c.1).unwrap_or(0.0) * 255.0) as u8, (color.map(|c| c.2).unwrap_or(0.0) * 255.0) as u8));
    sup.parent = base.map(|id| tsp::Reference { identifier: id, ..Default::default() });
    sup.is_variation = Some(true);
    sup.stylesheet = stylesheet.map(|id| tsp::Reference { identifier: id, ..Default::default() });
    let cs = tswp::CharacterStyleArchive { super_: sup, override_count: Some(4), char_properties: Some(cp) };
    let cs_blob = cs.encode_to_vec();

    let full = st.text.join("");
    let text_len = full.chars().count();
    let (start, end) = if let Some(sub) = substr {
        let pos = full.find(sub).ok_or_else(|| format!("文字里找不到「{sub}」：{full:?}"))?;
        (full[..pos].chars().count(), full[..pos + sub.len()].chars().count())
    } else { (0, text_len) };
    let tbl = st.table_char_style.get_or_insert_with(tswp::ObjectAttributeTable::default);
    apply_range(tbl, start, end, new_id, text_len);
    docs[di].segs[si].objs[oi] = st.encode_to_vec();

    let type_id = schema::type_id("TSWP.CharacterStyleArchive").ok_or("registry 缺少 CharacterStyleArchive")?;
    let mi = vec![
        Node { field: 1, value: NodeValue::Varint(type_id) },
        Node { field: 2, value: NodeValue::Bytes(vec![1, 0, 5]) },
        Node { field: 3, value: NodeValue::Varint(cs_blob.len() as u64) },
    ];
    let ainfo = vec![
        Node { field: 1, value: NodeValue::Varint(new_id) },
        Node { field: 2, value: NodeValue::Bytes(iwa::pb_serialize_nodes(&mi)) },
    ];
    docs[di].segs.push(Segment { ainfo, objs: vec![cs_blob], meta: vec![(type_id, 0)] });
    write::save_docs(src, dst, &docs)?;
    println!("第 {page} 页 {which} 文字样式已更新（新字符样式 ident={new_id}）");
    Ok(())
}

pub fn set_color(src: &str, dst: &str, page: usize, which: &str, hex: &str) -> Result<(), String> {
    let (r, g, b) = rgb(hex)?;
    apply(src, dst, page, which, None, None, None, Some((r, g, b)), None)
}
