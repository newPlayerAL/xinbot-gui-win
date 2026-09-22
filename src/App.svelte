<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    getRuntimeSources,
    getOfficialPluginCatalog,
    getStatus,
    importPlugin,
    inTauri,
    initializePluginConfigs,
    installRuntime,
    launchBot,
    listAvailablePlugins,
    listenBotConsole,
    listenBotState,
    listenRuntimeProgress,
    readPluginConfig,
    sendBotCommand,
    stopBot,
    openPluginLink,
    writePluginConfig,
  } from "./lib/api";
  import { officialPluginCatalog as previewOfficialPluginCatalog } from "./lib/officialPlugins";
  import type {
    AppStatus,
    BotConsoleEvent,
    LaunchRequest,
    OfficialPluginCatalog,
    PluginConfigFile,
    PluginDescriptor,
    RuntimeProgress,
    RuntimeSource,
  } from "./lib/types";

  type ServerProfile = LaunchRequest & { id: string };
  const STORAGE_KEY = "xinbot-gui-win.servers.v2";
  const THEME_KEY = "xinbot-gui-win.theme";

  function createId(): string {
    return globalThis.crypto?.randomUUID?.() ?? `server-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  }

  function createServer(index: number): ServerProfile {
    return {
      id: createId(),
      profileId: "",
      serverName: index === 1 ? "2b2t.xin" : `2b2t.xin ${index}`,
      host: "2b2t.xin",
      port: null,
      username: "",
      password: "",
      onlineMode: false,
      loginTemplate: "/login {password}",
      metaPluginId: "xinmeta",
      enabledPluginIds: [],
    };
  }

  function requestFrom(profile: ServerProfile): LaunchRequest {
    return {
      profileId: profile.id,
      serverName: profile.serverName,
      host: profile.host,
      port: profile.port,
      username: profile.username,
      password: profile.password,
      onlineMode: profile.onlineMode,
      loginTemplate: profile.loginTemplate,
      metaPluginId: profile.metaPluginId,
      enabledPluginIds: [...profile.enabledPluginIds],
    };
  }

  let servers: ServerProfile[] = [createServer(1)];
  let theme: "dark" | "light" = "dark";
  let activeView: "servers" | "plugins" = "servers";
  let selectedServerId = servers[0].id;
  let form: LaunchRequest = requestFrom(servers[0]);
  let savedSnapshot = JSON.stringify(form);
  let saveNotice = "";
  let saveTimer: number | undefined;
  let status: AppStatus | null = null;
  let sources: RuntimeSource[] = [];
  let selectedSource = "azul";
  let progress: RuntimeProgress | null = null;
  let installing = false;
  // Startup is tracked per profile. A slow connection for one server must not
  // disable editing or starting a different server profile.
  let launchingProfileIds = new Set<string>();
  let importingPlugin = false;
  let plugins: PluginDescriptor[] = [];
  let officialCatalog: OfficialPluginCatalog = previewOfficialPluginCatalog;
  let officialPluginQuery = "";
  let officialPluginTypeFilter: "all" | "PLUGIN" | "META_PLUGIN" = "all";
  let pluginConfigPlugin: PluginDescriptor | null = null;
  let pluginConfigPath = "";
  let pluginConfigContent = "";
  let pluginConfigExists = false;
  let pluginConfigDirty = false;
  let loadingPluginConfig = false;
  let savingPluginConfig = false;
  let initializingPluginProfileIds = new Set<string>();
  let configTab: "connection" | "plugins" = "connection";
  let configurationCollapsed = false;
  let errorMessage = "";
  const initialConsoleLine = (profileId: string): BotConsoleEvent => ({
    line: "控制台已就绪。选择左侧服务器，检查配置后即可启动。",
    stream: "launcher",
    profileId,
  });
  let consoleBuffers: Record<string, BotConsoleEvent[]> = {};
  let consoleLines: BotConsoleEvent[] = [initialConsoleLine(servers[0].id)];
  let command = "";
  let consoleElement: HTMLDivElement;

  $: runtimeReady = status?.runtimeState === "ready";
  $: selectedRunning = Boolean(status?.runningInstanceIds?.includes(selectedServerId));
  $: selectedLaunching = launchingProfileIds.has(selectedServerId);
  $: selectedInitializing = initializingPluginProfileIds.has(selectedServerId);
  $: selectedBusy = selectedRunning || selectedLaunching || selectedInitializing;
  $: metaPlugins = plugins.filter((plugin) => plugin.pluginType === "META_PLUGIN");
  $: ordinaryPlugins = plugins.filter((plugin) => plugin.pluginType === "PLUGIN");
  $: pluginConfigSpec = pluginConfigPlugin?.configFiles.find((file) => file.path === pluginConfigPath) ?? null;
  $: filteredOfficialPlugins = officialCatalog.entries.filter((plugin) => {
    const query = officialPluginQuery.trim().toLowerCase();
    const matchesType = officialPluginTypeFilter === "all" || plugin.pluginType === officialPluginTypeFilter;
    const searchable = [plugin.name, plugin.maintainer, plugin.description, ...plugin.supportedServers].join(" ").toLowerCase();
    return matchesType && (!query || searchable.includes(query));
  });
  $: officialMetaPlugins = filteredOfficialPlugins.filter((plugin) => plugin.pluginType === "META_PLUGIN");
  $: officialOrdinaryPlugins = filteredOfficialPlugins.filter((plugin) => plugin.pluginType === "PLUGIN");
  $: selectedMeta = metaPlugins.find((plugin) => plugin.id === form.metaPluginId);
  $: enabledPluginCount = form.enabledPluginIds.length;
  $: isDirty = JSON.stringify(form) !== savedSnapshot;
  $: canLaunch = Boolean(
    runtimeReady && status?.xinbotReady && form.host.trim() && form.username.trim()
      && metaPlugins.some((plugin) => plugin.id === form.metaPluginId) && !selectedBusy,
  );
  $: progressPercent =
    progress?.total && progress.total > 0
      ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100))
      : null;

  function icon(name: string): string {
    const icons: Record<string, string> = {
      server: '<rect x="3" y="4" width="18" height="6" rx="2"/><rect x="3" y="14" width="18" height="6" rx="2"/><path d="M7 7h.01M7 17h.01"/>',
      terminal: '<path d="m4 17 6-5-6-5M12 19h8"/>',
      download: '<path d="M12 3v12m0 0 4-4m-4 4-4-4M5 21h14"/>',
      play: '<path d="m8 5 11 7-11 7Z"/>',
      stop: '<rect x="6" y="6" width="12" height="12" rx="2"/>',
      send: '<path d="m22 2-7 20-4-9-9-4Z"/><path d="M22 2 11 13"/>',
      check: '<path d="m5 12 4 4L19 6"/>',
      plus: '<path d="M12 5v14M5 12h14"/>',
      save: '<path d="M5 3h12l2 2v16H5z"/><path d="M8 3v6h8V3M8 21v-7h8v7"/>',
      trash: '<path d="M4 7h16M9 7V4h6v3M7 7l1 14h8l1-14M10 11v6M14 11v6"/>',
      layers: '<path d="m12 3 9 5-9 5-9-5 9-5Z"/><path d="m3 12 9 5 9-5M3 16l9 5 9-5"/>',
      clear: '<path d="M3 6h18M8 6V4h8v2M6 6l1 15h10l1-15"/>',
      sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.93 4.93l1.42 1.42M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.42-1.42M17.66 6.34l1.41-1.41"/>',
      moon: '<path d="M21 12.8A9 9 0 1 1 11.2 3 7 7 0 0 0 21 12.8Z"/>',
    };
    return icons[name] ?? "";
  }

  function loadProfiles() {
    try {
      const parsed = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]") as Partial<ServerProfile>[];
      const valid = parsed
        .filter((item) => typeof item.id === "string" && typeof item.serverName === "string")
        .map((item, index) => ({
          ...createServer(index + 1),
          ...item,
          id: item.id as string,
          serverName: item.serverName as string,
          port: typeof item.port === "number" ? item.port : null,
          onlineMode: Boolean(item.onlineMode),
          metaPluginId: typeof item.metaPluginId === "string"
            ? item.metaPluginId
            : (item.host === "2b2t.xin" ? "xinmeta" : "directconnect"),
          enabledPluginIds: Array.isArray(item.enabledPluginIds)
            ? item.enabledPluginIds.filter((id): id is string => typeof id === "string")
            : [],
        }));
      if (valid.length > 0) servers = valid;
    } catch {
      // A damaged preference should not prevent the app from opening.
    }
    selectProfile(servers[0].id, false);
  }

  function loadTheme() {
    theme = localStorage.getItem(THEME_KEY) === "light" ? "light" : "dark";
  }

  function toggleTheme() {
    theme = theme === "dark" ? "light" : "dark";
    localStorage.setItem(THEME_KEY, theme);
  }

  function persistProfiles() {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(servers));
  }

  function showSaved() {
    saveNotice = "已保存";
    if (saveTimer) window.clearTimeout(saveTimer);
    saveTimer = window.setTimeout(() => (saveNotice = ""), 1800);
  }

  function saveCurrent(showConfirmation = true) {
    const index = servers.findIndex((server) => server.id === selectedServerId);
    if (index < 0) return;
    const normalized: LaunchRequest = {
      ...form,
      serverName: form.serverName.trim() || `服务器 ${index + 1}`,
      host: form.host.trim(),
      username: form.username.trim(),
      loginTemplate: form.loginTemplate.trim() || "/login {password}",
      metaPluginId: form.metaPluginId || "directconnect",
      enabledPluginIds: [...form.enabledPluginIds],
    };
    form = normalized;
    servers = servers.map((server, current) => current === index ? { ...server, ...normalized } : server);
    savedSnapshot = JSON.stringify(form);
    persistProfiles();
    if (showConfirmation) showSaved();
  }

  function selectProfile(id: string, saveBeforeSwitch = true) {
    activeView = "servers";
    if (id === selectedServerId && saveBeforeSwitch) return;
    if (saveBeforeSwitch && isDirty) saveCurrent(false);
    const profile = servers.find((server) => server.id === id);
    if (!profile) return;
    selectedServerId = id;
    form = requestFrom(profile);
    savedSnapshot = JSON.stringify(form);
    errorMessage = "";
    pluginConfigPlugin = null;
    pluginConfigPath = "";
    pluginConfigContent = "";
    pluginConfigExists = false;
    pluginConfigDirty = false;
    configTab = "connection";
    configurationCollapsed = false;
    consoleLines = consoleBuffers[id] ?? [initialConsoleLine(id)];
  }

  function addServer() {
    if (isDirty) saveCurrent(false);
    const profile = createServer(servers.length + 1);
    servers = [...servers, profile];
    persistProfiles();
    selectProfile(profile.id, false);
    configurationCollapsed = false;
  }

  function deleteServer() {
    if (selectedBusy || servers.length === 1) return;
    if (!window.confirm(`删除“${form.serverName || "未命名服务器"}”及其界面配置？`)) return;
    const index = servers.findIndex((server) => server.id === selectedServerId);
    const nextServers = servers.filter((server) => server.id !== selectedServerId);
    servers = nextServers;
    persistProfiles();
    selectProfile(nextServers[Math.min(Math.max(index, 0), nextServers.length - 1)].id, false);
  }

  function saveAndCollapse() {
    saveCurrent();
    configurationCollapsed = true;
  }

  function selectMetaPlugin(plugin: PluginDescriptor) {
    const next: LaunchRequest = { ...form, metaPluginId: plugin.id };
    if (plugin.id === "xinmeta") {
      next.serverName = form.serverName.trim() || "2b2t.xin";
      next.host = "2b2t.xin";
      next.port = null;
    }
    form = next;
  }

  function togglePlugin(id: string) {
    const enabled = form.enabledPluginIds.includes(id);
    form = {
      ...form,
      enabledPluginIds: enabled
        ? form.enabledPluginIds.filter((pluginId) => pluginId !== id)
        : [...form.enabledPluginIds, id],
    };
  }

  async function choosePluginJar() {
    if (!inTauri()) {
      errorMessage = "浏览器预览模式不能导入本地插件";
      return;
    }
    importingPlugin = true;
    errorMessage = "";
    try {
      const path = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "XinBot 插件", extensions: ["jar"] }],
      });
      if (!path) return;
      await importPlugin(path);
      plugins = await listAvailablePlugins();
    } catch (error) {
      errorMessage = String(error);
    } finally {
      importingPlugin = false;
    }
  }

  async function openPluginConfig(plugin: PluginDescriptor, spec?: PluginConfigFile) {
    const nextSpec = spec ?? plugin.configFiles[0];
    if (!nextSpec) return;
    if (pluginConfigDirty && !window.confirm("当前插件配置还有未保存的修改，确定切换吗？")) return;
    pluginConfigPlugin = plugin;
    pluginConfigPath = nextSpec.path;
    pluginConfigContent = "";
    pluginConfigExists = false;
    pluginConfigDirty = false;
    loadingPluginConfig = true;
    errorMessage = "";
    try {
      const document = await readPluginConfig({
        profileId: form.profileId,
        host: form.host,
        username: form.username,
        pluginId: plugin.id,
        path: nextSpec.path,
      });
      pluginConfigExists = document.exists;
      pluginConfigContent = document.content;
    } catch (error) {
      errorMessage = String(error);
      pluginConfigContent = "";
    } finally {
      loadingPluginConfig = false;
    }
  }

  async function savePluginConfig() {
    if (!pluginConfigPlugin || !pluginConfigSpec || !pluginConfigExists || loadingPluginConfig) return;
    if (pluginConfigSpec.format === "json") {
      try {
        JSON.parse(pluginConfigContent);
      } catch {
        errorMessage = "插件配置不是有效 JSON，请修正后再保存。";
        return;
      }
    }
    savingPluginConfig = true;
    errorMessage = "";
    try {
      const document = await writePluginConfig({
        profileId: form.profileId,
        host: form.host,
        username: form.username,
        pluginId: pluginConfigPlugin.id,
        path: pluginConfigSpec.path,
      }, pluginConfigContent);
      pluginConfigExists = document.exists;
      pluginConfigContent = document.content;
      pluginConfigDirty = false;
      showSaved();
    } catch (error) {
      errorMessage = String(error);
    } finally {
      savingPluginConfig = false;
    }
  }

  async function generatePluginConfig() {
    if (!pluginConfigPlugin || !pluginConfigSpec || selectedBusy) return;
    const profileId = selectedServerId;
    const plugin = pluginConfigPlugin;
    const spec = pluginConfigSpec;
    const request: LaunchRequest = {
      ...form,
      metaPluginId: plugin.pluginType === "META_PLUGIN" ? plugin.id : form.metaPluginId,
      enabledPluginIds: plugin.pluginType === "PLUGIN" && !form.enabledPluginIds.includes(plugin.id)
        ? [...form.enabledPluginIds, plugin.id]
        : [...form.enabledPluginIds],
    };
    initializingPluginProfileIds = new Set([...initializingPluginProfileIds, profileId]);
    errorMessage = "";
    try {
      await initializePluginConfigs(request);
      if (selectedServerId !== profileId) return;
      await openPluginConfig(plugin, spec);
      if (!pluginConfigExists) {
        errorMessage = `${plugin.name} 已完成短时加载，但没有生成 ${spec.path}；该插件可能只会在连接后生成配置。`;
      }
    } catch (error) {
      errorMessage = String(error);
    } finally {
      const next = new Set(initializingPluginProfileIds);
      next.delete(profileId);
      initializingPluginProfileIds = next;
    }
  }

  function openOfficialLink(event: MouseEvent, url: string) {
    event.stopPropagation();
    void openPluginLink(url).catch((error) => (errorMessage = String(error)));
  }

  function releasePageUrl(repositoryUrl: string): string {
    return `${repositoryUrl.replace(/\/+$/, "")}/releases`;
  }

  function isOfficialPluginInstalled(name: string, available: PluginDescriptor[]): boolean {
    const normalizedName = name.trim().toLowerCase();
    return available.some((plugin) => plugin.name.trim().toLowerCase() === normalizedName
      || (normalizedName === "xinmetaplugin" && plugin.id === "xinmeta"));
  }

  function addConsole(line: string, stream: BotConsoleEvent["stream"] = "launcher", profileId = selectedServerId) {
    const clean = line.replace(/\x1b\[[0-9;]*[A-Za-z]/g, "");
    const next = [...(consoleBuffers[profileId] ?? [initialConsoleLine(profileId)]), { line: clean, stream, profileId }].slice(-2000);
    consoleBuffers = { ...consoleBuffers, [profileId]: next };
    if (profileId === selectedServerId) consoleLines = next;
    requestAnimationFrame(() => {
      if (consoleElement) consoleElement.scrollTop = consoleElement.scrollHeight;
    });
  }

  function clearConsole() {
    const next = [initialConsoleLine(selectedServerId), { line: "控制台内容已清除。", stream: "launcher" as const, profileId: selectedServerId }];
    consoleBuffers = { ...consoleBuffers, [selectedServerId]: next };
    consoleLines = next;
  }

  async function refreshStatus() {
    try {
      status = await getStatus();
    } catch (error) {
      errorMessage = String(error);
    }
  }

  async function startRuntimeInstall() {
    installing = true;
    errorMessage = "";
    try {
      status = await installRuntime(selectedSource);
      addConsole(`运行环境安装完成：${status.runtimeVersion ?? "Java 21"}`);
    } catch (error) {
      errorMessage = String(error);
      addConsole(`运行环境安装失败：${errorMessage}`, "stderr");
    } finally {
      installing = false;
    }
  }

  async function startBot() {
    saveCurrent(false);
    const profileId = selectedServerId;
    const request = { ...form };
    launchingProfileIds = new Set([...launchingProfileIds, profileId]);
    configurationCollapsed = true;
    errorMessage = "";
    try {
      await launchBot(request);
      await refreshStatus();
    } catch (error) {
      errorMessage = String(error);
      addConsole(`启动失败：${errorMessage}`, "stderr", profileId);
    } finally {
      const next = new Set(launchingProfileIds);
      next.delete(profileId);
      launchingProfileIds = next;
    }
  }

  async function stopCurrent() {
    try {
      await stopBot(selectedServerId);
    } catch (error) {
      errorMessage = String(error);
    }
  }

  async function submitCommand() {
    const value = command.trim();
    if (!value || !selectedRunning) return;
    const profileId = selectedServerId;
    command = "";
    addConsole(`› ${value}`, "launcher", profileId);
    try {
      await sendBotCommand(profileId, value);
    } catch (error) {
      addConsole(`命令发送失败：${String(error)}`, "stderr", profileId);
    }
  }

  async function windowAction(action: "minimize" | "maximize" | "close") {
    if (!inTauri()) return;
    const appWindow = getCurrentWindow();
    if (action === "minimize") await appWindow.minimize();
    if (action === "maximize") await appWindow.toggleMaximize();
    if (action === "close") await appWindow.close();
  }

  onMount(() => {
    loadTheme();
    loadProfiles();
    const unlisteners: Array<() => void> = [];
    void Promise.all([getStatus(), getRuntimeSources(), listAvailablePlugins(), getOfficialPluginCatalog()]).then(([nextStatus, nextSources, nextPlugins, nextOfficialCatalog]) => {
      status = nextStatus;
      sources = nextSources;
      plugins = nextPlugins;
      officialCatalog = nextOfficialCatalog;
      selectedSource = nextSources.find((source) => source.recommended)?.id ?? nextSources[0]?.id ?? "azul";
    }).catch((error) => (errorMessage = String(error)));

    void listenRuntimeProgress((next) => (progress = next)).then((fn) => unlisteners.push(fn));
    void listenBotConsole((event) => addConsole(event.line, event.stream, event.profileId)).then((fn) => unlisteners.push(fn));
    void listenBotState((event) => {
      void refreshStatus();
      addConsole(event.message, event.exitCode && event.exitCode !== 0 ? "stderr" : "launcher", event.profileId);
    }).then((fn) => unlisteners.push(fn));

    return () => {
      if (saveTimer) window.clearTimeout(saveTimer);
      unlisteners.forEach((fn) => fn());
    };
  });
</script>

<svelte:head><title>XinBot · Windows 控制台</title></svelte:head>

<div class:light={theme === "light"} class="app-shell">
  <header class="titlebar" data-tauri-drag-region>
    <div class="titlebar-brand" data-tauri-drag-region><div class="mini-mark">X</div><span>XinBot</span><span class="preview-label">WINDOWS</span></div>
    <div class="titlebar-actions">
      <button class="theme-toggle" onclick={toggleTheme} title={theme === "dark" ? "切换到浅色模式" : "切换到深色模式"} aria-label={theme === "dark" ? "切换到浅色模式" : "切换到深色模式"}>
        <svg viewBox="0 0 24 24">{@html icon(theme === "dark" ? "sun" : "moon")}</svg><span>{theme === "dark" ? "浅色" : "深色"}</span>
      </button>
      <div class="window-controls">
        <button aria-label="最小化" onclick={() => windowAction("minimize")}>—</button>
        <button aria-label="最大化" onclick={() => windowAction("maximize")}>□</button>
        <button class="close" aria-label="关闭" onclick={() => windowAction("close")}>×</button>
      </div>
    </div>
  </header>

  <aside class="sidebar">
    <div class="brand-block"><div class="brand-mark">Ξ</div><div><strong>XinBot</strong><small>Windows 控制中心</small></div></div>
    <div class="sidebar-heading">
      <div><span>服务器</span><small>{servers.length} 个配置</small></div>
      <button class="add-icon-button" onclick={addServer} title="新增服务器" aria-label="新增服务器"><svg viewBox="0 0 24 24">{@html icon("plus")}</svg></button>
    </div>
    <button class="add-server" onclick={addServer}><svg viewBox="0 0 24 24">{@html icon("plus")}</svg>新增服务器</button>
    <div class="server-list" aria-label="服务器列表">
      {#each servers as server}
        <button class:selected={activeView === "servers" && server.id === selectedServerId} class:active={status?.runningInstanceIds?.includes(server.id)} class:pending={launchingProfileIds.has(server.id)} class="server-entry" onclick={() => selectProfile(server.id)}>
          <span class="server-dot"></span>
          <span class="server-copy"><strong>{server.serverName || "未命名服务器"}</strong><small>{server.host || "尚未填写服务器地址"}</small></span>
          {#if server.id === selectedServerId}<span class="selected-label">当前</span>{/if}
        </button>
      {/each}
    </div>
    <section class:ready={runtimeReady} class="sidebar-runtime">
      <div class="sidebar-runtime-heading">
        <span class="runtime-dot"></span>
        <div><small>全局运行环境</small><strong>{runtimeReady ? "Java 21 已就绪" : "需要安装 Java 21"}</strong></div>
      </div>
      {#if runtimeReady}
        <p>{status?.runtimeVersion ?? "私有 Java 21"}</p>
      {:else}
        <select bind:value={selectedSource} disabled={installing} aria-label="JRE 下载源">{#each sources as source}<option value={source.id}>{source.name}</option>{/each}</select>
        <button disabled={installing} onclick={startRuntimeInstall}><svg viewBox="0 0 24 24">{@html icon("download")}</svg>{installing ? "正在安装…" : "安装运行环境"}</button>
      {/if}
      {#if installing && progress}<div class="sidebar-progress"><span>{progress.message}</span><strong>{progressPercent === null ? "···" : `${progressPercent}%`}</strong><i><b style={`width: ${progressPercent ?? 12}%`}></b></i></div>{/if}
    </section>
    <button class:active={activeView === "plugins"} class="plugin-manager-entry" onclick={() => (activeView = "plugins")}>
      <svg viewBox="0 0 24 24">{@html icon("layers")}</svg>
      <span class="plugin-manager-entry-copy"><strong>插件管理</strong><small>{plugins.length} 个已下载插件</small></span>
    </button>
    <div class="sidebar-footer"><span class="local-badge">本地</span><div><strong>配置保存在此电脑</strong></div></div>
  </aside>

  <main class="workspace">
    {#if errorMessage}<div class="error-banner global-error"><strong>遇到问题</strong><span>{errorMessage}</span><button onclick={() => (errorMessage = "")}>×</button></div>{/if}
    {#if activeView === "plugins"}
      <section class="plugin-manager-page">
        <div class="plugin-manager-header">
          <div class="context-copy">
            <div class="breadcrumb"><span>工作区</span><i>/</i><strong>插件管理</strong></div>
            <h1>插件管理</h1>
            <p>插件下载一次后，可以在不同服务器配置中分别启用。</p>
          </div>
          <button class="plugin-manager-back" onclick={() => (activeView = "servers")}>返回当前服务器</button>
        </div>

        <div class="plugin-manager-layout">
          <article class="plugin-library-panel">
            <div class="plugin-manager-panel-heading">
              <div><span>本地插件库</span><h2>已下载插件</h2></div>
              <strong>{plugins.length}</strong>
            </div>
            <p class="plugin-manager-intro">这里的插件对所有服务器配置可用；是否启用由每台服务器单独决定。</p>
            <div class="plugin-library-section">
              <div class="official-subgroup-heading"><strong>Meta 适配插件</strong><span>{metaPlugins.length} 个</span></div>
              <div class="plugin-library-list">
                {#each metaPlugins as plugin}
                  <div class="plugin-library-card">
                    <span class="plugin-library-mark">M</span>
                    <span class="plugin-library-copy"><strong>{plugin.name}<i>{plugin.version}</i></strong><small>{plugin.description}{#if plugin.dependencies.length} · 依赖 {plugin.dependencies.join("、")}{/if}</small></span>
                    <em>已下载</em>
                  </div>
                {:else}
                  <p class="empty-plugins">尚未发现 Meta 插件。</p>
                {/each}
              </div>
            </div>
            <div class="plugin-library-section">
              <div class="official-subgroup-heading"><strong>普通插件</strong><span>{ordinaryPlugins.length} 个</span></div>
              <div class="plugin-library-list">
                {#each ordinaryPlugins as plugin}
                  <div class="plugin-library-card">
                    <span class="plugin-library-mark ordinary">P</span>
                    <span class="plugin-library-copy"><strong>{plugin.name}<i>{plugin.version}</i></strong><small>{plugin.description}{#if plugin.dependencies.length} · 依赖 {plugin.dependencies.join("、")}{/if}</small></span>
                    <em>已下载</em>
                  </div>
                {:else}
                  <p class="empty-plugins">尚未发现普通插件。</p>
                {/each}
              </div>
            </div>
            <button type="button" class="import-plugin-button plugin-library-import" disabled={importingPlugin} onclick={() => choosePluginJar()}><svg viewBox="0 0 24 24">{@html icon("plus")}</svg>{importingPlugin ? "正在导入…" : "导入插件 JAR"}</button>
            <p class="field-note">导入后会复制到 XinBot 的插件库，不会修改原始文件。</p>
          </article>

          <article class="official-catalog-panel">
            <div class="plugin-manager-panel-heading official-plugin-heading">
              <div><span>官方插件目录</span><h2>发现可用插件</h2></div>
              <button type="button" class="official-page-button" onclick={(event) => openOfficialLink(event, officialCatalog.sourceUrl)}>打开官方页面</button>
            </div>
            <p class="plugin-manager-intro">目录用于发现插件。网络不稳定时，也可以从发布页下载后再导入本地插件库。</p>
            <div class="official-plugin-toolbar">
              <input bind:value={officialPluginQuery} placeholder="搜索名称、服务器或维护者" aria-label="搜索官方插件" />
              <select bind:value={officialPluginTypeFilter} aria-label="筛选官方插件类型">
                <option value="all">全部类型</option>
                <option value="META_PLUGIN">Meta 插件</option>
                <option value="PLUGIN">普通插件</option>
              </select>
            </div>
            <div class="official-subgroup">
              <div class="official-subgroup-heading"><strong>Meta 适配插件</strong><span>{officialMetaPlugins.length} 个</span></div>
              <div class="official-plugin-list">
                {#each officialMetaPlugins as plugin}
                  <article class="official-plugin-card">
                    <div class="official-plugin-copy">
                      <strong>{plugin.name}<i>Meta</i>{#if isOfficialPluginInstalled(plugin.name, plugins)}<em>已下载</em>{/if}</strong>
                      <small>{plugin.description}</small>
                      <span>{plugin.supportedServers.length ? plugin.supportedServers.join("、") : "通用 / 页面未限定服务器"} · {plugin.maintainer}</span>
                    </div>
                    <div class="official-plugin-actions">
                      <button type="button" class="official-source-button" onclick={(event) => openOfficialLink(event, plugin.repositoryUrl)}>源码</button>
                      <button type="button" class="official-source-button" onclick={(event) => openOfficialLink(event, releasePageUrl(plugin.repositoryUrl))}>发布页</button>
                    </div>
                  </article>
                {:else}
                  <p class="empty-plugins">没有匹配的 Meta 插件。</p>
                {/each}
              </div>
            </div>
            <div class="official-subgroup">
              <div class="official-subgroup-heading"><strong>普通插件</strong><span>{officialOrdinaryPlugins.length} 个</span></div>
              <div class="official-plugin-list">
                {#each officialOrdinaryPlugins as plugin}
                  <article class="official-plugin-card">
                    <div class="official-plugin-copy">
                      <strong>{plugin.name}<i>普通</i>{#if isOfficialPluginInstalled(plugin.name, plugins)}<em>已下载</em>{/if}</strong>
                      <small>{plugin.description}</small>
                      <span>{plugin.supportedServers.length ? plugin.supportedServers.join("、") : "通用 / 页面未限定服务器"} · {plugin.maintainer}</span>
                    </div>
                    <div class="official-plugin-actions">
                      <button type="button" class="official-source-button" onclick={(event) => openOfficialLink(event, plugin.repositoryUrl)}>源码</button>
                      <button type="button" class="official-source-button" onclick={(event) => openOfficialLink(event, releasePageUrl(plugin.repositoryUrl))}>发布页</button>
                    </div>
                  </article>
                {:else}
                  <p class="empty-plugins">没有匹配的普通插件。</p>
                {/each}
              </div>
            </div>
            <p class="official-plugin-updated">目录快照：{officialCatalog.updatedAt}</p>
          </article>
        </div>
      </section>
    {:else}
    <section class="context-header">
      <div class="context-copy">
        <div class="breadcrumb"><span>服务器</span><i>/</i><strong>{form.serverName || "未命名服务器"}</strong></div>
        <h1>{form.serverName || "未命名服务器"}</h1>
        <p>当前服务器工作区 · 下方配置和控制台都只属于这个服务器</p>
        {#if configurationCollapsed}
          <div class="compact-summary">
            <div class="compact-summary-details">
              <span><b>连接</b>{form.host}:{form.port || 25565}</span>
              <span><b>账号</b>{form.username || "尚未填写"} · {form.onlineMode ? "正版" : "离线"}</span>
              <span><b>Meta</b>{selectedMeta?.name ?? form.metaPluginId}</span>
              <span><b>插件</b>{enabledPluginCount} 个</span>
            </div>
            <div class="compact-summary-actions">
              <button class="compact-edit" disabled={selectedBusy} onclick={() => (configurationCollapsed = false)}>修改配置</button>
              {#if selectedRunning}<button class="compact-stop" onclick={stopCurrent}>停止</button>{:else}<button class="compact-start" disabled={!canLaunch} onclick={startBot}>{selectedLaunching ? "启动中…" : "启动"}</button>{/if}
            </div>
          </div>
        {/if}
      </div>
      <div class="header-actions">
        <span class:dirty={isDirty} class="save-state">{isDirty ? "有未保存的修改" : saveNotice || "配置已保存"}</span>
        <button class="secondary-button" disabled={!isDirty || selectedBusy} onclick={() => saveCurrent()}><svg viewBox="0 0 24 24">{@html icon("save")}</svg>保存配置</button>
        <button class="icon-button danger" disabled={servers.length === 1 || selectedBusy} onclick={deleteServer} title="删除当前服务器" aria-label="删除当前服务器"><svg viewBox="0 0 24 24">{@html icon("trash")}</svg></button>
      </div>
    </section>

    <section class:configuration-collapsed={configurationCollapsed} class="server-workspace">
      <article class="configuration-panel">
        {#if configurationCollapsed}
          <div class="collapsed-heading"><div class="panel-heading-icon"><svg viewBox="0 0 24 24">{@html icon("server")}</svg></div><div><span>当前配置</span><h2>{form.serverName || "未命名服务器"}</h2></div></div>
          <div class="configuration-summary">
            <div><span>连接目标</span><strong>{form.host}:{form.port || 25565}</strong></div>
            <div><span>登录身份</span><strong>{form.username || "尚未填写"}</strong><small>{form.onlineMode ? "Microsoft 正版模式" : "离线模式"}</small></div>
            <div><span>Meta 适配</span><strong>{selectedMeta?.name ?? form.metaPluginId}</strong><small>{selectedMeta?.loginMode === "plugin" ? "登录流程由插件处理" : "GUI 登录命令模板"}</small></div>
            <div><span>普通插件</span><strong>{enabledPluginCount} 个已启用</strong></div>
          </div>
          <button class="edit-config-button" disabled={selectedBusy} onclick={() => (configurationCollapsed = false)}>修改配置</button>
        {:else}
          <div class="panel-heading"><div class="panel-heading-icon"><svg viewBox="0 0 24 24">{@html icon("server")}</svg></div><div><span>当前服务器 / 配置</span><h2>连接、身份与插件</h2></div><button class="collapse-button" onclick={saveAndCollapse}>保存并收起</button></div>
          <div class="config-tabs" role="tablist">
            <button class:active={configTab === "connection"} onclick={() => (configTab = "connection")}>连接设置</button>
            <button class:active={configTab === "plugins"} onclick={() => (configTab = "plugins")}>插件 <em>{1 + enabledPluginCount}</em></button>
          </div>
          <div class="configuration-scroll">
            <fieldset disabled={selectedBusy}>
              {#if configTab === "connection"}
                <div class="config-group">
                  <div class="group-heading"><b>1</b><div><strong>连接目标</strong><small>机器人要连接到哪里</small></div></div>
                  <label><span>显示名称</span><input bind:value={form.serverName} placeholder="例如：生存服" /></label>
                  <div class="split-fields"><label class="grow"><span>服务器地址</span><input bind:value={form.host} disabled={form.metaPluginId === "xinmeta"} placeholder="2b2t.xin" /></label><label class="port-field"><span>端口</span><input type="number" bind:value={form.port} disabled={form.metaPluginId === "xinmeta"} placeholder="25565" min="1" max="65535" /></label></div>
                  {#if form.metaPluginId === "xinmeta"}<p class="field-note">XinMetaPlugin 固定适配 2b2t.xin，切换到其他 Meta 后可修改地址。</p>{/if}
                </div>
                <div class="config-group">
                  <div class="group-heading"><b>2</b><div><strong>登录身份</strong><small>这个服务器使用的机器人账号</small></div></div>
                  <label><span>用户名</span><input bind:value={form.username} placeholder="Steve" /></label>
                  <label><span>二级登录密码 <i>可选</i></span><input type="password" bind:value={form.password} placeholder="服务器内登录使用" /></label>
                  {#if form.password && !form.onlineMode && selectedMeta?.loginMode === "template"}<label><span>登录命令模板</span><input bind:value={form.loginTemplate} placeholder="/login {password}" /></label>{/if}
                  {#if selectedMeta?.loginMode === "plugin"}<div class="managed-login-note"><svg viewBox="0 0 24 24">{@html icon("check")}</svg><span><strong>二次登录由 {selectedMeta.name} 处理</strong><small>GUI 不会再发送重复的登录命令。</small></span></div>{/if}
                  <label class="switch-label"><input type="checkbox" bind:checked={form.onlineMode} /><span class="switch"></span><span><strong>Microsoft 正版模式</strong><small>由 XinBot Core 处理账号认证</small></span></label>
                </div>
              {:else}
                <div class="config-group plugin-group">
                  <div class="group-heading"><b>M</b><div><strong>服务器 Meta 适配</strong><small>必选且只能选择一个，负责服务器特殊流程</small></div></div>
                  <div class="plugin-list">
                    {#each metaPlugins as plugin}
                      <div class="plugin-card-row">
                        <button type="button" class:selected={form.metaPluginId === plugin.id} class="plugin-card" onclick={() => selectMetaPlugin(plugin)}>
                          <span class="plugin-radio"></span><span class="plugin-copy"><strong>{plugin.name}<i>{plugin.version}</i></strong><small>{plugin.description}</small></span>{#if plugin.recommended}<em>推荐</em>{/if}
                        </button>
                        {#if plugin.configFiles.length}
                          <button type="button" class="plugin-config-button" onclick={() => void openPluginConfig(plugin)}>{plugin.configFiles.length > 1 ? "配置…" : "配置"}</button>
                        {/if}
                      </div>
                    {/each}
                  </div>
                </div>
                <div class="config-group plugin-group">
                  <div class="group-heading"><b>P</b><div><strong>普通插件</strong><small>按服务器分别选择，启动前自动同步</small></div></div>
                  <div class="plugin-list">
                    {#each ordinaryPlugins as plugin}
                      <div class="plugin-card-row">
                        <button type="button" class:selected={form.enabledPluginIds.includes(plugin.id)} class="plugin-card" onclick={() => togglePlugin(plugin.id)}>
                          <span class="plugin-check">{form.enabledPluginIds.includes(plugin.id) ? "✓" : ""}</span><span class="plugin-copy"><strong>{plugin.name}<i>{plugin.version}</i></strong><small>{plugin.description}{#if plugin.dependencies.length} · 依赖 {plugin.dependencies.join("、")}（自动加载）{/if}</small></span>
                        </button>
                        {#if plugin.configFiles.length}
                          <button type="button" class="plugin-config-button" onclick={() => void openPluginConfig(plugin)}>{plugin.configFiles.length > 1 ? "配置…" : "配置"}</button>
                        {/if}
                      </div>
                    {:else}
                      <p class="empty-plugins">尚未发现普通插件。</p>
                    {/each}
                  </div>
                  <p class="field-note">插件在左侧“插件管理”中统一下载或导入；这里仅选择当前服务器要启用的插件。</p>
                  {#if pluginConfigPlugin && pluginConfigSpec}
                    <div class="plugin-config-editor">
                      <div class="plugin-config-editor-heading">
                        <div><strong>{pluginConfigSpec.label || pluginConfigPlugin.name}</strong><small>{pluginConfigPlugin.name} · {pluginConfigPath}</small></div>
                        <span class:exists={pluginConfigExists}>{pluginConfigExists ? "已创建" : "待创建"}</span>
                      </div>
                      <textarea
                        value={pluginConfigContent}
                        oninput={(event) => { pluginConfigContent = (event.currentTarget as HTMLTextAreaElement).value; pluginConfigDirty = true; }}
                        disabled={!pluginConfigExists || loadingPluginConfig || savingPluginConfig || selectedInitializing}
                        spellcheck="false"
                        aria-label={`${pluginConfigPlugin.name} 配置内容`}
                      ></textarea>
                      <div class="plugin-config-editor-footer">
                        <span>{selectedInitializing ? "正在短时加载插件；不会连接服务器…" : loadingPluginConfig ? "正在读取…" : !pluginConfigExists ? "尚未生成；可以短时加载插件来创建默认配置" : pluginConfigDirty ? "有未保存的修改" : "使用插件原生配置路径"}</span>
                        {#if pluginConfigExists}
                          <button type="button" class="plugin-config-save" disabled={selectedInitializing || loadingPluginConfig || savingPluginConfig || !pluginConfigDirty} onclick={() => void savePluginConfig()}>{savingPluginConfig ? "保存中…" : "保存配置"}</button>
                        {:else}
                          <button type="button" class="plugin-config-save" disabled={selectedInitializing || loadingPluginConfig} onclick={() => void generatePluginConfig()}>{selectedInitializing ? "正在生成…" : "生成默认配置"}</button>
                        {/if}
                      </div>
                    </div>
                  {/if}
                </div>
              {/if}
            </fieldset>
          </div>
        {/if}
        <div class="launch-zone">
          <div><span>当前服务器实例</span><strong>{selectedRunning ? `${form.serverName} 正在运行` : selectedLaunching ? `${form.serverName} 正在启动` : selectedInitializing ? `${form.serverName} 正在初始化插件` : "配置完成后启动"}</strong><small>{selectedMeta?.name ?? "请选择 Meta"} · {enabledPluginCount} 个普通插件</small></div>
          {#if selectedRunning}<button class="danger-button" onclick={stopCurrent}><svg viewBox="0 0 24 24">{@html icon("stop")}</svg>停止</button>{:else}<button class="primary-button" disabled={!canLaunch} onclick={startBot}><svg viewBox="0 0 24 24">{@html icon("play")}</svg>{selectedLaunching ? "启动中…" : "启动"}</button>{/if}
        </div>
      </article>

      <article class="console-panel">
        <div class="console-header">
          <div class="console-title"><div class="terminal-icon"><svg viewBox="0 0 24 24">{@html icon("terminal")}</svg></div><div><span>当前服务器 / 控制台</span><h2>{form.serverName || "未命名服务器"} 的实时输出</h2></div></div>
          <div class="console-actions"><div class="console-meta"><i class:live={selectedRunning}></i>{selectedRunning ? "已连接实例" : "实例未启动"}<em>{consoleLines.length} 行</em></div><button onclick={clearConsole} title="清空控制台" aria-label="清空控制台"><svg viewBox="0 0 24 24">{@html icon("clear")}</svg></button></div>
        </div>
        <div class="console-output" bind:this={consoleElement}>{#each consoleLines as item, index}<div class:error={item.stream === "stderr"} class:launcher={item.stream === "launcher"} class="console-line"><span>{String(index + 1).padStart(3, "0")}</span><code>{item.line}</code></div>{/each}</div>
        <form class="command-row" onsubmit={(event) => { event.preventDefault(); void submitCommand(); }}><span>›</span><input bind:value={command} disabled={!selectedRunning} placeholder={selectedRunning ? "向当前实例发送命令，按 Enter 执行" : "启动当前服务器实例后可输入命令"} /><button disabled={!selectedRunning || !command.trim()} aria-label="发送命令"><svg viewBox="0 0 24 24">{@html icon("send")}</svg></button></form>
      </article>
    </section>

    {/if}
    <footer class="workspace-footer"><span>数据目录 · {status?.dataDir ?? "正在定位…"}</span><span>xinbot-gui-win 0.2.7</span></footer>
  </main>
</div>
