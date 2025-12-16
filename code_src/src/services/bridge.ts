import { invoke } from '@tauri-apps/api/core'
import type {
  GithubRuleEntity,
  GithubSettingsEntity,
  GithubSyncResult,
  IdeInstanceEntity,
  ModeEntity
} from '../stores/modes'

export const backendBridge = {
  async listModes() {
    return invoke<ModeEntity[]>('list_modes')
  },
  async saveMode(payload: ModeEntity) {
    return invoke<ModeEntity>('save_mode', { payload })
  },
  async listGithubRules() {
    return invoke<GithubRuleEntity[]>('list_github_rules')
  },
  async saveGithubRule(payload: GithubRuleEntity) {
    return invoke<GithubRuleEntity>('save_github_rule', { payload })
  },
  async listIdeInstances() {
    return invoke<IdeInstanceEntity[]>('list_ide_instances')
  },
  async saveIdeInstance(payload: IdeInstanceEntity) {
    return invoke<IdeInstanceEntity>('save_ide_instance', { payload })
  },
  async scanKnownIdeInstances() {
    return invoke<IdeInstanceEntity[]>('scan_known_instances')
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
  async syncGithubModes(payload: { query: string; pathHint: string }) {
    return invoke<GithubSyncResult>('sync_github_modes', {
      query: payload.query,
      path_hint: payload.pathHint
    })
  }
}
