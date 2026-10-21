//! Read-only inspections: hyperlinks, builds, transitions.
use prost::Message;

use crate::write;
use crate::{kn, tsd, tswp};

pub fn links(path: &str) -> Result<(), String> {
    let docs = write::load_docs(path)?;
    let map = write::ident_map(&docs);
    let mut n = 0usize;
    for (id, (_t, b)) in &map {
        if let Ok(m) = <tswp::HyperlinkFieldArchive as Message>::decode(b.as_slice()) {
            let url = m.url_ref.clone().unwrap_or_default();
            let ok = url.starts_with("http://")
                || url.starts_with("https://")
                || url.starts_with("mailto:")
                || url.starts_with("?slideid=");
            if !ok {
                continue;
            }
            let kind = if url.starts_with("?slideid=") { "页内跳转" } else { "URL" };
            println!("{id:>10}  {kind}  {url}");
            n += 1;
        }
    }
    println!("共 {n} 个超链接");
    Ok(())
}

pub fn transitions(path: &str) -> Result<(), String> {
    let docs = write::load_docs(path)?;
    let map = write::ident_map(&docs);
    let slides = write::slides(&map);
    let mut n = 0usize;
    for (i, sa) in slides.iter().enumerate() {
        let attrs = sa.transition.attributes.animation_attributes.as_ref();
        let effect = attrs.and_then(|a| a.effect.clone()).unwrap_or_default();
        let dur = attrs.and_then(|a| a.duration).unwrap_or(0.0);
        if effect.is_empty() && dur == 0.0 {
            continue;
        }
        println!("第 {} 页：effect={} duration={dur}", i + 1, if effect.is_empty() { "(默认)" } else { &effect });
        n += 1;
    }
    println!("共 {n} 页有转场");
    Ok(())
}

fn build_lines(path: &str) -> Result<(usize, usize), String> {
    let docs = write::load_docs(path)?;
    let map = write::ident_map(&docs);
    let slides = write::slides(&map);
    let mut total = 0usize;
    let mut pages = 0usize;
    for (i, sa) in slides.iter().enumerate() {
        if sa.builds.is_empty() { continue; }
        pages += 1;
        println!("--- 第 {} 页 ---", i + 1);
        for r in &sa.builds {
            let Some((_, b)) = map.get(&r.identifier) else { continue };
            let Ok(build) = <kn::BuildArchive as Message>::decode(b.as_slice()) else { continue };
            let target = build.drawable.as_ref().map(|d| d.identifier).unwrap_or(0);
            let anim = build.attributes.animation_attributes.as_ref();
            let effect = anim.and_then(|a| a.effect.clone()).unwrap_or_default();
            let dur = anim.and_then(|a| a.duration).unwrap_or(0.0);
            println!("  build {}: drawable={target} delivery={} effect={} duration={dur}", r.identifier, build.delivery, effect);
            total += 1;
        }
    }
    Ok((total, pages))
}

pub fn builds(path: &str, detail: bool) -> Result<(), String> {
    let (total, pages) = build_lines(path)?;
    if detail {
        println!("共 {total} 个构建动画（{pages} 页）");
    } else {
        println!("共 {total} 个构建动画");
    }
    Ok(())
}


pub fn set_link(src: &str, dst: &str, ident: u64, url: &str) -> Result<(), String> {
    let docs = write::load_docs(src)?;
    let (di, si, oi) = write::find_object(&docs, ident, "link").ok_or("找不到链接对象")?;
    let mut m = <tswp::HyperlinkFieldArchive as Message>::decode(
        docs[di].segs[si].objs[oi].as_slice()).map_err(|e| e.to_string())?;
    m.url_ref = Some(url.to_string());
    write::save_rewrite(src, dst, &docs[di].name, si, oi, &m.encode_to_vec())?;
    println!("超链接 {ident} → {url}");
    Ok(())
}


pub fn images(path: &str) -> Result<(), String> {
    let docs = write::load_docs(path)?;
    let map = write::ident_map(&docs);
    let mut n = 0usize;
    for (id, (_t, b)) in &map {
        if let Ok(m) = <tsd::ImageArchive as Message>::decode(b.as_slice()) {
            let Some(data) = m.data.as_ref() else { continue };
            let nat = m.natural_size.as_ref()
                .map(|s| format!("{}x{}", s.width as i64, s.height as i64))
                .unwrap_or_else(|| "?".into());
            let geo = m.super_.geometry.as_ref()
                .map(|g| {
                    let p = g.position.as_ref()
                        .map(|p| format!("{:.0},{:.0}", p.x, p.y))
                        .unwrap_or_else(|| "?".into());
                    let s = g.size.as_ref()
                        .map(|s| format!("{:.0}x{:.0}", s.width, s.height))
                        .unwrap_or_else(|| "?".into());
                    format!("({p}) {s}")
                })
                .unwrap_or_else(|| "?".into());
            println!("{id:>10}  pos={geo:<18} natural={nat:<12} data={}", data.identifier);
            n += 1;
        }
    }
    println!("共 {n} 张图片");
    Ok(())
}
