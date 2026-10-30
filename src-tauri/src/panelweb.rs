//! The panel's own website: HTTPS on the host's domain, signed in with a panel password (no Microsoft account). Requests
//! go through the same checks as the web console (see `remote::owner_call`). Certificates come from Let's Encrypt by
//! themselves, or from files the host brings (a Cloudflare origin certificate, say).
//!
//! web.json in the data folder holds the settings and the password's hash; the password itself is shown once.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::{ConnectInfo, DefaultBodyLimit, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::StreamExt;
use rustls_acme::caches::DirCache;
use rustls_acme::AcmeConfig;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;
use tokio::task::AbortHandle;

use crate::core::fsutil::read_json;

const COOKIE: &str = "rcpanel";
const SESSION: Duration = Duration::from_secs(12 * 60 * 60);
/// Wrong passwords from one address before it has to wait.
const MAX_TRIES: u32 = 5;
const LOCKOUT: Duration = Duration::from_secs(15 * 60);
/// Argon2 is slow and takes memory on purpose: a flood of logins mustn't take the machine with it.
const HASHING_AT_ONCE: usize = 2;
const BODY_LIMIT: usize = 1024 * 1024;
const LETS_ENCRYPT_PORT: u16 = 443;

#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WebConfig {
    pub domain: String,
    pub email: String,
    pub port: u16,
    /// Certificate files of the host's own; Let's Encrypt without them.
    pub cert: String,
    pub key: String,
    /// Another ACME server and its CA file (advanced; not a command).
    pub acme_url: String,
    pub acme_ca: String,
    pub password_hash: String,
}

fn config_file() -> PathBuf {
    crate::core::paths::root().join("web.json")
}

pub async fn config() -> WebConfig {
    read_json(&config_file()).await.unwrap_or_default()
}

/// Only the panel's user may read it.
async fn save(config: &WebConfig) -> Result<()> {
    let path = config_file();
    tokio::fs::write(&path, serde_json::to_vec_pretty(config)?).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).await?;
    }
    Ok(())
}

fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default().hash_password(password.as_bytes(), &salt).map_err(|e| anyhow!("{e}"))?.to_string())
}

fn password_matches(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| Argon2::default().verify_password(password.as_bytes(), &h).is_ok())
}

fn random_token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// A new password. Returns it: this is the only time it can be seen.
pub async fn new_password() -> Result<String> {
    let password = random_token();
    let mut config = config().await;
    config.password_hash = tokio::task::spawn_blocking({
        let password = password.clone();
        move || hash_password(&password)
    })
    .await??;
    save(&config).await?;
    Ok(password)
}

/// Saves where the site lives. Returns a first password when it has none yet.
pub async fn enable(domain: &str, email: &str, port: Option<u16>, cert: &str, key: &str) -> Result<Option<String>> {
    let domain = crate::server::domain::normalize(domain)?;
    if cert.is_empty() != key.is_empty() {
        bail!("A certificate needs both files: --cert and --key");
    }
    if cert.is_empty() && !email.contains('@') {
        bail!("Let's Encrypt needs an email address to reach you about the certificate");
    }
    if cert.is_empty() && port.is_some_and(|p| p != LETS_ENCRYPT_PORT) {
        bail!("Let's Encrypt only checks port {LETS_ENCRYPT_PORT}. Other ports need your own certificate: --cert and --key");
    }
    let mut config = config().await;
    config.domain = domain;
    config.email = email.to_string();
    config.port = port.unwrap_or(LETS_ENCRYPT_PORT);
    config.cert = cert.to_string();
    config.key = key.to_string();
    save(&config).await?;
    Ok(if config.password_hash.is_empty() { Some(new_password().await?) } else { None })
}

pub async fn disable() -> Result<()> {
    let mut config = config().await;
    config.domain.clear();
    save(&config).await
}

// ---------- The running site ----------

static RUNNING: Mutex<Option<AbortHandle>> = Mutex::new(None);

/// (Re)starts the site from web.json; signs everyone out, since it may be a new password.
pub async fn apply() {
    if let Some(old) = RUNNING.lock().unwrap().take() {
        old.abort();
    }
    sessions().lock().unwrap().clear();
    let config = config().await;
    if config.domain.is_empty() {
        return;
    }
    let task = tokio::spawn(async move {
        let (domain, port) = (config.domain.clone(), config.port);
        match serve(config).await {
            Ok(()) => {}
            Err(e) => eprintln!("The website on {domain}:{port} stopped: {e:#}"),
        }
    });
    *RUNNING.lock().unwrap() = Some(task.abort_handle());
}

