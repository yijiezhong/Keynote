//! Slide background and drawable geometry.
use std::collections::HashMap;
use prost::Message;

use crate::iwa::{self, Node, NodeValue, Segment};
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


pub fn resolve_path(map: &HashMap<u64, (u64, Vec<u8>)>, sa: &kn::SlideArchive, path: &str) -> Option<u64> {
    resolve(map, sa, path)
}

pub fn set_fill(src: &str, dst: &str, page: usize, path: &str, hex: &str) -> Result<(), String> {
    let (r, g, b) = parse_rgb(hex)?;
    let docs = write::load_docs(src)?;
    let map = write::ident_map(&docs);
    let slides = write::slides(&map);
    if page == 0 || page > slides.len() { return Err(format!("页码越界：1..{}", slides.len())); }
    let id = resolve_path(&map, &slides[page - 1], path).ok_or("找不到该形状路径")?;
    let typ = map.get(&id).map(|(t, _)| *t).unwrap_or(0);
    let name = schema::type_name(typ).unwrap_or("");
    let style_id = match name {
        "TSWP.ShapeInfoArchive" => {
            let m = <tswp::ShapeInfoArchive as Message>::decode(map.get(&id).unwrap().1.as_slice()).map_err(|e| e.to_string())?;
            m.super_.style.as_ref().map(|r| r.identifier)
        }
        "KN.PlaceholderArchive" => {
            let m = <kn::PlaceholderArchive as Message>::decode(map.get(&id).unwrap().1.as_slice()).map_err(|e| e.to_string())?;
            m.super_.super_.style.as_ref().map(|r| r.identifier)
        }
        _ => None,
    }.ok_or_else(|| format!("{name} 没有可编辑的填充样式"))?;
    let (di, si, oi) = write::find_object(&docs, style_id, "shapestyle").ok_or("找不到 ShapeStyleArchive")?;
    let mut st = <tswp::ShapeStyleArchive as Message>::decode(docs[di].segs[si].objs[oi].as_slice()).map_err(|e| e.to_string())?;
    let props = st.super_.shape_properties.as_mut().ok_or("形状样式没有 shape_properties")?;
    let fill = props.fill.as_mut().ok_or("形状样式没有填充")?;
    let c = fill.color.as_mut().ok_or("该形状不是纯色填充")?;
    c.model = 1; c.r = Some(r); c.g = Some(g); c.b = Some(b); c.a = Some(1.0); c.rgbspace = Some(1);
    write::save_rewrite(src, dst, &docs[di].name, si, oi, &st.encode_to_vec())?;
    println!("第 {page} 页形状 {path}（{name}）填充 → {hex}");
    Ok(())
}

pub fn delete_shape(src: &str, dst: &str, page: usize, path: &str) -> Result<(), String> {
    if path.contains('/') { return Err("del-shape 暂只支持顶层路径".into()); }
    let docs = write::load_docs(src)?;
    let map = write::ident_map(&docs);
    let slides = write::slides(&map);
    if page == 0 || page > slides.len() { return Err(format!("页码越界：1..{}", slides.len())); }
    let id = resolve_path(&map, &slides[page - 1], path).ok_or("找不到该形状路径")?;
    let ids = write::slide_node_ids(&map);
    let node = <kn::SlideNodeArchive as Message>::decode(map.get(&ids[page - 1]).ok_or("节点不存在")?.1.as_slice()).map_err(|e| e.to_string())?;
    let slide_id = node.slide.as_ref().ok_or("该页没有 SlideArchive")?.identifier;
    let (di, si, oi) = write::find_object(&docs, slide_id, "slidearchive").ok_or("找不到 SlideArchive")?;
    let mut sa = <kn::SlideArchive as Message>::decode(docs[di].segs[si].objs[oi].as_slice()).map_err(|e| e.to_string())?;
    sa.owned_drawables.retain(|r| r.identifier != id);
    sa.drawables_z_order.retain(|r| r.identifier != id);
    write::save_rewrite(src, dst, &docs[di].name, si, oi, &sa.encode_to_vec())?;
    println!("第 {page} 页已移除 drawable {id}（path={path}）");
    Ok(())
}


fn max_ident(docs: &[write::Doc]) -> u64 {
    let mut top = 0;
    for d in docs { for s in &d.segs { if let Some(id) = iwa::ident_of(s) { top = top.max(id); } } }
    top
}

