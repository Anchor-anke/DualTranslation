# DualTranslation 开发实施计划 v0.1

| 文档属性 | 内容 |
|---|---|
| 产品 | DualTranslation（二元编译） |
| 依据 | `DualTranslation_PRD_v0.1.md`、`DualTranslation_Functional_Modules_v0.1.md` |
| 目标平台 | macOS + Windows |
| 发布目标 | MVP 封闭测试版 |
| 文档状态 | 开发执行基线 |

## 1. 实施目标

在不读取代码仓库、不自动操作 Coding Agent、不执行命令的前提下，交付一个可在 macOS 与 Windows 使用的桌面 MVP：用户通过托盘或全局快捷键打开悬浮窗，手动输入或主动读取文本，完成“写给 Agent”或“看懂 Agent”转换，检查结果后复制、保存或继续调整。

## 2. 技术方案

### 2.1 技术栈

| 层 | 技术 | 用途 |
|---|---|---|
| 桌面壳 | Tauri 2 | 窗口、托盘、快捷键、IPC、系统能力 |
| 前端 | React + TypeScript + Vite | 页面、交互、状态和结果展示 |
| Rust 层 | Rust + serde + reqwest | 模型客户端、系统操作、数据访问和安全边界 |
| 数据库 | SQLite + Tauri SQL 插件 | 历史、版本、设置和本地指标 |
| 凭据 | Rust `keyring` | macOS Keychain、Windows Credential Manager |
| Schema | JSON Schema + Rust/TypeScript 校验 | 前端、Rust 和模型结果的数据契约 |
| 测试 | Vitest、React Testing Library、Rust test、Mock Server | 单元、组件、集成和契约测试 |
| 发布 | Tauri bundler + GitHub Actions | macOS DMG、Windows 安装包和 CI |

依赖版本在工程初始化时选择当时稳定版本，并提交 lockfile；不要在文档中硬编码可能快速变化的次版本号。

### 2.2 目录结构

```text
DualTranslation/
├── docs/
│   ├── DualTranslation_Functional_Modules_v0.1.md
│   └── DualTranslation_Development_Plan_v0.1.md
├── schemas/
│   ├── canonical-task-spec.schema.json
│   ├── conversion-request.schema.json
│   ├── conversion-response.schema.json
│   ├── explanation-spec.schema.json
│   └── provider-profile.schema.json
├── src/
│   ├── app/
│   ├── components/
│   ├── features/
│   │   ├── conversion/
│   │   ├── history/
│   │   ├── settings/
│   │   └── privacy/
│   ├── lib/
│   ├── state/
│   └── types/
├── src-tauri/
│   ├── migrations/
│   ├── src/
│   │   ├── commands/
│   │   ├── conversion/
│   │   ├── providers/
│   │   ├── privacy/
│   │   ├── storage/
│   │   └── platform/
│   └── capabilities/
├── tests/
│   ├── fixtures/
│   ├── integration/
│   └── e2e/
└── .github/workflows/
```

### 2.3 分层边界

- React 不直接调用模型端点，不读取 API Key，不访问 SQLite 文件。
- Tauri Commands 是前端唯一的系统能力入口。
- `conversion` 层只处理结构化转换和本地渲染，不负责窗口或数据库。
- `providers` 层负责 OpenAI 兼容协议、超时、错误映射和重试。
- `privacy` 层在请求前完成敏感扫描和遮盖。
- `storage` 层负责迁移、Repository、历史上限和指标清理。
- `platform` 层隔离 macOS/Windows 的窗口、快捷键和托盘差异。

### 2.4 MVP 非目标

以下内容不进入本次开发、测试或发布验收：

- Linux 客户端、移动端和 Web 端。
- 代码仓库、终端历史、编辑器上下文和后台应用监控。
- 自动粘贴、自动发送、Agent 操作、代码或命令执行。
- 图片/富文本输入、云同步、团队协作、多 Agent 调度和复杂 A/B 测试。
- 自动更新服务；更新机制作为后续版本单独评估。

## 3. 开发阶段与执行任务

### 阶段 0：工程初始化与契约冻结

**任务**

- 初始化 Tauri 2 + React + TypeScript + Vite。
- 配置 TypeScript 严格模式、ESLint、格式化、Rust `fmt`、Clippy 和基础 CI。
- 建立 JSON Schema、示例数据和 Schema 校验命令。
- 建立统一错误码、日志脱敏规则和环境配置约定。

**完成标准**

- `npm run lint`、`npm run typecheck`、`cargo fmt --check`、`cargo clippy` 和测试命令可运行。
- 空应用可以在本地启动，并能生成 macOS/Windows 开发构建。
- 所有核心 Schema 至少有一个成功样例和一个失败样例。

### 阶段 1：桌面外壳与窗口行为

**任务**

