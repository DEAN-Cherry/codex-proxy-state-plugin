import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { PluginPeer } from './qa-peer.mjs'

assert.ok(process.env.QA_PLUGIN_BINARY, 'QA_PLUGIN_BINARY must point to the fresh release binary')
const manifest = await Bun.file(new URL('../plugin.json', import.meta.url)).json()
const peer = new PluginPeer(manifest)
const labelCases = [
  ['acct_demo_001', 'openai OAuth', '  oauth@example.test\t', 'oauth@example.test'],
  ['acct_demo_002', 'Custom account name', 'custom@example.test', 'custom@example.test'],
  ['acct_demo_003', '  Missing email  ', null, 'Missing email'],
  ['acct_demo_004', '  Empty email\t', '', 'Empty email'],
  ['acct_demo_005', '\tBlank email  ', ' \t\n ', 'Blank email'],
  ['acct_demo_006', ' \t ', null, 'acct_demo_006'],
  ['acct_demo_007', '', '', 'acct_demo_007'],
  ['acct_demo_008', ' \n ', '\t ', 'acct_demo_008'],
  ['acct_demo_041', 'Another account', 'oauth@example.test', 'oauth@example.test'],
]
for (const [accountId, name, email] of labelCases) {
  peer.accountNames.set(accountId, name)
  peer.accountEmails.set(accountId, email)
}
const deadline = setTimeout(() => {
  peer.close()
  console.error('Release smoke test exceeded 60 seconds')
  process.exit(1)
}, 60_000)

try {
  await peer.start()
  const directory = await peer.api('GET', 'api/accounts')
  assert.equal(directory.status, 200)
  assert.equal(directory.value.accounts.length, 56)
  assert.equal(directory.value.nextCursor, null)
  assert.deepEqual(directory.value.accounts.map(account => account.accountId).sort(), [...peer.accounts].sort())
  for (const account of directory.value.accounts) {
    assert.deepEqual(Object.keys(account).sort(), ['accountId', 'accountName', 'enabled'])
    assert.equal(account.enabled, !peer.disabledAccounts.has(account.accountId))
  }
  for (const [accountId, , , label] of labelCases) {
    assert.equal(directory.value.accounts.find(account => account.accountId === accountId)?.accountName, label, accountId)
  }

  const accountId = 'acct_demo_001'
  const settings = { enabled: true, models: [], lengthRules: '292,400-500', retentionHours: 24 }
  const saved = await peer.api('POST', 'api/settings', { accountId, expectedVersion: null, settings })
  assert.equal(saved.status, 200)
  assert.equal(saved.value.version, 1)
  assert.deepEqual(saved.value.settings, settings)
  const updatedSettings = { ...settings, retentionHours: 48 }
  const updated = await peer.api('POST', 'api/settings', { accountId, expectedVersion: 1, settings: updatedSettings })
  assert.equal(updated.status, 200)
  assert.equal(updated.value.version, 2)
  const conflict = await peer.api('POST', 'api/settings', { accountId, expectedVersion: 1, settings })
  assert.equal(conflict.status, 409)
  assert.equal(conflict.value.error.code, 'conflict')
  const invalid = await peer.api('POST', 'api/settings', {
    accountId, expectedVersion: 2, settings: { ...settings, lengthRules: '4-3' },
  })
  assert.equal(invalid.status, 400)

  await peer.observe(accountId, 'qa-http-model', 292)
  await peer.observeWebsocket(accountId, 'qa-ws-model', 480)
  const snapshot = await peer.api('POST', 'api/account', { accountId })
  assert.equal(snapshot.status, 200)
  assert.equal(snapshot.value.version, 2)
  assert.deepEqual(snapshot.value.settings, updatedSettings)
  assert.equal(snapshot.value.models.length, 2)
  for (const [model, length, source] of [
    ['qa-http-model', 292, 'http_header'],
    ['qa-ws-model', 480, 'websocket_metadata'],
  ]) {
    const observation = snapshot.value.models.find(entry => entry.model === model)
    assert.ok(observation, `Missing ${model} observation`)
    assert.equal(observation.observations, 1)
    assert.equal(observation.latestLength, length)
    assert.equal(observation.latestFingerprint, createHash('sha256').update('Q'.repeat(length)).digest('hex').slice(0, 12))
    assert.equal(observation.source, source)
    assert.equal(observation.validation, 'matched')
    assert.equal(observation.history.length, 1)
    assert.deepEqual(Object.keys(observation.history[0]).sort(), [
      'eventId', 'fingerprint', 'length', 'observedAtMs', 'source', 'validation',
    ])
  }
  const persisted = peer.states.get(`observations:${accountId}`)
  assert.equal(persisted.schema_version, 1)
  assert.equal(persisted.value.models.length, 2)
  assert.equal(peer.states.get(`settings:${accountId}`).version, 2)

  const overviewPages = []
  let cursor = null
  do {
    assert.ok(overviewPages.length < 3, 'Overview must complete in three pages')
    const overview = await peer.api('POST', 'api/overview', { cursor, limit: 20 })
    assert.equal(overview.status, 200)
    assert.equal(overview.value.accounts.length, overviewPages.length < 2 ? 20 : 16)
    overviewPages.push(overview.value)
    cursor = overview.value.nextCursor
    if (cursor !== null) assert.equal(cursor, overview.value.accounts.at(-1).accountId)
  } while (cursor !== null)
  assert.equal(overviewPages.length, 3)
  const overviewAccounts = overviewPages.flatMap(page => page.accounts)
  assert.equal(overviewAccounts.length, 56)
  assert.deepEqual(overviewAccounts.map(account => account.accountId), directory.value.accounts.map(account => account.accountId))
  for (const [accountId, , , label] of labelCases) {
    assert.equal(overviewAccounts.find(account => account.accountId === accountId)?.accountName, label, accountId)
  }
  for (const account of overviewAccounts) {
    assert.deepEqual(Object.keys(account).sort(), ['accountId', 'accountName', 'enabled', 'error', 'models', 'observationEnabled'])
    assert.deepEqual(
      { accountId: account.accountId, accountName: account.accountName, enabled: account.enabled },
      directory.value.accounts.find(entry => entry.accountId === account.accountId),
    )
    assert.equal(account.error, peer.unavailableAccounts.has(account.accountId) ? 'unavailable' : null)
    assert.equal(account.observationEnabled, peer.unavailableAccounts.has(account.accountId) ? null : true)
    assert.equal(account.models.length, account.accountId === accountId ? 2 : 0)
    for (const model of account.models) {
      assert.ok(!Object.hasOwn(model, 'history'))
      assert.ok(!Object.hasOwn(model, 'lengths'))
    }
  }
  const exposed = JSON.stringify({ directory: directory.value, overview: overviewPages, snapshot: snapshot.value, states: [...peer.states] })
  assert.ok(!exposed.includes('Q'.repeat(292)), 'Raw State must not reach storage or management responses')
  assert.ok(!/"(?:email|provider_id|group_ids|credentialRevision)":/.test(exposed))
  console.log('RELEASE_SMOKE_PASS protocol=2 accounts=56 labels=ok overview=3pages privacy=ok http=ok websocket=ok settings=ok cas=409 invalid=400')
} finally {
  clearTimeout(deadline)
  peer.close()
}
