//! Inline TSP.Color discovery and editing.
use prost::Message;

use crate::iwa::{self, Node, NodeValue};
use crate::{tsp, write};

#[derive(Clone)]
pub struct ColorHit {
    pub doc: usize,
    pub seg: usize,
    pub obj: usize,
    pub path: Vec<usize>,
}

fn walk(nodes: &[Node], base: &[usize], doc: usize, seg: usize, obj: usize, out: &mut Vec<ColorHit>) {
    for (i, n) in nodes.iter().enumerate() {
        let NodeValue::Bytes(b) = &n.value else { continue };
        if let Ok(m) = <tsp::Color as Message>::decode(b.as_slice()) {
            if m.model == 1 && m.r.is_some() && m.g.is_some() && m.b.is_some() {
                let mut path = base.to_vec();
                path.push(i);
                out.push(ColorHit { doc, seg, obj, path });
                continue;
            }
        }
        if let Ok(sub) = iwa::pb_parse_nodes(b) {
            let mut path = base.to_vec();
            path.push(i);
            walk(&sub, &path, doc, seg, obj, out);
        }
    }
}

fn collect(docs: &[write::Doc]) -> Vec<ColorHit> {
    let mut out = Vec::new();
    for (di, d) in docs.iter().enumerate() {
        for (si, seg) in d.segs.iter().enumerate() {
            for (oi, obj) in seg.objs.iter().enumerate() {
                if let Ok(nodes) = iwa::pb_parse_nodes(obj) {
                    walk(&nodes, &[], di, si, oi, &mut out);
                }
            }
        }
    }
    out
}

fn get_color(docs: &[write::Doc], hit: &ColorHit) -> Option<tsp::Color> {
    let mut nodes = iwa::pb_parse_nodes(&docs[hit.doc].segs[hit.seg].objs[hit.obj]).ok()?;
    for idx in &hit.path {
        let NodeValue::Bytes(b) = &nodes[*idx].value else { return None };
        if !matches!(nodes[*idx].value, NodeValue::Bytes(_)) { return None; }
        let is_color = if let Ok(m) = <tsp::Color as Message>::decode(b.as_slice()) {
            m.model == 1 && m.r.is_some() && m.g.is_some() && m.b.is_some()
        } else { false };
        if is_color {
            let NodeValue::Bytes(b) = &nodes[*idx].value else { return None };
            return <tsp::Color as Message>::decode(b.as_slice()).ok();
        }
        nodes = iwa::pb_parse_nodes(b).ok()?;
    }
    None
}

fn hex(c: &tsp::Color) -> String {
    let f = |x: Option<f32>| ((x.unwrap_or(0.0).clamp(0.0, 1.0)) * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", f(c.r), f(c.g), f(c.b))
}

pub fn colors(path: &str) -> Result<(), String> {
    let docs = write::load_docs(path)?;
    let hits = collect(&docs);
    println!("内联 RGB 颜色实例 {} 个", hits.len());
    for (i, h) in hits.iter().enumerate() {
        let c = get_color(&docs, h).ok_or("读取颜色失败")?;
        let ident = iwa::ident_of(&docs[h.doc].segs[h.seg]).unwrap_or(0);
        println!("{:5}  {}  a={:.3}  ident={}  path={:?}", i + 1, hex(&c), c.a.unwrap_or(1.0), ident, h.path);
    }
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

pub fn set_color(src: &str, dst: &str, index: usize, rgb: &str, alpha: Option<f32>) -> Result<(), String> {
    let (r, g, b) = parse_rgb(rgb)?;
    let mut docs = write::load_docs(src)?;
    let hits = collect(&docs);
    if index == 0 || index > hits.len() {
        return Err(format!("序号越界：1..{}", hits.len()));
    }
    let hit = hits[index - 1].clone();
    let mut nodes = iwa::pb_parse_nodes(&docs[hit.doc].segs[hit.seg].objs[hit.obj])
        .map_err(|e| e.to_string())?;
    fn set_at(nodes: &mut Vec<Node>, path: &[usize], r: f32, g: f32, b: f32, alpha: Option<f32>) -> Result<tsp::Color, String> {
        if path.len() == 1 {
            let NodeValue::Bytes(bytes) = &nodes[path[0]].value else { return Err("路径不是字段".into()) };
            let mut c = <tsp::Color as Message>::decode(bytes.as_slice()).map_err(|e| e.to_string())?;
            c.r = Some(r); c.g = Some(g); c.b = Some(b);
            if let Some(a) = alpha { c.a = Some(a); }
            nodes[path[0]].value = NodeValue::Bytes(c.encode_to_vec());
            return Ok(c);
        }
        let NodeValue::Bytes(bytes) = &nodes[path[0]].value else { return Err("路径不是嵌套字段".into()) };
        let mut sub = iwa::pb_parse_nodes(bytes).map_err(|e| e.to_string())?;
        let c = set_at(&mut sub, &path[1..], r, g, b, alpha)?;
        nodes[path[0]].value = NodeValue::Bytes(iwa::pb_serialize_nodes(&sub));
        Ok(c)
    }
    let c = set_at(&mut nodes, &hit.path, r, g, b, alpha)?;
    docs[hit.doc].segs[hit.seg].objs[hit.obj] = iwa::pb_serialize_nodes(&nodes);
    write::save_docs(src, dst, &docs)?;
    println!("颜色 #{index} → {} a={:.3}", hex(&c), c.a.unwrap_or(1.0));
    Ok(())
}
