<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { storeToRefs } from 'pinia'
import { useRouter } from 'vue-router'
import { useModeStore, type GithubRuleEntity, type SyncLogRecord } from '../../stores/modes'
import { formatDateTime } from '../../composables/useFormat'

const router = useRouter()
const modeStore = useModeStore()
const { githubRules, githubSettings } = storeToRefs(modeStore)

const DEFAULT_PATH_HINT = 'customModes[].slug'

const ruleDraft = reactive<Pick<GithubRuleEntity, 'name' | 'query' | 'pathHint' | 'branch' | 'enabled' | 'delaySec'>>({
  name: '',
  query: '',
  pathHint: DEFAULT_PATH_HINT,
  branch: 'main',
  enabled: true,
  delaySec: 3
})
const editingRuleId = ref<string | null>(null)
const showAdvanced = ref(false)

const syncingRuleId = ref<string | null>(null)

const ruleKeyword = ref('')
const statusMessage = ref('')
const lastSyncResult = computed(() => githubSettings.value?.lastResult ?? null)

const syncLogs = ref<SyncLogRecord[]>([])
const logsLoading = ref(false)
const logsError = ref<string | null>(null)
const syncingAll = ref(false)

const filteredRules = computed(() => {
  const keyword = ruleKeyword.value.trim().toLowerCase()
  if (!keyword) return githubRules.value
  return githubRules.value.filter((rule) => {
    return rule.name.toLowerCase().includes(keyword) || rule.query.toLowerCase().includes(keyword)
  })
})

function formatSyncLogStatus(status: string) {
  if (status === 'success') return '成功'
  if (status === 'warning') return '警告'
  if (status === 'error') return '错误'
  return status
}

function resetDraft() {
  ruleDraft.name = ''
  ruleDraft.query = ''
  ruleDraft.pathHint = DEFAULT_PATH_HINT
  ruleDraft.branch = 'main'
  ruleDraft.enabled = true
  ruleDraft.delaySec = githubSettings.value?.delaySec ?? 3
  editingRuleId.value = null
  showAdvanced.value = false
}

async function handleSaveRule() {
  if (!ruleDraft.name || !ruleDraft.query) {
    statusMessage.value = '请填写规则名称与搜索语句'
    return
  }

  const existing = editingRuleId.value ? githubRules.value.find((item) => item.id === editingRuleId.value) : null
  const payload: GithubRuleEntity = {
    id: editingRuleId.value ?? '',
    name: ruleDraft.name,
    query: ruleDraft.query,
    pathHint: ruleDraft.pathHint?.trim() ? ruleDraft.pathHint.trim() : DEFAULT_PATH_HINT,
    branch: ruleDraft.branch,
    enabled: ruleDraft.enabled,
    delaySec: ruleDraft.delaySec,
    lastRunAt: existing?.lastRunAt ?? null
  }

  try {
    await modeStore.saveGithubRule(payload)
    resetDraft()
    statusMessage.value = '规则已保存'
    setTimeout(() => (statusMessage.value = ''), 3000)
  } catch (err) {
    statusMessage.value = err instanceof Error ? err.message : String(err)
  }
}

function handleEditRule(rule: GithubRuleEntity) {
  editingRuleId.value = rule.id
  ruleDraft.name = rule.name
  ruleDraft.query = rule.query
  ruleDraft.pathHint = rule.pathHint || DEFAULT_PATH_HINT
  ruleDraft.branch = rule.branch || 'main'
  ruleDraft.enabled = rule.enabled
  ruleDraft.delaySec = rule.delaySec
  showAdvanced.value = false
  statusMessage.value = '已载入规则，可在下方编辑后保存'
  setTimeout(() => (statusMessage.value = ''), 3000)
}

async function handleToggleRule(rule: GithubRuleEntity) {
  statusMessage.value = '正在更新规则状态...'
  try {
    await modeStore.saveGithubRule({
      ...rule,
      enabled: !rule.enabled
    })
    statusMessage.value = '已更新规则状态'
  } catch (err) {
    statusMessage.value = err instanceof Error ? err.message : String(err)
  } finally {
    setTimeout(() => (statusMessage.value = ''), 3000)
  }
}

