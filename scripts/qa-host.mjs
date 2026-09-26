// 本地 QA 入口不参与插件打包，页面调用真实插件进程，宿主账号及存储为测试替身。
import { fileURLToPath } from 'node:url'
import { PluginPeer } from './qa-peer.mjs'

const { resolveTheme, DEFAULT_THEME_COLOR, DEFAULT_CUSTOM_THEME_COLOR } = await import(
  new URL('../frontend/node_modules/@codex-proxy/ui/dist/theme/index.js', import.meta.url).href
)
const css = mode => Object.entries(resolveTheme(mode, DEFAULT_THEME_COLOR, DEFAULT_CUSTOM_THEME_COLOR).tokens)
  .map(([key, value]) => `${key}:${value}`).join(';')
const themeCss = `:root{${css('light')}}:root[data-theme=dark]{${css('dark')};color-scheme:dark}body{padding:16px;box-sizing:border-box}`
const manifest = await Bun.file(new URL('../plugin.json', import.meta.url)).json()
const peer = await new PluginPeer(manifest).start()
const firstAccount = peer.accounts[0]
await peer.api('POST', 'api/settings', {
  accountId: firstAccount, expectedVersion: null,
  settings: { enabled: true, models: [], lengthRules: '292,400-500', retentionHours: 24 },
})
for (const [model, lengths] of [
  ['gpt-6-astra', [292, 312, 292, 480]],
  ['gpt-6-sol', [400, 450, 500]],
  ['gpt-6-luna', [312, 780]],
]) {
  for (const length of lengths) await peer.observe(firstAccount, model, length)
}
await peer.observe(peer.accounts[1], 'gpt-6-astra', 780)

const bridge = `
window.codexProxyPlugin = {
  version: 2, theme: 'light',
  request: async input => {
    const response = await fetch('/qa/api', {method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(input)});
    const data = await response.json();
    return {status:data.status, contentType:'application/json', body:new TextEncoder().encode(JSON.stringify(data.value)).buffer};
  },
  resourceUrl: path => '/'+path.replace(/^web\\//,'')
};
window.qaSetTheme = theme => {
  window.codexProxyPlugin.theme = theme;
  document.documentElement.dataset.theme = theme;
  window.dispatchEvent(new CustomEvent('codex-proxy-themechange', {detail:{theme}}));
};`

const server = Bun.serve({
  hostname: '127.0.0.1', port: Number(process.env.QA_PORT ?? 4178),
  async fetch(request) {
    const url = new URL(request.url)
    if (url.pathname === '/qa/api' && request.method === 'POST') {
      try {
        const input = await request.json()
        const result = await peer.api(input.method, input.path,
          input.body ? JSON.parse(input.body) : undefined, input.query ?? '')
        return Response.json(result)
      } catch {
        return Response.json({ status: 502, value: { error: { code: 'qa_bridge', message: '测试宿主暂不可用' } } })
      }
    }
    if (url.pathname === '/qa/error') {
      peer.storageError = url.searchParams.get('enabled') === 'true'
      return Response.json({ enabled: peer.storageError })
    }
    if (url.pathname === '/qa/expire') {
      for (const [key, record] of peer.states) {
        if (key.startsWith('observations:')) {
          for (const model of record.value.models)
            for (const sample of model.history) sample.observedAtMs -= 8 * 24 * 3_600_000
        }
      }
      return Response.json({ expired: true })
    }
    const path = url.pathname === '/' ? 'index.html' : url.pathname.slice(1)
    if (!['index.html', 'app.js', 'app.css'].includes(path)) return new Response('Not found', { status: 404 })
    const file = Bun.file(new URL(`../frontend/dist/${path}`, import.meta.url))
    if (path === 'index.html') {
      const html = (await file.text()).replace('<head>', `<head><style>${themeCss}</style><script>${bridge}</script>`)
      return new Response(html, { headers: { 'Content-Type': 'text/html; charset=utf-8' } })
    }
    return new Response(file, { headers: { 'Content-Type': path.endsWith('.js') ? 'text/javascript' : 'text/css' } })
  },
})
console.log(`QA_READY http://127.0.0.1:${server.port} binary=${fileURLToPath(new URL('../backend/target/debug/', import.meta.url))}`)
for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => { server.stop(true); peer.close(); process.exit(0) })
}
