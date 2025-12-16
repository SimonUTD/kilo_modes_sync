<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { storeToRefs } from 'pinia'
import { useRouter } from 'vue-router'
import { useModeStore } from '../../stores/modes'
import { formatDateTime } from '../../composables/useFormat'

const router = useRouter()
const modeStore = useModeStore()
const { modes, ideInstances, githubRules, githubSettings, roleDefinitionThreshold } = storeToRefs(modeStore)

const toast = ref<string | null>(null)
const scanning = ref(false)

const modeStats = computed(() => {
  const total = modes.value.length
  const highQuality = modes.value.filter((item) => item.roleDefinitionLength >= roleDefinitionThreshold.value).length
  const local = modes.value.filter((item) => item.source === 'local').length
  const github = modes.value.filter((item) => item.source === 'github').length
  const ide = modes.value.filter((item) => item.source === 'ide').length
  return { total, highQuality, local, github, ide }
})

const instanceStats = computed(() => {
  const total = ideInstances.value.length
  const selected = ideInstances.value.filter((item) => item.selected).length
  const synced = ideInstances.value.filter((item) => item.status === 'synced').length
  const outdated = ideInstances.value.filter((item) => item.status === 'outdated').length
  const missing = ideInstances.value.filter((item) => item.status === 'missing').length
  const scanTimes = ideInstances.value.map((item) => item.lastScanAt).filter(Boolean).sort()
  const lastScanAt = scanTimes.length ? scanTimes[scanTimes.length - 1] : undefined
  return { total, selected, synced, outdated, missing, lastScanAt }
})

const githubRuleStats = computed(() => {
  const total = githubRules.value.length
  const enabled = githubRules.value.filter((item) => item.enabled).length
  const hasToken = Boolean(githubSettings.value?.token?.trim())
  return { total, enabled, hasToken }
})

const lastGithubResult = computed(() => githubSettings.value?.lastResult ?? null)

const quickHints = computed(() => {
  const hints: Array<{ title: string; desc: string; actionLabel: string; action: () => void }> = []
  if (!ideInstances.value.length) {
    hints.push({
      title: '先扫描 IDE 实例',
      desc: '自动发现本机多个 IDE 的 Kilo/Roo 配置文件，并把模式入库。',
      actionLabel: '去扫描',
      action: () => router.push('/ide')
    })
  } else if (!modes.value.length) {
    hints.push({
      title: '把模式入库',
      desc: '你已添加实例，但本地库还没有模式。先从实例扫描或从 GitHub/文本导入。',
      actionLabel: '模式管理',
      action: () => router.push('/library')
    })
  } else if (!githubRules.value.length) {
    hints.push({
      title: '配置 GitHub 搜索规则',
      desc: '用规则批量回流模式到本地库，并保留 raw 字段与同步记录。',
      actionLabel: '去配置',
      action: () => router.push('/github-sync')
    })
  }
  return hints
})

async function handleScanAllInstances() {
  scanning.value = true
  toast.value = '正在扫描实例并入库...'
  try {
    await modeStore.scanAllInstances()
    toast.value = '扫描完成，已写入本地库'
  } catch (err) {
    toast.value = err instanceof Error ? err.message : String(err)
  } finally {
    scanning.value = false
    setTimeout(() => (toast.value = null), 3500)
  }
}

onMounted(() => {
  Promise.all([modeStore.bootstrap(), modeStore.fetchAppSettings(), modeStore.fetchGithubSettings()]).catch(() => {
    /* 页面内按需展示 */
  })
})
</script>

