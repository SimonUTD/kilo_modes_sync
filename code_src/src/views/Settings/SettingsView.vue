<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { openPath } from '@tauri-apps/plugin-opener'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { useModeStore } from '../../stores/modes'

type SettingsSectionId = 'github' | 'log' | 'quality' | 'db'

const systemSettings = reactive({
  enableLog: true,
  logLevel: 'info',
  retentionDays: 30,
  showRoleDefinitionLength: true,
  qualityThreshold: 800,
  autoDeduplicate: true
})

const githubSettingsForm = reactive({
  token: '',
  delaySec: 3,
  proxyUrl: ''
})

const backupOptions = reactive({
  includeModes: true,
  includeRules: true,
  includeInstances: true
})

const message = ref('')
const githubMessage = ref('')
const activeSection = ref<SettingsSectionId>('github')
const logsDir = ref('')
const logsDirMessage = ref('')
const showImportModal = ref(false)
const importFilePath = ref('')
const importFileMeta = ref<{
  version: number
  exportedAt: string
  includeModes: boolean
  includeRules: boolean
  includeInstances: boolean
  includeSettings: boolean
  modesCount: number
  githubRulesCount: number
  ideInstancesCount: number
} | null>(null)
const importStatus = ref<string | null>(null)

const modeStore = useModeStore()

const sections = computed(() => {
  return [
    { id: 'github' as const, label: 'GitHub 配置', description: 'Token、代理与调用延时' },
    { id: 'log' as const, label: '日志配置', description: '本地日志与保留策略' },
    { id: 'quality' as const, label: '模式质量配置', description: '阈值与去重策略' },
    { id: 'db' as const, label: '数据库管理', description: '导入/导出备份文件' }
  ]
})

const githubTokenTestPassedAtText = computed(() => {
  const token = githubSettingsForm.token.trim()
  if (!token) return '请先填写 Token'
  const raw = modeStore.githubSettings?.lastTokenTestPassedAt
  if (!raw) return '尚未测试'
  const date = new Date(raw)
  if (Number.isNaN(date.getTime())) return raw
  return date.toLocaleString()
})

async function handleSaveGithubSettings() {
  githubMessage.value = '正在保存...'
  try {
    await modeStore.updateGithubSettings({
      token: githubSettingsForm.token,
      proxy: githubSettingsForm.proxyUrl || null,
      delaySec: githubSettingsForm.delaySec
    })
    githubMessage.value = 'GitHub 配置已保存'
  } catch (err) {
    githubMessage.value = err instanceof Error ? err.message : String(err)
  } finally {
    setTimeout(() => (githubMessage.value = ''), 3000)
  }
}

async function handleTestGithubToken() {
  githubMessage.value = '正在验证 Token...'
  try {
    await handleSaveGithubSettings()
    const result = await modeStore.testGithubToken()
    githubMessage.value = result.message
    if (result.ok) {
      await modeStore.fetchGithubSettings().catch(() => null)
    }
  } catch (err) {
    githubMessage.value = err instanceof Error ? err.message : String(err)
  } finally {
    setTimeout(() => (githubMessage.value = ''), 5000)
  }
}

async function handleSaveSettings() {
  try {
    await modeStore.updateAppSettings({
      enableLog: systemSettings.enableLog,
      logLevel: systemSettings.logLevel,
      retentionDays: systemSettings.retentionDays,
      showRoleDefinitionLength: systemSettings.showRoleDefinitionLength,
      qualityThreshold: systemSettings.qualityThreshold,
      autoDeduplicate: systemSettings.autoDeduplicate
    })
    message.value = '设置已保存'
  } catch (err) {
    message.value = err instanceof Error ? err.message : String(err)
  } finally {
    setTimeout(() => (message.value = ''), 2500)
  }
}

function handleExport() {
  handleExportToFile().catch(() => {
    /* 错误信息在内部处理 */
  })
}

function handleImport() {
  importStatus.value = null
  importFilePath.value = ''
  importFileMeta.value = null
  showImportModal.value = true
}

function closeImportModal() {
  showImportModal.value = false
}

