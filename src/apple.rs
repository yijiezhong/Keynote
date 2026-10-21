//! AppleScript bridge for operations that must be done by Keynote itself.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_script() -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    std::env::temp_dir().join(format!("kn_as_{}_{}.applescript", std::process::id(), nanos))
}

pub fn run(script: &str, args: &[String]) -> Result<String, String> {
    let path = temp_script();
    fs::write(&path, script).map_err(|e| e.to_string())?;
    let out = Command::new("osascript")
        .arg(&path)
        .args(args)
        .output()
        .map_err(|e| e.to_string());
    let _ = fs::remove_file(&path);
    let out = out?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() { "osascript 执行失败".into() } else { err });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

const NEW_DECK: &str = r#"
on run argv
	set outPath to item 1 of argv
	set themeName to item 2 of argv
	set layoutName to item 3 of argv
	set titleText to item 4 of argv
	set bodyText to item 5 of argv
	tell application "Keynote"
		set themeRef to theme themeName
		set d to make new document with properties {document theme: themeRef}
		set L to missing value
		repeat with x in slide layouts of d
			if (name of x) is layoutName then
				set L to x
				exit repeat
			end if
		end repeat
		if L is missing value then
			close d saving no
			error "找不到版式：" & layoutName
		end if
		set s to slide 1 of d
		set base layout of s to L
		if titleText is not "" then set object text of default title item of s to titleText
		if bodyText is not "" then set object text of default body item of s to bodyText
		save d in POSIX file outPath
		close d saving no
	end tell
	return "ok"
end run
"#;

pub fn new_deck(
    out: &str,
    theme: Option<&str>,
    layout: Option<&str>,
    title: &str,
    body: &str,
    force: bool,
) -> Result<(String, String), String> {
    let out_path = Path::new(out);
    if out_path.exists() && !force {
        return Err(format!("输出已存在：{out}（加 --force 覆盖）"));
    }
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let abs = if out_path.is_absolute() {
        out_path.to_path_buf()
    } else {
        std::env::current_dir().map_err(|e| e.to_string())?.join(out_path)
    };
    let abs = abs.to_string_lossy().to_string();

    let themes: Vec<String> = match theme {
        Some(t) => vec![t.to_string()],
        None => vec!["基本（白色）".into(), "Basic White".into()],
    };
    let layouts: Vec<String> = match layout {
        Some(l) => vec![l.to_string()],
        None => vec!["标题与项目符号".into(), "Title & Bullets".into()],
    };
    let mut errors = Vec::new();
    for t in &themes {
        for l in &layouts {
            let args = vec![abs.clone(), t.clone(), l.clone(), title.to_string(), body.to_string()];
            match run(NEW_DECK, &args) {
                Ok(_) => {
                    let size = fs::metadata(&abs).map(|m| m.len()).unwrap_or(0);
                    if size > 1000 {
                        return Ok((t.clone(), l.clone()));
                    }
                    errors.push(format!("{t} / {l}: 生成文件过小 {size}"));
                }
                Err(e) => {
                    errors.push(format!("{t} / {l}: {e}"));
                    let _ = fs::remove_file(&abs);
                }
            }
        }
    }
    Err(format!("创建失败：{}", errors.last().cloned().unwrap_or_default()))
}


const ADD_SLIDE: &str = r#"
on run argv
	set srcPath to item 1 of argv
	set dstPath to item 2 of argv
	set layoutName to item 3 of argv
	set titleText to item 4 of argv
	set bodyText to item 5 of argv
	set afterPos to (item 6 of argv) as integer
	set srcName to item 7 of argv
	set replacePage to (item 8 of argv) as integer
	tell application "Keynote"
		set d to missing value
		repeat with x in documents
			if (name of x as text) is srcName then set d to x
		end repeat
		if d is missing value then return "ERR_NO_DOC"
		set L to missing value
		set names to ""
		repeat with x in slide layouts of d
			set nm to name of x as text
			set names to names & nm & "|"
			if nm is layoutName then set L to x
		end repeat
		if L is missing value then
			close d saving no
			return "ERR_NO_LAYOUT|" & names
		end if
		set total to count of slides of d
		if replacePage is greater than 0 then
			set oldSlide to slide replacePage of d
			set oldTitle to ""
			set oldBody to ""
			try
				set oldTitle to object text of default title item of oldSlide as text
			end try
			try
				set oldBody to object text of default body item of oldSlide as text
			end try
			set s to make new slide at after slide replacePage of d with properties {base layout: L}
			if oldTitle is not "" then
				try
					set object text of default title item of s to oldTitle
				end try
			end if
			if oldBody is not "" then
				try
					set object text of default body item of s to oldBody
				end try
			end if
			delete oldSlide
		else
			if afterPos is less than or equal to 0 or afterPos is greater than or equal to total then
				set s to make new slide at end of slides of d with properties {base layout: L}
			else
				set s to make new slide at after slide afterPos of d with properties {base layout: L}
			end if
			if titleText is not "" then
				try
					set object text of default title item of s to titleText
				end try
			end if
			if bodyText is not "" then
				try
					set object text of default body item of s to bodyText
				end try
			end if
		end if
		save d in POSIX file dstPath
		set n to count of slides of d
		close d saving no
		return "OK|" & n
	end tell
end run
"#;

pub fn add_slide(
    src: &str,
    dst: &str,
    layout: &str,
    title: &str,
    body: &str,
    after: i64,
    replace: i64,
) -> Result<usize, String> {
    let src_abs = std::path::absolute(src).map_err(|e| e.to_string())?;
    let dst_abs = std::path::absolute(dst).map_err(|e| e.to_string())?;
    let src_name = Path::new(src)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .ok_or("源路径缺少文件名")?;
    let _ = run(r#"tell application "Keynote" to close every document saving no"#, &[]);
    let _ = Command::new("open")
        .args(["-b", "com.apple.Keynote", &src_abs.to_string_lossy()])
        .status();
    std::thread::sleep(std::time::Duration::from_secs(2));
    let args = vec![
        src_abs.to_string_lossy().to_string(),
        dst_abs.to_string_lossy().to_string(),
        layout.to_string(),
        title.to_string(),
        body.to_string(),
        after.to_string(),
        src_name,
        replace.to_string(),
    ];
    let out = run(ADD_SLIDE, &args)?;
    if let Some(rest) = out.strip_prefix("OK|") {
        return rest.parse::<usize>().map_err(|e| e.to_string());
    }
    if let Some(rest) = out.strip_prefix("ERR_NO_LAYOUT|") {
        let names: Vec<&str> = rest.split('|').filter(|s| !s.is_empty()).collect();
        return Err(format!("找不到版式「{layout}」；可用：{}", names.join("、")));
    }
    if out == "ERR_NO_DOC" {
        return Err(format!("Keynote 未打开源文件：{src}"));
    }
    Err(format!("未预期返回：{out}"))
}
