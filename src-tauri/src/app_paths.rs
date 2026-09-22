use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

pub struct AppPaths {
    pub data_dir: PathBuf,
    pub runtime_dir: PathBuf,
    pub downloads_dir: PathBuf,
    pub resources_dir: PathBuf,
    pub plugin_library_dir: PathBuf,
    pub xinbot_jar: PathBuf,
    pub direct_connect_jar: PathBuf,
}

impl AppPaths {
    pub fn resolve(app: &AppHandle) -> Result<Self, String> {
        let data_dir = app
            .path()
            .app_local_data_dir()
            .map_err(|error| format!("无法确定应用数据目录：{error}"))?;
        let resource_dir = app
            .path()
            .resource_dir()
            .map_err(|error| format!("无法确定资源目录：{error}"))?;

        let bundled = resource_dir.join("resources");
        let resources_dir = if bundled.join("xinbot.jar").is_file() {
            bundled
        } else {
            // Development fallback. Release builds always use Tauri's bundled resources.
            let project = Path::new(env!("CARGO_MANIFEST_DIR"));
            project.join("resources")
        };
        let xinbot_jar = resources_dir.join("xinbot.jar");
        let direct_connect_jar = resources_dir.join("directconnect.jar");

        Ok(Self {
            runtime_dir: data_dir.join("runtime").join("java-21"),
            downloads_dir: data_dir.join("downloads"),
            plugin_library_dir: data_dir.join("plugin-library"),
            data_dir,
            resources_dir,
            xinbot_jar,
            direct_connect_jar,
        })
    }

    pub fn java_exe(&self) -> PathBuf {
        self.runtime_dir.join("bin").join("java.exe")
    }
}
