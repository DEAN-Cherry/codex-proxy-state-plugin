import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'
import { defineConfig } from 'vite'

export default defineConfig(({ command }) => ({
  define: { 'process.env.NODE_ENV': JSON.stringify(command === 'serve' ? 'development' : 'production') },
  resolve: { dedupe: ['vue'] },
  plugins: [vue(), tailwindcss(), {
    name: 'plugin-page-entry',
    async generateBundle() {
      const template = await readFile(new URL('./index.html', import.meta.url), 'utf8')
      this.emitFile({ type: 'asset', fileName: 'index.html', source: template
        .replace('</head>', '<link rel="stylesheet" href="./app.css" /></head>')
        .replace('<script type="module" src="/src/main.ts"></script>', '<script src="./app.js" defer></script>') })
    },
  }],
  build: {
    outDir: 'dist', cssCodeSplit: false, sourcemap: false,
    lib: { entry: fileURLToPath(new URL('./src/main.ts', import.meta.url)), name: 'StateObserver', formats: ['iife'], fileName: () => 'app.js', cssFileName: 'app' },
  },
}))
