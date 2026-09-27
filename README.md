# State 观测插件

独立的 Codex Proxy RS 插件，按 **账号 + 实际发送的上游模型** 记录业务响应中的 State 摘要。
不主动刷新、不重写 State、不改变选号或计费，不需要修改宿主数据库。

## 兼容与安装

- 目标宿主：`>=3.15.2, <3.16.0`，SDK 固定到上游提交 `589e1bc999a8b110201b7fdcbf32d2e75bf08253`
- 插件身份：`dean-cherry.state-observer`
- 安装包目标：Linux x86_64 GNU，构建时最低 glibc 2.28
- 通过宿主「插件管理 → 安装 → GitHub」填写 `DEAN-Cherry/codex-proxy-state-plugin`，标签填写 `v0.1.2`，或留空查询最新稳定版
- 已发布的 `v0.1.0` 不包含本次响应流修复和聚合页面，不要将旧 Release 当作修复包
- 也可从 [Releases](https://github.com/DEAN-Cherry/codex-proxy-state-plugin/releases) 下载 `.tar.gz`，再通过「上传包」安装，不要使用 GitHub 自动生成的 Source code 归档
- 核对权限并安装、启用后，打开插件页面「State 观测」
- 不支持当前没有插件运行时的 fork 3.13.3，不能直接在该版本安装

从 fork 切换到上游之前，必须单独处理两边 `0017`、`0018` 数据库迁移冲突。
本插件不执行宿主升级或数据库迁移，不要直接替换生产镜像后复用 fork 数据库。

## 使用

1. 保留默认请求绑定，或在宿主高级设置中缩小 Key、Provider、模型范围
2. 通过正常客户端发送业务请求，不需要额外发起测试或刷票请求
3. 打开插件页面，默认分批加载全部账号，聚合查看账号与模型的最近 State 情况
4. 按账号名称或 ID、模型或观测状态筛选，每个账号一组，组内展示各模型的 State 数据

账号名称为主显示，ID 为次级信息，同名账号仍保持独立，缺少名称时回退到 ID。
账号组默认展开，可点击标题收起，分页以账号为单位，不把同一账号的模型拆到两页。
详情标题使用账号名称，配置保存和历史归属仍使用 ID，不因名称改变而丢失记录。
页面会标明已加载范围，全部分页完成前不将部分结果称为全局总计。
停用账号、尚无观测的账号和读取失败的账号均保留在概况中，读取失败不等同于未观测。
详情和配置按需读取，版本冲突时保留编辑草稿，不覆盖其他页面保存的设置。
模型列表为空表示观察所有模型；例如 `gpt-6-astra` 与 `gpt-6-sol` 是不同统计项。
长度规则支持 `292,312,400-500`，区间包含端点，留空表示不限制长度。
页面按当前长度规则重新解释历史摘要，不将过去的匹配结果冒充当前结果。

每账号最多保留最近 16 个有观测的模型，每模型最多 12 条摘要。
次数和长度分布仅计算这些摘要中仍在本地保留窗口内的样本，**不是全量请求次数或完整审计**。
超出模型容量时淘汰最久未观测的模型。记录过期后不计入统计，页面保留最近摘要并标记过期。
本地保留时间默认为 24 小时，可设为 1～168 小时，不代表上游票据有效期。

State 原文不保存、不返回浏览器或写日志，只保留原始字节长度与 SHA-256 的前 12 位十六进制指纹。
指纹仅供观察值是否变化，不用于鉴权、票据恢复或证明模型身份。

## 采集边界

| 来源 | 归属与限制 |
| --- | --- |
| 响应 Header | 记录中间件可见的 `x-codex-turn-state`，与同一请求终态的账号、实际发送模型关联 |
| WebSocket 旁路事件 | 使用宿主 `web_socket_observer`，从可见事件的 `headers` 或 `response.headers` 提取摘要，按请求和 attempt 匹配真实账号与模型 |
| 仅存在于 HTTP/SSE 正文的 State | 3.15.2 不提供安全的旁路接口，本版不采集，不读取或重新包装业务正文 |

Header 可能来自复用的上游 WS 连接，不能据此认定本次产生了新票据。
上游 3.15.2 会过滤部分 WS `response.metadata`，插件不能看到被过滤的内容。
旁路观察只处理不超过 256 KiB 的完整事件，单条 State 最多 16 KiB。
没有 State、事件被过滤或观测丢失，只表示未观测到，不表示账号异常或模型降档。

请求重试时，流式记录保留各 attempt 的独立归属；最终 Header 只使用终态账号和实际上游模型。
终态缺少账号或模型时不猜测 Header 归属，旁路事件缺少对应 attempt 或账号不一致时不写入账号记录。
待关联请求和 attempt 映射各最多 1024 项，关联窗口为 15 分钟。
超时、容量淘汰、进程重启或宿主终态队列丢弃可能造成观测缺失，页面诊断计数仅覆盖本进程已知的丢弃情况。

插件申请 `requests` 与 `accounts` 两个权限，不再申请 `data`，也不申请 `network` 或 `models`。
官方 3.15.2 的只读 `data` 目录不提供名称，因此 0.1.2 改用 `host.auth.list` 的非凭据运行投影。
`accounts` 授权域本身也包含读取原始凭据和修改账号的能力，但本插件不调用 `host.auth.get`、
`host.auth.get_runtime` 或 `host.auth.save`，只向页面投影 ID、名称与启用状态。
升级安装时需要重新核对并接受新包的访问域，不能把它描述为仅有只读账号权限。
配置与摘要使用宿主 `host.state.*`，写入采用版本 CAS，配置版本与业务观测版本分开。
观测写入最多等待 100 毫秒，失败计入诊断，不将存储错误返回业务请求。
中间件仍属于请求链，插件进程退出或 RPC 错误可能影响请求；不承诺与宿主旁路观察相同的故障隔离。

### 从 0.1.0 升级与失败策略

0.1.0 的 `inspect_frames` 把宿主正文句柄转换成插件流，官方 3.15.2 中没有 wire 正文的
Provider 控制事件会产生空 `RawBytes` 帧，SSE/JSON 正文回调会将它拒绝，随后流被关闭。
当前版本原样归还未读取的宿主正文句柄，改用旁路 WebSocket 观察，不通过丢弃控制帧或重试业务请求掩盖错误。

安装修复包并切换版本后，宿主会补充新声明的 `web_socket_observer` 观察绑定。
旧版本已设置的绑定范围和失败策略会保留，观察用途建议采用以下策略：

- 将 `request`、`attempt` 的「插件失败时」设为「交给后续处理」（`delegate`）
- 保留观察阶段的 `observe` 策略，并让各阶段的 Key、分组、Provider、模型范围保持一致

默认 `reject` 来自宿主安装器，不是插件清单可以覆盖的默认值，本插件不会擅自修改宿主实例配置。
官方 3.15.2 会隐藏没有普通参数或认证映射的插件设置入口，因此本插件可能看不到「高级设置」按钮。
这时可按[官方实例配置 API](https://github.com/zyycn/codex-proxy-rs/blob/v3.15.2/docs/api.md#121-运行实例)操作：
先通过 `GET /api/admin/plugins/instances` 取得当前实例，只修改其 `request`、`attempt` 绑定的
`failurePolicy` 为 `delegate`，保留其他绑定及所有范围。
向 `POST /api/admin/plugins/instances/update` 提交 `id` 和 `instance`，后者保留
`name`、`artifactSha256`、`enabled`、`configuration`、完整 `bindings`，并将当前 `revision`
作为 `expectedRevision`；省略 `secrets` 以保留已有值。不能直接回传列表中的只读字段，遇到 409 应重新读取。
`delegate` 只允许在调用 `next` 前安全委托，不会重放已经发送或开始交付的请求，因此它不能替代本次正文所有权修复。
原账号配置和私有记录 schema 保持兼容，旧的 SSE 摘要仍可查看。

## 构建

工具链：Rust 1.97、Node.js 24+、前端 `package.json` 指定的 pnpm。
依赖固定在 `backend/Cargo.lock`、`frontend/pnpm-lock.yaml`，不需要同级宿主源码。

```sh
pnpm --dir frontend install --frozen-lockfile
pnpm --dir frontend lint
pnpm --dir frontend build
cargo fmt --manifest-path backend/Cargo.toml -- --check
cargo clippy --manifest-path backend/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path backend/Cargo.toml --locked
cargo build --manifest-path backend/Cargo.toml --release --locked --target x86_64-unknown-linux-gnu
```

在 Windows 交叉构建 Linux 时，可使用 `cargo-zigbuild` 和 Zig：

```sh
rustup target add x86_64-unknown-linux-gnu --toolchain 1.97.0
cargo zigbuild --manifest-path backend/Cargo.toml --release --locked --target x86_64-unknown-linux-gnu.2.28
```

按 cargo-zigbuild 文档通过 `CARGO_ZIGBUILD_ZIG_PATH` 指向 Zig，不把 Windows `.exe` 冒充 Linux 二进制。

安装与 SDK 相同提交的官方打包器，然后打包：

```sh
cargo install --locked --git https://github.com/zyycn/codex-proxy-rs.git \
  --rev 589e1bc999a8b110201b7fdcbf32d2e75bf08253 codex-proxy-plugin-cli --root .tools
.tools/bin/cpr-plugin package --manifest plugin.json \
  --binary backend/target/x86_64-unknown-linux-gnu/release/codex-proxy-state-plugin \
  --target x86_64-unknown-linux-gnu --resource-map web=frontend/dist --output-dir dist
```

打包器生成 `.tar.gz` 和 `.sha256`，静态资源与二进制均通过摘要绑定。
没有授权时不自动上传、发布、安装或启用生产插件。

## 本地进程与页面验收

`scripts/qa-host.mjs` 只用于本地测试，不包含在插件包中。
它实际启动本机编译的插件二进制，通过协议握手、管理路由和观测调用生成页面数据。
宿主账号与 CAS 存储为可控测试替身，不连接 PostgreSQL、Redis 或真实上游。

```sh
cargo build --manifest-path backend/Cargo.toml --locked
pnpm --dir frontend build
bun scripts/qa-host.mjs
```

浏览器打开输出的 `http://127.0.0.1:4178`。
这验证生产页面与真实插件进程的连接，不等同于在 Linux 正式宿主中安装和真实模型流量验证。
管理 API 合同见 [CONTRACT.md](CONTRACT.md)，页面设计依据见 [DESIGN.md](DESIGN.md)。

## 发布与许可

源码按 [Apache-2.0](LICENSE) 发布，项目归属为个人账号
[DEAN-Cherry](https://github.com/DEAN-Cherry)，上游参考与归属说明见 [NOTICE](NOTICE)。

发布前通过本地检查与 `main` 分支 CI，将版本标签指向通过检查的源码提交。
使用与 SDK 相同提交的官方 CLI 打包，将 `.tar.gz` 和 `.sha256` 上传到对应 GitHub Release。
源码、清单版本与附件应一致，不覆盖已经发布的同版本内容。
稳定 Release 不标记 Draft 或 Pre-release，以便宿主留空标签查询最新稳定版。

当前验证覆盖自动测试、Windows 实际插件进程和生产前端联调。
完整 Linux 宿主安装、PostgreSQL/Redis 集成和真实上游流量仍需在目标环境验收，
不能将发布成功等同于已在生产环境生效。