fn geometry_of(map: &HashMap<u64, (u64, Vec<u8>)>, id: u64) -> Option<tsd::GeometryArchive> {
    let (t, b) = map.get(&id)?;
    match schema::type_name(*t)? {
        "TSWP.ShapeInfoArchive" => <tswp::ShapeInfoArchive as Message>::decode(b.as_slice()).ok()?.super_.super_.geometry,
        "KN.PlaceholderArchive" => <kn::PlaceholderArchive as Message>::decode(b.as_slice()).ok()?.super_.super_.super_.geometry,
        "TST.TableInfoArchive" => <tst::TableInfoArchive as Message>::decode(b.as_slice()).ok()?.super_.geometry,
        "TSD.ImageArchive" => <tsd::ImageArchive as Message>::decode(b.as_slice()).ok()?.super_.geometry,
        _ => None,
    }
}

fn set_parent(docs: &mut [write::Doc], id: u64, parent: u64) -> Result<(), String> {
    let (di, si, oi) = write::find_object(docs, id, "").ok_or("找不到子对象")?;
    let typ = docs[di].segs[si].meta.get(oi).map(|(t, _)| *t).unwrap_or(0);
    let name = schema::type_name(typ).unwrap_or("");
    let r = tsp::Reference { identifier: parent, ..Default::default() };
    let blob = docs[di].segs[si].objs[oi].clone();
    let new_blob = match name {
        "TSWP.ShapeInfoArchive" => { let mut m = <tswp::ShapeInfoArchive as Message>::decode(blob.as_slice()).map_err(|e| e.to_string())?; m.super_.super_.parent = Some(r); m.encode_to_vec() }
        "KN.PlaceholderArchive" => { let mut m = <kn::PlaceholderArchive as Message>::decode(blob.as_slice()).map_err(|e| e.to_string())?; m.super_.super_.super_.parent = Some(r); m.encode_to_vec() }
        "TST.TableInfoArchive" => { let mut m = <tst::TableInfoArchive as Message>::decode(blob.as_slice()).map_err(|e| e.to_string())?; m.super_.parent = Some(r); m.encode_to_vec() }
        "TSD.ImageArchive" => { let mut m = <tsd::ImageArchive as Message>::decode(blob.as_slice()).map_err(|e| e.to_string())?; m.super_.parent = Some(r); m.encode_to_vec() }
        _ => return Err(format!("暂不支持调整 {name} 的 parent")),
    };
    docs[di].segs[si].objs[oi] = new_blob;
    Ok(())
}

