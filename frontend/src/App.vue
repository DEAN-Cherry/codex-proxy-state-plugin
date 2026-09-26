<script setup lang="ts">
import { BaseButton, BaseCard, BaseInput } from '@codex-proxy/ui'
import ObservationTable from './ObservationTable.vue'
import { useState } from './useState'
const { accounts, cursor, selected, snapshot, loading, paging, saving, error, accountError, formError, notice, conflict, enabled, models, lengthRules, retention, dirty, setDraft, loadAccounts, refresh, selectAccount, save } = useState()
</script>

<template>
  <main class="flex min-w-0 flex-col gap-4 font-sans text-cp-text">
    <BaseCard padding="compact">
      <div class="flex flex-wrap items-end gap-3">
        <div class="flex min-w-0 flex-1 basis-64 flex-col gap-2">
          <label for="account" class="text-cp-sm font-bold">账号 ID</label>
          <select id="account" :value="selected" :disabled="dirty || saving || loading || !accounts.length"
            class="h-cp-control w-full min-w-0 rounded-cp border-0 bg-[var(--cp-input-bg)] px-3 font-mono text-cp text-cp-text shadow-cp-input focus-visible:outline-2 focus-visible:outline-cp-control-outline"
            @change="selectAccount">
            <option v-if="!accounts.length" value="">{{ paging ? '正在加载账号' : '暂无可用账号' }}</option>
            <option v-for="account in accounts" :key="account.accountId" :value="account.accountId">{{ account.accountId }}</option>
          </select>
        </div>
        <BaseButton v-if="cursor" :loading="paging" @click="loadAccounts">加载更多账号</BaseButton>
        <BaseButton :loading="loading" :disabled="!selected || saving" @click="refresh()">刷新</BaseButton>
      </div>
      <p v-if="dirty" class="mt-2 mb-0 text-cp-sm text-cp-text-secondary">切换账号前请保存或放弃修改，刷新观测不会覆盖草稿</p>
      <div v-if="accountError" role="alert" class="mt-3 flex flex-wrap items-center gap-2 text-cp-error-text">
        <span>{{ accountError }}</span><BaseButton size="sm" :loading="paging" @click="loadAccounts">重试账号列表</BaseButton>
      </div>
    </BaseCard>
    <div v-if="error" role="alert" class="rounded-cp bg-cp-error-container p-3 text-cp-error-on-container">{{ error }}</div>
    <p v-if="loading" role="status" class="m-0 text-cp-sm text-cp-text-secondary">正在读取观测与配置</p>
    <p v-if="!paging && !accounts.length && !accountError" class="m-0 p-4 text-cp-text-secondary">暂无可访问的 OpenAI 账号</p>
    <div v-if="snapshot" class="grid min-w-0 items-start gap-4 xl:grid-cols-[minmax(0,1fr)_20rem]" :aria-busy="loading">
      <ObservationTable :key="snapshot.accountId" :snapshot="snapshot" />
      <BaseCard title="账号配置" padding="compact">
        <form class="flex flex-col gap-4" @submit.prevent="save">
          <fieldset :disabled="saving || loading" class="m-0 flex min-w-0 flex-col gap-4 border-0 p-0">
            <label class="flex items-center gap-2 text-cp-sm font-bold"><input v-model="enabled" type="checkbox" class="size-4 accent-cp-primary">启用观测</label>
            <div class="flex flex-col gap-2">
              <label for="models" class="text-cp-sm font-bold">观察模型</label><BaseInput id="models" v-model="models" aria-describedby="models-help" placeholder="留空观察所有模型" />
              <p id="models-help" class="m-0 text-cp-xs text-cp-text-secondary">以逗号分隔实际发送模型名称</p>
            </div>
            <div class="flex flex-col gap-2">
              <label for="rules" class="text-cp-sm font-bold">长度规则</label><BaseInput id="rules" v-model="lengthRules" aria-describedby="rules-help" placeholder="200,300,400-500" />
              <p id="rules-help" class="m-0 text-cp-xs text-cp-text-secondary">正整数或闭区间，留空不限长度</p>
            </div>
            <div class="flex flex-col gap-2">
              <label for="retention" class="text-cp-sm font-bold">保留时间（小时）</label><BaseInput id="retention" v-model="retention" type="number" min="1" max="168" step="1" required aria-describedby="retention-help" />
              <p id="retention-help" class="m-0 text-cp-xs text-cp-text-secondary">1 至 168 小时，仅影响本地观测保留，不代表上游有效期</p>
            </div>
          </fieldset>
          <div v-if="conflict" role="alert" class="flex flex-col gap-3 rounded-cp bg-cp-warning-container p-3 text-cp-sm text-cp-warning-on-container">
            <span>配置已被其他页面修改，草稿仍保留，请载入最新配置后重新编辑</span><BaseButton :disabled="loading" @click="refresh(true)">放弃草稿并载入最新配置</BaseButton>
          </div>
          <p v-if="formError" role="alert" class="m-0 text-cp-sm text-cp-error-text">{{ formError }}</p>
          <p v-if="notice" role="status" class="m-0 text-cp-sm text-cp-success-text">{{ notice }}</p>
          <div class="flex flex-wrap gap-2">
            <BaseButton type="submit" variant="primary" :loading="saving" :disabled="loading || conflict || (!dirty && snapshot.version !== null)">保存配置</BaseButton>
            <BaseButton v-if="dirty && !conflict" :disabled="saving || loading" @click="setDraft(snapshot.settings, snapshot.version)">放弃修改</BaseButton>
          </div>
        </form>
      </BaseCard>
    </div>
  </main>
</template>
