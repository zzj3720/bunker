//! Running an instance's `SPT.Server.exe`: started in its folder with its output read line by line (each line's
//! level from its colour or its `[WARN]`/`[ERROR]`), ready once its port answers, stopped gracefully (Ctrl+Break on its
//! console, as a person closing it would) before being ended. One per instance; a server Bunker did not start but
//! that holds the port is told apart (`external`) and never touched.

use crate::events;
use crate::instance::Instance;
use crate::spt_http::port_open;
use crate::util::*;
use serde::Serialize;
use serde_json::json;
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct Status {
    pub instance: String,
    /// `stopped`, `starting`, `running`, `stopping`, `failed`, `external` (another program's server on the port)
    pub state: String,
    pub pid: u32,
    pub port: u16,
    pub started_at: i64,
    pub ready_at: i64,
    pub warns: usize,
    pub errors: usize,
    pub exit_code: Option<i32>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LogLine {
    pub n: u64,
    pub t: String,
    /// `info`, `ok`, `warn`, `error`
    pub level: String,
    pub text: String,
}

struct Running {
    child: Arc<Mutex<Child>>,
    status: Status,
    log: VecDeque<LogLine>,
    next_n: u64,
    stopping: bool,
}

const LOG_KEEP: usize = 5000;

fn table() -> &'static Mutex<HashMap<String, Arc<Mutex<Running>>>> {
    static T: OnceLock<Mutex<HashMap<String, Arc<Mutex<Running>>>>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(HashMap::new()))
}

fn entry(root: &str) -> Option<Arc<Mutex<Running>>> {
    table().lock().unwrap().get(root).cloned()
}

fn emit_status(s: &Status) {
    events::emit(&json!({"ev": "server", "status": s}));
}

/// A line's level: its ANSI colour (red error, yellow warning, green success) or its text.
pub fn level_of(raw: &str) -> &'static str {
    let mut i = 0;
    let b = raw.as_bytes();
    while let Some(p) = raw[i..].find("\x1b[") {
        let s = i + p + 2;
        let mut e = s;
        while e < b.len() && (b[e].is_ascii_digit() || b[e] == b';') {
            e += 1;
        }
        for code in raw[s..e].split(';') {
            match code {
                "31" | "91" => return "error",
                "33" | "93" => return "warn",
                "32" | "92" => return "ok",
                _ => {}
            }
        }
        i = e.max(s);
        if i >= raw.len() {
            break;
        }
    }
    let up = raw.to_ascii_uppercase();
    if up.contains("[ERROR") || up.contains("[ERR]") || up.contains("[FATAL") || up.contains("[CRITICAL") || up.contains("EXCEPTION:") {
        "error"
    } else if up.contains("[WARN") {
        "warn"
    } else if up.contains("[OK]") || up.contains("[SUCCESS") {
        "ok"
    } else {
        "info"
    }
}

/// The text without ANSI escapes.
pub fn strip_ansi(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut it = raw.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\x1b' {
            if it.peek() == Some(&'[') {
                it.next();
                for d in it.by_ref() {
                    if d.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        if c != '\r' {
            out.push(c);
        }
    }
    out
}

fn push_line(run: &Arc<Mutex<Running>>, raw: &str) {
    let level = level_of(raw);
    let text = strip_ansi(raw);
    if text.trim().is_empty() {
        return;
    }
    let mut r = run.lock().unwrap();
    let line = LogLine { n: r.next_n, t: clock_text(now_ms()), level: level.into(), text };
    r.next_n += 1;
    match level {
        "warn" => r.status.warns += 1,
        "error" => r.status.errors += 1,
        _ => {}
    }
    if r.log.len() >= LOG_KEEP {
        r.log.pop_front();
    }
    r.log.push_back(line.clone());
    let inst = r.status.instance.clone();
    let (w, e) = (r.status.warns, r.status.errors);
    drop(r);
    events::emit(&json!({"ev": "log", "instance": inst, "line": line, "warns": w, "errors": e}));
}

fn read_lines(run: Arc<Mutex<Running>>, src: impl Read + Send + 'static) {
    thread::spawn(move || {
        let mut rd = BufReader::new(src);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match rd.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let s = String::from_utf8_lossy(&buf);
                    push_line(&run, s.trim_end_matches(['\n', '\r']));
                }
            }
        }
    });
}

