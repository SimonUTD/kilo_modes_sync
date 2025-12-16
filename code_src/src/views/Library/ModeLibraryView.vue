<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'
import { useRouter } from 'vue-router'
import {
  useModeStore,
  type IdeInstanceEntity,
  type InstanceModeDiffSummary,
  type ModeCompareItem,
  type ModeDiffPreview,
  type ModeEntity,
  type ModeImportReport,
  type ModeHistoryRecord,
  type ModeMetaRecord
} from '../../stores/modes'
import { formatDateTime } from '../../composables/useFormat'

const activeTab = ref<'modes' | 'active' | 'compare'>('modes')
const router = useRouter()
const modeStore = useModeStore()
const { modes, ideInstances, githubRules, appSettings, roleDefinitionThreshold, loading, error } = storeToRefs(modeStore)

const summaryCards = computed(() => [
  { title: '本地模式', value: modes.value.length, tip: '合并去重后的模式数量' },
  { title: 'IDE 实例', value: ideInstances.value.length, tip: '当前识别到的 Kilo/Roo 实例' },
  { title: 'GitHub 检索规则', value: githubRules.value.length, tip: '可用的搜索规则' }
])

const compareList = ref<ModeCompareItem[]>([])
const compareLoading = ref(false)
const compareError = ref<string | null>(null)

const filterKeyword = ref('')
const showQualityOnly = ref(false)
const showRoleDefinitionColumn = computed(() => appSettings.value?.showRoleDefinitionLength ?? true)
const showImportDrawer = ref(false)
const importError = ref<string | null>(null)
const importSuccess = ref<string | null>(null)
const importProgress = ref<{ total: number; success: number }>({ total: 0, success: 0 })
const importText = ref('')
const importPreview = ref<ModeDiffPreview | null>(null)
const importReport = ref<ModeImportReport | null>(null)
const importing = ref(false)
const importConflictStrategy = ref<'overwrite' | 'rename' | 'skip'>('rename')

const showModeEditor = ref(false)
const editorMessage = ref<string | null>(null)
const editingMode = ref<ModeEntity | null>(null)
const editingModeMeta = ref<ModeMetaRecord | null>(null)
const editorShowRawPayload = ref(false)
const modeEditorTarget = ref<'db' | 'instance'>('db')
const modeEditorInstanceId = ref<string | null>(null)
const modeEditorSaveToDb = ref(true)
const modeEditorConflictStrategy = ref<'overwrite' | 'rename' | 'skip'>('overwrite')
const instanceModeDraft = ref<Record<string, unknown> | null>(null)
const modeForm = reactive({
  slug: '',
  name: '',
  description: '',
  groupsText: '',
  roleDefinition: '',
  whenToUse: '',
  customInstructions: '',
  configSource: 'global'
})

const showApplyModal = ref(false)
const applyModeSlug = ref<string | null>(null)
const applyStrategy = ref<'overwrite' | 'rename' | 'skip'>('overwrite')
const applyInstanceIds = ref<string[]>([])
const applyResultMessage = ref<string | null>(null)
const applyKindFilter = ref<'all' | 'kilocode' | 'roocode'>('all')
const applyTargetInstances = computed(() => {
  if (applyKindFilter.value === 'all') return ideInstances.value
  return ideInstances.value.filter((item) => item.kind === applyKindFilter.value)
})
const applySelectedInstances = computed(() => applyTargetInstances.value.filter((item) => applyInstanceIds.value.includes(item.id)))
const applyCompareInstanceId = ref<string | null>(null)
const applyCompareIdeRaw = ref<Record<string, unknown> | null>(null)
const applyCompareLoading = ref(false)
const applyCompareError = ref<string | null>(null)
const applyDbRaw = computed(() => {
  const slug = applyModeSlug.value
  if (!slug) return null
  const mode = modes.value.find((item) => item.slug === slug)
  if (!mode) return null
  const configSource = String((mode.payload as Record<string, unknown> | null)?.configSource ?? 'global')
  const raw: Record<string, unknown> = {
    slug: mode.slug,
    name: mode.name,
    description: mode.description,
    groups: mode.groups,
    roleDefinition: mode.roleDefinition,
    source: configSource
  }
  if (mode.whenToUse) raw.whenToUse = mode.whenToUse
  if (mode.customInstructions) raw.customInstructions = mode.customInstructions
  return raw
})
const activeMessage = ref<string | null>(null)

const showInstanceModesModal = ref(false)
const instanceModesLoading = ref(false)
const instanceModesError = ref<string | null>(null)
const activeInstance = ref<IdeInstanceEntity | null>(null)
const instanceModes = ref<{ slug: string; name?: string | null; raw: Record<string, unknown> }[]>([])

const showHistoryModal = ref(false)
const historyLoading = ref(false)
const historyError = ref<string | null>(null)
const historyItems = ref<ModeHistoryRecord[]>([])
const historySelectedId = ref<string | null>(null)
const historyReplayStrategy = ref<'overwrite' | 'rename' | 'skip'>('overwrite')
const historyReplaySaveToDb = ref(true)
const historyReplayMessage = ref<string | null>(null)
const historyStats = computed(() => {
  const counts = new Map<string, number>()
  for (const item of historyItems.value) {
    counts.set(item.action, (counts.get(item.action) ?? 0) + 1)
  }
  const items = Array.from(counts.entries())
    .map(([action, count]) => ({ action, count }))
    .sort((a, b) => b.count - a.count)
  const max = items[0]?.count ?? 0
  return {
    total: historyItems.value.length,
    max,
    latestAt: historyItems.value[0]?.createdAt ?? null,
    items
  }
})

const showDiffModal = ref(false)
const diffLoading = ref(false)
const diffError = ref<string | null>(null)
const diffSummary = ref<InstanceModeDiffSummary | null>(null)
const diffInstance = ref<IdeInstanceEntity | null>(null)
const diffMessage = ref<string | null>(null)
const diffApplyStrategy = ref<'overwrite' | 'rename' | 'skip'>('overwrite')

const filteredModes = computed(() => {
  const keyword = filterKeyword.value.trim().toLowerCase()
  let list = modes.value
  if (keyword) {
    list = list.filter(
    (mode) =>
      mode.name.toLowerCase().includes(keyword) ||
      mode.groups.some((group) => group.toLowerCase().includes(keyword)) ||
      mode.slug.toLowerCase().includes(keyword)
  )
  }
  if (showQualityOnly.value) {
    list = list.filter((item) => item.roleDefinitionLength >= roleDefinitionThreshold.value)
  }
  return list
})

function openImportDrawer() {
  importError.value = null
  importSuccess.value = null
  importProgress.value = { total: modes.value.length, success: 0 }
  importText.value = ''
  importPreview.value = null
  importReport.value = null
  importConflictStrategy.value = 'rename'
  showImportDrawer.value = true
}

function openModeEditor(mode?: ModeEntity) {
  editorMessage.value = null
  editingModeMeta.value = null
  editorShowRawPayload.value = false
  modeEditorTarget.value = 'db'
  modeEditorInstanceId.value = null
  modeEditorSaveToDb.value = true
  modeEditorConflictStrategy.value = 'overwrite'
  instanceModeDraft.value = null
  editingMode.value = mode ?? null
  if (mode) {
    modeForm.slug = mode.slug
    modeForm.name = mode.name
    modeForm.description = mode.description
    modeForm.groupsText = mode.groups.join(', ')
    modeForm.roleDefinition = mode.roleDefinition
    modeForm.whenToUse = mode.whenToUse ?? ''
    modeForm.customInstructions = mode.customInstructions ?? ''
    modeForm.configSource = String((mode.payload as Record<string, unknown> | null)?.configSource ?? 'global')
    modeStore
      .getModeMeta(mode.slug)
      .then((meta) => {
        editingModeMeta.value = meta
      })
      .catch(() => {
        editingModeMeta.value = null
      })
  } else {
    modeForm.slug = ''
    modeForm.name = ''
    modeForm.description = ''
    modeForm.groupsText = ''
    modeForm.roleDefinition = ''
    modeForm.whenToUse = ''
    modeForm.customInstructions = ''
    modeForm.configSource = 'global'
  }
  showModeEditor.value = true
}

function openModeClone(mode: ModeEntity) {
  openModeEditor()
  editingMode.value = null
  modeForm.slug = `${mode.slug}-copy`
  modeForm.name = mode.name
  modeForm.description = mode.description
  modeForm.groupsText = mode.groups.join(', ')
  modeForm.roleDefinition = mode.roleDefinition
  modeForm.whenToUse = mode.whenToUse ?? ''
  modeForm.customInstructions = mode.customInstructions ?? ''
  modeForm.configSource = String((mode.payload as Record<string, unknown> | null)?.configSource ?? 'global')
}