- 创建 macOS 菜单栏和 Windows 系统托盘菜单。
- 实现单实例、显示/隐藏、退出和窗口聚焦。
- 注册默认快捷键 `CommandOrControl+Shift+Space`，实现录入、注销和冲突提示。
- 保存窗口坐标、尺寸、置顶状态；启动时校验显示器范围。
- 关闭窗口时隐藏应用而不是终止常驻进程。

**完成标准**

- 两个平台都能从托盘和快捷键打开窗口。
- 快捷键冲突不会导致应用崩溃，并能回退到托盘入口。
- 拔除外接显示器后重启，窗口会回到可见屏幕。

### 阶段 2：本地数据和凭据安全

**任务**

- 添加 SQLite migration 和 Repository 层。
- 建立 `provider_profiles`、`conversions`、`conversion_versions`、`settings`、`local_metric_events` 表。
- 接入系统凭据库保存 API Key；数据库只保存凭据引用和非敏感配置。
- 实现默认 20 条历史、可配置上限、最旧记录清理、删除和全部清空。

**完成标准**

- 数据库迁移可重复执行，升级不会破坏已有记录。
- API Key 不出现在 SQLite、前端持久化状态、普通日志或错误堆栈。
- 清理历史后对应版本数据和索引均被删除。

### 阶段 3：模型配置与 OpenAI 兼容客户端

**任务**

- 实现模型配置页面：名称、Base URL、模型名、API Key、超时时间。
- 在 Rust 中规范化 Base URL，默认调用 Chat Completions。
- 默认启用 HTTPS；仅本机地址允许 HTTP。
- 实现连接测试、取消请求、30 秒默认超时和稳定错误码。
- 实现响应 JSON 提取、Schema 校验及一次修复请求。

**Mock Server 场景**

- 合法 JSON 结果。
- Markdown 代码块包裹的 JSON。
- 401/403 鉴权失败。
- 404 地址或模型不存在。
- 429 限流。
- 超时、断网和服务端 5xx。
- 字段缺失、类型错误和非法枚举。

**完成标准**

- 配置测试成功时展示延迟和模型名，不展示密钥。
- 所有错误能映射为用户可理解的提示和下一步动作。
- Schema 失败最多修复一次，仍失败则返回明确失败状态，不保存结果。

### 阶段 4：输入、剪贴板和隐私链路

**任务**

- 实现多行输入、清空、字符计数和输入上限。
- 实现用户点击触发的文本剪贴板读取。
- 实现纯文本复制和即时成功反馈。
- 实现 API Key、Token、私钥、密码字段和个人信息的本地扫描。
- 实现遮盖预览、取消、遮盖后继续和原文二次确认。
- 建立“敏感记录默认不保存”的保存策略。

**完成标准**

- 应用空闲、后台运行和打开窗口时均不自动读取剪贴板。
- 输入命中敏感信息时，在模型请求前一定出现确认流程。
- 遮盖映射仅保存在内存，转换完成后被清除。
- 敏感值不进入日志、指标或默认历史。

### 阶段 5：双向转换核心

**任务顺序**

1. 定义 Prompt 模板和 JSON 输出约束。
2. 实现“写给 Agent”的意图提取、歧义排序、问题生成和 Canonical Task Spec。
3. 实现快速模式、协商模式及最多 3 个澄清问题。
4. 实现 Codex、Cursor、通用 Agent 的本地渲染器。
5. 实现“看懂 Agent”的状态、动作、验证、风险、下一步和技术细节提取。
6. 实现中文、英文、双语路由，以及用户手动选择覆盖逻辑。

**核心约束**

- 用户输入与系统假设必须分字段保存。
- Agent 适配器不能修改目标、范围和约束。
- `unclear` 状态不能被渲染成“已完成”。
- 任何模型结果都必须通过 Schema 校验。

**完成标准**

- 使用 PRD 登录示例、UI 调整示例和 Agent 技术回复示例完成端到端转换。
- 三种目标 Agent 的结果只在表达格式上有差异。
- 关键假设和验收标准在 UI 中可见且可复制。

### 阶段 6：结果、历史和迭代 UI

**任务**

- 实现双模式主界面、结果区、假设区、技术详情折叠区和加载状态。
- 支持编辑结果后一键复制。
- 实现自然语言调整，创建新版本并展示变更字段。
- 实现历史列表、再次复制、继续调整、单条删除和全部清空。
- 实现设置页和本地指标查看/清空。

**完成标准**

- 从快捷键唤起到复制结果的主路径可连续完成。
- 调整失败不会覆盖上一版本。
- 历史记录能恢复结构化数据、渲染文本和版本列表。
- 复制成功、转换失败、取消和重试均有明确即时反馈。

### 阶段 7：质量、跨平台验收与发布

**任务**

- 完成前端单元、Rust 单元、Schema 契约、模型 Mock 集成和桌面端 E2E 测试。
- 在 macOS 和 Windows 实机/虚拟机执行功能验收矩阵。
- 配置 GitHub Actions：PR 检查、双平台构建、标签发布。
- 生成 macOS DMG 和 Windows 安装包。
- 配置签名、公证和安装/卸载验证；没有证书时只能发布内部测试包。

