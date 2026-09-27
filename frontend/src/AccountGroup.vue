<script setup lang="ts">
import { BaseButton, BaseTag } from '@codex-proxy/ui'
import type { OverviewAccount } from './api'
import { accountLabel, modelStatuses, statusNames } from './overviewGroups'

defineProps<{ account: OverviewAccount }>()
const emit = defineEmits<{ open: [accountId: string] }>()
const time = (ms: number) => new Date(ms).toLocaleString('zh-CN', { hour12: false })
</script>

<template>
  <details open class="min-w-0 rounded-cp bg-cp-fill-quaternary p-3">
    <summary class="cursor-pointer rounded-cp text-cp-sm focus-visible:outline-2 focus-visible:outline-cp-control-outline">
      <span class="ml-1 inline-flex max-w-full flex-wrap items-center gap-2 align-middle">
        <span class="min-w-0 break-all font-bold text-cp-text">{{ accountLabel(account) }}</span>
        <BaseTag size="sm">{{ account.models.length }} 个模型</BaseTag>
        <BaseTag v-if="account.error" size="sm" type="danger">读取失败</BaseTag>
        <BaseTag v-else-if="!account.models.length" size="sm">未观测</BaseTag>
        <BaseTag v-if="!account.enabled" size="sm">账号停用</BaseTag>
        <BaseTag v-if="account.observationEnabled === false" size="sm">观测停用</BaseTag>
      </span>
      <span v-if="account.accountName.trim()" class="mt-1 ml-5 block break-all font-mono text-cp-xs text-cp-text-secondary">{{ account.accountId }}</span>
    </summary>
    <div class="mt-3 flex min-w-0 flex-col gap-3">
      <div class="flex justify-end">
        <BaseButton size="sm" @click="emit('open', account.accountId)">详情与配置</BaseButton>
      </div>
      <p v-if="account.error" class="m-0 text-cp-sm text-cp-error-text">观测数据读取失败，可重新加载或打开详情重试</p>
      <p v-else-if="!account.models.length" class="m-0 text-cp-sm text-cp-text-secondary">该账号尚未观测到 State</p>
      <div v-else class="min-w-0 overflow-x-auto rounded-cp focus-visible:outline-2 focus-visible:outline-cp-control-outline" tabindex="0" role="region" :aria-label="`${accountLabel(account)}的模型观测，可横向滚动`">
        <table class="w-full border-collapse whitespace-nowrap text-left text-cp-sm">
          <thead class="bg-cp-fill-tertiary text-cp-text-secondary"><tr>
            <th scope="col" class="px-3 py-2">模型</th>
            <th scope="col" class="px-3 py-2">State 长度</th>
            <th scope="col" class="px-3 py-2">观测状态</th>
            <th scope="col" class="px-3 py-2">保留样本</th>
            <th scope="col" class="px-3 py-2">最后观测</th>
          </tr></thead>
          <tbody>
            <tr v-for="model in account.models" :key="model.model" class="even:bg-cp-fill-tertiary">
              <th scope="row" class="px-3 py-2 font-mono font-medium">{{ model.model }}</th>
              <td class="px-3 py-2 font-mono">{{ model.latestLength }}</td>
              <td class="px-3 py-2">
                <div class="flex flex-wrap gap-1">
                  <BaseTag v-for="status in modelStatuses(model)" :key="status" size="sm" :type="status === 'mismatch' || status === 'expired' ? 'warning' : 'neutral'">{{ statusNames[status] }}</BaseTag>
                </div>
              </td>
              <td class="px-3 py-2 font-mono">{{ model.observations }}</td>
              <td class="px-3 py-2">{{ time(model.lastObservedAtMs) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </details>
</template>