function openInstanceModes(instance: IdeInstanceEntity) {
  activeInstance.value = instance
  showInstanceModesModal.value = true
  refreshInstanceModes()
}

function closeInstanceModes() {
  showInstanceModesModal.value = false
}

async function refreshInstanceModes() {
  if (!activeInstance.value) return
  instanceModesLoading.value = true
  instanceModesError.value = null
  try {
    instanceModes.value = await modeStore.listInstanceModes(activeInstance.value.id)
  } catch (err) {
    instanceModesError.value = err instanceof Error ? err.message : String(err)
  } finally {
    instanceModesLoading.value = false
  }
}

function openInstanceModeEditor(item?: { slug: string; name?: string | null; raw: Record<string, unknown> }) {
  if (!activeInstance.value) return
  editorMessage.value = null
  modeEditorTarget.value = 'instance'
  modeEditorInstanceId.value = activeInstance.value.id
  modeEditorSaveToDb.value = true
  modeEditorConflictStrategy.value = 'overwrite'
  editingMode.value = null

  const raw = item?.raw ?? {}
  instanceModeDraft.value = { ...raw }
  modeForm.slug = String(raw.slug ?? item?.slug ?? '')
  modeForm.name = String(raw.name ?? item?.name ?? '')
  modeForm.description = String(raw.description ?? '')
  modeForm.groupsText = Array.isArray(raw.groups) ? (raw.groups as unknown[]).map(String).join(', ') : ''
  modeForm.roleDefinition = String(raw.roleDefinition ?? '')
  modeForm.whenToUse = String(raw.whenToUse ?? '')
  modeForm.customInstructions = String(raw.customInstructions ?? '')
  modeForm.configSource = String(raw.source ?? 'global')
  showModeEditor.value = true
}

function closeModeEditor() {
  showModeEditor.value = false
}

async function handleSaveMode() {
  editorMessage.value = null
  const groups = modeForm.groupsText
    .split(/[,\\n]/)
    .map((text) => text.trim())
    .filter(Boolean)

  if (
    !modeForm.slug.trim() ||
    !modeForm.name.trim() ||
    !modeForm.description.trim() ||
    !modeForm.roleDefinition.trim() ||
    !modeForm.configSource.trim() ||
    !groups.length
  ) {
    editorMessage.value = '请填写 slug/name/description/roleDefinition/source，并至少提供一个 group'
    return
  }

  try {
    if (modeEditorTarget.value === 'db') {
      const payload: ModeEntity = {
        id: editingMode.value?.id ?? '',
        slug: modeForm.slug.trim(),
        name: modeForm.name.trim(),
        description: modeForm.description.trim(),
        groups,
        roleDefinition: modeForm.roleDefinition,
        roleDefinitionLength: modeForm.roleDefinition.length,
        source: editingMode.value?.source ?? 'local',
        whenToUse: modeForm.whenToUse.trim() ? modeForm.whenToUse.trim() : null,
        customInstructions: modeForm.customInstructions.trim() ? modeForm.customInstructions.trim() : null,
        payload: { ...(editingMode.value?.payload ?? {}), configSource: modeForm.configSource.trim() },
        updatedAt: editingMode.value?.updatedAt ?? '',
        hash: editingMode.value?.hash ?? ''
      }
      await modeStore.saveMode(payload)
      editorMessage.value = '已保存到本地库'
      setTimeout(() => closeModeEditor(), 400)
      return
    }

    if (!modeEditorInstanceId.value) {
      editorMessage.value = '未选择目标实例'
      return
    }
    const draft = instanceModeDraft.value && typeof instanceModeDraft.value === 'object' ? { ...instanceModeDraft.value } : {}
    draft.slug = modeForm.slug.trim()
    draft.name = modeForm.name.trim()
    draft.description = modeForm.description.trim()
    draft.groups = groups
    draft.roleDefinition = modeForm.roleDefinition
    draft.source = modeForm.configSource.trim()
    if (modeForm.whenToUse.trim()) {
      draft.whenToUse = modeForm.whenToUse.trim()
    } else {
      delete (draft as Record<string, unknown>).whenToUse
    }
    if (modeForm.customInstructions.trim()) {
      draft.customInstructions = modeForm.customInstructions
    } else {
      delete (draft as Record<string, unknown>).customInstructions
    }

    const upserted = await modeStore.upsertInstanceMode({
      instanceId: modeEditorInstanceId.value,
      mode: draft,
      conflictStrategy: modeEditorConflictStrategy.value,
      saveToDb: modeEditorSaveToDb.value
    })
    await modeStore.scanInstanceModes(modeEditorInstanceId.value)
    editorMessage.value =
      upserted.finalSlug === upserted.requestedSlug ? '已写回实例' : `已写回实例（重命名为 ${upserted.finalSlug}）`
    await refreshInstanceModes()
    setTimeout(() => closeModeEditor(), 400)
  } catch (err) {
    editorMessage.value = err instanceof Error ? err.message : String(err)
  }
}

async function handleDeleteMode(slug: string) {
  const ok = window.confirm(`确认删除模式：${slug}？`)
  if (!ok) return
  try {
    await modeStore.deleteMode(slug)
  } catch (err) {
    importError.value = err instanceof Error ? err.message : String(err)
  }
}

async function handleDeleteInstanceMode(slug: string) {
  if (!activeInstance.value) return
  const ok = window.confirm(`确认从实例中删除模式：${slug}？`)
  if (!ok) return
  instanceModesError.value = null
  try {
    await modeStore.deleteInstanceMode({ instanceId: activeInstance.value.id, slug })
    await modeStore.scanInstanceModes(activeInstance.value.id)
    await refreshInstanceModes()
  } catch (err) {
    instanceModesError.value = err instanceof Error ? err.message : String(err)
  }
}

function openHistory(instance: IdeInstanceEntity) {
  activeInstance.value = instance
  historySelectedId.value = null
  historyReplayMessage.value = null
  showHistoryModal.value = true
  refreshHistory()
}

function closeHistory() {
  showHistoryModal.value = false
}

function openInstanceDiff(instance: IdeInstanceEntity) {
  diffInstance.value = instance
  diffError.value = null
  diffMessage.value = null
  diffSummary.value = null
  diffApplyStrategy.value = 'overwrite'
  showDiffModal.value = true
  refreshInstanceDiff()
}

function closeInstanceDiff() {
  showDiffModal.value = false
}

async function refreshInstanceDiff() {
  if (!diffInstance.value) return
  diffLoading.value = true
  diffError.value = null
  try {
    diffSummary.value = await modeStore.diffInstanceModes(diffInstance.value.id)
    await modeStore.bootstrap(true)
  } catch (err) {
    diffError.value = err instanceof Error ? err.message : String(err)
  } finally {
    diffLoading.value = false
  }
}

async function handleImportIdeOnlyToDb() {
  if (!diffInstance.value || !diffSummary.value) return
  const slugs = diffSummary.value.ideOnly.map((item) => item.slug)
  if (!slugs.length) {
    diffMessage.value = '没有发现 IDE 新增模式，无需导入'
    return
  }
  diffMessage.value = '正在将 IDE 新增模式导入本地库...'
  try {
    const report = await modeStore.importInstanceModesToDb({
      instanceId: diffInstance.value.id,
      modeSlugs: slugs,
      conflictStrategy: 'skip'
    })
    diffMessage.value = `导入完成：写入 ${report.saved} 条，重复 hash ${report.duplicateHash} 条，重复 slug ${report.duplicateSlug} 条`
    await refreshInstanceDiff()
  } catch (err) {
    diffMessage.value = err instanceof Error ? err.message : String(err)
  } finally {
    setTimeout(() => (diffMessage.value = null), 4000)
  }
}

async function handleResolveConflictsByDb() {
  if (!diffInstance.value || !diffSummary.value) return
  const slugs = diffSummary.value.conflicts.map((item) => item.slug)
  if (!slugs.length) {
    diffMessage.value = '没有发现冲突，无需写回'
    return
  }
  diffMessage.value = '正在用本地库版本写回 IDE...'
  try {
    const result = await modeStore.applyModesToInstances({
      modeSlugs: slugs,
      instanceIds: [diffInstance.value.id],
      conflictStrategy: diffApplyStrategy.value
    })
    const detail = result.details[0]
    if (detail) {
      diffMessage.value = `写回完成：新增 ${detail.applied}，覆盖 ${detail.overwritten}，重命名 ${detail.renamed}，跳过 ${detail.skipped}`
    } else {
      diffMessage.value = `写回完成：更新 ${result.updatedInstances}/${result.totalInstances} 个实例`
    }
    await refreshInstanceDiff()
  } catch (err) {
    diffMessage.value = err instanceof Error ? err.message : String(err)
  } finally {
    setTimeout(() => (diffMessage.value = null), 4000)
  }
}

