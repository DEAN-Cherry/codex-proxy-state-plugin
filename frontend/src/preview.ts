// Explicit development entry only, never imported by main.ts or emitted by the library build
import { applyResolvedTheme, DEFAULT_CUSTOM_THEME_COLOR, DEFAULT_THEME_COLOR, resolveTheme } from '@codex-proxy/ui/theme'
import type { PluginHost, Snapshot } from './api'

if (!import.meta.env.DEV) throw new Error('Development preview only')
const mode = new URLSearchParams(window.location.search).get('theme') === 'dark' ? 'dark' : 'light'
applyResolvedTheme(document.documentElement, resolveTheme(mode, DEFAULT_THEME_COLOR, DEFAULT_CUSTOM_THEME_COLOR))
const now = 1790000000000
const snapshot: Snapshot = {
  accountId: 'preview-account', version: null,
  settings: { enabled: true, models: [], lengthRules: '', retentionHours: 24 },
  models: [{ model: 'gpt-6-astra', observations: 2, matched: 1, mismatched: 1,
    lastObservedAtMs: now, expiresAtMs: now + 86400000, latestLength: 312, latestFingerprint: 'a1b2c3d4e5f6',
    source: 'http_header', validation: 'mismatch', expired: false,
    lengths: [{ length: 292, count: 1 }, { length: 312, count: 1 }],
    history: [{ observedAtMs: now, length: 312, fingerprint: 'a1b2c3d4e5f6', source: 'http_header', validation: 'mismatch' }],
  }], diagnostics: { unattributed: 0, dropped: 0, storageFailures: 0 }, nowMs: now,
}
const json = (data: unknown, status = 200) => Promise.resolve({ status, contentType: 'application/json', body: new TextEncoder().encode(JSON.stringify(data)).buffer })
const host: PluginHost = {
  version: 2, theme: mode, resourceUrl: path => path,
  request(input) {
    if (input.path === 'api/accounts') return json({ accounts: [{ accountId: input.query ? 'preview-empty' : 'preview-account', enabled: true }], nextCursor: input.query ? null : 'page-2' })
    const data = JSON.parse(input.body ?? '{}')
    if (input.path === 'api/settings') {
      if (new URLSearchParams(window.location.search).has('conflict')) return json({ error: { code: 'conflict', message: 'Conflict' } }, 409)
      if (data.expectedVersion !== snapshot.version) return json({}, 409)
      snapshot.settings = data.settings
      snapshot.version = Number(snapshot.version ?? 0) + 1
    }
    return json({ ...snapshot, accountId: data.accountId, models: data.accountId === 'preview-empty' ? [] : snapshot.models })
  },
}
window.codexProxyPlugin = host
await import('./main')