async function handleDeleteRule(rule: GithubRuleEntity) {
  const ok = window.confirm(`确认删除规则「${rule.name}」？`)
  if (!ok) return
  statusMessage.value = '正在删除规则...'
  try {
    await modeStore.deleteGithubRule(rule.id)
    if (editingRuleId.value === rule.id) {
      resetDraft()
    }
    statusMessage.value = '规则已删除'
  } catch (err) {
    statusMessage.value = err instanceof Error ? err.message : String(err)
  } finally {
    setTimeout(() => (statusMessage.value = ''), 3000)
  }
}

async function handleSyncRule(rule: GithubRuleEntity) {
  if (!rule.enabled) {
    statusMessage.value = '该规则已停用，请先启用后再执行同步'
    setTimeout(() => (statusMessage.value = ''), 2500)
    return
  }
  statusMessage.value = `正在根据规则「${rule.name}」同步...`
  syncingRuleId.value = rule.id
  try {
    await modeStore.syncGithubRule({
      query: rule.query,
      pathHint: rule.pathHint?.trim() ? rule.pathHint.trim() : DEFAULT_PATH_HINT,
      ruleId: rule.id,
      ruleName: rule.name,
      delaySec: rule.delaySec,
      branch: rule.branch
    })
    modeStore.updateRuleRunTime(rule.id, new Date().toISOString())
    statusMessage.value = '模式采集已完成'
    await refreshSyncLogs()
  } catch (err) {
    statusMessage.value = err instanceof Error ? err.message : String(err)
  } finally {
    syncingRuleId.value = null
    setTimeout(() => (statusMessage.value = ''), 4000)
  }
}

async function refreshSyncLogs() {
  logsLoading.value = true
  logsError.value = null
  try {
    syncLogs.value = await modeStore.listSyncLogs({ limit: 50, offset: 0 })
  } catch (err) {
    logsError.value = err instanceof Error ? err.message : String(err)
  } finally {
    logsLoading.value = false
  }
}

async function handleClearLogs() {
  logsError.value = null
  try {
    await modeStore.clearSyncLogs()
    await refreshSyncLogs()
    statusMessage.value = '已清空同步记录'
  } catch (err) {
    logsError.value = err instanceof Error ? err.message : String(err)
  } finally {
    setTimeout(() => (statusMessage.value = ''), 3000)
  }
}