pub fn status(inst: &Instance) -> Status {
    if let Some(run) = entry(&inst.root) {
        let r = run.lock().unwrap();
        if !matches!(r.status.state.as_str(), "stopped" | "failed") {
            return r.status.clone();
        }
        let mut s = r.status.clone();
        drop(r);
        if port_open(inst.port) {
            s.state = "external".into();
        }
        return s;
    }
    Status {
        instance: inst.root.clone(),
        state: if port_open(inst.port) { "external".into() } else { "stopped".into() },
        port: inst.port,
        ..Default::default()
    }
}

pub fn log(inst: &Instance, after: u64) -> Vec<LogLine> {
    entry(&inst.root).map(|run| run.lock().unwrap().log.iter().filter(|l| l.n >= after).cloned().collect()).unwrap_or_default()
}

/// Whether a server answers on the instance's port (ours or another's): profiles may not be touched then.
pub fn up(inst: &Instance) -> bool {
    port_open(inst.port) || entry(&inst.root).is_some_and(|r| matches!(r.lock().unwrap().status.state.as_str(), "starting" | "running" | "stopping"))
}

pub fn start(inst: &Instance) -> Res<Status> {
    if !inst.has_server {
        return err("这个实例没有 SPT.Server.exe");
    }
    if let Some(run) = entry(&inst.root) {
        let st = run.lock().unwrap().status.state.clone();
        if matches!(st.as_str(), "starting" | "running" | "stopping") {
            return err("服务端已经在运行");
        }
    }
    if port_open(inst.port) {
        return err(format!("端口 {} 已被占用：已有服务端在运行（不是 Bunker 启动的），Bunker 不会去动它", inst.port));
    }
    let dir = inst.server_dir();
    let mut cmd = Command::new(Path::new(&inst.server_exe));
    cmd.current_dir(&dir).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // a console of its own without a window: what Ctrl+C is sent to when it is stopped
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|e| format!("启动 SPT.Server.exe 失败：{e}"))?;
    let out = child.stdout.take().unwrap();
    let errp = child.stderr.take().unwrap();
    let status = Status {
        instance: inst.root.clone(),
        state: "starting".into(),
        pid: child.id(),
        port: inst.port,
        started_at: now_ms(),
        ..Default::default()
    };
    let run = Arc::new(Mutex::new(Running { child: Arc::new(Mutex::new(child)), status: status.clone(), log: VecDeque::new(), next_n: 0, stopping: false }));
    table().lock().unwrap().insert(inst.root.clone(), run.clone());
    emit_status(&status);
    read_lines(run.clone(), out);
    read_lines(run.clone(), errp);
    let port = inst.port;
    thread::spawn(move || watch(run, port));
    Ok(status)
}

/// Until it ends: ready once its port answers, then its end said (stopped by us, or exited by itself: failed).
fn watch(run: Arc<Mutex<Running>>, port: u16) {
    let child = run.lock().unwrap().child.clone();
    let mut ready = false;
    loop {
        let exited = child.lock().unwrap().try_wait().ok().flatten();
        if let Some(code) = exited {
            // let the readers finish the last lines first
            thread::sleep(Duration::from_millis(150));
            let mut r = run.lock().unwrap();
            r.status.exit_code = code.code();
            r.status.state = if r.stopping || code.success() { "stopped".into() } else { "failed".into() };
            if r.status.state == "failed" {
                r.status.message = if ready { format!("服务端意外退出（退出码 {:?}）", code.code()) } else { format!("服务端启动失败（退出码 {:?}）", code.code()) };
            }
            let s = r.status.clone();
            drop(r);
            emit_status(&s);
            return;
        }
        if !ready && port_open(port) {
            ready = true;
            let mut r = run.lock().unwrap();
            if r.status.state == "starting" {
                r.status.state = "running".into();
                r.status.ready_at = now_ms();
            }
            let s = r.status.clone();
            drop(r);
            emit_status(&s);
        }
        thread::sleep(Duration::from_millis(if ready { 500 } else { 200 }));
    }
}

