# 应用更新与发布

正式构建启动后静默检查更新：没有更新或网络失败时不弹窗；发现新版本时显示版本号和更新说明。也可以在设置底部点击「检查更新」。只有用户点击「下载并安装」才会下载；完成并通过签名验证后，由 Windows 安装器自动安装、重启应用。更新失败可以重试。照片处理和模型推理仍在本机进行。

实现参考 [MangoDisk 的更新配置](https://github.com/harry0703/MangoDisk/blob/main/src-tauri/tauri.conf.json) 与 [Tauri 官方更新插件](https://v2.tauri.app/plugin/updater/)，独立实现本项目的界面和状态管理。使用 Tauri 官方插件、HTTPS 更新清单、安装包签名验证以及 Windows passive 安装模式。

## 发布方式

正式版本由 GitHub Actions 构建、签名并上传发布草稿。仓库的 `TAURI_SIGNING_PRIVATE_KEY` 已配置；Actions 只公开安装包、签名和更新清单，私钥不进入 Git 或 Release。

## CI 发布前一次性配置

本次开发已生成本地密钥：`.release-keys/picturama.key`（私钥）和 `.release-keys/picturama.key.pub`（公钥）。整个目录已被 Git 忽略；公钥已写入 `src-tauri/tauri.conf.json`。

1. 妥善备份私钥。保留同一密钥，避免已安装的版本无法验证后续更新。不要提交、上传到 Release 或在日志中输出私钥。
2. 在 GitHub 仓库 Actions secrets 配置 `TAURI_SIGNING_PRIVATE_KEY`，值为私钥文件的完整内容。本地生成的是无密码密钥，`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 留空；若日后更换为加密私钥，应配置对应密码。
3. 按现有流程同步 `package.json`、`package-lock.json`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock` 和 `src-tauri/tauri.conf.json` 中的应用版本，再推送对应的 `v*` 标签。版本号必须与发布标签一致。

CI 使用 `createUpdaterArtifacts: true` 生成签名安装包，`tauri-action` 上传安装器、`.sig` 与 `latest.json` 到 Release 草稿。确认安装包及元数据正确后再发布草稿；程序读取：

`https://github.com/LoyWork/Loy_Picturama/releases/latest/download/latest.json`

当前未上传更新清单的旧 Release 无法提供自动更新，手动检查会显示失败并允许重试。已安装的旧版没有更新入口，需要手动安装一次包含此功能的版本，此后才能通过应用内升级。没有在线发布新版本前，无法完成真实的跨版本安装验收。

开发构建不主动检查网络，但设置中的手动检查可以测试真实服务器；更新状态机的测试覆盖静默失败、手动重试、重复检查/安装、下载进度、失败时不启动安装器。

## 相似照片性能验证（2026-11-11）

改动包括：默认快速扫描；两线程并行解码与特征准备；每批 8 张提交一次数据库事务；原图先以面积采样缩到最长边 1024，再计算特征与生成预览；深度识别复用专用的未编辑预览。普通相册的编辑后缩略图不参与识别。感知特征和场景向量版本均已更新，避免混用不同预处理结果；首次扫描会重建原有特征缓存。

本机使用 24 张 4000×3000 JPEG（同一测试风景图的副本），在相同开发编译配置下对比修改前的完整扫描函数与修改后函数，未进行照片库界面渲染：

| 场景 | 修改前 | 修改后 |
| --- | ---: | ---: |
| 快速冷扫描（独立运行） | 12.897 秒 | 1.934 秒 |
| 深度冷扫描（不含模型初始化） | 44.015 秒 | 5.516 秒 |
| 快速/深度缓存扫描 | — | 约 0.002 秒 |

深度对比这一轮也运行了快速扫描，快速结果为 15.032→3.031 秒；当时后台有 Rust 构建，存在 CPU 竞争。上述结果用于验证处理开销的改善，不代表 4000 张真实混合格式照片、其他设备、机械硬盘或网络盘的总耗时，也不是识别召回率评测。真实 DINOv2 裁切/亮度变化与缓存失效测试另外验证。

可复现当前路径的手动基准（默认不在单元测试中执行）：

```powershell
cd src-tauri
cargo test --bin Picturama benchmark_cold_and_warm_library_scan -- --ignored --nocapture
# 已准备模型时，可额外测深度路径：
$env:PICTURAMA_BENCH_DEEP = '1'
cargo test --bin Picturama benchmark_cold_and_warm_library_scan -- --ignored --nocapture
```

编译环境需要按 DEVELOP.md 设置 vcpkg 和静态 CRT。测试生成临时 JPEG 与数据库，完成后清理自己的临时目录。

本次验证：49 项前端测试、80 项 Rust 测试（含真实 DINOv2 推理）、7 种语言文案检查、前端生产构建与 Git 空白检查通过。多选按钮已由用户在开发版验证，切换筛选清空选择另有回归测试。全量 `tsc --noEmit` 仍存在项目原有的 43 项类型错误；与 HEAD 源码在同一依赖环境比较，错误数量和文件/错误码分类没有增加。
