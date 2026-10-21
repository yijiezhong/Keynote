//! File-level write support: load IWA docs, locate a text storage, re-pack a .key.
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Write};

use prost::Message;

use crate::iwa::{self, Segment};
use crate::{kn, tsd, tsp, tst, tswp};

pub struct Doc {
    pub name: String,
    pub segs: Vec<Segment>,
}

pub fn load_docs(path: &str) -> Result<Vec<Doc>, String> {
    let f = File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let mut docs = Vec::new();
    for i in 0..zip.len() {
        let name = zip.by_index(i).map_err(|e| e.to_string())?.name().map_err(|e| e.to_string())?.to_string();
        if !name.ends_with(".iwa") {
            continue;
        }
        let mut buf = Vec::new();
        zip.by_name(&name).map_err(|e| e.to_string())?.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        if buf.len() >= 4 && buf[0] != 0x00 {
            return Err("这个文件可能带密码保护（IWA 头不是明文）".into());
        }
        let payload = iwa::iwa_unwrap(&buf).map_err(|e| format!("{name}: {e}"))?;
        let segs = iwa::parse_segments(&payload).map_err(|e| format!("{name}: {e}"))?;
        docs.push(Doc { name, segs });
    }
    Ok(docs)
}

pub fn ident_map(docs: &[Doc]) -> HashMap<u64, (u64, Vec<u8>)> {
    let mut map = HashMap::new();
    for d in docs {
        for seg in &d.segs {
            if let Some(id) = iwa::ident_of(seg) {
                if let Some(obj) = seg.objs.first() {
                    let t = seg.meta.first().map(|(t, _)| *t).unwrap_or(0);
                    map.insert(id, (t, obj.clone()));
                }
            }
        }
    }
    map
}

fn find_show(map: &HashMap<u64, (u64, Vec<u8>)>) -> Option<kn::ShowArchive> {
    for (t, b) in map.values() {
        if *t == 2 {
            if let Ok(show) = <kn::ShowArchive as Message>::decode(b.as_slice()) {
                return Some(show);
            }
        }
    }
    None
}

fn walk_nodes(
    id: u64,
    map: &HashMap<u64, (u64, Vec<u8>)>,
    out: &mut Vec<u64>,
    seen: &mut HashSet<u64>,
) {
    if !seen.insert(id) {
        return;
    }
    let Some((_, b)) = map.get(&id) else { return };
    if let Ok(node) = <kn::SlideNodeArchive as Message>::decode(b.as_slice()) {
        if node.slide.is_some() {
            out.push(id);
        }
        for c in &node.children {
            walk_nodes(c.identifier, map, out, seen);
        }
    }
}

pub fn slide_node_ids(map: &HashMap<u64, (u64, Vec<u8>)>) -> Vec<u64> {
    let Some(show) = find_show(map) else { return Vec::new() };
    let mut ids: Vec<u64> = show.slide_tree.slides.iter().map(|r| r.identifier).collect();
    if ids.is_empty() {
        if let Some(root) = &show.slide_tree.root_slide_node {
            let mut seen = HashSet::new();
            walk_nodes(root.identifier, map, &mut ids, &mut seen);
        }
    }
    ids
}

pub fn slides(map: &HashMap<u64, (u64, Vec<u8>)>) -> Vec<kn::SlideArchive> {
    let Some(show) = find_show(map) else { return Vec::new() };
    let mut ids: Vec<u64> = show.slide_tree.slides.iter().map(|r| r.identifier).collect();
    if ids.is_empty() {
        if let Some(root) = &show.slide_tree.root_slide_node {
            let mut seen = HashSet::new();
            walk_nodes(root.identifier, map, &mut ids, &mut seen);
        }
    }
    let mut out = Vec::new();
    for id in ids {
        let Some((_, nb)) = map.get(&id) else { continue };
        let Ok(node) = <kn::SlideNodeArchive as Message>::decode(nb.as_slice()) else { continue };
        let Some(slide) = &node.slide else { continue };
        let Some((_, sb)) = map.get(&slide.identifier) else { continue };
        if let Ok(sa) = <kn::SlideArchive as Message>::decode(sb.as_slice()) {
            out.push(sa);
        }
    }
    out
}

