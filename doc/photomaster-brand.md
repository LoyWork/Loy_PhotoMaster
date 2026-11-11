# PhotoMaster 品牌更新

- 设置页移除原 Picturama 大幅装饰 Logo，保留设置内容和底部 PhotoMaster 版本信息。
- 图标采用 iPhone 4 / iOS 4–6 时代的拟物风格：深青色皮革相册、缝线、金属护角、米白纸页及真实山湖照片。最终透明 PNG 母版为 `src/image/app-icon.png`，由内置 image_gen 生成。完整提示词见 `photomaster-icon-prompt.md`。
- 同一透明 PNG 母版导出应用、桌面、任务栏、安装器、卸载器、Windows 商店与 macOS 图标；浏览器入口使用 PNG favicon。
- UI 标题、设置、关于菜单、许可证界面及 7 种语言中的产品名称统一为 PhotoMaster。原项目作者和许可证署名保留。
- 本版为 2.3.0。按全新应用安装，使用独立的 PhotoMaster 安装记录；保留现有照片库数据路径、更新签名与更新地址。

图标可重新生成：

```powershell
npx tauri icon src/image/app-icon.png --output src-tauri/target/photomaster-album-icons
npx tauri icon src/image/app-icon.png --output src-tauri/target/photomaster-album-source --png 1024 --png 48 --png 16
```

发布时使用 GitHub Actions 生成新安装包。按用户选择，先卸载旧版 Picturama，再安装 PhotoMaster，不加入旧版安装记录或快捷方式迁移脚本。

本次验证：49 项前端测试、7 种语言检查、前端生产构建、PhotoMaster 原生编译及 Windows NSIS 打包通过。已打开本地 PhotoMaster.exe，在实际桌面设置页确认旧装饰 Logo 消失、名称为 PhotoMaster；未运行安装器；正式包由 GitHub Actions 构建、签名并发布。

相册版已重新导出全部桌面图标、ICO 和 ICNS，后续重新打包安装后系统图标生效。之前的相机草稿未接入项目。
