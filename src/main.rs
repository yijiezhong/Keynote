// D3 spike：不依赖 Python，从零读 .key 并输出与 spec-1 一致的验收数字。
// prost-build 每个 proto package 生成一个文件，跨 package 用 super::xxx 引用
mod iwa;
mod write;

macro_rules! pb_mod {
    ($name:ident, $file:literal) => {
        pub mod $name {
            include!(concat!(env!("OUT_DIR"), "/", $file));
        }
    };
}
pb_mod!(kn, "kn.rs");
pb_mod!(tsa, "tsa.rs");
pb_mod!(tsce, "tsce.rs");
// TSCH 一个 package 被拆成多个文件，prost 用 super::X 引用同 package 的类型，
// 所以 generated / pre_uff 必须是 tsch 的子模块
pub mod tsch {
    include!(concat!(env!("OUT_DIR"), "/tsch.rs"));
    pub mod generated {
        include!(concat!(env!("OUT_DIR"), "/tsch.generated.rs"));
    }
    pub mod pre_uff {
        include!(concat!(env!("OUT_DIR"), "/tsch.pre_uff.rs"));
    }
}
pb_mod!(tsck, "tsck.rs");
pb_mod!(tsd, "tsd.rs");
pb_mod!(tsdsos, "tsdsos.rs");
pb_mod!(tsk, "tsk.rs");
pb_mod!(tsp, "tsp.rs");
pb_mod!(tss, "tss.rs");
pb_mod!(tsssos, "tsssos.rs");
pb_mod!(tst, "tst.rs");
pb_mod!(tswp, "tswp.rs");
pb_mod!(tswpsos, "tswpsos.rs");


use std::fs::File;
use std::io::Read;

fn snappy_decompress(src: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut i = 0usize;
    // raw Snappy block = varint(uncompressed_length) | tag 流
    let mut declared = 0u64;
    let mut shift = 0;
    loop {
        let b = *src.get(i).ok_or("snappy varint 越界")?;
        i += 1;
        declared |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    let n = src.len();
    while i < n {
        let tag = src[i];
        i += 1;
        let kind = tag & 0x03;
        let (len, off) = if kind == 0 {
            let mut ln = (tag >> 2) as usize;
            if ln < 60 {
                ln += 1;
            } else {
                let extra = ln - 59;
                let mut v = 0usize;
                for k in 0..extra {
                    v |= (src[i + k] as usize) << (8 * k);
                }
                i += extra;
                ln = v + 1;
            }
            out.extend_from_slice(&src[i..i + ln]);
            i += ln;
            continue;
        } else if kind == 1 {
            let ln = (((tag >> 2) & 0x07) + 4) as usize;
            let off = (((tag >> 5) as usize) << 8) | src[i] as usize;
            i += 1;
            (ln, off)
        } else if kind == 2 {
            let ln = ((tag >> 2) + 1) as usize;
            let off = (src[i] as usize) | ((src[i + 1] as usize) << 8);
            i += 2;
            (ln, off)
        } else {
            let ln = ((tag >> 2) + 1) as usize;
            let off = (src[i] as usize)
                | ((src[i + 1] as usize) << 8)
                | ((src[i + 2] as usize) << 16)
                | ((src[i + 3] as usize) << 24);
            i += 4;
            (ln, off)
        };
        if off == 0 || off > out.len() {
            return Err(format!("snappy copy offset 越界 off={off} out={}", out.len()));
        }
        for _ in 0..len {
            let b = out[out.len() - off];
            out.push(b);
        }
    }
    if declared as usize != out.len() {
        return Err(format!("snappy 长度不符：声明 {declared} 实际 {}", out.len()));
    }
    Ok(out)
}

fn unwrap_iwa(blob: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < blob.len() {
        if blob[i] != 0x00 {
            return Err(format!("非预期 IWA 头标记 0x{:02x} @{i}", blob[i]));
        }
        if i + 4 > blob.len() {
            return Err(format!("chunk 头被截断 @{i}"));
        }
        let clen = (blob[i + 1] as usize) | ((blob[i + 2] as usize) << 8) | ((blob[i + 3] as usize) << 16);
        if i + 4 + clen > blob.len() {
            return Err(format!("chunk @{i} 声明 {clen} 字节超出文件末尾"));
        }
        out.extend_from_slice(&snappy_decompress(&blob[i + 4..i + 4 + clen])?);
        i += 4 + clen;
    }
    Ok(out)
}

/// 段解析：返回 (ident, [(type, length)])，并保留每段的 (对象起始偏移, 长度)
enum Val {
    Varint(u64),
    Bytes(Vec<u8>),
}

fn read_varint(buf: &[u8], mut i: usize) -> Result<(u64, usize), String> {
    let mut v = 0u64;
    let mut shift = 0;
    loop {
        if i >= buf.len() {
            return Err("varint 越界".into());
        }
        let b = buf[i];
        i += 1;
        v |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            return Ok((v, i));
        }
        shift += 7;
    }
}

