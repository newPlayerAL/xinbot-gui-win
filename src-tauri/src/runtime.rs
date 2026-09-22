use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    process::Command,
    time::Duration,
};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

use crate::{app_paths::AppPaths, process::ProcessState};

const TEMURIN_API: &str = "https://api.adoptium.net/v3/assets/latest/21/hotspot?architecture=x64&image_type=jre&os=windows&vendor=eclipse";
const AZUL_API: &str = "https://api.azul.com/metadata/v1/zulu/packages/?java_version=21&os=windows&arch=x64&java_package_type=jre&javafx_bundled=false&archive_type=zip&release_status=ga&availability_types=CA&certifications=tck&latest=true&page=1&page_size=10";
const MICROSOFT_ZIP: &str = "https://aka.ms/download-jdk/microsoft-jdk-21-windows-x64.zip";
const MICROSOFT_SHA: &str =
    "https://aka.ms/download-jdk/microsoft-jdk-21-windows-x64.zip.sha256sum.txt";
const RUNTIME_SOURCE_IDS: [&str; 3] = ["azul", "temurin", "microsoft"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSource {
    id: &'static str,
    name: &'static str,
    detail: &'static str,
    recommended: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    runtime_state: &'static str,
    runtime_version: Option<String>,
    runtime_path: Option<String>,
    xinbot_ready: bool,
    xinbot_path: Option<String>,
    direct_connect_ready: bool,
    data_dir: String,
    running: bool,
    running_instance_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProgress {
    source: String,
    phase: &'static str,
    downloaded: u64,
    total: Option<u64>,
    message: String,
}

#[derive(Debug)]
struct RuntimePackage {
    source: String,
    url: String,
    checksum: String,
    filename: String,
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct TemurinRelease {
    binary: TemurinBinary,
}

#[derive(Debug, Deserialize)]
struct TemurinBinary {
    package: TemurinPackage,
}

#[derive(Debug, Deserialize)]
struct TemurinPackage {
    checksum: String,
    link: String,
    name: String,
    size: u64,
}

#[derive(Debug, Deserialize)]
struct AzulRelease {
    package_uuid: String,
    java_version: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct AzulPackage {
    download_url: String,
    name: String,
    sha256_hash: String,
    size: u64,
}

#[tauri::command]
pub fn get_runtime_sources() -> Vec<RuntimeSource> {
    vec![
        RuntimeSource {
            id: "azul",
            name: "Azul Zulu 21",
            detail: "轻量 JRE · 约 49 MB · CDN 推荐",
            recommended: true,
        },
        RuntimeSource {
            id: "temurin",
            name: "Eclipse Temurin 21",
            detail: "轻量 JRE · GitHub 备用",
            recommended: false,
        },
        RuntimeSource {
            id: "microsoft",
            name: "Microsoft OpenJDK 21",
            detail: "完整 JDK · 约 192 MB · 最后备用",
            recommended: false,
        },
    ]
}

#[tauri::command]
pub fn get_app_status(
    app: AppHandle,
    process_state: tauri::State<'_, ProcessState>,
) -> Result<AppStatus, String> {
    let paths = AppPaths::resolve(&app)?;
    let java = paths.java_exe();
    let runtime_version = if java.is_file() {
        java_version(&java).ok()
    } else {
        None
    };
    let runtime_state = if runtime_version.is_some() {
        "ready"
    } else if java.is_file() {
        "broken"
    } else {
        "missing"
    };

    Ok(AppStatus {
        runtime_state,
        runtime_version,
        runtime_path: java
            .is_file()
            .then(|| paths.runtime_dir.display().to_string()),
        xinbot_ready: paths.xinbot_jar.is_file(),
        xinbot_path: paths
            .xinbot_jar
            .is_file()
            .then(|| paths.xinbot_jar.display().to_string()),
        direct_connect_ready: paths.direct_connect_jar.is_file(),
        data_dir: paths.data_dir.display().to_string(),
        running: process_state.any_running(),
        running_instance_ids: process_state.running_ids(),
    })
}

#[tauri::command]
pub async fn install_runtime(app: AppHandle, source: String) -> Result<(), String> {
    let paths = AppPaths::resolve(&app)?;
    fs::create_dir_all(&paths.downloads_dir)
        .map_err(|error| format!("无法创建下载目录：{error}"))?;

    if !RUNTIME_SOURCE_IDS.contains(&source.as_str()) {
        return Err(format!("未知的运行时下载源：{source}"));
    }

    let client = reqwest::Client::builder()
        .user_agent("xinbot-gui-win/0.1")
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60 * 30))
        .build()
        .map_err(|error| format!("无法初始化下载器：{error}"))?;

    let mut candidates = vec![source.as_str()];
    candidates.extend(
        RUNTIME_SOURCE_IDS
            .iter()
            .copied()
            .filter(|candidate| *candidate != source),
    );
    let mut failures = Vec::new();

    for candidate in candidates {
        emit_progress(
            &app,
            source_label(candidate),
            "resolving",
            0,
            None,
            &format!("正在连接 {}", source_label(candidate)),
        );
        match install_from_source(&app, &client, &paths, candidate).await {
            Ok(()) => return Ok(()),
            Err(error) => {
                failures.push(format!("{}：{error}", source_label(candidate)));
                emit_progress(
                    &app,
                    source_label(candidate),
                    "resolving",
                    0,
                    None,
                    &format!("{} 不可用，正在尝试备用源", source_label(candidate)),
                );
            }
        }
    }

    Err(format!("所有 Java 下载源均失败：{}", failures.join("；")))
}

async fn install_from_source(
    app: &AppHandle,
    client: &reqwest::Client,
    paths: &AppPaths,
    source: &str,
) -> Result<(), String> {
    let package = resolve_package(client, source).await?;
    let archive = paths
        .downloads_dir
        .join(format!("{}.part", package.filename));

    download_package(app, client, &package, &archive).await?;
    emit_progress(
        app,
        &package.source,
        "verifying",
        package.size.unwrap_or(0),
        package.size,
        "正在校验 SHA-256",
    );
    verify_sha256(&archive, &package.checksum)?;

    emit_progress(
        app,
        &package.source,
        "extracting",
        package.size.unwrap_or(0),
        package.size,
        "正在安装私有运行环境",
    );
    install_archive(&archive, &paths.runtime_dir)?;
    let version = java_version(&paths.java_exe())?;
    emit_progress(
        app,
        &package.source,
        "ready",
        package.size.unwrap_or(0),
        package.size,
        &format!("安装完成 · {version}"),
    );
    Ok(())
}

async fn resolve_package(client: &reqwest::Client, source: &str) -> Result<RuntimePackage, String> {
    match source {
        "azul" => {
            let release = client
                .get(AZUL_API)
                .send()
                .await
                .map_err(|error| format!("无法连接 Azul Metadata API：{error}"))?
                .error_for_status()
                .map_err(|error| format!("Azul 发布接口返回错误：{error}"))?
                .json::<Vec<AzulRelease>>()
                .await
                .map_err(|error| format!("无法解析 Azul 发布信息：{error}"))?
                .into_iter()
                .max_by(|left, right| left.java_version.cmp(&right.java_version))
                .ok_or_else(|| "Azul 没有返回可用的 Windows JRE 21".to_string())?;
            let details_url = format!(
                "https://api.azul.com/metadata/v1/zulu/packages/{}",
                release.package_uuid
            );
            let package = client
                .get(details_url)
                .send()
                .await
                .map_err(|error| format!("无法获取 Azul 包详情：{error}"))?
                .error_for_status()
                .map_err(|error| format!("Azul 包详情接口返回错误：{error}"))?
                .json::<AzulPackage>()
                .await
                .map_err(|error| format!("无法解析 Azul 包详情：{error}"))?;
            if package.sha256_hash.len() != 64 || !package.download_url.starts_with("https://") {
                return Err("Azul 返回了无效的下载信息".to_string());
            }
            Ok(RuntimePackage {
                source: "Azul Zulu".to_string(),
                url: package.download_url,
                checksum: package.sha256_hash.to_ascii_lowercase(),
                filename: package.name,
                size: Some(package.size),
            })
        }
        "temurin" => {
            let releases = client
                .get(TEMURIN_API)
                .send()
                .await
                .map_err(|error| format!("无法连接 Eclipse Adoptium：{error}"))?
                .error_for_status()
                .map_err(|error| format!("Adoptium 发布接口返回错误：{error}"))?
                .json::<Vec<TemurinRelease>>()
                .await
                .map_err(|error| format!("无法解析 Adoptium 发布信息：{error}"))?;
            let package = releases
                .into_iter()
                .next()
                .ok_or_else(|| "Adoptium 没有返回可用的 Windows JRE 21".to_string())?
                .binary
                .package;
            Ok(RuntimePackage {
                source: "Eclipse Temurin".to_string(),
                url: package.link,
                checksum: package.checksum,
                filename: package.name,
                size: Some(package.size),
            })
        }
        "microsoft" => {
            let checksum_text = client
                .get(MICROSOFT_SHA)
                .send()
                .await
                .map_err(|error| format!("无法获取 Microsoft 校验值：{error}"))?
                .error_for_status()
                .map_err(|error| format!("Microsoft 校验接口返回错误：{error}"))?
                .text()
                .await
                .map_err(|error| format!("无法读取 Microsoft 校验值：{error}"))?;
            let mut fields = checksum_text.split_whitespace();
            let checksum = fields
                .next()
                .filter(|value| value.len() == 64)
                .ok_or_else(|| "Microsoft 返回了无效的 SHA-256".to_string())?;
            let filename = fields
                .next()
                .unwrap_or("microsoft-openjdk-21-windows-x64.zip");
            Ok(RuntimePackage {
                source: "Microsoft OpenJDK".to_string(),
                url: MICROSOFT_ZIP.to_string(),
                checksum: checksum.to_ascii_lowercase(),
                filename: filename.to_string(),
                size: None,
            })
        }
        _ => Err(format!("未知的运行时下载源：{source}")),
    }
}

fn source_label(source: &str) -> &'static str {
    match source {
        "azul" => "Azul Zulu",
        "temurin" => "Eclipse Temurin",
        "microsoft" => "Microsoft OpenJDK",
        _ => "未知来源",
    }
}

async fn download_package(
    app: &AppHandle,
    client: &reqwest::Client,
    package: &RuntimePackage,
    destination: &Path,
) -> Result<(), String> {
    let mut existing = destination.metadata().map(|meta| meta.len()).unwrap_or(0);
    if existing > 0 && verify_sha256(destination, &package.checksum).is_ok() {
        emit_progress(
            app,
            &package.source,
            "downloading",
            existing,
            Some(existing),
            "已找到完整的缓存文件",
        );
        return Ok(());
    }
    if package.size.is_some_and(|size| existing >= size) {
        existing = 0;
    }
    let mut request = client.get(&package.url);
    if existing > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={existing}-"));
    }
    let mut response = request
        .send()
        .await
        .map_err(|error| format!("下载 {} 失败：{error}", package.source))?;
    if response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        existing = 0;
        response = client
            .get(&package.url)
            .send()
            .await
            .map_err(|error| format!("重新下载 {} 失败：{error}", package.source))?;
    }
    response = response
        .error_for_status()
        .map_err(|error| format!("{} 下载服务器返回错误：{error}", package.source))?;

    let resumed = existing > 0 && response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    let start = if resumed { existing } else { 0 };
    let response_size = response.content_length();
    let total = package
        .size
        .or_else(|| response_size.map(|size| size + start));
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(!resumed)
        .open(destination)
        .map_err(|error| format!("无法创建下载文件：{error}"))?;
    if resumed {
        file.seek(SeekFrom::End(0))
            .map_err(|error| format!("无法续写下载文件：{error}"))?;
    }

    let mut downloaded = start;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| format!("下载中断：{error}"))?;
        file.write_all(&chunk)
            .map_err(|error| format!("无法写入下载文件：{error}"))?;
        downloaded += chunk.len() as u64;
        emit_progress(
            app,
            &package.source,
            "downloading",
            downloaded,
            total,
            if resumed {
                "正在断点续传"
            } else {
                "正在下载运行环境"
            },
        );
    }
    file.sync_all()
        .map_err(|error| format!("无法保存下载文件：{error}"))?;
    Ok(())
}

