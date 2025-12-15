# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

这是一个 KiloCode/Roo Code 自定义模式管理工具，用于统一管理和同步多个 IDE（Trae、Cursor、Windsurf、CodeBuddy 等）中的自定义模式配置。

**技术栈**：Tauri 2 + Vue 3 + TypeScript + Vite + Pinia + Tailwind CSS + SQLite

## 开发命令

### 前端开发
```bash
# 安装依赖（首次运行）
pnpm install

# 启动 Vite 开发服务器（仅前端，http://localhost:5173）
pnpm dev

# 类型检查 + 构建生产版本
pnpm build

# 预览构建结果
pnpm preview
```

### Tauri 桌面应用
```bash
# 启动 Tauri 开发环境（前端 + Rust 后端）
pnpm tauri dev

# ���建桌面应用（Windows/macOS）
pnpm tauri build

# 调试模式构建（带日志）
pnpm tauri dev -- --log-level debug
```

## 项目架构

### 目录结构
```
code_src/
├── src/                    # Vue 3 前端代码
│   ├── views/             # 页面组件（PascalCase 命名）
│   ├── components/        # 通用组件
│   ├── router/            # Vue Router 配置
│   ├── stores/            # Pinia 状态管理（Setup Store 模式）
│   ├── services/          # API 服务层（统一 Promise/async-await）
│   ├── composables/       # 组合式函数（useXxx 命名）
│   └── assets/            # 静态资源
├── src-tauri/             # Tauri Rust 后端
│   ├── src/lib.rs         # Tauri 命令定义
│   ├── Cargo.toml         # Rust 依赖
│   └── tauri.conf.json    # Tauri 配置
└── public/                # 静态资产
```

### 核心架构原则

**前后端分离**：
- **前端（Vue）**：仅负责 UI 渲染、用户交互、状态管理
- **后端（Rust）**：处理所有文件系统操作、SQLite 数据库访问、GitHub API 调用
- **通信方式**：通过 Tauri 的 `invoke()` 调用 Rust 命令

**关键约束**：
- ⚠️ **禁止前端直接访问文件系统**：Tauri 有严格的安全沙箱，所有文件操作必须在 Rust 侧实现
- ⚠️ **路径兼容性**：macOS 和 Windows 对空格路径处理不同，Rust 侧需统一处理
- ⚠️ **macOS 不使用沙箱**：需要访问用户配置文件（`~/.config/kilocode/` 等）

### 数据流架构

```
GitHub API → Rust 后端 → SQLite 本地数据库 → Rust 后端 → Vue 前端
                ↓                                    ↓
            本地文件系统                        IDE 配置文件同步
```

**核心流程**：
1. 所有数据输入（GitHub 抓取、用户录入）先保存到 SQLite
2. 从数据库读取后进行同步操作
3. 使用内容 hash 去重，避免重复存储

## 代码规范

### TypeScript/Vue 规范
- **缩进**：2 空格
- **引号**：单引号
- **组件结构**：`<script setup lang="ts">` + Composition API
- **命名约定**：
  - 组件/Store/路由：PascalCase（如 `ModeLibraryView`）
  - 组合式函数：`useXxx`（如 `useTheme`）
  - 文件名：与组件名一致或 `index.vue`
- **样式**：优先使用 Tailwind 原子类，必要时在 `src/style.css` 添加具名类并附中文注释

### Rust 规范
- **命令定义**：使用 `#[tauri::command]` 宏
- **错误处理**：返回 `Result<T, String>` 并提供清晰的中文错误信息
- **输入验证**：所有前端传入的路径/参数必须在 Rust 侧校验

### 注释规范
- **语言**：所有注释、文档、提交信息必须使用简体中文
- **内容**：描述意图和约束，而非重复代码逻辑
- **禁止**：不写"修改说明"式注释（由 Git 承担）

## 业务逻辑关键点

### 自定义模式（Custom Mode）字段
**必填字段**：
- `slug`：唯一标识符
- `name`：显示名称
- `description`：描述
- `roleDefinition`：角色定义（用于质量评估）
- `groups`：工具组权限列表
- `source`：来源标记（`local`/`github`/`ide-name`）

**选填字段**：
- `whenToUse`、`customInstructions`
- GitHub 抓取的额外字段需透传保留

**特殊处理**：
- YAML 多行字段使用 `|-` 或 `>-` 标记
- `groups` 支持二级嵌套（文件正则过滤）

### IDE 配置路径扫描
**预设白名单**（主流 IDE 默认路径）：
- VS Code：`~/.config/Code/User/globalStorage/kilocode.kilocode/`
- Trae：`~/.config/Trae/User/globalStorage/kilocode.kilocode/`
- Cursor：`~/.config/Cursor/User/globalStorage/kilocode.kilocode/`
- Windsurf：`~/.config/Windsurf/User/globalStorage/kilocode.kilocode/`

**实例别名机制**：扫描后为每个配置文件分配别名（如 "KiloCode - Trae版"）

### 同步冲突处理
当检测到同名 mode 时，提供三种策略：
1. **覆盖**：用新 mode 替换现有 mode
2. **重命名**：自动为新 mode 添加后缀（如 `-v2`）
3. **放弃**：跳过导入

## 测试要求

目前未集成测试框架，新功能必须手动测试：
- 组件交互路径（按钮点击、表单提交）
- Pinia 状态持久化
- Tauri 命令调用回路（前端 `invoke()` → Rust 命令 → 返回结果）

**未来集成测试框架时**：
- 使用 Vitest + Vue Test Utils
- 测试文件放在 `__tests__/` 目录，命名 `*.spec.ts`
- 覆盖率要求 ≥80%

## 安全注意事项

- **敏感信息**：GitHub Token 等通过 `.env` 或 Tauri 环境变量注入，禁止硬编码
- **输入验证**：所有用户输入（路径、URL、配置内容）必须在 Rust 侧校验
- **权限控制**：高权限操作（文件写入、系统命令）仅在 Rust 侧执行
- **构建检查**：生产构建前运行 `pnpm build` 确保无类型错误和源映射泄露

## UI 布局约定

**整体布局**：左侧菜单 + 右侧内容区

**菜单结构**：
1. GitHub 同步
2. 库管理（本地模式列表）
3. IDE 配置管理（多实例同步）
4. 设置（GitHub Token、搜索规则、代理配置）

## 重要提醒

- **不支持多用户**：单机本地工具，无用户管理
- **不考虑并发**：IDE 使用时的文件锁冲突概率极低，忽略
- **GitHub 批量导入**：一个文件包含多个 mode 时，直接分拆入库，无需中间选择
- **去重机制**：使用内容 hash 去重，记录来源标记和保存时间
- **错误容忍**：GitHub 数据处理时，单个 mode 解析失败应跳过而非中断整个流程