pub fn title_body_storage_ids(map: &HashMap<u64, (u64, Vec<u8>)>, sa: &kn::SlideArchive) -> (Option<u64>, Option<u64>) {
    let get = |r: &Option<tsp::Reference>| -> Option<u64> {
        let r = r.as_ref()?;
        let (_, pb) = map.get(&r.identifier)?;
        let ph = <kn::PlaceholderArchive as Message>::decode(pb.as_slice()).ok()?;
        let st = ph.super_.owned_storage.as_ref().or(ph.super_.deprecated_storage.as_ref())?;
        Some(st.identifier)
    };
    (get(&sa.title_placeholder), get(&sa.body_placeholder))
}

pub fn find_object(docs: &[Doc], ident: u64, type_hint: &str) -> Option<(usize, usize, usize)> {
    for (di, d) in docs.iter().enumerate() {
        for (si, seg) in d.segs.iter().enumerate() {
            if iwa::ident_of(seg) != Some(ident) {
                continue;
            }
            for (oi, obj) in seg.objs.iter().enumerate() {
                if type_hint == "storage" && <tswp::StorageArchive as Message>::decode(obj.as_slice()).is_ok() {
                    return Some((di, si, oi));
                }
                if type_hint == "tile" && <tst::Tile as Message>::decode(obj.as_slice()).is_ok() {
                    return Some((di, si, oi));
                }
                if type_hint == "slide" && <kn::SlideNodeArchive as Message>::decode(obj.as_slice()).is_ok() {
                    return Some((di, si, oi));
                }
                if type_hint == "header" && <tst::HeaderStorageBucket as Message>::decode(obj.as_slice()).is_ok() {
                    return Some((di, si, oi));
                }
                if type_hint == "uid" && <tst::ColumnRowUidMapArchive as Message>::decode(obj.as_slice()).is_ok() {
                    return Some((di, si, oi));
                }
                if type_hint == "link" && <tswp::HyperlinkFieldArchive as Message>::decode(obj.as_slice()).is_ok() {
                    return Some((di, si, oi));
                }
                if type_hint == "image" && <tsd::ImageArchive as Message>::decode(obj.as_slice()).is_ok() {
                    return Some((di, si, oi));
                }
                if type_hint == "slidearchive" && <kn::SlideArchive as Message>::decode(obj.as_slice()).is_ok() {
                    return Some((di, si, oi));
                }
                if type_hint == "buildchunk" && <kn::BuildChunkArchive as Message>::decode(obj.as_slice()).is_ok() {
                    return Some((di, si, oi));
                }
                if type_hint == "slidestyle" && <kn::SlideStyleArchive as Message>::decode(obj.as_slice()).is_ok() {
                    return Some((di, si, oi));
                }
                if type_hint == "" {
                    return Some((di, si, oi));
                }
            }
        }
    }
    None
}

pub fn set_storage_text(src: &str, dst: &str, page: usize, which: &str, text: &str) -> Result<(), String> {
    let docs = load_docs(src)?;
    let map = ident_map(&docs);
    let slides = slides(&map);
    if page == 0 || page > slides.len() {
        return Err(format!("页码越界：{}（共 {} 页）", page, slides.len()));
    }
    let (title_id, body_id) = title_body_storage_ids(&map, &slides[page - 1]);
    let ident = match which {
        "title" => title_id,
        "body" => body_id,
        _ => return Err("which 必须是 title 或 body".into()),
    }.ok_or_else(|| format!("第 {page} 页没有 {which} 文本"))?;

    let (di, si, oi) = find_object(&docs, ident, "storage").ok_or("找不到文本存储对象")?;
    let mut st = <tswp::StorageArchive as Message>::decode(docs[di].segs[si].objs[oi].as_slice())
        .map_err(|e| e.to_string())?;
    st.text = vec![text.to_string()];
    let new_bytes = st.encode_to_vec();
    save_rewrite(src, dst, &docs[di].name, si, oi, &new_bytes)
}