pub fn group(src: &str, dst: &str, page: usize, paths: &str) -> Result<(), String> {
    let mut docs = write::load_docs(src)?;
    let map = write::ident_map(&docs);
    let slides = write::slides(&map);
    if page == 0 || page > slides.len() { return Err(format!("页码越界：1..{}", slides.len())); }
    let sa = &slides[page - 1];
    let ids = write::slide_node_ids(&map);
    let node = <kn::SlideNodeArchive as Message>::decode(map.get(&ids[page - 1]).ok_or("节点不存在")?.1.as_slice()).map_err(|e| e.to_string())?;
    let page_id = node.slide.as_ref().ok_or("该页没有 SlideArchive")?.identifier;
    let mut targets = Vec::new();
    let mut x = f64::INFINITY; let mut y = f64::INFINITY; let mut x2 = f64::NEG_INFINITY; let mut y2 = f64::NEG_INFINITY;
    for part in paths.split(',') {
        let id = resolve_path(&map, sa, part.trim()).ok_or_else(|| format!("找不到路径 {part}"))?;
        targets.push(id);
        if let Some(g) = geometry_of(&map, id) {
            if let (Some(p), Some(sz)) = (g.position.as_ref(), g.size.as_ref()) {
                x = x.min(p.x as f64); y = y.min(p.y as f64);
                x2 = x2.max((p.x + sz.width) as f64); y2 = y2.max((p.y + sz.height) as f64);
            }
        }
    }
    if targets.len() < 2 { return Err("至少选两个形状".into()); }
    let group_id = max_ident(&docs) + 1;
    let mut geo = tsd::GeometryArchive::default();
    if x.is_finite() {
        geo.position = Some(tsp::Point { x: x as f32, y: y as f32 });
        geo.size = Some(tsp::Size { width: (x2 - x) as f32, height: (y2 - y) as f32 });
        geo.flags = Some(3);
    }
    let g = tsd::GroupArchive {
        super_: tsd::DrawableArchive { geometry: Some(geo), parent: Some(tsp::Reference { identifier: page_id, ..Default::default() }), ..Default::default() },
        children: targets.iter().map(|id| tsp::Reference { identifier: *id, ..Default::default() }).collect(),
        fake_shape_for_empty_group: None,
    };
    let (di, si, oi) = write::find_object(&docs, page_id, "slidearchive").ok_or("找不到页对象")?;
    let mut page_archive = <kn::SlideArchive as Message>::decode(docs[di].segs[si].objs[oi].as_slice()).map_err(|e| e.to_string())?;
    let oids: Vec<u64> = page_archive.owned_drawables.iter().map(|r| r.identifier).collect();
    let first = oids.iter().position(|id| targets.contains(id)).unwrap_or(oids.len());
    let mut keep: Vec<u64> = oids.into_iter().filter(|id| !targets.contains(id)).collect();
    keep.insert(first.min(keep.len()), group_id);
    page_archive.owned_drawables = keep.into_iter().map(|id| tsp::Reference { identifier: id, ..Default::default() }).collect();
    let zids: Vec<u64> = page_archive.drawables_z_order.iter().map(|r| r.identifier).collect();
    let zfirst = zids.iter().position(|id| targets.contains(id)).unwrap_or(zids.len());
    let mut zkeep: Vec<u64> = zids.into_iter().filter(|id| !targets.contains(id)).collect();
    zkeep.insert(zfirst.min(zkeep.len()), group_id);
    page_archive.drawables_z_order = zkeep.into_iter().map(|id| tsp::Reference { identifier: id, ..Default::default() }).collect();
    docs[di].segs[si].objs[oi] = page_archive.encode_to_vec();
    let type_id = schema::type_id("TSD.GroupArchive").ok_or("registry 缺少 GroupArchive")?;
    let blob = g.encode_to_vec();
    let mi = vec![
        Node { field: 1, value: NodeValue::Varint(type_id) },
        Node { field: 2, value: NodeValue::Bytes(vec![1, 0, 5]) },
        Node { field: 3, value: NodeValue::Varint(blob.len() as u64) },
    ];
    let ainfo = vec![
        Node { field: 1, value: NodeValue::Varint(group_id) },
        Node { field: 2, value: NodeValue::Bytes(iwa::pb_serialize_nodes(&mi)) },
    ];
    docs[di].segs.push(Segment { ainfo, objs: vec![blob], meta: vec![(type_id, 0)] });
    for id in &targets { set_parent(&mut docs, *id, group_id)?; }
    write::save_docs(src, dst, &docs)?;
    println!("第 {page} 页：已把 {} 个形状打成组 {group_id}", targets.len());
    Ok(())
}

pub fn ungroup(src: &str, dst: &str, page: usize, path: &str) -> Result<(), String> {
    let mut docs = write::load_docs(src)?;
    let map = write::ident_map(&docs);
    let slides = write::slides(&map);
    if page == 0 || page > slides.len() { return Err(format!("页码越界：1..{}", slides.len())); }
    let id = resolve_path(&map, &slides[page - 1], path).ok_or("找不到组路径")?;
    let (t, b) = map.get(&id).ok_or("找不到组对象")?;
    if schema::type_name(*t) != Some("TSD.GroupArchive") { return Err(format!("路径 {path} 不是组")); }
    let g = <tsd::GroupArchive as Message>::decode(b.as_slice()).map_err(|e| e.to_string())?;
    let kids: Vec<u64> = g.children.iter().map(|r| r.identifier).collect();
    let ids = write::slide_node_ids(&map);
    let node = <kn::SlideNodeArchive as Message>::decode(map.get(&ids[page - 1]).ok_or("节点不存在")?.1.as_slice()).map_err(|e| e.to_string())?;
    let page_id = node.slide.as_ref().ok_or("该页没有 SlideArchive")?.identifier;
    let (di, si, oi) = write::find_object(&docs, page_id, "slidearchive").ok_or("找不到页对象")?;
    let mut sa = <kn::SlideArchive as Message>::decode(docs[di].segs[si].objs[oi].as_slice()).map_err(|e| e.to_string())?;
    let oids: Vec<u64> = sa.owned_drawables.iter().map(|r| r.identifier).collect();
    let pos = oids.iter().position(|x| *x == id).unwrap_or(oids.len());
    let mut out: Vec<u64> = oids.into_iter().filter(|x| *x != id).collect();
    for (off, k) in kids.iter().enumerate() { out.insert((pos + off).min(out.len()), *k); }
    sa.owned_drawables = out.into_iter().map(|x| tsp::Reference { identifier: x, ..Default::default() }).collect();
    let zids: Vec<u64> = sa.drawables_z_order.iter().map(|r| r.identifier).collect();
    let zpos = zids.iter().position(|x| *x == id).unwrap_or(zids.len());
    let mut zout: Vec<u64> = zids.into_iter().filter(|x| *x != id).collect();
    for (off, k) in kids.iter().enumerate() { zout.insert((zpos + off).min(zout.len()), *k); }
    sa.drawables_z_order = zout.into_iter().map(|x| tsp::Reference { identifier: x, ..Default::default() }).collect();
    docs[di].segs[si].objs[oi] = sa.encode_to_vec();
    for k in &kids { set_parent(&mut docs, *k, page_id)?; }
    write::save_docs(src, dst, &docs)?;
    println!("第 {page} 页：已拆组 {id}，释放 {} 个形状", kids.len());
    Ok(())
}