async fn serve(config: WebConfig) -> Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let app = site(&config.domain).into_make_service_with_connect_info::<SocketAddr>();
    println!("The website is on https://{}{}", config.domain, if config.port == 443 { String::new() } else { format!(":{}", config.port) });
    if !config.cert.is_empty() {
        let tls = axum_server::tls_rustls::RustlsConfig::from_pem_file(&config.cert, &config.key).await.context("Couldn't read the certificate files")?;
        axum_server::bind_rustls(addr, tls).serve(app).await.context("Couldn't open the port")?;
        return Ok(());
    }
    let acme = match config.acme_ca.as_str() {
        "" => AcmeConfig::new([&config.domain]),
        ca => AcmeConfig::new_with_client_config([&config.domain], client_trusting(Path::new(ca))?),
    };
    // The certificate's private key lives here.
    let certificates = crate::core::paths::root().join("certificates");
    tokio::fs::create_dir_all(&certificates).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&certificates, std::fs::Permissions::from_mode(0o700)).await?;
    }
    let acme = acme.contact_push(format!("mailto:{}", config.email)).cache(DirCache::new(certificates));
    let acme = if config.acme_url.is_empty() { acme.directory_lets_encrypt(true) } else { acme.directory(&config.acme_url) };
    let mut state = acme.state();
    let acceptor = state.axum_acceptor(state.default_rustls_config());
    let renewing = tokio::spawn(async move {
        while let Some(event) = state.next().await {
            match event {
                Ok(ok) => println!("Certificate: {ok:?}"),
                Err(e) => eprintln!("Certificate problem: {e:?}"),
            }
        }
    });
    let served = axum_server::bind(addr).acceptor(acceptor).serve(app).await.context("Couldn't open the port");
    renewing.abort();
    served
}

/// For an ACME server with its own CA.
fn client_trusting(ca: &Path) -> Result<Arc<rustls::ClientConfig>> {
    let mut roots = rustls::RootCertStore::empty();
    let pem = std::fs::read(ca).context("Couldn't read the ACME CA file")?;
    for cert in rustls_pemfile::certs(&mut pem.as_slice()) {
        roots.add(cert?)?;
    }
    Ok(Arc::new(rustls::ClientConfig::builder().with_root_certificates(roots).with_no_client_auth()))
}

#[derive(Clone)]
struct Site {
    domain: Arc<str>,
    hashing: Arc<Semaphore>,
}

fn site(domain: &str) -> Router {
    let site = Site { domain: domain.into(), hashing: Arc::new(Semaphore::new(HASHING_AT_ONCE)) };
    Router::new()
        .route("/", get(|| page("text/html; charset=utf-8", include_str!("panelweb/index.html"))))
        .route("/app.js", get(|| page("text/javascript; charset=utf-8", include_str!("panelweb/app.js"))))
        .route("/app.css", get(|| page("text/css; charset=utf-8", include_str!("panelweb/app.css"))))
        .route("/api/session", get(session))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/call", post(call))
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(middleware::from_fn_with_state(site.clone(), guard))
        .with_state(site)
}

async fn page(kind: &'static str, body: &'static str) -> impl IntoResponse {
    ([(header::CONTENT_TYPE, kind)], body)
}

fn host_of(request: &Request) -> Option<String> {
    let host = request.uri().host().map(str::to_string).or_else(|| request.headers().get(header::HOST)?.to_str().ok().map(str::to_string))?;
    Some(host.split(':').next().unwrap_or_default().to_ascii_lowercase())
}

/// Every response: only this domain answers (another site can't reach the panel through its own name in a browser),
/// a request that changes things must come from the panel's page, and the browser is told to stay on HTTPS.
async fn guard(State(site): State<Site>, request: Request, next: Next) -> Response {
    if host_of(&request).as_deref() != Some(&*site.domain) {
        return (StatusCode::MISDIRECTED_REQUEST, "Not this site").into_response();
    }
    if request.method() == axum::http::Method::POST && !request.headers().contains_key("x-resincraft-panel") {
        return (StatusCode::FORBIDDEN, "Not from the panel's page").into_response();
    }
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    for (name, value) in [
        ("strict-transport-security", "max-age=31536000"),
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "no-referrer"),
        ("cache-control", "no-store"),
        ("content-security-policy", "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"),
    ] {
        headers.insert(name, HeaderValue::from_static(value));
    }
    response
}

// ---------- Signing in ----------

/// Open sessions, by the hash of their token, with when they end.
fn sessions() -> &'static Mutex<HashMap<String, Instant>> {
    static SESSIONS: std::sync::OnceLock<Mutex<HashMap<String, Instant>>> = std::sync::OnceLock::new();
    SESSIONS.get_or_init(Default::default)
}

