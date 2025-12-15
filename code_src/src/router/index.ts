import { createRouter, createWebHistory } from 'vue-router'
import type { RouteRecordRaw } from 'vue-router'

import Home from '../views/Home/index.vue'
import ModeLibraryView from '../views/Library/ModeLibraryView.vue'
import GithubSyncView from '../views/Github/GithubSyncView.vue'
import IdeConfigView from '../views/Ide/IdeConfigView.vue'
import SettingsView from '../views/Settings/SettingsView.vue'

const routes: RouteRecordRaw[] = [
  {
    path: '/',
    name: 'Dashboard',
    component: Home
  },
  {
    path: '/library',
    name: 'Library',
    component: ModeLibraryView
  },
  {
    path: '/github-sync',
    name: 'GithubSync',
    component: GithubSyncView
  },
  {
    path: '/ide',
    name: 'IdeConfig',
    component: IdeConfigView
  },
  {
    path: '/settings',
    name: 'Settings',
    component: SettingsView
  }
]

const router = createRouter({
  history: createWebHistory(),
  routes
})

export default router