fn parse_fields(buf: &[u8]) -> Result<Vec<(u64, u8, Val)>, String> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < buf.len() {
        let (tag, ni) = read_varint(buf, i)?;
        i = ni;
        let fn_ = tag >> 3;
        let wt = (tag & 0x07) as u8;
        match wt {
            0 => {
                let (v, ni) = read_varint(buf, i)?;
                i = ni;
                out.push((fn_, wt, Val::Varint(v)));
            }
            2 => {
                let (ln, ni) = read_varint(buf, i)?;
                i = ni;
                let ln = ln as usize;
                if i + ln > buf.len() {
                    return Err("length 越界".into());
                }
                out.push((fn_, wt, Val::Bytes(buf[i..i + ln].to_vec())));
                i += ln;
            }
            5 => {
                out.push((fn_, wt, Val::Bytes(buf[i..i + 4].to_vec())));
                i += 4;
            }
            1 => {
                out.push((fn_, wt, Val::Bytes(buf[i..i + 8].to_vec())));
                i += 8;
            }
            other => return Err(format!("未知 wire type {other}")),
        }
    }
    Ok(out)
}

struct ObjRef {
    type_: u64,
    start: usize,
    len: usize,
}

struct Seg {
    ident: u64,
    objs: Vec<ObjRef>,
}

fn parse_segments(payload: &[u8]) -> Result<Vec<Seg>, String> {
    let mut segs = Vec::new();
    let mut i = 0usize;
    while i < payload.len() {
        let (alen, after_len) = read_varint(payload, i)?;
        let aend = after_len + alen as usize;
        if aend > payload.len() {
            return Err("ArchiveInfo 越界".into());
        }
        let ainfo = &payload[after_len..aend];
        let mut ident = 0u64;
        let mut infos = Vec::new();
        for (fn_, wt, val) in parse_fields(ainfo)? {
            match (fn_, wt, val) {
                (1, 0, Val::Varint(v)) => ident = v,
                (2, 2, Val::Bytes(mi)) => {
                    let mut mtype = 0u64;
                    let mut length = 0usize;
                    for (mfn, mwt, mv) in parse_fields(&mi)? {
                        if let Val::Varint(v) = mv {
                            if mfn == 1 && mwt == 0 {
                                mtype = v;
                            } else if mfn == 3 && mwt == 0 {
                                length = v as usize;
                            }
                        }
                    }
                    infos.push((mtype, length));
                }
                _ => {}
            }
        }
        let mut cursor = aend;
        let mut objs = Vec::new();
        for (t, ln) in infos {
            if cursor + ln > payload.len() {
                return Err("object 越界".into());
            }
            objs.push(ObjRef { type_: t, start: cursor, len: ln });
            cursor += ln;
        }
        segs.push(Seg { ident, objs });
        i = cursor;
    }
    Ok(segs)
}

/// M6.x：slideTree 是树（root_slide_node + children），DFS 数「带 slide 的 node」
fn count_tree(idents: &std::collections::HashMap<u64, (u64, Vec<u8>)>, root: u64) -> usize {
    use prost::Message;
    let mut n = 0usize;
    let mut stack = vec![root];
    let mut seen = std::collections::HashSet::new();
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        let Some((t, data)) = idents.get(&id) else { continue };
        if *t != 4 {
            continue; // 4 = KN.SlideNodeArchive
        }
        if let Ok(node) = <kn::SlideNodeArchive as Message>::decode(&data[..]) {
            if node.slide.is_some() {
                n += 1;
            }
            for c in &node.children {
                stack.push(c.identifier);
            }
        }
    }
    n
}

