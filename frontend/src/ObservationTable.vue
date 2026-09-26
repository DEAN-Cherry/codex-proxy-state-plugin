<script setup lang="ts">
import { computed, ref } from 'vue'
import { BaseCard, BaseInput, BaseTag } from '@codex-proxy/ui'
import type { Snapshot } from './api'
import ModelDetails from './ModelDetails.vue'
const props = defineProps<{ snapshot: Snapshot }>()
const filter = ref('')
const activeModel = ref('')
const rows = computed(() => props.snapshot.models.filter(row => row.model.toLowerCase().includes(filter.value.toLowerCase())))
const detail = computed(() => rows.value.find(row => row.model === activeModel.value))
const ruleNames = { matched: '匹配', mismatch: '不匹配', unrestricted: '未限制' }
const time = (ms: number) => new Date(ms).toLocaleString('zh-CN', { hour12: false })
</script>

<template>
  <div class="flex min-w-0 flex-col gap-4">
    <BaseCard title="实际发送模型" padding="compact">
      <template #actions><BaseTag>{{ snapshot.models.length }} 个模型</BaseTag></template>
      <label for="filter" class="mb-2 text-cp-sm font-bold">筛选模型</label>
      <BaseInput id="filter" v-model="filter" placeholder="输入模型名称" type="search" />
      <div v-if="rows.length" class="mt-4 overflow-x-auto rounded-cp focus-visible:outline-2 focus-visible:outline-cp-control-outline" tabindex="0" role="region" aria-label="模型观测表，可横向滚动">
        <table class="w-full border-collapse whitespace-nowrap text-left text-cp-sm">
          <thead class="bg-cp-fill-tertiary text-cp-text-secondary"><tr>
            <th scope="col" class="px-3 py-2">模型</th><th scope="col" class="px-3 py-2">最新长度</th>
            <th scope="col" class="px-3 py-2">长度规则</th><th scope="col" class="px-3 py-2">保留样本</th>
            <th scope="col" class="px-3 py-2">最后观测</th><th scope="col" class="px-3 py-2">本地保留</th>
          </tr></thead>
          <tbody><tr v-for="row in rows" :key="row.model" :class="activeModel === row.model ? 'bg-cp-primary-container' : 'even:bg-cp-fill-quaternary hover:bg-cp-fill-tertiary'">
            <th scope="row" class="min-w-32 px-3 py-2">
              <button type="button" class="max-w-64 cursor-pointer whitespace-normal break-all rounded-cp border-0 bg-transparent p-0 text-left font-mono text-cp-primary-text underline-offset-4 hover:underline focus-visible:outline-2 focus-visible:outline-cp-control-outline" :aria-expanded="activeModel === row.model" :aria-controls="detail ? 'model-details' : undefined" @click="activeModel = activeModel === row.model ? '' : row.model">{{ row.model }}</button>
            </th>
            <td class="px-3 py-2 font-mono">{{ row.latestLength }}</td>
            <td class="px-3 py-2"><BaseTag :type="row.validation === 'mismatch' ? 'warning' : 'neutral'">{{ ruleNames[row.validation] }}</BaseTag></td>
            <td class="px-3 py-2 font-mono">{{ row.observations }}</td><td class="px-3 py-2">{{ time(row.lastObservedAtMs) }}</td>
            <td class="px-3 py-2"><BaseTag>{{ row.expired ? '已过期' : '保留中' }}</BaseTag></td>
          </tr></tbody>
        </table>
      </div>
      <p v-else class="my-4 text-cp-text-secondary">{{ snapshot.models.length ? '没有匹配的模型' : '未观测到 State，业务响应出现可见元数据后记录将在此显示' }}</p>
      <p class="mt-3 mb-0 text-cp-xs text-cp-text-secondary">每账号最多保留 16 个模型，每模型统计保留窗口内最近 12 条有界观测，不是全量审计</p>
      <p class="mt-2 mb-0 text-cp-xs text-cp-text-secondary">数据截至 {{ time(snapshot.nowMs) }}，点击模型查看详情</p>
    </BaseCard>
    <ModelDetails v-if="detail" id="model-details" :model="detail" />
    <BaseCard padding="compact">
      <details>
        <summary class="cursor-pointer text-cp-sm font-bold">观测范围与诊断</summary>
        <div class="mt-3 flex flex-col gap-2 text-cp-sm text-cp-text-secondary">
          <p class="m-0">只记录业务响应中可见的 State 元数据，不主动请求或刷新 State</p>
          <p class="m-0">宿主会过滤部分上游 response.metadata 事件，缺少响应头或元数据仅表示未观测，不表示账号健康或异常</p>
          <p class="m-0">响应头可能来自复用的 WebSocket 连接，WebSocket 元数据表示可见 JSON 帧，均不证明刚发生握手</p>
          <p class="m-0">长度匹配仅是配置规则检查，摘要不代表模型能力，本地过期不代表上游 State 失效</p>
          <dl class="m-0 grid grid-cols-2 gap-2">
            <dt>未归属事件</dt><dd class="m-0 font-mono">{{ snapshot.diagnostics.unattributed }}</dd>
            <dt>丢弃事件</dt><dd class="m-0 font-mono">{{ snapshot.diagnostics.dropped }}</dd>
            <dt>存储失败</dt><dd class="m-0 font-mono">{{ snapshot.diagnostics.storageFailures }}</dd>
          </dl>
        </div>
      </details>
    </BaseCard>
  </div>
</template>
