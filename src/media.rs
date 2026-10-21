//! Image replacement.
use prost::Message;
use std::io::Read;

use crate::{tsd, write};

pub fn set_image(src: &str, dst: &str, ident: u64, image: &str) -> Result<(), String> {
    let docs = write::load_docs(src)?;
    let (di, si, oi) = write::find_object(&docs, ident, "image").ok_or("找不到图片对象")?;
    let mut m = <tsd::ImageArchive as Message>::decode(docs[di].segs[si].objs[oi].as_slice())
        .map_err(|e| e.to_string())?;
    let data_id = m.data.as_ref().ok_or("该图片没有 data 引用")?.identifier;

    let f = std::fs::File::open(src).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let mut target = None;
    for i in 0..z.len() {
        let name = z.by_index(i).map_err(|e| e.to_string())?.name().map_err(|e| e.to_string())?.to_string();
        if !name.starts_with("Data/") { continue; }
        let stem = name.rsplit('-').next().unwrap_or("").split('.').next().unwrap_or("");
        if stem == data_id.to_string() { target = Some(name); break; }
    }
    let target = target.ok_or_else(|| format!("找不到 data={data_id} 对应的 Data/ 文件"))?;
    let bytes = std::fs::read(image).map_err(|e| e.to_string())?;

    let out = std::process::Command::new("sips")
        .args(["-g", "pixelWidth", "-g", "pixelHeight", image])
        .output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut w = None;
    let mut h = None;
    for line in text.lines() {
        if let Some(v) = line.trim().strip_prefix("pixelWidth:") { w = v.trim().parse::<f64>().ok(); }
        if let Some(v) = line.trim().strip_prefix("pixelHeight:") { h = v.trim().parse::<f64>().ok(); }
    }
    if let (Some(w), Some(h)) = (w, h) {
        if let Some(s) = m.natural_size.as_mut() { s.width = w as f32; s.height = h as f32; }
        if let Some(s) = m.original_size.as_mut() { s.width = w as f32; s.height = h as f32; }
    }
    write::save_rewrite_data(src, dst, &docs[di].name, si, oi, &m.encode_to_vec(), &target, &bytes)?;
    println!("图片 {ident}：已替换 {target}");
    if let (Some(w), Some(h)) = (w, h) { println!("  新尺寸 {w:.0}x{h:.0}，已同步 naturalSize/originalSize"); }
    Ok(())
}

pub fn image_info(path: &str, ident: u64) -> Result<(), String> {
    let docs = write::load_docs(path)?;
    let (di, si, oi) = write::find_object(&docs, ident, "image").ok_or("找不到图片对象")?;
    let m = <tsd::ImageArchive as Message>::decode(docs[di].segs[si].objs[oi].as_slice())
        .map_err(|e| e.to_string())?;
    let data = m.data.as_ref().map(|d| d.identifier).unwrap_or(0);
    println!("image {ident}: data={data} bytes={}", docs[di].segs[si].objs[oi].len());
    Ok(())
}
