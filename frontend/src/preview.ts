// Explicit development entry only, never imported by main.ts or emitted by the library build
import { applyResolvedTheme, DEFAULT_CUSTOM_THEME_COLOR, DEFAULT_THEME_COLOR, resolveTheme } from '@codex-proxy/ui/theme'
import type { OverviewAccount, PluginHost, Snapshot } from './api'

if (!import.meta.env.DEV) throw new Error('Development preview only')
const params = new URLSearchParams(window.location.search)
const mode = params.get('theme') === 'dark' ? 'dark' : 'light'
const scenario = params.get('scenario') ?? 'full'
applyResolvedTheme(document.documentElement, resolveTheme(mode, DEFAULT_THEME_COLOR, DEFAULT_CUSTOM_THEME_COLOR))
const now = 1790000000000
const modelNames = ['gpt-6-astra', 'gpt-6-vega', 'gpt-5.4-sonic', 'gpt-5.2-orbit']
const summary = (name: string, index: number) => ({
  model: name, observations: 3 + index, matched: index % 2, mismatched: index % 3 === 0 ? 1 : 0,
  lastObservedAtMs: now - index * 60000, expiresAtMs: now + (index % 4 === 0 ? -1000 : 86400000),
  latestLength: 292 + index * 10, latestFingerprint: `${index.toString(16).padStart(12, '0')}`,
  source: index % 2 ? 'sse_metadata' : 'http_header', validation: index % 3 === 0 ? 'mismatch' : index % 2 ? 'unrestricted' : 'matched',
  expired: index % 4 === 0,
}) satisfies OverviewAccount['models'][number]
const accounts: OverviewAccount[] = scenario === 'empty' ? [] : Array.from({ length: 56 }, (_, index) => {
  const id = index < 52 ? `acct_demo_${String(index + 1).padStart(3, '0')}` : ['acct_demo_pro', 'acct_demo_plus', 'acct_demo_empty', 'acct_demo_unavailable'][index - 52]
  const unavailable = id.endsWith('unavailable') || (scenario === 'partial-error' && id === 'acct_demo_002')
  return {
    accountId: id, accountName: index < 2 ? '同名账号' : index === 2 ? '' : `演示账号 ${index + 1}`, enabled: id !== 'acct_demo_empty' && index % 9 !== 0,
    observationEnabled: unavailable ? null : index % 7 !== 0,
    error: unavailable ? 'unavailable' : null,
    models: unavailable || id.endsWith('empty') ? [] : modelNames.slice(0, (index % 4) + 1).map((name, modelIndex) => summary(name, index + modelIndex)),
  }
})
const snapshots = new Map<string, Snapshot>()
const json = (data: unknown, status = 200) => Promise.resolve({ status, contentType: 'application/json', body: new TextEncoder().encode(JSON.stringify(data)).buffer })
const detail = (account: OverviewAccount): Snapshot => {
  const existing = snapshots.get(account.accountId)
  if (existing) return existing
  const created: Snapshot = {
    accountId: account.accountId, version: null,
    settings: { enabled: account.observationEnabled ?? true, models: [], lengthRules: indexRule(account.accountId), retentionHours: 24 },
    models: account.models.map(item => ({ ...item, lengths: [{ length: item.latestLength, count: item.observations }], history: [{ observedAtMs: item.lastObservedAtMs, length: item.latestLength, fingerprint: item.latestFingerprint, source: item.source, validation: item.validation }] })),
    diagnostics: { unattributed: 2, dropped: 1, storageFailures: account.error ? 1 : 0 }, nowMs: now,
  }
  snapshots.set(account.accountId, created)
  return created
}
function indexRule(accountId: string) { return /_00[25]$/.test(accountId) ? '200,300,400-500' : '' }
const host: PluginHost = {
  version: 2, theme: mode, resourceUrl: path => path,
  async request(input) {
    if (input.path === 'api/overview') {
      const data = JSON.parse(input.body ?? '{}') as { cursor: string | null, limit: number }
      const start = data.cursor ? accounts.findIndex(account => account.accountId === data.cursor) + 1 : 0
      const limit = Math.min(Math.max(data.limit || 20, 1), 50)
      const page = accounts.slice(start, start + limit)
      await new Promise(resolve => setTimeout(resolve, params.has('slow') ? 120 : 0))
      return json({ accounts: page, nextCursor: start + limit < accounts.length ? page.at(-1)?.accountId ?? null : null, diagnostics: { unattributed: 2, dropped: 1, storageFailures: scenario === 'partial-error' ? 1 : 0 }, nowMs: Date.now() })
    }
    const data = JSON.parse(input.body ?? '{}') as { accountId?: string, expectedVersion?: number | null, settings?: Snapshot['settings'] }
    const account = accounts.find(item => item.accountId === data.accountId)
    if (!account) return json({ error: { code: 'not_found', message: '账号不存在' } }, 404)
    if (input.path === 'api/account') return json(detail(account))
    if (input.path === 'api/settings') {
      const current = detail(account)
      if (params.has('conflict')) return json({ error: { code: 'conflict', message: 'Conflict' } }, 409)
      if (data.expectedVersion !== current.version) return json({ error: { code: 'conflict', message: 'Conflict' } }, 409)
      if (!data.settings) return json({ error: { code: 'invalid_request', message: '缺少配置' } }, 400)
      current.settings = data.settings
      current.version = Number(current.version ?? 0) + 1
      account.observationEnabled = data.settings.enabled
      return json(current)
    }
    return json({ error: { code: 'not_found', message: '未知接口' } }, 404)
  },
}
window.codexProxyPlugin = host
await import('./main')
