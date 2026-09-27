import { test } from 'node:test'
import assert from 'node:assert/strict'
import { createStateController } from './useState.ts'
import { filterAccountGroups, modelStatuses } from './overviewGroups.ts'
import type { PluginHost } from './api.ts'

const now = 1790000000000
const json = (data: unknown, status = 200) => Promise.resolve({ status, contentType: 'application/json', body: new TextEncoder().encode(JSON.stringify(data)).buffer })
const model = (name: string, matched: number, mismatched: number, expired = false) => ({
  model: name, observations: matched + mismatched, matched, mismatched, lastObservedAtMs: now, expiresAtMs: now + 1000,
  latestLength: 312, latestFingerprint: 'a1b2c3d4e5f6', source: 'http_header' as const, validation: mismatched ? 'mismatch' as const : 'matched' as const, expired,
})
const account = (id: string, models: ReturnType<typeof model>[] = [], error: 'unavailable' | null = null) => ({
  accountId: id, accountName: id, enabled: true, observationEnabled: error ? null : true, error, models,
})
const hostWithPages = (pages: Array<() => ReturnType<typeof json>>): PluginHost => ({
  version: 2, theme: 'light', resourceUrl: path => path,
  request: input => {
    assert.equal(input.path, 'api/overview')
    assert.equal(input.method, 'POST')
    const next = pages.shift()
    assert.ok(next, 'unexpected extra overview request')
    return next()
  },
})

test('overview auto-loads cursor pages and aggregates all accounts', async () => {
  const requested: unknown[] = []
  const host: PluginHost = {
    version: 2, theme: 'light', resourceUrl: path => path,
    request: input => {
      requested.push(JSON.parse(input.body ?? '{}'))
      if (requested.length === 1) return json({ accounts: [account('acct-a', [model('m1', 1, 0)]), account('acct-b')], nextCursor: 'acct-b', diagnostics: { unattributed: 0, dropped: 0, storageFailures: 0 }, nowMs: now })
      return json({ accounts: [account('acct-c', [], 'unavailable')], nextCursor: null, diagnostics: { unattributed: 1, dropped: 0, storageFailures: 0 }, nowMs: now })
    },
  }
  const state = createStateController(() => host)
  await state.loadOverview('reload')
  assert.deepEqual(requested, [{ cursor: null, limit: 20 }, { cursor: 'acct-b', limit: 20 }])
  assert.equal(state.complete.value, true)
  assert.equal(state.accounts.value.length, 3)
  assert.deepEqual(state.stats.value, { total: 3, withObservations: 1, mismatch: 0, expired: 0, unavailable: 1 })
  assert.ok(state.accounts.value.some(account => account.error === 'unavailable'))
})

test('cancel keeps loaded scope honest and prevents stale page writes', async () => {
  let markSecondStarted: () => void = () => { throw new Error('missing signal') }
  const secondStarted = new Promise<void>(resolve => { markSecondStarted = resolve })
  let releaseSecond: (value: { status: number, contentType: string, body: ArrayBuffer }) => void = () => { throw new Error('missing second request') }
  const host: PluginHost = {
    version: 2, theme: 'light', resourceUrl: path => path,
    request: input => {
      const body = JSON.parse(input.body ?? '{}')
      if (body.cursor === null) return json({ accounts: [account('acct-a')], nextCursor: 'acct-a', diagnostics: { unattributed: 0, dropped: 0, storageFailures: 0 }, nowMs: now })
      return new Promise(resolve => { releaseSecond = resolve; markSecondStarted() })
    },
  }
  const state = createStateController(() => host)
  const pending = state.loadOverview('reload')
  await secondStarted
  state.cancelOverview()
  const second = json({ accounts: [account('acct-b')], nextCursor: null, diagnostics: { unattributed: 0, dropped: 0, storageFailures: 0 }, nowMs: now })
  releaseSecond(await second)
  await pending
  assert.equal(state.cancelled.value, true)
  assert.equal(state.complete.value, false)
  assert.deepEqual(state.accounts.value.map(item => item.accountId), ['acct-a', 'acct-b'])
  assert.equal(state.loading.value, false)
})

