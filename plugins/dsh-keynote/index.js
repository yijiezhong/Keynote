/**
 * dsh-keynote —— 把本仓库的 `kn` 命令行暴露成模型可调用的 `keynote_kn` 工具。
 *
 * 纯 Host 插件：注册一个工具，执行 `python3 <projectRoot>/python/keynote_iwa.py <args...>`。
 * kn 本身有 56 条命令（读结构 / 改文字 / 改表格 / 加页面 / 从零创建 …），插件只做一层薄封装：
 * 参数按数组原样传给 CLI，不经过 shell，因此文件名里的空格、引号都安全。
 *
 * 设计取舍：
 * - 只暴露一个通用工具，而不是 56 个工具：命令语法稳定、帮助文本可由 `["--help"]` 取到，
 *   一个工具不会污染工具列表；description 里列了最常用的十条。
 * - 写入类命令与只读命令分开标记并发安全，避免两个写操作同时改同一个文件。
 */
import { defineTool } from '@deepseek-ai/dsh-tools'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import os from 'node:os'
import path from 'node:path'

const execFileAsync = promisify(execFile)

export const name = 'dsh-keynote'

/** 宿主必须提供工具注册表。 */
export const inject = ['tools']

/** 只读命令：可以并行跑，不产生新文件。 */
const READ_ONLY = new Set(['list', 'verify', 'info'])


const DESCRIPTION = [
  '在 Apple Keynote `.key` 文件上执行 kn（Rust）命令；当前是阶段 1，只读。',
  '参数 `args` 是完整命令行数组，例如 `["list","deck.key"]`、`["verify","deck.key"]`。',
  '已实现：list（zip 条目）、verify / info（IWA / 段 / 对象 / slides 统计）。',
  '写回与编辑命令（outline、set-text、new、add-slide 等）正在从 Python 迁移到 Rust，暂不可用。',
  '取帮助用 `["help"]`。',
].join(' ')

export function apply(ctx, config = {}) {
  config = config || {}
  const projectRoot = config.projectRoot
    || process.env.KN_PROJECT_ROOT
    || path.join(os.homedir(), 'PARA', '8.Code', 'Keynote')
  const timeoutMs = Number.isFinite(Number(config.timeoutMs)) && Number(config.timeoutMs) > 0
    ? Number(config.timeoutMs)
    : 180_000
  const maxOutputChars = Number.isFinite(Number(config.maxOutputChars)) && Number(config.maxOutputChars) > 0
    ? Number(config.maxOutputChars)
    : 40_000

  const toolPath = path.join(projectRoot, 'bin', 'kn')

  function clip(text) {
    if (text.length <= maxOutputChars) return text
    return `${text.slice(0, maxOutputChars)}\n…（输出已截断，共 ${text.length} 字符）`
  }

  function compose(stdout, stderr) {
    const out = (stdout || '').trimEnd()
    const err = (stderr || '').trimEnd()
    if (!err) return out
    if (!out) return err
    return `${out}\n\n[stderr]\n${err}`
  }

  const tool = defineTool({
    name: 'keynote_kn',
    description: DESCRIPTION,
    parameters: {
      args: {
        type: 'array',
        items: { type: 'string' },
        required: true,
        description: '完整的 kn 参数数组，第一个元素是子命令，例如 ["verify","deck.key"]；取帮助用 ["help"]。',
      },
      cwd: {
        type: 'string',
        description: '命令的工作目录；默认项目根目录（相对路径的文件名按它解析）。',
      },
    },
    output: {
      schema: {
        type: 'object',
        additionalProperties: false,
        properties: {
          command: { type: 'string', required: true },
          exitCode: { type: 'integer', required: true },
          output: { type: 'string', required: true },
        },
      },
      render: (_args, value) => [
        { type: 'text', text: `$ kn ${value.command}\n${value.output}` },
      ],
    },
    timeoutMs,
    isConcurrencySafe: (args) => READ_ONLY.has(Array.isArray(args?.args) ? args.args[0] : ''),
    async execute(args) {
      const argv = Array.isArray(args?.args) ? args.args.map(String) : []
      if (argv.length === 0) {
        return { command: '', exitCode: 2, output: 'args 不能为空；取帮助用 ["--help"]。' }
      }
      const cwd = args?.cwd ? path.resolve(String(args.cwd)) : projectRoot
      const printable = argv.map((a) => (a.includes(' ') ? JSON.stringify(a) : a)).join(' ')
      try {
        const { stdout, stderr } = await execFileAsync(toolPath, argv, {
          cwd,
          timeout: timeoutMs,
          maxBuffer: 32 * 1024 * 1024,
        })
        return { command: printable, exitCode: 0, output: clip(compose(stdout, stderr)) || '(无输出)' }
      } catch (error) {
        const timedOut = error?.killed || error?.signal === 'SIGTERM' || error?.code === 'ETIMEDOUT'
        const code = timedOut ? 124 : (Number.isInteger(error?.code) ? error.code : 1)
        const body = compose(error?.stdout, error?.stderr)
          || (timedOut ? `命令超时（${timeoutMs} ms）` : String(error?.message ?? error))
        return { command: printable, exitCode: code, output: clip(body) }
      }
    },
  })

  ctx.effect(() => ctx.tools.register(tool), 'dsh-keynote: keynote_kn tool')
}

