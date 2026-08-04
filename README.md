<p align="center">
  <img src="assets/app-icon.svg" width="112" alt="DualTranslation 应用图标" />
</p>

<h1 align="center">DualTranslation · 二元编译</h1>

<p align="center">
  把模糊想法整理成 Coding Agent 能执行的任务，也把 Agent 的技术回复解释成人能快速判断的结果。
</p>

<p align="center">
  <a href="https://github.com/Anchor-anke/DualTranslation/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/Anchor-anke/DualTranslation/actions/workflows/ci.yml/badge.svg" /></a>
  <a href="https://github.com/Anchor-anke/DualTranslation/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/Anchor-anke/DualTranslation?display_name=tag" /></a>
  <img alt="Tauri 2" src="https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white" />
  <img alt="React 19" src="https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=black" />
</p>

## DualTranslation 是什么？

DualTranslation 是一个面向 macOS 和 Windows 的轻量桌面工具，位于用户与 Coding Agent 之间，负责双向语义转换：

- **写给 Agent**：将一句模糊需求整理为目标、背景、范围、约束、假设、未知项、验收标准和验证要求。
- **看懂 Agent**：将 Agent 的技术回复解释为完成状态、实际动作、验证证据、风险、待决策事项和建议下一步。

它不是代码编辑器，也不会替你执行项目命令。当你明确选择本地项目文件夹后，它可以安全整理相关上下文，并在发送给模型前让你预览确切片段，减少因项目背景缺失造成的误解。

## 主要功能

- **智能模式与直接生成**
  - 智能模式是默认策略，会在缺少关键用户决定时提出 1–3 个高信息量问题。
  - 直接生成只会使用低风险、可逆的假设，并将其明确标注。
- **多项目上下文**：项目文件夹只需添加一次，可快速切换或组合最多 3 个项目，并根据当前任务自动筛选清单、文档、源码和测试。
- **上下文预览与遮盖**：发送前可以取消选择文件并查看确切代码片段；敏感信息和本机绝对路径不会进入模型请求。
- **面向不同 Agent 的表达**：支持 Codex、Cursor 和通用 Coding Agent。
- **输出语言控制**：支持自动选择、中文、英文和双语输出。
- **事实保真的回复解释**：不会把未验证、失败或不清楚的工作描述成“已完成”。
- **自然语言调整**：生成后可继续提出修改要求，同时保留原任务中的事实和约束。
- **本地历史与版本**：保存转换结果和后续调整版本，便于回看与复制。
- **敏感信息检测**：发送前识别 API Key、Token、密码、私钥等高风险内容并要求确认。
- **系统托盘与快捷键**：适合在编写需求或阅读 Agent 回复时随时调用。
- **OpenAI Chat Completions 兼容**：可连接支持该协议的模型服务；对官方 DeepSeek 接口提供专门的速度优化。

## 下载与安装

