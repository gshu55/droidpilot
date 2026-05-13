# DroidPilot 智控助手 v1.0.0

基于 Tauri + Vue3 + TypeScript 的桌面端 ADB 工具，面向开发、测试和排障场景，提供常用命令快捷执行、设备识别、文件上传下载、历史与收藏等能力。

## 功能概览
- ADB 可用性检测与设备列表展示
- 命令输入、快捷按钮、自动联想
- 命令历史与收藏
- 文件上传（push）与下载（pull）
- 命令输出记录与传输记录

## 快速开始
1. 安装依赖：`npm install`
2. 开发模式：`npm run tauri dev`
3. 生产打包：`npm run tauri build`
4. Windows 安装包（NSIS）：`npm run build:win:exe`
5. Windows 绿色版单文件：`npm run build:win:portable`
   - 产物路径：`src-tauri/target/release/droidpilot.exe`

## 交付文档
- 用户手册：`docs/USER_GUIDE.md`
- 发布说明：`docs/RELEASE_NOTES.md`
- 需求文档：`docs/README.md`

## 项目技术栈
- 桌面框架：Tauri 2
- 前端框架：Vue 3 + TypeScript + Pinia
- 构建工具：Vite
- 后端执行：Rust Command 调用 adb
