# Repository Guidelines

## 项目结构与模块组织
仓库业务代码集中在 `code_src/`，其中 `src/` 存放 Vue 3 前端，按 `components/`、`views/`、`router/`、`stores/`、`services/`、`composables/`、`assets/` 划分，方便模块化协作；`src-tauri/` 保存 Tauri 宿主配置与 Rust 侧资源；`public/` 挂载静态资产；`docs/` 可放对外说明文档。提交前请确认新增目录遵循现有分层，例如自定义模式逻辑优先落在 `src/views` 与 `src/stores`，共享逻辑抽离到 `composables/`。

## 构建、测试与开发命令
- `pnpm install`：一次性安装前端与 Tauri 依赖。
- `pnpm dev`：启动 Vite 前端开发服务器，默认监听 http://localhost:5173。
- `pnpm build`：先运行 `vue-tsc --noEmit` 类型检查，再生成生产构建供 Tauri 或 Web 分发。
- `pnpm preview`：以本地静态服务器验证构建结果。
- `pnpm tauri dev` / `pnpm tauri build`：分别调试与打包桌面应用，命令会读取 `src-tauri/tauri.conf.json`。

## 代码风格与命名约定
TypeScript 组件使用 2 空格缩进与单引号，Vue 单文件组件遵循 `<script setup lang="ts">` 结构；组件、Pinia Store、路由名称采用 PascalCase（如 `ModeDashboardView`），组合式函数以 `useXxx` 命名，Tailwind 原子类优先于手写 CSS，必要时在 `src/style.css` 内添加具名类并附中文注释；API 方法集中在 `services/`，统一返回 Promise 并在调用侧使用 `await`。

## 测试指引
目前模板尚未集成专用测试框架，新增功能至少自测以下内容：组件交互路径、Pinia 状态持久化、Tauri 调用回路。若引入单元测试，首选 Vitest 与 Vue Test Utils，测试文件放置在与源文件同级的 `__tests__` 目录（命名 `*.spec.ts`），并在 `package.json` 增补 `pnpm test`，覆盖率低于 80% 时不得合并。

## 提交与 PR 指南
仓库缺少历史 commit，可参考 Conventional Commits（如 `feat: 支持批量导入模式`、`fix: 修复 tauri window card`）；PR 需包含：变更摘要、对应需求或 Issue、可复现步骤与截图/录屏（若 UI 相关）、测试结果与构建日志；保持单一主题，若涉及配置或安全文件（例如 `tauri.conf.json`、`.env`），需注明风险及回滚方式。

## 安全与配置提示
API Host、密钥等敏感信息通过 `.env` 或 Tauri `env` 注入，切勿直接写入 `src/services`；桌面端高权限操作必须在 Rust 侧校验输入，前端仅负责参数收集；调试桌面功能时，使用 `pnpm tauri dev -- --log-level debug` 观察安全警告，构建包前运行 `pnpm build && pnpm tauri build` 确保 Web 端资源无源映射泄露。