async function refreshHistory() {
  if (!activeInstance.value) return
  historyLoading.value = true
  historyError.value = null
  try {
    historyItems.value = await modeStore.listModeHistory({ instanceId: activeInstance.value.id, limit: 100, offset: 0 })
  } catch (err) {
    historyError.value = err instanceof Error ? err.message : String(err)
  } finally {
    historyLoading.value = false
  }
}

function formatPayload(value?: Record<string, unknown> | null) {
  if (!value) return ''
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

function formatImportPreviewStatus(status: string) {
  if (status === 'new') return '新增'
  if (status === 'slugConflict') return 'slug 冲突'
  if (status === 'invalid') return '无效'
  if (status === 'hashMatch') return 'hash 匹配'
  return status
}

async function handleReplay(historyId: string) {
  historyReplayMessage.value = '正在回放...'
  try {
    const result = await modeStore.replayModeHistory({
      historyId,
      conflictStrategy: historyReplayStrategy.value,
      saveToDb: historyReplaySaveToDb.value
    })
    historyReplayMessage.value =
      result.result.finalSlug === result.result.requestedSlug
        ? '已回放并写回实例'
        : `已回放并写回实例（重命名为 ${result.result.finalSlug}）`
    if (activeInstance.value) {
      await modeStore.scanInstanceModes(activeInstance.value.id)
    }
    await refreshHistory()
  } catch (err) {
    historyReplayMessage.value = err instanceof Error ? err.message : String(err)
  } finally {
    setTimeout(() => (historyReplayMessage.value = null), 4000)
  }
}

async function handleBootstrap() {
  try {
    await modeStore.bootstrap(true)
    importProgress.value = { total: modes.value.length, success: modes.value.length }
    importSuccess.value = '已从数据库刷新模式列表'
  } catch (err) {
    importError.value = err instanceof Error ? err.message : String(err)
  }
}

async function handlePreviewImport() {
  importError.value = null
  importSuccess.value = null
  importReport.value = null
  if (!importText.value.trim()) {
    importError.value = '请先粘贴 YAML/JSON 配置内容'
    return
  }
  importing.value = true
  try {
    importPreview.value = await modeStore.previewModeDiff(importText.value)
    importSuccess.value = `解析完成：发现 ${importPreview.value.discovered} 条，新增 ${importPreview.value.newModes} 条，冲突 ${importPreview.value.conflicts} 条`
  } catch (err) {
    importError.value = err instanceof Error ? err.message : String(err)
  } finally {
    importing.value = false
  }
}

async function handleImportFromText() {
  importError.value = null
  importSuccess.value = null
  if (!importText.value.trim()) {
    importError.value = '请先粘贴 YAML/JSON 配置内容'
    return
  }
  importing.value = true
  try {
    importReport.value = await modeStore.importModesFromText({ text: importText.value, conflictStrategy: importConflictStrategy.value })
    importSuccess.value = `导入完成：写入 ${importReport.value.saved} 条，重复 hash ${importReport.value.duplicateHash} 条，重复 slug ${importReport.value.duplicateSlug} 条`
    importPreview.value = null
  } catch (err) {
    importError.value = err instanceof Error ? err.message : String(err)
  } finally {
    importing.value = false
  }
}

function closeDrawer() {
  showImportDrawer.value = false
}

function openApplyModal(slug: string) {
  applyModeSlug.value = slug
  applyStrategy.value = 'overwrite'
  applyResultMessage.value = null
  applyKindFilter.value = 'all'
  applyInstanceIds.value = ideInstances.value.filter((item) => item.selected).map((item) => item.id)
  applyCompareInstanceId.value = applyInstanceIds.value[0] ?? null
  applyCompareIdeRaw.value = null
  applyCompareError.value = null
  showApplyModal.value = true
  refreshApplyCompare()
}

function openApplyModalForKind(slug: string, kind: 'kilocode' | 'roocode') {
  applyModeSlug.value = slug
  applyStrategy.value = 'overwrite'
  applyResultMessage.value = null
  applyKindFilter.value = kind
  const preferred = ideInstances.value.filter((item) => item.kind === kind && item.selected).map((item) => item.id)
  applyInstanceIds.value = preferred.length ? preferred : ideInstances.value.filter((item) => item.kind === kind).map((item) => item.id)
  if (!applyInstanceIds.value.length) {
    applyResultMessage.value = '未找到对应类型的实例，请先在 “IDE 配置” 扫描或手动添加'
  }
  applyCompareInstanceId.value = applyInstanceIds.value[0] ?? null
  applyCompareIdeRaw.value = null
  applyCompareError.value = null
  showApplyModal.value = true
  refreshApplyCompare()
}

function closeApplyModal() {
  showApplyModal.value = false
  applyCompareInstanceId.value = null
  applyCompareIdeRaw.value = null
  applyCompareLoading.value = false
  applyCompareError.value = null
}

async function refreshApplyCompare() {
  const slug = applyModeSlug.value
  const instanceId = applyCompareInstanceId.value
  applyCompareError.value = null
  applyCompareIdeRaw.value = null
  if (!slug || !instanceId) return
  applyCompareLoading.value = true
  try {
    const raw = await modeStore.getInstanceModeRaw({ instanceId, slug })
    applyCompareIdeRaw.value = raw && typeof raw === 'object' ? (raw as Record<string, unknown>) : null
  } catch (err) {
    applyCompareError.value = err instanceof Error ? err.message : String(err)
  } finally {
    applyCompareLoading.value = false
  }
}

async function handleApplyToInstances() {
  if (!applyModeSlug.value) return
  if (!applyInstanceIds.value.length) {
    applyResultMessage.value = '请先选择至少一个目标实例'
    return
  }
  applyResultMessage.value = '正在写入实例配置...'
  try {
    const result = await modeStore.applyModesToInstances({
      modeSlugs: [applyModeSlug.value],
      instanceIds: applyInstanceIds.value,
      conflictStrategy: applyStrategy.value
    })
    applyResultMessage.value = `写入完成：更新 ${result.updatedInstances}/${result.totalInstances} 个实例`
    await modeStore.bootstrap(true)
  } catch (err) {
    applyResultMessage.value = err instanceof Error ? err.message : String(err)
  }
}

async function handleSyncInstanceToDb(instanceId: string) {
  activeMessage.value = '正在从实例读取并入库...'
  try {
    await modeStore.scanInstanceModes(instanceId)
    activeMessage.value = '已从实例同步到本地库'
  } catch (err) {
    activeMessage.value = err instanceof Error ? err.message : String(err)
  } finally {
    setTimeout(() => (activeMessage.value = null), 3000)
  }
}

onMounted(() => {
  Promise.all([modeStore.bootstrap(), modeStore.fetchAppSettings()]).catch(() => {
    /* store 已记录错误 */
  })
})

async function refreshCompare() {
  compareLoading.value = true
  compareError.value = null
  try {
    compareList.value = await modeStore.compareKiloRooModes()
  } catch (err) {
    compareError.value = err instanceof Error ? err.message : String(err)
  } finally {
    compareLoading.value = false
  }
}

watch(
  () => activeTab.value,
  (tab) => {
    if (tab === 'compare') {
      refreshCompare()
    }
  }
)

watch(
  () => [showApplyModal.value, applyModeSlug.value, applyCompareInstanceId.value] as const,
  ([visible]) => {
    if (visible) {
      refreshApplyCompare()
    }
  }
)
</script>

<template>
  <div class="space-y-2 p-2">
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
          <div class="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
            <!-- <div>
              <h2 class="text-lg font-semibold text-gray-900">本地模式库</h2>
              <p class="mt-1 text-xs text-gray-400">
                高质量阈值：{{ roleDefinitionThreshold }} 字（高质量：{{
                  modes.filter((item) => item.roleDefinitionLength >= roleDefinitionThreshold).length
                }} / {{ modes.length }}）
              </p>
            </div> -->
            <div class="flex flex-wrap gap-2 md:justify-end">
              <input
                v-model="filterKeyword"
                type="text"
                placeholder="搜索名称或 slug"
                class="w-full rounded-md border border-gray-200 px-3 py-2 text-sm md:w-64"
              />
              <label class="flex items-center gap-2 rounded-md border border-gray-200 px-3 py-2 text-sm text-gray-700">
                <input v-model="showQualityOnly" type="checkbox" class="rounded border-gray-300" />
                仅看高质量
              </label>
              <button class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white shadow" @click="openModeEditor()">
                新建模式
              </button>
              <button class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700" @click="openImportDrawer">
                批量导入
              </button>
            </div>
          </div>

          <div v-if="filteredModes.length" class="overflow-hidden rounded-lg border border-gray-100">
            <table class="w-full table-fixed divide-y divide-gray-100">
              <thead class="bg-gray-50">
                <tr>
                  <th class="w-48 px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">
                    模式名称
                  </th>
                  <th class="w-16 px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">
                    来源
                  </th>
                  <th class="w-32 px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">
                    更新时间
                  </th>
                  <th
                    v-if="showRoleDefinitionColumn"
                    class="w-24 px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500"
                  >
                    roleDefinition 长度
                  </th>
                  <th class="w-32 px-4 py-3 text-right text-xs font-medium uppercase tracking-wider text-gray-500">
                    操作
                  </th>
                </tr>
              </thead>
              <tbody class="divide-y divide-gray-100 bg-white">
                <tr v-for="mode in filteredModes" :key="mode.id" class="hover:bg-gray-50">
                  <td class="min-w-0 px-4 py-3">
                    <div class="truncate font-medium text-gray-900" :title="mode.name">{{ mode.name }}</div>
                    <div class="mt-0.5 truncate text-xs text-gray-400" :title="mode.slug">slug: {{ mode.slug }}</div>
                  </td>
                  <td class="px-4 py-3 text-sm text-gray-600">{{ mode.source }}</td>
                  <td class="px-4 py-3 text-sm text-gray-600">{{ formatDateTime(mode.updatedAt) }}</td>
                  <td v-if="showRoleDefinitionColumn" class="px-4 py-3 text-sm text-gray-600">
                    {{ mode.roleDefinitionLength }} 字
                  </td>
                  <td class="px-4 py-3 text-right text-sm text-gray-600">
                    <div class="flex justify-end gap-1">
                      <button
                        class="rounded-md border border-gray-200 p-1 text-gray-700 hover:border-blue-500 hover:text-blue-600"
                        title="同步"
                        aria-label="同步"
                        @click="openApplyModal(mode.slug)"
                      >
                        <svg viewBox="0 0 20 20" fill="currentColor" class="h-4 w-4">
                          <path
                            fill-rule="evenodd"
                            d="M10 3a7 7 0 00-6.32 4H2.75a.75.75 0 000 1.5H5.5a.75.75 0 00.75-.75V5.5a.75.75 0 00-1.5 0v.86A5.5 5.5 0 0115.5 10a.75.75 0 001.5 0A7 7 0 0010 3zm5.25 10h2.75a.75.75 0 010 1.5H14.5a.75.75 0 01-.75-.75V12.5a.75.75 0 011.5 0v.86A5.5 5.5 0 014.5 10a.75.75 0 00-1.5 0A7 7 0 0010 17a7 7 0 006.32-4h-1.07z"
                            clip-rule="evenodd"
                          />
                        </svg>
                      </button>
                      <button
                        class="rounded-md border border-gray-200 p-1 text-gray-700 hover:border-blue-500 hover:text-blue-600"
                        title="编辑"
                        aria-label="编辑"
                        @click="openModeEditor(mode)"
                      >
                        <svg viewBox="0 0 20 20" fill="currentColor" class="h-4 w-4">
                          <path
                            d="M13.586 3.586a2 2 0 012.828 2.828l-8.5 8.5a1 1 0 01-.39.242l-3 1a1 1 0 01-1.265-1.265l1-3a1 1 0 01.242-.39l8.5-8.5z"
                          />
                        </svg>
                      </button>
                      <button
                        class="rounded-md border border-gray-200 p-1 text-gray-700 hover:border-blue-500 hover:text-blue-600"
                        title="克隆"
                        aria-label="克隆"
                        @click="openModeClone(mode)"
                      >
                        <svg viewBox="0 0 20 20" fill="currentColor" class="h-4 w-4">
                          <path
                            fill-rule="evenodd"
                            d="M6.25 4A2.25 2.25 0 004 6.25v6.5A2.25 2.25 0 006.25 15h6.5A2.25 2.25 0 0015 12.75v-6.5A2.25 2.25 0 0012.75 4h-6.5zm.25 2.25a.75.75 0 01.75-.75h5.5a.75.75 0 01.75.75v5.5a.75.75 0 01-.75.75h-5.5a.75.75 0 01-.75-.75v-5.5z"
                            clip-rule="evenodd"
                          />
                          <path d="M3.5 7.25A.75.75 0 012.75 6.5h.5a.75.75 0 010 1.5h-.5a.75.75 0 01-.75-.75zM16.75 12a.75.75 0 01.75.75v.5a.75.75 0 01-1.5 0v-.5a.75.75 0 01.75-.75z" />
                        </svg>
                      </button>
                      <button
                        class="rounded-md border border-red-200 p-1 text-red-600 hover:border-red-400"
                        title="删除"
                        aria-label="删除"
                        @click="handleDeleteMode(mode.slug)"
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
          <div v-else class="rounded-lg border border-dashed border-gray-200 bg-white p-4 text-sm text-gray-600">
            <p>暂无模式，请先通过 模式采集 或 IDE 扫描入库。</p>
            <div class="mt-3 flex flex-wrap gap-3">
              <button class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white" @click="router.push('/github-sync')">
                去 模式采集
              </button>
              <button class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700" @click="router.push('/ide')">
                去 IDE 配置
              </button>
            </div>
          </div>
        </div>

        <!-- 当前生效模式 -->
        <div v-else-if="activeTab === 'active'" class="space-y-4">
          <div class="grid gap-4 md:grid-cols-1" v-if="ideInstances.length">
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
                    'rounded-full px-3 py-1 text-xs whitespace-nowrap',
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
              <p class="mt-3 text-xs text-gray-500">最近扫描：{{ formatDateTime(instance.lastScanAt) }}</p>
              <div class="mt-4 flex flex-wrap gap-2">
                <button
                  class="rounded-md border border-gray-200 px-3 py-2 text-sm text-gray-600"
                  @click="openInstanceDiff(instance)"
                >
                  检查差异
                </button>
                <button
                  class="rounded-md border border-gray-200 px-3 py-2 text-sm text-gray-600"
                  @click="openInstanceModes(instance)"
                >
                  查看/编辑模式
                </button>
                <button
                  class="rounded-md border border-gray-200 px-3 py-2 text-sm text-gray-600"
                  @click="openHistory(instance)"
                >
                  操作历史
                </button>
                <button
                  class="rounded-md bg-blue-600 px-3 py-2 text-sm text-white"
                  @click="handleSyncInstanceToDb(instance.id)"
                >
                  同步到本地库
                </button>
              </div>
            </div>
          </div>
          <p v-if="activeMessage" class="text-sm text-blue-600">{{ activeMessage }}</p>
          <div v-else class="rounded-lg border border-dashed border-gray-200 bg-white p-4 text-sm text-gray-600">
            <p>暂无识别到的实例，请前往 “IDE 配置” 页扫描。</p>
            <div class="mt-3 flex flex-wrap gap-3">
              <button class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white" @click="router.push('/ide')">去 IDE 配置</button>
              <button class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700" @click="router.push('/github-sync')">
                去 模式采集
              </button>
            </div>
          </div>
        </div>

        <!-- 模式对比 -->
        <div v-else class="space-y-4">
          <!-- <h2 class="text-lg font-semibold text-gray-900">跨软件模式对比</h2> -->
          <p class="text-sm text-gray-500">
            按 slug 对比所有 KiloCode 实例与 RooCode 实例的合集；“缺失”表示该类型的实例中未发现该 slug。下方操作会将本地库中同 slug 的模式写入勾选实例。
          </p>

          <div v-if="compareLoading" class="rounded-lg border border-dashed border-gray-200 bg-white p-4 text-sm text-gray-500">
            正在读取实例配置并计算差异...
          </div>
          <div v-else-if="compareError" class="rounded-lg border border-red-200 bg-red-50 p-4 text-sm text-red-600">
            {{ compareError }}
          </div>

          <div class="overflow-hidden rounded-lg border border-gray-100">
            <table class="w-full table-fixed divide-y divide-gray-100">
              <thead class="bg-gray-50">
                <tr>
                  <th class="w-64 px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">
                    模式 Slug
                  </th>
                  <th class="w-20 px-4 py-3 text-center text-xs font-medium uppercase tracking-wider text-gray-500">
                    KiloCode
                  </th>
                  <th class="w-20 px-4 py-3 text-center text-xs font-medium uppercase tracking-wider text-gray-500">
                    RooCode
                  </th>
                  <th class="w-44 px-4 py-3 text-right text-xs font-medium uppercase tracking-wider text-gray-500">
                    操作
                  </th>
                </tr>
              </thead>
              <tbody class="divide-y divide-gray-100 bg-white">
                <tr v-for="item in compareList" :key="item.slug" class="hover:bg-gray-50">
                  <td class="min-w-0 px-4 py-3 font-mono text-xs text-gray-900">
                    <span class="block truncate" :title="item.slug">{{ item.slug }}</span>
                  </td>
                  <td class="px-4 py-3 text-center">
                    <span
                      :class="[
                        'rounded-full px-2 py-1 text-xs font-medium whitespace-nowrap',
                        item.inKilocode
                          ? 'bg-green-50 text-green-600'
                          : 'bg-gray-50 text-gray-400',
                      ]"
                    >
                      {{ item.inKilocode ? '存在' : '缺失' }}
                    </span>
                  </td>
                  <td class="px-4 py-3 text-center">
                    <span
                      :class="[
                        'rounded-full px-2 py-1 text-xs font-medium whitespace-nowrap',
                        item.inRoocode
                          ? 'bg-green-50 text-green-600'
                          : 'bg-gray-50 text-gray-400',
                      ]"
                    >
                      {{ item.inRoocode ? '存在' : '缺失' }}
                    </span>
                  </td>
                  <td class="px-4 py-3 text-right">
                    <div class="flex justify-end gap-2">
                      <button
                        class="rounded-md border border-gray-200 px-2 py-1 text-xs text-gray-600 whitespace-nowrap disabled:cursor-not-allowed disabled:opacity-60"
                        :disabled="item.inKilocode"
                        title="补齐到 KiloCode"
                        @click="openApplyModalForKind(item.slug, 'kilocode')"
                      >
                        补齐 Kilo
                      </button>
                      <button
                        class="rounded-md border border-gray-200 px-2 py-1 text-xs text-gray-600 whitespace-nowrap disabled:cursor-not-allowed disabled:opacity-60"
                        :disabled="item.inRoocode"
                        title="补齐到 RooCode"
                        @click="openApplyModalForKind(item.slug, 'roocode')"
                      >
                        补齐 Roo
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

    <!-- 导入抽屉 -->
    <div
      v-if="showImportDrawer"
      class="fixed inset-0 z-20 flex justify-end bg-black/30 backdrop-blur-sm"
      aria-modal="true"
      role="dialog"
    >
      <div class="flex w-full max-w-md flex-col bg-white shadow-xl">
        <header class="flex items-center justify-between border-b border-gray-100 px-5 py-4">
          <div>
            <h3 class="text-lg font-semibold text-gray-900">批量导入模式</h3>
            <p class="text-sm text-gray-500">支持从 IDE 配置或 GitHub 拉取的模式重新刷新本地库</p>
          </div>
          <button @click="closeDrawer" class="text-gray-400 hover:text-gray-600">✕</button>
        </header>

        <div class="flex-1 space-y-4 overflow-auto px-5 py-4 text-sm text-gray-600">
          <p class="text-xs text-gray-500">
            当前库中模式数：{{ importProgress.total }}，最近一次扫描：{{ formatDateTime(ideInstances[0]?.lastScanAt) }}
          </p>

          <label class="block text-xs font-semibold text-gray-600">
            粘贴配置内容（支持 `customModes:` 的 YAML/JSON）
            <textarea
              v-model="importText"
              rows="6"
              placeholder="customModes:\n  - slug: ...\n    name: ...\n    description: ...\n    groups: [...]"
              class="mt-2 w-full rounded-md border border-gray-200 px-3 py-2 text-xs font-mono text-gray-800"
            />
          </label>

          <div class="grid grid-cols-2 gap-3">
            <button
              class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700 disabled:cursor-not-allowed disabled:opacity-60"
              :disabled="importing"
              @click="handlePreviewImport"
            >
              预览差异
            </button>
            <button
              class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white disabled:cursor-not-allowed disabled:opacity-60"
              :disabled="importing"
              @click="handleImportFromText"
            >
              导入到本地库
            </button>
          </div>

          <div class="rounded-md border border-gray-100 bg-gray-50/70 p-3">
            <p class="text-xs font-semibold text-gray-700">冲突处理策略</p>
            <p class="mt-1 text-[11px] text-gray-500">
              当导入的 slug 与本地库同名但内容不同（slugConflict）时：
            </p>
            <select v-model="importConflictStrategy" class="mt-2 w-full rounded-md border border-gray-200 px-3 py-2 text-sm text-gray-700">
              <option value="rename">自动重命名（推荐）</option>
              <option value="skip">跳过</option>
              <option value="overwrite">覆盖同名</option>
            </select>
          </div>

          <button
            class="w-full rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700"
            @click="handleBootstrap"
          >
            仅刷新模式列表
          </button>

          <div v-if="importPreview" class="space-y-2">
            <p class="text-xs font-semibold text-gray-700">预览结果</p>
            <div class="overflow-x-auto rounded-md border border-gray-100">
              <table class="w-full table-fixed divide-y divide-gray-100 text-xs" style="min-width: 560px;">
                <thead class="bg-gray-50">
                  <tr>
                    <th class="w-48 px-3 py-2 text-left font-medium text-gray-500">slug</th>
                    <th class="w-24 px-3 py-2 text-left font-medium text-gray-500">状态</th>
                    <th class="px-3 py-2 text-left font-medium text-gray-500">建议</th>
                  </tr>
                </thead>
                <tbody class="divide-y divide-gray-100 bg-white">
                  <tr v-for="item in importPreview.items" :key="item.slug + item.status">
                    <td class="px-3 py-2">
                      <p class="truncate font-medium text-gray-900" :title="item.slug">{{ item.slug }}</p>
                      <p class="truncate text-[10px] text-gray-400" :title="item.name">{{ item.name }}</p>
                    </td>
                    <td class="px-3 py-2">
                      <span
                        :class="[
                          'rounded-full px-2 py-1 text-[10px] font-medium',
                          item.status === 'new'
                            ? 'bg-green-50 text-green-600'
                            : item.status === 'slugConflict'
                              ? 'bg-yellow-50 text-yellow-700'
                              : item.status === 'invalid'
                                ? 'bg-red-50 text-red-600'
                                : 'bg-gray-50 text-gray-500'
                        ]"
                      >
                        {{ formatImportPreviewStatus(item.status) }}
                      </span>
                      <p v-if="item.missingFields.length" class="mt-1 text-[10px] text-red-600">
                        缺少：{{ item.missingFields.join(', ') }}
                      </p>
                    </td>
                    <td class="px-3 py-2 text-[10px] text-gray-600">
                      <div class="truncate" :title="item.recommendedAction">{{ item.recommendedAction }}</div>
                      <div v-if="item.renameSuggestion" class="mt-0.5 truncate text-gray-400" :title="item.renameSuggestion">
                        → {{ item.renameSuggestion }}
                      </div>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>

          <div v-if="importReport" class="space-y-2">
            <p class="text-xs font-semibold text-gray-700">导入统计</p>
            <ul class="list-disc space-y-1 pl-4 text-xs text-gray-600">
              <li>发现：{{ importReport.discovered }}</li>
              <li>写入：{{ importReport.saved }}</li>
              <li>缺字段跳过：{{ importReport.skippedDueToMissingFields }}</li>
              <li>重复 hash：{{ importReport.duplicateHash }}</li>
              <li>重复 slug：{{ importReport.duplicateSlug }}</li>
            </ul>
            <div v-if="importReport.errors.length" class="rounded-md bg-red-50 p-3 text-[10px] text-red-700">
              <p class="font-semibold">错误明细</p>
              <ul class="mt-2 list-disc space-y-1 pl-4">
                <li v-for="item in importReport.errors" :key="item">{{ item }}</li>
              </ul>
            </div>
          </div>

          <div class="rounded-md bg-blue-50 p-3 text-xs text-blue-700" v-if="importSuccess">
            {{ importSuccess }}
          </div>
          <div class="rounded-md bg-red-50 p-3 text-xs text-red-600" v-if="importError">
            {{ importError }}
          </div>
        </div>

        <footer class="border-t border-gray-100 px-5 py-4 text-right">
          <button class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700" @click="closeDrawer">
            关闭
          </button>
        </footer>
      </div>
    </div>

    <!-- 应用到实例 -->
    <div v-if="showApplyModal" class="fixed inset-0 z-30 flex items-center justify-center bg-black/30 backdrop-blur-sm" aria-modal="true" role="dialog">
      <div class="w-full max-w-3xl rounded-lg bg-white shadow-xl">
        <header class="flex items-center justify-between border-b border-gray-100 px-5 py-4">
          <div>
            <h3 class="text-lg font-semibold text-gray-900">应用到实例</h3>
            <p class="text-sm text-gray-500">将模式 <span class="font-mono text-gray-700">{{ applyModeSlug }}</span> 写入选定实例</p>
          </div>
          <button @click="closeApplyModal" class="text-gray-400 hover:text-gray-600">✕</button>
        </header>
        <div class="grid gap-4 px-5 py-4 text-sm text-gray-700 md:grid-cols-2">
          <div class="space-y-4">
            <label class="block text-sm font-medium text-gray-700">
              冲突策略
              <select v-model="applyStrategy" class="mt-2 w-full rounded-md border border-gray-200 px-3 py-2 text-sm">
              <option value="overwrite">覆盖同 slug</option>
              <option value="rename">自动重命名</option>
              <option value="skip">跳过</option>
              </select>
            </label>

            <div>
              <p class="text-sm font-medium text-gray-700">目标实例</p>
              <div class="mt-2 max-h-56 space-y-2 overflow-auto rounded-md border border-gray-100 p-3">
                <label v-for="instance in applyTargetInstances" :key="instance.id" class="flex items-start gap-3 text-sm">
                  <input v-model="applyInstanceIds" :value="instance.id" type="checkbox" class="mt-1 rounded border-gray-300" />
                  <span class="min-w-0">
                    <span class="font-medium text-gray-900">{{ instance.alias }}</span>
                    <span class="ml-2 text-xs text-gray-400">{{ instance.kind }}</span>
                    <span class="block truncate text-xs text-gray-400" :title="instance.path">{{ instance.path }}</span>
                  </span>
                </label>
                <p v-if="!applyTargetInstances.length" class="text-xs text-gray-500">暂无实例，请先在 “IDE 配置” 扫描或添加。</p>
              </div>
              <p class="mt-2 text-xs text-gray-500">
                当目标实例已存在同 slug：覆盖=直接替换；自动重命名=写入 `${slug}-copy`；跳过=保持不变。
              </p>
            </div>

            <p v-if="applyResultMessage" class="text-sm text-blue-600">{{ applyResultMessage }}</p>
          </div>

          <div class="space-y-3">
            <p class="text-sm font-semibold text-gray-900">冲突对比（可选）</p>
            <div v-if="!applyDbRaw" class="rounded-md bg-red-50 p-3 text-sm text-red-600">
              未在本地库中找到该 slug，无法展示对比。
            </div>
            <div v-else-if="!applySelectedInstances.length" class="rounded-md bg-gray-50 p-3 text-sm text-gray-500">
              请选择至少一个目标实例后，可查看该实例当前内容与本地库将写入内容的对比。
            </div>
            <div v-else class="space-y-3">
              <div class="flex flex-wrap items-center justify-between gap-2">
                <label class="text-sm text-gray-700">
                  选择实例
                  <select v-model="applyCompareInstanceId" class="ml-2 rounded-md border border-gray-200 px-3 py-2 text-sm">
                    <option v-for="instance in applySelectedInstances" :key="instance.id" :value="instance.id">
                      {{ instance.alias }}（{{ instance.kind }}）
                    </option>
                  </select>
                </label>
                <button
                  class="rounded-md border border-gray-200 px-3 py-2 text-sm text-gray-700 disabled:cursor-not-allowed disabled:opacity-60"
                  :disabled="applyCompareLoading || !applyCompareInstanceId"
                  @click="refreshApplyCompare"
                >
                  刷新对比
                </button>
              </div>

              <div v-if="applyCompareLoading" class="text-sm text-gray-500">正在加载实例当前内容...</div>
              <div v-else-if="applyCompareError" class="rounded-md bg-red-50 p-3 text-sm text-red-600">{{ applyCompareError }}</div>

              <div v-else class="grid gap-3 md:grid-cols-2">
                <div class="rounded-lg border border-gray-100 bg-white p-3">
                  <p class="mb-2 text-xs font-semibold text-gray-600">实例当前内容</p>
                  <p v-if="!applyCompareIdeRaw" class="text-xs text-gray-500">
                    该实例当前不存在该 slug，写入后会作为新增项。
                  </p>
                  <pre
                    v-else
                    class="max-h-72 overflow-auto rounded-md border border-gray-200 bg-gray-50 p-3 text-[11px] text-gray-800"
                  >{{ formatPayload(applyCompareIdeRaw) }}</pre>
                </div>
                <div class="rounded-lg border border-gray-100 bg-white p-3">
                  <p class="mb-2 text-xs font-semibold text-gray-600">本地库将写入内容</p>
                  <pre class="max-h-72 overflow-auto rounded-md border border-gray-200 bg-gray-50 p-3 text-[11px] text-gray-800">{{ formatPayload(applyDbRaw) }}</pre>
                </div>
              </div>
            </div>
          </div>
        </div>
        <footer class="flex justify-end gap-3 border-t border-gray-100 px-5 py-4">
          <button class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700" @click="closeApplyModal">取消</button>
          <button class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white" @click="handleApplyToInstances">写入</button>
        </footer>
      </div>
    </div>

    <!-- 模式编辑 -->
    <div v-if="showModeEditor" class="fixed inset-0 z-30 flex items-center justify-center bg-black/30 backdrop-blur-sm" aria-modal="true" role="dialog">
      <div class="w-full max-w-3xl rounded-lg bg-white shadow-xl">
        <header class="flex items-center justify-between border-b border-gray-100 px-5 py-4">
          <div>
            <h3 class="text-lg font-semibold text-gray-900">{{ editingMode ? '编辑模式' : '新建模式' }}</h3>
            <p class="text-sm text-gray-500">
              {{ modeEditorTarget === 'instance' ? '写回实例配置文件，可选择同时保存到本地库' : '保存到本地模式库' }}
            </p>
          </div>
          <button @click="closeModeEditor" class="text-gray-400 hover:text-gray-600">✕</button>
        </header>
        <div class="grid gap-4 px-5 py-4 md:grid-cols-2">
          <div v-if="modeEditorTarget === 'instance'" class="md:col-span-2 grid gap-3 rounded-md border border-gray-100 bg-gray-50/70 p-3 md:grid-cols-3">
            <label class="text-sm text-gray-700">
              冲突策略
              <select v-model="modeEditorConflictStrategy" class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm">
                <option value="overwrite">覆盖同 slug</option>
                <option value="rename">自动重命名</option>
                <option value="skip">跳过</option>
              </select>
            </label>
            <label class="flex items-center gap-2 text-sm text-gray-700 md:col-span-2">
              <input v-model="modeEditorSaveToDb" type="checkbox" class="rounded border-gray-300" />
              同时保存到本地库（方便后续同步/对比）
            </label>
          </div>
          <label class="text-sm text-gray-700">
            slug（必填）
            <input v-model="modeForm.slug" type="text" class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm" />
          </label>
          <label class="text-sm text-gray-700">
            name（必填）
            <input v-model="modeForm.name" type="text" class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm" />
          </label>
          <label class="text-sm text-gray-700 md:col-span-2">
            description（必填）
            <input
              v-model="modeForm.description"
              type="text"
              class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm"
            />
          </label>
          <label class="text-sm text-gray-700">
            source（必填，写入配置用）
            <input
              v-model="modeForm.configSource"
              type="text"
              placeholder="global"
              class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm"
            />
          </label>
          <label class="text-sm text-gray-700">
            groups（必填，逗号/换行分隔）
            <input
              v-model="modeForm.groupsText"
              type="text"
              placeholder="read, edit, browser"
              class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm"
            />
          </label>
          <label class="text-sm text-gray-700 md:col-span-2">
            roleDefinition（必填）
            <textarea
              v-model="modeForm.roleDefinition"
              rows="8"
              class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-xs font-mono text-gray-800"
            />
          </label>
          <label class="text-sm text-gray-700 md:col-span-2">
            whenToUse（选填）
            <textarea v-model="modeForm.whenToUse" rows="2" class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm" />
          </label>
          <label class="text-sm text-gray-700 md:col-span-2">
            customInstructions（选填）
            <textarea
              v-model="modeForm.customInstructions"
              rows="5"
              class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-xs font-mono text-gray-800"
            />
          </label>
          <div v-if="modeEditorTarget === 'db' && editingMode && editingModeMeta" class="md:col-span-2 rounded-md border border-gray-100 bg-gray-50/70 p-3">
            <div class="flex flex-wrap items-center justify-between gap-2">
              <div>
                <p class="text-sm font-semibold text-gray-900">来源与原始字段</p>
                <p class="mt-1 text-xs text-gray-500">
                  来源别名：{{ editingModeMeta.sourceAlias || '-' }}；来源路径：{{ editingModeMeta.sourcePath || '-' }}
                </p>
              </div>
              <button
                class="rounded-md border border-gray-200 px-3 py-2 text-sm text-gray-700"
                @click="editorShowRawPayload = !editorShowRawPayload"
              >
                {{ editorShowRawPayload ? '收起原始字段' : '展开原始字段' }}
              </button>
            </div>
            <pre
              v-if="editorShowRawPayload"
              class="mt-3 max-h-72 overflow-auto rounded-md border border-gray-200 bg-white p-3 text-[11px] text-gray-800"
            >{{ formatPayload(editingModeMeta.rawPayload as Record<string, unknown> | null) }}</pre>
          </div>
          <p v-if="editorMessage" class="text-sm text-blue-600 md:col-span-2">{{ editorMessage }}</p>
        </div>
        <footer class="flex justify-end gap-3 border-t border-gray-100 px-5 py-4">
          <button class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700" @click="closeModeEditor">取消</button>
          <button class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white" @click="handleSaveMode">保存</button>
        </footer>
      </div>
	    </div>
	
	    <!-- 实例差异检查 -->
	    <div v-if="showDiffModal" class="fixed inset-0 z-30 flex items-center justify-center bg-black/30 backdrop-blur-sm" aria-modal="true" role="dialog">
		      <div class="w-full max-w-3xl rounded-lg bg-white shadow-xl">
	        <header class="flex items-center justify-between border-b border-gray-100 px-5 py-4">
	          <div>
	            <h3 class="text-lg font-semibold text-gray-900">实例差异检查</h3>
	            <p class="text-sm text-gray-500">{{ diffInstance?.alias }}（{{ diffInstance?.kind }}）- {{ diffInstance?.path }}</p>
	          </div>
	          <button @click="closeInstanceDiff" class="text-gray-400 hover:text-gray-600">✕</button>
	        </header>
	
	        <div class="space-y-4 px-5 py-4">
	          <div v-if="diffLoading" class="text-sm text-gray-500">正在读取实例配置并计算差异...</div>
	          <div v-else-if="diffError" class="rounded-md bg-red-50 p-3 text-sm text-red-600">{{ diffError }}</div>
	
	          <template v-else-if="diffSummary">
	            <div class="grid gap-3 rounded-md border border-gray-100 bg-gray-50/70 p-3 md:grid-cols-4">
	              <div class="text-sm text-gray-700">
	                <p class="text-xs text-gray-500">状态</p>
	                <p class="mt-1 font-semibold">
	                  {{
	                    diffSummary.status === 'synced'
	                      ? '已同步'
	                      : diffSummary.status === 'outdated'
	                        ? '待检查'
	                        : '未发现'
	                  }}
	                </p>
	              </div>
	              <div class="text-sm text-gray-700">
	                <p class="text-xs text-gray-500">IDE 模式数</p>
	                <p class="mt-1 font-semibold">{{ diffSummary.totalIde }}</p>
	              </div>
	              <div class="text-sm text-gray-700">
	                <p class="text-xs text-gray-500">本地库模式数</p>
	                <p class="mt-1 font-semibold">{{ diffSummary.totalDb }}</p>
	              </div>
	              <div class="text-sm text-gray-700">
	                <p class="text-xs text-gray-500">同名一致</p>
	                <p class="mt-1 font-semibold">{{ diffSummary.same }}</p>
	              </div>
	            </div>
	
	            <div v-if="!diffSummary.fileExists" class="rounded-md bg-red-50 p-3 text-sm text-red-700">
	              未找到实例配置文件，无法计算差异。请先在 “IDE 配置” 检查路径，或重新扫描。
	            </div>
	
	            <div class="grid gap-4 md:grid-cols-1">
	              <section class="space-y-2 rounded-lg border border-gray-100 bg-white p-4">
	                <div class="flex items-center justify-between">
	                  <p class="text-sm font-semibold text-gray-900">IDE 新增（本地库缺失）</p>
	                  <span class="text-xs text-gray-500">{{ diffSummary.ideOnly.length }} 条</span>
	                </div>
	                <p class="text-xs text-gray-500">可将这些模式入库（不会覆盖同名模式）。</p>
	                <button
	                  class="w-full rounded-md bg-blue-600 px-3 py-2 text-sm text-white disabled:cursor-not-allowed disabled:opacity-60"
	                  :disabled="!diffSummary.ideOnly.length || !diffSummary.fileExists"
	                  @click="handleImportIdeOnlyToDb"
	                >
	                  仅导入 IDE 新增到本地库
	                </button>
	                <ul
	                  v-if="diffSummary.ideOnly.length"
	                  class="max-h-44 overflow-auto rounded-md border border-gray-100 p-2 text-xs text-gray-700"
	                >
	                  <li v-for="item in diffSummary.ideOnly" :key="item.slug" class="py-1">
	                    <span class="font-mono text-gray-900">{{ item.slug }}</span>
	                    <span v-if="item.name" class="ml-2 text-gray-500">{{ item.name }}</span>
	                  </li>
	                </ul>
	              </section>
	
	              <section class="space-y-2 rounded-lg border border-gray-100 bg-white p-4">
	                <div class="flex items-center justify-between">
	                  <p class="text-sm font-semibold text-gray-900">同名冲突（内容不同）</p>
	                  <span class="text-xs text-gray-500">{{ diffSummary.conflicts.length }} 条</span>
	                </div>
	                <p class="text-xs text-gray-500">可将本地库版本写回 IDE，或使用自动重命名避免覆盖。</p>
	                <label class="block text-xs text-gray-600">
	                  写回冲突策略
	                  <select v-model="diffApplyStrategy" class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm">
	                    <option value="overwrite">覆盖同 slug</option>
	                    <option value="rename">自动重命名</option>
	                    <option value="skip">跳过</option>
	                  </select>
	                </label>
	                <button
	                  class="w-full rounded-md bg-blue-600 px-3 py-2 text-sm text-white disabled:cursor-not-allowed disabled:opacity-60"
	                  :disabled="!diffSummary.conflicts.length || !diffSummary.fileExists"
	                  @click="handleResolveConflictsByDb"
	                >
	                  用本地库版本写回 IDE（仅冲突项）
	                </button>
	                <ul
	                  v-if="diffSummary.conflicts.length"
	                  class="max-h-44 overflow-auto rounded-md border border-gray-100 p-2 text-xs text-gray-700"
	                >
	                  <li v-for="item in diffSummary.conflicts" :key="item.slug" class="py-1">
	                    <span class="font-mono text-gray-900">{{ item.slug }}</span>
	                    <span v-if="item.name" class="ml-2 text-gray-500">{{ item.name }}</span>
	                  </li>
	                </ul>
	              </section>
	            </div>
	
	            <section v-if="diffSummary.invalid.length" class="rounded-lg border border-yellow-100 bg-yellow-50 p-4">
	              <p class="text-sm font-semibold text-yellow-800">不可比较项（字段缺失或异常）</p>
	              <ul class="mt-2 max-h-44 list-disc overflow-auto pl-5 text-xs text-yellow-800">
	                <li v-for="(item, index) in diffSummary.invalid" :key="index">
	                  <span v-if="item.slug" class="font-mono">{{ item.slug }}</span>
	                  <span v-else>（无 slug）</span>
	                  ：{{ item.reason }}
	                </li>
	              </ul>
	            </section>
	
	            <section class="rounded-lg border border-gray-100 bg-white p-4">
	              <div class="flex items-center justify-between">
	                <p class="text-sm font-semibold text-gray-900">本地库存在但 IDE 缺失</p>
	                <span class="text-xs text-gray-500">{{ diffSummary.dbOnlyTotal }} 条</span>
	              </div>
	              <p class="mt-1 text-xs text-gray-500">
	                本地库是你的模式集合，不一定需要全部写入某个实例。若要补齐，请在 “模式列表” 选择目标模式后使用“同步”。
	              </p>
	              <ul
	                v-if="diffSummary.dbOnlySample.length"
	                class="mt-3 max-h-44 overflow-auto rounded-md border border-gray-100 p-2 text-xs text-gray-700"
	              >
	                <li v-for="item in diffSummary.dbOnlySample" :key="item.slug" class="py-1">
	                  <span class="font-mono text-gray-900">{{ item.slug }}</span>
	                  <span v-if="item.name" class="ml-2 text-gray-500">{{ item.name }}</span>
	                </li>
	              </ul>
	            </section>
	
	            <p v-if="diffMessage" class="text-sm text-blue-600">{{ diffMessage }}</p>
	          </template>
	        </div>
	
	        <footer class="flex justify-end gap-3 border-t border-gray-100 px-5 py-4">
	          <button class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700" @click="refreshInstanceDiff">刷新</button>
	          <button class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700" @click="closeInstanceDiff">关闭</button>
	        </footer>
	      </div>
	    </div>
	
	    <!-- 实例模式列表 -->
	    <div v-if="showInstanceModesModal" class="fixed inset-0 z-30 flex items-center justify-center bg-black/30 backdrop-blur-sm" aria-modal="true" role="dialog">
	      <div class="w-full max-w-4xl rounded-lg bg-white shadow-xl">
	        <header class="flex items-center justify-between border-b border-gray-100 px-5 py-4">
          <div>
            <h3 class="text-lg font-semibold text-gray-900">实例模式列表</h3>
            <p class="text-sm text-gray-500">
              {{ activeInstance?.alias }}（{{ activeInstance?.kind }}）- {{ activeInstance?.path }}
            </p>
          </div>
          <button @click="closeInstanceModes" class="text-gray-400 hover:text-gray-600">✕</button>
        </header>

          <div class="space-y-4 px-5 py-4">
            <div class="flex flex-wrap items-center justify-between gap-2">
            <div class="flex flex-wrap gap-2">
              <button class="whitespace-nowrap rounded-md bg-blue-600 px-3 py-2 text-sm text-white" @click="openInstanceModeEditor()">
                新建并写回
              </button>
              <button class="whitespace-nowrap rounded-md border border-gray-200 px-3 py-2 text-sm text-gray-700" @click="refreshInstanceModes">
                刷新
              </button>
            </div>
            <p v-if="instanceModesError" class="text-sm text-red-600">{{ instanceModesError }}</p>
          </div>

          <div v-if="instanceModesLoading" class="text-sm text-gray-500">正在读取实例配置...</div>
          <div v-else-if="!instanceModes.length" class="text-sm text-gray-500">
            {{
              activeInstance?.status === 'missing'
                ? '未找到实例配置文件，无法读取 customModes。请先在 “IDE 配置” 检查路径或重新扫描。'
                : '未读取到 customModes'
            }}
          </div>
          <div v-else class="overflow-x-auto rounded-lg border border-gray-100">
            <table class="w-full table-fixed divide-y divide-gray-100 text-sm" style="min-width: 560px;">
              <thead class="bg-gray-50">
                <tr>
                  <th class="w-72 px-4 py-2 text-left text-xs font-medium text-gray-500">slug</th>
                  <th class="px-4 py-2 text-left text-xs font-medium text-gray-500">name</th>
                  <th class="w-28 px-4 py-2 text-right text-xs font-medium text-gray-500">操作</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-gray-100 bg-white">
                <tr v-for="item in instanceModes" :key="item.slug" class="hover:bg-gray-50">
                  <td class="px-4 py-3 font-mono text-xs text-gray-700">
                    <div class="truncate" :title="item.slug">{{ item.slug }}</div>
                  </td>
                  <td class="px-4 py-3 text-xs text-gray-600">
                    <div class="truncate" :title="item.name || '-'">{{ item.name || '-' }}</div>
                  </td>
                  <td class="px-4 py-3 text-right">
                    <div class="flex flex-wrap justify-end gap-2">
                      <button
                        class="whitespace-nowrap rounded-md border border-gray-200 px-3 py-1 text-xs text-gray-700"
                        @click="openInstanceModeEditor(item)"
                      >
                        编辑
                      </button>
                      <button
                        class="whitespace-nowrap rounded-md border border-red-200 px-3 py-1 text-xs text-red-600"
                        @click="handleDeleteInstanceMode(item.slug)"
                      >
                        删除
                      </button>
                    </div>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>

        <footer class="flex justify-end gap-3 border-t border-gray-100 px-5 py-4">
          <button class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700" @click="closeInstanceModes">关闭</button>
        </footer>
      </div>
    </div>

    <!-- 实例操作历史 -->
    <div v-if="showHistoryModal" class="fixed inset-0 z-30 flex items-center justify-center bg-black/30 backdrop-blur-sm" aria-modal="true" role="dialog">
      <div class="w-full max-w-4xl rounded-lg bg-white shadow-xl">
        <header class="flex items-center justify-between border-b border-gray-100 px-5 py-4">
          <div>
            <h3 class="text-lg font-semibold text-gray-900">模式写回历史</h3>
            <p class="text-sm text-gray-500">
              {{ activeInstance?.alias }}（{{ activeInstance?.kind }}）- {{ activeInstance?.path }}
            </p>
          </div>
          <button @click="closeHistory" class="text-gray-400 hover:text-gray-600">✕</button>
        </header>

        <div class="grid gap-4 px-5 py-4 md:grid-cols-2">
          <section class="space-y-3">
            <div class="flex items-center justify-between">
              <p class="text-sm font-semibold text-gray-900">历史列表</p>
              <button class="rounded-md border border-gray-200 px-3 py-2 text-sm text-gray-700" @click="refreshHistory">
                刷新
              </button>
            </div>

            <div v-if="historyLoading" class="text-sm text-gray-500">正在加载历史...</div>
            <div v-else-if="historyError" class="rounded-md bg-red-50 p-3 text-sm text-red-600">{{ historyError }}</div>
            <div v-else-if="!historyItems.length" class="text-sm text-gray-500">暂无记录</div>
            <div v-else class="rounded-lg border border-gray-100 bg-gray-50/70 p-3">
              <div class="flex flex-wrap items-center justify-between gap-2">
                <p class="text-sm font-semibold text-gray-900">概览</p>
                <p class="text-xs text-gray-500">最近一次：{{ historyStats.latestAt ? formatDateTime(historyStats.latestAt) : '-' }}</p>
              </div>
              <p class="mt-1 text-xs text-gray-500">近 {{ historyStats.total }} 条记录按操作类型统计：</p>
              <div class="mt-3 space-y-2">
                <div v-for="item in historyStats.items" :key="item.action" class="grid grid-cols-12 items-center gap-2">
                  <p class="col-span-4 truncate text-xs text-gray-700" :title="item.action">{{ item.action }}</p>
                  <div class="col-span-6 h-2 overflow-hidden rounded-full bg-gray-200">
                    <div
                      class="h-2 rounded-full bg-blue-500"
                      :style="{ width: historyStats.max ? `${Math.round((item.count / historyStats.max) * 100)}%` : '0%' }"
                    ></div>
                  </div>
                  <p class="col-span-2 text-right text-xs text-gray-500">{{ item.count }}</p>
                </div>
              </div>
            </div>
            <div v-if="!historyLoading && historyItems.length" class="max-h-[420px] overflow-auto rounded-lg border border-gray-100">
              <ul class="divide-y divide-gray-100 bg-white">
                <li
                  v-for="item in historyItems"
                  :key="item.id"
                  class="cursor-pointer px-4 py-3 hover:bg-gray-50"
                  :class="historySelectedId === item.id ? 'bg-blue-50' : ''"
                  @click="historySelectedId = item.id"
                >
                  <div class="flex items-center justify-between">
                    <p class="text-sm font-medium text-gray-900">{{ item.action }}</p>
                    <p class="text-xs text-gray-400">{{ formatDateTime(item.createdAt) }}</p>
                  </div>
                  <p class="mt-1 text-xs text-gray-500">实例：{{ item.instanceAlias || '-' }}</p>
                </li>
              </ul>
            </div>
          </section>

          <section class="space-y-3">
            <p class="text-sm font-semibold text-gray-900">详情与回放</p>
            <div v-if="!historySelectedId" class="text-sm text-gray-500">请选择一条历史记录查看详情</div>
            <template v-else>
              <div class="grid gap-3 rounded-md border border-gray-100 bg-gray-50/70 p-3 md:grid-cols-3">
                <label class="text-sm text-gray-700">
                  回放冲突策略
                  <select v-model="historyReplayStrategy" class="mt-1 w-full rounded-md border border-gray-200 px-3 py-2 text-sm">
                    <option value="overwrite">覆盖同 slug</option>
                    <option value="rename">自动重命名</option>
                    <option value="skip">跳过</option>
                  </select>
                </label>
                <label class="flex items-center gap-2 text-sm text-gray-700 md:col-span-2">
                  <input v-model="historyReplaySaveToDb" type="checkbox" class="rounded border-gray-300" />
                  回放时同时保存到本地库
                </label>
                <button class="rounded-md bg-blue-600 px-4 py-2 text-sm text-white md:col-span-3" @click="handleReplay(historySelectedId)">
                  回放到实例
                </button>
                <p v-if="historyReplayMessage" class="text-sm text-blue-600 md:col-span-3">{{ historyReplayMessage }}</p>
              </div>

              <div class="grid gap-4 md:grid-cols-1">
                <div>
                  <p class="mb-2 text-xs font-semibold text-gray-600">Before</p>
                  <pre class="max-h-[320px] overflow-auto rounded-md border border-gray-200 bg-white p-3 text-[11px] text-gray-800">{{
                    formatPayload(historyItems.find((i) => i.id === historySelectedId)?.beforePayload)
                  }}</pre>
                </div>
                <div>
                  <p class="mb-2 text-xs font-semibold text-gray-600">After</p>
                  <pre class="max-h-[320px] overflow-auto rounded-md border border-gray-200 bg-white p-3 text-[11px] text-gray-800">{{
                    formatPayload(historyItems.find((i) => i.id === historySelectedId)?.afterPayload)
                  }}</pre>
                </div>
              </div>
            </template>
          </section>
        </div>

        <footer class="flex justify-end gap-3 border-t border-gray-100 px-5 py-4">
          <button class="rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700" @click="closeHistory">关闭</button>
        </footer>
      </div>
    </div>
  </div>
</template>
