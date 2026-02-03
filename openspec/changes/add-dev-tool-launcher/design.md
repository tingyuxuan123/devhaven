## Context
需要在项目卡片中选择并使用开发工具打开项目，同时允许用户维护工具列表与默认工具。该能力涉及前端设置界面、状态持久化与后端命令执行。

## Goals / Non-Goals
- Goals:
  - 提供内置常见 IDE 预设（VS Code / JetBrains 系列等）
  - 仅展示已检测到安装的预设工具
  - 支持自定义工具与默认工具
- Non-Goals:
  - 不做项目级别的工具绑定（仅全局默认）
  - 不提供复杂的命令模板编辑器（仅 {path} 占位符）

## Decisions
- 将开发工具配置存入 AppSettings，结构包含：id、name、commandPath、arguments、enabled、isPreset
- 预设检测在后端完成：按 OS 检查常见安装路径/命令是否存在
- 前端只渲染“检测可用的预设 + 用户自定义”
- 打开项目使用已有 open_in_editor 逻辑（传入 commandPath/arguments），不新增重复 API
- Windows 端 JetBrains 系列优先检测 Toolbox 安装路径

## Risks / Trade-offs
- 预设检测为“尽力而为”，可能漏检或误判 → 允许用户添加自定义工具作为兜底

## Migration Plan
- 增加 settings 版本与默认值，旧数据自动补齐

## Open Questions
- 暂无
