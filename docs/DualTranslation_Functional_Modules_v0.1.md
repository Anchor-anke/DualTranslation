# DualTranslation 功能模块文档 v0.1

| 文档属性 | 内容 |
|---|---|
| 产品 | DualTranslation（二元编译） |
| 版本 | v0.1 |
| 依据 | `DualTranslation_PRD_v0.1.md` |
| 实施范围 | macOS + Windows 桌面 MVP |
| 文档状态 | 开发基线 |

## 1. 文档目的

本文档把 PRD 转化为开发可执行的功能模块、模块边界、数据契约和验收标准。它不改变产品的核心定位：应用只负责在人类与 Coding Agent 之间进行双向转换，不读取代码仓库、不自动操作 Agent、不执行命令。

### 1.1 已确认的实施决策

原 PRD 的首期描述以 macOS 为主；实施版本调整为 macOS 与 Windows 同步支持。桌面层采用 Tauri 2 + React + TypeScript + Vite：React 负责界面和状态，Rust 负责窗口、系统能力、模型请求、存储与安全边界。

- 模型服务：用户自填 OpenAI 兼容接口（Base URL、API Key、模型名）。
- API Key：写入 macOS Keychain 或 Windows Credential Manager，不写入 SQLite。
- 本地数据：SQLite；默认保留最近 20 条历史。
- 敏感信息：本地检测，默认提供遮盖后继续；命中记录默认不保存。
- 默认语言：解释模式中文；写给 Agent 模式自动路由，用户手动选择优先。
- 统计：仅记录本地匿名计数，不上传原文、结果或事件。
- 首期输入：仅支持纯文本；读取剪贴板必须由用户点击触发。

## 2. 总体架构

```text
┌──────────────────────────────────────────────────────────────┐
│ React/TypeScript UI                                           │
│ 主窗口 · 双模式编辑 · 结果区 · 历史 · 设置 · 提示弹窗         │
└───────────────────────┬──────────────────────────────────────┘
                        │ Tauri IPC / JSON Schema
┌───────────────────────▼──────────────────────────────────────┐
│ Rust 应用服务层                                                │
│ 窗口与快捷键 · 剪贴板 · 敏感扫描 · 模型客户端 · 渲染器         │
│ 历史 Repository · 设置 · 本地指标 · 错误映射                  │
└───────────────┬─────────────────────┬────────────────────────┘
                │                     │
       ┌────────▼────────┐  ┌────────▼─────────┐
       │ SQLite           │  │ OS Credential    │
       │ 历史/版本/设置/指标│  │ Keychain/WinCred │
       └─────────────────┘  └──────────────────┘
                │
       ┌────────▼────────┐
       │ 用户配置的模型端点│
       │ OpenAI-compatible │
       └─────────────────┘
```

### 2.1 调用原则

1. 前端不得直接持有 API Key；所有模型请求经过 Rust 服务层。
2. 剪贴板只有 `read_clipboard_text` 明确调用时才读取。
3. 模型响应必须先解析、再通过 Schema 校验，校验失败不能作为成功结果保存。
4. Agent 适配器只能改变表达与交付格式，不得改变 Canonical Task Spec 的目标、范围和用户约束。
5. 原始敏感值不进入日志、指标和默认历史。

## 3. 功能模块清单

| 模块 | 名称 | 对应 PRD | 优先级 |
|---|---|---|---:|
| FM-01 | 桌面应用外壳 | FR-01、FR-02 | P0 |
| FM-02 | 输入与剪贴板 | FR-03 | P0 |
| FM-03 | 敏感信息检测与遮盖 | FR-11 | P0 |
| FM-04 | 模型服务配置与客户端 | FR-04、FR-07 | P0 |
| FM-05 | 写给 Agent | FR-04、FR-05、FR-12 | P0/P1 |
| FM-06 | 看懂 Agent | FR-04、FR-06 | P0 |
| FM-07 | 语言路由与 Agent 适配 | FR-07、FR-08 | P0 |
| FM-08 | 结果、复制与迭代 | FR-09、FR-12 | P0/P1 |
| FM-09 | 本地历史 | FR-10 | P0 |
| FM-10 | 设置与本地指标 | FR-01、FR-02、FR-10 | P0 |

## 4. 模块详细说明

### FM-01 桌面应用外壳

**职责**

- 在 macOS 显示菜单栏入口，在 Windows 显示系统托盘入口。
- 创建单一悬浮窗口，支持置顶、拖动、调整大小和上次位置恢复。
- 默认快捷键为 `CommandOrControl+Shift+Space`，设置页可重新录入。
- 应用窗口关闭时隐藏应用，不退出常驻进程；托盘菜单提供打开、设置和退出。
- 启动时校验已保存位置是否仍在当前显示器范围内，失效时回到主显示器中央。

**交互规则**

- 快捷键触发只显示窗口，不读取剪贴板，不自动粘贴，不改变前台应用内容。
- 窗口重新打开时聚焦输入区，用户可以继续手动输入。
- 快捷键冲突时显示失败原因，保留托盘入口并允许重新录入。

**异常处理**

