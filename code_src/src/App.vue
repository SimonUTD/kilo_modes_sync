<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, RouterView, useRoute } from 'vue-router'

const route = useRoute()

const menuItems = [
  { label: '总览', description: '查看本地库概况', to: '/' },
  { label: '库管理', description: '维护模式仓库', to: '/library' },
  { label: 'GitHub 同步', description: '搜索并回流模式', to: '/github-sync' },
  { label: 'IDE 配置', description: '识别本地实例并同步', to: '/ide' },
  { label: '设置', description: 'GitHub Key、代理与日志', to: '/settings' }
]

const activePath = computed(() => {
  const match = menuItems.find((item) => route.path.startsWith(item.to))
  return match?.to ?? '/'
})
</script>

<template>
  <div class="flex min-h-screen bg-gray-800/5">
    <aside class="hidden w-64 flex-col border-r border-gray-200 bg-white/90 p-4 shadow-sm lg:flex">
      <div>
        <p class="text-sm font-semibold text-gray-900">Kilo & Roo 模式同步</p>
        <p class="text-xs text-gray-400">统一管理多实例配置</p>
      </div>

      <nav class="mt-8 space-y-2">
        <RouterLink
          v-for="item in menuItems"
          :key="item.to"
          :to="item.to"
          class="block rounded-lg px-3 py-3 transition-all"
          :class="activePath === item.to ? 'bg-blue-50 text-blue-600 shadow-inner' : 'text-gray-600 hover:bg-gray-50'"
        >
          <p class="text-sm font-medium">{{ item.label }}</p>
          <p class="text-xs text-gray-400" :class="activePath === item.to ? 'text-blue-400' : ''">{{ item.description }}</p>
        </RouterLink>
      </nav>
    </aside>

    <main class="flex-1">
      <header class="sticky top-0 z-10 border-b border-gray-200 bg-white/80 px-4 py-4 backdrop-blur">
        <p class="text-xs uppercase tracking-widest text-gray-400">当前视图</p>
        <h1 class="text-2xl font-semibold text-gray-900">
          {{ menuItems.find((item) => item.to === activePath)?.label || '总览' }}
        </h1>
        <p class="text-sm text-gray-500">
          {{ menuItems.find((item) => item.to === activePath)?.description }}
        </p>
      </header>

      <section class="min-h-[calc(100vh-96px)] bg-gray-50/80">
        <RouterView />
      </section>
    </main>
  </div>
</template>
