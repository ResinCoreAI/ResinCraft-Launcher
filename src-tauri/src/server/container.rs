//! Servers in containers (the panel's `containers on`): Docker or Podman runs the same program as always, with the
//! server's folder (and its Java, read-only) mounted at the same paths, so nothing else in the engine has to know. The
//! console still goes through stdin and stdout of `docker run -i`.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use tokio::process::Command;

/// The system a container starts from; the server's own files bring everything else.
const IMAGE: &str = "debian:12-slim";
/// Java from Alpine's packages needs Alpine's libc.
const ALPINE_IMAGE: &str = "alpine:3.20";
/// A container can't run away with the machine's processes.
const PIDS_LIMIT: &str = "4096";
/// What Java uses besides the heap, on top of a quarter of the heap.
const OVERHEAD_MB: u32 = 256;
/// How long a leftover server gets to save before it's stopped for good.
const LEFTOVER_SAVE_SECS: &str = "30";

/// One server's container.
#[derive(Clone)]
pub struct Container {
    engine: &'static str,
    name: String,
}

pub struct Run<'a> {
    /// The server's folder, mounted at `workdir`.
    pub dir: &'a Path,
    pub workdir: &'a Path,
    /// A Java folder to mount read-only.
    pub java_home: Option<&'a Path>,
    pub program: &'a str,
    pub args: &'a [String],
    /// TCP and UDP: Minecraft, query and anything on the same port.
    pub port: u16,
    /// UDP only: Bedrock's IPv6 port and Geyser.
    pub udp: &'a [u16],
    pub memory_mb: u32,
}

/// Docker when this user can reach it, else Podman.
async fn engine() -> Option<&'static str> {
    for engine in ["docker", "podman"] {
        let answered = Command::new(engine).arg("info").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().await;
        if answered.is_ok_and(|s| s.success()) {
            return Some(engine);
        }
    }
    None
}

impl Container {
    /// Finds the engine and stops a container an earlier run left behind (the panel was killed, say), letting it save.
    pub async fn prepare(id: &str) -> Result<Container> {
        let Some(engine) = engine().await else {
            bail!("Containers are on, but this user can't reach Docker or Podman. Install one, or turn them off: resincraft-panel containers off");
        };
        let container = Container { engine, name: format!("resincraft-{id}") };
        let _ = Command::new(engine).args(["stop", "-t", LEFTOVER_SAVE_SECS, &container.name]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().await;
        container.remove().await;
        Ok(container)
    }

    pub fn engine(&self) -> &'static str {
        self.engine
    }

    /// Stops it at once: killing `docker run` alone would leave the server running.
    pub async fn remove(&self) {
        let _ = Command::new(self.engine).args(["rm", "-f", &self.name]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().await;
    }

    /// The server's process as this machine sees it, for the RAM display.
    pub async fn pid(&self) -> Option<u32> {
        let out = Command::new(self.engine).args(["inspect", "-f", "{{.State.Pid}}", &self.name]).output().await.ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse().ok().filter(|pid| *pid != 0)
    }

    /// Everything after the engine's name.
    pub fn run_args(&self, run: &Run) -> Vec<String> {
        let image = if Path::new("/etc/alpine-release").exists() { ALPINE_IMAGE } else { IMAGE };
        run_args(self.engine, &self.name, image, owner(run.dir), run)
    }
}

fn run_args(engine: &str, name: &str, image: &str, owner: Option<(u32, u32)>, run: &Run) -> Vec<String> {
    let path = |p: &Path| p.to_string_lossy().into_owned();
    let mut args: Vec<String> = ["run", "-i", "--rm", "--name", name].map(String::from).into();
    // Its files stay the panel user's. Rootless Podman gets there by mapping that user in.
    if engine == "podman" {
        args.push("--userns=keep-id".into());
    } else if let Some((uid, gid)) = owner {
        args.extend(["--user".to_string(), format!("{uid}:{gid}")]);
    }
    let memory = run.memory_mb + run.memory_mb / 4 + OVERHEAD_MB;
    args.extend(["--memory".to_string(), format!("{memory}m"), "--pids-limit".to_string(), PIDS_LIMIT.to_string()]);
    // Only the server's folder and /tmp can be written, and nothing in it can gain rights.
    args.extend(["--read-only", "--tmpfs", "/tmp", "--cap-drop", "ALL", "--security-opt", "no-new-privileges"].map(String::from));
    args.extend(["-e".to_string(), format!("HOME={}", path(run.workdir))]);
    args.extend(["-p".to_string(), format!("{0}:{0}/tcp", run.port), "-p".to_string(), format!("{0}:{0}/udp", run.port)]);
    for port in run.udp {
        args.extend(["-p".to_string(), format!("{port}:{port}/udp")]);
    }
    args.extend(["-v".to_string(), format!("{}:{}", path(run.dir), path(run.workdir))]);
    if let Some(java) = run.java_home {
        args.extend(["-v".to_string(), format!("{0}:{0}:ro", path(java))]);
    }
    args.extend(["-w".to_string(), path(run.workdir), image.to_string(), run.program.to_string()]);
    args.extend(run.args.iter().cloned());
    args
}

#[cfg(unix)]
fn owner(dir: &Path) -> Option<(u32, u32)> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(dir).ok().map(|m| (m.uid(), m.gid()))
}

#[cfg(not(unix))]
fn owner(_: &Path) -> Option<(u32, u32)> {
    None
}

/// The real Java program (a system `java` is usually a chain of links) and its folder, to mount.
pub fn java_home(program: &str) -> Option<(String, PathBuf)> {
    let real = std::fs::canonicalize(program).ok()?;
    let home = real.parent()?.parent()?.to_path_buf();
    Some((real.to_string_lossy().into_owned(), home))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample<'a>(args: &'a [String], udp: &'a [u16]) -> Run<'a> {
        Run { dir: Path::new("/data/s1"), workdir: Path::new("/data/s1"), java_home: Some(Path::new("/data/java/21")), program: "/data/java/21/bin/java", args, port: 25565, udp, memory_mb: 4096 }
    }

    #[test]
    fn docker_runs_as_the_folder_owner_with_ports_and_limits() {
        let args = vec!["-jar".to_string(), "server.jar".to_string()];
        let line = run_args("docker", "resincraft-s1", IMAGE, Some((998, 997)), &sample(&args, &[19132])).join(" ");
        assert!(line.starts_with("run -i --rm --name resincraft-s1 --user 998:997 --memory 5376m"), "{line}");
        assert!(line.contains("-p 25565:25565/tcp -p 25565:25565/udp -p 19132:19132/udp"), "{line}");
        assert!(line.contains("-v /data/s1:/data/s1 -v /data/java/21:/data/java/21:ro"), "{line}");
        assert!(line.ends_with("-w /data/s1 debian:12-slim /data/java/21/bin/java -jar server.jar"), "{line}");
    }

    #[test]
    fn podman_maps_the_user_in() {
        let line = run_args("podman", "resincraft-s1", IMAGE, Some((998, 997)), &sample(&[], &[])).join(" ");
        assert!(line.contains("--userns=keep-id"), "{line}");
        assert!(!line.contains("--user "), "{line}");
    }
}
