import { invoke } from '@tauri-apps/api/core'
import type {
  ApplyModesResult,
  AppSettings,
  BackupImportResult,
  BackupOptions,
  BackupPayload,
  GithubRuleEntity,
  GithubSettingsEntity,
  GithubSyncResult,
  IdeInstanceEntity,
  InstanceModeDiffSummary,
  InstanceModeItem,
  InstanceModeUpsertResult,
  ModeCompareItem,
  ModeDiffPreview,
  ModeMetaRecord,
  ModeImportReport,
  ModeEntity,
  SyncLogRecord
} from '../stores/modes'

export const backendBridge = {
  async listModes() {
    return invoke<ModeEntity[]>('list_modes')
  },
  async saveMode(payload: ModeEntity) {
    return invoke<ModeEntity>('save_mode', { payload })
  },
  async deleteMode(payload: { slug: string }) {
    return invoke<void>('delete_mode', { slug: payload.slug })
  },
  async getModeMeta(payload: { slug: string }) {
    return invoke<ModeMetaRecord>('get_mode_meta', { slug: payload.slug })
  },
  async listGithubRules() {
    return invoke<GithubRuleEntity[]>('list_github_rules')
  },
  async saveGithubRule(payload: GithubRuleEntity) {
    return invoke<GithubRuleEntity>('save_github_rule', { payload })
  },
  async deleteGithubRule(payload: { ruleId: string }) {
    return invoke<void>('delete_github_rule', { rule_id: payload.ruleId })
  },
  async listIdeInstances() {
    return invoke<IdeInstanceEntity[]>('list_ide_instances')
  },
  async saveIdeInstance(payload: IdeInstanceEntity) {
    return invoke<IdeInstanceEntity>('save_ide_instance', { payload })
  },
  async deleteIdeInstance(payload: { instanceId: string }) {
    return invoke<void>('delete_ide_instance', { instance_id: payload.instanceId })
  },
  async scanKnownIdeInstances() {
    return invoke<IdeInstanceEntity[]>('scan_known_instances')
  },
  async scanAllIdeInstances() {
    return invoke<IdeInstanceEntity[]>('scan_all_instances')
  },
  async scanInstanceModes(payload: { instanceId: string }) {
    return invoke<IdeInstanceEntity>('scan_instance_modes', { instance_id: payload.instanceId })
  },
  async listInstanceModes(payload: { instanceId: string }) {
    return invoke<InstanceModeItem[]>('list_instance_modes', { instance_id: payload.instanceId })
  },
  async getInstanceModeRaw(payload: { instanceId: string; slug: string }) {
    return invoke<Record<string, unknown> | null>('get_instance_mode_raw', {
      instance_id: payload.instanceId,
      slug: payload.slug
    })
  },
  async diffInstanceModes(payload: { instanceId: string }) {
    return invoke<InstanceModeDiffSummary>('diff_instance_modes', { instance_id: payload.instanceId })
  },
  async importInstanceModesToDb(payload: {
    instanceId: string
    modeSlugs?: string[] | null
    conflictStrategy: string
  }) {
    return invoke<ModeImportReport>('import_instance_modes_to_db', {
      instance_id: payload.instanceId,
      mode_slugs: payload.modeSlugs ?? null,
      conflict_strategy: payload.conflictStrategy
    })
  },
  async upsertInstanceMode(payload: {
    instanceId: string
    mode: Record<string, unknown>
    conflictStrategy: string
    saveToDb: boolean
  }) {
    return invoke<InstanceModeUpsertResult>('upsert_instance_mode', {
      instance_id: payload.instanceId,
      mode: payload.mode,
      conflict_strategy: payload.conflictStrategy,
      save_to_db: payload.saveToDb
    })
  },
  async deleteInstanceMode(payload: { instanceId: string; slug: string }) {
    return invoke<void>('delete_instance_mode', { instance_id: payload.instanceId, slug: payload.slug })
  },
  async previewModeDiff(payload: { text: string }) {
    return invoke<ModeDiffPreview>('preview_mode_diff', { text: payload.text })
  },
  async importModesFromText(payload: { text: string; conflictStrategy?: string | null }) {
    return invoke<ModeImportReport>('import_modes_from_text', {
      text: payload.text,
      conflict_strategy: payload.conflictStrategy ?? null
    })
  },
  async applyModesToInstances(payload: { modeSlugs: string[]; instanceIds: string[]; conflictStrategy: string }) {
    return invoke<ApplyModesResult>('apply_modes_to_instances', {
      mode_slugs: payload.modeSlugs,
      instance_ids: payload.instanceIds,
      conflict_strategy: payload.conflictStrategy
    })
  },
  async compareKiloRooModes() {
    return invoke<ModeCompareItem[]>('compare_kilo_roo_modes')
  },
  async getAppSettings() {
    return invoke<AppSettings>('get_app_settings')
  },
  async updateAppSettings(payload: AppSettings) {
    return invoke<AppSettings>('update_app_settings', { payload })
  },
  async listSyncLogs(payload?: { limit?: number; offset?: number }) {
    return invoke<SyncLogRecord[]>('list_sync_logs', {
      limit: payload?.limit ?? null,
      offset: payload?.offset ?? null
    })
  },
  async clearSyncLogs() {
    return invoke<void>('clear_sync_logs')
  },
  async listModeHistory(payload: { instanceId?: string | null; limit?: number | null; offset?: number | null }) {
    return invoke<
      Array<{
        id: string
        modeId?: string | null
        instanceId?: string | null
        instanceAlias?: string | null
        action: string
        beforePayload?: Record<string, unknown> | null
        afterPayload?: Record<string, unknown> | null
        createdAt: string
      }>
    >('list_mode_history', {
      instance_id: payload.instanceId ?? null,
      limit: payload.limit ?? null,
      offset: payload.offset ?? null
    })
  },
  async replayModeHistory(payload: {
    historyId: string
    conflictStrategy: string
    saveToDb: boolean
  }) {
    return invoke<{
      historyId: string
      instanceId: string
      result: { requestedSlug: string; finalSlug: string }
    }>('replay_mode_history', {
      history_id: payload.historyId,
      conflict_strategy: payload.conflictStrategy,
      save_to_db: payload.saveToDb
    })
  },
  async exportBackup(payload: { options: BackupOptions }) {
    return invoke<BackupPayload>('export_backup', { options: payload.options })
  },
  async exportBackupToFile(payload: { options: BackupOptions; targetDir: string }) {
    return invoke<string>('export_backup_to_file', { options: payload.options, target_dir: payload.targetDir })
  },
  async validateBackupFile(payload: { path: string }) {
    return invoke<{
      version: number
      exportedAt: string
      includeModes: boolean
      includeRules: boolean
      includeInstances: boolean
      includeSettings: boolean
      modesCount: number
      githubRulesCount: number
      ideInstancesCount: number
    }>('validate_backup_file', { path: payload.path })
  },
  async importBackupFromFile(payload: { path: string }) {
    return invoke<BackupImportResult>('import_backup_from_file', { path: payload.path })
  },
  async importBackup(payload: { payload: BackupPayload }) {
    return invoke<BackupImportResult>('import_backup', { payload: payload.payload })
  },
  async getGithubSettings() {
    return invoke<GithubSettingsEntity>('get_github_settings')
  },
  async updateGithubSettings(payload: { token: string; proxy?: string | null; delaySec: number }) {
    return invoke<void>('update_github_settings', {
      token: payload.token,
      proxy: payload.proxy ?? null,
      delay_sec: payload.delaySec
    })
  },
  async testGithubToken() {
    return invoke<{ ok: boolean; status: number; remaining?: number | null; resetAt?: string | null; message: string }>(
      'test_github_token_command'
    )
  },
  async getLogsDir() {
    return invoke<string>('get_logs_dir')
  },
  async syncGithubModes(payload: {
    query: string
    pathHint: string
    ruleId?: string | null
    ruleName?: string | null
    delaySec?: number | null
    branch?: string | null
  }) {
    return invoke<GithubSyncResult>('sync_github_modes', {
      query: payload.query,
      path_hint: payload.pathHint,
      rule_id: payload.ruleId ?? null,
      rule_name: payload.ruleName ?? null,
      delay_sec: payload.delaySec ?? null,
      branch: payload.branch ?? null
    })
  }
}