- 快捷键注册失败：`SHORTCUT_CONFLICT`。
- 窗口位置失效：自动修正并记录本地诊断日志。
- 第二个实例启动：激活已有实例并退出新实例。

### FM-02 输入与剪贴板

**职责**

- 提供多行输入框、字符数提示、清空和粘贴入口。
- “读取剪贴板”按钮调用系统文本剪贴板；读取后原文仍可编辑。
- “一键复制”只写入纯文本结果并显示成功反馈。

**限制**

- 不监听剪贴板变化。
- 不读取 Cursor、Codex、编辑器、终端或其他应用窗口。
- 图片、文件、富文本等不支持类型显示“不支持的内容类型”，不得静默转换。
- 单次输入超过配置上限时阻止转换，并提示用户分段处理；MVP 默认上限为 100,000 个 Unicode 字符。

### FM-03 敏感信息检测与遮盖

**检测类型**

- 常见 API Key 和 Access Token。
- `Bearer`、Basic Auth 和 Authorization Header。
- PEM 私钥块。
- `password=...`、`secret=...` 等高风险键值对。
- 邮箱、电话和身份证等高风险个人信息模式（低置信度命中只提醒）。

**流程**

```text
用户点击开始转换
        ↓
本地扫描原文
        ↓
无命中 ───────────────→ 进入模型请求
        ↓
展示命中项与预览
        ├─ 遮盖后继续：替换为 <REDACTED:TYPE_N>
        ├─ 取消：返回编辑状态
        └─ 原文继续：二次确认，默认不保存历史
```

遮盖映射只保存在当前内存会话，转换完成后清除。系统不保证正则能识别所有敏感信息，界面必须明确提示用户自行检查。

### FM-04 模型服务配置与客户端

**配置字段**

| 字段 | 必填 | 说明 |
|---|---:|---|
| `name` | 是 | 用户自定义配置名称 |
| `baseUrl` | 是 | OpenAI 兼容服务根地址 |
| `model` | 是 | 模型标识 |
| `apiKey` | 是 | 写入系统凭据库的密钥，不返回前端 |
| `timeoutMs` | 否 | 默认 30,000 ms |

**请求规则**

- 使用 Chat Completions 兼容协议，默认请求 `stream: false`。
- Base URL 必须经过规范化，避免重复拼接 `/v1` 或 `/chat/completions`。
- 默认要求 HTTPS；仅 `localhost`、`127.0.0.1` 和 `::1` 允许 HTTP。
- 模型响应按“提取 JSON → Schema 校验 → 一次修复请求 → 失败”处理。
- 不把 API Key、Authorization Header 和完整输入写入日志。

**错误映射**

| 错误 | 用户提示 | 可执行动作 |
|---|---|---|
| `401/403` | API Key 无效或无权限 | 检查密钥并重新保存 |
| `404` | 地址或模型不存在 | 检查 Base URL、路径和模型名 |
| `429` | 服务限流 | 稍后重试或调整供应商配额 |
| 超时 | 模型响应超时 | 重试、缩短输入或调高超时 |
| 网络失败 | 无法连接服务 | 检查网络、代理和地址 |
| Schema 失败 | 模型返回格式不完整 | 执行一次修复后重试 |

### FM-05 写给 Agent

**输入**：用户的自然语言想法、目标 Agent、语言偏好、快速/协商模式及澄清答案。

**输出**：Canonical Task Spec、渲染后的 Prompt、语言决策、关键假设和未确认项。

**处理规则**

1. 识别目标、对象、范围、动机、约束、动作类型和验收线索。
2. 按影响程度排序歧义：业务行为、架构、安全/数据、范围、样式。
3. 高影响缺失项进入协商模式；每轮最多 3 个问题，并说明影响。
4. 低风险缺失项写入 `assumptions`，不得伪装成用户事实。
5. 可由 Agent 检查的信息写入 `inspect_before_action`，DualTranslation 不读取仓库。
6. 本地模板根据 `targetAgent` 渲染最终 Prompt。

### FM-06 看懂 Agent

**输入**：用户手动输入或主动读取的 Agent 回复。

**输出**：通俗解释、状态、行动、验证、风险、决策和原始技术细节。

**状态判断**

- `completed`：回复明确说明完成，且有对应验证证据。
- `partial`：仅完成部分任务或仍有未完成项。
- `failed`：明确失败或验证失败。
- `unclear`：无法从原文确认结果，不允许猜测。

原始命令、路径、错误、警告和代码片段必须作为可展开字段保留。解释层可以改变语言和组织方式，但不得改变事实。

### FM-07 语言路由与 Agent 适配

**语言优先级**

```text
用户手动选择 > 模式默认值 > 本地规则建议 + 模型判断
```

支持 `zh`、`en`、`bilingual` 和仅用于请求阶段的 `auto`。解释模式首次默认 `zh`；写给 Agent 模式首次使用 `auto`。

语言决策必须返回：最终语言、选择理由和是否由用户覆盖。目标 Agent 只负责表达适配：是否先规划、验证命令要求、交付报告格式，不得改变原始目标。

### FM-08 结果、复制与迭代

结果页至少包含：