pub fn note_storage_id(map: &HashMap<u64, (u64, Vec<u8>)>, sa: &kn::SlideArchive) -> Option<u64> {
    let r = sa.note.as_ref()?;
    let (_, nb) = map.get(&r.identifier)?;
    let na = <kn::NoteArchive as Message>::decode(nb.as_slice()).ok()?;
    Some(na.contained_storage.identifier)
}

pub fn set_note_text(src: &str, dst: &str, page: usize, text: &str, append: bool) -> Result<(), String> {
    let docs = load_docs(src)?;
    let map = ident_map(&docs);
    let slides = slides(&map);
    if page == 0 || page > slides.len() {
        return Err(format!("页码越界：{}（共 {} 页）", page, slides.len()));
    }
    let ident = note_storage_id(&map, &slides[page - 1])
        .ok_or_else(|| format!("第 {page} 页没有演讲者备注"))?;
    let (di, si, oi) = find_object(&docs, ident, "storage").ok_or("找不到备注存储对象")?;
    let mut st = <tswp::StorageArchive as Message>::decode(docs[di].segs[si].objs[oi].as_slice())
        .map_err(|e| e.to_string())?;
    if append {
        let mut old = st.text.join("\n");
        if !old.is_empty() {
            old.push('\n');
        }
        old.push_str(text);
        st.text = vec![old];
    } else {
        st.text = vec![text.to_string()];
    }
    let new_bytes = st.encode_to_vec();
    save_rewrite(src, dst, &docs[di].name, si, oi, &new_bytes)
}


pub fn note_text(map: &HashMap<u64, (u64, Vec<u8>)>, sa: &kn::SlideArchive) -> Option<String> {
    let r = sa.note.as_ref()?;
    let (_, nb) = map.get(&r.identifier)?;
    let na = <kn::NoteArchive as Message>::decode(nb.as_slice()).ok()?;
    let (_, sb) = map.get(&na.contained_storage.identifier)?;
    let st = <tswp::StorageArchive as Message>::decode(sb.as_slice()).ok()?;
    Some(st.text.join(""))
}

