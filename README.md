# 🎵 音乐下载

> 安卓端音乐下载器：Tauri 2 + Vue 3 界面，Rust 下载核心，Material Design 3 手机优先设计。

![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)
![Platform](https://img.shields.io/badge/platform-Android%20arm64--v8a-3DDC84.svg)
![Rust](https://img.shields.io/badge/rust-1.77.2+-orange.svg)
![Node](https://img.shields.io/badge/node-22.12+-green.svg)

---

## ✨ 特性

- 🔍 **音乐搜索**：关键词搜索、搜索建议和热搜词，结果分页加载；点击歌手或专辑查看详情，返回时保留搜索状态与滚动位置
- 🎧 **六大内置音源**：QQ 音乐、酷我音乐、酷狗音乐、网易云音乐、哔哩哔哩、咪咕音乐全部内置，无需填写任何音源配置；QQ 音乐与酷我音乐支持歌手 / 专辑 / 歌单，其余四者提供歌曲搜索与下载
- 📋 **歌单搜索与导入**：支持歌单关键词搜索，或通过歌单链接 / ID 导入并批量下载
- ⬇️ **批量与智能下载**：列表中勾选多首统一入队；多任务并发、断点续传、下载链接按需刷新与自动重试
- 🔄 **自动降级**：以指定音质为起点，按设置中的顺序选择可用品质
- 🎵 **音频解密与标签**：支持加密格式音频解密；写入封面与歌词标签，也可单独保存 `.lrc` 歌词文件
- 🔐 **匿名下载**：本版本已移除应用内登录入口，默认匿名解析；设备上若仍保留旧版本的登录凭据，会继续用于获取更高音质
- 🔁 **重复文件处理**：下载前检测同名文件，可选询问、覆盖、保留两份或取消
- 📱 **Android 适配**：默认写入系统 Download 目录，也可通过 SAF 选择公共下载目录；适配安全区与手势返回
- 📋 **任务管理**：按状态分类，支持批量删除、全部重试、取消、恢复与批量清理历史；任务列表分页渲染
- 🔔 **通知与退出确认**：任务结果与链接状态变化时弹出应用内通知；有进行中的任务时退出先二次确认
- ⬆️ **检查更新**：从本仓库 GitHub Releases 检查新版本并展示更新说明
- ⚙️ **个性化设置**：默认音质、自动降级、下载目录、文件命名模板、歌手分隔符、并发数、歌曲标签、LRC 保存等
- 🎨 **深色模式**：跟随系统主题
- 💾 **本地持久化**：任务、设置与登录状态保存在应用数据目录

---

## 📱 下载与安装

在 [Releases](https://github.com/YG224586/Music-DownLoads/releases) 页面下载最新的 `music-downloads-<版本>-arm64.apk`（当前 `v1.0.0`）。

- 包名 `com.musicdownloads.app`，`minSdk 24`，`targetSdk 36`，本仓库发布的安装包仅包含 `arm64-v8a`
- 安装包已使用 release keystore 签名；首次安装需在系统设置中允许「安装未知来源应用」
- 更新检查读取 `https://api.github.com/repos/YG224586/Music-DownLoads/releases/latest`

---

## 🖥️ 技术栈

| 界面                       | 下载核心                          |
| -------------------------- | --------------------------------- |
| Vue 3 (Composition API)    | Rust 共享核心（`hotdownloader-core`） |
| TypeScript / Vite          | Tokio（异步运行时）               |
| Pinia / Vue Router (Hash)  | Reqwest（HTTP 客户端）            |
| Naive UI（Material Design 3 主题） | lofty（音频标签写入）      |
| Tauri 2（Android 运行时）  | SAF 适配、Android 通知           |

---

## 📦 环境要求

| 组件        | 版本 / 说明                                                                 |
| ----------- | --------------------------------------------------------------------------- |
| Node.js     | 22.12+（含 npm）                                                            |
| JDK         | 21（`JAVA_HOME`）                                                           |
| Android SDK | `cmdline-tools`、`platform-tools`、`platforms;android-36`、`build-tools;36.0.0` |
| Android NDK | `27.2.12479018`                                                             |
| Rust        | 1.77.2+，并添加 Android 目标：`aarch64-linux-android`（另可加 `armv7-linux-androideabi`、`i686-linux-android`、`x86_64-linux-android`） |

> Windows 构建宿主使用 `x86_64-pc-windows-gnu` 工具链时无需安装 MSVC：设置 `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=rust-lld` 与 `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS=-C link-self-contained=yes` 即可用 Rust 自带的 `rust-lld` 与 MinGW 运行库完成宿主侧链接。

---

## 🚀 从源码构建 APK

```bash
git clone https://github.com/YG224586/Music-DownLoads.git
cd Music-DownLoads
git submodule update --init --recursive   # libs/um_crypto，QMC 解密模块，构建必需
npm install
```

设置构建环境（示例为 Windows PowerShell，请把 SDK / NDK / JDK 路径替换成你自己的）：

```powershell
$env:JAVA_HOME        = '<JDK 21 路径>'
$env:ANDROID_HOME     = '<Android SDK 路径>'
$env:ANDROID_SDK_ROOT = $env:ANDROID_HOME
$env:NDK_HOME         = "$env:ANDROID_HOME\ndk\27.2.12479018"
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER   = 'rust-lld'
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS = '-C link-self-contained=yes'
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:JAVA_HOME\bin;$env:PATH"
```

构建 arm64 APK：

```bash
npm run tauri android build -- --target aarch64 --apk
```

产物位于：

```
src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk
```

### 签名

Release 构建读取 `src-tauri/gen/android/keystore.properties`（该文件已被 `.gitignore` 忽略）：

```bash
keytool -genkeypair -v -keystore music-downloads.keystore \
  -alias musicdownloads -keyalg RSA -keysize 2048 -validity 10000
```

```properties
storeFile=/absolute/path/to/music-downloads.keystore
keyAlias=musicdownloads
keyPassword=<你的口令>
storePassword=<你的口令>
```

未配置签名时可先构建调试包：`npm run tauri android build -- --target aarch64 --debug --apk`

### 开发调试

```bash
npm run dev                 # 浏览器预览界面（接口需自行提供）
npm run tauri android dev   # 在连接的设备 / 模拟器上运行
```

---

## 🗂️ 项目结构

```
src/                          Vue 3 界面（Material Design 3 手机优先）
src-tauri/                    Tauri 2 应用，Android 工程位于 src-tauri/gen/android
crates/hotdownloader-core/    Rust 下载核心：任务调度、平台请求、解密与标签写入
libs/um_crypto/               QMC 解密子模块（git submodule，构建必需）
scripts/                      许可证生成与存储恢复测试脚本
```

---

## ✅ 代码检查

```bash
npm run format:ts:check
npm run format:vue:check
npm run format:css:check
npm run build                 # vue-tsc 类型检查 + vite 构建
npm run test:storage-recovery
```

---

## 📄 许可证

本项目基于 [Apache License 2.0](LICENSE) 开源。

本项目派生自 [lerdb/HotDownloader](https://github.com/lerdb/HotDownloader)，依 Apache License 2.0 保留原始版权声明、`NOTICE` 与 `THIRD_PARTY_LICENSES.txt`。

- `NOTICE`：随包分发的第三方组件名称、版本与许可证标识
- `THIRD_PARTY_LICENSES.txt`：上述组件的许可证全文

应用内「设置 → 关于」页面同样可以查看组件列表并展开许可证全文。

---

## ⚠️ 免责声明

**「音乐下载」仅用于学习和研究目的。**

用户需自行承担使用本软件所带来的法律责任。请确保你下载的音乐文件拥有合法的使用权，遵守相关音乐平台的版权规定。本项目开发者不对任何侵权行为负责。
