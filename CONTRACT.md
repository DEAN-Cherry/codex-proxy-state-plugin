# State 观测插件

目标宿主为 codex-proxy-rs v3.15.2，SDK 固定提交 `589e1bc999a8b110201b7fdcbf32d2e75bf08253`。
独立工程不依赖 fork 业务代码，不主动请求模型、不刷新 State、不改写请求、不参与调度。

## 用户能力

- 按账号和实际发送模型记录业务响应中的 State
- 每账号选择观察模型，空列表表示观察所有模型
- 长度规则支持逗号分隔的正整数及闭区间，例如 `200,300,400-500`，空字符串表示不限长度
- 保存最近 State 观测摘要、次数、长度分布、最近历史与本地过期时间
- 独立管理页选择账号，按模型显示 State 记录并编辑配置
- State 原文不返回浏览器、日志或诊断，摘要不表示模型能力或服务端有效期
- 不可确定账号或实际发送模型的事件不归入任何账号

## 管理 API

通过 `window.codexProxyPlugin.request` 访问相对路由，JSON 响应不使用宿主信封。

### GET api/accounts

```json
{"accounts":[{"accountId":"acct_example","enabled":true}],"nextCursor":null}
```

基础账号来自 `data` 权限，不申请能读取原始凭据的 `accounts` 权限。
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

账号选择、刷新、筛选模型；紧凑模型表格展示长度、规则匹配、观测次数、最后观测时间和过期状态。
选择模型可查看长度分布和最近历史。
账号配置表单包括启用观测、模型列表、长度规则、保留时间。
支持宿主明暗主题、375/768/1280 宽度、键盘操作、加载/空/错误状态。
生产构建不包含模拟记录或自动安装预览宿主。
