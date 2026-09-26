import { createApp } from 'vue'
import App from './App.vue'
import '@codex-proxy/ui/styles.css'
import './style.css'

// The host owns theme token injection and live theme updates, as in the official sample
createApp(App).mount('#app')
