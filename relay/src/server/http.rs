//! A deliberately tiny HTTP/1.1 server side (one request per connection), shared by the local admin port and
//! the admin dashboard. Enough for small JSON APIs and a few embedded files; nothing else is accepted.

use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const MAX_HEAD: usize = 16 * 1024;
const MAX_BODY: usize = 64 * 1024;

pub(crate) struct Request {
    pub method: String,
    /// Without the query string.
    pub path: String,
    pub query: String,
    headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    /// A header by (case-insensitive) name.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    pub fn cookie(&self, name: &str) -> Option<&str> {
        self.header("cookie")?.split(';').find_map(|c| {
            let (k, v) = c.trim().split_once('=')?;
            (k == name).then_some(v)
        })
    }

    pub fn bearer(&self) -> Option<&str> {
        self.header("authorization")?.strip_prefix("Bearer ").map(str::trim)
    }

    /// The body as JSON (an empty object when there's none, Null when it isn't JSON).
    pub fn json(&self) -> Value {
        if self.body.is_empty() {
            json!({})
        } else {
            serde_json::from_slice(&self.body).unwrap_or(Value::Null)
        }
    }

    /// A query parameter, percent-decoded.
    pub fn param(&self, name: &str) -> Option<String> {
        self.query.split('&').find_map(|kv| {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            (k == name).then(|| decode(v))
        })
    }
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'+', _) => out.push(b' '),
            (b'%', Some(b)) => {
                out.push(b);
                i += 2;
            }
            (b, _) => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Reads one request (head up to 16 KB, body up to 64 KB).
pub(crate) async fn read_request<S: AsyncRead + Unpin>(sock: &mut S) -> std::io::Result<Request> {
    let bad = |m: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, m.to_string());
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if buf.len() > MAX_HEAD {
            return Err(bad("request head too large"));
        }
        let n = sock.read(&mut chunk).await?;
        if n == 0 {
            return Err(bad("closed"));
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split(' ');
    let method = first.next().unwrap_or_default().to_string();
    let target = first.next().unwrap_or_default();
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let headers: Vec<(String, String)> =
        lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
    let length = match headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("content-length")) {
        Some((_, v)) => v.parse::<usize>().map_err(|_| bad("bad content-length"))?,
        None => 0,
    };
    if length > MAX_BODY {
        return Err(bad("body too large"));
    }
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < length {
        let n = sock.read(&mut chunk).await?;
        if n == 0 {
            return Err(bad("closed"));
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(length);
    Ok(Request { method, path: path.to_string(), query: query.to_string(), headers, body })
}

pub(crate) struct Response {
    pub status: &'static str,
    pub content_type: &'static str,
    pub body: Vec<u8>,
    pub headers: Vec<(String, String)>,
}

impl Response {
    pub fn new(status: &'static str, content_type: &'static str, body: impl Into<Vec<u8>>) -> Response {
        Response { status, content_type, body: body.into(), headers: Vec::new() }
    }

    pub fn json(status: &'static str, v: Value) -> Response {
        Response::new(status, "application/json", v.to_string())
    }

    pub fn ok(v: Value) -> Response {
        Response::json("200 OK", v)
    }

    pub fn error(status: &'static str, error: &str) -> Response {
        Response::json(status, json!({ "error": error }))
    }

    pub fn header(mut self, name: &str, value: impl Into<String>) -> Response {
        self.headers.push((name.to_string(), value.into()));
        self
    }
}

pub(crate) async fn write_response<S: AsyncWrite + Unpin>(sock: &mut S, r: &Response) -> std::io::Result<()> {
    let mut head = format!("HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n", r.status, r.content_type, r.body.len());
    for (k, v) in &r.headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    sock.write_all(head.as_bytes()).await?;
    sock.write_all(&r.body).await?;
    sock.flush().await?;
    let _ = sock.shutdown().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn parses_requests() {
        let raw = b"POST /api/x?q=a%20b+c&n=1 HTTP/1.1\r\nHost: h\r\nCookie: a=1; rc_session=abc\r\nContent-Length: 7\r\nX-CSRF-Token: t\r\n\r\n{\"k\":1}";
        let mut src: &[u8] = raw;
        let r = read_request(&mut src).await.unwrap();
        assert_eq!((r.method.as_str(), r.path.as_str()), ("POST", "/api/x"));
        assert_eq!(r.param("q").as_deref(), Some("a b c"));
        assert_eq!(r.param("n").as_deref(), Some("1"));
        assert_eq!(r.cookie("rc_session"), Some("abc"));
        assert_eq!(r.header("x-csrf-token"), Some("t"));
        assert_eq!(r.json()["k"], 1);
        let mut big: &[u8] = b"POST / HTTP/1.1\r\nContent-Length: 99999999\r\n\r\n";
        assert!(read_request(&mut big).await.is_err());
        assert_eq!(decode("%zz%41"), "%zzA");
    }
}
