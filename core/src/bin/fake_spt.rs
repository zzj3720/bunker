//! A stand-in SPT server (and game) for Bunker's tests, never shipped. Named `SPT.Server.exe` it acts as a server:
//! prints coloured log lines, serves the launcher's routes over HTTPS with a self-signed certificate on the port of
//! `SPT_Data/configs/http.json`, and on Ctrl+C says it saves profiles and exits. Named `EscapeFromTarkov.exe` it
//! writes the arguments it was started with to `game_args.txt` and exits.

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;

fn zlib(b: &[u8]) -> Vec<u8> {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
    e.write_all(b).unwrap();
    e.finish().unwrap()
}

fn inflate(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    if ZlibDecoder::new(b).read_to_end(&mut out).is_ok() {
        out
    } else {
        b.to_vec()
    }
}

#[cfg(windows)]
fn on_ctrl_c() {
    use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
    unsafe extern "system" fn handler(_t: u32) -> i32 {
        println!("[INFO] Saving profiles before exit");
        std::process::exit(0);
    }
    unsafe {
        SetConsoleCtrlHandler(Some(handler), 1);
    }
}

#[cfg(not(windows))]
fn on_ctrl_c() {}

fn answer(method: &str, path: &str, body: &[u8]) -> (u16, Vec<u8>) {
    let body = inflate(body);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    let text = match (method, path) {
        (_, "/launcher/profiles") => r#"[{"username":"bear","nickname":"BEAR_Main","side":"Bear","currlvl":34,"edition":"Edge Of Darkness","profileId":"6afeab5dc8ca1f0ed4b321be"}]"#.to_string(),
        ("PUT", "/launcher/profile/login") => {
            if v.get("username").and_then(|u| u.as_str()) == Some("bear") {
                "6afeab5dc8ca1f0ed4b321be".into()
            } else {
                "FAILED".into()
            }
        }
        ("PUT", "/launcher/profile/register") => "OK".into(),
        (_, "/launcher/server/connect") => r#"{"backendUrl":"https://127.0.0.1","name":"SPT","editions":["Standard","Edge Of Darkness"]}"#.into(),
        _ => return (404, Vec::new()),
    };
    (200, zlib(text.as_bytes()))
}

fn serve(port: u16) {
    let cert = rcgen::generate_simple_self_signed(vec!["127.0.0.1".into(), "localhost".into()]).unwrap();
    let der = rustls::pki_types::CertificateDer::from(cert.cert.der().to_vec());
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()));
    let cfg = rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![der], key)
        .unwrap();
    let cfg = Arc::new(cfg);
    let l = TcpListener::bind(("127.0.0.1", port)).expect("port");
    println!("\x1b[32m[OK] webserver started on https://127.0.0.1:{port}\x1b[0m");
    println!("\x1b[92mServer is running, do not close while playing SPT, Happy playing!!\x1b[0m");
    for s in l.incoming().flatten() {
        let cfg = cfg.clone();
        std::thread::spawn(move || {
            let conn = rustls::ServerConnection::new(cfg).unwrap();
            let mut tls = rustls::StreamOwned::new(conn, s);
            let mut rd = BufReader::new(&mut tls);
            let mut line = String::new();
            if rd.read_line(&mut line).is_err() || line.is_empty() {
                return;
            }
            let mut parts = line.split_whitespace();
            let method = parts.next().unwrap_or("").to_string();
            let path = parts.next().unwrap_or("").to_string();
            let mut len = 0usize;
            loop {
                let mut h = String::new();
                if rd.read_line(&mut h).is_err() || h == "\r\n" || h.is_empty() {
                    break;
                }
                if let Some((k, v)) = h.split_once(':') {
                    if k.eq_ignore_ascii_case("content-length") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                }
            }
            let mut body = vec![0u8; len];
            let _ = rd.read_exact(&mut body);
            let (code, out) = answer(&method, &path, &body);
            let head = format!("HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", out.len());
            let _ = tls.write_all(head.as_bytes());
            let _ = tls.write_all(&out);
            let _ = tls.flush();
            tls.conn.send_close_notify();
            let _ = tls.flush();
        });
    }
}

fn main() {
    let exe = std::env::current_exe().unwrap();
    let name = exe.file_name().unwrap().to_string_lossy().to_ascii_lowercase();
    if name.contains("escapefromtarkov") {
        let args: Vec<String> = std::env::args().skip(1).collect();
        std::fs::write("game_args.txt", args.join(" ")).unwrap();
        std::thread::sleep(Duration::from_millis(500));
        return;
    }
    on_ctrl_c();
    let http = std::fs::read("SPT_Data/configs/http.json").ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
    let port = http.and_then(|v| v.get("port").and_then(|p| p.as_u64())).unwrap_or(6969) as u16;
    println!("[INFO] Loading 2 server mods from SPT_Runtime/user/mods...");
    println!("\x1b[33m[WARN] GuidingLight: config value out of range, using default\x1b[0m");
    eprintln!("\x1b[31m[ERROR] Mod Broken failed to load\x1b[0m");
    println!("[INFO] 正在生成跳蚤报价……");
    std::thread::sleep(Duration::from_millis(300));
    serve(port);
}
