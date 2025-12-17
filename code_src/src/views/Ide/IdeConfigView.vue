<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { storeToRefs } from 'pinia'
import { useRouter } from 'vue-router'
import { useModeStore, type IdeInstanceEntity } from '../../stores/modes'
import { formatDateTime } from '../../composables/useFormat'

type IdeType = 'kilocode' | 'roocode'

const router = useRouter()
const modeStore = useModeStore()
const { ideInstances } = storeToRefs(modeStore)

const knownPaths = [
  {
    label: 'VSCode KiloCode',
    path: '~/Library/Application Support/Code/User/globalStorage/kilocode.kilo-code/settings/custom_modes.yaml',
    type: 'kilocode' as IdeType
  },
  {
    label: 'Trae KiloCode（CN）',
    path: '~/Library/Application Support/Trae CN/User/globalStorage/kilocode.kilo-code/settings/custom_modes.yaml',
    type: 'kilocode' as IdeType
  },
  {
    label: 'Trae KiloCode（国际）',
    path: '~/Library/Application Support/Trae/User/globalStorage/kilocode.kilo-code/settings/custom_modes.yaml',
    type: 'kilocode' as IdeType
  },
  {
    label: 'VSCode 主版 RooCode',
    path: '~/Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/custom_modes.yaml',
    type: 'roocode' as IdeType
  },
  {
    label: 'Trae RooCode（国服）',
    path: '~/Library/Application Support/Trae CN/User/globalStorage/rooveterinaryinc.roo-cline/settings/custom_modes.yaml',
    type: 'roocode' as IdeType
  },
  {
    label: 'Trae RooCode（国际）',
    path: '~/Library/Application Support/Trae/User/globalStorage/rooveterinaryinc.roo-cline/settings/custom_modes.yaml',
    type: 'roocode' as IdeType
  }
]

const manualForm = reactive({
  alias: '',
  path: '',
  type: 'kilocode' as IdeType
})

const showEditModal = ref(false)
const editingInstance = ref<IdeInstanceEntity | null>(null)
const editForm = reactive({
  id: '',
  alias: '',
  path: '',
  type: 'kilocode' as IdeType,
  selected: false
})

const toast = ref('')

const totalInstances = computed(() => ideInstances.value.length)
const selectedInstances = computed(() => ideInstances.value.filter((item) => item.selected).length)

const statusSummary = computed(() => {
  const synced = ideInstances.value.filter((item) => item.status === 'synced').length
  const outdated = ideInstances.value.filter((item) => item.status === 'outdated').length
  const missing = ideInstances.value.filter((item) => item.status === 'missing').length
  const scanTimes = ideInstances.value.map((item) => item.lastScanAt).filter(Boolean).sort()
  const lastScanAt = scanTimes.length ? scanTimes[scanTimes.length - 1] : undefined
  return {
    total: totalInstances.value,
    selected: selectedInstances.value,
    synced,
    outdated,
    missing,
    lastScanAt
  }
})

async function handleScanKnownPaths() {
  toast.value = '正在扫描已知路径...'
  try {
    await modeStore.scanKnownInstances()
    toast.value = '扫描完成（白名单 + 自动发现），结果已写入数据库'
  } catch (err) {
    toast.value = err instanceof Error ? err.message : String(err)
  } finally {
    setTimeout(() => (toast.value = ''), 2500)
  }
}

async function handleScanAllInstances() {
  toast.value = '正在全量扫描...'
  try {
    await modeStore.scanAllInstances()
    toast.value = '全量扫描完成（含手动实例），结果已写入数据库'
  } catch (err) {
    toast.value = err instanceof Error ? err.message : String(err)
  } finally {
    setTimeout(() => (toast.value = ''), 2500)
  }
}

async function handleToggleSelection(target: IdeInstanceEntity, selected: boolean) {
  await modeStore.saveIdeInstance({ ...target, selected })
}

function formatStatusLabel(status: IdeInstanceEntity['status']) {
  if (status === 'synced') return '已查'
  if (status === 'outdated') return '待查'
  return '未找到文件'
}

