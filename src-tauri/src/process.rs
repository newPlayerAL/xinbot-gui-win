use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};

use crate::{app_paths::AppPaths, plugins::sync_server_plugins};

#[derive(Default)]
pub struct ProcessState {
    processes: Mutex<HashMap<String, RunningProcess>>,
    starting: Mutex<HashSet<String>>,
}

struct StartReservation<'a> {
    state: &'a ProcessState,
    profile_id: String,
    committed: bool,
}

impl StartReservation<'_> {
    fn commit(mut self) {
        self.state.clear_starting(&self.profile_id);
        self.committed = true;
    }
}

impl Drop for StartReservation<'_> {
    fn drop(&mut self) {
        if !self.committed {
            self.state.clear_starting(&self.profile_id);
        }
    }
}

struct RunningProcess {
    child: Arc<Mutex<Child>>,
    stdin: Arc<Mutex<ChildStdin>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequest {
    #[serde(default)]
    profile_id: String,
    server_name: String,
    host: String,
    port: Option<u16>,
    username: String,
    password: String,
    online_mode: bool,
    login_template: String,
    #[serde(default = "default_meta_plugin")]
    meta_plugin_id: String,
    #[serde(default)]
    enabled_plugin_ids: Vec<String>,
}

fn default_meta_plugin() -> String {
    "directconnect".to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConsoleEvent {
    line: String,
    stream: &'static str,
    profile_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StateEvent {
    running: bool,
    exit_code: Option<i32>,
    message: String,
    profile_id: String,
}

impl ProcessState {
    fn reserve_start(&self, profile_id: &str) -> Result<StartReservation<'_>, String> {
        let mut starting = self
            .starting
            .lock()
            .map_err(|_| "启动状态锁已损坏".to_string())?;
        if starting.contains(profile_id) {
            return Err("该服务器配置正在启动".to_string());
        }

        let processes = self
            .processes
            .lock()
            .map_err(|_| "进程状态锁已损坏".to_string())?;
        if processes.get(profile_id).is_some_and(|process| {
            process
                .child
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .try_wait()
                .map(|status| status.is_none())
                .unwrap_or(false)
        }) {
            return Err("该服务器配置已有 XinBot 实例正在运行".to_string());
        }

        starting.insert(profile_id.to_string());
        Ok(StartReservation {
            state: self,
            profile_id: profile_id.to_string(),
            committed: false,
        })
    }

    fn clear_starting(&self, profile_id: &str) {
        if let Ok(mut starting) = self.starting.lock() {
            starting.remove(profile_id);
        }
    }

    pub fn is_running(&self, profile_id: &str) -> bool {
        let guard = self
            .processes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.get(profile_id).is_some_and(|process| {
            process
                .child
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .try_wait()
                .map(|status| status.is_none())
                .unwrap_or(false)
        })
    }

    pub fn any_running(&self) -> bool {
        let guard = self
            .processes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.values().any(|process| {
            process
                .child
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .try_wait()
                .map(|status| status.is_none())
                .unwrap_or(false)
        })
    }

    pub fn running_ids(&self) -> Vec<String> {
        let guard = self
            .processes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard
            .iter()
            .filter_map(|(id, process)| {
                process
                    .child
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .try_wait()
                    .ok()
                    .filter(|status| status.is_none())
                    .map(|_| id.clone())
            })
            .collect()
    }

    pub fn force_kill(&self) {
        if let Ok(mut starting) = self.starting.lock() {
            starting.clear();
        }
        let mut processes = self
            .processes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for process in processes.drain().map(|(_, process)| process) {
            let _ = process
                .child
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .kill();
        }
    }
}

#[tauri::command]
pub fn launch_bot(
    app: AppHandle,
    state: tauri::State<'_, ProcessState>,
    request: LaunchRequest,
) -> Result<(), String> {
    validate_request(&request)?;
    let profile_id = profile_id_for_request(&request);
    let reservation = state.reserve_start(&profile_id)?;

    let paths = AppPaths::resolve(&app)?;
    // Tauri can return Windows paths using the `\\?\` verbatim prefix. The
    // Windows process APIs accept those paths, but the Java launcher does not
    // resolve a verbatim path supplied as a classpath entry. Convert only the
    // paths crossing the Java command-line boundary back to their conventional
    // form; filesystem operations can keep using the original paths.
    let java = java_compatible_path(&paths.java_exe());
    if !java.is_file() {
        return Err("请先安装 Java 21 运行环境".to_string());
    }
    if !paths.xinbot_jar.is_file() {
        return Err(format!(
            "没有找到 xinbot.jar：{}",
            paths.xinbot_jar.display()
        ));
    }
    validate_core_jar(&paths.xinbot_jar)?;

    let id = instance_id(&request.host, &request.username, &profile_id);
    let working_dir = paths.data_dir.join("instances").join(&id);
    let plugins_dir = working_dir.join("plugins");
    fs::create_dir_all(&plugins_dir).map_err(|error| format!("无法创建实例目录：{error}"))?;
    let selected_plugins = sync_server_plugins(
        &app,
        &plugins_dir,
        &request.meta_plugin_id,
        &request.enabled_plugin_ids,
    )?;
    let config = working_dir.join("config.conf");
    write_config(&config, &request)?;
    let java_jar = java_compatible_path(&paths.xinbot_jar);
    let java_config = java_compatible_path(&config);

    let mut command = Command::new(&java);
    command
        .current_dir(&working_dir)
        .arg("-Dorg.jline.terminal.dumb=true")
        .arg("-Dfile.encoding=UTF-8");
    if request.meta_plugin_id == "directconnect" {
        command.arg(format!("-Dxinbot.server.host={}", request.host));
        if let Some(port) = request.port {
            command.arg(format!("-Dxinbot.server.port={port}"));
        }
        if !request.online_mode && !request.login_template.trim().is_empty() {
            command.arg(format!(
                "-Dxinbot.login.template={}",
                request.login_template.trim()
            ));
        }
    }
    command
        .arg("-cp")
        .arg(&java_jar)
        .arg("xin.bbtt.mcbot.Xinbot")
        .arg(&java_config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("无法启动 XinBot：{error}"))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "无法连接 XinBot 标准输入".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法连接 XinBot 标准输出".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "无法连接 XinBot 错误输出".to_string())?;
    let child = Arc::new(Mutex::new(child));
    let stdin = Arc::new(Mutex::new(stdin));

    pump_output(app.clone(), stdout, "stdout", profile_id.clone());
    pump_output(app.clone(), stderr, "stderr", profile_id.clone());

    {
        let mut processes = state
            .processes
            .lock()
            .map_err(|_| "进程状态锁已损坏".to_string())?;
        processes.insert(
            profile_id.clone(),
            RunningProcess {
                child: Arc::clone(&child),
                stdin,
            },
        );
    }
    reservation.commit();

    let _ = app.emit(
        "bot-console",
        ConsoleEvent {
            line: format!("[launcher] Core 已验证：{}", java_jar.display()),
            stream: "launcher",
            profile_id: profile_id.clone(),
        },
    );
    let _ = app.emit(
        "bot-console",
        ConsoleEvent {
            line: format!(
                "[launcher] 本实例插件：{}",
                selected_plugins
                    .iter()
                    .map(|plugin| plugin.name.as_str())
                    .collect::<Vec<_>>()
                    .join("、")
            ),
            stream: "launcher",
            profile_id: profile_id.clone(),
        },
    );
    let _ = app.emit(
        "bot-state",
        StateEvent {
            running: true,
            exit_code: None,
            message: format!(
                "[launcher] 已启动 {} / {}",
                request.server_name, request.username
            ),
            profile_id: profile_id.clone(),
        },
    );
    watch_exit(app, profile_id, child);
    Ok(())
}

fn validate_core_jar(path: &Path) -> Result<(), String> {
    let file = fs::File::open(path).map_err(|error| format!("无法读取 XinBot Core：{error}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| format!("XinBot Core 压缩包无效：{error}"))?;
    archive
        .by_name("xin/bbtt/mcbot/Xinbot.class")
        .map_err(|_| "XinBot Core 缺少主类 xin.bbtt.mcbot.Xinbot".to_string())?;
    Ok(())
}

fn java_compatible_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let raw = path.as_os_str().to_string_lossy();
        if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = raw.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path.to_path_buf()
}

#[tauri::command]
pub fn send_bot_command(
    state: tauri::State<'_, ProcessState>,
    profile_id: String,
    command: String,
) -> Result<(), String> {
    let line = command.trim();
    if line.is_empty() {
        return Ok(());
    }
    let processes = state
        .processes
        .lock()
        .map_err(|_| "进程状态锁已损坏".to_string())?;
    let process = processes
        .get(&profile_id)
        .ok_or_else(|| "当前服务器实例尚未运行".to_string())?;
    let mut stdin = process
        .stdin
        .lock()
        .map_err(|_| "标准输入锁已损坏".to_string())?;
    writeln!(stdin, "{line}").map_err(|error| format!("无法发送命令：{error}"))?;
    stdin
        .flush()
        .map_err(|error| format!("无法发送命令：{error}"))
}

#[tauri::command]
pub fn stop_bot(state: tauri::State<'_, ProcessState>, profile_id: String) -> Result<(), String> {
    let processes = state
        .processes
        .lock()
        .map_err(|_| "进程状态锁已损坏".to_string())?;
    let Some(process) = processes.get(&profile_id) else {
        return Ok(());
    };
    let mut stdin = process
        .stdin
        .lock()
        .map_err(|_| "标准输入锁已损坏".to_string())?;
    writeln!(stdin, "stop").map_err(|error| format!("无法发送停止命令：{error}"))?;
    stdin
        .flush()
        .map_err(|error| format!("无法发送停止命令：{error}"))
}

fn validate_request(request: &LaunchRequest) -> Result<(), String> {
    if request.host.trim().is_empty() {
        return Err("请填写服务器地址".to_string());
    }
    if request.host.chars().any(char::is_whitespace) {
        return Err("服务器地址不能包含空格".to_string());
    }
    if request.username.trim().is_empty() {
        return Err("请填写用户名".to_string());
    }
    Ok(())
}

fn profile_id_for_request(request: &LaunchRequest) -> String {
    let profile_id = request.profile_id.trim();
    if profile_id.is_empty() {
        instance_id(&request.host, &request.username, "legacy")
    } else {
        profile_id.to_string()
    }
}

fn instance_id(host: &str, username: &str, profile_id: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(host.trim().to_ascii_lowercase());
    hash.update([0]);
    hash.update(username.trim().to_ascii_lowercase());
    hash.update([0]);
    hash.update(profile_id);
    format!("instance-{}", &hex::encode(hash.finalize())[..12])
}

fn write_config(path: &Path, request: &LaunchRequest) -> Result<(), String> {
    let full_session = fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|root| root.get("account")?.get("fullSession").cloned())
        .unwrap_or(Value::Null);
    let config = json!({
        "account": {
            "fullSession": full_session,
            "name": request.username.trim(),
            "onlineMode": request.online_mode,
            "password": request.password,
        },
        "enableTranslation": false,
        "reconnectTimeout": 5000,
        "reconnectDelay": 3000,
        "owner": request.username.trim(),
        "plugin": { "directory": "plugins" },
        "proxy": {
            "enable": false,
            "info": { "address": "", "type": "", "password": "", "username": "" }
        }
    });
    let text = serde_json::to_string_pretty(&config)
        .map_err(|error| format!("无法生成 XinBot 配置：{error}"))?;
    fs::write(path, text).map_err(|error| format!("无法保存 XinBot 配置：{error}"))
}

fn pump_output<R>(app: AppHandle, reader: R, stream: &'static str, profile_id: String)
where
    R: std::io::Read + Send + 'static,
{
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer) {
                Ok(0) => break,
                Ok(_) => {
                    while buffer
                        .last()
                        .is_some_and(|byte| *byte == b'\n' || *byte == b'\r')
                    {
                        buffer.pop();
                    }
                    let line = match std::str::from_utf8(&buffer) {
                        Ok(text) => text.to_owned(),
                        Err(_) => encoding_rs::GBK.decode(&buffer).0.into_owned(),
                    };
                    let _ = app.emit(
                        "bot-console",
                        ConsoleEvent {
                            line,
                            stream,
                            profile_id: profile_id.clone(),
                        },
                    );
                }
                Err(error) => {
                    let _ = app.emit(
                        "bot-console",
                        ConsoleEvent {
                            line: format!("[launcher] 读取输出失败：{error}"),
                            stream: "stderr",
                            profile_id: profile_id.clone(),
                        },
                    );
                    break;
                }
            }
        }
    });
}

fn watch_exit(app: AppHandle, profile_id: String, child: Arc<Mutex<Child>>) {
    thread::spawn(move || {
        let exit_code = loop {
            let result = child
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .try_wait();
            match result {
                Ok(Some(status)) => break status.code(),
                Ok(None) => thread::sleep(Duration::from_millis(250)),
                Err(_) => break None,
            }
        };

        let state = app.state::<ProcessState>();
        let mut processes = state
            .processes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let should_remove = processes
            .get(&profile_id)
            .is_some_and(|running| Arc::ptr_eq(&running.child, &child));
        if should_remove {
            processes.remove(&profile_id);
        }
        drop(processes);
        let _ = app.emit(
            "bot-state",
            StateEvent {
                running: false,
                exit_code,
                message: format!(
                    "[launcher] XinBot 已退出{}",
                    exit_code
                        .map(|code| format!("（代码 {code}）"))
                        .unwrap_or_default()
                ),
                profile_id,
            },
        );
    });
}

#[allow(dead_code)]
fn _assert_path_is_local(_path: &PathBuf) {}
