//! The encrypted tunnel between two launchers. Each game connection gets its own Noise session
//! (Noise_NNpsk0_25519_ChaChaPoly_SHA256): fresh X25519 keys on both sides for forward secrecy, and the code's secret
//! as the pre-shared key, so only someone holding the code can finish the handshake. Whatever carries the bytes (the
//! internet, ResinCraft Relay) sees ciphertext. Frames are a big-endian u16 length and a Noise message; counters are
//! implicit, so an altered, replayed, dropped or reordered frame fails to decrypt and ends the connection.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use snow::{Builder, StatelessTransportState};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const PATTERN: &str = "Noise_NNpsk0_25519_ChaChaPoly_SHA256";
const PROLOGUE: &[u8] = b"ResinCraft friend tunnel 1";
const TAG: usize = 16;
const MAX_FRAME: usize = 65535;
const CHUNK: usize = MAX_FRAME - TAG;
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

fn key(secret: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"resincraft-friend-key");
    h.update(secret);
    h.finalize().into()
}

fn builder<'a>(psk: &'a [u8; 32]) -> Result<Builder<'a>> {
    Ok(Builder::new(PATTERN.parse()?).prologue(PROLOGUE)?.psk(0, psk)?)
}

async fn read_frame<R: AsyncRead + Unpin>(r: &mut R, buf: &mut Vec<u8>) -> std::io::Result<()> {
    let len = r.read_u16().await? as usize;
    buf.resize(len, 0);
    r.read_exact(buf).await?;
    Ok(())
}

async fn write_frame<W: AsyncWrite + Unpin>(w: &mut W, frame: &[u8]) -> std::io::Result<()> {
    let mut out = Vec::with_capacity(frame.len() + 2);
    out.extend_from_slice(&(frame.len() as u16).to_be_bytes());
    out.extend_from_slice(frame);
    w.write_all(&out).await?;
    w.flush().await
}

/// The two directions' keys after a handshake.
pub struct Keys {
    state: Arc<StatelessTransportState>,
}

/// The friend's side of the handshake.
pub async fn connect<S: AsyncRead + AsyncWrite + Unpin>(io: &mut S, secret: &[u8]) -> Result<Keys> {
    let psk = key(secret);
    let mut hs = builder(&psk)?.build_initiator()?;
    let mut msg = [0u8; 128];
    let n = hs.write_message(&[], &mut msg)?;
    write_frame(io, &msg[..n]).await?;
    let mut frame = Vec::new();
    read_frame(io, &mut frame).await?;
    let mut payload = [0u8; 128];
    hs.read_message(&frame, &mut payload)?;
    Ok(Keys { state: Arc::new(hs.into_stateless_transport_mode()?) })
}

/// The host's side: fails when the other side doesn't hold the secret.
pub async fn accept<S: AsyncRead + AsyncWrite + Unpin>(io: &mut S, secret: &[u8]) -> Result<Keys> {
    let psk = key(secret);
    let mut hs = builder(&psk)?.build_responder()?;
    let mut frame = Vec::new();
    read_frame(io, &mut frame).await?;
    if frame.len() > 128 {
        return Err(anyhow!("handshake too long"));
    }
    let mut payload = [0u8; 128];
    hs.read_message(&frame, &mut payload)?;
    let mut msg = [0u8; 128];
    let n = hs.write_message(&[], &mut msg)?;
    write_frame(io, &msg[..n]).await?;
    Ok(Keys { state: Arc::new(hs.into_stateless_transport_mode()?) })
}

async fn seal<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(plain: &mut R, sealed: &mut W, keys: &StatelessTransportState) -> Result<()> {
    let mut buf = vec![0u8; CHUNK.min(32 * 1024)];
    let mut out = vec![0u8; MAX_FRAME];
    let mut nonce = 0u64;
    loop {
        let n = plain.read(&mut buf).await?;
        if n == 0 {
            return Ok(());
        }
        let m = keys.write_message(nonce, &buf[..n], &mut out)?;
        nonce += 1;
        write_frame(sealed, &out[..m]).await?;
    }
}

async fn open<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(sealed: &mut R, plain: &mut W, keys: &StatelessTransportState) -> Result<()> {
    let mut frame = Vec::with_capacity(32 * 1024);
    let mut out = vec![0u8; MAX_FRAME];
    let mut nonce = 0u64;
    loop {
        match read_frame(sealed, &mut frame).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(e.into()),
        }
        let n = keys.read_message(nonce, &frame, &mut out)?;
        nonce += 1;
        plain.write_all(&out[..n]).await?;
        plain.flush().await?;
    }
}

