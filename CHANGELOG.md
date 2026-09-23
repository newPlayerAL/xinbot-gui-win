# Changelog

## 0.2.8 - 2026-09-23

First public Windows release.

### Highlights

- Independent concurrent XinBot instances for multiple server profiles.
- Server editing and controls remain available while other instances are running.
- Dedicated plugin manager with Meta/ordinary plugin separation, local imports and official catalog.
- Per-server native plugin configuration editing and short-lived default-config generation.
- Unsaved plugin configuration is protected when switching, adding or deleting server profiles.
- Irrelevant WebView context menus are disabled and plugin links use validated native URL opening.
- XinBot Core 2.4.3 integration with a Windows x86-64 slim distribution.
- Private Java 21 download with checksum verification and provider fallback.
- 20.36 MiB NSIS installer; existing application data is preserved during upgrades.

### Verification

- XinBot Core: 89 tests passed.
- Windows x86-64 release and NSIS builds passed.
- XinMetaPlugin and ChatFilter initialization passed.
- Real server connection and multi-instance operation were tested on Windows 10 x64.
