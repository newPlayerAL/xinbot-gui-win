# xinbot-gui-win

Windows-only XinBot control center built with Tauri 2, Rust and Svelte. It intentionally does not
bundle a JRE: on first launch it downloads a private Java 21 runtime into the application's local
data directory, verifies the official SHA-256 checksum, and keeps that runtime isolated from the
system `PATH`.

This is a new implementation. It neither reads nor writes the legacy `%APPDATA%/xinbot-gui` data.

## Prototype scope

- Modern frameless Windows UI with persistent server profiles and dark/light themes.
- Multiple server profiles can run at the same time; each XinBot process has its own working
  directory, configuration, plugin set, console stream and command input.
- A server-scoped workspace whose configuration can collapse to a compact summary, leaving most
  of the window to the live console.
- Per-server plugin selection: exactly one Meta plugin plus any number of ordinary plugins.
- `2b2t.xin` and the official XinMetaPlugin are the default; DirectConnect remains available for
  generic servers, and additional XinBot plugin JARs can be imported into the local plugin library.
- Private Java 21 detection and installation.
- Azul Zulu JRE as the small CDN-backed default, with Eclipse Temurin and Microsoft OpenJDK as
  automatic fallbacks.
- Resumable runtime downloads, SHA-256 verification and archive path validation.
- XinBot launch through redirected stdin/stdout/stderr, command input and graceful `stop`.
- Existing `xinbot.jar` and `directconnect.jar` are bundled as release resources without changing
  the Java GUI project.

## Development

```bash
npm install
npm run dev          # browser UI preview (native actions are disabled)
npm run tauri dev    # full desktop app; requires Tauri's platform prerequisites
```

Build the NSIS installer on Windows:

```powershell
./scripts/prepare-resources.ps1
npm ci
npm run tauri build
```

`prepare-resources.ps1` copies the current XinBot artifacts and removes native libraries for
Linux, macOS and non-x64 Windows targets from the bundled JAR. It does not modify the original
cross-platform `xinbot.jar`. The resulting installer therefore targets Windows x64 only.

The release profile enables LTO, size optimization, panic aborts and symbol stripping. The NSIS
installer embeds only the small WebView2 bootstrapper; the Java runtime is always downloaded after
the GUI starts.

## Data layout

```text
%LOCALAPPDATA%/io.github.newplayeral.xinbot-gui-win/
  runtime/java-21/           private Java runtime
  downloads/                resumable `.part` archives
  plugin-library/           user-imported XinBot plugin JARs
  instances/<stable-id>/    generated config, plugins and logs
```

License: GPL-3.0-or-later.
