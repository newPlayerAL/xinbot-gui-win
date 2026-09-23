# XinBot Windows

这是一个仅面向 Windows 的 [XinBot](https://github.com/huangdihd/xinbot) 控制中心，使用
Tauri 2、Rust 和 Svelte 开发。

[English](README.md) · [下载最新版本](https://github.com/newPlayerAL/xinbot-gui-win/releases/latest)

## 功能

- 同时运行多个服务器配置。每个配置都有独立的 XinBot 进程、工作目录、配置、插件、控制台
  和命令输入。
- Bot 运行时仍可编辑或启动其他服务器配置，一个实例不会阻断其他实例。
- Meta 插件与普通插件分开管理；支持导入本地插件 JAR 和浏览官方插件目录。
- 插件按服务器分别配置。对于已知插件，可以直接编辑原生配置文件；也可以短时加载插件，
  让插件生成默认配置后退出，全程不会连接 Minecraft 服务器。
- 首次运行时下载独立 Java 21 运行环境，校验 SHA-256 后存入应用数据目录，不修改系统
  `PATH`。默认使用 Azul Zulu，并自动回退到 Eclipse Temurin 或 Microsoft OpenJDK。
- 内置针对 Windows x86-64 精简的 XinBot Core，同时保留 Core 的通用跨平台构建方式。

## 安装

1. 打开 [最新 GitHub Release](https://github.com/newPlayerAL/xinbot-gui-win/releases/latest)。
2. 下载 `XinBot_0.2.8_x64-setup.exe`。
3. 按 `SHA256SUMS.txt` 校验 SHA-256，然后运行安装程序。

运行要求：

- Windows 10 或 Windows 11，x86-64。
- 首次启动需要联网下载 Java 21。
- 安装程序目前没有代码签名，Windows SmartScreen 可能显示“未知发布者”。继续前请先校验
  SHA-256。

升级不会删除以下目录中的应用数据：

```text
%LOCALAPPDATA%/io.github.newplayeral.xinbot-gui-win/
```

服务器配置可能包含二级登录密码。该密码只保存在本机 WebView 配置中，但没有加密；请勿在
这里使用重要密码。

## 数据目录

```text
%LOCALAPPDATA%/io.github.newplayeral.xinbot-gui-win/
  runtime/java-21/           独立 Java 运行环境
  downloads/                可续传的 `.part` 下载文件
  plugin-library/           用户导入的 XinBot 插件 JAR
  instances/<stable-id>/    每个服务器的配置、插件和日志
```

本程序使用独立的数据结构，不会导入旧 Java GUI 的配置。

## 从源码构建

仅预览前端：

```bash
npm ci
npm run dev
```

完整 Windows 构建还需要 Rust MSVC target、Tauri 2 Windows 构建依赖、Java 17+ 和 Maven。
请先按照英文 README 中的目录结构构建 XinBot Core、DirectConnect 和 ChatFilter，然后执行：

```powershell
./scripts/prepare-resources.ps1
npm ci
npm run tauri -- build --target x86_64-pc-windows-msvc
```

## 源码与许可证

本项目使用 `GPL-3.0-or-later`，详见 [LICENSE](LICENSE)。安装包内 Core 和插件的源码及
许可证链接见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。

XinBot 与 Minecraft 是彼此独立的项目。本程序与 Mojang Studios、Microsoft 没有关联，
也没有得到其认可。
