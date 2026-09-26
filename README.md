# State 观测插件

独立的 Codex Proxy RS 插件，按 **账号 + 实际发送的上游模型** 记录业务响应中的 State 摘要。
不主动刷新、不重写 State、不改变选号或计费，不需要修改宿主数据库。

## 兼容与安装

- 目标宿主：`>=3.15.2, <3.16.0`，SDK 固定到上游提交 `589e1bc999a8b110201b7fdcbf32d2e75bf08253`
- 插件身份：`dean-cherry.state-observer`
- 安装包目标：Linux x86_64 GNU，构建时最低 glibc 2.28
- 通过宿主「插件管理 → 安装 → GitHub」填写 `DEAN-Cherry/codex-proxy-state-plugin`，稳定版标签可留空，也可填写 `v0.1.0`
- 也可从 [Releases](https://github.com/DEAN-Cherry/codex-proxy-state-plugin/releases) 下载 `.tar.gz`，再通过「上传包」安装，不要使用 GitHub 自动生成的 Source code 归档
- 核对权限并安装、启用后，打开插件页面「State 观测」
- 不支持当前没有插件运行时的 fork 3.13.3，不能直接在该版本安装

从 fork 切换到上游之前，必须单独处理两边 `0017`、`0018` 数据库迁移冲突。
本插件不执行宿主升级或数据库迁移，不要直接替换生产镜像后复用 fork 数据库。

## 使用

1. 保留默认请求绑定，或在宿主高级设置中缩小 Key、Provider、模型范围
2. 通过正常客户端发送业务请求，不需要额外发起测试或刷票请求
3. 在插件页面选择账号，刷新查看各模型最近 State 长度、指纹、规则匹配情况与历史
4. 配置观察模型、长度规则和本地保留时间，保存后对后续采集生效

账号下拉框显示宿主账号 ID，不申请读取邮箱、账号名或原始凭据的权限。
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
| SSE 元数据 | 在对应 attempt 的公开事件中读取顶层 `headers` 或 `response.headers` |
| JSON 元数据帧 | 与对应 attempt 的实际账号、模型关联，不使用响应中的 `model` 字段反推实际发送模型 |

Header 可能来自复用的上游 WS 连接，不能据此认定本次产生了新票据。
上游 3.15.2 会过滤部分 WS `response.metadata`，插件不能看到被过滤的内容。
流观察只处理不超过 256 KiB 的完整事件，单条 State 最多 16 KiB。
没有 State、事件被过滤或观测丢失，只表示未观测到，不表示账号异常或模型降档。

请求重试时，流式记录保留各 attempt 的独立归属；最终 Header 只使用终态账号和实际上游模型。
终态缺少账号或模型时不猜测归属。待关联请求最多 1024 个，每请求最多 64 条流式摘要，关联窗口为 15 分钟。
超时、容量淘汰、进程重启或宿主终态队列丢弃可能造成观测缺失，页面诊断计数仅覆盖本进程已知的丢弃情况。

插件只申请 `requests` 与 `data` 两个权限，不申请 `accounts`、`network` 或 `models`。
配置与摘要使用宿主 `host.state.*`，写入采用版本 CAS，配置版本与业务观测版本分开。
观测写入最多等待 100 毫秒，失败计入诊断，不将存储错误返回业务请求。
中间件仍属于请求链，插件进程退出或 RPC/流错误可能影响请求；不承诺与宿主旁路观察相同的故障隔离。

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