function handleExportLogs() {
  const data = JSON.stringify(syncLogs.value, null, 2)
  const blob = new Blob([data], { type: 'application/json;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `sync_logs_${Date.now()}.json`
  a.click()
  URL.revokeObjectURL(url)
}

function handleExportLogsCsv() {
  const headers = ['createdAt', 'status', 'ruleName', 'target', 'message']
  const escapeCell = (value: string) => `"${value.split('"').join('""')}"`
  const rows = syncLogs.value.map((log) => {
    return [
      formatDateTime(log.createdAt),
      log.status ?? '',
      log.ruleName ?? '',
      log.target ?? '',
      log.message ?? ''
    ].map((cell) => escapeCell(String(cell ?? ''))).join(',')
  })
  const csv = [headers.join(','), ...rows].join('\n')
  const blob = new Blob([csv], { type: 'text/csv;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `sync_logs_${Date.now()}.csv`
  a.click()
  URL.revokeObjectURL(url)
}

async function handleSyncAllEnabled() {
  const enabledRules = githubRules.value.filter((rule) => rule.enabled)
  if (!enabledRules.length) {
    statusMessage.value = '暂无启用规则'
    setTimeout(() => (statusMessage.value = ''), 2500)
    return
  }
  syncingAll.value = true
  try {
    for (let index = 0; index < enabledRules.length; index += 1) {
      const rule = enabledRules[index]
      statusMessage.value = `正在同步（${index + 1}/${enabledRules.length}）：${rule.name}`
      syncingRuleId.value = rule.id
      await modeStore.syncGithubRule({
        query: rule.query,
        pathHint: rule.pathHint?.trim() ? rule.pathHint.trim() : DEFAULT_PATH_HINT,
        ruleId: rule.id,
        ruleName: rule.name,
        delaySec: rule.delaySec,
        branch: rule.branch
      })
      modeStore.updateRuleRunTime(rule.id, new Date().toISOString())
    }
    statusMessage.value = '已完成所有启用规则同步'
    await refreshSyncLogs()
  } catch (err) {
    statusMessage.value = err instanceof Error ? err.message : String(err)
  } finally {
    syncingRuleId.value = null
    syncingAll.value = false
    setTimeout(() => (statusMessage.value = ''), 5000)
  }
}

onMounted(async () => {
  await modeStore.bootstrap().catch(() => {
    statusMessage.value = '加载规则失败，请稍后重试'
  })
  const settings = await modeStore.fetchGithubSettings().catch(() => null)
  if (settings) ruleDraft.delaySec = settings.delaySec
  await refreshSyncLogs()
})
</script>

<template>
  <div class="space-y-2 p-2">
    <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
      <header class="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
        <div>
          <h2 class="text-lg font-semibold text-gray-900">模式采集</h2>
          <p class="text-sm text-gray-500">按规则搜索并入库；Token/代理/延时请在“设置”中配置</p>
        </div>
        <div class="flex flex-wrap gap-2">
          <button
            class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700 hover:border-blue-500 hover:text-blue-600"
            @click="router.push('/settings')"
          >
            前往设置
          </button>
        </div>
      </header>
      <p v-if="statusMessage" class="mt-3 text-sm text-blue-600">{{ statusMessage }}</p>
    </section>

    <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
      <header class="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
        <div>
          <h2 class="text-lg font-semibold text-gray-900">同步记录（本地）</h2>
          <p class="text-sm text-gray-500">用于追溯 GitHub 拉取与入库过程，可在设置中关闭记录</p>
        </div>
        <div class="flex flex-wrap gap-2">
          <button
            class="whitespace-nowrap rounded-md border border-gray-200 px-3 py-2 text-sm text-gray-700"
            :disabled="logsLoading"
            @click="refreshSyncLogs"
          >
            刷新
          </button>
          <button
            class="whitespace-nowrap rounded-md border border-gray-200 px-3 py-2 text-sm text-gray-700"
            :disabled="!syncLogs.length"
            @click="handleExportLogs"
          >
            导出 JSON
          </button>
          <button
            class="whitespace-nowrap rounded-md border border-gray-200 px-3 py-2 text-sm text-gray-700"
            :disabled="!syncLogs.length"
            @click="handleExportLogsCsv"
          >
            导出 CSV
          </button>
          <button
            class="whitespace-nowrap rounded-md border border-red-200 px-3 py-2 text-sm text-red-600"
            :disabled="logsLoading"
            @click="handleClearLogs"
          >
            清空
          </button>
        </div>
      </header>

      <div v-if="logsLoading" class="mt-4 text-sm text-gray-500">正在加载同步记录...</div>
      <div v-else-if="logsError" class="mt-4 rounded-md bg-red-50 p-3 text-sm text-red-600">{{ logsError }}</div>
      <div v-else-if="!syncLogs.length" class="mt-4 text-sm text-gray-500">暂无记录</div>
      <div v-else class="mt-4 overflow-x-auto rounded-lg border border-gray-100">
        <table class="w-full table-fixed divide-y divide-gray-100 text-sm">
          <thead class="bg-gray-50">
            <tr>
              <th class="w-36 px-4 py-2 text-left text-xs font-medium text-gray-500">时间</th>
              <th class="w-20 px-4 py-2 text-left text-xs font-medium text-gray-500">状态</th>
              <th class="w-32 px-4 py-2 text-left text-xs font-medium text-gray-500">规则</th>
              <th class="w-40 px-4 py-2 text-left text-xs font-medium text-gray-500">目标</th>
              <th class="px-4 py-2 text-left text-xs font-medium text-gray-500">摘要</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-gray-100 bg-white">
            <tr v-for="log in syncLogs" :key="log.id" class="hover:bg-gray-50">
              <td class="px-4 py-3 text-xs text-gray-600">{{ formatDateTime(log.createdAt) }}</td>
              <td class="px-4 py-3">
                <span
                  :class="[
                    'rounded-full px-2 py-1 text-xs',
                    log.status === 'success'
                      ? 'bg-green-50 text-green-600'
                      : log.status === 'warning'
                        ? 'bg-yellow-50 text-yellow-700'
                        : 'bg-red-50 text-red-600'
                  ]"
	                >
	                  {{ formatSyncLogStatus(log.status) }}
	                </span>
	              </td>
              <td class="px-4 py-3 text-xs text-gray-600">
                <div class="truncate" :title="log.ruleName || '-'">{{ log.ruleName || '-' }}</div>
              </td>
              <td class="px-4 py-3 text-xs text-gray-600">
                <div class="truncate" :title="log.target || '-'">{{ log.target || '-' }}</div>
              </td>
              <td class="px-4 py-3 text-xs text-gray-600">
                <div class="truncate" :title="log.message || '-'">{{ log.message || '-' }}</div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>

    <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
      <header class="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
        <div>
          <h2 class="text-lg font-semibold text-gray-900">GitHub 搜索规则</h2>
          <p class="text-sm text-gray-500">支持多条规则并行执行，结果先入库再处理</p>
        </div>
        <div class="flex flex-wrap items-center gap-2">
          <button
            class="rounded-md bg-blue-600 px-3 py-2 text-sm text-white disabled:cursor-not-allowed disabled:opacity-60"
            :disabled="syncingAll || syncingRuleId !== null"
            @click="handleSyncAllEnabled"
          >
            {{ syncingAll ? '同步中...' : '同步所有启用规则' }}
          </button>
          <input
            v-model="ruleKeyword"
            type="text"
            placeholder="搜索规则"
            class="rounded-md border border-gray-200 px-3 py-2 text-sm md:w-64"
          />
        </div>
      </header>

      <div class="mt-4 overflow-hidden rounded-lg border border-gray-100" v-if="filteredRules.length">
        <table class="w-full table-fixed divide-y divide-gray-100">
          <thead class="bg-gray-50">
            <tr>
              <!-- <th class="w-28 px-4 py-2 text-left text-xs font-medium text-gray-500">规则</th> -->
              <th class="px-4 py-2 text-left text-xs font-medium text-gray-500" >查询语句</th>
              <th class="w-16 px-4 py-2 text-left text-xs font-medium text-gray-500">分支</th>
              <th class="w-16 px-4 py-2 text-left text-xs font-medium text-gray-500">延时</th>
              <th class="w-16 px-4 py-2 text-left text-xs font-medium text-gray-500">状态</th>
              <th class="w-24 px-4 py-2 text-left text-xs font-medium text-gray-500">上次执行</th>
              <th class="w-32 px-4 py-2 text-right text-xs font-medium text-gray-500">操作</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-gray-100 bg-white">
            <tr v-for="rule in filteredRules" :key="rule.id" class="hover:bg-gray-50">
              <!-- <td class="px-4 py-3">
                <p class="truncate text-sm font-medium text-gray-900" :title="rule.name">{{ rule.name }}</p>
                <p class="truncate text-xs text-gray-400" :title="rule.id">{{ rule.id }}</p>
              </td> -->
              <td class="px-4 py-3 text-sm text-gray-600">
                <div class="text-clip" :title="rule.query">{{ rule.query }}</div>
              </td>
              <td class="px-4 py-3 text-sm text-gray-600">
                <span class="whitespace-nowrap">{{ rule.branch || 'main' }}</span>
              </td>
              <td class="px-4 py-3 text-sm text-gray-600">
                <span class="whitespace-nowrap">{{ rule.delaySec }}s</span>
              </td>
              <td class="px-4 py-3 text-sm">
                <span
                  :class="[
                    'rounded-full px-2 py-1 text-xs truncate',
                    rule.enabled ? 'bg-green-50 text-green-600' : 'bg-gray-50 text-gray-500'
                  ]"
                >
                  {{ rule.enabled ? '启用' : '停用' }}
                </span>
              </td>
              <td class="px-4 py-3 text-sm text-gray-600">{{ rule.lastRunAt ? formatDateTime(rule.lastRunAt) : '-' }}</td>
              <td class="px-4 py-3 text-right">
                <div class="flex justify-end gap-2">
                  <button
                    class="rounded-md border border-gray-200 p-1.5 text-gray-700 hover:border-blue-500 hover:text-blue-600 disabled:cursor-not-allowed disabled:opacity-60"
                    :disabled="syncingRuleId === rule.id || !rule.enabled"
                    :title="syncingRuleId === rule.id ? '同步中...' : '执行同步'"
                    :aria-label="syncingRuleId === rule.id ? '同步中...' : '执行同步'"
                    @click="handleSyncRule(rule)"
                  >
                    <svg viewBox="0 0 20 20" fill="currentColor" class="h-4 w-4">
                      <path
                        fill-rule="evenodd"
                        d="M10 3a7 7 0 00-6.32 4H2.75a.75.75 0 000 1.5H5.5a.75.75 0 00.75-.75V5.5a.75.75 0 00-1.5 0v.86A5.5 5.5 0 0115.5 10a.75.75 0 001.5 0A7 7 0 0010 3z"
                        clip-rule="evenodd"
                      />
                      <path d="M8.25 7.5a.75.75 0 00-.75.75v3.5a.75.75 0 001.5 0v-3.5a.75.75 0 00-.75-.75zM12.5 9.25a.75.75 0 00-1.2-.6l-2.5 1.75a.75.75 0 000 1.2l2.5 1.75a.75.75 0 001.2-.6v-3.5z" />
                    </svg>
                  </button>
                  <button
                    class="rounded-md border border-gray-200 p-1.5 text-gray-700 hover:border-blue-500 hover:text-blue-600"
                    title="编辑"
                    aria-label="编辑"
                    @click="handleEditRule(rule)"
                  >
                    <svg viewBox="0 0 20 20" fill="currentColor" class="h-4 w-4">
                      <path d="M13.586 3.586a2 2 0 012.828 2.828l-8.5 8.5a1 1 0 01-.39.242l-3 1a1 1 0 01-1.265-1.265l1-3a1 1 0 01.242-.39l8.5-8.5z" />
                    </svg>
                  </button>
                  <button
                    class="rounded-md border border-gray-200 p-1.5 text-gray-700 hover:border-blue-500 hover:text-blue-600"
                    :title="rule.enabled ? '停用' : '启用'"
                    :aria-label="rule.enabled ? '停用' : '启用'"
                    @click="handleToggleRule(rule)"
                  >
                    <svg v-if="rule.enabled" viewBox="0 0 20 20" fill="currentColor" class="h-4 w-4">
                      <path d="M6.75 5.5a.75.75 0 00-.75.75v7.5a.75.75 0 001.5 0v-7.5a.75.75 0 00-.75-.75zM13.25 5.5a.75.75 0 00-.75.75v7.5a.75.75 0 001.5 0v-7.5a.75.75 0 00-.75-.75z" />
                    </svg>
                    <svg v-else viewBox="0 0 20 20" fill="currentColor" class="h-4 w-4">
                      <path d="M7.5 5.75a.75.75 0 011.2-.6l6 4.25a.75.75 0 010 1.2l-6 4.25A.75.75 0 017.5 14.25v-8.5z" />
                    </svg>
                  </button>
                  <button
                    class="rounded-md border border-red-200 p-1.5 text-red-600 hover:border-red-400"
                    title="删除"
                    aria-label="删除"
                    @click="handleDeleteRule(rule)"
                  >
                    <svg viewBox="0 0 20 20" fill="currentColor" class="h-4 w-4">
                      <path
                        fill-rule="evenodd"
                        d="M7.5 2.75A.75.75 0 018.25 2h3.5a.75.75 0 01.75.75V4h3a.75.75 0 010 1.5h-.72l-.76 11.02A2 2 0 0112.03 18H7.97a2 2 0 01-1.99-1.48L5.22 5.5H4.5a.75.75 0 010-1.5h3V2.75zM9 4h2V3.5H9V4z"
                        clip-rule="evenodd"
                      />
                      <path d="M8.5 8a.75.75 0 011.5 0v6a.75.75 0 01-1.5 0V8zM10.75 8a.75.75 0 011.5 0v6a.75.75 0 01-1.5 0V8z" />
                    </svg>
                  </button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      <p v-else class="mt-2 text-sm text-gray-500">暂无规则，可通过下方表单添加。</p>

      <div class="mt-6 rounded-lg border border-dashed border-blue-200 bg-blue-50/40 p-4">
        <p class="text-sm font-semibold text-gray-700">{{ editingRuleId ? '编辑规则' : '新增规则' }}</p>
        <!-- <p class="mt-1 text-xs text-gray-500">解析提示为系统内置（无需填写），会保留 GitHub 原始字段并透传入库。</p> -->
        <div class="mt-3 grid gap-3 md:grid-cols-2">
          <label class="text-xs text-gray-600">
            规则名称
            <input v-model="ruleDraft.name" type="text" class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm" />
          </label>
          <label class="text-xs text-gray-600">
            搜索语句
            <input v-model="ruleDraft.query" type="text" class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm" />
          </label>
          <label class="text-xs text-gray-600">
            分支（默认 main）
            <input v-model="ruleDraft.branch" type="text" class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm" />
          </label>
          <label class="text-xs text-gray-600">
            单次调用延时（秒）
            <input
              v-model.number="ruleDraft.delaySec"
              type="number"
              min="1"
              class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm"
            />
          </label>
          <div class="flex items-center justify-between md:col-span-2">
            <label class="flex items-center gap-2 text-xs text-gray-600">
              <input v-model="ruleDraft.enabled" type="checkbox" class="rounded border-gray-300" />
              保存后立即启用
            </label>
            <!-- <button
              class="text-xs text-gray-600 underline decoration-dashed underline-offset-4 hover:text-blue-600"
              @click="showAdvanced = !showAdvanced"
            >
              {{ showAdvanced ? '收起高级选项' : '展开高级选项' }}
            </button> -->
          </div>
          <!-- <label v-if="showAdvanced" class="text-xs text-gray-600 md:col-span-2">
            解析提示（高级选项）
            <textarea
              v-model="ruleDraft.pathHint"
              rows="2"
              class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm"
            ></textarea>
            <p class="mt-1 text-[11px] text-gray-500">默认值即可；除非你明确知道自定义结构，否则不建议修改。</p>
          </label> -->
          <div class="flex flex-wrap justify-end gap-2 md:col-span-2">
            <button @click="handleSaveRule" class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white">
              {{ editingRuleId ? '保存修改' : '保存规则' }}
            </button>
            <button
              v-if="editingRuleId"
              class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700"
              @click="resetDraft"
            >
              取消编辑
            </button>
          </div>
        </div>
      </div>
      <div
        v-if="lastSyncResult"
        class="mt-4 rounded-lg border border-green-100 bg-green-50/70 p-4 text-xs text-green-700"
      >
        <p class="font-semibold">最近一次同步结果</p>
        <p>抓取 {{ lastSyncResult.fetchedFiles }} 个文件，成功入库 {{ lastSyncResult.savedModes }} 条，跳过 {{ lastSyncResult.skippedDueToMissingFields }} 条。</p>
        <p v-if="lastSyncResult.errors.length" class="mt-1 text-red-600">
          {{ lastSyncResult.errors.length }} 个错误示例：{{ lastSyncResult.errors[0] }}
        </p>
      </div>
    </section>
  </div>
</template>
