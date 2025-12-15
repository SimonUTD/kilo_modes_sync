<script setup lang="ts">
const overviewCards = [
  { label: '本地模式', value: 140, tip: '去重后模式数量' },
  { label: 'GitHub 规则', value: 6, tip: '启用的规则数' },
  { label: 'IDE 实例', value: 8, tip: '已识别实例总计' },
  { label: '待同步差异', value: 5, tip: '跨实例缺失的模式数' }
]

const workflow = [
  {
    title: '入库标准化',
    desc: '所有数据先写入 SQLite，并记录来源与内容哈希，便于追踪差异'
  },
  {
    title: '本地&IDE 对比',
    desc: '支持对比 KiloCode / RooCode 之间的模式差异，必要时重命名或覆盖'
  },
  {
    title: 'GitHub 回流',
    desc: '根据规则批量抓取模式，字段缺失会自动跳过且记录日志'
  }
]

const upcomingTasks = [
  { title: '接入 SQLite Repository 层', detail: '实现模式库的增删改查与内容哈希' },
  { title: '封装 GitHub 同步命令', detail: '在 Tauri 端调用 octocrab 并应用延时 + 代理设置' },
  { title: '实现 IDE 模式写回', detail: '支持 “仅更新 IDE / 同步到本地库” 双向处理' }
]
</script>

<template>
  <div class="space-y-6 p-6">
    <section class="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
      <article
        v-for="card in overviewCards"
        :key="card.label"
        class="rounded-lg border border-gray-200 bg-white p-5 shadow-sm"
      >
        <p class="text-xs uppercase tracking-wide text-gray-400">{{ card.label }}</p>
        <p class="mt-2 text-4xl font-semibold text-gray-900">{{ card.value }}</p>
        <p class="text-xs text-gray-500">{{ card.tip }}</p>
      </article>
    </section>

    <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
      <header class="mb-4">
        <h2 class="text-lg font-semibold text-gray-900">数据流概览</h2>
        <p class="text-sm text-gray-500">遵循“抓取 → 入库 → 对比 → 同步”的闭环流程</p>
      </header>
      <div class="grid gap-4 md:grid-cols-3">
        <article
          v-for="item in workflow"
          :key="item.title"
          class="rounded-lg border border-dashed border-blue-200 bg-blue-50/50 p-4"
        >
          <h3 class="text-base font-semibold text-gray-900">{{ item.title }}</h3>
          <p class="mt-2 text-sm text-gray-600">{{ item.desc }}</p>
        </article>
      </div>
    </section>

    <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
      <header class="mb-4 flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold text-gray-900">下一步计划</h2>
          <p class="text-sm text-gray-500">根据需求文档排定的首批实现任务</p>
        </div>
        <button class="rounded-md border border-gray-200 px-3 py-2 text-sm text-gray-700">
          查看全部任务
        </button>
      </header>
      <ul class="space-y-3">
        <li
          v-for="task in upcomingTasks"
          :key="task.title"
          class="rounded-lg border border-gray-100 bg-gray-50/80 p-4"
        >
          <p class="text-sm font-semibold text-gray-900">{{ task.title }}</p>
          <p class="text-sm text-gray-600">{{ task.detail }}</p>
        </li>
      </ul>
    </section>
  </div>
</template>
