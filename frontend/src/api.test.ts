import { test } from 'node:test'
import assert from 'node:assert/strict'
import { accountsSchema, ApiError, parseDraft, request, snapshotSchema } from './api.ts'
import type { PluginHost } from './api.ts'

test('draft normalizes model names and accepts closed intervals', () => {
  assert.deepEqual(parseDraft(true, 'm1, m2,m1', '200,300,400-500', '24'), { enabled: true, models: ['m1', 'm2'], lengthRules: '200,300,400-500', retentionHours: 24 })
  for (const rules of ['0', '-1', '2-1', '1.5', '200,', 'abc']) assert.throws(() => parseDraft(true, '', rules, '24'))
  for (const hours of ['0', '169', '1.2', '']) assert.throws(() => parseDraft(true, '', '', hours))
})
test('bridge preserves cursor query and validates its reply', async () => {
  const host: PluginHost = { version: 2, theme: 'light', resourceUrl: path => path, request: async input => {
    assert.deepEqual(input, { path: 'api/accounts', method: 'GET', query: 'cursor=a%2Fb' })
    return { status: 200, contentType: 'application/json; charset=utf-8', body: new TextEncoder().encode(JSON.stringify({ accounts: [], nextCursor: null })).buffer }
  } }
  assert.deepEqual(await request(host, 'api/accounts', accountsSchema, undefined, 'cursor=a%2Fb'), { accounts: [], nextCursor: null })
  await assert.rejects(request(undefined, 'api/accounts', accountsSchema))
  assert.equal(snapshotSchema.safeParse({ accountId: 'a', version: null }).success, false)
})
test('409 remains a typed conflict even for an empty error response', async () => {
  const host: PluginHost = { version: 2, theme: 'light', resourceUrl: path => path, request: async () => ({ status: 409, contentType: 'text/plain', body: new ArrayBuffer(0) }) }
  await assert.rejects(request(host, 'api/settings', snapshotSchema, {}), error => error instanceof ApiError && error.status === 409)
})