fn verify(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let f = File::open(&path)?;
    let mut zip = zip::ZipArchive::new(f)?;
    let mut names = Vec::new();
    for i in 0..zip.len() {
        names.push(zip.by_index(i)?.name()?.to_string());
    }
    let iwa: Vec<String> = names.iter().filter(|n| n.ends_with(".iwa")).cloned().collect();
    println!("entries={} iwa={}", names.len(), iwa.len());

    // ident → (type, object bytes)；段内只有第一个对象有 ident（ArchiveInfo.identifier）
    let mut idents: std::collections::HashMap<u64, (u64, Vec<u8>)> = std::collections::HashMap::new();
    let mut segments = 0usize;
    let mut objects = 0usize;
    let mut doc_raw = 0usize;
    let mut doc_unwrapped = 0usize;
    for name in &iwa {
        let mut buf = Vec::new();
        zip.by_name(name)?.read_to_end(&mut buf)?;
        let payload = unwrap_iwa(&buf)?;
        let segs = parse_segments(&payload)?;
        segments += segs.len();
        objects += segs.iter().map(|s| s.objs.len()).sum::<usize>();
        if name == "Index/Document.iwa" {
            doc_raw = buf.len();
            doc_unwrapped = payload.len();
        }
        for seg in &segs {
            for (idx, obj) in seg.objs.iter().enumerate() {
                if idx != 0 {
                    continue; // 段内非首对象没有 ident，跳过（与 Python 的 _locate 一致）
                }
                idents.insert(
                    seg.ident,
                    (obj.type_, payload[obj.start..obj.start + obj.len].to_vec()),
                );
            }
        }
    }
    println!("segments={segments} objects={objects}");
    println!("document.iwa raw={doc_raw} unwrapped={doc_unwrapped}");

    // 页序：优先 slideTree.slides（扁平），否则 root_slide_node（M6.x 树形）
    use prost::Message;
    let mut slides = 0usize;
    for (_id, (t, data)) in &idents {
        if *t != 2 {
            continue; // 2 = KN.ShowArchive
        }
        if let Ok(show) = <kn::ShowArchive as Message>::decode(&data[..]) {
            let tree = &show.slide_tree;
            if !tree.slides.is_empty() {
                slides += tree.slides.len();
            } else if let Some(root) = &tree.root_slide_node {
                slides += count_tree(&idents, root.identifier);
            }
        }
    }
    println!("show_slides={slides}");
    Ok(())
}


fn list(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let f = File::open(path)?;
    let mut zip = zip::ZipArchive::new(f)?;
    println!("entries\t{}", zip.len());
    for i in 0..zip.len() {
        let e = zip.by_index(i)?;
        let name = e.name()?;
        println!("{}\t{}", name, e.size());
    }
    Ok(())
}

fn is_cjk(text: &str) -> bool {
    text.chars().any(|c| {
        let u = c as u32;
        (0x4e00..=0x9fff).contains(&u)
    })
}

fn text(path: &str, keyword: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let f = File::open(path)?;
    let mut zip = zip::ZipArchive::new(f)?;
    for i in 0..zip.len() {
        let name = zip.by_index(i)?.name()?.to_string();
        if !name.ends_with(".iwa") {
            continue;
        }
        let mut buf = Vec::new();
        zip.by_name(&name)?.read_to_end(&mut buf)?;
        let payload = match iwa::iwa_unwrap(&buf) {
            Ok(v) => v,
            Err(e) => {
                println!("--- {name} 解压失败: {e}");
                continue;
            }
        };
        let segs = match iwa::parse_segments(&payload) {
            Ok(v) => v,
            Err(e) => {
                println!("--- {name} 段解析失败: {e}");
                continue;
            }
        };
        let mut hits: Vec<(usize, u64, String)> = Vec::new();
        for seg in &segs {
            for obj in &seg.objs {
                let _ = iwa::pb_deep_strings(obj, 0, &mut hits);
            }
        }
        let keep: Vec<_> = hits
            .into_iter()
            .filter(|(_, _, t)| {
                (is_cjk(t) || t.chars().count() >= 6)
                    && keyword.map_or(true, |k| t.contains(k))
            })
            .collect();
        if !keep.is_empty() {
            println!("\n--- {name} ({} 条) ---", keep.len());
            for (depth, field, t) in keep.iter().take(15) {
                let head: String = t.chars().take(80).collect();
                println!("    depth={depth} field={field} {head:?}");
            }
        }
    }
    Ok(())
}

fn texts(path: &str, keyword: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    use prost::Message;
    let f = File::open(path)?;
    let mut zip = zip::ZipArchive::new(f)?;
    let mut rows = 0usize;
    for i in 0..zip.len() {
        let name = zip.by_index(i)?.name()?.to_string();
        if !name.ends_with(".iwa") {
            continue;
        }
        let mut buf = Vec::new();
        zip.by_name(&name)?.read_to_end(&mut buf)?;
        let payload = match iwa::iwa_unwrap(&buf) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let segs = match iwa::parse_segments(&payload) {
            Ok(v) => v,
            Err(_) => continue,
        };
        for seg in &segs {
            for obj in &seg.objs {
                if let Ok(msg) = <tswp::StorageArchive as Message>::decode(obj.as_slice()) {
                    let text = msg.text.join("");
                    if !text.trim().is_empty()
                        && keyword.map_or(true, |k| text.contains(k))
                    {
                        let head: String = text.split('\n').filter(|s| !s.trim().is_empty())
                            .collect::<Vec<_>>().join(" ⏎ ").chars().take(70).collect();
                        let short = name.rsplit('/').next().unwrap_or(&name);
                        println!("  {short:32} {head:?}");
                        rows += 1;
                    }
                }
            }
        }
    }
    println!("含文字的文本对象 {rows} 个");
    Ok(())
}

