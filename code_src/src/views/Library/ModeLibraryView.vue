<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { storeToRefs } from 'pinia'
import { useModeStore } from '../../stores/modes'

interface CompareItem {
  slug: string
  inKiloCode: boolean
  inRooCode: boolean
}

const activeTab = ref<'modes' | 'active' | 'compare'>('modes')
const modeStore = useModeStore()
const { modes, ideInstances, githubRules, loading, error } = storeToRefs(modeStore)

const summaryCards = computed(() => [
  { title: '本地模式', value: modes.value.length, tip: '合并去重后的模式数量' },
  { title: 'IDE 实例', value: ideInstances.value.length, tip: '当前识别到的 Kilo/Roo 实例' },
  { title: 'GitHub 规则', value: githubRules.value.length, tip: '可复用的搜索规则' }
])

const compareList = ref<CompareItem[]>([
  { slug: 'mode-review-pro', inKiloCode: true, inRooCode: false },
  { slug: 'mode-performance', inKiloCode: false, inRooCode: true },
  { slug: 'mode-full-stack', inKiloCode: true, inRooCode: true }
])

const filterKeyword = ref('')

const filteredModes = computed(() => {
  const keyword = filterKeyword.value.trim().toLowerCase()
  if (!keyword) return modes.value
  return modes.value.filter(
    (mode) =>
      mode.name.toLowerCase().includes(keyword) ||
      mode.groups.some((group) => group.toLowerCase().includes(keyword)) ||
      mode.slug.toLowerCase().includes(keyword)
  )
})

onMounted(() => {
  modeStore.bootstrap().catch(() => {
    /* store 已记录错误 */
  })
})
</script>

