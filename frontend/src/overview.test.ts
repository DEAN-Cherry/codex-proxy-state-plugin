import { test } from 'node:test'
import assert from 'node:assert/strict'
import { overviewSchema } from './api.ts'
import { accountLabel, filterAccountGroups, pageGroups } from './overviewGroups.ts'
import type { OverviewAccount } from './api.ts'

const now = 1790000000000
const validAccount = {
  accountId: 'acct-a', accountName: 'Shared name', enabled: false, observationEnabled: null, error: 'unavailable',
  models: [],
}
const validModel = {
  model: 'gpt-6-astra', observations: 2, matched: 1, mismatched: 1, lastObservedAtMs: now, expiresAtMs: now + 1000,
  latestLength: 312, latestFingerprint: 'a1b2c3d4e5f6', source: 'http_header', validation: 'mismatch', expired: true,
}

test('overview boundary accepts unavailable and empty accounts without summaries', () => {
  const result = overviewSchema.safeParse({
    accounts: [validAccount, { accountId: 'acct-b', accountName: '', enabled: true, observationEnabled: true, error: null, models: [validModel] }],
    nextCursor: null, diagnostics: { unattributed: 0, dropped: 0, storageFailures: 0 }, nowMs: now,
  })
  assert.equal(result.success, true)
  assert.equal(result.success && result.data.accounts[0].models.length, 0)
})

test('overview boundary rejects lengths, history and impossible counters', () => {
  const base = { accounts: [{ accountId: 'acct-a', accountName: 'Shared name', enabled: true, observationEnabled: true, error: null, models: [{ ...validModel, lengths: [] }] }], nextCursor: null, diagnostics: { unattributed: 0, dropped: 0, storageFailures: 0 }, nowMs: now }
  assert.equal(overviewSchema.safeParse(base).success, false)
  const impossible = { ...base, accounts: [{ ...base.accounts[0], models: [{ ...validModel, matched: 3 }] }] }
  assert.equal(overviewSchema.safeParse(impossible).success, false)
})

test('name and model filters preserve distinct account groups with matching children', () => {
  const groups: OverviewAccount[] = overviewSchema.parse({
    accounts: ['acct-a', 'acct-b'].map(accountId => ({
      accountId, accountName: 'Shared name', enabled: true, observationEnabled: true, error: null,
      models: [{ ...validModel, model: 'model-a' }, { ...validModel, model: 'model-b' }],
    })),
    nextCursor: null, diagnostics: { unattributed: 0, dropped: 0, storageFailures: 0 }, nowMs: now,
  }).accounts
  const filtered = filterAccountGroups(groups, { account: ' SHARED ', model: 'model-b', status: '' })
  assert.deepEqual(filtered.map(account => account.accountId), ['acct-a', 'acct-b'])
  assert.deepEqual(filtered.map(account => account.models.map(model => model.model)), [['model-b'], ['model-b']])
  assert.equal(groups[0].models.length, 2)
  assert.deepEqual(filterAccountGroups(groups, { account: 'acct-b', model: '', status: '' }).map(account => account.accountId), ['acct-b'])
})

test('expired models past the display grace move to history and stay reachable via the expired filter', () => {
  const hour = 3_600_000
  const base = { ...validModel, source: 'http_header' as const, validation: 'mismatch' as const }
  const live = { ...base, model: 'live', expired: false, expiresAtMs: now + hour }
  const recent = { ...base, model: 'recent', expired: true, expiresAtMs: now - hour / 2 }
  const stale = { ...base, model: 'stale', expired: true, expiresAtMs: now - 2 * hour }
  const groups: OverviewAccount[] = [
    { accountId: 'acct-a', accountName: '', enabled: true, observationEnabled: true, error: null, models: [live, recent, stale] },
    { accountId: 'acct-b', accountName: '', enabled: true, observationEnabled: true, error: null, models: [stale] },
  ]
  const none = { account: '', model: '', status: '' as const }
  const strict = filterAccountGroups(groups, none, { nowMs: now, graceMs: 0 })
  assert.deepEqual(strict.map(group => [group.accountId, group.models.map(model => model.model), group.archived]), [['acct-a', ['live'], 2], ['acct-b', [], 1]])
  const grace = filterAccountGroups(groups, none, { nowMs: now, graceMs: hour })
  assert.deepEqual(grace.map(group => [group.models.map(model => model.model), group.archived]), [[['live', 'recent'], 1], [[], 1]])
  // 带筛选条件时，只剩历史 State 的账号不再占位
  assert.deepEqual(filterAccountGroups(groups, { ...none, model: 'live' }, { nowMs: now, graceMs: 0 }).map(group => group.accountId), ['acct-a'])
  const expired = filterAccountGroups(groups, { ...none, status: 'expired' }, { nowMs: now, graceMs: 0 })
  assert.deepEqual(expired.map(group => group.models.map(model => model.model)), [['recent', 'stale'], ['stale']])
  assert.deepEqual(filterAccountGroups(groups, none).map(group => group.models.length), [3, 1])
})

test('pagination never splits models of the same account and unnamed accounts retain identity', () => {
  const groups: OverviewAccount[] = ['acct-a', 'acct-b'].map(accountId => ({
    accountId, accountName: '', enabled: true, observationEnabled: true, error: null,
    models: [0, 1, 2].map(index => ({ ...validModel, model: `model-${index}`, source: 'http_header', validation: 'mismatch' })),
  }))
  assert.equal(accountLabel(groups[0]), 'acct-a')
  assert.equal(accountLabel({ accountId: 'acct-b', accountName: '  Primary  ' }), 'Primary')
  assert.deepEqual(pageGroups(groups, 1, 1).map(account => [account.accountId, account.models.length]), [['acct-a', 3]])
  assert.deepEqual(pageGroups(groups, 2, 1).map(account => [account.accountId, account.models.length]), [['acct-b', 3]])
})
