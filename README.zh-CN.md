# Process Console

Process Console 是一个 macOS 桌面工具，用来发现并清理 coding agent 启动后遗留的本地 server 进程。

[English README](./README.md)

## 背景

现在很多 coding agent 和 AI 编辑器会在开发项目时启动 dev server、MCP server、测试 watcher 或其他辅助进程。问题是，当 agent session 关闭、重启、崩溃，或者忘记自己启动过哪些进程时，这些进程可能仍然留在后台运行，并继续占用本地端口。

Process Console 的目标是把这些信息可视化，让你快速回答：

- 当前有哪些本地端口正在监听？
- 这些端口大概率属于哪个项目？
- 这些进程是否可能由 Cursor、Claude Code、opencode、Codex 或 OpenClaw 启动？
- 是否可以从 GUI 中安全地结束对应的进程组？

## 功能

- macOS 进程与监听端口扫描。
- 按项目目录分组展示进程。
- Agent tab 只展示疑似由 coding agent 启动的进程。
- 其他 tab 收纳 Unknown 进程，避免 WPS、WeChat、浏览器等无关进程干扰主界面。
- 内置 Cursor、Claude Code、opencode、Codex、OpenClaw 的归因规则。
- 每条归因结果都显示置信度和判断证据。
- 命令行参数默认遮罩 token、secret、password、api key 等敏感信息。
- 支持带确认的进程组终止：优先发送 `SIGTERM`，必要时再 force kill。
- 支持手动刷新和 5 秒自动刷新。

## 归因模型

V1 不安装常驻后台 daemon，也不保存历史快照。应用打开时会扫描当前系统状态。

因此，agent 归因不是绝对结论，而是基于置信度的判断。Process Console 会综合进程父子关系、进程组、命令行参数、已知应用路径、项目工作目录等信号。如果原始 agent 已经退出，归因可能只能达到部分可信。

## 本地开发

环境要求：

- macOS
- Node.js 22+
- pnpm
- Rust stable

安装依赖：

```bash
pnpm install
```

启动开发模式：

```bash
pnpm dev
```

运行检查：

```bash
pnpm test
pnpm exec tsc --noEmit
pnpm vite build
```

构建 app：

```bash
pnpm build
```

构建后的 macOS app bundle 位于：

```text
src-tauri/target/release/bundle/macos/Process Console.app
```

本地构建 DMG：

```bash
pnpm tauri build --bundles dmg
```

## 发布

项目配置了 GitHub Actions，会自动构建 macOS DMG 并发布到 GitHub Releases。

推送版本 tag 即可触发 release：

```bash
git tag v0.1.0
git push origin v0.1.0
```

也可以在 GitHub Actions 页面手动触发 release workflow。

## License

MIT

