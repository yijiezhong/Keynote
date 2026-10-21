//! IWA / Snappy / protobuf byte-level primitives.
//! Ported from the private Python reference; no Python runtime is used.


pub fn uvarint(buf: &[u8], mut i: usize) -> Result<(u64, usize), String> {
    let mut v = 0u64;
    let mut shift = 0u32;
    loop {
        let b = *buf.get(i).ok_or("varint 越界")?;
        i += 1;
        v |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            return Ok((v, i));
        }
        shift += 7;
        if shift > 63 {
            return Err("varint 过长".into());
        }
    }
}

pub fn uvarint_encode(out: &mut Vec<u8>, mut n: u64) {
    loop {
        let b = (n & 0x7f) as u8;
        n >>= 7;
        if n != 0 {
            out.push(b | 0x80);
        } else {
            out.push(b);
            return;
        }
    }
}

pub fn snappy_decompress(src: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut i = 0usize;
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

/// Literal-only Snappy encoder. Valid output; Keynote accepts it.
pub fn snappy_compress(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 16);
    uvarint_encode(&mut out, data.len() as u64);
    for piece in data.chunks(60) {
        let n = piece.len();
        if n == 0 {
            continue;
        }
        if n <= 60 {
            out.push(((n - 1) as u8) << 2);
        }
        out.extend_from_slice(piece);
    }
    out
}

pub fn iwa_unwrap(blob: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < blob.len() {
        if blob[i] != 0x00 {
            return Err(format!("非预期 IWA 头标记 0x{:02x} @{i}", blob[i]));
        }
        if i + 4 > blob.len() {
            return Err(format!("chunk 头被截断 @{i}"));
        }
        let clen = (blob[i + 1] as usize)
            | ((blob[i + 2] as usize) << 8)
            | ((blob[i + 3] as usize) << 16);
        if i + 4 + clen > blob.len() {
            return Err(format!("chunk 越界 @{i} len={clen}"));
        }
        let chunk = &blob[i + 4..i + 4 + clen];
        out.extend_from_slice(&snappy_decompress(chunk)?);
        i += 4 + clen;
    }
    Ok(out)
}

pub fn iwa_wrap(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() / 2 + 16);
    for chunk in payload.chunks(1 << 20) {
        let comp = snappy_compress(chunk);
        out.push(0x00);
        out.push((comp.len() & 0xff) as u8);
        out.push(((comp.len() >> 8) & 0xff) as u8);
        out.push(((comp.len() >> 16) & 0xff) as u8);
        out.extend_from_slice(&comp);
    }
    out
}

#[derive(Clone, Debug)]
pub enum NodeValue {
    Varint(u64),
    Bytes(Vec<u8>),
    F32([u8; 4]),
    F64([u8; 8]),
}

#[derive(Clone, Debug)]
pub struct Node {
    pub field: u64,
    pub value: NodeValue,
}

pub fn pb_parse_nodes(buf: &[u8]) -> Result<Vec<Node>, String> {
    let mut nodes = Vec::new();
    let mut i = 0usize;
    while i < buf.len() {
        let (key, j) = uvarint(buf, i)?;
        i = j;
        let field = key >> 3;
        let wt = key & 0x07;
        if field == 0 {
            return Err("field 0 非法".into());
        }
        let value = match wt {
            0 => {
                let (v, j) = uvarint(buf, i)?;
                i = j;
                NodeValue::Varint(v)
            }
            2 => {
                let (ln, j) = uvarint(buf, i)?;
                i = j;
                let ln = ln as usize;
                if i + ln > buf.len() {
                    return Err("length-delimited 越界".into());
                }
                let v = buf[i..i + ln].to_vec();
                i += ln;
                NodeValue::Bytes(v)
            }
            5 => {
                if i + 4 > buf.len() {
                    return Err("f32 越界".into());
                }
                let mut a = [0u8; 4];
                a.copy_from_slice(&buf[i..i + 4]);
                i += 4;
                NodeValue::F32(a)
            }
            1 => {
                if i + 8 > buf.len() {
                    return Err("f64 越界".into());
                }
                let mut a = [0u8; 8];
                a.copy_from_slice(&buf[i..i + 8]);
                i += 8;
                NodeValue::F64(a)
            }
            _ => return Err(format!("不支持的 wire type {wt}")),
        };
        nodes.push(Node { field, value });
    }
    Ok(nodes)
}

pub fn pb_serialize_nodes(nodes: &[Node]) -> Vec<u8> {
    let mut out = Vec::new();
    for n in nodes {
        match &n.value {
            NodeValue::Varint(v) => {
                uvarint_encode(&mut out, n.field << 3);
                uvarint_encode(&mut out, *v);
            }
            NodeValue::Bytes(b) => {
                uvarint_encode(&mut out, (n.field << 3) | 2);
                uvarint_encode(&mut out, b.len() as u64);
                out.extend_from_slice(b);
            }
            NodeValue::F32(b) => {
                uvarint_encode(&mut out, (n.field << 3) | 5);
                out.extend_from_slice(b);
            }
            NodeValue::F64(b) => {
                uvarint_encode(&mut out, (n.field << 3) | 1);
                out.extend_from_slice(b);
            }
        }
    }
    out
}

