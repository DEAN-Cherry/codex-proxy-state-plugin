# State 观测插件

宿主最低版本为 codex-proxy-rs v3.18.0，不设人为上限，要求以 `plugin.json` 的 `engines` 为准；
构建使用的 SDK 提交以 `backend/Cargo.toml` 中 `gateway-plugin-sdk` 的 `rev` 为准（固定至 v3.18.2 提交 `e30aad475560b94db2d999e2251d180e45d52671`）。
契约规范遵循清单格式版本 2（`manifestVersion: 2`）、进程通信协议版本 2（`package.protocolVersion: 2`）、洋葱中间件版本 3（`middleware.version: 3`，声明挂载 `request` 与 `attempt` 阶段）以及统一事件观察能力（`observer`，替代原 `request_lifecycle` 与 `web_socket_observer`）。
宿主运行采用完整信任模型（`trustedProcess`），无权限清单隔离，插件与宿主拥有相同系统身份，可访问数据、凭据、配置与网络；清单与握手均移除 `permissions` 字段。
私有状态保持 `settings` 与 `observations` 命名空间的 `schemaVersion: 1`，插件 ID 维持 `dean-cherry.state-observer`；插件不直接修改宿主数据库结构（私有状态通过宿主 `host.state.*` 持久化），已有私有状态记录（含历史 SSE 观测）持续兼容可读。
独立工程不依赖 fork 业务代码，不主动请求模型、不刷新 State、不改写请求、不参与调度。
响应正文保持宿主未读句柄，不使用流映射或 `inspect_frames`。
WebSocket State 通过 `observer` 旁路事件回调采集，HTTP/SSE 仅采集中间件可见响应头，不读取正文。

## 用户能力

- 按账号和实际发送模型记录业务响应中的 State
- 每账号选择观察模型，空列表表示观察所有模型
- 长度规则支持逗号分隔的正整数及闭区间，例如 `200,300,400-500`，空字符串表示不限长度
- 保存最近 State 观测摘要、次数、长度分布、最近历史与本地过期时间
- 独立管理页默认聚合全部账号的 State 概况，支持账号、模型和状态筛选，再下钻详情与账号配置
- 账号显示标签（邮箱优先、名称其次、ID 兜底）为主显示，ID 用于区分同名账号和内部关联，外层账号组、内层模型表，按账号整组分页
- State 原文不返回浏览器、日志或诊断，摘要不表示模型能力或服务端有效期
- 不可确定账号或实际发送模型的事件不归入任何账号

## 管理 API

通过 `window.codexProxyPlugin.request` 访问相对路由，JSON 响应不使用宿主信封。

### POST api/overview

请求 `{"cursor":null,"limit":20}`。`cursor` 可省略或为上一页返回的账号 ID，
`limit` 为 1～50，缺省为 20。按宿主账号 ID 升序分页，不遗漏停用或尚无观测的账号。

```json
{
  "accounts":[{
    "accountId":"acct_example",
    "accountName":"account@example.test",
    "enabled":true,
    "observationEnabled":true,
    "error":null,
    "models":[{
      "model":"gpt-6-astra",
      "observations":2,
      "matched":1,
      "mismatched":1,
      "lastObservedAtMs":1790000000000,
      "expiresAtMs":1790086400000,
      "latestLength":312,
      "latestFingerprint":"a1b2c3d4e5f6",
      "source":"http_header",
      "validation":"mismatch",
      "expired":false
    }]
  }],
  "nextCursor":null,
  "diagnostics":{"unattributed":0,"dropped":0,"storageFailures":0},
  "nowMs":1790000000000
}
```

单个账号的私有状态读取失败时，`error` 为 `unavailable`、`observationEnabled` 为 `null`、
`models` 为空，不能将它显示成“未观测”或“停用观测”。宿主账号目录失败则整个请求失败。
摘要不包含长度分布或历史，点击账号/模型后通过 `api/account` 读取详情。
加载全部账号时依次请求后续页，界面持续显示加载进度，不把已加载部分称为全局总计。
页面按账号整组分页，名称或 ID 筛选选择账号，模型和状态筛选保留匹配模型所属的账号层级。
筛选覆盖所有已加载账号，完成全部加载后才能表示全部账号。
聚合查询不持有观测写锁，不阻塞业务观测写入等待队列。
`accountName` 与 `api/accounts` 使用下述同一显示标签规则，并非宿主原始 `name` 字段。