fn types(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let f = File::open(path)?;
    let mut zip = zip::ZipArchive::new(f)?;
    let mut counts = std::collections::BTreeMap::new();
    let mut total = 0usize;
    for i in 0..zip.len() {
        let name = zip.by_index(i)?.name()?.to_string();
        if !name.ends_with(".iwa") {
            continue;
        }
        let mut buf = Vec::new();
        zip.by_name(&name)?.read_to_end(&mut buf)?;
        let payload = match iwa::iwa_unwrap(&buf) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let segs = match iwa::parse_segments(&payload) {
            Ok(v) => v,
            Err(_) => continue,
        };
        for (t, _) in segs.iter().flat_map(|s| s.meta.iter()) {
            *counts.entry(*t).or_insert(0usize) += 1;
            total += 1;
        }
    }
    println!("对象总数 {total}");
    for (t, c) in counts.iter().take(30) {
        println!("  {c:5}  type {t}");
    }
    Ok(())
}

fn usage() {
    println!("kn (Rust) — Apple Keynote .key 工具，阶段 1：只读");
    println!();
    println!("用法：");
    println!("  kn list   <deck.key>             列出 zip 条目");
    println!("  kn verify <deck.key>             IWA / 段 / 对象 / slides 统计");
    println!("  kn text   <deck.key> [关键字]    按对象提取可见字符串");
    println!("  kn texts  <deck.key> [关键字]    按文本对象列出文字");
    println!("  kn types  <deck.key>             对象类型计数");
    println!("  kn outline <deck.key>           按页导出标题/正文");
    println!("  kn set-text <src> <dst> <页> <title|body> <文本>");
    println!("  kn set-note <src> <dst> <页> <文本>");
    println!("  kn append-note <src> <dst> <页> <文本>");
    println!("  kn info   <deck.key>             verify 的别名");
    println!("  kn help                          显示本帮助");
    println!();
    println!("写回与编辑命令（outline、set-text、new、add-slide 等）正在迁移。");
}


fn ident_map(path: &str) -> Result<std::collections::HashMap<u64, (u64, Vec<u8>)>, Box<dyn std::error::Error>> {
    let mut map = std::collections::HashMap::new();
    let f = File::open(path)?;
    let mut zip = zip::ZipArchive::new(f)?;
    for i in 0..zip.len() {
        let name = zip.by_index(i)?.name()?.to_string();
        if !name.ends_with(".iwa") {
            continue;
        }
        let mut buf = Vec::new();
        zip.by_name(&name)?.read_to_end(&mut buf)?;
        let payload = match iwa::iwa_unwrap(&buf) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let segs = match iwa::parse_segments(&payload) {
            Ok(v) => v,
            Err(_) => continue,
        };
        for seg in &segs {
            if let Some(id) = iwa::ident_of(seg) {
                if let Some(obj) = seg.objs.first() {
                    let t = seg.meta.first().map(|(t, _)| *t).unwrap_or(0);
                    map.insert(id, (t, obj.clone()));
                }
            }
        }
    }
    Ok(map)
}

