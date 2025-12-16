<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { storeToRefs } from 'pinia'
import { useModeStore, type IdeInstanceEntity } from '../../stores/modes'

type IdeType = 'kilocode' | 'roocode'

const modeStore = useModeStore()
const { ideInstances } = storeToRefs(modeStore)

const knownPaths = [
  {
    label: 'VSCode 主版 KiloCode',
    path: '~/Library/Application Support/Code/User/globalStorage/kilocode.kilo-code/settings/custom_modes.yaml',
    type: 'kilocode' as IdeType
  },
  {
    label: 'Trae KiloCode（国服）',
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

const toast = ref('')

const totalInstances = computed(() => ideInstances.value.length)
const selectedInstances = computed(() => ideInstances.value.filter((item) => item.selected).length)

const syncSummary = computed(() => ({
  total: totalInstances.value,
  selected: selectedInstances.value,
  lastRun: '2024-03-21 22:30',
  lastResult: '全部实例写入成功'
}))

async function handleScanKnownPaths() {
  toast.value = '正在扫描已知路径...'
  try {
    await modeStore.scanKnownInstances()
    toast.value = '扫描完成，结果已写入数据库'
  } catch (err) {
    toast.value = err instanceof Error ? err.message : String(err)
  } finally {
    setTimeout(() => (toast.value = ''), 2500)
  }
}

async function handleToggleSelection(target: IdeInstanceEntity, selected: boolean) {
  await modeStore.saveIdeInstance({ ...target, selected })
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

onMounted(() => {
  modeStore.bootstrap().catch(() => {
    toast.value = '加载实例失败，请稍后重试'
  })
})
</script>

<template>
  <div class="space-y-6 p-6">
    <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
      <header class="flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold text-gray-900">实例识别</h2>
          <p class="text-sm text-gray-500">程序启动时优先扫描白名单路径，可手动添加自定义实例</p>
        </div>
        <button class="rounded-md border border-gray-200 px-3 py-2 text-sm text-gray-700" @click="handleScanKnownPaths">
          扫描白名单路径
        </button>
      </header>

      <div class="mt-4 grid gap-4 lg:grid-cols-3">
        <div class="rounded-lg border border-gray-100 bg-gray-50/80 p-4">
          <p class="text-xs text-gray-500">已识别实例</p>
          <p class="text-3xl font-semibold text-gray-900">{{ syncSummary.total }}</p>
        </div>
        <div class="rounded-lg border border-gray-100 bg-gray-50/80 p-4">
          <p class="text-xs text-gray-500">已勾选同步</p>
          <p class="text-3xl font-semibold text-blue-600">
            {{ syncSummary.selected }}
          </p>
        </div>
        <div class="rounded-lg border border-gray-100 bg-gray-50/80 p-4">
          <p class="text-xs text-gray-500">上次同步</p>
          <p class="text-lg font-semibold text-gray-900">{{ syncSummary.lastRun }}</p>
          <p class="text-xs text-gray-500">{{ syncSummary.lastResult }}</p>
        </div>
      </div>

      <div class="mt-6 overflow-hidden rounded-lg border border-gray-100" v-if="ideInstances.length">
        <table class="min-w-full divide-y divide-gray-200">
          <thead class="bg-gray-50">
            <tr>
              <th class="px-4 py-2 text-left text-xs font-semibold text-gray-500">勾选</th>
              <th class="px-4 py-2 text-left text-xs font-semibold text-gray-500">别名</th>
              <th class="px-4 py-2 text-left text-xs font-semibold text-gray-500">类型</th>
              <th class="px-4 py-2 text-left text-xs font-semibold text-gray-500">路径</th>
              <th class="px-4 py-2 text-left text-xs font-semibold text-gray-500">状态</th>
              <th class="px-4 py-2 text-left text-xs font-semibold text-gray-500">包含模式</th>
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
                <p class="text-xs text-gray-400">{{ instance.lastScanAt || '未扫描' }}</p>
              </td>
              <td class="px-4 py-3 text-xs uppercase text-gray-500">
                {{ instance.kind === 'kilocode' ? 'KiloCode' : 'RooCode' }}
              </td>
              <td class="px-4 py-3 text-xs font-mono text-gray-600">{{ instance.path }}</td>
              <td class="px-4 py-3 text-sm">
                <span
                  :class="[
                    'rounded-full px-2 py-1 text-xs font-medium',
                    instance.status === 'synced'
                      ? 'bg-green-50 text-green-600'
                      : instance.status === 'outdated'
                        ? 'bg-yellow-50 text-yellow-700'
                        : 'bg-red-50 text-red-600'
                  ]"
                >
                  {{
                    instance.status === 'synced'
                      ? '最新'
                      : instance.status === 'outdated'
                        ? '待同步'
                        : '未发现文件'
                  }}
                </span>
              </td>
              <td class="px-4 py-3 text-sm text-gray-700">{{ instance.modesCount }}</td>
            </tr>
          </tbody>
        </table>
      </div>
      <p v-else class="mt-4 text-sm text-gray-500">暂无实例，请扫描默认路径或手动添加。</p>

      <div class="mt-4 flex flex-wrap gap-3">
        <button class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white">同步到勾选实例</button>
        <button class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700">仅刷新当前列表</button>
        <p v-if="toast" class="text-sm text-blue-600">{{ toast }}</p>
      </div>
    </section>

    <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
      <header>
        <h2 class="text-lg font-semibold text-gray-900">快速新增实例</h2>
        <p class="text-sm text-gray-500">适配路径中含空格的场景，后端写入前会自动转义</p>
      </header>

      <div class="mt-4 grid gap-4 md:grid-cols-2">
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
        <div class="text-xs text-gray-500 md:col-span-2">
          <p>常见路径：</p>
          <ul class="mt-1 list-disc space-y-1 pl-5">
            <li v-for="item in knownPaths" :key="item.path">
              {{ item.label }} — <span class="font-mono text-gray-600">{{ item.path }}</span>
            </li>
          </ul>
        </div>
        <div class="md:col-span-2">
          <button @click="handleAddInstance" class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white">
            保存实例
          </button>
        </div>
      </div>
    </section>
  </div>
</template>