test('resume retries from last cursor and ignores superseded responses', async () => {
  const host: PluginHost = {
    version: 2, theme: 'light', resourceUrl: path => path,
    request: input => {
      const body = JSON.parse(input.body ?? '{}')
      if (body.cursor === null) return json({ accounts: [account('acct-a')], nextCursor: 'acct-a', diagnostics: { unattributed: 0, dropped: 0, storageFailures: 0 }, nowMs: now })
      return json({ error: { code: 'unavailable', message: '读取失败' } }, 500)
    },
  }
  const state = createStateController(() => host)
  await state.loadOverview('reload')
  assert.equal(state.overviewError.value, '读取失败')
  assert.equal(state.nextCursor.value, 'acct-a')
  assert.equal(state.complete.value, false)
})

test('detail and save preserve dirty drafts and typed 409 conflicts', async () => {
  const snapshot = (version: number | null) => ({
    accountId: 'acct-a', version,
    settings: { enabled: true, models: [], lengthRules: '', retentionHours: 24 },
    models: [], diagnostics: { unattributed: 0, dropped: 0, storageFailures: 0 }, nowMs: now,
  })
  const host: PluginHost = {
    version: 2, theme: 'light', resourceUrl: path => path,
    request: input => {
      if (input.path === 'api/account') return json(snapshot(null))
      if (input.path === 'api/settings') return json({ error: { code: 'conflict', message: 'Conflict' } }, 409)
      throw new Error(`unexpected path ${input.path}`)
    },
  }
  const state = createStateController(() => host)
  await state.openAccount('acct-a')
  state.models.value = 'draft-model'
  assert.equal(state.dirty.value, true)
  await state.refreshDetail()
  assert.equal(state.models.value, 'draft-model')
  await state.save()
  assert.equal(state.conflict.value, true)
  assert.equal(state.models.value, 'draft-model')
  state.requestClose()
  assert.equal(state.modalOpen.value, true)
  assert.equal(state.confirmClose.value, true)
  state.requestClose()
  assert.equal(state.modalOpen.value, true)
  state.confirmDiscardClose()
  assert.equal(state.modalOpen.value, false)
  assert.equal(state.snapshot.value, null)
})

test('account groups retain empty accounts and per-account failures for filtering', () => {
  const state = createStateController(() => hostWithPages([]))
  state.accounts.value = [
    account('acct-empty'),
    account('acct-error', [], 'unavailable'),
    account('acct-mixed', [model('m1', 2, 1, true)]),
  ]
  const empty = filterAccountGroups(state.accounts.value, { account: '', model: '', status: 'unobserved' })
  const failed = filterAccountGroups(state.accounts.value, { account: '', model: '', status: 'unavailable' })
  assert.deepEqual(empty.map(account => account.accountId), ['acct-empty'])
  assert.deepEqual(failed.map(account => account.accountId), ['acct-error'])
  assert.deepEqual(modelStatuses(state.accounts.value[2].models[0]), ['mismatch', 'matched', 'expired'])
})

test('saving keeps the account open and refreshes its overview summaries', { timeout: 1000 }, async () => {
  const snapshot = (updated: boolean) => ({
    accountId: 'acct-a', version: updated ? 2 : 1,
    settings: { enabled: true, models: [], lengthRules: updated ? '292' : '', retentionHours: 24 },
    models: [{ ...model('m1', updated ? 0 : 1, updated ? 1 : 0), latestLength: updated ? 780 : 312, lengths: [], history: [] }],
    diagnostics: { unattributed: 0, dropped: 0, storageFailures: 0 }, nowMs: now,
  })
  let finishSave: (value: Awaited<ReturnType<typeof json>>) => void = () => { throw new Error('missing save signal') }
  const saveResponse = new Promise<Awaited<ReturnType<typeof json>>>(resolve => { finishSave = resolve })
  const host: PluginHost = {
    version: 2, theme: 'light', resourceUrl: path => path,
    request: input => input.path === 'api/account' ? json(snapshot(false)) : saveResponse,
  }
  const state = createStateController(() => host)
  state.accounts.value = [account('acct-a', [model('m1', 1, 0)])]
  await state.openAccount('acct-a')
  state.lengthRules.value = '292'
  const saving = state.save()
  state.requestClose()
  state.confirmDiscardClose()
  await state.openAccount('acct-b')
  assert.equal(state.modalOpen.value, true)
  assert.equal(state.activeAccountId.value, 'acct-a')
  finishSave(await json(snapshot(true)))
  await saving
  assert.equal(state.accounts.value[0].models[0].latestLength, 780)
  assert.equal(state.stats.value.mismatch, 1)
})
