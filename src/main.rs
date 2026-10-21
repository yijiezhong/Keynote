// D3 spike：不依赖 Python，从零读 .key 并输出与 spec-1 一致的验收数字。
// prost-build 每个 proto package 生成一个文件，跨 package 用 super::xxx 引用
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

fn usage() {
    println!("kn (Rust) — Apple Keynote .key 工具，阶段 1：只读");
    println!();
    println!("用法：");
    println!("  kn list   <deck.key>     列出 zip 条目");
    println!("  kn verify <deck.key>     IWA / 段 / 对象 / slides 统计");
    println!("  kn info   <deck.key>     verify 的别名");
    println!("  kn help                  显示本帮助");
    println!();
    println!("写回与编辑命令（outline、set-text、new、add-slide 等）正在迁移。");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let cmd = args.next();
    let path = args.next();
    match (cmd.as_deref(), path.as_deref()) {
        (None, _) | (Some("help"), _) | (Some("--help"), _) | (Some("-h"), _) => {
            usage();
            Ok(())
        }
        (Some("verify"), Some(p)) | (Some("info"), Some(p)) => verify(p),
        (Some("list"), Some(p)) => list(p),
        _ => {
            usage();
            std::process::exit(2);
        }
    }
}
