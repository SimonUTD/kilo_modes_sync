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

export interface GithubSettingsEntity {
  token: string
  proxy?: string | null
  delaySec: number
  lastResult?: GithubSyncResult | null
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
      lastResult: data.lastResult
    }
    return githubSettings.value
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

  async function syncGithubRule(payload: { query: string; pathHint: string }) {
    const result = await backendBridge.syncGithubModes(payload)
    if (!githubSettings.value) {
      githubSettings.value = { token: '', proxy: null, delaySec: 3, lastResult: result }
    } else {
      githubSettings.value.lastResult = result
    }
    return result
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

  async function scanKnownInstances() {
    const synced = await backendBridge.scanKnownIdeInstances()
    const instanceMap = new Map(ideInstances.value.map((item) => [item.id, item]))
    synced.forEach((item) => {
      instanceMap.set(item.id, item)
    })
    ideInstances.value = Array.from(instanceMap.values())
    return synced
  }

  function updateSyncLog(message: string) {
    lastSyncLog.value = message
  }

  return {
    modes,
    githubRules,
    githubSettings,
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
    saveGithubRule,
    updateRuleRunTime,
    fetchGithubSettings,
    updateGithubSettings,
    syncGithubRule,
    saveIdeInstance,
    scanKnownInstances,
    updateSyncLog
  }
})
