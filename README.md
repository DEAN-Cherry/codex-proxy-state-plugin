# State 观测插件

独立的 Codex Proxy RS 插件，按 **账号 + 实际发送的上游模型** 记录业务响应中的 State 摘要。
不主动发起请求、不刷新或重写 State、不改变选号或计费，不执行宿主数据库迁移。

## 兼容与安装

- 插件身份：`dean-cherry.state-observer`，安装包目标为 Linux x86_64 GNU（glibc 2.28+）
- v0.2.0 要求宿主 `>=3.18.0`，使用上游 v3.18.2 SDK（技术细节见 [CONTRACT.md](CONTRACT.md) 与 [backend/Cargo.toml](backend/Cargo.toml)）
- 宿主 3.15.2～3.17.x 请指定标签 `v0.1.3`；宿主 `>=3.18.0` 使用 `v0.2.0` 或后续兼容版本
- 被停用的不兼容实例保留配置与私有数据，请勿卸载；安装兼容版本后核对绑定并重新启用
- 在宿主「插件管理 → 安装 → GitHub」填写 `DEAN-Cherry/codex-proxy-state-plugin`，标签留空即查询最新稳定版；旧宿主不要留空
- 或从 [Releases](https://github.com/DEAN-Cherry/codex-proxy-state-plugin/releases) 下载 `.tar.gz` 后「上传包」安装，不要使用 GitHub 自动生成的 Source code 归档
- 安装并启用后，打开插件页面「State 观测」

## 使用

1. 保留默认请求绑定，或在宿主实例设置中缩小 Key、Provider、模型范围
2. 正常发送业务请求即可，不需要额外的测试或刷票请求
3. 页面按账号分组聚合各模型的最近 State，可按账号名称或 ID、模型、观测状态筛选
4. 在账号详情中配置观察模型（留空表示全部）和长度规则，例如 `292,312,400-500`（区间含端点，留空不限）

停用、尚无观测和读取失败的账号都会保留在概况中，读取失败不等同于未观测。
全部分页加载完成前，页面只显示「已加载」范围，不称为全局总计。

## 数据与限制

- **State 原文不保存、不返回浏览器、不写日志**，只保留字节长度和 SHA-256 前 12 位十六进制指纹；
  指纹仅用于观察变化，不用于鉴权、票据恢复或证明模型身份
- 每账号最多保留最近 16 个模型、每模型 12 条摘要；次数和分布只统计本地保留窗口内的样本，
  **不是全量请求次数或完整审计**
- 本地保留默认 24 小时（可设 1～168 小时），**与上游票据有效期无关**
- 采集来源：中间件可见的 `x-codex-turn-state` 响应头，以及宿主 WebSocket 旁路事件中的 State；
  仅存在于 HTTP/SSE 正文中的 State 不采集，插件不读取或重新包装业务正文
- 无法确定账号或实际上游模型的观测不归入任何账号；超时、容量淘汰、进程重启或宿主丢弃都可能造成缺失，
  「未观测到」不表示账号异常或模型降档

用户能力与管理 API 合同见 [CONTRACT.md](CONTRACT.md)，页面设计见 [DESIGN.md](DESIGN.md)。

## 信任模型与故障隔离

上游 3.18.x 无权限清单隔离，采用完整信任模型（trustedProcess），与宿主拥有相同系统身份，可访问数据、凭据、配置与网络，请只从可信来源安装。
账号名称由 `host.data.accounts.list`（`account_facts`）非凭据运行投影获取；向浏览器返回的接口仍仅包含账号 ID、名称和启用状态，不暴露邮箱等字段。

配置与摘要存放在宿主 `host.state.*`（保持 schemaVersion 1，插件 ID 仍为 `dean-cherry.state-observer`），插件不直接修改宿主数据库结构，已有私有状态及历史记录（含历史 SSE 来源）持续可读。写入最多等待 100 毫秒，存储失败只计入诊断，不返回给业务请求。
中间件仍位于请求链中，插件进程退出或 RPC 错误可能影响请求，`delegate`（交给后续处理）仅为流转策略，不具备真正的进程故障隔离。
建议将实例 `request`、`attempt` 绑定的「插件失败时」设为「交给后续处理」（`delegate`）；
宿主安装器默认的 `reject` 不由插件清单控制，本插件也不会修改宿主实例配置。
本插件没有配置项或认证映射，宿主可能隐藏其设置入口；此时可按
[官方实例 API](https://github.com/zyycn/codex-proxy-rs/blob/v3.18.2/docs/api.md#121-运行实例) 修改绑定。

## 开发

稳定版标签由 GitHub Actions 在云端检查、构建、打包并验证附件后发布，无需本地构建。
开发命令与发布流程见 [docs/development.md](docs/development.md)。

## 许可

源码按 [Apache-2.0](LICENSE) 发布，项目归属为个人账号 [DEAN-Cherry](https://github.com/DEAN-Cherry)，
上游参考与归属说明见 [NOTICE](NOTICE)。