/// Wrong passwords by address: how many, and when they're forgotten.
fn failures() -> &'static Mutex<HashMap<IpAddr, (u32, Instant)>> {
    static FAILURES: std::sync::OnceLock<Mutex<HashMap<IpAddr, (u32, Instant)>>> = std::sync::OnceLock::new();
    FAILURES.get_or_init(Default::default)
}

fn token_key(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

fn token_of(headers: &HeaderMap) -> Option<String> {
    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;
    cookies.split(';').find_map(|c| c.trim().strip_prefix(COOKIE)?.strip_prefix('=').map(str::to_string))
}

fn signed_in(headers: &HeaderMap) -> bool {
    let Some(token) = token_of(headers) else { return false };
    let now = Instant::now();
    let mut open = sessions().lock().unwrap();
    open.retain(|_, ends| *ends > now);
    open.contains_key(&token_key(&token))
}

fn locked_out(ip: IpAddr) -> bool {
    let now = Instant::now();
    let mut failed = failures().lock().unwrap();
    failed.retain(|_, (_, forgotten)| *forgotten > now);
    failed.get(&ip).is_some_and(|(count, _)| *count >= MAX_TRIES)
}

fn note_failure(ip: IpAddr) {
    let mut failed = failures().lock().unwrap();
    let entry = failed.entry(ip).or_insert((0, Instant::now()));
    entry.0 += 1;
    entry.1 = Instant::now() + LOCKOUT;
}

async fn session(headers: HeaderMap) -> Json<Value> {
    Json(json!({ "signedIn": signed_in(&headers) }))
}

#[derive(Deserialize)]
struct Login {
    password: String,
}

fn refused(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

async fn login(State(site): State<Site>, ConnectInfo(from): ConnectInfo<SocketAddr>, Json(body): Json<Login>) -> Response {
    let ip = from.ip();
    if locked_out(ip) {
        return refused(StatusCode::TOO_MANY_REQUESTS, "Too many wrong passwords. Try again in 15 minutes.");
    }
    let hash = config().await.password_hash;
    let Ok(_slot) = site.hashing.acquire().await else { return refused(StatusCode::SERVICE_UNAVAILABLE, "Try again") };
    let right = tokio::task::spawn_blocking(move || password_matches(&body.password, &hash)).await.unwrap_or(false);
    if !right {
        note_failure(ip);
        eprintln!("Website: wrong password from {ip}");
        return refused(StatusCode::UNAUTHORIZED, "Wrong password");
    }
    failures().lock().unwrap().remove(&ip);
    println!("Website: signed in from {ip}");
    let token = random_token() + &random_token();
    sessions().lock().unwrap().insert(token_key(&token), Instant::now() + SESSION);
    let cookie = format!("{COOKIE}={token}; Path=/; Max-Age={}; HttpOnly; Secure; SameSite=Strict", SESSION.as_secs());
    ([(header::SET_COOKIE, cookie)], Json(json!({ "signedIn": true }))).into_response()
}

async fn logout(headers: HeaderMap) -> Response {
    if let Some(token) = token_of(&headers) {
        sessions().lock().unwrap().remove(&token_key(&token));
    }
    let cookie = format!("{COOKIE}=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Strict");
    ([(header::SET_COOKIE, cookie)], Json(json!({ "signedIn": false }))).into_response()
}

#[derive(Deserialize)]
struct Call {
    method: String,
    #[serde(default)]
    params: Value,
}

async fn call(headers: HeaderMap, Json(body): Json<Call>) -> Response {
    if !signed_in(&headers) {
        return refused(StatusCode::UNAUTHORIZED, "Sign in first");
    }
    match crate::remote::owner_call(&body.method, body.params).await {
        Ok(value) => Json(json!({ "ok": value })).into_response(),
        Err(e) => refused(StatusCode::BAD_REQUEST, &format!("{e:#}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hash_checks_out() {
        let hash = hash_password("one").unwrap();
        assert!(password_matches("one", &hash));
        assert!(!password_matches("two", &hash));
        assert!(!password_matches("one", ""));
    }

    #[test]
    fn cookie_is_found_among_others() {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, HeaderValue::from_static("a=1; rcpanel=abc; rcpanelx=no"));
        assert_eq!(token_of(&headers).as_deref(), Some("abc"));
    }

    #[test]
    fn five_wrong_passwords_lock_an_address_out() {
        let ip: IpAddr = "203.0.113.9".parse().unwrap();
        for _ in 0..MAX_TRIES {
            assert!(!locked_out(ip));
            note_failure(ip);
        }
        assert!(locked_out(ip));
    }
}
