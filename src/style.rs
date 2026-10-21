//! Style listing and character-property edits.
use prost::Message;

use crate::write;
use crate::{schema, tsp, tss, tswp};

enum Style {
    Para(tswp::ParagraphStyleArchive),
    Char(tswp::CharacterStyleArchive),
    List(tswp::ListStyleArchive),
}

fn kind_name(k: &Style) -> &'static str {
    match k { Style::Para(_) => "段落", Style::Char(_) => "字符", Style::List(_) => "列表" }
}
fn super_of(k: &Style) -> &tss::StyleArchive {
    match k { Style::Para(m) => &m.super_, Style::Char(m) => &m.super_, Style::List(m) => &m.super_ }
}
fn cp_of(k: &Style) -> Option<&tswp::CharacterStylePropertiesArchive> {
    match k { Style::Para(m) => m.char_properties.as_ref(), Style::Char(m) => m.char_properties.as_ref(), Style::List(_) => None }
}
fn cp_mut(k: &mut Style) -> Option<&mut tswp::CharacterStylePropertiesArchive> {
    match k { Style::Para(m) => m.char_properties.as_mut(), Style::Char(m) => m.char_properties.as_mut(), Style::List(_) => None }
}
fn decode_style(t: u64, b: &[u8]) -> Option<Style> {
    match schema::type_name(t)? {
        "TSWP.ParagraphStyleArchive" => <tswp::ParagraphStyleArchive as Message>::decode(b).ok().map(Style::Para),
        "TSWP.CharacterStyleArchive" => <tswp::CharacterStyleArchive as Message>::decode(b).ok().map(Style::Char),
        "TSWP.ListStyleArchive" => <tswp::ListStyleArchive as Message>::decode(b).ok().map(Style::List),
        _ => None,
    }
}
fn encode_style(k: &Style) -> Vec<u8> {
    match k { Style::Para(m) => m.encode_to_vec(), Style::Char(m) => m.encode_to_vec(), Style::List(m) => m.encode_to_vec() }
}
fn hex(c: &tsp::Color) -> String {
    let f = |x: Option<f32>| ((x.unwrap_or(0.0).clamp(0.0, 1.0)) * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", f(c.r), f(c.g), f(c.b))
}

pub fn styles(path: &str, keyword: Option<&str>) -> Result<(), String> {
    let docs = write::load_docs(path)?;
    let map = write::ident_map(&docs);
    let mut rows = Vec::new();
    for (id, (_t, b)) in &map {
        let Some(k) = decode_style(*_t, b) else { continue };
        let s = super_of(&k);
        let name = s.name.clone().unwrap_or_default();
        let sid = s.style_identifier.clone().unwrap_or_default();
        let mut attrs = Vec::new();
        if let Some(cp) = cp_of(&k) {
            if let Some(v) = cp.font_size { attrs.push(format!("size={v}")); }
            if let Some(v) = cp.bold { attrs.push(format!("bold={v}")); }
            if let Some(v) = cp.italic { attrs.push(format!("italic={v}")); }
            if let Some(c) = &cp.font_color { attrs.push(format!("color={}", hex(c))); }
        }
        let kind = kind_name(&k);
        if let Some(kw) = keyword {
            if !name.contains(kw) && !sid.contains(kw) && !kind.contains(kw) { continue; }
        }
        rows.push((*id, kind.to_string(), name, sid, attrs.join(" ")));
    }
    rows.sort_by(|a, b| (a.1.clone(), a.2.clone()).cmp(&(b.1.clone(), b.2.clone())));
    for (id, kind, name, sid, attrs) in &rows {
        println!("{id:>10}  {kind}  {name:<14} {sid:<44} {attrs}");
    }
    println!("共 {} 个样式", rows.len());
    Ok(())
}

fn parse_rgb(s: &str) -> Result<(f32, f32, f32), String> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 { return Err("颜色格式应为 #RRGGBB".into()); }
    let r = u8::from_str_radix(&s[0..2], 16).map_err(|_| "颜色格式错误")?;
    let g = u8::from_str_radix(&s[2..4], 16).map_err(|_| "颜色格式错误")?;
    let b = u8::from_str_radix(&s[4..6], 16).map_err(|_| "颜色格式错误")?;
    Ok((r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0))
}

pub fn set_style(src: &str, dst: &str, ident: u64, spec: &str) -> Result<(), String> {
    let (prop, value) = spec.split_once('=').ok_or("属性写法：font_size=48 / bold=true / font_color=#FF0000")?;
    let docs = write::load_docs(src)?;
    let mut found = None;
    for (di, d) in docs.iter().enumerate() {
        for (si, seg) in d.segs.iter().enumerate() {
            if crate::iwa::ident_of(seg) != Some(ident) { continue; }
            for (oi, obj) in seg.objs.iter().enumerate() {
                if let Some(k) = decode_style(seg.meta.get(oi).map(|(t, _)| *t).unwrap_or(0), obj) { found = Some((di, si, oi, k)); break; }
            }
            if found.is_some() { break; }
        }
        if found.is_some() { break; }
    }
    let (di, si, oi, mut k) = found.ok_or(format!("找不到样式 {ident}"))?;
    let cp = cp_mut(&mut k).ok_or("该样式没有 char_properties")?;
    match prop {
        "font_size" => cp.font_size = Some(value.parse().map_err(|_| "font_size 不是数字")?),
        "bold" => cp.bold = Some(value == "true" || value == "1" || value == "yes"),
        "italic" => cp.italic = Some(value == "true" || value == "1" || value == "yes"),
        "font_color" => {
            let (r, g, b) = parse_rgb(value)?;
            cp.font_color = Some(tsp::Color {
                model: 1,
                r: Some(r), g: Some(g), b: Some(b), a: Some(1.0), rgbspace: Some(1),
                ..Default::default()
            });
        }
        _ => return Err(format!("不支持的属性：{prop}")),
    }
    let name = super_of(&k).name.clone().unwrap_or_default();
    write::save_rewrite(src, dst, &docs[di].name, si, oi, &encode_style(&k))?;
    println!("样式 {ident}「{name}」：{prop} → {value}");
    Ok(())
}
