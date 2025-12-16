<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { RouterLink, RouterView, useRoute } from 'vue-router'
import { storeToRefs } from 'pinia'
import { useModeStore } from './stores/modes'

const route = useRoute()
const modeStore = useModeStore()
const { modes, ideInstances, githubRules } = storeToRefs(modeStore)

const menuItems = [
  { label: '总览', description: '一键开始使用', to: '/' },
  { label: '库管理', description: '模式库 + 写回历史', to: '/library', badge: () => String(modes.value.length) },
  { label: 'GitHub 同步', description: '规则同步 + 记录追溯', to: '/github-sync', badge: () => String(githubRules.value.length) },
  { label: 'IDE 配置', description: '实例识别 + 勾选目标', to: '/ide', badge: () => String(ideInstances.value.length) },
  { label: '设置', description: '日志、备份与 GitHub', to: '/settings' }
]

const activePath = computed(() => {
  const sorted = [...menuItems].sort((a, b) => b.to.length - a.to.length)
  const match = sorted.find((item) => (item.to === '/' ? route.path === '/' : route.path.startsWith(item.to)))
  return match?.to ?? '/'
})

const activeMenuItem = computed(() => menuItems.find((item) => item.to === activePath.value) ?? menuItems[0])

onMounted(async () => {
  await Promise.all([modeStore.bootstrap(), modeStore.fetchAppSettings(), modeStore.fetchGithubSettings()]).catch(() => {
    /* 交由各页面自行展示错误 */
  })
  modeStore.scanKnownInstances().catch(() => {
    /* 首次扫描失败不阻塞启动 */
  })
})
</script>

<template>
  <div class="flex h-screen overflow-hidden bg-slate-950/5">
    <aside class="sticky top-0 h-screen w-72 shrink-0 overflow-y-auto border-r border-gray-200 bg-white/90 p-4 shadow-sm">
      <div>
        <p class="text-sm font-semibold text-gray-900">Kilo/Roo 自定义模式管家</p>
      </div>

      <nav class="mt-8 space-y-2">
        <RouterLink
          v-for="item in menuItems"
          :key="item.to"
          :to="item.to"
          class="group flex items-start justify-between gap-3 rounded-xl px-3 py-3 transition-all"
          :class="activePath === item.to ? 'bg-blue-50 text-blue-700 ring-1 ring-blue-100' : 'text-gray-700 hover:bg-gray-50'"
        >
          <span class="min-w-0">
            <p class="text-sm font-medium">{{ item.label }}</p>
            <p class="text-xs text-gray-400" :class="activePath === item.to ? 'text-blue-500' : ''">{{ item.description }}</p>
          </span>
          <span
            v-if="item.badge"
            class="mt-0.5 rounded-full bg-gray-100 px-2 py-0.5 text-[11px] text-gray-600 group-hover:bg-gray-200"
            :class="activePath === item.to ? 'bg-blue-100 text-blue-700 group-hover:bg-blue-100' : ''"
          >
            {{ item.badge() }}
          </span>
        </RouterLink>
      </nav>
    </aside>

    <main class="h-screen min-w-0 flex-1 overflow-y-auto">
      <header class="sticky top-0 z-10 border-b border-gray-200 bg-white/80 px-4 py-4 backdrop-blur">
        <p class="text-xs uppercase tracking-widest text-gray-400">Kilo/Roo 模式管理</p>
        <h1 class="text-2xl font-semibold text-gray-900">
          {{ activeMenuItem.label }}
        </h1>
        <p class="text-sm text-gray-500">
          {{ activeMenuItem.description }}
        </p>
      </header>

      <section class="min-h-[calc(100vh-96px)] bg-gray-50/80">
        <RouterView />
      </section>
    </main>
  </div>
</template>
