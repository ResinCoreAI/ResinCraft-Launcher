//! TLS on the agent port with a certificate from files (Let's Encrypt's fullchain.pem / privkey.pem). The
//! files are checked for changes every few minutes, so renewals are picked up without a restart.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;

use anyhow::{anyhow, Context, Result};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;

use super::config::TlsConfig;

pub struct CertFiles {
    cert: PathBuf,
    key: PathBuf,
    current: RwLock<Arc<CertifiedKey>>,
    stamp: Mutex<(Option<SystemTime>, Option<SystemTime>)>,
}

impl std::fmt::Debug for CertFiles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CertFiles").field("cert", &self.cert).field("key", &self.key).finish()
    }
}

fn modified(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn load(cert: &Path, key_path: &Path) -> Result<CertifiedKey> {
    let chain: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(cert)
        .with_context(|| format!("couldn't read {}", cert.display()))?
        .collect::<Result<_, _>>()
        .with_context(|| format!("{} isn't a PEM certificate chain", cert.display()))?;
    if chain.is_empty() {
        return Err(anyhow!("{} has no certificates", cert.display()));
    }
    let key = PrivateKeyDer::from_pem_file(key_path).with_context(|| format!("couldn't read the private key {}", key_path.display()))?;
    let signer = rustls::crypto::ring::sign::any_supported_type(&key).map_err(|e| anyhow!("unsupported private key: {e}"))?;
    let pair = CertifiedKey::new(chain, signer);
    // Also catches reading the files halfway through a renewal (new key, old certificate): retried later.
    pair.keys_match().map_err(|e| anyhow!("{} and {} don't belong together: {e}", cert.display(), key_path.display()))?;
    Ok(pair)
}

impl CertFiles {
    pub fn open(cfg: &TlsConfig) -> Result<Arc<CertFiles>> {
        let key = load(&cfg.cert, &cfg.key)?;
        Ok(Arc::new(CertFiles {
            stamp: Mutex::new((modified(&cfg.cert), modified(&cfg.key))),
            cert: cfg.cert.clone(),
            key: cfg.key.clone(),
            current: RwLock::new(Arc::new(key)),
        }))
    }

    /// Re-reads the files when they changed; returns true when a new certificate is in use.
    pub fn reload_if_changed(&self) -> Result<bool> {
        let now = (modified(&self.cert), modified(&self.key));
        if *self.stamp.lock().unwrap() == now {
            return Ok(false);
        }
        let key = load(&self.cert, &self.key)?;
        *self.current.write().unwrap() = Arc::new(key);
        *self.stamp.lock().unwrap() = now;
        Ok(true)
    }
}

impl ResolvesServerCert for CertFiles {
    fn resolve(&self, _hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        Some(self.current.read().unwrap().clone())
    }
}

pub fn acceptor(files: Arc<CertFiles>) -> Result<tokio_rustls::TlsAcceptor> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut cfg = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_cert_resolver(files);
    cfg.alpn_protocols = vec![crate::proto::ALPN.to_vec()];
    Ok(tokio_rustls::TlsAcceptor::from(Arc::new(cfg)))
}
