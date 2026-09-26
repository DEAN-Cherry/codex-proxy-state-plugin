import { computed, onMounted, ref } from 'vue'
import { accountsSchema, ApiError, parseDraft, request, snapshotSchema } from './api'
import type { Account, Settings, Snapshot } from './api'

export function useState() {
  const accounts = ref<Account[]>([])
  const cursor = ref<string | null>(null)
  const selected = ref('')
  const snapshot = ref<Snapshot | null>(null)
  const loading = ref(false)
  const paging = ref(false)
  const saving = ref(false)
  const error = ref('')
  const accountError = ref('')
  const formError = ref('')
  const notice = ref('')
  const conflict = ref(false)
  const enabled = ref(true)
  const models = ref('')
  const lengthRules = ref('')
  const retention = ref('24')
  const baseline = ref('')
  const editVersion = ref<Snapshot['version']>(null)
  let sequence = 0
  const draftKey = computed(() => JSON.stringify([enabled.value, models.value, lengthRules.value, retention.value]))
  const dirty = computed(() => baseline.value !== '' && baseline.value !== draftKey.value)
  const message = (e: unknown) => e instanceof Error ? e.message : '请求失败，请重试'

  function setDraft(settings: Settings, version: Snapshot['version']) {
    enabled.value = settings.enabled
    models.value = settings.models.join(', ')
    lengthRules.value = settings.lengthRules
    retention.value = String(settings.retentionHours)
    editVersion.value = version
    baseline.value = draftKey.value
    formError.value = ''
    conflict.value = false
  }
  async function loadAccounts() {
    if (paging.value) return
    paging.value = true
    accountError.value = ''
    try {
      const page = await request(window.codexProxyPlugin, 'api/accounts', accountsSchema, undefined, cursor.value ? `cursor=${encodeURIComponent(cursor.value)}` : undefined)
      const known = new Set(accounts.value.map(account => account.accountId))
      accounts.value.push(...page.accounts.filter(account => !known.has(account.accountId)))
      if (page.nextCursor && page.nextCursor === cursor.value) throw new Error('账号分页游标未前进，请重新打开页面')
      cursor.value = page.nextCursor
      if (!selected.value && accounts.value.length) {
        selected.value = accounts.value[0].accountId
        await refresh(true)
      }
    } catch (e) { accountError.value = message(e) }
    finally { paging.value = false }
  }
  async function refresh(replaceDraft = false) {
    if (!selected.value || saving.value) return
    const token = ++sequence
    const id = selected.value
    loading.value = true
    error.value = ''
    notice.value = ''
    try {
      const result = await request(window.codexProxyPlugin, 'api/account', snapshotSchema, { accountId: id })
      if (result.accountId !== id) throw new Error('插件返回的账号与请求不一致')
      if (token !== sequence) return
      snapshot.value = result
      if (replaceDraft || (!dirty.value && !conflict.value)) setDraft(result.settings, result.version)
    } catch (e) { if (token === sequence) error.value = message(e) }
    finally { if (token === sequence) loading.value = false }
  }
  function selectAccount(event: Event) {
    if (!(event.target instanceof HTMLSelectElement)) return
    selected.value = event.target.value
    snapshot.value = null
    baseline.value = ''
    void refresh(true)
  }
  async function save() {
    if (!snapshot.value || saving.value || loading.value || conflict.value) return
    formError.value = ''
    notice.value = ''
    let settings: Settings
    try { settings = parseDraft(enabled.value, models.value, lengthRules.value, retention.value) }
    catch (e) { formError.value = message(e); return }
    saving.value = true
    try {
      const result = await request(window.codexProxyPlugin, 'api/settings', snapshotSchema, {
        accountId: selected.value, expectedVersion: editVersion.value, settings,
      })
      if (result.accountId !== selected.value) throw new Error('插件返回的账号与请求不一致')
      snapshot.value = result
      setDraft(result.settings, result.version)
      notice.value = '配置已保存'
    } catch (e) {
      if (e instanceof ApiError && e.status === 409) conflict.value = true
      else formError.value = message(e)
    } finally { saving.value = false }
  }
  onMounted(loadAccounts)
  return { accounts, cursor, selected, snapshot, loading, paging, saving, error, accountError, formError, notice, conflict, enabled, models, lengthRules, retention, dirty, setDraft, loadAccounts, refresh, selectAccount, save }
}
