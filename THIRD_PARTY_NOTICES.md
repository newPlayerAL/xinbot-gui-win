# Bundled components

The release installer contains the following Java components. Their corresponding source code is
available at the linked repositories.

| Component | Version/source | License |
| --- | --- | --- |
| XinBot Core | [2.4.3 with GUI integration changes](https://github.com/newPlayerAL/xinbot/tree/xinbot-gui-win-v0.2.8-core) | GPL-3.0-or-later |
| XinMetaPlugin | [1.1.0-RELEASE](https://github.com/huangdihd/XinMetaPlugin/tree/1.1.0-RELEASE) | GPL-3.0 |
| DirectConnect | [1.0.0 source](bundled-plugins/directconnect) | GPL-3.0-or-later |
| ChatFilter | [1.0.0-RELEASE source](https://github.com/newPlayerAL/ChatFilter) | GPL-3.0 |

The installer does not bundle a Java runtime. On first launch, the application downloads a Java 21
runtime directly from Azul, Eclipse Adoptium or Microsoft and verifies the provider-published
SHA-256 checksum.

Rust and npm dependencies are listed in `src-tauri/Cargo.lock` and `package-lock.json`. Each remains
subject to its own license and copyright notice.
