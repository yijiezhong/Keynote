//! Add a build animation by cloning an existing BuildArchive/BuildChunkArchive.
use prost::Message;

use crate::iwa::{self, Node, NodeValue, Segment};
use crate::{kn, schema, shape, tsp, write};

fn max_ident(docs: &[write::Doc]) -> u64 {
    let mut top = 0;
    for d in docs { for s in &d.segs { if let Some(id) = iwa::ident_of(s) { top = top.max(id); } } }
    top
}

fn segment(id: u64, type_id: u64, blob: Vec<u8>) -> Segment {
    let mi = vec![
        Node { field: 1, value: NodeValue::Varint(type_id) },
        Node { field: 2, value: NodeValue::Bytes(vec![1, 0, 5]) },
        Node { field: 3, value: NodeValue::Varint(blob.len() as u64) },
    ];
    let ainfo = vec![
        Node { field: 1, value: NodeValue::Varint(id) },
        Node { field: 2, value: NodeValue::Bytes(iwa::pb_serialize_nodes(&mi)) },
    ];
    Segment { ainfo, objs: vec![blob], meta: vec![(type_id, 0)] }
}

pub fn add_build(src: &str, dst: &str, page: usize, path: &str, effect: &str) -> Result<(), String> {
    let mut docs = write::load_docs(src)?;
    let map = write::ident_map(&docs);
    let slides = write::slides(&map);
    if page == 0 || page > slides.len() { return Err(format!("页码越界：1..{}", slides.len())); }
    let target = shape::resolve_path(&map, &slides[page - 1], path).ok_or("找不到形状路径")?;

    let mut tmpl: Option<(kn::BuildArchive, kn::BuildChunkArchive)> = None;
    for sa in &slides {
        let (Some(b), Some(c)) = (sa.builds.first(), sa.build_chunks.first()) else { continue };
        if let (Some((_, bb)), Some((_, cb))) = (map.get(&b.identifier), map.get(&c.identifier)) {
            let b = <kn::BuildArchive as Message>::decode(bb.as_slice()).ok();
            let c = <kn::BuildChunkArchive as Message>::decode(cb.as_slice()).ok();
            if let (Some(b), Some(c)) = (b, c) { tmpl = Some((b, c)); break; }
        }
    }
    let (mut b, mut c) = tmpl.ok_or("文件里没有可复制的动画模板（先用 Keynote 加一个动画）")?;
    let top = max_ident(&docs);
    let build_id = top + 1;
    let chunk_id = top + 2;
    b.drawable = Some(tsp::Reference { identifier: target, ..Default::default() });
    if let Some(attrs) = b.attributes.animation_attributes.as_mut() {
        attrs.effect = Some(effect.to_string());
        attrs.duration = Some(1.0);
        attrs.delay = Some(0.0);
    }
    c.build = Some(tsp::Reference { identifier: build_id, ..Default::default() });
    if let Some(bcid) = c.build_chunk_identifier.as_mut() {
        if let Some(bid) = bcid.build_id.as_mut() {
            bid.lower = 0x9e3779b97f4a7c15; bid.upper = 0xbf58476d1ce4e5b9;
        }
    }
    let ids = write::slide_node_ids(&map);
    let node = <kn::SlideNodeArchive as Message>::decode(map.get(&ids[page - 1]).ok_or("节点不存在")?.1.as_slice()).map_err(|e| e.to_string())?;
    let page_id = node.slide.as_ref().ok_or("该页没有 SlideArchive")?.identifier;
    let (di, si, oi) = write::find_object(&docs, page_id, "slidearchive").ok_or("找不到页对象")?;
    let mut sa = <kn::SlideArchive as Message>::decode(docs[di].segs[si].objs[oi].as_slice()).map_err(|e| e.to_string())?;
    sa.builds.push(tsp::Reference { identifier: build_id, ..Default::default() });
    sa.build_chunks.push(tsp::Reference { identifier: chunk_id, ..Default::default() });
    docs[di].segs[si].objs[oi] = sa.encode_to_vec();
    let bt = schema::type_id("KN.BuildArchive").ok_or("registry 缺少 BuildArchive")?;
    let ct = schema::type_id("KN.BuildChunkArchive").ok_or("registry 缺少 BuildChunkArchive")?;
    docs[di].segs.push(segment(build_id, bt, b.encode_to_vec()));
    docs[di].segs.push(segment(chunk_id, ct, c.encode_to_vec()));
    write::save_docs(src, dst, &docs)?;
    println!("第 {page} 页：已给 {path} 加构建动画 {effect}（build={build_id}）");
    Ok(())
}