请前往 [Releases](https://github.com/Anchor-anke/DualTranslation/releases/latest) 下载最新版本。

| 平台                | 下载文件                               | 说明                                                 |
| ------------------- | -------------------------------------- | ---------------------------------------------------- |
| Windows 10/11 x64   | `.msi` 或 `-setup.exe`                 | 双击安装；系统需要 Microsoft Edge WebView2 Runtime。 |
| macOS Apple Silicon | 文件名包含 `aarch64` 的 `.dmg`         | 适用于 M1、M2、M3、M4 及后续 Apple 芯片。            |
| macOS Intel         | 文件名包含 `x64` 或 `x86_64` 的 `.dmg` | 适用于 Intel Mac。                                   |

### 关于首版签名

当前自动构建版本尚未配置 Apple Developer ID、公证和 Windows 商业代码签名证书。因此 macOS Gatekeeper 或 Windows SmartScreen 可能显示“未知开发者”警告。

- macOS：优先在 Finder 中按住 Control 点击应用，选择“打开”，再确认一次。
- Windows：确认安装包来自本仓库的 Release 页面后，可在 SmartScreen 中选择“更多信息”→“仍要运行”。

如果系统仍阻止运行，请不要从第三方下载站获取副本。后续版本可在配置正式签名证书后消除大部分警告。

## 首次使用

1. 启动 DualTranslation，进入顶部的“设置”。
2. 新增一个模型服务配置。
3. 填写模型信息并保存。
4. 回到“转换”；如需项目上下文，点击“添加项目”选择本地项目文件夹。
5. 输入文本，选择目标 Agent、输出语言与生成策略，然后开始转换。
6. 在上下文预览中确认相关项目文件和片段，再发送给模型。

### 模型配置字段

| 字段         | 如何填写                                                          |
| ------------ | ----------------------------------------------------------------- |
| 配置名称     | 仅用于本地识别，例如“DeepSeek 主力模型”。                         |
| 模型名       | 服务商提供的模型标识，例如 `deepseek-v4-flash`。                  |
| Base URL     | OpenAI 兼容 API 的基础地址，例如 `https://api.deepseek.com/v1`。  |
| API Key      | 从模型服务商后台创建的密钥；只保存到操作系统凭据库。              |
| 超时（毫秒） | 默认可使用 `30000`；推理模型或复杂任务可提高到 `60000`–`120000`。 |

远程服务必须使用 HTTPS。本机开发服务只有在地址为 `localhost`、`127.0.0.1` 或 `::1` 时才能使用 HTTP。

## API Key 与隐私

DualTranslation 将凭据与普通设置分开处理：

- API Key 写入 **macOS Keychain** 或 **Windows Credential Manager**，不会写入项目文件或前端环境变量。
- 本地 SQLite 数据库只保存凭据引用，不保存 API Key 明文。
- 前端只能通过受控的 Tauri 命令保存和使用凭据，无法读取已保存密钥并显示出来。
- 剪贴板只在用户点击“读取剪贴板”时读取，不会在后台监听。
- 应用只会在你明确选择文件夹后读取项目；不会执行命令、修改源码或自动操作 Coding Agent。
- `.env`、密钥、证书、凭据文件、`.git`、`node_modules`、`target`、`dist`、二进制文件和大型文件默认不参与项目扫描。
- 本地项目库只保存路径、技术栈、文件数量和变更指纹，不持久化保存源码内容。
- 只有相对路径和你确认过的片段会发送给配置的模型服务，本机绝对路径不会发送。
- 历史记录默认保存在本机；检测到敏感内容时会在发送或保存前要求确认。
- 模型请求会直接发送到你配置的服务商，请同时阅读该服务商的数据处理与隐私政策。

> 不要把真实 API Key 写进 `.env`、源码、Issue、日志或截图。仓库中的 `.env.example` 只包含非敏感示例配置。

## 工作原理

```text
用户输入
   │
   ├─ 敏感信息扫描与用户确认
   ├─ 已选项目 ──> 本地安全扫描 ──> 相关上下文预览
   │
   ├─ 写给 Agent ──> 智能澄清 ──> 规范化任务 JSON ──> Codex / Cursor / 通用提示词
   │
   └─ 看懂 Agent ──> 事实保真解释 JSON ──> 易读说明
                         │
                         └─ 本地历史与调整版本（可选）
```

模型输出必须通过项目内置的 JSON Schema 校验。若模型第一次返回的格式不符合契约，应用只会尝试一次格式修复，避免无限重试和不可控消耗。

## 技术栈与目录

- 桌面框架：[Tauri 2](https://v2.tauri.app/)
- 前端：React 19、TypeScript、Vite
- 桌面后端：Rust、Reqwest、SQLite、Keyring
- 契约验证：JSON Schema、AJV、jsonschema
- 测试：Vitest、Rust unit tests、GitHub Actions

```text
DualTranslation/
├── src/                    # React 界面、功能模块与 IPC 封装
├── src-tauri/              # Rust 后端、模型调用、凭据、历史与平台能力
├── schemas/                # 请求、响应、任务与解释的 JSON Schema
├── tests/                  # 前端测试、契约测试与测试夹具
├── docs/                   # 产品、模块、计划和实施状态文档
├── assets/                 # 应用图标等静态资源
└── .github/workflows/      # CI 与跨平台 Release 构建流程
```

## 本地开发

### 环境要求

- Node.js 22
- npm 10 或更高版本
- Rust stable
- Tauri 2 对应平台的系统依赖
  - macOS：Xcode Command Line Tools
  - Windows：Microsoft C++ Build Tools 与 WebView2

详细依赖请参考 [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)。

### 启动桌面开发版

```bash
git clone https://github.com/Anchor-anke/DualTranslation.git
cd DualTranslation
npm ci
npm run tauri dev
```

### 常用命令

```bash
# 前端开发服务器
npm run dev

# 启动 Tauri 桌面开发版
npm run tauri dev

# 前端测试
npm run test

# 前端构建、类型与代码检查
npm run build
npm run typecheck
npm run lint

# Rust 格式、静态检查与测试
npm run rust:fmt
npm run rust:clippy
npm run rust:test

# 完整质量检查
npm run check
```

模型测试使用本机回环地址上的 Mock Server，不会调用真实模型服务，也不需要 API Key。

## 构建安装包

在当前操作系统构建本地安装包：

```bash
npm ci
npm run tauri build
```

跨平台 Release 由 [release.yml](.github/workflows/release.yml) 完成。推送符合 `v*.*.*` 的版本标签后，GitHub Actions 会在对应系统上构建 Windows x64、macOS Apple Silicon 和 macOS Intel 安装包，并上传到同一个 GitHub Release。

## 故障排查

### 显示“模型响应超时”

- 对简单且信息完整的任务可使用“直接生成”。对于官方 DeepSeek 接口，该模式会关闭模型默认思考过程。
- 确认 Base URL、模型名和网络代理配置正确。
- 将超时从 `30000` 调高到 `60000` 或更高后重试。
- 输入很长时先缩短内容，或选择响应速度更快的模型。

### 显示“API Key 无效或没有模型权限”

- 检查密钥是否仍有效、是否有余额或模型权限。
- 重新保存配置；已保存的 API Key 不会回显，这是预期的安全行为。

### 显示“模型服务返回了空内容”或“推理资源不足”

- 应用会自动重试临时的 HTTP 5xx 和模型推理资源不足，最多尝试 3 次。
- 如果重试后仍失败，通常是服务商当前负载较高，请稍后再试或临时切换模型。
- 内容安全过滤和输出长度截断会显示独立提示，方便针对性调整输入。

### 界面可以打开，但无法连接本机模型

- 使用 `http://127.0.0.1:<端口>/v1` 或 `http://localhost:<端口>/v1`。
- 确认服务兼容 OpenAI Chat Completions，并提供 `/chat/completions` 接口。

## 项目文档

- [产品需求文档](DualTranslation_PRD_v0.1.md)
- [功能模块说明](docs/DualTranslation_Functional_Modules_v0.1.md)
- [开发实施计划](docs/DualTranslation_Development_Plan_v0.1.md)
- [当前实施状态](docs/DualTranslation_Implementation_Status_v0.1.md)
- [数据契约](schemas/)

## 参与贡献

欢迎通过 Issue 提交问题、复现步骤和功能建议。涉及安全或隐私问题时，请不要在公开内容中附带真实 API Key、Token、数据库或包含敏感信息的截图。
