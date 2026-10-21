//! Type-id → protobuf full name, generated from proto/registry.json.
use std::collections::HashMap;
use std::sync::OnceLock;

pub fn type_name(id: u64) -> Option<&'static str> {
    static MAP: OnceLock<HashMap<u64, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut m = HashMap::new();
        for line in include_str!("../proto/registry.json").lines() {
            let line = line.trim();
            let Some((k, v)) = line.split_once(':') else { continue };
            let Ok(id) = k.trim().trim_matches('"').parse::<u64>() else { continue };
            let name = v.trim().trim_end_matches(',').trim().trim_matches('"');
            if !name.is_empty() {
                m.insert(id, Box::leak(name.to_string().into_boxed_str()) as &'static str);
            }
        }
        m
    }).get(&id).copied()
}
