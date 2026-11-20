//! Talking to an SPT server: HTTPS on 127.0.0.1 with the server's self-signed certificate (accepted only there),
//! bodies sent zlib-compressed by PUT (the server always inflates a PUT's body), answers inflated (or taken as they
//! are when not compressed). The launcher's routes of both generations: `/launcher/profiles`,
//! `/launcher/profile/login`, `/launcher/profile/register` (4.0, 4.1), and `/launcher/v2/*` (later servers).

use crate::util::*;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use serde::Serialize;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug)]
struct AnyCert;

impl rustls::client::danger::ServerCertVerifier for AnyCert {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes()
    }
}

/// An agent for the local server only: it accepts any certificate, so it is never used for another host.
fn local_agent(timeout: Duration) -> ureq::Agent {
    let cfg = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .expect("TLS versions")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AnyCert))
        .with_no_client_auth();
    ureq::AgentBuilder::new().tls_config(Arc::new(cfg)).timeout(timeout).build()
}

pub fn inflate_or_raw(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    if ZlibDecoder::new(b).read_to_end(&mut out).is_ok() && !out.is_empty() {
        out
    } else {
        b.to_vec()
    }
}

pub fn deflate(b: &[u8]) -> Vec<u8> {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::best());
    e.write_all(b).unwrap();
    e.finish().unwrap()
}

pub struct Server {
    pub port: u16,
    agent: ureq::Agent,
}

impl Server {
    pub fn new(port: u16) -> Server {
        Server { port, agent: local_agent(Duration::from_secs(10)) }
    }

    fn url(&self, path: &str) -> String {
        format!("https://127.0.0.1:{}{}", self.port, path)
    }

    /// GET (no body) or PUT (a zlib body); the answer's JSON. A 404 or an "UNHANDLED" answer is `Ok(None)`.
    pub fn call(&self, path: &str, body: Option<&Value>) -> Res<Option<Value>> {
        let r = match body {
            None => self.agent.get(&self.url(path)).call(),
            Some(b) => self.agent.put(&self.url(path)).set("Content-Type", "application/json").send_bytes(&deflate(b.to_string().as_bytes())),
        };
        let resp = match r {
            Ok(r) => r,
            Err(ureq::Error::Status(404, _)) => return Ok(None),
            Err(ureq::Error::Status(code, _)) => return err(format!("服务端返回 HTTP {code}：{path}")),
            Err(e) => return err(format!("连不上服务端（127.0.0.1:{}）：{e}", self.port)),
        };
        let mut raw = Vec::new();
        resp.into_reader().take(64 << 20).read_to_end(&mut raw).map_err(|e| e.to_string())?;
        let text = inflate_or_raw(&raw);
        if text.is_empty() {
            return Ok(Some(Value::Null));
        }
        let v: Value = serde_json::from_slice(&text).unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&text).into_owned()));
        if v.get("errmsg").and_then(|m| m.as_str()).is_some_and(|m| m.contains("UNHANDLED")) {
            return Ok(None);
        }
        Ok(Some(v))
    }

    /// The first of several routes the server knows.
    fn first(&self, tries: &[(&str, Option<&Value>)]) -> Res<(usize, Value)> {
        for (i, (path, body)) in tries.iter().enumerate() {
            if let Some(v) = self.call(path, *body)? {
                return Ok((i, v));
            }
        }
        err(format!("服务端不认识这些接口：{}", tries.iter().map(|t| t.0).collect::<Vec<_>>().join("、")))
    }

    pub fn profiles(&self) -> Res<Vec<MiniProfile>> {
        let (_, v) = self.first(&[("/launcher/profiles", None), ("/launcher/v2/profiles", Some(&json!({})))])?;
        let list = v.get("Response").cloned().unwrap_or(v);
        Ok(list.as_array().map(|a| a.iter().map(MiniProfile::from_json).collect()).unwrap_or_default())
    }

    /// Logs in a profile by its username; the token the game is started with (its profile id).
    pub fn login(&self, username: &str, password: &str) -> Res<String> {
        let body = json!({"username": username, "password": password});
        let (i, v) = self.first(&[("/launcher/profile/login", Some(&body)), ("/launcher/v2/login", Some(&json!({"username": username})))])?;
        let ok = if i == 0 {
            v.as_str().is_some_and(|s| !s.is_empty() && s != "FAILED")
        } else {
            v.get("Response").and_then(|x| x.as_bool()).unwrap_or(false)
        };
        if !ok {
            return err(format!("登录失败：没有名为 {username} 的角色，或密码不对"));
        }
        if let Some(s) = v.as_str() {
            if i == 0 && s.len() >= 8 {
                return Ok(s.to_string());
            }
        }
        self.profiles()?
            .into_iter()
            .find(|p| p.username == username)
            .map(|p| p.profile_id)
            .ok_or_else(|| format!("登录后没有找到角色 {username}"))
    }

    pub fn register(&self, username: &str, password: &str, edition: &str) -> Res<()> {
        let body = json!({"username": username, "password": password, "edition": edition});
        let (i, v) = self.first(&[("/launcher/profile/register", Some(&body)), ("/launcher/v2/register", Some(&json!({"username": username, "edition": edition})))])?;
        let ok = if i == 0 {
            v.as_str().map_or(true, |s| s != "FAILED")
        } else {
            v.get("Response").and_then(|x| x.as_bool()).unwrap_or(false)
        };
        if ok {
            Ok(())
        } else {
            err(format!("创建失败：用户名 {username} 已被占用"))
        }
    }

    /// The editions a new profile can be (`Standard`, `Edge Of Darkness`…).
    pub fn editions(&self) -> Res<Vec<String>> {
        let (i, v) = self.first(&[("/launcher/server/connect", None), ("/launcher/v2/types", Some(&json!({})))])?;
        let list: Vec<String> = if i == 0 {
            v.get("editions").and_then(|e| e.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
        } else {
            v.get("Response").and_then(|r| r.as_object()).map(|o| o.keys().cloned().collect()).unwrap_or_default()
        };
        Ok(list)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct MiniProfile {
    pub username: String,
    pub nickname: String,
    pub side: String,
    pub level: i64,
    pub edition: String,
    pub profile_id: String,
}

impl MiniProfile {
    pub fn from_json(v: &Value) -> MiniProfile {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        MiniProfile {
            username: s("username"),
            nickname: s("nickname"),
            side: s("side"),
            level: v.get("currlvl").and_then(|x| x.as_i64()).unwrap_or(0),
            edition: s("edition"),
            profile_id: s("profileId"),
        }
    }
}

/// Whether something listens on 127.0.0.1:`port`.
pub fn port_open(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(&std::net::SocketAddr::from(([127, 0, 0, 1], port)), Duration::from_millis(300)).is_ok()
}
