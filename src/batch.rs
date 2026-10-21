//! Apply one <src> <dst> subcommand to every .key in a directory.
use std::path::Path;
use std::process::Command;

pub fn run(in_dir: &str, out_dir: &str, sub: &str, rest: &[String]) -> Result<(), String> {
    if sub == "batch" {
        return Err("batch 不能递归调用自身".into());
    }
    let inp = Path::new(in_dir);
    if !inp.is_dir() {
        return Err(format!("输入目录不存在：{in_dir}"));
    }
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut files: Vec<_> = std::fs::read_dir(inp)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "key").unwrap_or(false))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(format!("{in_dir} 里没有 .key 文件"));
    }
    let mut ok = 0usize;
    let mut failed = Vec::new();
    for f in &files {
        let src = f.to_string_lossy().to_string();
        let name = f.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let dst = Path::new(out_dir).join(&name).to_string_lossy().to_string();
        let mut cmd = Command::new(&exe);
        cmd.arg(sub).arg(&src).arg(&dst).args(rest);
        let status = cmd.status().map_err(|e| e.to_string())?;
        if status.success() {
            ok += 1;
            println!("✅ {name}");
        } else {
            failed.push(name.clone());
            println!("❌ {name}（退出码 {:?}）", status.code());
        }
    }
    println!("\n共 {} 个文件：成功 {ok}，失败 {}", files.len(), failed.len());
    if !failed.is_empty() {
        return Err(format!("失败：{}", failed.join("、")));
    }
    Ok(())
}
