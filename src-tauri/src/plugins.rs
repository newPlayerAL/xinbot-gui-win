use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::AppHandle;

use crate::app_paths::AppPaths;

const PLUGIN: &str = "PLUGIN";
const META_PLUGIN: &str = "META_PLUGIN";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginConfigFile {
    pub path: String,
    #[serde(default = "default_config_format")]
    pub format: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDescriptor {
    pub id: String,
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    pub plugin_type: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub resource: String,
    #[serde(default)]
    pub login_mode: String,
    #[serde(default)]
    pub host_patterns: Vec<String>,
    #[serde(default)]
    pub recommended: bool,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub config_files: Vec<PluginConfigFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialPluginEntry {
    pub id: String,
    pub name: String,
    pub plugin_type: String,
    #[serde(default)]
    pub supported_servers: Vec<String>,
    #[serde(default)]
    pub maintainer: String,
    pub repository_url: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialPluginCatalog {
    pub source_url: String,
    pub updated_at: String,
    pub entries: Vec<OfficialPluginEntry>,
}

fn default_version() -> String {
    "未知版本".to_string()
}

fn default_config_format() -> String {
    "text".to_string()
}

#[tauri::command]
pub fn list_available_plugins(app: AppHandle) -> Result<Vec<PluginDescriptor>, String> {
    let paths = AppPaths::resolve(&app)?;
    collect_plugins(&paths)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginConfigRequest {
    pub profile_id: String,
    pub host: String,
    pub username: String,
    pub plugin_id: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginConfigDocument {
    pub plugin_id: String,
    pub path: String,
    pub format: String,
    pub exists: bool,
    pub content: String,
}

#[tauri::command]
pub fn read_plugin_config(
    app: AppHandle,
    request: PluginConfigRequest,
) -> Result<PluginConfigDocument, String> {
    let paths = AppPaths::resolve(&app)?;
    let (plugin, relative_path) = resolve_config_file(&paths, &request)?;
    let target = instance_config_path(&paths, &request).join(&relative_path);
    if !target.exists() {
        return Ok(PluginConfigDocument {
            plugin_id: plugin.id.clone(),
            path: relative_path,
            format: config_format(&plugin, &request.path),
            exists: false,
            content: String::new(),
        });
    }
    let metadata = fs::metadata(&target).map_err(|error| format!("无法读取插件配置：{error}"))?;
    if metadata.len() > 1024 * 1024 {
        return Err("插件配置超过 1 MiB，暂不通过编辑器打开".to_string());
    }
    let content =
        fs::read_to_string(&target).map_err(|error| format!("无法读取插件配置：{error}"))?;
    Ok(PluginConfigDocument {
        plugin_id: plugin.id.clone(),
        path: relative_path,
        format: config_format(&plugin, &request.path),
        exists: true,
        content,
    })
}

#[tauri::command]
pub fn write_plugin_config(
    app: AppHandle,
    request: PluginConfigRequest,
    content: String,
) -> Result<PluginConfigDocument, String> {
    if content.len() > 1024 * 1024 {
        return Err("插件配置超过 1 MiB，无法保存".to_string());
    }
    let paths = AppPaths::resolve(&app)?;
    let (plugin, relative_path) = resolve_config_file(&paths, &request)?;
    let instance_dir = instance_config_path(&paths, &request);
    fs::create_dir_all(&instance_dir).map_err(|error| format!("无法创建插件运行目录：{error}"))?;
    let target = instance_dir.join(&relative_path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("无法创建插件配置目录：{error}"))?;
    }
    let temporary = target.with_extension(format!(
        "{}tmp",
        target
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
    ));
    fs::write(&temporary, &content).map_err(|error| format!("无法写入插件配置：{error}"))?;
    if target.exists() {
        fs::remove_file(&target).map_err(|error| {
            let _ = fs::remove_file(&temporary);
            format!("无法替换插件配置：{error}")
        })?;
    }
    fs::rename(&temporary, &target).map_err(|error| format!("无法完成插件配置保存：{error}"))?;
    Ok(PluginConfigDocument {
        plugin_id: plugin.id.clone(),
        path: relative_path,
        format: config_format(&plugin, &request.path),
        exists: true,
        content,
    })
}

fn resolve_config_file(
    paths: &AppPaths,
    request: &PluginConfigRequest,
) -> Result<(PluginDescriptor, String), String> {
    let plugins = collect_plugins(paths)?;
    let plugin = plugins
        .iter()
        .find(|plugin| plugin.id == request.plugin_id)
        .ok_or_else(|| format!("找不到插件：{}", request.plugin_id))?;
    let relative_path = request.path.trim();
    if relative_path.is_empty() {
        return Err("插件配置路径不能为空".to_string());
    }
    let spec = plugin
        .config_files
        .iter()
        .find(|config| config.path == relative_path)
        .ok_or_else(|| format!("插件未声明配置文件：{}", relative_path))?;
    if !is_safe_relative_path(&spec.path) {
        return Err(format!("插件配置路径无效：{}", spec.path));
    }
    Ok((plugin.clone(), relative_path.to_string()))
}

fn is_safe_relative_path(path: &str) -> bool {
    let candidate = Path::new(path);
    !path.is_empty()
        && path.len() <= 240
        && !candidate.is_absolute()
        && candidate
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

fn instance_config_path(paths: &AppPaths, request: &PluginConfigRequest) -> PathBuf {
    paths
        .data_dir
        .join("instances")
        .join(crate::process::instance_id(
            &request.host,
            &request.username,
            &request.profile_id,
        ))
}

fn config_format(plugin: &PluginDescriptor, path: &str) -> String {
    plugin
        .config_files
        .iter()
        .find(|config| config.path == path)
        .map(|config| config.format.clone())
        .unwrap_or_else(default_config_format)
}

#[tauri::command]
pub fn list_official_plugins(app: AppHandle) -> Result<OfficialPluginCatalog, String> {
    let paths = AppPaths::resolve(&app)?;
    let catalog_path = paths.resources_dir.join("official_plugins.json");
    let catalog_text = fs::read_to_string(&catalog_path)
        .map_err(|error| format!("无法读取官方插件目录 {}：{error}", catalog_path.display()))?;
    let mut catalog: OfficialPluginCatalog = serde_json::from_str(&catalog_text)
        .map_err(|error| format!("官方插件目录格式无效：{error}"))?;
    if !catalog
        .source_url
        .starts_with("https://xinbot.shouldbe.top/")
    {
        return Err("官方插件目录来源地址无效".to_string());
    }
    for entry in &catalog.entries {
        if entry.id.is_empty()
            || !entry.id.chars().all(|character| {
                character.is_ascii_alphanumeric() || character == '-' || character == '_'
            })
        {
            return Err(format!("官方插件 {} 的 ID 无效", entry.name));
        }
        if entry.plugin_type != PLUGIN && entry.plugin_type != META_PLUGIN {
            return Err(format!("官方插件 {} 的类型无效", entry.name));
        }
        if !is_allowed_plugin_link(&entry.repository_url) {
            return Err(format!("官方插件 {} 的源码地址无效", entry.name));
        }
    }
    catalog.entries.sort_by(|left, right| {
        let left_rank = if left.plugin_type == META_PLUGIN {
            0
        } else {
            1
        };
        let right_rank = if right.plugin_type == META_PLUGIN {
            0
        } else {
            1
        };
        left_rank
            .cmp(&right_rank)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(catalog)
}

#[tauri::command]
pub fn open_plugin_link(url: String) -> Result<(), String> {
    if !is_allowed_plugin_link(&url) {
        return Err("只允许打开官方目录或 GitHub 源码地址".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn()
            .map_err(|error| format!("无法打开链接：{error}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|error| format!("无法打开链接：{error}"))?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|error| format!("无法打开链接：{error}"))?;
    }
    Ok(())
}

fn is_allowed_plugin_link(url: &str) -> bool {
    (url.starts_with("https://xinbot.shouldbe.top/") || url.starts_with("https://github.com/"))
        && url.len() <= 2048
        && !url
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
}

#[tauri::command]
pub fn import_plugin(app: AppHandle, path: String) -> Result<PluginDescriptor, String> {
    let paths = AppPaths::resolve(&app)?;
    let source = PathBuf::from(path);
    if !source.is_file() {
        return Err("没有找到所选插件文件".to_string());
    }
    if !source
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"))
    {
        return Err("请选择 .jar 格式的 XinBot 插件".to_string());
    }

    let metadata = read_plugin_metadata(&source)?;
    let digest = sha256_file(&source)?;
    let id = format!("local-{}", &digest[..16]);
    fs::create_dir_all(&paths.plugin_library_dir)
        .map_err(|error| format!("无法创建插件库：{error}"))?;
    let destination = paths.plugin_library_dir.join(format!("{id}.jar"));
    fs::copy(&source, &destination).map_err(|error| format!("无法导入插件：{error}"))?;

    let mut descriptor = PluginDescriptor {
        id: id.clone(),
        name: metadata.name,
        version: metadata.version,
        plugin_type: metadata.plugin_type.clone(),
        description: format!(
            "用户导入 · {}",
            source
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("plugin.jar")
        ),
        source: "imported".to_string(),
        resource: format!("{id}.jar"),
        login_mode: if metadata.plugin_type == META_PLUGIN {
            "plugin".to_string()
        } else {
            "none".to_string()
        },
        host_patterns: Vec::new(),
        recommended: false,
        dependencies: Vec::new(),
        config_files: Vec::new(),
    };
    apply_known_plugin_config(&mut descriptor);
    Ok(descriptor)
}

pub fn sync_server_plugins(
    app: &AppHandle,
    plugins_dir: &Path,
    meta_plugin_id: &str,
    enabled_plugin_ids: &[String],
) -> Result<Vec<PluginDescriptor>, String> {
    let paths = AppPaths::resolve(app)?;
    let available = collect_plugins(&paths)?;
    let by_id: HashMap<&str, &PluginDescriptor> = available
        .iter()
        .map(|plugin| (plugin.id.as_str(), plugin))
        .collect();

    let meta = by_id
        .get(meta_plugin_id)
        .copied()
        .ok_or_else(|| format!("找不到所选 Meta 插件：{meta_plugin_id}"))?;
    if meta.plugin_type != META_PLUGIN {
        return Err(format!("{} 不是 Meta 插件", meta.name));
    }

    let mut selected = vec![meta.clone()];
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    ids.insert(meta.id.clone());
    names.insert(meta.name.to_ascii_lowercase());
    for id in enabled_plugin_ids {
        if !ids.insert(id.clone()) {
            continue;
        }
        let plugin = by_id
            .get(id.as_str())
            .copied()
            .ok_or_else(|| format!("找不到已启用插件：{id}"))?;
        if plugin.plugin_type == META_PLUGIN {
            return Err(format!("每台服务器只能选择一个 Meta 插件：{}", plugin.name));
        }
        if !names.insert(plugin.name.to_ascii_lowercase()) {
            return Err(format!("不能同时启用两个同名插件：{}", plugin.name));
        }
        selected.push(plugin.clone());
    }

    let mut sources = Vec::with_capacity(selected.len());
    for plugin in &selected {
        let source = source_path(&paths, plugin)?;
        let actual = read_plugin_metadata(&source)?;
        if actual.plugin_type != plugin.plugin_type {
            return Err(format!("插件类型与目录信息不一致：{}", plugin.name));
        }
        sources.push((plugin, source));
    }

    fs::create_dir_all(plugins_dir).map_err(|error| format!("无法创建插件目录：{error}"))?;
    let staging = plugins_dir.join(".gui-staging");
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|error| format!("无法清理插件暂存目录：{error}"))?;
    }
    fs::create_dir_all(&staging).map_err(|error| format!("无法创建插件暂存目录：{error}"))?;

    let stage_result = (|| {
        for (plugin, source) in &sources {
            let destination = staging.join(format!("{}.jar", plugin.id));
            fs::copy(source, destination)
                .map_err(|error| format!("无法准备插件 {}：{error}", plugin.name))?;
        }
        Ok::<(), String>(())
    })();
    if let Err(error) = stage_result {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }

    let entries =
        fs::read_dir(plugins_dir).map_err(|error| format!("无法读取插件目录：{error}"))?;
    for entry in entries {
        let path = entry
            .map_err(|error| format!("无法读取插件目录项：{error}"))?
            .path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"))
        {
            fs::remove_file(&path).map_err(|error| format!("无法停用旧插件：{error}"))?;
        }
    }
    for entry in fs::read_dir(&staging).map_err(|error| format!("无法读取暂存插件：{error}"))?
    {
        let source = entry
            .map_err(|error| format!("无法读取暂存插件项：{error}"))?
            .path();
        let destination = plugins_dir.join(
            source
                .file_name()
                .ok_or_else(|| "插件文件名无效".to_string())?,
        );
        fs::rename(source, destination).map_err(|error| format!("无法启用插件：{error}"))?;
    }
    fs::remove_dir_all(&staging).map_err(|error| format!("无法完成插件同步：{error}"))?;
    Ok(selected)
}

fn collect_plugins(paths: &AppPaths) -> Result<Vec<PluginDescriptor>, String> {
    let catalog_path = paths.resources_dir.join("catalog.json");
    let catalog_text = fs::read_to_string(&catalog_path)
        .map_err(|error| format!("无法读取插件目录 {}：{error}", catalog_path.display()))?;
    let mut plugins: Vec<PluginDescriptor> = serde_json::from_str(&catalog_text)
        .map_err(|error| format!("插件目录格式无效：{error}"))?;
    for plugin in &mut plugins {
        plugin.source = "bundled".to_string();
        normalize_descriptor(plugin)?;
    }

    if paths.plugin_library_dir.is_dir() {
        let mut entries: Vec<PathBuf> = fs::read_dir(&paths.plugin_library_dir)
            .map_err(|error| format!("无法读取用户插件库：{error}"))?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.is_file()
                    && path
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"))
            })
            .collect();
        entries.sort();
        for path in entries {
            let id = path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string();
            if !is_local_id(&id) {
                continue;
            }
            match read_plugin_metadata(&path) {
                Ok(metadata) => {
                    let mut descriptor = PluginDescriptor {
                        id: id.clone(),
                        name: metadata.name,
                        version: metadata.version,
                        plugin_type: metadata.plugin_type.clone(),
                        description: "用户导入的插件".to_string(),
                        source: "imported".to_string(),
                        resource: format!("{id}.jar"),
                        login_mode: if metadata.plugin_type == META_PLUGIN {
                            "plugin".to_string()
                        } else {
                            "none".to_string()
                        },
                        host_patterns: Vec::new(),
                        recommended: false,
                        dependencies: Vec::new(),
                        config_files: Vec::new(),
                    };
                    apply_known_plugin_config(&mut descriptor);
                    plugins.push(descriptor);
                }
                Err(_) => continue,
            }
        }
    }

    plugins.sort_by(|left, right| {
        let left_rank = if left.plugin_type == META_PLUGIN {
            0
        } else {
            1
        };
        let right_rank = if right.plugin_type == META_PLUGIN {
            0
        } else {
            1
        };
        left_rank
            .cmp(&right_rank)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(plugins)
}

fn normalize_descriptor(plugin: &mut PluginDescriptor) -> Result<(), String> {
    plugin.plugin_type = plugin.plugin_type.trim().to_ascii_uppercase();
    if plugin.plugin_type != PLUGIN && plugin.plugin_type != META_PLUGIN {
        return Err(format!("插件 {} 的类型无效", plugin.name));
    }
    if plugin.id.is_empty()
        || !plugin.id.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        return Err(format!("插件 {} 的 ID 无效", plugin.name));
    }
    if plugin.resource.is_empty()
        || Path::new(&plugin.resource)
            .file_name()
            .and_then(|name| name.to_str())
            != Some(plugin.resource.as_str())
    {
        return Err(format!("插件 {} 的资源路径无效", plugin.name));
    }
    apply_known_plugin_config(plugin);
    Ok(())
}

fn apply_known_plugin_config(plugin: &mut PluginDescriptor) {
    if plugin.name.eq_ignore_ascii_case("BackToTheBase") {
        if plugin.dependencies.is_empty() {
            plugin.dependencies.push("MovementSync".to_string());
        }
        if plugin.config_files.is_empty() {
            plugin.config_files.push(PluginConfigFile {
                path: "base_config.json".to_string(),
                format: "json".to_string(),
                label: "BackToTheBase 配置".to_string(),
            });
        }
    }
}

fn source_path(paths: &AppPaths, plugin: &PluginDescriptor) -> Result<PathBuf, String> {
    let path = match plugin.source.as_str() {
        "bundled" => paths.resources_dir.join(&plugin.resource),
        "imported" if is_local_id(&plugin.id) => paths.plugin_library_dir.join(&plugin.resource),
        _ => return Err(format!("插件来源无效：{}", plugin.name)),
    };
    path.is_file()
        .then_some(path)
        .ok_or_else(|| format!("插件文件不存在：{}", plugin.name))
}

fn is_local_id(id: &str) -> bool {
    id.strip_prefix("local-").is_some_and(|suffix| {
        suffix.len() == 16
            && suffix
                .chars()
                .all(|character| character.is_ascii_hexdigit())
    })
}

struct JarMetadata {
    name: String,
    version: String,
    plugin_type: String,
}

fn read_plugin_metadata(path: &Path) -> Result<JarMetadata, String> {
    let file = fs::File::open(path).map_err(|error| format!("无法打开插件：{error}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| format!("插件不是有效的 JAR：{error}"))?;
    let mut entry = archive
        .by_name("plugin.yml")
        .map_err(|_| "所选 JAR 不包含 plugin.yml，不是 XinBot 插件".to_string())?;
    let mut text = String::new();
    entry
        .read_to_string(&mut text)
        .map_err(|error| format!("无法读取 plugin.yml：{error}"))?;
    let name = yaml_scalar(&text, "name").ok_or_else(|| "plugin.yml 缺少 name".to_string())?;
    let version = yaml_scalar(&text, "version").unwrap_or_else(|| "未知版本".to_string());
    let plugin_type = yaml_scalar(&text, "type")
        .unwrap_or_else(|| PLUGIN.to_string())
        .to_ascii_uppercase();
    if plugin_type != PLUGIN && plugin_type != META_PLUGIN {
        return Err(format!("plugin.yml 中的插件类型无效：{plugin_type}"));
    }
    Ok(JarMetadata {
        name,
        version,
        plugin_type,
    })
}

fn yaml_scalar(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        if line.is_empty() || line.starts_with(char::is_whitespace) || line.starts_with('#') {
            continue;
        }
        let Some((candidate, value)) = line.split_once(':') else {
            continue;
        };
        if candidate.trim().eq_ignore_ascii_case(key) {
            let value = value.trim();
            let value = if value.len() >= 2
                && ((value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\'')))
            {
                &value[1..value.len() - 1]
            } else {
                value
            };
            return (!value.is_empty()).then(|| value.to_string());
        }
    }
    None
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|error| format!("无法读取插件：{error}"))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("无法读取插件：{error}"))?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hex::encode(hash.finalize()))
}
