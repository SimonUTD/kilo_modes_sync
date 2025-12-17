import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { backendBridge } from '../services/bridge'

export type ModeSource = 'local' | 'github' | 'ide'
export type IdeKind = 'kilocode' | 'roocode'

export interface ModeEntity {
  id: string
  slug: string
  name: string
  description: string
  groups: string[]
  roleDefinition: string
  roleDefinitionLength: number
  source: ModeSource
  whenToUse?: string | null
  customInstructions?: string | null
  payload?: Record<string, unknown> | null
  updatedAt: string
  hash: string
}

export interface GithubRuleEntity {
  id: string
  name: string
  query: string
  pathHint: string
  branch: string
  enabled: boolean
  delaySec: number
  lastRunAt?: string | null
}

export interface GithubSyncResult {
  fetchedFiles: number
  savedModes: number
  skippedDueToMissingFields: number
  errors: string[]
}

export interface GithubTokenTestResult {
  ok: boolean
  status: number
  remaining?: number | null
  resetAt?: string | null
  message: string
}

export interface GithubSettingsEntity {
  token: string
  proxy?: string | null
  delaySec: number
  lastResult?: GithubSyncResult | null
  lastTokenTestPassedAt?: string | null
}

export interface ModeImportReport {
  discovered: number
  saved: number
  skippedDueToMissingFields: number
  duplicateSlug: number
  duplicateHash: number
  errors: string[]
}

export interface ModeDiffPreviewItem {
  slug: string
  name: string
  contentHash: string
  status: string
  recommendedAction: string
  existingSlug?: string | null
  existingHash?: string | null
  renameSuggestion?: string | null
  missingFields: string[]
}

export interface ModeDiffPreview {
  discovered: number
  newModes: number
  duplicates: number
  conflicts: number
  invalid: number
  items: ModeDiffPreviewItem[]
}

export interface ApplyInstanceResult {
  instanceId: string
  alias: string
  path: string
  applied: number
  overwritten: number
  renamed: number
  skipped: number
  status: string
  messages: string[]
}

export interface ApplyModesResult {
  totalInstances: number
  updatedInstances: number
  skippedInstances: number
  errors: string[]
  details: ApplyInstanceResult[]
}

export interface ModeCompareItem {
  slug: string
  inKilocode: boolean
  inRoocode: boolean
}

export interface ModeMetaRecord {
  rawPayload?: Record<string, unknown> | null
  sourcePath?: string | null
  sourceAlias?: string | null
}

export interface AppSettings {
  enableLog: boolean
  logLevel: string
  retentionDays: number
  showRoleDefinitionLength: boolean
  qualityThreshold: number
  autoDeduplicate: boolean
}

export interface SyncLogRecord {
  id: string
  syncKind: string
  ruleId?: string | null
  ruleName?: string | null
  target?: string | null
  status: string
  message?: string | null
  createdAt: string
}

export interface BackupOptions {
  includeModes: boolean
  includeRules: boolean
  includeInstances: boolean
  includeSettings: boolean
}

export interface BackupModeRecord {
  id: string
  slug: string
  name: string
  description: string
  groups: string[]
  roleDefinition: string
  roleDefinitionLength: number
  source: string
  whenToUse?: string | null
  customInstructions?: string | null
  payload?: Record<string, unknown> | null
  rawPayload?: Record<string, unknown> | null
  sourcePath?: string | null
  sourceAlias?: string | null
  updatedAt: string
  contentHash: string
}

export interface BackupPayload {
  version: number
  exportedAt: string
  options: BackupOptions
  modes: BackupModeRecord[]
  githubRules: GithubRuleEntity[]
  ideInstances: IdeInstanceEntity[]
  githubSettingsJson?: string | null
  appSettingsJson?: string | null
}

export interface BackupImportResult {
  importedModes: number
  skippedDuplicateModes: number
  importedRules: number
  importedInstances: number
  updatedSettings: boolean
  errors: string[]
}

export interface BackupFileMeta {
  version: number
  exportedAt: string
  includeModes: boolean
  includeRules: boolean
  includeInstances: boolean
  includeSettings: boolean
  modesCount: number
  githubRulesCount: number
  ideInstancesCount: number
}

