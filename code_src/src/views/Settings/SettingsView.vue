<script setup lang="ts">
import { reactive, ref } from 'vue'

const systemSettings = reactive({
  enableLog: true,
  logLevel: 'info',
  retentionDays: 30,
  showRoleDefinitionLength: true,
  qualityThreshold: 800,
  autoDeduplicate: true
})

const backupOptions = reactive({
  includeModes: true,
  includeRules: true,
  includeInstances: true
})

const message = ref('')

function handleSaveSettings() {
  message.value = '配置已保存，将在下次与 Tauri 同步时写入本地数据库'
  setTimeout(() => (message.value = ''), 2500)
}

function handleExport() {
  message.value = '已准备导出本地数据库，稍后将触发 Tauri 命令生成备份文件'
  setTimeout(() => (message.value = ''), 2500)
}

function handleImport() {
  message.value = '请稍后选择备份文件，导入过程会进行字段校验'
  setTimeout(() => (message.value = ''), 2500)
}
</script>

<template>
  <div class="space-y-6 p-6">
    <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
      <header class="flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold text-gray-900">日志与监控</h2>
          <p class="text-sm text-gray-500">日志默认保留 30 天，可按级别拆分</p>
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
        <label class="text-sm text-gray-700">
          RoleDefinition 长度筛选（字数）
          <input
            v-model.number="systemSettings.qualityThreshold"
            type="number"
            min="0"
            class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm"
          />
        </label>
      </div>
      <div class="mt-4 flex flex-wrap gap-4 text-sm text-gray-700">
        <label class="flex items-center gap-2">
          <input v-model="systemSettings.showRoleDefinitionLength" type="checkbox" class="rounded border-gray-300" />
          列表中显示 roleDefinition 长度
        </label>
        <label class="flex items-center gap-2">
          <input v-model="systemSettings.autoDeduplicate" type="checkbox" class="rounded border-gray-300" />
          自动按照内容哈希去重
        </label>
      </div>
      <div class="mt-4">
        <button @click="handleSaveSettings" class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white">保存设置</button>
      </div>
    </section>

    <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
      <header class="flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold text-gray-900">数据库备份</h2>
          <p class="text-sm text-gray-500">支持导出/导入本地库，方便在多台机器之间迁移</p>
        </div>
        <p class="text-xs text-gray-400">文件格式：加密 SQLite 或 JSON 包</p>
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

    <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
      <header>
        <h2 class="text-lg font-semibold text-gray-900">其它默认值</h2>
        <p class="text-sm text-gray-500">用于指导后端在新安装时刻的初始化策略</p>
      </header>
      <div class="mt-4 space-y-3 text-sm text-gray-700">
        <p>• 程序首次运行会扫描白名单路径并以来源标记写入库；后续再同步时依赖内容哈希判断变更。</p>
        <p>• GitHub 拉取失败会记录日志，但不会阻断其它规则的执行。</p>
        <p>• 本地日志按分钟切分文件，并在 30 天后自动清理。</p>
      </div>
    </section>

    <p v-if="message" class="text-sm text-blue-600">{{ message }}</p>
  </div>
</template>
