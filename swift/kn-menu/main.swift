// kn-menu —— Keynote 菜单栏工具（Swift 壳，调用 kn CLI）
//
// 实测结论（macOS 26）：
//   NSStatusItem 一旦被隐藏（isVisible=false），在同一进程里无法恢复显示；
//   在 applicationDidFinishLaunching 之后重建也不显示。
// 因此「Keynote 出现/消失 → 图标出现/消失」用「状态变化时进程重启」实现：
//   - launch 时按当前 Keynote 状态决定是否创建图标；
//   - 运行中检测到状态变化 → exit(1)；
//   - LaunchAgent 配 KeepAlive={SuccessfulExit:false}，非零退出会被立即重启。
// 用户点「退出」走 NSApp.terminate（exit 0），不会被拉起。

import AppKit

let KN_BIN = ProcessInfo.processInfo.environment["KN_BIN"]
    ?? FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent("PARA/8.Code/Keynote/bin/kn").path
let KEYNOTE_BUNDLE = "com.apple.Keynote"

class KnDelegate: NSObject, NSApplicationDelegate {
    var item: NSStatusItem?
    var timer: Timer?
    var launchedWithKeynote = false

    func applicationDidFinishLaunching(_ note: Notification) {
        launchedWithKeynote = keynoteRunning()
        // 延迟 0.5s 创建：launchd 快速重启后立刻建项会落到屏幕外
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
            if self.launchedWithKeynote {
                self.createItem()
            }
        }
        // 关键：Timer 不能在 applicationDidFinishLaunching 里同步创建，
        // 否则 NSStatusItem 会被布局到屏幕外（实测 position y=937，屏幕高 932）
        DispatchQueue.main.async {
            self.timer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { _ in
                self.watch()
            }
        }
        log("started, keynote=\(launchedWithKeynote)")
    }

    // MARK: Keynote 状态

    func keynoteRunning() -> Bool {
        !NSRunningApplication.runningApplications(withBundleIdentifier: KEYNOTE_BUNDLE).isEmpty
    }

    func watch() {
        let running = keynoteRunning()
        if running != launchedWithKeynote {
            launchedWithKeynote = running
            log("keynote -> \(running)")
        }
        // 实测：isVisible / length / title 三者一旦变过都无法恢复，
        // 所以状态项常驻，只用「菜单项是否可点 + 标题后缀」表达 Keynote 状态
        item?.button?.title = running ? "kn" : ""
        for mi in item?.menu?.items ?? [] where !mi.isSeparatorItem && mi.action != #selector(quit) && mi.action != #selector(cmdHelp) {
            mi.isEnabled = running
        }
    }

    // MARK: 菜单栏

    func createItem() {
        let it = NSStatusBar.system.statusItem(withLength: 80)
        it.button?.title = "kn"
        it.menu = buildMenu()
        item = it
    }

    func icon() -> NSImage {
        NSImage(size: NSSize(width: 18, height: 18), flipped: false) { rect in
            NSColor.systemRed.setFill()
            NSBezierPath(ovalIn: rect.insetBy(dx: 4, dy: 4)).fill()
            return true
        }
    }

    func buildMenu() -> NSMenu {
        let menu = NSMenu()
        add(menu, "大纲 outline", #selector(cmdOutline))
        add(menu, "文字 text", #selector(cmdText))
        add(menu, "表格 tables", #selector(cmdTables))
        add(menu, "链接 links", #selector(cmdLinks))
        add(menu, "颜色 colors", #selector(cmdColors))
        add(menu, "样式 styles", #selector(cmdStyles))
        add(menu, "图表 charts", #selector(cmdCharts))
        add(menu, "图片 images", #selector(cmdImages))
        add(menu, "校验 verify", #selector(cmdVerify))
        menu.addItem(NSMenuItem.separator())
        add(menu, "新增一页（Keynote 路径）…", #selector(cmdAddSlide))
        add(menu, "删除一页…", #selector(cmdDeleteSlide))
        add(menu, "复制一页…", #selector(cmdDupSlide))
        menu.addItem(NSMenuItem.separator())
        add(menu, "设置第 1 页标题…", #selector(cmdSetTitle))
        add(menu, "追加第 1 页备注…", #selector(cmdAppendNote))
        add(menu, "跳过某页…", #selector(cmdSkip))
        menu.addItem(NSMenuItem.separator())
        add(menu, "全部命令帮助", #selector(cmdHelp))
        add(menu, "退出", #selector(quit))
        return menu
    }

    func add(_ menu: NSMenu, _ title: String, _ action: Selector) {
        let mi = NSMenuItem(title: title, action: action, keyEquivalent: "")
        mi.target = self
        menu.addItem(mi)
    }

    // MARK: 命令

    @objc func cmdOutline() { runKn(["outline"]) }
    @objc func cmdText() { runKn(["text"]) }
    @objc func cmdVerify() { runKn(["verify"]) }

    @objc func cmdAddSlide() {
        guard let path = keynotePath() else {
            showResult("kn", "没有打开的 Keynote 文档（或尚未保存）")
            return
        }
        let out = path.replacingOccurrences(of: ".key", with: "-new-page.key")
        runProcess(KN_BIN, ["add-slide", path, out, "标题项目符号"]) { text in
            self.showResult("kn add-slide", text)
        }
    }

    @objc func cmdTables() { runKn(["tables"]) }
    @objc func cmdLinks() { runKn(["links"]) }
    @objc func cmdColors() { runKn(["colors"]) }
    @objc func cmdStyles() { runKn(["styles"]) }
    @objc func cmdCharts() { runKn(["charts"]) }
    @objc func cmdImages() { runKn(["images"]) }

    @objc func cmdDeleteSlide() {
        guard let path = keynotePath(), let page = prompt("删除一页", "页码（从 1 开始）") else { return }
        let out = outputPath(path, "-del.key")
        runProcess(KN_BIN, ["del-slide", path, out, page]) { self.showResult("kn del-slide", $0) }
    }

    @objc func cmdDupSlide() {
        guard let path = keynotePath(), let page = prompt("复制一页", "页码（从 1 开始）") else { return }
        let out = outputPath(path, "-dup.key")
        runProcess(KN_BIN, ["dup-slide", path, out, page]) { self.showResult("kn dup-slide", $0) }
    }

    @objc func cmdSetTitle() {
        guard let path = keynotePath(), let title = prompt("设置第 1 页标题", "新标题") else { return }
        let out = outputPath(path, "-title.key")
        runProcess(KN_BIN, ["set-text", path, out, "1", "title", title]) { self.showResult("kn set-text", $0) }
    }

    @objc func cmdAppendNote() {
        guard let path = keynotePath(), let text = prompt("追加第 1 页备注", "要追加的文字") else { return }
        let out = outputPath(path, "-note.key")
        runProcess(KN_BIN, ["append-note", path, out, "1", text]) { self.showResult("kn append-note", $0) }
    }

    @objc func cmdSkip() {
        guard let path = keynotePath(), let page = prompt("跳过一页", "页码（从 1 开始）") else { return }
        let out = outputPath(path, "-skip.key")
        runProcess(KN_BIN, ["skip", path, out, page]) { self.showResult("kn skip", $0) }
    }

    @objc func cmdHelp() {
        runProcess(KN_BIN, ["help"]) { self.showResult("kn help", $0) }
    }

    func prompt(_ title: String, _ message: String) -> String? {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        alert.addButton(withTitle: "确定")
        alert.addButton(withTitle: "取消")
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 320, height: 24))
        alert.accessoryView = field
        alert.window.initialFirstResponder = field
        guard alert.runModal() == .alertFirstButtonReturn else { return nil }
        let value = field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        return value.isEmpty ? nil : value
    }

    func outputPath(_ path: String, _ suffix: String) -> String {
        if path.hasSuffix(".key") {
            return String(path.dropLast(4)) + suffix
        }
        return path + suffix
    }

    func runKn(_ args: [String]) {
        guard let path = keynotePath() else {
            showResult("kn", "没有打开的 Keynote 文档（或尚未保存）")
            return
        }
        runProcess(KN_BIN, args + [path]) { text in
            self.showResult("kn " + args.joined(separator: " "), text)
        }
    }

    func keynotePath() -> String? {
        let src = "tell application \"Keynote\" to return POSIX path of (file of document 1 as text)"
        guard let script = NSAppleScript(source: src) else { return nil }
        var err: NSDictionary?
        let out = script.executeAndReturnError(&err)
        if err != nil { return nil }
        return out.stringValue
    }

    func runProcess(_ bin: String, _ args: [String], completion: @escaping (String) -> Void) {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: bin)
        p.arguments = args
        let pipe = Pipe()
        p.standardOutput = pipe
        p.standardError = pipe
        p.terminationHandler = { _ in
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            let text = String(data: data, encoding: .utf8) ?? ""
            DispatchQueue.main.async { completion(text) }
        }
        do { try p.run() } catch {
            showResult("kn", "无法启动 kn：\(error.localizedDescription)")
        }
    }

    func showResult(_ title: String, _ text: String) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = String(text.prefix(3000))
        alert.alertStyle = .informational
        alert.runModal()
    }

    @objc func quit() { NSApplication.shared.terminate(nil) }

    func log(_ s: String) {
        FileHandle.standardError.write("[kn-menu] \(s)\n".data(using: .utf8)!)
    }
}

let app = NSApplication.shared
let delegate = KnDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory)
app.run()