async function pickImportFile() {
  importStatus.value = null
  importFileMeta.value = null
  try {
    const selected = await openDialog({
      title: '选择备份文件',
      multiple: false,
      directory: false,
      filters: [{ name: '备份文件', extensions: ['json'] }]
    })
    if (!selected || Array.isArray(selected)) return
    importFilePath.value = selected
    importStatus.value = '正在校验备份文件...'
    importFileMeta.value = await modeStore.validateBackupFile(selected)
    importStatus.value = null
  } catch (err) {
    importStatus.value = err instanceof Error ? err.message : String(err)
  }
}

async function confirmImport() {
  if (!importFilePath.value) {
    importStatus.value = '请先选择备份文件'
    return
  }
  importStatus.value = '正在导入...'
  try {
    const result = await modeStore.importBackupFromFile(importFilePath.value)
    importStatus.value = `导入完成：模式 ${result.importedModes}（跳过重复 ${result.skippedDuplicateModes}），规则 ${result.importedRules}，实例 ${result.importedInstances}`
  } catch (err) {
    importStatus.value = err instanceof Error ? err.message : String(err)
  }
}

async function handleExportToFile() {
  message.value = '请选择备份保存目录...'
  try {
    const selected = await openDialog({
      title: '选择备份保存目录',
      multiple: false,
      directory: true
    })
    if (!selected || Array.isArray(selected)) {
      message.value = ''
      return
    }
    message.value = '正在生成备份文件...'
    const filePath = await modeStore.exportBackupToFile({
      targetDir: selected,
      options: {
        includeModes: backupOptions.includeModes,
        includeRules: backupOptions.includeRules,
        includeInstances: backupOptions.includeInstances,
        includeSettings: true
      }
    })
    message.value = `备份已导出：${filePath}`
  } catch (err) {
    message.value = err instanceof Error ? err.message : String(err)
  } finally {
    setTimeout(() => (message.value = ''), 3500)
  }
}

async function refreshLogsDir() {
  logsDirMessage.value = ''
  try {
    logsDir.value = await modeStore.getLogsDir()
  } catch (err) {
    logsDirMessage.value = err instanceof Error ? err.message : String(err)
  }
}

async function openLogsDir() {
  if (!logsDir.value) return
  logsDirMessage.value = '正在打开目录...'
  try {
    await openPath(logsDir.value)
    logsDirMessage.value = ''
  } catch (err) {
    logsDirMessage.value = err instanceof Error ? err.message : String(err)
  }
}

onMounted(async () => {
  const settings = await modeStore.fetchAppSettings().catch(() => null)
  if (settings) {
    systemSettings.enableLog = settings.enableLog
    systemSettings.logLevel = settings.logLevel
    systemSettings.retentionDays = settings.retentionDays
    systemSettings.showRoleDefinitionLength = settings.showRoleDefinitionLength
    systemSettings.qualityThreshold = settings.qualityThreshold
    systemSettings.autoDeduplicate = settings.autoDeduplicate
  }
  const github = await modeStore.fetchGithubSettings().catch(() => null)
  if (github) {
    githubSettingsForm.token = github.token
    githubSettingsForm.delaySec = github.delaySec
    githubSettingsForm.proxyUrl = github.proxy ?? ''
  }
  await refreshLogsDir()
})
</script>