/// Waits until the server is running (or has ended); its status then.
pub fn wait_ready(inst: &Instance, most: Duration) -> Res<Status> {
    let t0 = Instant::now();
    loop {
        let s = status(inst);
        match s.state.as_str() {
            "running" | "external" => return Ok(s),
            "failed" | "stopped" => return err(if s.message.is_empty() { "服务端没有启动".to_string() } else { s.message }),
            _ => {}
        }
        if t0.elapsed() > most {
            return err(format!("服务端 {} 秒内没有就绪", most.as_secs()));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Ctrl+Break to the server's console (as a person closing it would; unlike Ctrl+C it reaches a program that was
/// started ignoring Ctrl+C, which a launcher's own launcher can pass on). .NET's host takes it as a stop and saves.
#[cfg(windows)]
fn ctrl_c(pid: u32) -> bool {
    use windows_sys::Win32::System::Console::{
        AttachConsole, FreeConsole, GenerateConsoleCtrlEvent, SetConsoleCtrlHandler, SetStdHandle,
        CTRL_BREAK_EVENT, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };
    // (this process takes the event it sends to the console it shares for that moment, and does nothing with it: a
    // handler, not "ignore", which its later children would inherit)
    unsafe extern "system" fn swallow(_t: u32) -> i32 {
        1
    }
    static ONE: Mutex<()> = Mutex::new(());
    let _g = ONE.lock().unwrap();
    unsafe {
        FreeConsole();
        if AttachConsole(pid) == 0 {
            return false;
        }
        // (set while attached: attaching starts this process's handlers anew; kept, as the event comes to this
        // process on a thread of its own, later)
        SetConsoleCtrlHandler(Some(swallow), 1);
        let ok = GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, 0) != 0;
        std::thread::sleep(Duration::from_millis(100));
        FreeConsole();
        // AttachConsole + FreeConsole leaves stale/closed std handles in the PEB, which causes later
        // Command::spawn() calls inheriting stdio to fail with ERROR_INVALID_HANDLE (os error 6).
        SetStdHandle(STD_INPUT_HANDLE, std::ptr::null_mut());
        SetStdHandle(STD_OUTPUT_HANDLE, std::ptr::null_mut());
        SetStdHandle(STD_ERROR_HANDLE, std::ptr::null_mut());
        ok
    }
}

#[cfg(not(windows))]
fn ctrl_c(pid: u32) -> bool {
    Command::new("kill").arg("-INT").arg(pid.to_string()).status().is_ok_and(|s| s.success())
}

/// Stops the server Bunker started for this instance: Ctrl+Break (SIGINT elsewhere), then ended after `grace` if it is still there.
pub fn stop(inst: &Instance, grace: Duration) -> Res<Status> {
    let Some(run) = entry(&inst.root) else {
        return if port_open(inst.port) { err("运行中的服务端不是 Bunker 启动的，Bunker 不会去停它") } else { Ok(status(inst)) };
    };
    let (child, pid) = {
        let mut r = run.lock().unwrap();
        if !matches!(r.status.state.as_str(), "starting" | "running") {
            return Ok(r.status.clone());
        }
        r.stopping = true;
        r.status.state = "stopping".into();
        emit_status(&r.status);
        (r.child.clone(), r.status.pid)
    };
    let graceful = ctrl_c(pid);
    let t0 = Instant::now();
    while t0.elapsed() < grace {
        if child.lock().unwrap().try_wait().ok().flatten().is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    if child.lock().unwrap().try_wait().ok().flatten().is_none() {
        let _ = child.lock().unwrap().kill();
        let _ = child.lock().unwrap().wait();
        let mut r = run.lock().unwrap();
        r.status.message = if graceful { "服务端未在时限内退出，已强制结束".into() } else { "无法发送 Ctrl+C，已强制结束".into() };
    }
    // the watcher says it stopped; wait for that so the answer is the end state
    let t1 = Instant::now();
    while t1.elapsed() < Duration::from_secs(3) {
        if run.lock().unwrap().status.state == "stopped" || run.lock().unwrap().status.state == "failed" {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    let mut r = run.lock().unwrap();
    r.status.state = "stopped".into();
    Ok(r.status.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_from_colour_and_text() {
        assert_eq!(level_of("\x1b[31mSomething broke\x1b[0m"), "error");
        assert_eq!(level_of("\x1b[1;33mCareful\x1b[0m"), "warn");
        assert_eq!(level_of("\x1b[92mServer is running\x1b[0m"), "ok");
        assert_eq!(level_of("[WARN] old config"), "warn");
        assert_eq!(level_of("[Error] no"), "error");
        assert_eq!(level_of("[INFO] Loading 15 server mods"), "info");
        assert_eq!(strip_ansi("\x1b[33mCareful\x1b[0m\r"), "Careful");
    }
}
