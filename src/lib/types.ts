export type RuntimeState = "ready" | "missing" | "installing" | "broken";

export interface AppStatus {
  runtimeState: RuntimeState;
  runtimeVersion: string | null;
  runtimePath: string | null;
  xinbotReady: boolean;
  xinbotPath: string | null;
  directConnectReady: boolean;
  dataDir: string;
  running: boolean;
  runningInstanceIds: string[];
}

export interface RuntimeSource {
  id: string;
  name: string;
  detail: string;
  recommended: boolean;
}

export interface RuntimeProgress {
  source: string;
  phase: "resolving" | "downloading" | "verifying" | "extracting" | "ready";
  downloaded: number;
  total: number | null;
  message: string;
}

export interface LaunchRequest {
  profileId: string;
  serverName: string;
  host: string;
  port: number | null;
  username: string;
  password: string;
  onlineMode: boolean;
  loginTemplate: string;
  metaPluginId: string;
  enabledPluginIds: string[];
}

export interface PluginDescriptor {
  id: string;
  name: string;
  version: string;
  pluginType: "PLUGIN" | "META_PLUGIN";
  description: string;
  source: "bundled" | "imported";
  resource: string;
  loginMode: "plugin" | "template" | "none";
  hostPatterns: string[];
  recommended: boolean;
  dependencies: string[];
  configFiles: PluginConfigFile[];
}

export interface PluginConfigFile {
  path: string;
  format: string;
  label: string;
}

export interface PluginConfigRequest {
  profileId: string;
  host: string;
  username: string;
  pluginId: string;
  path: string;
}

export interface PluginConfigDocument {
  pluginId: string;
  path: string;
  format: string;
  exists: boolean;
  content: string;
}

export interface OfficialPluginEntry {
  id: string;
  name: string;
  pluginType: "PLUGIN" | "META_PLUGIN";
  supportedServers: string[];
  maintainer: string;
  repositoryUrl: string;
  description: string;
}

export interface OfficialPluginCatalog {
  sourceUrl: string;
  updatedAt: string;
  entries: OfficialPluginEntry[];
}

export interface BotConsoleEvent {
  line: string;
  stream: "stdout" | "stderr" | "launcher";
  profileId: string;
}

export interface BotStateEvent {
  running: boolean;
  exitCode: number | null;
  message: string;
  profileId: string;
}
