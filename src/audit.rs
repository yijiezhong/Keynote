//! Structural audit and presenter-note checks.
use std::collections::HashMap;

use crate::{iwa, write};

pub fn audit(path: &str) -> Result<(), String> {
    let docs = write::load_docs(path)?;
    let mut segments = 0usize;
    let mut objects = 0usize;
    let mut idents: HashMap<u64, usize> = HashMap::new();
    for d in &docs {
        for seg in &d.segs {
            segments += 1;
            objects += seg.objs.len();
            if let Some(id) = iwa::ident_of(seg) {
                *idents.entry(id).or_insert(0) += 1;
            }
        }
    }
    let map = write::ident_map(&docs);
    let slides = write::slides(&map);
    let dups: Vec<u64> = idents.iter().filter(|(_, c)| **c > 1).map(|(id, _)| *id).collect();
    println!("IWA 文档：{} 个", docs.len());
    println!("段：{segments}，对象：{objects}，页面：{}", slides.len());
    if dups.is_empty() {
        println!("结构检查：通过（identifier 无重复）");
        Ok(())
    } else {
        println!("结构检查：发现 {} 个重复 identifier：{:?}", dups.len(), dups);
        Err("identifier 重复".into())
    }
}

pub fn check_notes(path: &str, requires: &[String]) -> Result<(), String> {
    let docs = write::load_docs(path)?;
    let map = write::ident_map(&docs);
    let slides = write::slides(&map);
    if slides.is_empty() {
        return Err("未找到幻灯片树".into());
    }
    let mut missing = 0usize;
    for (i, sa) in slides.iter().enumerate() {
        let note = write::note_text(&map, sa).unwrap_or_default();
        let lack: Vec<&str> = requires.iter().filter(|r| !note.contains(r.as_str())).map(|s| s.as_str()).collect();
        if !lack.is_empty() {
            missing += 1;
            println!("第 {} 页缺少：{}", i + 1, lack.join("、"));
        }
    }
    if missing == 0 {
        println!("共 {} 页，备注检查通过", slides.len());
        Ok(())
    } else {
        Err(format!("{} / {} 页缺少必需备注", missing, slides.len()))
    }
}
