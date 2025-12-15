<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { storeToRefs } from 'pinia'
import { useModeStore, type GithubRuleEntity } from '../../stores/modes'

interface SyncRecord {
  id: string
  ruleName: string
  fetched: number
  stored: number
  skipped: number
  finishedAt: string
  status: 'success' | 'warning' | 'error'
}

const githubSettings = reactive({
  token: '',
  delay: 3,
  enableProxy: false,
  proxyUrl: ''
})

const modeStore = useModeStore()
const { githubRules } = storeToRefs(modeStore)

const ruleDraft = reactive<Pick<GithubRuleEntity, 'name' | 'query' | 'pathHint' | 'enabled' | 'delaySec'>>({
  name: '',
  query: '',
  pathHint: 'customModes: - slug: path:*.md',
  enabled: true,
  delaySec: 3
})

const syncHistories = ref<SyncRecord[]>([
  {
    id: 'sync-1',
    ruleName: '默认热门模式',
    fetched: 8,
    stored: 6,
    skipped: 2,
    finishedAt: '2024-03-22 09:36',
    status: 'success'
  }
])

const ruleKeyword = ref('')
const statusMessage = ref('')

const filteredRules = computed(() => {
  const keyword = ruleKeyword.value.trim().toLowerCase()
  if (!keyword) return githubRules.value
  return githubRules.value.filter((rule) => {
    return rule.name.toLowerCase().includes(keyword) || rule.query.toLowerCase().includes(keyword)
  })
})

function handleTestToken() {
  statusMessage.value = '已模拟验证 GitHub Token，可安全调用接口'
  setTimeout(() => {
    statusMessage.value = ''
  }, 3000)
}

function resetDraft() {
  ruleDraft.name = ''
  ruleDraft.query = ''
  ruleDraft.pathHint = 'customModes: - slug: path:*.md'
  ruleDraft.enabled = true
  ruleDraft.delaySec = githubSettings.delay
}

async function handleSaveRule() {
  if (!ruleDraft.name || !ruleDraft.query) {
    statusMessage.value = '请填写规则名称与搜索语句'
    return
  }

  const payload: GithubRuleEntity = {
    id: '',
    name: ruleDraft.name,
    query: ruleDraft.query,
    pathHint: ruleDraft.pathHint,
    enabled: ruleDraft.enabled,
    delaySec: ruleDraft.delaySec,
    lastRunAt: null
  }

  try {
    await modeStore.saveGithubRule(payload)
    resetDraft()
    statusMessage.value = '规则已保存，待后端实现后可立即调用'
    setTimeout(() => (statusMessage.value = ''), 3000)
  } catch (err) {
    statusMessage.value = err instanceof Error ? err.message : String(err)
  }
}

onMounted(() => {
  modeStore.bootstrap().catch(() => {
    statusMessage.value = '加载规则失败，请稍后重试'
  })
})
</script>

