import { createApp, watch } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import './styles/base.css'
import { useAppStore } from '@/stores/app'
import { applyAll, applyTheme, watchSystemTheme } from '@/lib/theme'

const app = createApp(App)
const pinia = createPinia()
app.use(pinia)
app.mount('#app')

const store = useAppStore(pinia)

async function boot() {
  // Theme first so the frame is painted in the right colours even if the backend is slow.
  applyTheme(store.settings?.appearance.theme ?? 'system')
  try {
    await store.init()
  } catch (e) {
    // `npm run dev:web` opens the UI in a plain browser, where invoke has no backend.
    store.flash(`后端未就绪：${String(e)}`)
    return
  }
  if (store.settings) applyAll(store.settings)
  watch(
    () => store.settings,
    (settings) => {
      if (settings) applyAll(settings)
    },
    { deep: true },
  )
}

const stopThemeWatch = watchSystemTheme(() => applyTheme(store.settings?.appearance.theme ?? 'system'))
window.addEventListener('beforeunload', stopThemeWatch)

void boot()
