# v0.2.0

## 契约与兼容性

- 适配上游 codex-proxy-rs v3.18.x 破坏性契约断代，`engines.codex-proxy-rs` 设为 `>=3.18.0`（不设人为上限）
- 清单格式升级至 `manifestVersion: 2`，进程通信协议升级至 `package.protocolVersion: 2`
- 洋葱中间件升级至 `middleware.version: 3`，显式声明挂载 `request` 与 `attempt` 阶段
- 移除已废除的 `request_lifecycle` 与 `web_socket_observer` 能力，合并迁移至统一 `observer` 能力（`observer.observe` 接收 `Event::RequestCompleted` 与 `Event::WebSocketResponse`）
- 遵循宿主 3.18 完整信任模型（`trustedProcess`），无权限清单隔离，与宿主同身份运行；清单与进程握手均彻底移除 `permissions` 声明与检查
- 账号名称及状态获取从 `host.auth.list` 迁移至 `host.data.accounts.list`（`account_facts`）非凭据运行投影；浏览器管理端仍保持仅接收 ID、名称和启用状态（不向前端暴露邮箱）
- SDK 依赖固定至上游 v3.18.2 正式发布提交（`e30aad475560b94db2d999e2251d180e45d52671`）

## 数据与升级注意事项

- 宿主 3.15.2～3.17.x 继续使用 v0.1.3 插件包，并在安装时明确指定标签；宿主 `>=3.18.0` 使用 v0.2.0 兼容包
- 被停用的不兼容实例保留配置与私有数据，请勿卸载；安装兼容版本后核对绑定并重新启用
- 插件身份保持 `dean-cherry.state-observer`，私有状态 `settings` 与 `observations` 的 `schemaVersion: 1` 不变，插件不直接修改宿主数据库结构（私有状态通过宿主 `host.state.*` 持久化），包括历史 SSE 观测在内的全部已有记录持续兼容可读
- 管理 API 结构保持不变（`POST api/overview`、`GET api/accounts`、`POST api/account`、`POST api/settings`），前端 UI 无额外破坏性调整

## 验证

- Rust 单元与集成测试、前端测试通过（包含新增回归测试）
- 本地代码格式化（Rustfmt）、全目标严格 Lint（Clippy）、本机构建与前端 Lint、构建通过
- 基于 Bun 的真实进程 QA 通过（协议 2 握手、管理路由与观测调用）
- 使用匹配的新版官方 `cpr-plugin` 工具成功完成本地打包，验证清单 2、协议 2、中间件 3 结构及包内资源摘要绑定（macOS aarch64 验证产物，非 Linux 发布包）
- 发布工作流将重新在云端执行检查与测试，构建 Linux x86_64 GNU（glibc 2.28 基线）二进制，再进行协议 2 进程冒烟、官方打包与附件回读校验；云端结果以对应 Actions 运行记录为准
- 未在 Linux 正式宿主、PostgreSQL/Redis 环境或真实上游流量下完成最终验收