async function handleAddInstance() {
  if (!manualForm.alias.trim() || !manualForm.path.trim()) {
    toast.value = '请填写别名与路径'
    return
  }

  const payload: IdeInstanceEntity = {
    id: '',
    alias: manualForm.alias.trim(),
    kind: manualForm.type,
    path: manualForm.path.trim(),
    lastScanAt: null,
    modesCount: 0,
    status: 'outdated',
    selected: false
  }

  try {
    await modeStore.saveIdeInstance(payload)
    manualForm.alias = ''
    manualForm.path = ''
    manualForm.type = 'kilocode'
    toast.value = '实例已保存，可在后端扫描后自动识别模式'
    setTimeout(() => (toast.value = ''), 2500)
  } catch (err) {
    toast.value = err instanceof Error ? err.message : String(err)
  }
}

function openEditModal(instance: IdeInstanceEntity) {
  editingInstance.value = instance
  editForm.id = instance.id
  editForm.alias = instance.alias
  editForm.path = instance.path
  editForm.type = instance.kind
  editForm.selected = instance.selected
  showEditModal.value = true
}

function closeEditModal() {
  showEditModal.value = false
  editingInstance.value = null
}

async function handleSaveEdit() {
  if (!editForm.alias.trim() || !editForm.path.trim()) {
    toast.value = '请填写别名与路径'
    setTimeout(() => (toast.value = ''), 2500)
    return
  }

  try {
    const original = editingInstance.value
    const normalizedPath = editForm.path.trim()
    const shouldResetScan =
      !original || normalizedPath !== original.path || editForm.type !== original.kind
    await modeStore.saveIdeInstance({
      id: editForm.id,
      alias: editForm.alias.trim(),
      kind: editForm.type,
      path: normalizedPath,
      lastScanAt: shouldResetScan ? null : original.lastScanAt ?? null,
      modesCount: shouldResetScan ? 0 : original.modesCount ?? 0,
      status: shouldResetScan ? 'outdated' : original.status ?? 'outdated',
      selected: editForm.selected
    })
    toast.value = '已保存实例信息'
    closeEditModal()
  } catch (err) {
    toast.value = err instanceof Error ? err.message : String(err)
  } finally {
    setTimeout(() => (toast.value = ''), 2500)
  }
}

async function handleDeleteInstance(instance: IdeInstanceEntity) {
  const ok = window.confirm(`确认删除实例「${instance.alias}」？`)
  if (!ok) return
  toast.value = '正在删除...'
  try {
    await modeStore.deleteIdeInstance(instance.id)
    toast.value = '已删除'
  } catch (err) {
    toast.value = err instanceof Error ? err.message : String(err)
  } finally {
    setTimeout(() => (toast.value = ''), 2500)
  }
}

async function handleRescanInstance(instance: IdeInstanceEntity) {
  toast.value = `正在扫描：${instance.alias}...`
  try {
    await modeStore.scanInstanceModes(instance.id)
    toast.value = '扫描完成'
  } catch (err) {
    toast.value = err instanceof Error ? err.message : String(err)
  } finally {
    setTimeout(() => (toast.value = ''), 2500)
  }
}

async function handleRefreshList() {
  toast.value = '正在刷新列表...'
  try {
    await modeStore.bootstrap(true)
    toast.value = '已刷新'
  } catch (err) {
    toast.value = err instanceof Error ? err.message : String(err)
  } finally {
    setTimeout(() => (toast.value = ''), 2000)
  }
}

onMounted(() => {
  modeStore.bootstrap().catch(() => {
    toast.value = '加载实例失败，请稍后重试'
  })
})
</script>

<template>
  <div class="space-y-2 p-2">
    <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
      <header class="flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold text-gray-900">实例识别</h2>
          <p class="text-sm text-gray-500">程序启动时优先扫描常见的程序安装路径，可手动添