pub fn pb_deep_strings(
    buf: &[u8],
    depth: usize,
    out: &mut Vec<(usize, u64, String)>,
) -> Result<(), String> {
    let mut i = 0usize;
    while i < buf.len() {
        let (key, j) = uvarint(buf, i)?;
        i = j;
        let field = key >> 3;
        let wt = key & 0x07;
        if field == 0 {
            return Err("field 0 非法".into());
        }
        match wt {
            0 => {
                let (_v, j) = uvarint(buf, i)?;
                i = j;
            }
            2 => {
                let (ln, j) = uvarint(buf, i)?;
                i = j;
                let ln = ln as usize;
                if i + ln > buf.len() {
                    return Err("length-delimited 越界".into());
                }
                let raw = &buf[i..i + ln];
                i += ln;
                if let Ok(text) = std::str::from_utf8(raw) {
                    let printable = text.chars().all(|c| c >= ' ' || c == '\n' || c == '\t');
                    let meaningful = text.chars().any(|c| (c as u32) > 0x7f || c.is_alphabetic());
                    if text.chars().count() >= 2 && printable && meaningful {
                        out.push((depth, field, text.to_string()));
                    }
                }
                let _ = pb_deep_strings(raw, depth + 1, out);
            }
            5 => {
                if i + 4 > buf.len() {
                    return Err("f32 越界".into());
                }
                i += 4;
            }
            1 => {
                if i + 8 > buf.len() {
                    return Err("f64 越界".into());
                }
                i += 8;
            }
            _ => return Err(format!("不支持的 wire type {wt}")),
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Segment {
    pub ainfo: Vec<Node>,
    pub objs: Vec<Vec<u8>>,
    pub meta: Vec<(u64, u64)>,
}

pub fn parse_segments(payload: &[u8]) -> Result<Vec<Segment>, String> {
    let mut segs = Vec::new();
    let mut i = 0usize;
    while i < payload.len() {
        let (alen, j) = uvarint(payload, i)?;
        i = j;
        let alen = alen as usize;
        if i + alen > payload.len() {
            return Err(format!("ArchiveInfo 越界 @{i}"));
        }
        let ainfo = pb_parse_nodes(&payload[i..i + alen])?;
        i += alen;
        let mut objs = Vec::new();
        let mut meta = Vec::new();
        for n in &ainfo {
            if n.field != 2 {
                continue;
            }
            let NodeValue::Bytes(mi_bytes) = &n.value else {
                continue;
            };
            let mi = pb_parse_nodes(mi_bytes)?;
            let mut mtype = 0u64;
            let mut length: Option<u64> = None;
            for f in &mi {
                if let NodeValue::Varint(v) = f.value {
                    if f.field == 1 {
                        mtype = v;
                    } else if f.field == 3 {
                        length = Some(v);
                    }
                }
            }
            let ln = length.ok_or("MessageInfo 缺少 length(field 3)")? as usize;
            if i + ln > payload.len() {
                return Err(format!("object 越界 @{i} len={ln}"));
            }
            objs.push(payload[i..i + ln].to_vec());
            meta.push((mtype, ln as u64));
            i += ln;
        }
        segs.push(Segment { ainfo, objs, meta });
    }
    Ok(segs)
}

pub fn serialize_segments(segs: &[Segment]) -> Vec<u8> {
    let mut out = Vec::new();
    for seg in segs {
        let mut ainfo = seg.ainfo.clone();
        let mut oi = 0usize;
        for n in &mut ainfo {
            if n.field != 2 {
                continue;
            }
            let NodeValue::Bytes(mi_bytes) = &n.value else {
                continue;
            };
            let mut mi = pb_parse_nodes(mi_bytes).unwrap_or_default();
            if oi >= seg.objs.len() {
                break;
            }
            for f in &mut mi {
                if f.field == 3 && matches!(f.value, NodeValue::Varint(_)) {
                    f.value = NodeValue::Varint(seg.objs[oi].len() as u64);
                }
            }
            n.value = NodeValue::Bytes(pb_serialize_nodes(&mi));
            oi += 1;
        }
        let ainfo_bytes = pb_serialize_nodes(&ainfo);
        uvarint_encode(&mut out, ainfo_bytes.len() as u64);
        out.extend_from_slice(&ainfo_bytes);
        for o in &seg.objs {
            out.extend_from_slice(o);
        }
    }
    out
}

pub fn ident_of(seg: &Segment) -> Option<u64> {
    for n in &seg.ainfo {
        if n.field == 1 {
            if let NodeValue::Varint(v) = n.value {
                return Some(v);
            }
        }
    }
    None
}