fn find_show(
    map: &std::collections::HashMap<u64, (u64, Vec<u8>)>,
) -> Option<kn::ShowArchive> {
    use prost::Message;
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
    map: &std::collections::HashMap<u64, (u64, Vec<u8>)>,
    out: &mut Vec<u64>,
    seen: &mut std::collections::HashSet<u64>,
) {
    use prost::Message;
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

fn slides_of(
    map: &std::collections::HashMap<u64, (u64, Vec<u8>)>,
) -> Vec<kn::SlideArchive> {
    use prost::Message;
    let Some(show) = find_show(map) else { return Vec::new() };
    let mut node_ids: Vec<u64> = show.slide_tree.slides.iter().map(|r| r.identifier).collect();
    if node_ids.is_empty() {
        if let Some(root) = &show.slide_tree.root_slide_node {
            let mut seen = std::collections::HashSet::new();
            walk_nodes(root.identifier, map, &mut node_ids, &mut seen);
        }
    }
    let mut out = Vec::new();
    for id in node_ids {
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

fn placeholder_text(
    map: &std::collections::HashMap<u64, (u64, Vec<u8>)>,
    r: &Option<tsp::Reference>,
) -> Option<String> {
    use prost::Message;
    let r = r.as_ref()?;
    let (_, pb) = map.get(&r.identifier)?;
    let ph = <kn::PlaceholderArchive as Message>::decode(pb.as_slice()).ok()?;
    let st_ref = ph.super_.owned_storage.as_ref().or(ph.super_.deprecated_storage.as_ref())?;
    let (_, sb) = map.get(&st_ref.identifier)?;
    let st = <tswp::StorageArchive as Message>::decode(sb.as_slice()).ok()?;
    Some(st.text.join(""))
}

fn outline(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let map = ident_map(path)?;
    let slides = slides_of(&map);
    if slides.is_empty() {
        println!("未找到幻灯片树（可能不是 Keynote 演示文稿）。");
        return Ok(());
    }
    println!("共 {} 页\n", slides.len());
    for (i, sa) in slides.iter().enumerate() {
        let title = placeholder_text(&map, &sa.title_placeholder);
        let body = placeholder_text(&map, &sa.body_placeholder);
        println!("--- 第 {} 页 ---", i + 1);
        if let Some(t) = title.as_ref() {
            let head: String = t.trim().chars().take(80).collect();
            println!("  标题: {head}");
        }
        if let Some(b) = body.as_ref() {
            let lines: Vec<&str> = b.split('\n').filter(|s| !s.trim().is_empty()).collect();
            for line in lines.iter().take(3) {
                let head: String = line.chars().take(80).collect();
                println!("  正文: {head}");
            }
            if lines.len() > 3 {
                println!("        …（共 {} 行）", lines.len());
            }
        }
        match note_text(&map, sa) {
            Some(n) if !n.trim().is_empty() => println!("  备注: 有（{} 字）\n", n.chars().count()),
            _ => println!("  备注: 无\n"),
        }
    }
    println!("统计：{} 页", slides.len());
    Ok(())
}

fn note_text(
    map: &std::collections::HashMap<u64, (u64, Vec<u8>)>,
    sa: &kn::SlideArchive,
) -> Option<String> {
    use prost::Message;
    let r = sa.note.as_ref()?;
    let (_, nb) = map.get(&r.identifier)?;
    let na = <kn::NoteArchive as Message>::decode(nb.as_slice()).ok()?;
    let (_, sb) = map.get(&na.contained_storage.identifier)?;
    let st = <tswp::StorageArchive as Message>::decode(sb.as_slice()).ok()?;
    Some(st.text.join(""))
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        None | Some("help") | Some("--help") | Some("-h") => {
            usage();
            Ok(())
        }
        Some("verify") | Some("info") => {
            let p = args.get(1).ok_or("缺少文件")?;
            verify(p)
        }
        Some("list") => {
            let p = args.get(1).ok_or("缺少文件")?;
            list(p)
        }
        Some("text") => {
            let p = args.get(1).ok_or("缺少文件")?;
            text(p, args.get(2).map(|s| s.as_str()))
        }
        Some("texts") => {
            let p = args.get(1).ok_or("缺少文件")?;
            texts(p, args.get(2).map(|s| s.as_str()))
        }
        Some("types") => {
            let p = args.get(1).ok_or("缺少文件")?;
            types(p)
        }
        Some("outline") => {
            let p = args.get(1).ok_or("缺少文件")?;
            outline(p)
        }
        Some("set-note") | Some("append-note") => {
            if args.len() < 5 {
                eprintln!("用法: kn set-note <src.key> <dst.key> <页码> <文本>");
                std::process::exit(2);
            }
            let page: usize = args[3].parse().map_err(|_| "页码不是整数")?;
            let append = args[0] == "append-note";
            write::set_note_text(&args[1], &args[2], page, &args[4], append)
                .map_err(|e| e.to_string())?;
            println!("已写出 {}", args[2]);
            Ok(())
        }
        Some("set-text") => {
            if args.len() < 6 {
                eprintln!("用法: kn set-text <src.key> <dst.key> <页码> <title|body> <新文本>");
                std::process::exit(2);
            }
            let page: usize = args[3].parse().map_err(|_| "页码不是整数")?;
            write::set_storage_text(&args[1], &args[2], page, &args[4], &args[5]).map_err(|e| e.to_string())?;
            println!("已写出 {}", args[2]);
            Ok(())
        }
        _ => {
            usage();
            std::process::exit(2);
        }
    }
}
