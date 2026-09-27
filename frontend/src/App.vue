<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { BaseButton, BaseCard, BaseConfirmModal, BaseEmpty, BaseInput, BaseModal, BaseTablePagination, BaseTag } from '@codex-proxy/ui'
import ObservationTable from './ObservationTable.vue'
import AccountGroup from './AccountGroup.vue'
import { useState } from './useState'
import { accountLabel, clampPage, filterAccountGroups, pageGroups, statusNames } from './overviewGroups'
import type { RowStatus } from './overviewGroups'

const state = useState()
const accountFilter = ref('')
const modelFilter = ref('')
const statusFilter = ref<RowStatus | ''>('')
const currentPage = ref(1)
const pageSize = ref(10)
const filteredGroups = computed(() => filterAccountGroups(state.accounts.value, { account: accountFilter.value, model: modelFilter.value, status: statusFilter.value }))
const totalPages = computed(() => Math.max(1, Math.ceil(filteredGroups.value.length / pageSize.value)))
const pagedGroups = computed(() => pageGroups(filteredGroups.value, currentPage.value, pageSize.value))
const pagination = computed(() => ({ currentPage: currentPage.value, pageSize: pageSize.value, total: filteredGroups.value.length, pageSizes: [10, 20, 50] }))
const activeName = computed(() => {
  const account = state.accounts.value.find(account => account.accountId === state.activeAccountId.value)
  return account ? accountLabel(account) : state.activeAccountId.value || '账号详情'
})
const scopeLabel = computed(() => state.complete.value ? `全部账号 ${state.accounts.value.length} 个` : `已加载 ${state.accounts.value.length} 个账号${state.nextCursor.value ? '，尚未完成' : ''}`)
watch([accountFilter, modelFilter, statusFilter], () => { currentPage.value = 1 })
watch(totalPages, () => { currentPage.value = clampPage(currentPage.value, filteredGroups.value.length, pageSize.value) })
watch(() => state.saving.value, async (saving, wasSaving) => {
  if (wasSaving && !saving && state.modalOpen.value) {
    // 保存按钮变为禁用后恢复到可用控件，避免焦点落到弹窗外导致 Escape 失效。
    await nextTick()
    document.getElementById('account-close')?.focus()
  }
})

function retryOverview() {
  void state.loadOverview(state.accounts.value.length && state.nextCursor.value ? 'resume' : 'reload')
}
</script>