<template>
  <div class="p-6 space-y-6">
    <!-- 顶部信息概览 -->
    <section class="grid gap-4 md:grid-cols-3">
      <div
        v-for="card in summaryCards"
        :key="card.title"
        class="rounded-lg border border-gray-200 bg-white p-4 shadow-sm"
      >
        <div class="text-sm text-gray-500">{{ card.title }}</div>
        <div class="mt-2 text-3xl font-semibold text-gray-900">{{ card.value }}</div>
        <p class="mt-1 text-xs text-gray-400">{{ card.tip }}</p>
      </div>
    </section>

    <div v-if="loading" class="rounded-lg border border-dashed border-gray-300 bg-white/80 p-6 text-center text-sm text-gray-500">
      正在加载本地数据库数据...
    </div>
    <div v-else-if="error" class="rounded-lg border border-red-200 bg-red-50 p-4 text-sm text-red-600">
      {{ error }}
    </div>

    <!-- 标签切换 -->
    <div class="rounded-lg border border-gray-200 bg-white shadow-sm">
      <div class="flex border-b border-gray-100">
        <button
          v-for="tab in [
            { key: 'modes', label: '模式列表' },
            { key: 'active', label: '当前生效模式' },
            { key: 'compare', label: '模式对比' },
          ]"
          :key="tab.key"
          @click="activeTab = tab.key as typeof activeTab"
          class="flex-1 px-4 py-3 text-center text-sm font-medium"
          :class="[
            activeTab === tab.key
              ? 'border-b-2 border-blue-500 text-blue-600'
              : 'text-gray-500 hover:text-gray-700',
          ]"
        >
          {{ tab.label }}
        </button>
      </div>

      <div class="p-4">
        <!-- 模式列表 -->
        <div v-if="activeTab === 'modes'" class="space-y-4">
          <div class="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
            <div>
              <h2 class="text-lg font-semibold text-gray-900">本地模式库</h2>
              <p class="text-sm text-gray-500">从数据库读取的所有模式，可批量同步到不同实例</p>
            </div>
            <div class="flex gap-3">
              <input
                v-model="filterKeyword"
                type="text"
                placeholder="搜索名称或分组关键词"
                class="w-full rounded-md border border-gray-200 px-3 py-2 text-sm md:w-64"
              />
              <button class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white shadow">
                新建模式
              </button>
              <button class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700">
                批量导入
              </button>
            </div>
          </div>

          <div v-if="filteredModes.length" class="overflow-hidden rounded-lg border border-gray-100">
            <table class="min-w-full divide-y divide-gray-100">
              <thead class="bg-gray-50">
                <tr>
                  <th class="px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">
                    模式名称
                  </th>
                  <th class="px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">
                    来源
                  </th>
                  <th class="px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">
                    分组
                  </th>
                  <th class="px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">
                    更新时间
                  </th>
                  <th class="px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">
                    roleDefinition 长度
                  </th>
                  <th class="px-4 py-3 text-right text-xs font-medium uppercase tracking-wider text-gray-500">
                    操作
                  </th>
                </tr>
              </thead>
              <tbody class="divide-y divide-gray-100 bg-white">
                <tr v-for="mode in filteredModes" :key="mode.id" class="hover:bg-gray-50">
                  <td class="px-4 py-3">
                    <div class="font-medium text-gray-900">{{ mode.name }}</div>
                    <div class="text-xs text-gray-400">slug: {{ mode.slug }}</div>
                  </td>
                  <td class="px-4 py-3 text-sm text-gray-600">{{ mode.source }}</td>
                  <td class="px-4 py-3">
                    <div class="flex flex-wrap gap-2">
                      <span
                        v-for="group in mode.groups"
                        :key="group"
                        class="rounded-full bg-blue-50 px-2 py-1 text-xs text-blue-600"
                      >
                        {{ group }}
                      </span>
                    </div>
                  </td>
                  <td class="px-4 py-3 text-sm text-gray-600">{{ mode.updatedAt }}</td>
                  <td class="px-4 py-3 text-sm text-gray-600">
                    {{ mode.roleDefinitionLength }} 字
                  </td>
                  <td class="px-4 py-3 text-right text-sm text-gray-600">
                    <div class="flex justify-end gap-2">
                      <button class="rounded-md border border-gray-200 px-3 py-1 hover:border-blue-500 hover:text-blue-600">
                        同步
                      </button>
                      <button class="rounded-md border border-gray-200 px-3 py-1 hover:border-blue-500 hover:text-blue-600">
                        编辑
                      </button>
                    </div>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
          <p v-else class="text-sm text-gray-500">暂无模式，请先通过 GitHub 同步或 IDE 扫描入库。</p>
        </div>

        <!-- 当前生效模式 -->
        <div v-else-if="activeTab === 'active'" class="space-y-4">
          <h2 class="text-lg font-semibold text-gray-900">当前生效模式</h2>
          <p class="text-sm text-gray-500">实时读取每个实例的配置，可直接编辑后写回</p>

          <div class="grid gap-4 md:grid-cols-2" v-if="ideInstances.length">
            <div
              v-for="instance in ideInstances"
              :key="instance.id"
              class="rounded-lg border border-gray-200 bg-white p-4 shadow-sm"
            >
              <div class="flex items-center justify-between">
                <div>
                  <h3 class="text-base font-semibold text-gray-900">{{ instance.alias }}</h3>
                  <p class="text-xs text-gray-500">识别路径：{{ instance.path }}</p>
                </div>
                <span
                  :class="[
                    'rounded-full px-3 py-1 text-xs',
                    instance.status === 'synced'
                      ? 'bg-green-50 text-green-600'
                      : instance.status === 'outdated'
                        ? 'bg-yellow-50 text-yellow-700'
                        : 'bg-red-50 text-red-600'
                  ]"
                >
                  {{
                    instance.status === 'synced'
                      ? '已同步'
                      : instance.status === 'outdated'
                        ? '待检查'
                        : '未发现'
                  }}
                </span>
              </div>
              <p class="mt-3 text-xs text-gray-500">最近扫描：{{ instance.lastScanAt || '未扫描' }}</p>
              <div class="mt-4 flex gap-2">
                <button class="rounded-md border border-gray-200 px-3 py-2 text-sm text-gray-600">仅更新 IDE</button>
                <button class="rounded-md bg-blue-600 px-3 py-2 text-sm text-white">同步到本地库</button>
              </div>
            </div>
          </div>
          <p v-else class="text-sm text-gray-500">暂无识别到的实例，请前往 “IDE 配置” 页扫描。</p>
        </div>

        <!-- 模式对比 -->
        <div v-else class="space-y-6">
          <h2 class="text-lg font-semibold text-gray-900">跨软件模式对比</h2>
          <p class="text-sm text-gray-500">对比两个软件的模式差异，可将单个模式复制到另一端</p>

          <div class="overflow-hidden rounded-lg border border-gray-100">
            <table class="min-w-full divide-y divide-gray-100">
              <thead class="bg-gray-50">
                <tr>
                  <th class="px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">
                    模式 Slug
                  </th>
                  <th class="px-4 py-3 text-center text-xs font-medium uppercase tracking-wider text-gray-500">
                    KiloCode
                  </th>
                  <th class="px-4 py-3 text-center text-xs font-medium uppercase tracking-wider text-gray-500">
                    RooCode
                  </th>
                  <th class="px-4 py-3 text-right text-xs font-medium uppercase tracking-wider text-gray-500">
                    操作
                  </th>
                </tr>
              </thead>
              <tbody class="divide-y divide-gray-100 bg-white">
                <tr v-for="item in compareList" :key="item.slug" class="hover:bg-gray-50">
                  <td class="px-4 py-3 font-medium text-gray-900">{{ item.slug }}</td>
                  <td class="px-4 py-3 text-center">
                    <span
                      :class="[
                        'rounded-full px-2 py-1 text-xs font-medium',
                        item.inKiloCode
                          ? 'bg-green-50 text-green-600'
                          : 'bg-gray-50 text-gray-400',
                      ]"
                    >
                      {{ item.inKiloCode ? '存在' : '缺失' }}
                    </span>
                  </td>
                  <td class="px-4 py-3 text-center">
                    <span
                      :class="[
                        'rounded-full px-2 py-1 text-xs font-medium',
                        item.inRooCode
                          ? 'bg-green-50 text-green-600'
                          : 'bg-gray-50 text-gray-400',
                      ]"
                    >
                      {{ item.inRooCode ? '存在' : '缺失' }}
                    </span>
                  </td>
                  <td class="px-4 py-3 text-right">
                    <div class="flex justify-end gap-2">
                      <button class="rounded-md border border-gray-200 px-3 py-1 text-sm text-gray-600">
                        推送到 KiloCode
                      </button>
                      <button class="rounded-md border border-gray-200 px-3 py-1 text-sm text-gray-600">
                        推送到 RooCode
                      </button>
                    </div>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
