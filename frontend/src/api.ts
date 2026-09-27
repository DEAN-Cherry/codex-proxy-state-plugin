import { z } from 'zod'

export interface PluginHost {
  readonly version: 2
  readonly theme: 'light' | 'dark'
  request(input: { method: 'GET' | 'POST', path: string, query?: string, contentType?: string, body?: string }): Promise<{ status: number, contentType: string, body: ArrayBuffer }>
  resourceUrl(path: string): string
}
declare global { interface Window { codexProxyPlugin?: PluginHost } }

const count = z.number().int().nonnegative().safe()
const timestamp = count.max(8640000000000000)
const validation = z.enum(['matched', 'mismatch', 'unrestricted'])
const source = z.enum(['http_header', 'sse_metadata', 'websocket_metadata'])
export const settingsSchema = z.object({
  enabled: z.boolean(), models: z.array(z.string().min(1)), lengthRules: z.string(), retentionHours: z.number().int().min(1).max(168),
})
const modelSummary = z.object({
  model: z.string().min(1), observations: count, matched: count, mismatched: count,
  lastObservedAtMs: timestamp, expiresAtMs: timestamp, latestLength: count, latestFingerprint: z.string(),
  source, validation, expired: z.boolean(),
}).strict().refine(row => row.matched + row.mismatched <= row.observations)
export const overviewSchema = z.object({
  accounts: z.array(z.object({
    accountId: z.string().min(1), accountName: z.string(), enabled: z.boolean(), observationEnabled: z.boolean().nullable(), error: z.literal('unavailable').nullable(), models: z.array(modelSummary),
  }).strict()),
  nextCursor: z.string().min(1).nullable(),
  diagnostics: z.object({ unattributed: count, dropped: count, storageFailures: count }), nowMs: timestamp,
})
export const accountsSchema = z.object({
  accounts: z.array(z.object({ accountId: z.string().min(1), accountName: z.string(), enabled: z.boolean() })), nextCursor: z.string().min(1).nullable(),
})
export const snapshotSchema = z.object({
  accountId: z.string().min(1), version: count.nullable(), settings: settingsSchema,
  models: z.array(modelSummary.extend({
    lengths: z.array(z.object({ length: count, count })),
    history: z.array(z.object({ observedAtMs: timestamp, length: count, fingerprint: z.string(), source, validation })),
  })),
  diagnostics: z.object({ unattributed: count, dropped: count, storageFailures: count }), nowMs: timestamp,
})
export type Overview = z.infer<typeof overviewSchema>
export type Snapshot = z.infer<typeof snapshotSchema>
export type Settings = z.infer<typeof settingsSchema>
export type Account = z.infer<typeof accountsSchema>['accounts'][number]
export type OverviewAccount = Overview['accounts'][number]
export type ModelSummary = OverviewAccount['models'][number]
export type Model = Snapshot['models'][number]

export class ApiError extends Error {
  status: number
  constructor(message: string, status: number) { super(message); this.status = status }
}

export async function request<T>(host: PluginHost | undefined, path: string, schema: z.ZodType<T>, data?: unknown, query?: string): Promise<T> {
  if (!host || host.version !== 2) throw new Error('请从网关的插件管理页面打开')
  const reply = await host.request({ path, method: data === undefined ? 'GET' : 'POST',
    ...(query ? { query } : {}), ...(data === undefined ? {} : { contentType: 'application/json', body: JSON.stringify(data) }) })
  if (reply.status === 409) throw new ApiError('配置已被其他页面修改', 409)
  if (reply.contentType.split(';')[0].trim().toLowerCase() !== 'application/json') throw new Error('插件返回了不支持的内容类型')
  let value: unknown
  try { value = JSON.parse(new TextDecoder().decode(reply.body)) }
  catch { throw new Error('插件返回了无效 JSON') }
  if (reply.status < 200 || reply.status >= 300) {
    const error = z.object({ error: z.object({ code: z.string(), message: z.string() }) }).safeParse(value)
    throw new ApiError(error.success ? error.data.error.message : `插件请求失败（HTTP ${reply.status}）`, reply.status)
  }
  const parsed = schema.safeParse(value)
  if (!parsed.success) throw new Error('插件响应格式不符合约定，请检查插件版本')
  return parsed.data
}

export function parseDraft(enabled: boolean, models: string, lengthRules: string, retention: string): Settings {
  const rules = lengthRules.trim()
  if (rules && !rules.split(',').every(part => {
    const match = /^\s*([1-9]\d*)\s*(?:-\s*([1-9]\d*)\s*)?$/.exec(part)
    return match && Number.isSafeInteger(Number(match[1])) && (!match[2] || (Number.isSafeInteger(Number(match[2])) && Number(match[2]) >= Number(match[1])))
  })) throw new Error('长度规则请输入正整数或闭区间，例如 200,300,400-500')
  const retentionHours = Number(retention)
  if (!Number.isInteger(retentionHours) || retentionHours < 1 || retentionHours > 168) throw new Error('保留时间必须为 1 至 168 的整数')
  return { enabled, models: [...new Set(models.split(/[,\n]/).map(model => model.trim()).filter(Boolean))], lengthRules: rules, retentionHours }
}
