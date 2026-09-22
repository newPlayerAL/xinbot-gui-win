import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppStatus,
  BotConsoleEvent,
  BotStateEvent,
  LaunchRequest,
  OfficialPluginCatalog,
  PluginConfigDocument,
  PluginConfigRequest,
  PluginDescriptor,
  RuntimeProgress,
  RuntimeSource,
} from "./types";
import { officialPluginCatalog } from "./officialPlugins";

export const inTauri = () => Boolean(window.__TAURI_INTERNALS__);

const previewStatus: AppStatus = {
  runtimeState: "missing",
  runtimeVersion: null,
  runtimePath: null,
  xinbotReady: true,
  xinbotPath: "resources/xinbot.jar",
  directConnectReady: true,
  dataDir: "%LOCALAPPDATA%\\xinbot-gui-win",
  running: false,
  runningInstanceIds: [],
};

export async function getStatus(): Promise<AppStatus> {
  return inTauri() ? invoke<AppStatus>("get_app_status") : previewStatus;
}

export async function getRuntimeSources(): Promise<RuntimeSource[]> {
  if (!inTauri()) {
    return [
      {
        id: "azul",
        name: "Azul Zulu 21",
        detail: "轻量 JRE · 约 49 MB · CDN 推荐",
        recommended: true,
      },
      {
        id: "temurin",
        name: "Eclipse Temurin 21",
        detail: "轻量 JRE · GitHub 备用",
        recommended: false,
      },
      {
        id: "microsoft",
        name: "Microsoft OpenJDK 21",
        detail: "完整 JDK · 约 192 MB · 最后备用",
        recommended: false,
      },
    ];
  }
  return invoke<RuntimeSource[]>("get_runtime_sources");
}

export async function listAvailablePlugins(): Promise<PluginDescriptor[]> {
  if (!inTauri()) {
    return [
      { id: "xinmeta", name: "XinMetaPlugin", version: "1.1.0-RELEASE", pluginType: "META_PLUGIN", description: "2b2t.xin 官方适配", source: "bundled", resource: "xinmetaplugin.jar", loginMode: "plugin", hostPatterns: ["2b2t.xin"], recommended: true, dependencies: [], configFiles: [] },
      { id: "directconnect", name: "DirectConnect", version: "1.0.0-RELEASE", pluginType: "META_PLUGIN", description: "通用服务器直连适配", source: "bundled", resource: "directconnect.jar", loginMode: "template", hostPatterns: [], recommended: false, dependencies: [], configFiles: [] },
      { id: "chatfilter", name: "ChatFilter", version: "1.0.0-RELEASE", pluginType: "PLUGIN", description: "聊天消息过滤插件", source: "bundled", resource: "chatfilter.jar", loginMode: "none", hostPatterns: [], recommended: false, dependencies: [], configFiles: [] },
    ];
  }
  return invoke<PluginDescriptor[]>("list_available_plugins");
}

export async function getOfficialPluginCatalog(): Promise<OfficialPluginCatalog> {
  return inTauri()
    ? invoke<OfficialPluginCatalog>("list_official_plugins")
    : officialPluginCatalog;
}

export async function openPluginLink(url: string): Promise<void> {
  if (inTauri()) {
    await invoke("open_plugin_link", { url });
    return;
  }
  window.open(url, "_blank", "noopener,noreferrer");
}

export async function importPlugin(path: string): Promise<PluginDescriptor> {
  return invoke<PluginDescriptor>("import_plugin", { path });
}

export async function readPluginConfig(request: PluginConfigRequest): Promise<PluginConfigDocument> {
  if (!inTauri()) {
    return {
      pluginId: request.pluginId,
      path: request.path,
      format: "json",
      exists: false,
      content: "",
    };
  }
  return invoke<PluginConfigDocument>("read_plugin_config", { request });
}

export async function writePluginConfig(
  request: PluginConfigRequest,
  content: string,
): Promise<PluginConfigDocument> {
  if (!inTauri()) {
    return {
      pluginId: request.pluginId,
      path: request.path,
      format: "json",
      exists: true,
      content,
    };
  }
  return invoke<PluginConfigDocument>("write_plugin_config", { request, content });
}

export async function installRuntime(source: string): Promise<AppStatus> {
  if (!inTauri()) {
    throw new Error("浏览器预览模式不会实际下载 JRE");
  }
  await invoke("install_runtime", { source });
  return getStatus();
}

export async function launchBot(request: LaunchRequest): Promise<void> {
  if (!inTauri()) {
    throw new Error("浏览器预览模式不会启动 xinbot");
  }
  await invoke("launch_bot", { request });
}

export async function sendBotCommand(profileId: string, command: string): Promise<void> {
  await invoke("send_bot_command", { profileId, command });
}

export async function stopBot(profileId: string): Promise<void> {
  await invoke("stop_bot", { profileId });
}

export async function listenRuntimeProgress(
  handler: (payload: RuntimeProgress) => void,
): Promise<UnlistenFn> {
  return inTauri()
    ? listen<RuntimeProgress>("runtime-download-progress", ({ payload }) => handler(payload))
    : () => {};
}

export async function listenBotConsole(
  handler: (payload: BotConsoleEvent) => void,
): Promise<UnlistenFn> {
  return inTauri()
    ? listen<BotConsoleEvent>("bot-console", ({ payload }) => handler(payload))
    : () => {};
}

export async function listenBotState(
  handler: (payload: BotStateEvent) => void,
): Promise<UnlistenFn> {
  return inTauri()
    ? listen<BotStateEvent>("bot-state", ({ payload }) => handler(payload))
    : () => {};
}