/// Carries `plain` (the game's connection) over `sealed` until either side closes or a frame fails to decrypt.
pub async fn pump<A, B>(sealed: B, plain: A, keys: Keys) -> Result<()>
where
    A: AsyncRead + AsyncWrite + Unpin,
    B: AsyncRead + AsyncWrite + Unpin,
{
    let (mut sealed_r, mut sealed_w) = tokio::io::split(sealed);
    let (mut plain_r, mut plain_w) = tokio::io::split(plain);
    let state = keys.state.clone();
    let ended = tokio::select! {
        r = seal(&mut plain_r, &mut sealed_w, &state) => r,
        r = open(&mut sealed_r, &mut plain_w, &state) => r,
    };
    let _ = sealed_w.shutdown().await;
    let _ = plain_w.shutdown().await;
    ended
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    async fn pair(a: &[u8], b: &[u8]) -> (Result<Keys>, Result<Keys>, tokio::io::DuplexStream, tokio::io::DuplexStream) {
        let (mut l, mut r) = duplex(4096);
        let (ka, kb) = tokio::join!(
            async { tokio::time::timeout(Duration::from_secs(2), connect(&mut l, a)).await.unwrap_or_else(|_| Err(anyhow!("timed out"))) },
            async { tokio::time::timeout(Duration::from_secs(2), accept(&mut r, b)).await.unwrap_or_else(|_| Err(anyhow!("timed out"))) }
        );
        (ka, kb, l, r)
    }

    /// One side's game traffic arrives at the other, both ways, and the wire never holds the plain bytes.
    #[tokio::test]
    async fn carries_data_both_ways_encrypted() {
        let (mut ka, kb, _l, _r) = pair(b"secret", b"secret").await;
        let ka = ka.take_keys();
        let kb = kb.unwrap();
        let (mut game, game_side) = duplex(1 << 20);
        let (mut server, server_side) = duplex(1 << 20);

        // Everything crossing the wire goes through this tap.
        let (tap_l, mut tap_l_far) = duplex(1 << 20);
        let (tap_r, mut tap_r_far) = duplex(1 << 20);
        let seen = Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
        let tap_seen = seen.clone();
        tokio::spawn(async move {
            let mut a = [0u8; 8192];
            let mut b = [0u8; 8192];
            loop {
                tokio::select! {
                    Ok(n) = tap_l_far.read(&mut a) => { if n == 0 { break } tap_seen.lock().unwrap().extend_from_slice(&a[..n]); tap_r_far.write_all(&a[..n]).await.unwrap(); }
                    Ok(n) = tap_r_far.read(&mut b) => { if n == 0 { break } tap_seen.lock().unwrap().extend_from_slice(&b[..n]); tap_l_far.write_all(&b[..n]).await.unwrap(); }
                }
            }
        });
        tokio::spawn(pump(tap_l, game_side, ka));
        tokio::spawn(pump(tap_r, server_side, kb));

        let big: Vec<u8> = b"minecraft-login-packet ".iter().copied().cycle().take(200_000).collect();
        game.write_all(&big).await.unwrap();
        let mut got = vec![0u8; big.len()];
        server.read_exact(&mut got).await.unwrap();
        assert_eq!(got, big);
        server.write_all(b"chunk data").await.unwrap();
        let mut back = [0u8; 10];
        game.read_exact(&mut back).await.unwrap();
        assert_eq!(&back, b"chunk data");
        let wire = seen.lock().unwrap();
        assert!(wire.len() > big.len());
        assert!(!wire.windows(9).any(|w| w == b"minecraft"), "plain text on the wire");
    }

    #[tokio::test]
    async fn a_wrong_secret_never_finishes() {
        let (ka, kb, _l, _r) = pair(b"secret", b"other").await;
        assert!(kb.is_err(), "the host refuses");
        assert!(ka.is_err(), "the friend gets nothing back");
    }

    /// A flipped bit, a replayed frame and a dropped frame each end the connection.
    #[tokio::test]
    async fn tampering_ends_the_connection() {
        for attack in ["flip", "replay", "drop"] {
            let (mut ka, kb, _l, _r) = pair(b"s", b"s").await;
            let ka = ka.take_keys();
            let kb = kb.unwrap();
            // Two sealed frames.
            let mut wire = Vec::new();
            let mut plain: &[u8] = b"one";
            let _ = seal(&mut plain, &mut wire, &ka.state).await;
            let first = wire.clone();
            let mut second = Vec::new();
            let mut out = vec![0u8; 256];
            let m = ka.state.write_message(1, b"two", &mut out).unwrap();
            write_frame(&mut second, &out[..m]).await.unwrap();

            let bytes = match attack {
                "flip" => {
                    let mut f = first.clone();
                    *f.last_mut().unwrap() ^= 1;
                    f
                }
                "replay" => [first.clone(), first.clone()].concat(),
                _ => second.clone(),
            };
            let mut got = Vec::new();
            let r = open(&mut bytes.as_slice(), &mut got, &kb.state).await;
            assert!(r.is_err(), "{attack} went through");
        }
    }

    trait TakeKeys {
        fn take_keys(&mut self) -> Keys;
    }
    impl TakeKeys for Result<Keys> {
        fn take_keys(&mut self) -> Keys {
            std::mem::replace(self, Err(anyhow!("taken"))).unwrap()
        }
    }
}
