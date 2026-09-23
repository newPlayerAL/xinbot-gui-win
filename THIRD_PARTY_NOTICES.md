# Bundled components

The release installer contains the following Java components. Their corresponding source code is
available at the linked repositories.

| Component | Version/source | License |
| --- | --- | --- |
| XinBot Core | [2.4.3 with GUI integration changes](https://github.com/newPlayerAL/xinbot/tree/xinbot-gui-win-v0.2.8-core) | GPL-3.0-or-later |
| XinMetaPlugin | [1.1.0-RELEASE](https://github.com/huangdihd/XinMetaPlugin/tree/1.1.0-RELEASE) | GPL-3.0 |
| DirectConnect | [1.0.0 source](bundled-plugins/directconnect) | GPL-3.0-or-later |
| ChatFilter | [1.0.0-RELEASE source](https://github.com/newPlayerAL/ChatFilter) | GPL-3.0 |
| BackToTheBase | [1.8.0-RELEASE](https://github.com/huangdihd/BackToTheBase/tree/1.8.0-RELEASE) | GPL-3.0 |
| MovementSync | [1.6.0-RELEASE](https://github.com/huangdihd/MovementSync/tree/1.6.0-RELEASE) | GPL-3.0-or-later |
| JOML | [1.10.5](https://github.com/JOML-CI/JOML/tree/1.10.5) | MIT |

The bundled `movementsync.jar` is assembled from the upstream original (unshaded) MovementSync
artifact plus JOML 1.10.5. XinBot Core classes already supplied by this application are not
duplicated. The reproducible assembly steps and pinned input checksums are in
`scripts/prepare-resources.ps1`.

### JOML MIT license

Copyright (c) 2015-2022 JOML

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction,
including without limitation the rights to use, copy, modify, merge, publish, distribute,
sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial
portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES
OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

The installer does not bundle a Java runtime. On first launch, the application downloads a Java 21
runtime directly from Azul, Eclipse Adoptium or Microsoft and verifies the provider-published
SHA-256 checksum.

Rust and npm dependencies are listed in `src-tauri/Cargo.lock` and `package-lock.json`. Each remains
subject to its own license and copyright notice.
