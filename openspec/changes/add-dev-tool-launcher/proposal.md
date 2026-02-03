# Change: 新增项目开发工具选择与默认工具

## Why
用户希望在打开项目时可以选择具体的开发工具，并支持维护个人常用工具与默认打开方式。

## What Changes
- 新增“开发工具”列表（内置常见 IDE 预设 + 自定义工具），并支持启用/禁用
- 支持设置默认开发工具，用于一键打开项目
- 项目卡片新增“开发工具”按钮，可选择工具打开项目
- 仅展示检测到已安装的内置预设工具

## Impact
- Affected specs: dev-tools
- Affected code: src/models/types.ts, src/components/SettingsModal.tsx, src/components/ProjectCard.tsx, src/App.tsx, src-tauri/src/system.rs, src-tauri/src/models.rs
