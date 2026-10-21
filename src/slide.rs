//! Slide skip flags.
use prost::Message;

use crate::{kn, write};

pub fn parse_pages(expr: &str, total: usize) -> Vec<usize> {
    let e = expr.trim().to_lowercase();
    if e == "all" || e == "*" {
        return (1..=total).collect();
    }
    let mut out = Vec::new();
    for part in e.split(',') {
        let part = part.trim();
        if part.is_empty() { continue; }
        if let Some((a, b)) = part.split_once('-') {
            if let (Ok(mut x), Ok(mut y)) = (a.trim().parse::<usize>(), b.trim().parse::<usize>()) {
                if x > y { std::mem::swap(&mut x, &mut y); }
                for p in x..=y { if p >= 1 && p <= total { out.push(p); } }
            }
        } else if let Ok(p) = part.parse::<usize>() {
            if p >= 1 && p <= total { out.push(p); }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

pub fn set_skip(src: &str, dst: &str, expr: &str, skip: bool) -> Result<String, String> {
    let mut docs = write::load_docs(src)?;
    let map = write::ident_map(&docs);
    let ids = write::slide_node_ids(&map);
    if ids.is_empty() {
        return Err("未找到幻灯片树".into());
    }
    let pages = parse_pages(expr, ids.len());
    if pages.is_empty() {
        return Err(format!("页码表达式无有效页：{expr:?}（共 {} 页）", ids.len()));
    }
    let mut done = Vec::new();
    for page in pages {
        let id = ids[page - 1];
        let Some((di, si, oi)) = write::find_object(&docs, id, "slide") else { continue };
        let mut node = <kn::SlideNodeArchive as Message>::decode(docs[di].segs[si].objs[oi].as_slice())
            .map_err(|e| e.to_string())?;
        node.is_skipped = skip;
        docs[di].segs[si].objs[oi] = node.encode_to_vec();
        done.push(page);
    }
    write::save_docs(src, dst, &docs)?;
    Ok(format!("共处理 {} 页：{:?}", done.len(), done))
}
