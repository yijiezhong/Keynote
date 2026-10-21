//! AppleScript bridge for operations that must be done by Keynote itself.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{schema, tst, write};

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


const INSERT_IMAGE: &str = r#"
on run argv
	set imgPath to item 1 of argv
	set pageNum to (item 2 of argv) as integer
	tell application "Keynote"
		activate
		delay 1
		set d to document 1
		set s to slide pageNum of d
		if pageNum > 1 then
			show s
			delay 1
		end if
		set n0 to count of images of s
	end tell
	repeat with attempt from 1 to 3
		set the clipboard to ""
		delay 0.3
		set the clipboard to (read (POSIX file imgPath) as JPEG picture)
		delay 1
		tell application "Keynote"
			activate
			delay 1
		end tell
		tell application "System Events"
			keystroke "v" using command down
		end tell
		delay 3
		tell application "Keynote"
			set d to document 1
			set s to slide pageNum of d
			set n1 to count of images of s
			if n1 > n0 then
				save d
				close d saving no
				return "ok:" & n1
			end if
		end tell
	end repeat
	tell application "Keynote"
		close document 1 saving no
	end tell
	error "粘贴 3 次仍未插入图片（剪贴板或辅助功能权限异常）"
end run
"#;

pub fn insert_image_paste(src: &str, dst: &str, page: usize, image: &str) -> Result<usize, String> {
    let src_abs = std::path::absolute(src).map_err(|e| e.to_string())?;
    let dst_abs = std::path::absolute(dst).map_err(|e| e.to_string())?;
    let img_abs = std::path::absolute(image).map_err(|e| e.to_string())?;
    fs::copy(&src_abs, &dst_abs).map_err(|e| e.to_string())?;
    let _ = run(r#"tell application "Keynote" to close every document saving no"#, &[]);
    let _ = Command::new("open")
        .args(["-b", "com.apple.Keynote", &dst_abs.to_string_lossy()])
        .status();
    std::thread::sleep(std::time::Duration::from_secs(2));
    let args = vec![img_abs.to_string_lossy().to_string(), page.to_string()];
    let out = run(INSERT_IMAGE, &args)?;
    if let Some(rest) = out.strip_prefix("ok:") {
        return rest.parse::<usize>().map_err(|e| e.to_string());
    }
    Err(format!("粘贴失败：{out}"))
}


const DELETE_SLIDE: &str = r#"
on run argv
	set dstPath to item 1 of argv
	set pageNum to (item 2 of argv) as integer
	tell application "Keynote"
		set d to document 1
		delete slide pageNum of d
		save d in POSIX file dstPath
		set n to count of slides of d
		close d saving no
		return "ok:" & n
	end tell
end run
"#;

const DUPLICATE_SLIDE: &str = r#"
on run argv
	set dstPath to item 1 of argv
	set pageNum to (item 2 of argv) as integer
	tell application "Keynote"
		set d to document 1
		duplicate slide pageNum of d to after slide pageNum of d
		save d in POSIX file dstPath
		set n to count of slides of d
		close d saving no
		return "ok:" & n
	end tell
end run
"#;

fn open_keynote_doc(src: &str) -> Result<(), String> {
    let src_abs = std::path::absolute(src).map_err(|e| e.to_string())?;
    let _ = run(r#"tell application "Keynote" to close every document saving no"#, &[]);
    let _ = Command::new("open")
        .args(["-b", "com.apple.Keynote", &src_abs.to_string_lossy()])
        .status();
    std::thread::sleep(std::time::Duration::from_secs(2));
    Ok(())
}

fn slide_op(src: &str, dst: &str, page: usize, script: &str, op: &str) -> Result<usize, String> {
    let dst_abs = std::path::absolute(dst).map_err(|e| e.to_string())?;
    open_keynote_doc(src)?;
    let out = run(script, &[dst_abs.to_string_lossy().to_string(), page.to_string()])?;
    if let Some(rest) = out.strip_prefix("ok:") {
        return rest.parse::<usize>().map_err(|e| e.to_string());
    }
    Err(format!("{op} 失败：{out}"))
}

pub fn delete_slide(src: &str, dst: &str, page: usize) -> Result<usize, String> {
    slide_op(src, dst, page, DELETE_SLIDE, "删除页")
}

pub fn duplicate_slide(src: &str, dst: &str, page: usize) -> Result<usize, String> {
    slide_op(src, dst, page, DUPLICATE_SLIDE, "复制页")
}


const TABLE_FILL: &str = r#"
on colorOf(s)
	if s is "" then return missing value
	set AppleScript's text item delimiters to ","
	set parts to text items of s
	set AppleScript's text item delimiters to ""
	return {item 1 of parts as integer, item 2 of parts as integer, item 3 of parts as integer}
end colorOf

on run argv
	set dstPath to item 1 of argv
	set pageNum to (item 2 of argv) as integer
	set tableIdx to (item 3 of argv) as integer
	set hrArg to (item 4 of argv) as integer
	set bodyColor to item 5 of argv
	set headerColor to item 6 of argv
	set footerColor to item 7 of argv
	set hcColor to item 8 of argv
	set bandColor to item 9 of argv
	tell application "Keynote"
		set d to document 1
		set t to table tableIdx of slide pageNum of d
		set nRows to count of rows of t
		set nCols to count of columns of t
		set nHr to header row count of t
		if hrArg > 0 then set nHr to hrArg
		set nFr to footer row count of t
		set nHc to header column count of t
		set bodyCol to my colorOf(bodyColor)
		set headerCol to my colorOf(headerColor)
		set footerCol to my colorOf(footerColor)
		set hcCol to my colorOf(hcColor)
		set bandCol to my colorOf(bandColor)
		if bodyCol is not missing value then
			repeat with r from (nHr + 1) to (nRows - nFr)
				repeat with c from (nHc + 1) to nCols
					set background color of cell c of row r of t to bodyCol
				end repeat
			end repeat
		end if
		if headerCol is not missing value then
			repeat with r from 1 to nHr
				repeat with c from 1 to nCols
					set background color of cell c of row r of t to headerCol
				end repeat
			end repeat
		end if
		if footerCol is not missing value then
			repeat with r from (nRows - nFr + 1) to nRows
				repeat with c from 1 to nCols
					set background color of cell c of row r of t to footerCol
				end repeat
			end repeat
		end if
		if hcCol is not missing value then
			repeat with r from (nHr + 1) to (nRows - nFr)
				repeat with c from 1 to nHc
					set background color of cell c of row r of t to hcCol
				end repeat
			end repeat
		end if
		if bandCol is not missing value then
			repeat with r from (nHr + 1) to (nRows - nFr)
				if ((r - hr) mod 2) is 0 then
					repeat with c from (nHc + 1) to nCols
						set background color of cell c of row r of t to bandCol
					end repeat
				end if
			end repeat
		end if
		save d in POSIX file dstPath
		close d saving no
		return "ok"
	end tell
end run
"#;

fn table_page_index(src: &str, ident: u64) -> Result<(usize, usize), String> {
    let docs = write::load_docs(src)?;
    let map = write::ident_map(&docs);
    let slides = write::slides(&map);
    for (page, sa) in slides.iter().enumerate() {
        let mut idx = 0usize;
        for r in &sa.owned_drawables {
            let Some((t, b)) = map.get(&r.identifier) else { continue };
            if schema::type_name(*t) == Some("TST.TableInfoArchive") {
                if let Ok(ti) = <tst::TableInfoArchive as prost::Message>::decode(b.as_slice()) {
                    idx += 1;
                    if ti.table_model.identifier == ident {
                        return Ok((page + 1, idx));
                    }
                }
            }
        }
    }
    Err(format!("在页面上找不到表格 {ident}"))
}

fn hex_color(s: &str) -> Result<String, String> {
    let s = s.trim().trim_start_matches('#');
    if s.is_empty() { return Ok(String::new()); }
    if s.len() != 6 { return Err("颜色格式应为 #RRGGBB".into()); }
    let r = u8::from_str_radix(&s[0..2], 16).map_err(|_| "颜色格式错误")?;
    let g = u8::from_str_radix(&s[2..4], 16).map_err(|_| "颜色格式错误")?;
    let b = u8::from_str_radix(&s[4..6], 16).map_err(|_| "颜色格式错误")?;
    Ok(format!("{},{},{}", r as u32 * 65535 / 255, g as u32 * 65535 / 255, b as u32 * 65535 / 255))
}

pub fn set_table_fill(
    src: &str,
    dst: &str,
    ident: u64,
    body: Option<&str>,
    header: Option<&str>,
    footer: Option<&str>,
    header_col: Option<&str>,
    band: Option<&str>,
    header_rows: i64,
) -> Result<(), String> {
    let (page, idx) = table_page_index(src, ident)?;
    let dst_abs = std::path::absolute(dst).map_err(|e| e.to_string())?;
    open_keynote_doc(src)?;
    let h = hex_color(body.unwrap_or(""))?;
    let hd = hex_color(header.unwrap_or(""))?;
    let ft = hex_color(footer.unwrap_or(""))?;
    let hc = hex_color(header_col.unwrap_or(""))?;
    let bd = hex_color(band.unwrap_or(""))?;
    let args = vec![
        dst_abs.to_string_lossy().to_string(),
        page.to_string(),
        idx.to_string(),
        header_rows.to_string(),
        h, hd, ft, hc, bd,
    ];
    let out = run(TABLE_FILL, &args)?;
    if out != "ok" { return Err(format!("设置表格底色失败：{out}")); }
    Ok(())
}


const INSERT_IMAGE_NEW: &str = r#"
on run argv
	set dstPath to item 1 of argv
	set pageNum to (item 2 of argv) as integer
	set imgPath to item 3 of argv
	set xPos to item 4 of argv
	set yPos to item 5 of argv
	set wVal to item 6 of argv
	tell application "Keynote"
		activate
		delay 1
		set d to document 1
		set s to slide pageNum of d
		tell s
			set im to make new image with properties {file:POSIX file imgPath}
		end tell
		if xPos is not "" and yPos is not "" then set position of im to {(xPos as real), (yPos as real)}
		if wVal is not "" then set width of im to (wVal as real)
		set n to count of images of s
		save d in POSIX file dstPath
		close d saving no
		return "ok:" & n
	end tell
end run
"#;

pub fn insert_image(
    src: &str,
    dst: &str,
    page: usize,
    image: &str,
    x: Option<f64>,
    y: Option<f64>,
    width: Option<f64>,
) -> Result<usize, String> {
    let dst_abs = std::path::absolute(dst).map_err(|e| e.to_string())?;
    let img_abs = std::path::absolute(image).map_err(|e| e.to_string())?;
    open_keynote_doc(src)?;
    let args = vec![
        dst_abs.to_string_lossy().to_string(),
        page.to_string(),
        img_abs.to_string_lossy().to_string(),
        x.map(|v| v.to_string()).unwrap_or_default(),
        y.map(|v| v.to_string()).unwrap_or_default(),
        width.map(|v| v.to_string()).unwrap_or_default(),
    ];
    let out = run(INSERT_IMAGE_NEW, &args)?;
    if let Some(rest) = out.strip_prefix("ok:") {
        return rest.parse::<usize>().map_err(|e| e.to_string());
    }
    Err(format!("插入图片失败：{out}"))
}


fn col_letter(mut n: u32) -> String {
    let mut s = String::new();
    while n > 0 {
        let r = ((n - 1) % 26) as u8;
        s.insert(0, (b'A' + r) as char);
        n = (n - 1) / 26;
    }
    s
}

const MERGE_CELLS: &str = r#"
on run argv
	set dstPath to item 1 of argv
	set pageNum to (item 2 of argv) as integer
	set tableIdx to (item 3 of argv) as integer
	set rangeText to item 4 of argv
	tell application "Keynote"
		activate
		delay 1
		set d to document 1
		set t to table tableIdx of slide pageNum of d
		set selection range of t to range rangeText of t
		delay 1
	end tell
	tell application "System Events"
		tell process "Keynote"
			click menu item "合并单元格" of menu 1 of menu item "表格" of menu 1 of menu bar item "格式" of menu bar 1
		end tell
	end tell
	delay 1
	tell application "Keynote"
		set d to document 1
		save d in POSIX file dstPath
		close d saving no
	end tell
	return "ok"
end run
"#;

pub fn merge_cells(src: &str, dst: &str, ident: u64, r1: u32, c1: u32, r2: u32, c2: u32) -> Result<(), String> {
    let (page, idx) = table_page_index(src, ident)?;
    let dst_abs = std::path::absolute(dst).map_err(|e| e.to_string())?;
    let range = format!("{}{}:{}{}", col_letter(c1 + 1), r1 + 1, col_letter(c2 + 1), r2 + 1);
    open_keynote_doc(src)?;
    let out = run(MERGE_CELLS, &[dst_abs.to_string_lossy().to_string(), page.to_string(), idx.to_string(), range])?;
    if out != "ok" { return Err(format!("合并失败：{out}")); }
    Ok(())
}
