<script setup lang="ts">
import { computed, ref } from 'vue'
import { BaseCard, BaseInput, BaseTag } from '@codex-proxy/ui'
import type { Snapshot } from './api'
import ModelDetails from './ModelDetails.vue'
const props = defineProps<{ snapshot: Snapshot }>()
const filter = ref('')
const activeModel = ref('')
const rows = computed(() => props.snapshot.models.filter(row => row.model.toLowerCase().includes(filter.value.toLowerCase())))
const sections = computed(() => [
  { key: 'active', title: '有效 State', rows: rows.value.filter(row => !row.expired), empty: '没有保留窗口内的有效 State' },
  { key: 'history', title: '历史 State', rows: rows.value.filter(row => row.expired), empty: '' },
])
const historyCount = computed(() => props.snapshot.models.filter(row => row.expired).length)
const detail = computed(() => rows.value.find(row => row.model === activeModel.value))
const ruleNames = { matched: '匹配', mismatch: '不匹配', unrestricted: '未限制' }
const time = (ms: number) => new Date(ms).toLocaleString('zh-CN', { hour12: false })
</script>

<template>
  <div class="flex min-w-0 flex-col gap-4">
    <BaseCard title="实际发送模型" padding="compact">
      <template #actions>
        <div class="flex flex-wrap gap-2">
          <BaseTag>{{ snapshot.models.length - historyCount }} 个有效</BaseTag>
          <BaseTag v-if="historyCount">{{ historyCount }} 个历史</BaseTag>
        </div>
      </template>
      <label for="filter" class="mb-2 text-cp-sm font-bold">筛选模型</label>
      <BaseInput id="filter" v-model="filter" placeholder="输入模型名称" type="search" />
      <template v-if="rows.length">
        <template v-for="section in sections" :key="section.key">
          <h3 v-if="section.rows.length || section.empty" class="mt-4 mb-2 text-cp-sm font-bold">{{ section.title }}<span v-if="section.key === 'history'" class="ml-2 font-normal text-cp-xs text-cp-text-secondary">保留窗口已过，仅保留最新摘要</span></h3>
          <div v-if="section.rows.length" class="overflow-x-auto rounded-cp focus-visible:outline-2 focus-visible:outline-cp-control-outline" tabindex="0" role="region" :aria-label="`${section.title}模型观测表，可横向滚动`">
            <table class="w-full border-collapse whitespace-nowrap text-left text-cp-sm">
              <thead class="bg-cp-fill-tertiary text-cp-text-secondary"><tr>
                <th scope="col" class="px-3 py-2">模型</th><th scope="col" class="px-3 py-2">最新长度</th>
                <th scope="col" class="px-3 py-2">长度规则</th><th scope="col" class="px-3 py-2">保留样本</th>
                <th scope="col" class="px-3 py-2">最后观测</th><th scope="col" class="px-3 py-2">{{ section.key === 'history' ? '过期于' : '保留至' }}</th>
              </tr></thead>
              <tbody><tr v-for="row in section.rows" :key="row.model" :class="activeModel === row.model ? 'bg-cp-primary-container' : 'even:bg-cp-fill-quaternary hover:bg-cp-fill-tertiary'">
                <th scope="row" class="min-w-32 px-3 py-2">
                  <button type="button" class="max-w-64 cursor-pointer whitespace-normal break-all rounded-cp border-0 bg-transparent p-0 text-left font-mono text-cp-primary-text underline-offset-4 hover:underline focus-visible:outline-2 focus-visible:outline-cp-control-outline" :aria-expanded="activeModel === row.model" :aria-controls="detail ? 'model-details' : undefined" @click="activeModel = activeModel === row.model ? '' : row.model">{{ row.model }}</button>
                </th>
                <td class="px-3 py-2 font-mono">{{ row.latestLength }}</td>
                <td class="px-3 py-2"><BaseTag :type="row.validation === 'mismatch' ? 'warning' : 'neutral'">{{ ruleNames[row.validation] }}</BaseTag></td>
                <td class="px-3 py-2 font-mono">{{ row.observations }}</td><td class="px-3 py-2">{{ time(row.lastObservedAtMs) }}</td>
                <td class="px-3 py-2">{{ time(row.expiresAtMs) }}</td>
              </tr></tbody>
            </table>
          </div>
          <p v-else-if="section.empty" class="m-0 text-cp-sm text-cp-text-secondary">{{ section.empty }}</p>
        </template>
      </template>
      <p v-else class="my-4 text-cp-text-secondary">{{ snapshot.models.length ? '没有匹配的模型' : '未观测到 State，业务响应出现可见元数据后记录将在此显示' }}</p>
      <p class="mt-3 mb-0 text-cp-xs text-cp-text-secondary">每账号最多保留 16 个模型，每模型统计保留窗口内最近 12 条有界观测，不是全量审计</p>
      <p class="mt-2 mb-0 text-cp-xs text-cp-text-secondary">数据截至 {{ time(snapshot.nowMs) }}，点击模型查看长度分布和历史</p>
    </BaseCard>
    <ModelDetails v-if="detail" id="model-details" :model="detail" />
  </div>
</template>
