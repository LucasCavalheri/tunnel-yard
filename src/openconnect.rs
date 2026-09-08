//! Translate openfortivpn `.conf` into OpenConnect 9.21 arguments.
//! Password is never placed on the command line.

use base64::Engine;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{
    ClientConfig, ClientConnection, DigitallySignedStruct, RootCertStore, SignatureScheme,
};
use sha2::{Digest, Sha256};
use std::net::{IpAddr, Ipv4Addr, TcpStream};
use std::sync::Mutex;
use std::time::Duration;
use x509_parser::prelude::FromDer;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HealthCheck {
    pub health_host: Option<String>,
    pub health_port: Option<u16>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OpenConnectPlan {
    pub args: Vec<String>,
    pub password: String,
    pub host: String,
    pub port: u16,
    pub trusted_certs: Vec<String>,
    pub set_dns: bool,
    pub set_routes: bool,
    pub persistent: u32,
    pub health_host: Option<String>,
    pub health_port: Option<u16>,
    pub no_dtls: bool,
    pub legacy_tunnel: bool,
}

pub fn profile_health_check(raw: &str) -> Result<HealthCheck, String> {
    let host = comment_value(raw, "my-vpns-health-host");
    let port_text = comment_value(raw, "my-vpns-health-port");
    if host.is_none() && port_text.is_none() {
        return Ok(HealthCheck::default());
    }
    let host = host.unwrap_or_default();
    let port = port_text
        .and_then(|t| t.parse::<u16>().ok())
        .filter(|p| *p >= 1);
    let ipv4_ok = host.parse::<Ipv4Addr>().is_ok();
    if !ipv4_ok || port.is_none() {
        return Err("VPN health check requires an IPv4 address and a TCP port (1–65535).".into());
    }
    Ok(HealthCheck {
        health_host: Some(host),
        health_port: port,
    })
}

fn comment_value(raw: &str, key: &str) -> Option<String> {
    for line in raw.lines() {
        let t = line.trim();
        if !t.starts_with('#') {
            continue;
        }
        let rest = t.trim_start_matches('#').trim_start();
        let Some(eq) = rest.find('=') else { continue };
        let k = rest[..eq].trim();
        let current_key = key.replacen("my-vpns-", "tunnel-yard-", 1);
        if k.eq_ignore_ascii_case(key) || k.eq_ignore_ascii_case(&current_key) {
            let v = rest[eq + 1..].trim();
            if !v.is_empty() && !v.contains(char::is_whitespace) {
                return Some(v.to_string());
            }
            return Some(v.to_string());
        }
    }
    None
}

fn flag_comment(raw: &str, key: &str) -> bool {
    for line in raw.lines() {
        let t = line.trim();
        if !t.starts_with('#') {
            continue;
        }
        let rest = t.trim_start_matches('#').trim_start();
        let Some(eq) = rest.find('=') else { continue };
        let current_key = key.replacen("my-vpns-", "tunnel-yard-", 1);
        if !rest[..eq].trim().eq_ignore_ascii_case(key)
            && !rest[..eq].trim().eq_ignore_ascii_case(&current_key)
        {
            continue;
        }
        let v = rest[eq + 1..].trim().to_ascii_lowercase();
        if matches!(v.as_str(), "1" | "true" | "yes" | "on") {
            return true;
        }
    }
    false
}

pub fn profile_no_dtls(raw: &str) -> bool {
    flag_comment(raw, "my-vpns-no-dtls")
}

pub fn profile_legacy_tunnel(raw: &str) -> bool {
    flag_comment(raw, "my-vpns-legacy-tunnel")
}

pub fn conf_entries(raw: &str) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let text = line.trim();
        if text.is_empty() || text.starts_with('#') || text.starts_with(';') {
            continue;
        }
        match text.find('=') {
            Some(eq) if eq >= 1 => {
                out.push((
                    text[..eq].trim().to_lowercase(),
                    text[eq + 1..].trim().to_string(),
                ));
            }
            _ => return Err("Invalid .conf line (expected key = value).".into()),
        }
    }
    Ok(out)
}

