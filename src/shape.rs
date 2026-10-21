//! Slide background and drawable geometry.
use std::collections::HashMap;
use prost::Message;

use crate::{kn, schema, tsd, tsp, tst, tswp, write};

fn parse_rgb(s: &str) -> Result<(f32, f32, f32), String> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 { return Err("颜色格式应为 #RRGGBB".into()); }
    let r = u8::from_str_radix(&s[0..2], 16).map_err(|_| "颜色格式错误")?;
    let g = u8::from_str_radix(&s[2..4], 16).map_err(|_| "颜色格式错误")?;
    let b = u8::from_str_radix(&s[4..6], 16).map_err(|_| "颜色格式错误")?;
    Ok((r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0))
}

pub fn set_bg(src: &str, dst: &str, page: usize, rgb: &str) -> Result<(), String> {
    let (r, g, b) = parse_rgb(rgb)?;
    let docs = write::load_docs(src)?;
    let map = write::ident_map(&docs);
    let slides = write::slides(&map);
    if page == 0 || page > slides.len() { return Err(format!("页码越界：1..{}", slides.len())); }
    let sid = slides[page - 1].style.identifier;
    let (di, si, oi) = write::find_object(&docs, sid, "slidestyle").ok_or("未找到该页 SlideStyleArchive")?;
    let mut st = <kn::SlideStyleArchive as Message>::decode(docs[di].segs[si].objs[oi].as_slice())
        .map_err(|e| e.to_string())?;
    let props = st.slide_properties.as_mut().ok_or("该页没有 slide_properties")?;
    let fill = props.fill.as_mut().ok_or("该页没有填充")?;
    let c = fill.color.as_mut().ok_or("该页背景不是纯色填充")?;
    c.model = 1; c.r = Some(r); c.g = Some(g); c.b = Some(b); c.a = Some(1.0); c.rgbspace = Some(1);
    write::save_rewrite(src, dst, &docs[di].name, si, oi, &st.encode_to_vec())?;
    println!("第 {page} 页背景样式 {sid} → {}", rgb);
    Ok(())
}

fn child_ids(map: &HashMap<u64, (u64, Vec<u8>)>, id: u64) -> Option<Vec<u64>> {
    let (t, b) = map.get(&id)?;
    if schema::type_name(*t) != Some("TSD.GroupArchive") { return None; }
    let g = <tsd::GroupArchive as Message>::decode(b.as_slice()).ok()?;
    Some(g.children.iter().map(|r| r.identifier).collect())
}

fn resolve(map: &HashMap<u64, (u64, Vec<u8>)>, sa: &kn::SlideArchive, path: &str) -> Option<u64> {
    let parts: Vec<usize> = path.split('/').filter_map(|s| s.parse().ok()).collect();
    if parts.is_empty() { return None; }
    let mut list: Vec<u64> = sa.owned_drawables.iter().map(|r| r.identifier).collect();
    let mut cur = *list.get(parts[0].checked_sub(1)?)?;
    for p in &parts[1..] {
        list = child_ids(map, cur)?;
        cur = *list.get(p.checked_sub(1)?)?;
    }
    Some(cur)
}

pub fn set_geometry(src: &str, dst: &str, page: usize, path: &str, x: Option<f32>, y: Option<f32>, w: Option<f32>, h: Option<f32>, angle: Option<f32>) -> Result<(), String> {
    let mut docs = write::load_docs(src)?;
    let map = write::ident_map(&docs);
    let slides = write::slides(&map);
    if page == 0 || page > slides.len() { return Err(format!("页码越界：1..{}", slides.len())); }
    let id = resolve(&map, &slides[page - 1], path).ok_or("找不到该形状路径")?;
    let (di, si, oi) = write::find_object(&docs, id, "").ok_or("找不到形状对象")?;
    let typ = docs[di].segs[si].meta.get(oi).map(|(t, _)| *t).unwrap_or(0);
    let name = schema::type_name(typ).unwrap_or("");
    let mut geo = match name {
        "TSWP.ShapeInfoArchive" => {
            let mut m = <tswp::ShapeInfoArchive as Message>::decode(docs[di].segs[si].objs[oi].as_slice()).map_err(|e| e.to_string())?;
            let g = m.super_.super_.geometry.take().ok_or("形状没有 geometry")?;
            let out = g.clone();
            m.super_.super_.geometry = Some(g);
            docs[di].segs[si].objs[oi] = m.encode_to_vec();
            out
        }
        "TST.TableInfoArchive" => {
            let mut m = <tst::TableInfoArchive as Message>::decode(docs[di].segs[si].objs[oi].as_slice()).map_err(|e| e.to_string())?;
            let g = m.super_.geometry.take().ok_or("表格没有 geometry")?;
            let out = g.clone();
            m.super_.geometry = Some(g);
            docs[di].segs[si].objs[oi] = m.encode_to_vec();
            out
        }
        "KN.PlaceholderArchive" => {
            let mut m = <kn::PlaceholderArchive as Message>::decode(docs[di].segs[si].objs[oi].as_slice()).map_err(|e| e.to_string())?;
            let g = m.super_.super_.super_.geometry.take().ok_or("占位符没有 geometry")?;
            let out = g.clone();
            m.super_.super_.super_.geometry = Some(g);
            docs[di].segs[si].objs[oi] = m.encode_to_vec();
            out
        }
        _ => return Err(format!("暂不支持的形状类型：{name}")),
    };
    if let Some(v) = x { if let Some(p) = geo.position.as_mut() { p.x = v; } }
    if let Some(v) = y { if let Some(p) = geo.position.as_mut() { p.y = v; } }
    if let Some(v) = w { if let Some(s) = geo.size.as_mut() { s.width = v; } }
    if let Some(v) = h { if let Some(s) = geo.size.as_mut() { s.height = v; } }
    if let Some(v) = angle { geo.angle = Some(v); }
    // 重新写入刚改的 geometry（上面 clone 仅用于取字段）
    let typ = docs[di].segs[si].meta.get(oi).map(|(t, _)| *t).unwrap_or(0);
    match schema::type_name(typ).unwrap_or("") {
        "TSWP.ShapeInfoArchive" => {
            let mut m = <tswp::ShapeInfoArchive as Message>::decode(docs[di].segs[si].objs[oi].as_slice()).map_err(|e| e.to_string())?;
            m.super_.super_.geometry = Some(geo);
            docs[di].segs[si].objs[oi] = m.encode_to_vec();
        }
        "TST.TableInfoArchive" => {
            let mut m = <tst::TableInfoArchive as Message>::decode(docs[di].segs[si].objs[oi].as_slice()).map_err(|e| e.to_string())?;
            m.super_.geometry = Some(geo);
            docs[di].segs[si].objs[oi] = m.encode_to_vec();
        }
        "KN.PlaceholderArchive" => {
            let mut m = <kn::PlaceholderArchive as Message>::decode(docs[di].segs[si].objs[oi].as_slice()).map_err(|e| e.to_string())?;
            m.super_.super_.super_.geometry = Some(geo);
            docs[di].segs[si].objs[oi] = m.encode_to_vec();
        }
        _ => {}
    }
    write::save_docs(src, dst, &docs)?;
    println!("第 {page} 页形状 {id}（path={path}）几何已更新");
    Ok(())
}