<template>
  <main class="flex min-w-0 flex-col gap-4 font-sans text-cp-text">
    <BaseCard padding="compact" aria-label="账号概况范围">
      <div class="flex flex-wrap items-center justify-between gap-3">
        <div class="flex min-w-0 flex-wrap items-center gap-2">
          <BaseTag type="primary">{{ scopeLabel }}</BaseTag>
          <BaseTag v-if="state.cancelled.value" type="warning">已取消自动加载</BaseTag>
          <BaseTag v-if="state.loading.value" type="info">正在加载账号页</BaseTag>
        </div>
        <div class="flex flex-wrap gap-2">
          <BaseButton v-if="state.loading.value" size="sm" @click="state.cancelOverview()">取消加载</BaseButton>
          <BaseButton v-else-if="!state.complete.value && state.nextCursor.value" size="sm" @click="state.loadOverview('resume')">继续加载</BaseButton>
          <BaseButton size="sm" :loading="state.loading.value" @click="state.loadOverview('reload')">重新加载</BaseButton>
        </div>
      </div>
      <p v-if="!state.complete.value" class="mt-2 mb-0 text-cp-xs text-cp-text-secondary">加载完成前，统计只覆盖已加载账号</p>
      <div v-if="state.overviewError.value" role="alert" class="mt-3 flex flex-wrap items-center gap-2 rounded-cp bg-cp-error-container p-3 text-cp-sm text-cp-error-on-container">
        <span>{{ state.overviewError.value }}</span>
        <BaseButton size="sm" @click="retryOverview">重试</BaseButton>
      </div>
    </BaseCard>

    <section class="grid min-w-0 grid-cols-3 gap-3 md:grid-cols-5" aria-label="已加载账号统计">
      <BaseCard v-for="item in [
        { label: '账号', value: state.stats.value.total, hint: scopeLabel },
        { label: '有观测', value: state.stats.value.withObservations, hint: '包含历史观测' },
        { label: '有不匹配', value: state.stats.value.mismatch, hint: '保留样本内' },
        { label: '有过期', value: state.stats.value.expired, hint: '本地保留窗口' },
        { label: '读取失败', value: state.stats.value.unavailable, hint: '不视为健康' },
      ]" :key="item.label" padding="compact" as="article">
        <p class="m-0 text-cp-xs text-cp-text-secondary">{{ item.label }}</p>
        <p class="mt-2 mb-0 font-mono text-cp-xl font-bold text-cp-text">{{ item.value }}</p>
        <p class="mt-1 mb-0 hidden text-cp-xs text-cp-text-tertiary sm:block">{{ item.hint }}</p>
      </BaseCard>
    </section>

    <BaseCard title="账号与模型概况" padding="compact">
      <template #actions><BaseTag>{{ filteredGroups.length }} 个账号</BaseTag></template>
      <div class="grid min-w-0 gap-3 md:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_12rem]">
        <div class="flex min-w-0 flex-col gap-2">
          <label for="account-filter" class="text-cp-sm font-bold">账号</label>
          <BaseInput id="account-filter" v-model="accountFilter" type="search" placeholder="搜索名称或 ID" />
        </div>
        <div class="flex min-w-0 flex-col gap-2">
          <label for="model-filter" class="text-cp-sm font-bold">模型</label>
          <BaseInput id="model-filter" v-model="modelFilter" type="search" placeholder="输入实际发送模型" />
        </div>
        <div class="flex min-w-0 flex-col gap-2">
          <label for="status-filter" class="text-cp-sm font-bold">状态</label>
          <select id="status-filter" v-model="statusFilter"
            class="h-cp-control w-full min-w-0 rounded-cp border-0 bg-[var(--cp-input-bg)] px-3 text-cp text-cp-text shadow-cp-input focus-visible:outline-2 focus-visible:outline-cp-control-outline">
            <option value="">全部状态</option>
            <option v-for="(name, status) in statusNames" :key="status" :value="status">{{ name }}</option>
          </select>
        </div>
      </div>
      <p v-if="!state.complete.value" class="mt-2 mb-0 text-cp-xs text-cp-text-secondary">筛选暂时只覆盖已加载账号</p>

      <p v-if="state.loading.value && !state.accounts.value.length" role="status" class="my-6 text-cp-sm text-cp-text-secondary">正在加载账号与模型记录</p>
      <BaseEmpty v-else-if="!state.accounts.value.length && !state.overviewError.value" class="mt-4" title="暂无可访问的 OpenAI 账号" description="概况包含停用账号、空账号和读取失败账号" surface="none" />
      <BaseEmpty v-else-if="!filteredGroups.length && state.accounts.value.length" class="mt-4" title="没有匹配的账号或模型" description="调整筛选条件后结果会立即更新" surface="none" />
      <div v-else-if="filteredGroups.length" class="mt-4 flex min-w-0 flex-col gap-3" :aria-busy="state.loading.value">
        <AccountGroup v-for="account in pagedGroups" :key="account.accountId" :account="account" @open="state.openAccount" />
      </div>
      <p class="mt-3 mb-0 text-cp-xs text-cp-text-secondary">按账号整组分页，点击账号标题可收起或展开模型</p>
      <BaseTablePagination :pagination="pagination" :loading="state.loading.value" @page-change="page => currentPage = page" @page-size-change="size => { pageSize = size; currentPage = 1 }" />
      <p class="mt-2 mb-0 text-cp-xs text-cp-text-secondary">统计只覆盖本地保留窗口内的有界观测样本，不代表全部请求，过期不表示上游 State 失效</p>
    </BaseCard>

    <BaseCard padding="compact">
      <details>
        <summary class="cursor-pointer text-cp-sm font-bold">观测范围与诊断</summary>
        <div class="mt-3 flex flex-col gap-2 text-cp-sm text-cp-text-secondary">
          <p class="m-0">只记录业务响应中可见的 State 元数据，不主动请求或刷新 State</p>
          <p class="m-0">诊断计数覆盖当前进程已知的未归属、丢弃和存储失败事件，不是全量审计</p>
          <dl v-if="state.diagnostics.value" class="m-0 grid grid-cols-2 gap-2">
            <dt>未归属事件</dt><dd class="m-0 font-mono">{{ state.diagnostics.value.unattributed }}</dd>
            <dt>丢弃事件</dt><dd class="m-0 font-mono">{{ state.diagnostics.value.dropped }}</dd>
            <dt>存储失败</dt><dd class="m-0 font-mono">{{ state.diagnostics.value.storageFailures }}</dd>
          </dl>
          <p v-else class="m-0">完成首个概况页后显示诊断计数</p>
        </div>
      </details>
    </BaseCard>

    <BaseModal :model-value="state.modalOpen.value" :title="activeName" :description="activeName !== state.activeAccountId.value ? state.activeAccountId.value : undefined" size="xl" :dismissible="!state.saving.value" @update:model-value="open => { if (!open) state.requestClose() }">
      <div class="flex min-w-0 flex-col gap-4">
        <div v-if="state.detailError.value" role="alert" class="flex flex-wrap items-center gap-2 rounded-cp bg-cp-error-container p-3 text-cp-sm text-cp-error-on-container">
          <span>{{ state.detailError.value }}</span>
          <BaseButton size="sm" :loading="state.detailLoading.value" @click="state.refreshDetail(true)">重试</BaseButton>
        </div>
        <p v-if="state.detailLoading.value" role="status" class="m-0 text-cp-sm text-cp-text-secondary">正在读取账号观测与配置</p>
        <template v-if="state.snapshot.value">
          <ObservationTable :key="state.snapshot.value.accountId" :snapshot="state.snapshot.value" />
          <BaseCard title="账号配置" padding="compact">
            <form class="flex flex-col gap-4" @submit.prevent="state.save()">
              <fieldset :disabled="state.saving.value || state.detailLoading.value" class="m-0 flex min-w-0 flex-col gap-4 border-0 p-0">
                <label class="flex items-center gap-2 text-cp-sm font-bold"><input v-model="state.enabled.value" type="checkbox" class="size-4 accent-cp-primary">启用观测</label>
                <div class="flex flex-col gap-2">
                  <label for="models" class="text-cp-sm font-bold">观察模型</label><BaseInput id="models" v-model="state.models.value" aria-describedby="models-help" placeholder="留空观察所有模型" />
                  <p id="models-help" class="m-0 text-cp-xs text-cp-text-secondary">以逗号分隔实际发送模型名称</p>
                </div>
                <div class="flex flex-col gap-2">
                  <label for="rules" class="text-cp-sm font-bold">长度规则</label><BaseInput id="rules" v-model="state.lengthRules.value" aria-describedby="rules-help" placeholder="200,300,400-500" />
                  <p id="rules-help" class="m-0 text-cp-xs text-cp-text-secondary">正整数或闭区间，留空不限长度</p>
                </div>
                <div class="flex flex-col gap-2">
                  <label for="retention" class="text-cp-sm font-bold">保留时间（小时）</label><BaseInput id="retention" v-model="state.retention.value" type="number" min="1" max="168" step="1" required aria-describedby="retention-help" />
                  <p id="retention-help" class="m-0 text-cp-xs text-cp-text-secondary">1 至 168 小时，仅影响本地观测保留，不代表上游有效期</p>
                </div>
              </fieldset>
            </form>
          </BaseCard>
        </template>
      </div>
      <template #footer>
        <div v-if="state.conflict.value" role="alert" class="flex basis-full flex-col gap-2 rounded-cp bg-cp-warning-container p-3 text-cp-sm text-cp-warning-on-container">
          <span>配置已被其他页面修改，草稿仍保留，请载入最新配置后重新编辑</span>
          <BaseButton :disabled="state.detailLoading.value" @click="state.refreshDetail(true)">放弃草稿并载入最新配置</BaseButton>
        </div>
        <p v-if="state.formError.value" role="alert" class="m-0 basis-full text-cp-sm text-cp-error-text">{{ state.formError.value }}</p>
        <p v-if="state.notice.value" role="status" class="m-0 basis-full text-cp-sm text-cp-success-text">{{ state.notice.value }}</p>
        <BaseButton v-if="state.snapshot.value && state.dirty.value && !state.conflict.value" :disabled="state.saving.value || state.detailLoading.value" @click="state.setDraft(state.snapshot.value.settings, state.snapshot.value.version)">放弃修改</BaseButton>
        <BaseButton variant="primary" :loading="state.saving.value" :disabled="!state.snapshot.value || state.detailLoading.value || state.conflict.value || (!state.dirty.value && state.snapshot.value.version !== null)" @click="state.save()">保存配置</BaseButton>
        <BaseButton id="account-close" :disabled="state.saving.value" @click="state.requestClose()">关闭</BaseButton>
      </template>
    </BaseModal>

    <BaseConfirmModal v-model="state.confirmClose.value" title="放弃未保存的配置草稿" confirm-text="放弃并关闭" destructive @confirm="state.confirmDiscardClose()">
      当前账号配置有未保存修改，关闭后将丢失草稿
    </BaseConfirmModal>
  </main>
</template>