fn verify_sha256(path: &Path, expected: &str) -> Result<(), String> {
    let mut file = File::open(path).map_err(|error| format!("无法读取下载文件：{error}"))?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 1024 * 128];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("校验下载文件失败：{error}"))?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    let actual = hex::encode(hash.finalize());
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(format!(
            "JRE 校验失败。期望 {expected}，实际 {actual}。保留了断点文件以便重试"
        ));
    }
    Ok(())
}

fn install_archive(archive: &Path, final_dir: &Path) -> Result<(), String> {
    let parent = final_dir
        .parent()
        .ok_or_else(|| "运行时安装目录无效".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建运行时目录：{error}"))?;
    let staging = parent.join(format!(".java-21-installing-{}", std::process::id()));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|error| format!("无法清理临时安装：{error}"))?;
    }
    fs::create_dir_all(&staging).map_err(|error| format!("无法创建临时安装目录：{error}"))?;

    let file = File::open(archive).map_err(|error| format!("无法打开 JRE 压缩包：{error}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|error| format!("JRE 压缩包无效：{error}"))?;
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|error| format!("无法读取压缩包条目：{error}"))?;
        let Some(name) = entry.enclosed_name() else {
            return Err("JRE 压缩包包含不安全的路径".to_string());
        };
        let relative = strip_first_component(&name);
        if relative.as_os_str().is_empty() {
            continue;
        }
        let output = staging.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&output).map_err(|error| format!("无法创建目录：{error}"))?;
        } else {
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("无法创建运行时目录：{error}"))?;
            }
            let mut target =
                File::create(&output).map_err(|error| format!("无法解压运行时文件：{error}"))?;
            std::io::copy(&mut entry, &mut target)
                .map_err(|error| format!("无法解压运行时文件：{error}"))?;
        }
    }

    if !staging.join("bin").join("java.exe").is_file() {
        fs::remove_dir_all(&staging).ok();
        return Err("解压完成，但没有找到 bin\\java.exe".to_string());
    }
    if final_dir.exists() {
        fs::remove_dir_all(final_dir).map_err(|error| format!("无法替换旧运行时：{error}"))?;
    }
    fs::rename(&staging, final_dir).map_err(|error| format!("无法启用新运行时：{error}"))?;
    Ok(())
}

fn strip_first_component(path: &Path) -> PathBuf {
    let mut components = path.components();
    let _ = components.next();
    components
        .filter_map(|part| match part {
            Component::Normal(value) => Some(value),
            _ => None,
        })
        .collect()
}

fn java_version(java: &Path) -> Result<String, String> {
    let mut command = Command::new(java);
    command.arg("-version");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let output = command
        .output()
        .map_err(|error| format!("无法运行私有 Java：{error}"))?;
    if !output.status.success() {
        return Err(format!("私有 Java 返回错误代码 {}", output.status));
    }
    let text = String::from_utf8_lossy(&output.stderr);
    Ok(text.lines().next().unwrap_or("Java 21").trim().to_string())
}

fn emit_progress(
    app: &AppHandle,
    source: &str,
    phase: &'static str,
    downloaded: u64,
    total: Option<u64>,
    message: &str,
) {
    let _ = app.emit(
        "runtime-download-progress",
        RuntimeProgress {
            source: source.to_string(),
            phase,
            downloaded,
            total,
            message: message.to_string(),
        },
    );
}
