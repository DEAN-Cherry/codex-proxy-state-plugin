<script setup lang="ts">
import { BaseCard, BaseTag } from '@codex-proxy/ui'
import type { Model } from './api'
defineProps<{ model: Model }>()
const sourceNames = { http_header: '响应头', sse_metadata: 'SSE 元数据', websocket_metadata: 'WebSocket 元数据' }
const ruleNames = { matched: '匹配', mismatch: '不匹配', unrestricted: '未限制' }
const time = (ms: number) => new Date(ms).toLocaleString('zh-CN', { hour12: false })
</script>

<template>
  <BaseCard title="模型详情" padding="compact">
    <p class="mt-0 mb-3 break-all font-mono text-cp-sm">{{ model.model }}</p>
    <dl class="m-0 grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-2 text-cp-sm">
      <dt class="text-cp-text-secondary">最新摘要</dt><dd class="m-0 break-all font-mono">{{ model.latestFingerprint }}</dd>
      <dt class="text-cp-text-secondary">来源</dt><dd class="m-0">{{ sourceNames[model.source] }}</dd>
      <dt class="text-cp-text-secondary">本地过期时间</dt><dd class="m-0">{{ time(model.expiresAtMs) }}</dd>
      <dt class="text-cp-text-secondary">样本检查</dt><dd class="m-0">匹配 {{ model.matched }} · 不匹配 {{ model.mismatched }} · 未限制 {{ model.observations - model.matched - model.mismatched }}</dd>
    </dl>
    <p v-if="model.expired" class="mt-3 mb-0 text-cp-sm text-cp-text-secondary">保留窗口已过，仅保留最新摘要，不计入窗口内统计</p>
    <h3 class="mt-4 mb-2 text-cp-sm font-bold">长度分布</h3>
    <div v-if="model.lengths.length" class="flex flex-wrap gap-2"><BaseTag v-for="item in model.lengths" :key="item.length">{{ item.length }} 长度 × {{ item.count }}</BaseTag></div>
    <p v-else class="m-0 text-cp-sm text-cp-text-secondary">保留窗口内暂无长度样本</p>
    <h3 class="mt-4 mb-2 text-cp-sm font-bold">最近历史</h3>
    <div v-if="model.history.length" class="overflow-x-auto rounded-cp focus-visible:outline-2 focus-visible:outline-cp-control-outline" tabindex="0" role="region" aria-label="最近观测历史，可横向滚动">
      <table class="w-full border-collapse whitespace-nowrap text-left text-cp-sm">
        <thead class="bg-cp-fill-tertiary text-cp-text-secondary"><tr><th scope="col" class="px-3 py-2">时间</th><th scope="col" class="px-3 py-2">长度</th><th scope="col" class="px-3 py-2">摘要</th><th scope="col" class="px-3 py-2">来源</th><th scope="col" class="px-3 py-2">规则</th></tr></thead>
        <tbody><tr v-for="(item, index) in model.history" :key="index" class="even:bg-cp-fill-quaternary"><td class="px-3 py-2">{{ time(item.observedAtMs) }}</td><td class="px-3 py-2 font-mono">{{ item.length }}</td><td class="px-3 py-2 font-mono">{{ item.fingerprint }}</td><td class="px-3 py-2">{{ sourceNames[item.source] }}</td><td class="px-3 py-2">{{ ruleNames[item.validation] }}</td></tr></tbody>
      </table>
    </div>
    <p v-else class="m-0 text-cp-sm text-cp-text-secondary">保留窗口内暂无历史</p>
  </BaseCard>
</template>