<template>
  <div class="space-y-2 p-2">
    <section class="grid gap-4 md:grid-cols-3 xl:grid-cols-3">
      <article class="rounded-xl border border-gray-200 bg-white p-5 shadow-sm">
        <p class="text-xs uppercase tracking-wide text-gray-400">模式库总数</p>
        <p class="mt-2 text-4xl font-semibold text-gray-900">{{ modeStats.total }}</p>
        <p class="text-xs text-gray-500">来源：本软件 {{ modeStats.local }} / GitHub {{ modeStats.github }} / IDE {{ modeStats.ide }}</p>
      </article>
      <article class="rounded-xl border border-gray-200 bg-white p-5 shadow-sm">
        <p class="text-xs uppercase tracking-wide text-gray-400">高质量候选</p>
        <p class="mt-2 text-4xl font-semibold text-gray-900">{{ modeStats.highQuality }}</p>
        <p class="text-xs text-gray-500">规则：指令长度 ≥ {{ roleDefinitionThreshold }} </p>
      </article>
      <article class="rounded-xl border border-gray-200 bg-white p-5 shadow-sm">
        <p class="text-xs uppercase tracking-wide text-gray-400">IDE 实例</p>
        <p class="mt-2 text-4xl font-semibold text-gray-900">{{ instanceStats.total }}</p>
        <p class="text-xs text-gray-500">最近扫描：{{ formatDateTime(instanceStats.lastScanAt) }}</p>
      </article>
    </section>

    <section class="grid gap-4 lg:grid-cols-3">
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm lg:col-span-3">
        <header class="flex items-start justify-between gap-4">
          <div>
            <h2 class="text-lg font-semibold text-gray-900">快速开始</h2>
            <p class="text-sm text-gray-500">建议流程：先扫实例入库 → 再做差异与写回 → 最后用 GitHub 扩充模式</p>
          </div>
          <div class="flex flex-wrap gap-2">
            <button
              class="whitespace-nowrap rounded-md bg-blue-600 px-4 py-2 text-sm text-white disabled:cursor-not-allowed disabled:opacity-60"
              :disabled="scanning"
              @click="handleScanAllInstances"
            >
              扫描实例并入库
            </button>
            <button
              class="whitespace-nowrap rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700"
              @click="router.push('/library')"
            >
              模式管理
            </button>
          </div>
        </header>

        <div class="mt-5 grid gap-3 md:grid-cols-3">
          <button
            class="rounded-lg border border-gray-200 bg-gray-50 px-4 py-4 text-left hover:bg-gray-100"
            @click="router.push('/ide')"
          >
            <p class="text-sm font-semibold text-gray-900">1) 识别程序</p>
            <p class="mt-1 text-xs text-gray-500">扫描所有 Kilo Code / RooCode 配置文件</p>
          </button>
          <button
            class="rounded-lg border border-gray-200 bg-gray-50 px-4 py-4 text-left hover:bg-gray-100"
            @click="router.push('/library')"
          >
            <p class="text-sm font-semibold text-gray-900">2) 模式同步</p>
            <p class="mt-1 text-xs text-gray-500">对比偏差，管理各软件的可用模式</p>
          </button>
          <button
            class="rounded-lg border border-gray-200 bg-gray-50 px-4 py-4 text-left hover:bg-gray-100"
            @click="router.push('/github-sync')"
          >
            <p class="text-sm font-semibold text-gray-900">3) 模式采集</p>
            <p class="mt-1 text-xs text-gray-500">从GitHub搜集好用的模式</p>
          </button>
        </div>

        <div v-if="quickHints.length" class="mt-5 rounded-lg border border-yellow-100 bg-yellow-50 p-4">
          <p class="text-sm font-semibold text-yellow-900">建议你先做：</p>
          <ul class="mt-2 space-y-2">
            <li v-for="item in quickHints" :key="item.title" class="flex items-start justify-between gap-3">
              <div>
                <p class="text-sm text-yellow-900">{{ item.title }}</p>
                <p class="text-xs text-yellow-800/80">{{ item.desc }}</p>
              </div>
              <button class="shrink-0 rounded-md bg-yellow-900 px-3 py-2 text-xs text-yellow-50" @click="item.action">
                {{ item.actionLabel }}
              </button>
            </li>
          </ul>
        </div>

        <p v-if="toast" class="mt-4 text-sm text-blue-600">{{ toast }}</p>
      </div>

      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm lg:col-span-3">
        <header>
          <h2 class="text-lg font-semibold text-gray-900">GitHub 模式采集概况</h2>
          <p class="text-sm text-gray-500">检索规则：{{ githubRuleStats.total }}（GitHub Token：{{ githubRuleStats.hasToken ? '已配置' : '未配置' }}）</p>
        </header>

        <div v-if="lastGithubResult" class="mt-4 rounded-lg border border-gray-100 bg-gray-50 p-4">
          <p class="text-sm font-semibold text-gray-900">最近一次同步结果</p>
          <p class="mt-2 text-sm text-gray-700">抓取文件：{{ lastGithubResult.fetchedFiles }}</p>
          <p class="text-sm text-gray-700">写入模式：{{ lastGithubResult.savedModes }}</p>
          <p class="text-sm text-gray-700">缺字段跳过：{{ lastGithubResult.skippedDueToMissingFields }}</p>
          <p v-if="lastGithubResult.errors.length" class="mt-2 text-xs text-red-600">错误：{{ lastGithubResult.errors.length }} 条（详见 G模式采集页）</p>
        </div>
        <div v-else class="mt-4 rounded-lg border border-dashed border-gray-200 bg-white p-4 text-sm text-gray-500">
          暂无同步记录。可前往设置功能配置 Github 的 Token 与规则后执行同步。
        </div>

        <div class="mt-4 flex flex-wrap gap-2">
          <button class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white" @click="router.push('/github-sync')">
            去同步
          </button>
          <button class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700" @click="router.push('/settings')">
            去设置
          </button>
        </div>
      </div>
    </section>
  </div>
</template>
