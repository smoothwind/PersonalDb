import { createApp } from 'vue'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'
import './styles/tokens.css'
import './styles/overrides.css'
import App from './App.vue'
import { i18n } from './i18n'
import { initLogging, logger } from './utils/logger'

// 启动性能标记：mount 完成后输出首帧耗时（>800ms 提示 INFO，便于回归观察）
const T0 = performance.now()

const app = createApp(App)
app.use(createPinia())
app.use(i18n)
// EP 全量注册；组件文案由 App.vue 的 el-config-provider 按当前语言动态绑定
app.use(ElementPlus)

// 日志：console 转发 + 全局错误捕获（写入日志文件，便于调试排查）
void initLogging()
app.config.errorHandler = (err, _instance, info) => {
  logger.error(`vue errorHandler [${info}]: ${String(err)}`)
}

// 移除 WebView 默认右键菜单（应用内使用各组件自定义右键菜单）
document.addEventListener('contextmenu', (e) => e.preventDefault())

app.mount('#app')

requestAnimationFrame(() => {
  const ms = Math.round(performance.now() - T0)
  logger.info(`[perf] first frame in ${ms}ms (mount)`)
})