<template>
  <div class="space-y-6 p-6">
    <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
      <header class="flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold text-gray-900">GitHub Key 管理</h2>
          <p class="text-sm text-gray-500">支持代理、调用延时，避免触发限流</p>
        </div>
        <button class="rounded-md border border-gray-200 px-3 py-2 text-sm text-gray-700">
          从系统密钥链导入
        </button>
      </header>
      <div class="mt-4 grid gap-4 md:grid-cols-2">
        <label class="text-sm text-gray-700">
          Personal Access Token
          <input
            v-model="githubSettings.token"
            type="password"
            placeholder="ghp_xxx"
            class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm"
          />
        </label>
        <label class="text-sm text-gray-700">
          调用延时（秒）
          <input
            v-model.number="githubSettings.delay"
            type="number"
            min="1"
            class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm"
          />
        </label>
        <label class="text-sm text-gray-700">
          代理地址（可选）
          <input
            v-model="githubSettings.proxyUrl"
            type="text"
            placeholder="http://127.0.0.1:7890"
            class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm"
          />
        </label>
        <label class="flex items-center gap-2 text-sm text-gray-700">
          <input v-model="githubSettings.enableProxy" type="checkbox" class="rounded border-gray-300" />
          启用代理
        </label>
      </div>
      <div class="mt-4 flex flex-wrap gap-3">
        <button @click="handleTestToken" class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white">测试 Token</button>
        <button class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700">保存配置</button>
        <p v-if="statusMessage" class="text-sm text-blue-600">{{ statusMessage }}</p>
      </div>
    </section>

    <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
      <header class="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
        <div>
          <h2 class="text-lg font-semibold text-gray-900">GitHub 搜索规则</h2>
          <p class="text-sm text-gray-500">支持多条规则并行执行，结果先入库再处理</p>
        </div>
        <input
          v-model="ruleKeyword"
          type="text"
          placeholder="搜索规则"
          class="rounded-md border border-gray-200 px-3 py-2 text-sm md:w-64"
        />
      </header>

      <div class="mt-4 overflow-hidden rounded-lg border border-gray-100" v-if="filteredRules.length">
        <table class="min-w-full divide-y divide-gray-100">
          <thead class="bg-gray-50">
            <tr>
              <th class="px-4 py-2 text-left text-xs font-medium text-gray-500">规则</th>
              <th class="px-4 py-2 text-left text-xs font-medium text-gray-500">查询语句</th>
              <th class="px-4 py-2 text-left text-xs font-medium text-gray-500">路径处理</th>
              <th class="px-4 py-2 text-left text-xs font-medium text-gray-500">状态</th>
              <th class="px-4 py-2 text-left text-xs font-medium text-gray-500">上次执行</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-gray-100 bg-white">
            <tr v-for="rule in filteredRules" :key="rule.id" class="hover:bg-gray-50">
              <td class="px-4 py-3">
                <p class="text-sm font-medium text-gray-900">{{ rule.name }}</p>
                <p class="text-xs text-gray-400">{{ rule.id }}</p>
              </td>
              <td class="px-4 py-3 text-sm text-gray-600">{{ rule.query }}</td>
              <td class="px-4 py-3 text-sm text-gray-600">{{ rule.pathHint }}</td>
              <td class="px-4 py-3 text-sm">
                <span
                  :class="[
                    'rounded-full px-2 py-1 text-xs',
                    rule.enabled ? 'bg-green-50 text-green-600' : 'bg-gray-50 text-gray-500'
                  ]"
                >
                  {{ rule.enabled ? '启用' : '停用' }}
                </span>
              </td>
              <td class="px-4 py-3 text-sm text-gray-600">{{ rule.lastRunAt || '-' }}</td>
            </tr>
          </tbody>
        </table>
      </div>
      <p v-else class="mt-2 text-sm text-gray-500">暂无规则，可通过下方表单添加。</p>

      <div class="mt-6 rounded-lg border border-dashed border-blue-200 bg-blue-50/40 p-4">
        <p class="text-sm font-semibold text-gray-700">新增规则</p>
        <div class="mt-3 grid gap-3 md:grid-cols-2">
          <label class="text-xs text-gray-600">
            规则名称
            <input v-model="ruleDraft.name" type="text" class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm" />
          </label>
          <label class="text-xs text-gray-600">
            搜索语句
            <input v-model="ruleDraft.query" type="text" class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm" />
          </label>
          <label class="text-xs text-gray-600 md:col-span-2">
            自定义解析提示（保留 GitHub 原始字段）
            <textarea
              v-model="ruleDraft.pathHint"
              rows="2"
              class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm"
            ></textarea>
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
          <label class="flex items-center gap-2 text-xs text-gray-600">
            <input v-model="ruleDraft.enabled" type="checkbox" class="rounded border-gray-300" />
            保存后立即启用
          </label>
          <div class="flex justify-end md:col-span-2">
            <button @click="handleSaveRule" class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white">
              保存规则
            </button>
          </div>
        </div>
      </div>
    </section>

    <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
      <header class="flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold text-gray-900">同步记录</h2>
          <p class="text-sm text-gray-500">每次同步先写入本地数据库，再调用对比逻辑</p>
        </div>
        <button class="rounded-md border border-gray-200 px-3 py-2 text-sm text-gray-700">导出日志</button>
      </header>

      <div class="mt-4 space-y-3">
        <article
          v-for="record in syncHistories"
          :key="record.id"
          class="rounded-lg border border-gray-100 bg-gray-50/80 p-4"
        >
          <div class="flex flex-wrap items-center justify-between gap-3">
            <div>
              <p class="text-sm font-semibold text-gray-900">{{ record.ruleName }}</p>
              <p class="text-xs text-gray-500">{{ record.finishedAt }}</p>
            </div>
            <span
              :class="[
                'rounded-full px-3 py-1 text-xs font-medium',
                record.status === 'success'
                  ? 'bg-green-50 text-green-600'
                  : record.status === 'warning'
                    ? 'bg-yellow-50 text-yellow-700'
                    : 'bg-red-50 text-red-600'
              ]"
            >
              {{ record.status }}
            </span>
          </div>
          <dl class="mt-3 grid grid-cols-3 gap-2 text-xs text-gray-600">
            <div>
              <dt class="text-gray-400">抓取</dt>
              <dd class="text-sm font-semibold text-gray-900">{{ record.fetched }}</dd>
            </div>
            <div>
              <dt class="text-gray-400">入库</dt>
              <dd class="text-sm font-semibold text-gray-900">{{ record.stored }}</dd>
            </div>
            <div>
              <dt class="text-gray-400">跳过（字段缺失或重复）</dt>
              <dd class="text-sm font-semibold text-gray-900">{{ record.skipped }}</dd>
            </div>
          </dl>
        </article>
      </div>
    </section>
  </div>
</template>