pub fn save_rewrite_data(
    src: &str,
    dst: &str,
    target_name: &str,
    seg_i: usize,
    obj_i: usize,
    new_obj: &[u8],
    data_name: &str,
    data_bytes: &[u8],
) -> Result<(), String> {
    let f = File::open(src).map_err(|e| e.to_string())?;
    let mut zin = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let tmp = format!("{dst}.tmp");
    let out = File::create(&tmp).map_err(|e| e.to_string())?;
    let mut zw = zip::ZipWriter::new(out);
    let mut found_data = false;
    for i in 0..zin.len() {
        let name = {
            let info = zin.by_index(i).map_err(|e| e.to_string())?;
            info.name().map_err(|e| e.to_string())?.to_string()
        };
        let mut data = Vec::new();
        zin.by_name(&name).map_err(|e| e.to_string())?.read_to_end(&mut data).map_err(|e| e.to_string())?;
        if name == target_name {
            let payload = iwa::iwa_unwrap(&data).map_err(|e| e.to_string())?;
            let mut segs = iwa::parse_segments(&payload).map_err(|e| e.to_string())?;
            segs[seg_i].objs[obj_i] = new_obj.to_vec();
            data = iwa::iwa_wrap(&iwa::serialize_segments(&segs));
        } else if name == data_name {
            data = data_bytes.to_vec();
            found_data = true;
        }
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        if name.ends_with('/') {
            zw.add_directory(&name, opts).map_err(|e| e.to_string())?;
        } else {
            zw.start_file(&name, opts).map_err(|e| e.to_string())?;
            zw.write_all(&data).map_err(|e| e.to_string())?;
        }
    }
    if !found_data {
        return Err(format!("找不到数据文件 {data_name}"));
    }
    zw.finish().map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dst).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn save_docs(src: &str, dst: &str, docs: &[Doc]) -> Result<(), String> {
    let f = File::open(src).map_err(|e| e.to_string())?;
    let mut zin = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let tmp = format!("{dst}.tmp");
    let out = File::create(&tmp).map_err(|e| e.to_string())?;
    let mut zw = zip::ZipWriter::new(out);
    for i in 0..zin.len() {
        let name = {
            let info = zin.by_index(i).map_err(|e| e.to_string())?;
            info.name().map_err(|e| e.to_string())?.to_string()
        };
        let mut data = Vec::new();
        zin.by_name(&name).map_err(|e| e.to_string())?.read_to_end(&mut data).map_err(|e| e.to_string())?;
        if let Some(d) = docs.iter().find(|d| d.name == name) {
            data = iwa::iwa_wrap(&iwa::serialize_segments(&d.segs));
        }
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        if name.ends_with('/') {
            zw.add_directory(&name, opts).map_err(|e| e.to_string())?;
        } else {
            zw.start_file(&name, opts).map_err(|e| e.to_string())?;
            zw.write_all(&data).map_err(|e| e.to_string())?;
        }
    }
    zw.finish().map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dst).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn save_rewrite(
    src: &str,
    dst: &str,
    target_name: &str,
    seg_i: usize,
    obj_i: usize,
    new_obj: &[u8],
) -> Result<(), String> {
    let f = File::open(src).map_err(|e| e.to_string())?;
    let mut zin = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let tmp = format!("{dst}.tmp");
    let out = File::create(&tmp).map_err(|e| e.to_string())?;
    let mut zw = zip::ZipWriter::new(out);
    for i in 0..zin.len() {
        let name = {
            let info = zin.by_index(i).map_err(|e| e.to_string())?;
            info.name().map_err(|e| e.to_string())?.to_string()
        };
        let mut data = Vec::new();
        zin.by_name(&name).map_err(|e| e.to_string())?.read_to_end(&mut data).map_err(|e| e.to_string())?;
        if name == target_name {
            let payload = iwa::iwa_unwrap(&data).map_err(|e| e.to_string())?;
            let mut segs = iwa::parse_segments(&payload).map_err(|e| e.to_string())?;
            segs[seg_i].objs[obj_i] = new_obj.to_vec();
            if let Some(m) = segs[seg_i].meta.get_mut(obj_i) {
                m.1 = new_obj.len() as u64;
            }
            let new_payload = iwa::serialize_segments(&segs);
            data = iwa::iwa_wrap(&new_payload);
        }
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        if name.ends_with('/') {
            zw.add_directory(&name, opts).map_err(|e| e.to_string())?;
        } else {
            zw.start_file(&name, opts).map_err(|e| e.to_string())?;
            zw.write_all(&data).map_err(|e| e.to_string())?;
        }
    }
    zw.finish().map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dst).map_err(|e| e.to_string())?;
    Ok(())
}


pub fn edit_text(
    src: &str,
    dst: &str,
    iwa_name: &str,
    old: &str,
    new: &str,
    resize: bool,
) -> Result<usize, String> {
    if !resize && old.len() != new.len() {
        return Err("未加 --resize 时只允许等长替换".into());
    }
    let mut docs = load_docs(src)?;
    let mut total = 0usize;
    for d in &mut docs {
        if iwa_name != "-" && d.name != iwa_name { continue; }
        let payload = iwa::serialize_segments(&d.segs);
        let (new_payload, hits) = iwa::replace_payload(&payload, old.as_bytes(), new.as_bytes())?;
        if hits > 0 {
            d.segs = iwa::parse_segments(&new_payload)?;
            total += hits;
        }
    }
    if total == 0 { return Err("没有找到要替换的字符串".into()); }
    save_docs(src, dst, &docs)?;
    Ok(total)
}