加自定义实例</p>
        </div>
        <div class="flex flex-wrap gap-2">
          <button class="rounded-md border border-gray-200 px-3 py-2 text-sm text-gray-700" @click="handleScanKnownPaths">
            扫描白名单路径
          </button>
          <button class="rounded-md border border-gray-200 px-3 py-2 text-sm text-gray-700" @click="handleScanAllInstances">
            全量扫描
          </button>
        </div>
      </header>

      <div class="mt-4 grid gap-4 lg:grid-cols-3">
        <div class="rounded-lg border border-gray-100 bg-gray-50/80 p-4">
          <p class="text-xs text-gray-500">已识别实例</p>
          <p class="text-3xl font-semibold text-gray-900">{{ statusSummary.total }}</p>
        </div>
        <div class="rounded-lg border border-gray-100 bg-gray-50/80 p-4">
          <p class="text-xs text-gray-500">已勾选同步</p>
          <p class="text-3xl font-semibold text-blue-600">
            {{ statusSummary.selected }}
          </p>
        </div>
        <div class="rounded-lg border border-gray-100 bg-gray-50/80 p-4">
          <p class="text-xs text-gray-500">实例状态</p>
          <p class="text-sm font-semibold text-gray-900">最新 {{ statusSummary.synced }} / 待检查 {{ statusSummary.outdated }} / 缺失 {{ statusSummary.missing }}</p>
          <p class="mt-1 text-xs text-gray-500">最近扫描：{{ statusSummary.lastScanAt ? formatDateTime(statusSummary.lastScanAt) : '未扫描' }}</p>
        </div>
      </div>

      <div class="mt-6 overflow-hidden rounded-lg border border-gray-100" v-if="ideInstances.length">
        <table class="w-full table-fixed divide-y divide-gray-200">
          <thead class="bg-gray-50">
            <tr>
              <th class="w-8 px-4 py-2 text-left text-xs font-semibold text-gray-500"></th>
              <th class="w-36 px-4 py-2 text-left text-xs font-semibold text-gray-500">别名</th>
              <th class="w-20 px-4 py-2 text-left text-xs font-semibold text-gray-500">类型</th>
              <th class="px-4 py-2 text-left text-xs font-semibold text-gray-500">路径</th>
              <th class="w-20 px-4 py-2 text-left text-xs font-semibold text-gray-500">状态</th>
              <th class="w-14 px-4 py-2 text-left text-xs font-semibold text-gray-500">模式</th>
              <th class="w-28 px-4 py-2 text-right text-xs font-semibold text-gray-500">操作</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-gray-100 bg-white">
            <tr v-for="instance in ideInstances" :key="instance.id" class="hover:bg-gray-50">
              <td class="px-4 py-3 text-sm">
                <input
                  :checked="instance.selected"
                  type="checkbox"
                  class="rounded border-gray-300"
                  @change="handleToggleSelection(instance, !instance.selected)"
                />
              </td>
              <td class="px-4 py-3">
                <p class="text-sm font-semibold text-gray-900">{{ instance.alias }}</p>
                <p class="text-xs text-gray-400">{{ formatDateTime(instance.lastScanAt) }}</p>
              </td>
              <td class="px-4 py-3 text-xs uppercase text-gray-500">
                {{ instance.kind === 'kilocode' ? 'Kilo' : 'Roo' }}
              </td>
              <td class="min-w-0 px-4 py-3 text-xs font-mono text-gray-600">
                <div class="text-clip" :title="instance.path">{{ instance.path }}</div>
              </td>
              <td class="px-4 py-3 text-sm">
                <span
                  :class="[
                    'rounded-full px-2 py-1 text-xs font-medium truncate',
                    instance.status === 'synced'
                      ? 'bg-green-50 text-green-600'
                      : instance.status === 'outdated'
                        ? 'bg-yellow-50 text-yellow-700'
                        : 'bg-red-50 text-red-600'
                  ]"
                >
                  {{ formatStatusLabel(instance.status) }}
                </span>
              </td>
              <td class="px-4 py-3 text-sm text-gray-700">{{ instance.modesCount }}</td>
              <td class="px-4 py-3 text-right">
                <div class="flex justify-end gap-2">
                  <button
                    class="rounded-md border border-gray-200 p-1.5 text-gray-700 hover:border-blue-500 hover:text-blue-600"
                    title="重新扫描"
                    aria-label="重新扫描"
                    @click="handleRescanInstance(instance)"
                  >
                    <svg viewBox="0 0 20 20" fill="currentColor" class="h-4 w-4">
                      <path
                        fill-rule="evenodd"
                        d="M4.75 10a5.25 5.25 0 019.6-2.95l.48-.82a.75.75 0 011.42.38v3.5a.75.75 0 01-.75.75H12a.75.75 0 01-.58-1.22l.69-.85A3.75 3.75 0 106.25 10a.75.75 0 01-1.5 0z"
                        clip-rule="evenodd"
                      />
                    </svg>
                  </button>
                  <button
                    class="rounded-md border border-gray-200 p-1.5 text-gray-700 hover:border-blue-500 hover:text-blue-600"
                    title="编辑"
                    aria-label="编辑"
                    @click="openEditModal(instance)"
                  >
                    <svg viewBox="0 0 20 20" fill="currentColor" class="h-4 w-4">
                      <path
                        d="M13.586 3.586a2 2 0 012.828 2.828l-8.5 8.5a1 1 0 01-.39.242l-3 1a1 1 0 01-1.265-1.265l1-3a1 1 0 01.242-.39l8.5-8.5z"
                      />
                    </svg>
                  </button>
                  <button
                    class="rounded-md border border-red-200 p-1.5 text-red-600 hover:border-red-400"
                    title="删除"
                    aria-label="删除"
                    @click="handleDeleteInstance(instance)"
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
      <div v-else class="mt-4 rounded-lg border border-dashed border-gray-200 bg-white p-4 text-sm text-gray-600">
        <p>暂无实例，请先扫描默认路径或手动添加。</p>
        <div class="mt-3 flex flex-wrap gap-3">
          <button class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white" @click="handleScanKnownPaths">立即扫描白名单路径</button>
          <button class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700" @click="handleScanAllInstances">全量扫描</button>
        </div>
      </div>

      <div class="mt-4 flex flex-wrap gap-3">
        <button class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white" @click="router.push('/library')">去库管理同步模式</button>
        <button class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700" @click="handleRefreshList">仅刷新当前列表</button>
        <p v-if="toast" class="text-sm text-blue-600">{{ toast }}</p>
      </div>
    </section>

    <div
      v-if="showEditModal"
      class="fixed inset-0 z-30 flex items-center justify-center bg-black/30 backdrop-blur-sm"
      aria-modal="true"
      role="dialog"
    >
      <div class="w-full max-w-lg rounded-lg bg-white shadow-xl">
        <header class="flex items-center justify-between border-b border-gray-100 px-5 py-4">
          <div>
            <h3 class="text-lg font-semibold text-gray-900">编辑实例</h3>
            <p class="text-sm text-gray-500">修改别名或路径后建议重新扫描</p>
          </div>
          <button @click="closeEditModal" class="text-gray-400 hover:text-gray-600">✕</button>
        </header>
        <div class="grid gap-4 px-5 py-4">
          <label class="text-sm text-gray-700">
            实例别名
            <input v-model="editForm.alias" type="text" class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm" />
          </label>
          <label class="text-sm text-gray-700">
            类型
            <select v-model="editForm.type" class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm">
              <option value="kilocode">KiloCode</option>
              <option value="roocode">RooCode</option>
            </select>
          </label>
          <label class="text-sm text-gray-700">
            配置文件路径
            <input v-model="editForm.path" type="text" class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm" />
          </label>
          <label class="flex items-center gap-2 text-sm text-gray-700">
            <input v-model="editForm.selected" type="checkbox" class="rounded border-gray-300" />
            勾选为同步目标
          </label>
        </div>
        <footer class="flex justify-end gap-3 border-t border-gray-100 px-5 py-4">
          <button class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700" @click="closeEditModal">取消</button>
          <button class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white" @click="handleSaveEdit">保存</button>
        </footer>
      </div>
    </div>

    <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
      <header>
        <h2 class="text-lg font-semibold text-gray-900">快速新增实例</h2>
        <p class="text-sm text-gray-500">适配路径中含空格的场景，后端写入前会自动转义</p>
      </header>

      <div class="mt-4 grid gap-4 md:grid-cols-1">
        <label class="text-sm text-gray-700">
          实例别名
          <input
            v-model="manualForm.alias"
            type="text"
            placeholder="KiloCode - Windsurf"
            class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm"
          />
        </label>
        <label class="text-sm text-gray-700">
          类型
          <select v-model="manualForm.type" class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm">
            <option value="kilocode">KiloCode</option>
            <option value="roocode">RooCode</option>
          </select>
        </label>
        <label class="text-sm text-gray-700 md:col-span-2">
          配置文件路径
          <input
            v-model="manualForm.path"
            type="text"
            placeholder="~/Library/Application Support/Code/User/..."
            class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm"
          />
        </label>
        <!-- <div class="text-xs text-gray-500 md:col-span-2">
          <p>常见路径：</p>
          <ul class="mt-1 list-disc space-y-1 pl-5">
            <li v-for="item in knownPaths" :key="item.path">
              {{ item.label }} — <span class="font-mono text-gray-600">{{ item.path }}</span>
            </li>
          </ul>
        </div> -->
        <div class="md:col-span-2">
          <button @click="handleAddInstance" class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white">
            保存实例
          </button>
        </div>
      </div>
    </section>
  </div>
</template>