- 可直接复制的纯文本结果。
- 假设与未确认项。
- 验收标准和验证要求。
- 语言策略、目标 Agent 与选择理由。
- 可展开的命令、路径、错误和代码片段。

结果正文允许用户直接编辑。自然语言调整会创建新版本，不覆盖旧版本；版本记录包含版本号、调整指令、结构化结果、渲染文本和变更字段列表。

### FM-09 本地历史

默认只保存非敏感记录。每条记录包含模式、原文、结构化结果、渲染结果、目标 Agent、输出语言、创建时间和版本列表。

支持：查看最近记录、再次复制、打开继续调整、删除单条、清空全部、修改保留数量。敏感记录只有在用户明确确认保存后才入库。

### FM-10 设置与本地指标

设置页提供快捷键、模型配置、默认 Agent、默认语言、历史上限和本地统计清空。

指标只保存事件类型和计数，不保存文本。至少记录生成、复制、轻微编辑复制、重大编辑复制、转换失败、敏感命中、取消和历史再次复制。

## 5. 数据契约

Schema 文件建议放在 `schemas/`：

- `conversion-request.schema.json`
- `conversion-response.schema.json`
- `canonical-task-spec.schema.json`
- `explanation-spec.schema.json`
- `provider-profile.schema.json`

### 5.1 ConversionRequest

```json
{
  "schemaVersion": 1,
  "mode": "write | explain",
  "input": "string",
  "targetAgent": "generic | cursor | codex",
  "languagePreference": "auto | zh | en | bilingual",
  "generationMode": "quick | negotiated",
  "clarificationAnswers": [],
  "providerProfileId": "string"
}
```

### 5.2 ConversionResponse

```json
{
  "schemaVersion": 1,
  "kind": "clarification_required | write_completed | explain_completed | failed",
  "requestId": "string",
  "data": {}
}
```

`clarification_required` 的问题数量不得超过 3。`write_completed` 必须包含 Canonical Task Spec、语言决策和渲染 Prompt；`explain_completed` 必须包含 Explanation Spec 和通俗解释文本；`failed` 必须包含稳定错误码和用户可见提示。

### 5.3 SQLite 表

| 表 | 关键字段 | 约束 |
|---|---|---|
| `provider_profiles` | `id`, `name`, `base_url`, `model`, `timeout_ms` | 不存 API Key |
| `conversions` | `id`, `mode`, `original_input`, `sensitive`, `created_at` | 按创建时间索引 |
| `conversion_versions` | `id`, `conversion_id`, `version_no`, `structured_json`, `rendered_text`, `adjustment_text` | 版本号唯一 |
| `settings` | `key`, `value_json` | 非敏感偏好 |
| `local_metric_events` | `event_name`, `count`, `updated_at` | 不含用户文本 |

## 6. Tauri IPC 接口

以下接口为前端与 Rust 之间的最小边界；接口返回值均为可序列化 DTO，并使用 Schema 校验。

| 命令 | 作用 |
|---|---|
| `show_main_window` / `hide_main_window` | 显示或隐藏悬浮窗 |
| `register_global_shortcut` / `unregister_global_shortcut` | 注册、注销并检测快捷键 |
| `read_clipboard_text` | 用户主动读取纯文本剪贴板 |
| `write_clipboard_text` | 写入纯文本结果 |
| `scan_sensitive_text` | 本地检测敏感信息并返回命中摘要 |
| `save_provider_profile` / `test_provider_profile` | 保存配置或测试连接 |
| `convert` | 执行写给 Agent 或看懂 Agent 转换 |
| `adjust_conversion` | 基于上一版本生成新版本 |
| `list_history` / `get_history` / `delete_history` / `clear_history` | 历史查询与清理 |
| `get_settings` / `update_settings` | 读取或修改设置 |
| `get_local_metrics` / `clear_local_metrics` | 查看或清空本地指标 |

## 7. 验收标准

1. macOS 与 Windows 均能通过托盘和可配置快捷键打开/隐藏悬浮窗。
2. 未点击读取按钮前，应用不访问剪贴板；不读取其他应用或仓库。
3. 敏感信息在发送前可被识别、遮盖或取消，API Key 不出现在数据库和日志。
4. 写给 Agent 能输出目标、范围、约束、假设和验收标准，且不把推断冒充用户要求。
5. 看懂 Agent 能区分完成、部分完成、失败和无法判断，并保留关键技术事实。
6. Codex、Cursor、通用 Agent 和中文/英文/双语选项可切换。
7. 结果可编辑、复制、再次调整，并保留版本记录。
8. 历史可查看、再次复制、单条删除和全部清空，默认最多 20 条。
9. 401、404、429、超时、网络失败和 Schema 失败均有明确提示。
10. MVP 不执行命令、不自动发送、不自动操作 Agent、不读取仓库、不提供云端同步。

## 8. 非目标

- Linux 客户端。
- 自动读取或操作 Cursor/Codex。
- 代码仓库、终端历史、图片或富文本上下文。
- 多 Agent 调度、团队协作、云同步和复杂 A/B 测试。
- 代码执行、命令执行、自动粘贴和自动发布。