<template>
  <div class="flex gap-2 p-2">
    <aside class="w-36 shrink-0 top-0 h-fit verflow-y-auto border-r border-gray-200 bg-white/90 p-4 shadow-sm">
      <div class="p-0 shadow-sm">
        <p class="text-sm font-semibold text-gray-900">设置</p>
        <nav class="mt-3 space-y-1">
          <button v-for="item in sections" :key="item.id" type="button"
            class="w-full rounded-md px-3 py-2 text-left transition-all"
            :class="activeSection === item.id ? 'bg-blue-50 text-blue-700 ring-1 ring-blue-100' : 'text-gray-700 hover:bg-gray-50'"
            @click="activeSection = item.id">
            <p class="text-sm font-medium">{{ item.label }}</p>
            <!-- <p class="mt-0.5 text-xs text-gray-500" :class="activeSection === item.id ? 'text-blue-600/80' : ''">{{ item.description }}</p> -->
          </button>
        </nav>
      </div>
    </aside>

    <div class="min-w-0 flex-1 space-y-3">
      <section v-if="activeSection === 'github'" class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
        <header class="flex items-center justify-between">
          <div>
            <h2 class="text-lg font-semibold text-gray-900">GitHub 配置</h2>
          </div>
        </header>
        <div class="mt-4 grid gap-4 sm:grid-cols-1">
          <label class="text-sm text-gray-700">
            Personal Access Token
            <input v-model="githubSettingsForm.token" type="password" placeholder="ghp_xxx"
              class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm" />
          </label>
        </div>
        <div class="mt-4 grid gap-4 sm:grid-cols-1">
          <label class="text-sm text-gray-700">
            调用延时（秒）
            <input v-model.number="githubSettingsForm.delaySec" type="number" min="1"
              class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm" />
          </label>
        </div>
        <div class="mt-4 grid gap-4 sm:grid-cols-1">
          <label class="text-sm text-gray-700 sm:col-span-2">
            代理地址（可选）
            <input v-model="githubSettingsForm.proxyUrl" type="text" placeholder="http://127.0.0.1:7890"
              class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm" />
          </label>
        </div>
        <div class="mt-3 rounded-md bg-gray-50 px-4 py-3 text-sm text-gray-700">
          <p class="flex flex-wrap items-center gap-x-2 gap-y-1">
            <span class="font-medium text-gray-800">最近测试通过时间：</span>
            <span class="font-mono text-[13px]">{{ githubTokenTestPassedAtText }}</span>
          </p>
        </div>
        <div class="mt-4 items-center gap-x-2 gap-y-1 flex flex-wrap">
          <button @click="handleTestGithubToken" class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white">测试
            Token</button>
          <button @click="handleSaveGithubSettings"
            class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700">保存</button>
        </div>
        <p v-if="githubMessage" class="mt-3 text-sm text-blue-600">{{ githubMessage }}</p>
      </section>

      <section v-if="activeSection === 'quality'" class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
        <header>
          <h2 class="text-lg font-semibold text-gray-900">模式质量</h2>
          <p class="text-sm text-gray-500">用于筛选“高质量候选”与列表展示</p>
        </header>
        <div class="mt-4 grid gap-4 md:grid-cols-1">
          <label class="text-sm text-gray-700 md:col-span-1">
            高质量阈值（以模式的角色定义的字符数计算）
            <input v-model.number="systemSettings.qualityThreshold" type="number" min="0"
              class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm" />
          </label>
          <label class="flex items-center gap-2 text-sm text-gray-700">
            <input v-model="systemSettings.showRoleDefinitionLength" type="checkbox" class="rounded border-gray-300" />
            列表中显示长度列
          </label>
        </div>
        <div class="mt-4 flex flex-wrap gap-4 text-sm text-gray-700">
          <label class="flex items-center gap-2">
            <input v-model="systemSettings.autoDeduplicate" type="checkbox" class="rounded border-gray-300" />
            入库时按内容哈希自动去重
          </label>
        </div>
        <div class="mt-4">
          <button @click="handleSaveSettings" class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white">保存</button>
        </div>
      </section>

      <section v-if="activeSection === 'log'" class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
        <header class="flex flex-wrap items-center justify-between gap-3">
          <div>
            <h2 class="text-lg font-semibold text-gray-900">日志</h2>
            <p class="text-sm text-gray-500">文件日志按分钟切分，按保留天数自动清理</p>
          </div>
        </header>
        <div class="mt-4 grid gap-4 lg:grid-cols-1">
          <div class="gap-4 sm:grid-cols-1 flex items-center gap-2 text-sm text-gray-700">

            <input v-model="systemSettings.enableLog" type="checkbox" class="rounded border-gray-300" />
            <span>启用本地日志</span>
          </div>
          <div class="grid gap-4 sm:grid-cols-1">
            <label class="text-sm text-gray-700">
              日志级别
              <select v-model="systemSettings.logLevel"
                class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm">
                <option value="error">Error</option>
                <option value="warn">Warn</option>
                <option value="info">Info</option>
                <option value="debug">Debug</option>
              </select>
            </label>
            <label class="text-sm text-gray-700">
              保留天数
              <input v-model.number="systemSettings.retentionDays" type="number" min="1"
                class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm" />
            </label>
          </div>
          <div class="rounded-md text-sm text-gray-700">
            <p class="text-sm font-medium text-gray-800">日志目录</p>
            <p class="mt-1 break-all font-mono text-[13px] text-gray-600">{{ logsDir || '读取中...' }}</p>
            <div class="mt-3 flex flex-wrap gap-2">
              <button
                class="rounded-md border border-gray-200 bg-white px-3 py-2 text-sm text-gray-700 disabled:cursor-not-allowed disabled:opacity-50"
                :disabled="!logsDir" @click="openLogsDir">
                打开目录
              </button>
              <button class="rounded-md border border-gray-200 bg-white px-3 py-2 text-sm text-gray-700"
                @click="refreshLogsDir">
                刷新
              </button>
            </div>
            <p v-if="logsDirMessage" class="mt-2 text-sm text-blue-600">{{ logsDirMessage }}</p>
            <p class="mt-2 text-xs text-gray-500">关闭日志会同时停止写入同步记录与写回历史。</p>
          </div>
        </div>
        <div class="mt-4">
          <button @click="handleSaveSettings"
            class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white">保存</button>
        </div>
      </section>

      <section v-if="activeSection === 'db'" class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
        <header class="flex items-center justify-between">
          <div>
            <h2 class="text-lg font-semibold text-gray-900">数据库备份</h2>
            <p class="text-sm text-gray-500">支持导出/导入本地库，方便在多台机器之间迁移</p>
          </div>
        </header>
        <div class="mt-4 grid gap-4 md:grid-cols-3">
          <label class="flex items-center gap-2 text-sm text-gray-700">
            <input v-model="backupOptions.includeModes" type="checkbox" class="rounded border-gray-300" />
            模式数据
          </label>
          <label class="flex items-center gap-2 text-sm text-gray-700">
            <input v-model="backupOptions.includeRules" type="checkbox" class="rounded border-gray-300" />
            GitHub 规则
          </label>
          <label class="flex items-center gap-2 text-sm text-gray-700">
            <input v-model="backupOptions.includeInstances" type="checkbox" class="rounded border-gray-300" />
            IDE 实例
          </label>
        </div>
        <div class="mt-4 flex gap-3">
          <button @click="handleExport"
            class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700">导出备份</button>
          <button @click="handleImport" class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white">导入备份</button>
        </div>
      </section>

      <p v-if="message" class="text-sm text-blue-600">{{ message }}</p>

      <div v-if="showImportModal"
        class="fixed inset-0 z-30 flex items-center justify-center bg-black/30 backdrop-blur-sm" aria-modal="true"
        role="dialog">
        <div class="w-full max-w-2xl rounded-lg bg-white shadow-xl">
          <header class="flex items-center justify-between border-b border-gray-100 px-5 py-4">
            <div>
              <h3 class="text-lg font-semibold text-gray-900">导入备份</h3>
              <p class="text-sm text-gray-500">选择备份文件（JSON），校验后再导入</p>
            </div>
            <button @click="closeImportModal" class="text-gray-400 hover:text-gray-600">✕</button>
          </header>
          <div class="space-y-3 px-5 py-4">
            <div class="flex flex-wrap items-center gap-2">
              <button class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white"
                @click="pickImportFile">选择备份文件</button>
              <span class="min-w-0 break-all font-mono text-[12px] text-gray-600">{{ importFilePath || '未选择' }}</span>
            </div>
            <div v-if="importFileMeta"
              class="rounded-md border border-gray-200 bg-gray-50 px-4 py-3 text-sm text-gray-700">
              <p class="text-sm font-medium text-gray-800">校验结果</p>
              <ul class="mt-2 grid gap-1 text-sm text-gray-700 sm:grid-cols-2">
                <li>版本：{{ importFileMeta.version }}</li>
                <li>导出时间：{{ new Date(importFileMeta.exportedAt).toLocaleString() }}</li>
                <li>模式：{{ importFileMeta.modesCount }}</li>
                <li>规则：{{ importFileMeta.githubRulesCount }}</li>
                <li>实例：{{ importFileMeta.ideInstancesCount }}</li>
                <li>包含设置：{{ importFileMeta.includeSettings ? '是' : '否' }}</li>
              </ul>
            </div>
            <p v-if="importStatus" class="text-sm text-blue-600">{{ importStatus }}</p>
          </div>
          <footer class="flex justify-end gap-3 border-t border-gray-100 px-5 py-4">
            <button class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700" @click="closeImportModal">
              取消
            </button>
            <button
              class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white disabled:cursor-not-allowed disabled:opacity-50"
              :disabled="!importFileMeta || !importFilePath" @click="confirmImport">
              开始导入
            </button>
          </footer>
        </div>
      </div>
    </div>
  </div>
</template>