pub fn duplicate_shape(src: &str, dst: &str, page: usize, path: &str, dx: f32, dy: f32) -> Result<(), String> {
    let mut docs = write::load_docs(src)?;
    let map = write::ident_map(&docs);
    let slides = write::slides(&map);
    if page == 0 || page > slides.len() { return Err(format!("页码越界：1..{}", slides.len())); }
    let id = resolve_path(&map, &slides[page - 1], path).ok_or("找不到该形状路径")?;
    let (typ, old) = map.get(&id).ok_or("找不到对象")?;
    let name = schema::type_name(*typ).unwrap_or("");
    let ids = write::slide_node_ids(&map);
    let node = <kn::SlideNodeArchive as Message>::decode(map.get(&ids[page - 1]).ok_or("节点不存在")?.1.as_slice()).map_err(|e| e.to_string())?;
    let page_id = node.slide.as_ref().ok_or("该页没有 SlideArchive")?.identifier;
    let new_id = max_ident(&docs) + 1;
    let pref = tsp::Reference { identifier: page_id, ..Default::default() };
    fn bump(g: &mut Option<tsd::GeometryArchive>, dx: f32, dy: f32) {
        let geo = g.get_or_insert_with(|| tsd::GeometryArchive { position: None, size: None, flags: None, angle: None });
        if let Some(p) = geo.position.as_mut() { p.x += dx; p.y += dy; } else { geo.position = Some(tsp::Point { x: dx, y: dy }); }
    }
    let blob = match name {
        "TSD.ImageArchive" => {
            let mut m = <tsd::ImageArchive as Message>::decode(old.as_slice()).map_err(|e| e.to_string())?;
            m.super_.parent = Some(pref); bump(&mut m.super_.geometry, dx, dy); m.encode_to_vec()
        }
        "TST.TableInfoArchive" => {
            let mut m = <tst::TableInfoArchive as Message>::decode(old.as_slice()).map_err(|e| e.to_string())?;
            m.super_.parent = Some(pref); bump(&mut m.super_.geometry, dx, dy); m.encode_to_vec()
        }
        "TSWP.ShapeInfoArchive" => {
            let mut m = <tswp::ShapeInfoArchive as Message>::decode(old.as_slice()).map_err(|e| e.to_string())?;
            m.super_.super_.parent = Some(pref); bump(&mut m.super_.super_.geometry, dx, dy); m.encode_to_vec()
        }
        _ => return Err(format!("dup-shape 暂不支持 {name}")),
    };
    let (di, si, oi) = write::find_object(&docs, page_id, "slidearchive").ok_or("找不到页对象")?;
    let mut sa = <kn::SlideArchive as Message>::decode(docs[di].segs[si].objs[oi].as_slice()).map_err(|e| e.to_string())?;
    sa.owned_drawables.push(tsp::Reference { identifier: new_id, ..Default::default() });
    sa.drawables_z_order.push(tsp::Reference { identifier: new_id, ..Default::default() });
    docs[di].segs[si].objs[oi] = sa.encode_to_vec();
    let mi = vec![
        Node { field: 1, value: NodeValue::Varint(*typ) },
        Node { field: 2, value: NodeValue::Bytes(vec![1, 0, 5]) },
        Node { field: 3, value: NodeValue::Varint(blob.len() as u64) },
    ];
    let ainfo = vec![
        Node { field: 1, value: NodeValue::Varint(new_id) },
        Node { field: 2, value: NodeValue::Bytes(iwa::pb_serialize_nodes(&mi)) },
    ];
    docs[di].segs.push(Segment { ainfo, objs: vec![blob], meta: vec![(*typ, 0)] });
    write::save_docs(src, dst, &docs)?;
    println!("第 {page} 页：已复制 {name}（{id} → {new_id}，offset {dx},{dy}）");
    Ok(())
}