export interface InstanceModeItem {
  slug: string
  name?: string | null
  raw: Record<string, unknown>
}

export interface InstanceModeUpsertResult {
  requestedSlug: string
  finalSlug: string
}

export interface InstanceModeDiffOnlyItem {
  slug: string
  name?: string | null
}

export interface InstanceModeDiffConflictItem {
  slug: string
  name?: string | null
  dbHash: string
  ideHash: string
}

export interface InstanceModeDiffInvalidItem {
  slug?: string | null
  reason: string
}

export interface InstanceModeDiffSummary {
  instanceId: string
  alias: string
  kind: IdeKind
  path: string
  fileExists: boolean
  status: 'synced' | 'outdated' | 'missing'
  totalDb: number
  totalIde: number
  same: number
  conflicts: InstanceModeDiffConflictItem[]
  ideOnly: InstanceModeDiffOnlyItem[]
  invalid: InstanceModeDiffInvalidItem[]
  dbOnlyTotal: number
  dbOnlySample: InstanceModeDiffOnlyItem[]
}

export interface ModeHistoryRecord {
  id: string
  modeId?: string | null
  instanceId?: string | null
  instanceAlias?: string | null
  action: string
  beforePayload?: Record<string, unknown> | null
  afterPayload?: Record<string, unknown> | null
  createdAt: string
}

export interface ModeHistoryReplayResult {
  historyId: string
  instanceId: string
  result: InstanceModeUpsertResult
}

export interface IdeInstanceEntity {
  id: string
  alias: string
  kind: IdeKind
  path: string
  lastScanAt?: string | null
  modesCount: number
  status: 'synced' | 'outdated' | 'missing'
  selected: boolean
}

