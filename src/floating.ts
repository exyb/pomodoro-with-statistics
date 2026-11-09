import { createApp, watch } from 'vue'
import { createPinia } from 'pinia'
import FloatingApp from './FloatingApp.vue'
import './styles/base.css'
import { useAppStore } from '@/stores/app'
import { applyAll, applyScale, applyTheme } from '@/lib/theme'

const app = createApp(FloatingApp)
const pinia = createPinia()
app.use(pinia)
app.mount('#app')

const store = useAppStore(pinia)

async function boot() {
  applyTheme('system')
  try {
    await store.init()
  } catch (e) {
    store.flash(`悬浮窗无法连接主进程：${String(e)}`)
    return
  }
  const sync = () => {
    const settings = store.settings
    if (!settings) return
    applyAll(settings)
    // The mini window has its own size knob; it wins over the main window's scale.
    applyScale(settings.floating.scale)
  }
  sync()
  watch(() => store.settings, sync, { deep: true })
}

void boot()
