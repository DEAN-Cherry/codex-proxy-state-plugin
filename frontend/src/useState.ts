import { computed, hasInjectionContext, onMounted, ref } from 'vue'
import { ApiError, overviewSchema, parseDraft, request, snapshotSchema } from './api'
import type { Overview, OverviewAccount, PluginHost, Settings, Snapshot } from './api'

const PAGE_LIMIT = 20
const message = (e: unknown) => e instanceof Error ? e.message : '请求失败，请重试'

export function createStateController(host: () => PluginHost | undefined = () => window.codexProxyPlugin) {
  const accounts = ref<OverviewAccount[]>([])
  const nextCursor = ref<string | null>(null)
  const diagnostics = ref<Overview['diagnostics'] | null>(null)
  const nowMs = ref(0)
  const loading = ref(false)
  const cancelled = ref(false)
  const complete = ref(false)
  const overviewError = ref('')
  let overviewSequence = 0

  const modalOpen = ref(false)
  const confirmClose = ref(false)
  const activeAccountId = ref('')
  const snapshot = ref<Snapshot | null>(null)
  const detailLoading = ref(false)
  const detailError = ref('')
  const saving = ref(false)
  const formError = ref('')
  const notice = ref('')
  const conflict = ref(false)
  const enabled = ref(true)
  const models = ref('')
  const lengthRules = ref('')
  const retention = ref('24')
  const baseline = ref('')
  const editVersion = ref<Snapshot['version']>(null)
  let detailSequence = 0

  const draftKey = computed(() => JSON.stringify([enabled.value, models.value, lengthRules.value, retention.value]))
  const dirty = computed(() => baseline.value !== '' && baseline.value !== draftKey.value)
  const stats = computed(() => {
    const withObservations = accounts.value.filter(account => !account.error && account.models.length > 0).length
    const mismatch = accounts.value.filter(account => !account.error && account.models.some(model => model.mismatched > 0)).length
    const expired = accounts.value.filter(account => !account.error && account.models.some(model => model.expired)).length
    const unavailable = accounts.value.filter(account => account.error).length
    return { total: accounts.value.length, withObservations, mismatch, expired, unavailable }
  })

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
  function clearDetail() {
    activeAccountId.value = ''
    snapshot.value = null
    detailError.value = ''
    formError.value = ''
    notice.value = ''
    conflict.value = false
    baseline.value = ''
  }
  async function loadOverview(mode: 'reload' | 'resume' = accounts.value.length || nextCursor.value ? 'resume' : 'reload') {
    if (loading.value) return
    const token = ++overviewSequence
    loading.value = true
    cancelled.value = false
    overviewError.value = ''
    if (mode === 'reload') {
      accounts.value = []
      nextCursor.value = null
      diagnostics.value = null
      nowMs.value = 0
      complete.value = false
    }
    try {
      let cursor = mode === 'reload' ? null : nextCursor.value
      while (true) {
        const page = await request(host(), 'api/overview', overviewSchema, { cursor, limit: PAGE_LIMIT })
        if (token !== overviewSequence) return
        const known = new Set(accounts.value.map(account => account.accountId))
        accounts.value.push(...page.accounts.filter(account => !known.has(account.accountId)))
        if (cursor !== null && page.nextCursor === cursor) throw new Error('概况分页游标未前进，请重新加载')
        cursor = page.nextCursor
        nextCursor.value = page.nextCursor
        diagnostics.value = page.diagnostics
        nowMs.value = page.nowMs
        if (!page.nextCursor || cancelled.value || token !== overviewSequence) break
      }
      complete.value = !cancelled.value && nextCursor.value === null
    } catch (e) {
      overviewError.value = message(e)
      complete.value = false
    } finally {
      if (token === overviewSequence) loading.value = false
    }
  }
  function cancelOverview() {
    if (!loading.value) return
    cancelled.value = true
  }
  async function openAccount(accountId: string) {
    if (saving.value || dirty.value) return
    const token = ++detailSequence
    confirmClose.value = false
    modalOpen.value = true
    activeAccountId.value = accountId
    snapshot.value = null
    baseline.value = ''
    detailError.value = ''
    formError.value = ''
    notice.value = ''
    conflict.value = false
    detailLoading.value = true
    try {
      const result = await request(host(), 'api/account', snapshotSchema, { accountId })
      if (result.accountId !== accountId) throw new Error('插件返回的账号与请求不一致')
      if (token !== detailSequence) return
      snapshot.value = result
      setDraft(result.settings, result.version)
    } catch (e) {
      if (token === detailSequence) detailError.value = message(e)
    } finally {
      if (token === detailSequence) detailLoading.value = false
    }
  }
  async function refreshDetail(replaceDraft = false) {
    const accountId = activeAccountId.value
    if (!accountId || detailLoading.value || saving.value) return
    const token = ++detailSequence
    detailLoading.value = true
    detailError.value = ''
    notice.value = ''
    try {
      const result = await request(host(), 'api/account', snapshotSchema, { accountId })
      if (result.accountId !== accountId) throw new Error('插件返回的账号与请求不一致')
      if (token !== detailSequence) return
      snapshot.value = result
      if (replaceDraft || (!dirty.value && !conflict.value)) setDraft(result.settings, result.version)
    } catch (e) {
      if (token === detailSequence) detailError.value = message(e)
    } finally {
      if (token === detailSequence) detailLoading.value = false
    }
  }
  function requestClose() {
    if (saving.value) return
    if (dirty.value) {
      confirmClose.value = true
      return
    }
    confirmClose.value = false
    modalOpen.value = false
    detailSequence += 1
    clearDetail()
  }
  function confirmDiscardClose() {
    if (saving.value) return
    confirmClose.value = false
    modalOpen.value = false
    detailSequence += 1
    clearDetail()
  }
  async function save() {
    const accountId = activeAccountId.value
    if (!accountId || saving.value || detailLoading.value || conflict.value) return
    formError.value = ''
    notice.value = ''
    let settings: Settings
    try { settings = parseDraft(enabled.value, models.value, lengthRules.value, retention.value) }
    catch (e) { formError.value = message(e); return }
    saving.value = true
    try {
      const result = await request(host(), 'api/settings', snapshotSchema, { accountId, expectedVersion: editVersion.value, settings })
      if (result.accountId !== accountId) throw new Error('插件返回的账号与请求不一致')
      snapshot.value = result
      setDraft(result.settings, result.version)
      notice.value = '配置已保存'
      const overviewAccount = accounts.value.find(account => account.accountId === accountId)
      if (overviewAccount && !overviewAccount.error) {
        overviewAccount.observationEnabled = result.settings.enabled
        overviewAccount.models = result.models
      }
    } catch (e) {
      if (e instanceof ApiError && e.status === 409) conflict.value = true
      else formError.value = message(e)
    } finally { saving.value = false }
  }
  function start() { void loadOverview('reload') }
  if (hasInjectionContext()) onMounted(start)
  return {
    accounts, stats, nextCursor, diagnostics, nowMs, loading, cancelled, complete, overviewError,
    modalOpen, confirmClose, activeAccountId, snapshot, detailLoading, detailError, saving, formError, notice, conflict,
    enabled, models, lengthRules, retention, dirty, setDraft,
    loadOverview, cancelOverview, openAccount, refreshDetail, requestClose, confirmDiscardClose, save,
  }
}
export function useState() { return createStateController() }