### GET api/accounts

```json
{"accounts":[{"accountId":"acct_example","accountName":"account@example.test","enabled":true}],"nextCursor":null}
```

账号来自 `host.data.accounts.list`（`account_facts`）非凭据运行投影，不依赖也不声明已废除的 `accounts` 权限。
两接口的 `accountName` 均由后端统一派生：优先取去除首尾空白后的非空 `email`，其次取去除首尾空白后的非空 `name`，最后回退到未经修改的账号 ID；邮箱为 `null`、空串或仅含空白均视为缺失。不按 OAuth 等名称字符串判断账号类型。
`api/accounts` 的账号字段仍仅限于 ID、显示名称和启用状态；邮箱可通过 `accountName` 返回并在浏览器显示，但不新增独立 `email`、备注、Provider 或凭据字段，不调用原始凭据读取、宿主管理端或账号写入接口。
SDK 的账号事实不提供认证类型或备注，因此不承诺完全复刻宿主 UI 的备注规则。相同显示标签不会合并账号；分页、分组、去重、存储键和所有操作始终使用 ID，宿主账号资料和既有持久化记录不变。
支持 `?cursor=...` 分页，每页最多 200 个 OpenAI 账号。

### POST api/account

请求 `{"accountId":"acct_example"}`。

```json
{
  "accountId":"acct_example",
  "version":null,
  "settings":{"enabled":true,"models":[],"lengthRules":"","retentionHours":24},
  "models":[{
    "model":"gpt-6-astra",
    "observations":2,
    "matched":1,
    "mismatched":1,
    "lastObservedAtMs":1790000000000,
    "expiresAtMs":1790086400000,
    "latestLength":312,
    "latestFingerprint":"a1b2c3d4e5f6",
    "source":"http_header",
    "validation":"mismatch",
    "expired":false,
    "lengths":[{"length":292,"count":1},{"length":312,"count":1}],
    "history":[{"observedAtMs":1790000000000,"length":312,"fingerprint":"a1b2c3d4e5f6","source":"http_header","validation":"mismatch"}]
  }],
  "diagnostics":{"unattributed":0,"dropped":0,"storageFailures":0},
  "nowMs":1790000000000
}
```

`validation` 为 `matched`、`mismatch` 或 `unrestricted`。
`source` 为 `http_header`、`sse_metadata` 或 `websocket_metadata`。
匹配计数仅统计配置了长度规则的匹配观测，无规则观测单独由总数减去两类计数得到。
历史和次数均为保留窗口内的有界样本，页面必须说明统计口径，不宣称全量审计。
无记录账号返回默认配置及空模型列表，不造示例数据。
`version` 是账号配置的乐观并发版本，不因正常观测写入而导致配置编辑冲突。

### POST api/settings

请求 `{"accountId":"acct_example","expectedVersion":null,"settings":{"enabled":true,"models":[],"lengthRules":"200,300,400-500","retentionHours":24}}`。
成功返回与 `api/account` 相同结构，版本冲突返回 409。
`retentionHours` 为 1 到 168 的整数，代表本地观测保留时间，不代表上游 State 有效期。

错误响应为 `{"error":{"code":"invalid_request","message":"可展示的简短说明"}}`。

## 页面要求

默认页面提供账号概况和分组模型表格，自动分批加载全部账号。
支持账号名称或 ID、模型和匹配/过期/未观测/读取失败筛选，区分停用账号与停用观测。
打开账号详情后，点击模型查看长度分布和最近历史，并可编辑启用观测、模型列表、长度规则和保留时间。
保留并发编辑冲突处理，详情或配置错误不能隐藏已加载的全局概况。
支持宿主明暗主题、375/768/1280 宽度、键盘操作、加载/空/错误状态。
生产构建不包含模拟记录或自动安装预览宿主。
