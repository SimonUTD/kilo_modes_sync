import { invoke } from '@tauri-apps/api/core'
import type {
  GithubRuleEntity,
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
  }
}
