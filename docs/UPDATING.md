# 在线更新与发布

关于页沿用 Portiva 的 Tauri 官方 updater 方案。更新源为：

`https://github.com/gshu55/droidpilot/releases/latest/download/latest.json`

仅在用户点击时检查和安装。检查最多等待 60 秒；下载允许 30 分钟，临时网络错误最多尝试 3 次。
更新包必须通过内置公钥的签名校验才会安装。发布说明按纯文本展示，不执行远程 HTML。
安装会退出并重启应用；开始前结束录屏、终端和文件传输任务。

## 首次配置

独立签名密钥已生成在本地 `.release-signing/droidpilot.key`，公钥已写入
`src-tauri/tauri.conf.json`。该目录被 Git 忽略。备份私钥，不要提交到仓库或输出到日志。

在仓库 Settings → Secrets and variables → Actions 中添加：

- `TAURI_SIGNING_PRIVATE_KEY`：上述私钥文件的完整内容。
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：本次密钥没有密码，可不设置；更换加密密钥时才需提供密码。

不要使用 Portiva 的签名私钥，也不要在后续版本中重新生成密钥，否则已安装客户端不能验证更新。
签名与 Windows Authenticode 是不同机制，此处不提供 Windows 代码签名证书。

## 发布新版

1. 同步修改 `package.json`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock` 和 `src-tauri/tauri.conf.json` 的应用版本。
2. 添加对应的 `docs/releases/vX.Y.Z.md`。
3. 提交、推送，并创建对应的 `vX.Y.Z` 标签。
4. Actions 检查签名配置、测试、构建，并生成安装包签名和 `latest.json`。
5. 所有文件先上传到草稿 Release，再发布为 latest，避免客户端读到不完整的更新。

工作流也支持手动运行，按当前 main 中的应用版本查找已存在的不可变标签构建。
若上次上传失败遗留草稿，应检查草稿后再重试；不要覆盖已发布版本的安装包。

## 平台与首次迁移

当前发布工作流只生成 Windows x64 的在线更新包。其他平台使用“查看所有版本”手动下载；
未来添加其他平台时，需要为清单加入对应目标和签名包。

Windows 使用 NSIS 安装程序更新。绿色版也可检查和安装，但会转为安装版，原独立 EXE 不会被替换。
v1.1.2 尚无更新模块和 `latest.json`：用户需要手动安装首个带此功能的版本，之后才可在线更新。
在首个签名 Release 发布前，“检查更新”会显示获取更新失败，并提供手动下载入口。
