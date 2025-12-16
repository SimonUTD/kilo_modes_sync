<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { useModeStore } from '../../stores/modes'

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
const showImportModal = ref(false)
const importText = ref('')
const importStatus = ref<string | null>(null)

const modeStore = useModeStore()

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
    githubMessage.value = result.ok ? result.message : result.message
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
  message.value = '正在生成备份...'
  importStatus.value = null
  modeStore
    .exportBackup({
      includeModes: backupOptions.includeModes,
      includeRules: backupOptions.includeRules,
      includeInstances: backupOptions.includeInstances,
      includeSettings: true
    })
    .then((payload) => {
      const data = JSON.stringify(payload, null, 2)
      const blob = new Blob([data], { type: 'application/json;charset=utf-8' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `kilo_roo_backup_${Date.now()}.json`
      a.click()
      URL.revokeObjectURL(url)
      message.value = '备份已导出'
    })
    .catch((err) => {
      message.value = err instanceof Error ? err.message : String(err)
    })
    .finally(() => {
      setTimeout(() => (message.value = ''), 2500)
    })
}

function handleImport() {
  importStatus.value = null
  importText.value = ''
  showImportModal.value = true
}

function closeImportModal() {
  showImportModal.value = false
}

async function confirmImport() {
  importStatus.value = '正在导入...'
  try {
    const parsed = JSON.parse(importText.value)
    const result = await modeStore.importBackup(parsed)
    importStatus.value = `导入完成：模式 ${result.importedModes}（跳过重复 ${result.skippedDuplicateModes}），规则 ${result.importedRules}，实例 ${result.importedInstances}`
  } catch (err) {
    importStatus.value = err instanceof Error ? err.message : String(err)
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
})
</script>

<template>
  <div class="space-y-2 p-2">
    <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
      <header class="flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold text-gray-900">GitHub 配置</h2>
          <p class="text-sm text-gray-500">Token、代理与调用延时（用于 GitHub 模式采集）</p>
        </div>
        <div class="flex flex-wrap gap-2">
          <button @click="handleTestGithubToken" class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white">测试 Token</button>
          <button @click="handleSaveGithubSettings" class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700">保存</button>
        </div>
      </header>
      <div class="mt-4 grid gap-4 sm:grid-cols-2">
        <label class="text-sm text-gray-700">
          Personal Access Token
          <input
            v-model="githubSettingsForm.token"
            type="password"
            placeholder="ghp_xxx"
            class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm"
          />
        </label>
        <label class="text-sm text-gray-700">
          调用延时（秒）
          <input
            v-model.number="githubSettingsForm.delaySec"
            type="number"
            min="1"
            class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm"
          />
        </label>
        <label class="text-sm text-gray-700 sm:col-span-2">
          代理地址（可选）
          <input
            v-model="githubSettingsForm.proxyUrl"
            type="text"
            placeholder="http://127.0.0.1:7890"
            class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm"
          />
        </label>
      </div>
      <p v-if="githubMessage" class="mt-3 text-sm text-blue-600">{{ githubMessage }}</p>
    </section>

    <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
      <header>
        <h2 class="text-lg font-semibold text-gray-900">模式质量</h2>
        <p class="text-sm text-gray-500">用于筛选“高质量候选”与列表展示</p>
      </header>
      <div class="mt-4 grid gap-4 md:grid-cols-3">
        <label class="text-sm text-gray-700 md:col-span-2">
          高质量阈值（roleDefinition 字数）
          <input
            v-model.number="systemSettings.qualityThreshold"
            type="number"
            min="0"
            class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm"
          />
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
        <button @click="handleSaveSettings" class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white">保存模式设置</button>
      </div>
    </section>

    <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
      <header class="flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold text-gray-900">日志</h2>
          <p class="text-sm text-gray-500">文件日志按分钟切分，按保留天数自动清理</p>
        </div>
        <label class="flex items-center gap-2 text-sm text-gray-700">
          <input v-model="systemSettings.enableLog" type="checkbox" class="rounded border-gray-300" />
          启用本地日志
        </label>
      </header>
      <div class="mt-4 grid gap-4 md:grid-cols-3">
        <label class="text-sm text-gray-700">
          日志级别
          <select v-model="systemSettings.logLevel" class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm">
            <option value="error">Error</option>
            <option value="warn">Warn</option>
            <option value="info">Info</option>
            <option value="debug">Debug</option>
          </select>
        </label>
        <label class="text-sm text-gray-700">
          保留天数
          <input
            v-model.number="systemSettings.retentionDays"
            type="number"
            min="1"
            class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm"
          />
        </label>
        <div class="text-sm text-gray-700">
          <p class="text-sm font-medium text-gray-700">提示</p>
          <p class="mt-1 text-xs text-gray-500">关闭日志会同时停止写入同步记录与写回历史。</p>
        </div>
      </div>
      <div class="mt-4">
        <button @click="handleSaveSettings" class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white">保存日志设置</button>
      </div>
    </section>

    <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
      <header class="flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold text-gray-900">数据库备份</h2>
          <p class="text-sm text-gray-500">支持导出/导入本地库，方便在多台机器之间迁移</p>
        </div>
        <p class="text-xs text-gray-400">文件格式：JSON 备份包</p>
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
        <button @click="handleExport" class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700">导出备份</button>
        <button @click="handleImport" class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white">导入备份</button>
      </div>
    </section>

    <p v-if="message" class="text-sm text-blue-600">{{ message }}</p>

    <div v-if="showImportModal" class="fixed inset-0 z-30 flex items-center justify-center bg-black/30 backdrop-blur-sm" aria-modal="true" role="dialog">
      <div class="w-full max-w-2xl rounded-lg bg-white shadow-xl">
        <header class="flex items-center justify-between border-b border-gray-100 px-5 py-4">
          <div>
            <h3 class="text-lg font-semibold text-gray-900">导入备份</h3>
            <p class="text-sm text-gray-500">粘贴 `export_backup` 生成的 JSON 内容</p>
          </div>
          <button @click="closeImportModal" class="text-gray-400 hover:text-gray-600">✕</button>
        </header>
        <div class="space-y-3 px-5 py-4">
	          <textarea
	            v-model="importText"
	            rows="8"
	            placeholder="粘贴备份 JSON（例如：{ version: 1, ... }）"
	            class="w-full rounded-md border border-gray-200 px-3 py-2 text-xs font-mono text-gray-800"
	          />
          <p v-if="importStatus" class="text-sm text-blue-600">{{ importStatus }}</p>
        </div>
        <footer class="flex justify-end gap-3 border-t border-gray-100 px-5 py-4">
          <button class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700" @click="closeImportModal">
            取消
          </button>
          <button class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white" @click="confirmImport">开始导入</button>
        </footer>
      </div>
    </div>
  </div>
</template>