export const useModeStore = defineStore('mode', () => {
  const modes = ref<ModeEntity[]>([])
  const githubRules = ref<GithubRuleEntity[]>([])
  const githubSettings = ref<GithubSettingsEntity | null>(null)
  const appSettings = ref<AppSettings | null>(null)
  const ideInstances = ref<IdeInstanceEntity[]>([])
  const lastSyncLog = ref<string>('')
  const roleDefinitionThreshold = ref(800)
  const initialized = ref(false)
  const loading = ref(false)
  const error = ref<string | null>(null)

  const highQualityModes = computed(() =>
    modes.value.filter((item) => item.roleDefinitionLength >= roleDefinitionThreshold.value)
  )

  const groupedBySource = computed(() => {
    return modes.value.reduce<Record<ModeSource, ModeEntity[]>>(
      (acc, mode) => {
        acc[mode.source].push(mode)
        return acc
      },
      { local: [], github: [], ide: [] }
    )
  })

  async function bootstrap(force = false) {
    if (initialized.value && !force) return
    loading.value = true
    error.value = null
    try {
      const [modeData, ruleData, instanceData] = await Promise.all([
        backendBridge.listModes(),
        backendBridge.listGithubRules(),
        backendBridge.listIdeInstances()
      ])
      modes.value = modeData
      githubRules.value = ruleData
      ideInstances.value = instanceData
      initialized.value = true
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err)
      throw err
    } finally {
      loading.value = false
    }
  }

  async function saveMode(entry: ModeEntity) {
    const saved = await backendBridge.saveMode(entry)
    upsertModeLocal(saved)
    return saved
  }

  async function deleteMode(slug: string) {
    await backendBridge.deleteMode({ slug })
    modes.value = modes.value.filter((item) => item.slug !== slug)
  }

  async function getModeMeta(slug: string) {
    return backendBridge.getModeMeta({ slug })
  }

  function upsertModeLocal(entry: ModeEntity) {
    const index = modes.value.findIndex((item) => item.slug === entry.slug)
    if (index >= 0) {
      modes.value[index] = entry
    } else {
      modes.value.push(entry)
    }
  }

  async function saveGithubRule(entry: GithubRuleEntity) {
    const saved = await backendBridge.saveGithubRule(entry)
    const index = githubRules.value.findIndex((item) => item.id === saved.id)
    if (index >= 0) {
      githubRules.value[index] = saved
    } else {
      githubRules.value.push(saved)
    }
    return saved
  }

  async function deleteGithubRule(ruleId: string) {
    await backendBridge.deleteGithubRule({ ruleId })
    githubRules.value = githubRules.value.filter((item) => item.id !== ruleId)
  }

  function updateRuleRunTime(ruleId: string, timestamp: string) {
    const target = githubRules.value.find((rule) => rule.id === ruleId)
    if (target) {
      target.lastRunAt = timestamp
    }
  }

  async function fetchGithubSettings() {
    const data = await backendBridge.getGithubSettings()
    githubSettings.value = {
      token: data.token,
      proxy: data.proxy,
      delaySec: data.delaySec,
      lastResult: data.lastResult,
      lastTokenTestPassedAt: data.lastTokenTestPassedAt ?? null
    }
    return githubSettings.value
  }

  async function fetchAppSettings() {
    const settings = await backendBridge.getAppSettings()
    appSettings.value = settings
    roleDefinitionThreshold.value = settings.qualityThreshold
    return settings
  }

  async function updateAppSettings(payload: AppSettings) {
    const settings = await backendBridge.updateAppSettings(payload)
    appSettings.value = settings
    roleDefinitionThreshold.value = settings.qualityThreshold
    return settings
  }

  async function listSyncLogs(payload?: { limit?: number; offset?: number }) {
    return backendBridge.listSyncLogs(payload)
  }

  async function clearSyncLogs() {
    return backendBridge.clearSyncLogs()
  }

  async function exportBackup(options: BackupOptions) {
    return backendBridge.exportBackup({ options })
  }

  async function importBackup(payload: BackupPayload) {
    return backendBridge.importBackup({ payload })
  }

  async function exportBackupToFile(payload: { options: BackupOptions; targetDir: string }) {
    return backendBridge.exportBackupToFile(payload)
  }

  async function validateBackupFile(path: string) {
    return backendBridge.validateBackupFile({ path })
  }

  async function importBackupFromFile(path: string) {
    return backendBridge.importBackupFromFile({ path })
  }

  async function getLogsDir() {
    return backendBridge.getLogsDir()
  }

  async function listInstanceModes(instanceId: string) {
    return backendBridge.listInstanceModes({ instanceId })
  }

  async function getInstanceModeRaw(payload: { instanceId: string; slug: string }) {
    return backendBridge.getInstanceModeRaw(payload)
  }

  async function upsertInstanceMode(payload: {
    instanceId: string
    mode: Record<string, unknown>
    conflictStrategy: 'overwrite' | 'rename' | 'skip'
    saveToDb: boolean
  }) {
    return backendBridge.upsertInstanceMode(payload)
  }

  async function deleteInstanceMode(payload: { instanceId: string; slug: string }) {
    return backendBridge.deleteInstanceMode(payload)
  }

  async function diffInstanceModes(instanceId: string) {
    return backendBridge.diffInstanceModes({ instanceId })
  }

  async function importInstanceModesToDb(payload: {
    instanceId: string
    modeSlugs?: string[] | null
    conflictStrategy: 'overwrite' | 'rename' | 'skip'
  }) {
    return backendBridge.importInstanceModesToDb({
      instanceId: payload.instanceId,
      modeSlugs: payload.modeSlugs ?? null,
      conflictStrategy: payload.conflictStrategy
    })
  }

  async function listModeHistory(payload: { instanceId?: string | null; limit?: number; offset?: number }) {
    return backendBridge.listModeHistory({
      instanceId: payload.instanceId ?? null,
      limit: payload.limit ?? null,
      offset: payload.offset ?? null
    })
  }

  async function replayModeHistory(payload: { historyId: string; conflictStrategy: string; saveToDb: boolean }) {
    return backendBridge.replayModeHistory(payload)
  }

  async function updateGithubSettings(payload: { token: string; proxy?: string | null; delaySec: number }) {
    await backendBridge.updateGithubSettings(payload)
    if (!githubSettings.value) {
      githubSettings.value = { ...payload, lastResult: null }
    } else {
      githubSettings.value.token = payload.token
      githubSettings.value.proxy = payload.proxy
      githubSettings.value.delaySec = payload.delaySec
    }
  }

  async function testGithubToken() {
    return backendBridge.testGithubToken()
  }

  async function syncGithubRule(payload: {
    query: string
    pathHint: string
    ruleId?: string | null
    ruleName?: string | null
    delaySec?: number | null
    branch?: string | null
  }) {
    const result = await backendBridge.syncGithubModes(payload)
    if (!githubSettings.value) {
      githubSettings.value = { token: '', proxy: null, delaySec: 3, lastResult: result }
    } else {
      githubSettings.value.lastResult = result
    }
    return result
  }

  async function previewModeDiff(text: string) {
    return backendBridge.previewModeDiff({ text })
  }

  async function importModesFromText(payload: { text: string; conflictStrategy?: 'overwrite' | 'rename' | 'skip' | null }) {
    const result = await backendBridge.importModesFromText({ text: payload.text, conflictStrategy: payload.conflictStrategy ?? null })
    await bootstrap(true)
    return result
  }

  async function applyModesToInstances(payload: { modeSlugs: string[]; instanceIds: string[]; conflictStrategy: string }) {
    return backendBridge.applyModesToInstances(payload)
  }

  async function compareKiloRooModes() {
    return backendBridge.compareKiloRooModes()
  }

  async function saveIdeInstance(entry: IdeInstanceEntity) {
    const saved = await backendBridge.saveIdeInstance(entry)
    const index = ideInstances.value.findIndex((item) => item.id === saved.id)
    if (index >= 0) {
      ideInstances.value[index] = saved
    } else {
      ideInstances.value.push(saved)
    }
    return saved
  }

  async function deleteIdeInstance(instanceId: string) {
    await backendBridge.deleteIdeInstance({ instanceId })
    ideInstances.value = ideInstances.value.filter((item) => item.id !== instanceId)
  }

  async function scanKnownInstances() {
    const synced = await backendBridge.scanKnownIdeInstances()
    const instanceMap = new Map(ideInstances.value.map((item) => [item.id, item]))
    synced.forEach((item) => {
      instanceMap.set(item.id, item)
    })
    ideInstances.value = Array.from(instanceMap.values())
    await bootstrap(true)
    return synced
  }

  async function scanAllInstances() {
    const synced = await backendBridge.scanAllIdeInstances()
    ideInstances.value = synced
    await bootstrap(true)
    return synced
  }

  async function scanInstanceModes(instanceId: string) {
    const updated = await backendBridge.scanInstanceModes({ instanceId })
    const index = ideInstances.value.findIndex((item) => item.id === updated.id)
    if (index >= 0) {
      ideInstances.value[index] = updated
    }
    await bootstrap(true)
    return updated
  }

  function updateSyncLog(message: string) {
    lastSyncLog.value = message
  }

  return {
    modes,
    githubRules,
    githubSettings,
    appSettings,
    ideInstances,
    lastSyncLog,
    roleDefinitionThreshold,
    initialized,
    loading,
    error,
    highQualityModes,
    groupedBySource,
    bootstrap,
    saveMode,
    deleteMode,
    getModeMeta,
    saveGithubRule,
    deleteGithubRule,
    updateRuleRunTime,
    fetchAppSettings,
    updateAppSettings,
    listSyncLogs,
    clearSyncLogs,
    exportBackup,
    importBackup,
    exportBackupToFile,
    validateBackupFile,
    importBackupFromFile,
    getLogsDir,
    listInstanceModes,
    getInstanceModeRaw,
    upsertInstanceMode,
    deleteInstanceMode,
    diffInstanceModes,
    importInstanceModesToDb,
    listModeHistory,
    replayModeHistory,
    fetchGithubSettings,
    updateGithubSettings,
    testGithubToken,
    syncGithubRule,
    previewModeDiff,
    importModesFromText,
    applyModesToInstances,
    compareKiloRooModes,
    saveIdeInstance,
    deleteIdeInstance,
    scanKnownInstances,
    scanAllInstances,
    scanInstanceModes,
    updateSyncLog
  }
})