fn bool_strict(value: Option<&str>, fallback: bool) -> Result<bool, String> {
    match value {
        None => Ok(fallback),
        Some(v) if matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on") => {
            Ok(true)
        }
        Some(v)
            if matches!(
                v.to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off"
            ) =>
        {
            Ok(false)
        }
        Some(_) => Err("Invalid boolean in VPN configuration.".into()),
    }
}

const WINDOWS_SUPPORTED: &[&str] = &[
    "host",
    "port",
    "username",
    "user",
    "password",
    "trusted-cert",
    "set-dns",
    "set-routes",
    "realm",
    "persistent",
    "ca-file",
    "user-cert",
    "user-key",
    "otp",
];

pub const MACOS_ALLOWED: &[&str] = &[
    "host",
    "port",
    "username",
    "user",
    "password",
    "trusted-cert",
    "set-dns",
    "set-routes",
    "realm",
    "persistent",
    "ca-file",
    "user-cert",
    "user-key",
    "otp",
    "otp-prompt",
    "otp-delay",
    "pppd-use-peerdns",
    "pppd-accept-remote",
    "half-internet-routes",
    "min-tls",
    "cipher-list",
    "seclevel",
    "saml-login",
];

pub fn macos_disallowed_keys(raw: &str) -> Result<Vec<String>, String> {
    let entries = conf_entries(raw)?;
    Ok(entries
        .into_iter()
        .filter(|(k, _)| !MACOS_ALLOWED.contains(&k.as_str()))
        .map(|(k, _)| k)
        .collect())
}

pub fn assert_macos_profile_safe(raw: &str) -> Result<(), String> {
    let invalid = macos_disallowed_keys(raw)?;
    if invalid.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Unsupported or unsafe .conf options on macOS: {}",
            invalid.join(", ")
        ))
    }
}

pub fn build_open_connect_plan(raw: &str) -> Result<OpenConnectPlan, String> {
    let entries = conf_entries(raw)?;
    let mut unknown = Vec::new();
    for (key, _) in &entries {
        if !WINDOWS_SUPPORTED.contains(&key.as_str()) && !unknown.contains(key) {
            unknown.push(key.clone());
        }
    }
    if !unknown.is_empty() {
        return Err(format!(
            "Unsupported .conf options on Windows: {}. The original profile has been preserved.",
            unknown.join(", ")
        ));
    }
    let mut values = std::collections::HashMap::new();
    for (k, v) in &entries {
        values.insert(k.clone(), v.clone());
    }
    let host = values.get("host").cloned().unwrap_or_default();
    if host.is_empty()
        || host.contains('\0')
        || host.starts_with('-')
        || host.chars().any(|c| " \t/\\?#@".contains(c))
    {
        return Err("Invalid VPN host.".into());
    }
    let port: u16 = values
        .get("port")
        .map(|s| s.as_str())
        .unwrap_or("443")
        .parse()
        .map_err(|_| "Invalid VPN port.".to_string())?;
    if port < 1 {
        return Err("Invalid VPN port.".into());
    }
    let persistent: i64 = values
        .get("persistent")
        .map(|s| s.as_str())
        .unwrap_or("0")
        .parse()
        .map_err(|_| "Invalid persistent interval.".to_string())?;
    if persistent < 0 {
        return Err("Invalid persistent interval.".into());
    }
    let mut args = vec![
        "--protocol=fortinet".into(),
        "--passwd-on-stdin".into(),
        "--disable-ipv6".into(),
        "--force-dpd=10".into(),
    ];
    let no_dtls = profile_no_dtls(raw);
    if no_dtls {
        args.push("--no-dtls".into());
    }
    if values.get("otp").is_none() {
        args.push("--non-inter".into());
    }
    if let Some(user) = values.get("username").or_else(|| values.get("user")) {
        args.push(format!("--user={user}"));
    }
    if let Some(realm) = values.get("realm") {
        args.push(format!("--usergroup={}", urlencoding::encode(realm)));
    }
    for (key, option) in [
        ("ca-file", "cafile"),
        ("user-cert", "certificate"),
        ("user-key", "sslkey"),
    ] {
        if let Some(v) = values.get(key) {
            args.push(format!("--{option}={v}"));
        }
    }
    args.push("--reconnect-timeout=1".into());
    let url_host = match host.parse::<IpAddr>() {
        Ok(IpAddr::V6(v6)) => format!("[{v6}]"),
        _ => host.clone(),
    };
    args.push(format!("https://{url_host}:{port}"));
    let trusted_certs: Vec<String> = entries
        .iter()
        .filter(|(k, _)| k == "trusted-cert")
        .map(|(_, v)| v.replace(':', "").to_lowercase())
        .collect();
    if trusted_certs
        .iter()
        .any(|pin| pin.len() != 64 || !pin.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return Err(
            "trusted-cert must be a complete SHA256 certificate fingerprint (64 hex characters)."
                .into(),
        );
    }
    let password = values.get("password").cloned().unwrap_or_default();
    let otp = values.get("otp");
    let mut stdin = password;
    stdin.push('\n');
    if let Some(otp) = otp {
        stdin.push_str(otp);
        stdin.push('\n');
    }
    let health = profile_health_check(raw)?;
    Ok(OpenConnectPlan {
        args,
        password: stdin,
        host,
        port,
        trusted_certs,
        set_dns: bool_strict(values.get("set-dns").map(String::as_str), true)?,
        set_routes: bool_strict(values.get("set-routes").map(String::as_str), true)?,
        persistent: persistent as u32,
        health_host: health.health_host,
        health_port: health.health_port,
        no_dtls,
        legacy_tunnel: profile_legacy_tunnel(raw),
    })
}