**封闭测试发布门槛**

- P0 验收项全部通过。
- 无高危隐私问题：自动剪贴板读取、API Key 明文落盘、敏感内容误入历史均为阻断问题。
- 双平台核心路径通过率 100%。
- 关键错误有可理解提示，不能出现未处理崩溃。

## 4. 测试矩阵

### 4.1 自动化测试

| 层级 | 覆盖内容 |
|---|---|
| Schema | 必填字段、枚举、长度、版本和非法结构 |
| Rust 单元 | URL 规范化、错误映射、敏感扫描、遮盖、语言路由、版本差异 |
| 前端单元 | 模式切换、表单校验、加载/取消、结果编辑、复制反馈 |
| Repository 集成 | migration、历史上限、删除、清空、版本恢复 |
| 模型集成 | 正常结果、修复重试、401、404、429、超时、畸形 JSON |
| Tauri E2E | 托盘、快捷键、窗口显示、剪贴板、主流程和设置保存 |

### 4.2 跨平台人工验收

| 场景 | macOS | Windows |
|---|:---:|:---:|
| 托盘/菜单栏入口 | ✓ | ✓ |
| 全局快捷键及冲突 | ✓ | ✓ |
| 关闭隐藏、退出 | ✓ | ✓ |
| 多显示器位置恢复 | ✓ | ✓ |
| 文本剪贴板读取/复制 | ✓ | ✓ |
| 系统凭据存储 | Keychain | Credential Manager |
| 安装、卸载、再次启动 | ✓ | ✓ |

### 4.3 隐私专项测试

- 进程启动、快捷键触发和窗口打开不会发起网络请求。
- 未点击“读取剪贴板”时无法读取剪贴板内容。
- API Key 不在 SQLite、日志、前端 DevTools 状态或错误提示中出现。
- 敏感命中后取消操作不会发送原文。
- 敏感记录默认不进入历史，显式保存才进入历史。
- 日志脱敏后仍能定位错误类型和请求 ID。

## 5. CI/CD 与发布流程

### Pull Request

1. 安装依赖并检查 lockfile。
2. 执行前端 lint、typecheck、单元测试。
3. 执行 Rust fmt、Clippy、单元测试。
4. 执行 Schema 契约测试和 Mock Server 集成测试。
5. 在 macOS 与 Windows runner 执行 Tauri 编译 smoke test。

### Release Tag

1. 固定版本号并生成变更记录。
2. 构建 macOS DMG 和 Windows 安装包。
3. 执行安装、启动、升级/卸载和签名检查。
4. macOS 完成 Developer ID 签名与 notarization；Windows 完成代码签名。
5. 上传仅包含安装包和校验值的发布资产，不上传用户数据或 API Key。

## 6. 风险与处理

| 风险 | 处理方式 |
|---|---|
| OpenAI 兼容服务的 JSON 能力不同 | 不依赖单一供应商结构化输出；应用层 Schema 校验，最多一次修复重试 |
| 快捷键被系统或其他软件占用 | 注册失败提示、可配置快捷键、始终保留托盘入口 |
| 正则误报或漏报敏感信息 | 显示命中类型和预览，保留用户最终确认，不宣称完全检测 |
| 模型响应慢 | 立即显示进度状态，支持取消；默认 30 秒超时 |
| 不同平台窗口行为差异 | Rust `platform` 层隔离，并在两平台执行人工验收 |
| API Key 存储不可用 | 阻止保存并提示用户，禁止回退到明文文件 |
| Prompt 偏离用户意图 | 强制使用 Canonical Task Spec，分离用户要求/假设/建议并显示差异 |
| 解释遗漏失败或风险 | Explanation Schema 将状态、验证、风险和原始技术细节设为必备字段 |

## 7. 交付物清单

- 可运行的 Tauri 2 + React 工程。
- macOS 与 Windows 开发构建及封闭测试安装包。
- JSON Schema 与示例夹具。
- SQLite migration 和数据访问层。
- OpenAI 兼容模型客户端及 Mock Server 测试。
- 功能模块文档、开发计划和版本变更记录。
- 自动化测试报告与跨平台人工验收清单。

## 8. 完成定义

当以下条件全部满足时，v0.1 可进入封闭测试：

1. 双平台核心使用流程可完成：唤起 → 输入/主动读取 → 敏感检查 → 转换 → 检查 → 复制。
2. “写给 Agent”和“看懂 Agent”均通过 Schema 校验并保留必要事实。
3. 历史、版本、设置和删除策略有效，默认保留 20 条。
4. API Key 使用系统凭据库保存，不存在明文落盘。
5. PRD 中 FR-01～FR-12 的 MVP 项均有实现、测试和验收记录。
6. PRD 明确排除的仓库读取、Agent 自动操作、命令执行和云同步没有被引入。