pub fn certificate_public_key_pin(
    raw_certificate: &[u8],
    trusted_certs: &[String],
) -> Result<String, String> {
    let digest = hex::encode(Sha256::digest(raw_certificate));
    if !trusted_certs.iter().any(|t| t == &digest) {
        return Err("VPN certificate does not match trusted-cert. Connection refused.".into());
    }
    let (_, cert) = x509_parser::certificate::X509Certificate::from_der(raw_certificate)
        .map_err(|e| e.to_string())?;
    let spki = cert.public_key().raw;
    let pin = Sha256::digest(spki);
    Ok(format!(
        "pin-sha256:{}",
        base64::engine::general_purpose::STANDARD.encode(pin)
    ))
}

fn ensure_crypto() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

#[derive(Debug)]
struct CaptureCert {
    cert: Mutex<Option<Vec<u8>>>,
}

impl ServerCertVerifier for CaptureCert {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        *self.cert.lock().unwrap() = Some(end_entity.as_ref().to_vec());
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

pub fn resolve_server_pin(plan: &OpenConnectPlan) -> Result<Option<String>, String> {
    if plan.trusted_certs.is_empty() {
        return Ok(None);
    }
    ensure_crypto();
    let capture = std::sync::Arc::new(CaptureCert {
        cert: Mutex::new(None),
    });
    let config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(capture.clone())
        .with_no_client_auth();
    let server_name =
        ServerName::try_from(plan.host.clone()).map_err(|_| "Invalid VPN host.".to_string())?;
    let mut conn = ClientConnection::new(std::sync::Arc::new(config), server_name)
        .map_err(|e| e.to_string())?;
    let mut stream =
        TcpStream::connect((plan.host.as_str(), plan.port)).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    conn.complete_io(&mut stream).map_err(|e| {
        if e.kind() == std::io::ErrorKind::TimedOut {
            "VPN certificate probe timed out.".to_string()
        } else {
            e.to_string()
        }
    })?;
    let raw = capture
        .cert
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "VPN certificate probe timed out.".to_string())?;
    certificate_public_key_pin(&raw, &plan.trusted_certs).map(Some)
}

/// Unused import kept out of the public API; RootCertStore referenced so rustls config typechecks.
#[allow(dead_code)]
fn _unused_roots() -> RootCertStore {
    RootCertStore::empty()
}
