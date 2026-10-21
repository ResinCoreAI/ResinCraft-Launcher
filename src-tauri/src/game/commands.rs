//! The player's own commands around a launch and extra environment variables for the game, as Prism Launcher has
//! them: a pre-launch command (a failure stops the launch), a wrapper the game runs through, and a post-exit command.
//! Commands run through the OS shell (cmd on Windows, sh elsewhere) in the instance's game folder and see
//! `$INST_NAME`, `$INST_ID`, `$INST_DIR`, `$INST_MC_DIR`, `$INST_JAVA`, `$INST_JAVA_ARGS` and the player's variables.

use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use tokio::io::AsyncReadExt;

use crate::core::paths;
use crate::core::settings::Settings;
use crate::core::types::{EnvVar, Instance, LaunchCommands, LogLine};
use crate::{bail_tr, err_tr};

use super::java::hidden_command;
use super::launch::LogParser;
use super::mojang::plain;

/// How long a pre-launch or post-exit command may run before it's stopped.
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// Output lines kept for the error shown when the pre-launch command fails.
const TAIL_LINES: usize = 15;

/// A name every shell can use: letters, digits and `_`, not starting with a digit.
pub fn valid_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Names compare without case on Windows, like its environment does.
fn env_key(name: &str) -> String {
    if cfg!(windows) {
        name.to_ascii_uppercase()
    } else {
        name.to_string()
    }
}

/// Checks a list before it's saved: usable names, none of ResinCraft's own `INST_*`, no repeats, no NUL in values.
pub fn check_env(vars: &[EnvVar]) -> Result<()> {
    let mut seen = HashSet::new();
    for v in vars {
        if !valid_env_name(&v.name) {
            bail_tr!("commands.badEnvName", "\"{name}\" can't be an environment variable name. Use letters, digits and _, and don't start with a digit.", name = v.name);
        }
        if v.name.to_ascii_uppercase().starts_with("INST_") {
            bail_tr!("commands.reservedEnvName", "{name} is set by ResinCraft itself. Pick a name that doesn't start with INST_.", name = v.name);
        }
        if !seen.insert(env_key(&v.name)) {
            bail_tr!("commands.duplicateEnvName", "{name} is in the list twice.", name = v.name);
        }
        if v.value.contains('\0') {
            bail_tr!("commands.badEnvValue", "The value of {name} can't contain a NUL character.", name = v.name);
        }
    }
    Ok(())
}

/// Whether a variable probably holds a password, key or token, so the log shows `<hidden>` instead of its value.
pub fn looks_secret(name: &str, value: &str) -> bool {
    const WORDS: [&str; 10] = ["TOKEN", "SECRET", "PASS", "PWD", "KEY", "AUTH", "CREDENTIAL", "COOKIE", "SESSION", "PRIVATE"];
    let upper = name.to_ascii_uppercase();
    if WORDS.iter().any(|w| upper.contains(w)) {
        return true;
    }
    // A long run of mostly letters and digits with no spaces or path separators reads like a generated key.
    let alnum = value.chars().filter(|c| c.is_ascii_alphanumeric()).count();
    value.len() >= 24
        && alnum * 10 >= value.len() * 9
        && !value.contains(char::is_whitespace)
        && !value.contains(['/', '\\'])
        && value.chars().any(|c| c.is_ascii_digit())
        && value.chars().any(|c| c.is_ascii_alphabetic())
}

/// The game's variables: Settings' first, then the instance's (a name in both takes the instance's value).
/// Names that can't be used (a hand-edited file) are left out.
pub fn merged_env(settings: &Settings, inst: &Instance) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for v in settings.env.iter().chain(&inst.env) {
        if !valid_env_name(&v.name) || v.value.contains('\0') {
            continue;
        }
        match out.iter_mut().find(|(n, _)| env_key(n) == env_key(&v.name)) {
            Some(e) => e.1 = v.value.clone(),
            None => out.push((v.name.clone(), v.value.clone())),
        }
    }
    out
}

/// "A=1, API_TOKEN=<hidden>" for the game log.
pub fn shown_env(env: &[(String, String)]) -> String {
    env.iter().map(|(n, v)| if looks_secret(n, v) { format!("{n}=<hidden>") } else { format!("{n}={v}") }).collect::<Vec<_>>().join(", ")
}

/// The instance's own commands when it has them, else the ones in Settings.
pub fn effective(settings: &Settings, inst: &Instance) -> LaunchCommands {
    inst.commands.clone().unwrap_or_else(|| settings.commands.clone())
}

/// What commands and the game get on top of the inherited environment: the player's variables, then `INST_*`.
pub fn command_vars(inst: &Instance, java: &str, java_args: &[String], env: &[(String, String)]) -> Vec<(String, String)> {
    let mut vars = env.to_vec();
    vars.extend(
        [
            ("INST_NAME", inst.name.clone()),
            ("INST_ID", inst.id.clone()),
            ("INST_DIR", plain(&paths::instance_dir(&inst.id))),
            ("INST_MC_DIR", plain(&paths::game_dir(&inst.id))),
            ("INST_JAVA", java.to_string()),
            ("INST_JAVA_ARGS", java_args.join(" ")),
        ]
        .map(|(k, v)| (k.to_string(), v)),
    );
    vars
}

/// Replaces `$NAME` and `${NAME}` with the value of `vars`' NAME; anything else stays as written. cmd has no `$`
/// variables, so on Windows this is how `$INST_DIR` works (as cmd's own `%INST_DIR%` it's plain text replacement).
pub fn substitute(text: &str, vars: &[(String, String)]) -> String {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)").unwrap());
    re.replace_all(text, |c: &regex::Captures| {
        let name = c.get(1).or_else(|| c.get(2)).map(|m| m.as_str()).unwrap_or_default();
        vars.iter().rev().find(|(n, _)| env_key(n) == env_key(name)).map(|(_, v)| v.clone()).unwrap_or_else(|| c[0].to_string())
    })
    .into_owned()
}

/// `script` as a shell command: `cmd /D /S /C "…"` on Windows, `sh -c …` (in its own process group) elsewhere.
fn shell(script: &str, vars: &[(String, String)]) -> tokio::process::Command {
    #[cfg(windows)]
    let cmd = {
        let mut cmd = hidden_command("cmd.exe");
        // Passed as written: cmd doesn't parse quotes the way Rust's argument escaping expects. /S takes off the
        // outer quotes and keeps the rest.
        cmd.raw_arg(format!("/D /S /C \"{}\"", substitute(script, vars)));
        cmd
    };
    #[cfg(not(windows))]
    let cmd = {
        let mut cmd = hidden_command("sh");
        cmd.arg("-c").arg(script).process_group(0);
        cmd
    };
    let mut cmd = cmd;
    cmd.envs(vars.iter().map(|(k, v)| (k, v)));
    cmd
}

/// The program and arguments that run Java through `wrapper`. `{java}` marks where the Java command goes; without
/// it the Java command goes last (`prime-run` → `prime-run java …`).
pub fn wrapped(wrapper: &str, java: &str, args: &[String], vars: &[(String, String)]) -> (String, Vec<String>) {
    #[cfg(not(windows))]
    {
        // sh expands the variables and the wrapper's own quoting; the Java command comes in as "$@", untouched.
        let script = if wrapper.contains("{java}") { wrapper.replace("{java}", "\"$@\"") } else { format!("{} \"$@\"", wrapper.trim()) };
        let _ = vars;
        let mut out = vec!["-c".to_string(), script, "resincraft".to_string(), java.to_string()];
        out.extend(args.iter().cloned());
        ("sh".to_string(), out)
    }
    #[cfg(windows)]
    {
        // Started directly: through cmd, a modded classpath would pass cmd's 8191-character line limit.
        let mut words: Vec<String> = Vec::new();
        let mut placed = false;
        for w in super::launch::split_args(wrapper) {
            if w == "{java}" {
                words.push(java.to_string());
                words.extend(args.iter().cloned());
                placed = true;
            } else {
                words.push(substitute(&w, vars));
            }
        }
        if !placed {
            words.push(java.to_string());
            words.extend(args.iter().cloned());
        }
        let program = words.remove(0);
        (program, words)
    }
}

/// Stops a process and everything it started: `taskkill /T` on Windows, its process group elsewhere.
pub fn kill_tree(pid: u32) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(super::java::CREATE_NO_WINDOW)
            .status();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("kill").args(["-TERM", "--", &format!("-{pid}")]).stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
}

/// Kills a command's process tree when dropped before the command ended (cancelled from the dock, or timed out).
struct TreeGuard(Option<u32>);

impl Drop for TreeGuard {
    fn drop(&mut self) {
        if let Some(pid) = self.0 {
            kill_tree(pid);
        }
    }
}

/// How a command ended: its exit code (none when a signal stopped it) and its last lines of output.
pub struct Finished {
    pub code: Option<i32>,
    pub tail: Vec<String>,
}

/// Runs `script` through the shell in `dir`, sending each line it prints to `push` (stderr as warnings).
/// `None` when it ran longer than `limit`: it's stopped, with whatever it started.
pub async fn run(script: &str, vars: &[(String, String)], dir: &Path, limit: Duration, push: impl Fn(LogLine) + Clone + Send + Sync + 'static) -> Result<Option<Finished>> {
    let mut child = shell(script, vars)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| err_tr!("commands.startFailed", "Couldn't run the command: {error}", error = e.to_string()))?;
    let mut guard = TreeGuard(child.id());
    let tail: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
    let mut readers = Vec::new();
    let pipes: [(Option<Box<dyn tokio::io::AsyncRead + Unpin + Send>>, bool); 2] =
        [(child.stdout.take().map(|o| Box::new(o) as _), false), (child.stderr.take().map(|e| Box::new(e) as _), true)];
    for (pipe, is_err) in pipes {
        let Some(mut pipe) = pipe else { continue };
        let (push, tail) = (push.clone(), tail.clone());
        readers.push(tokio::spawn(async move {
            let mut parser = LogParser::new(move |l: LogLine| {
                {
                    let mut t = tail.lock().unwrap();
                    t.push_back(l.text.clone());
                    if t.len() > TAIL_LINES {
                        t.pop_front();
                    }
                }
                push(LogLine { level: if is_err && l.level == "info" { "warn" } else { l.level }, ..l });
            });
            let mut buf = vec![0u8; 8 * 1024];
            while let Ok(n) = pipe.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                parser.feed_bytes(&buf[..n]);
            }
            parser.feed("\n");
        }));
    }
    let status = tokio::time::timeout(limit, child.wait()).await;
    let code = match status {
        Ok(s) => {
            guard.0 = None;
            Some(s?.code())
        }
        Err(_) => None,
    };
    drop(guard);
    // Something the command left running in the background can hold the pipes open; don't wait on it for long.
    for r in readers {
        let _ = tokio::time::timeout(Duration::from_secs(2), r).await;
    }
    let tail = tail.lock().unwrap().iter().cloned().collect();
    Ok(code.map(|code| Finished { code, tail }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var(name: &str, value: &str) -> EnvVar {
        EnvVar { name: name.into(), value: value.into() }
    }

    #[test]
    fn env_names_are_checked() {
        assert!(valid_env_name("MESA_GL_VERSION_OVERRIDE") && valid_env_name("_x1"));
        assert!(!valid_env_name("") && !valid_env_name("1ABC") && !valid_env_name("A-B") && !valid_env_name("A B") && !valid_env_name("Ä"));
        assert!(check_env(&[var("A", "1"), var("B", "")]).is_ok());
        assert!(check_env(&[var("A=B", "1")]).is_err());
        assert!(check_env(&[var("INST_NAME", "x")]).is_err(), "INST_* belongs to the launcher");
        assert!(check_env(&[var("A", "1"), var("A", "2")]).is_err());
        assert!(check_env(&[var("A", "a\0b")]).is_err());
    }

    #[test]
    fn instance_variables_win() {
        let settings = Settings { env: vec![var("A", "global"), var("B", "global")], ..Settings::default() };
        let inst: Instance = serde_json::from_value(serde_json::json!({
            "id": "i", "name": "I", "mcVersion": "1.21.1", "loader": "vanilla", "color": "#fff", "createdAt": 0,
            "env": [{ "name": "B", "value": "own" }, { "name": "C", "value": "own" }, { "name": "bad name", "value": "x" }]
        }))
        .unwrap();
        let env = merged_env(&settings, &inst);
        assert_eq!(env, [("A".into(), "global".into()), ("B".into(), "own".into()), ("C".into(), "own".into())]);
    }

    #[test]
    fn secrets_stay_out_of_the_log() {
        let env = vec![
            ("MY_API_TOKEN".to_string(), "abc".to_string()),
            ("DB_PASSWORD".to_string(), "hunter2".to_string()),
            ("SOMETHING".to_string(), "sk4f9a8b7c6d5e4f3a2b1c0d".to_string()),
            ("JAVA_TOOL_OPTIONS".to_string(), "-Dfile.encoding=UTF-8".to_string()),
            ("LIBGL_DIR".to_string(), "/usr/lib/x86_64-linux-gnu/dri".to_string()),
        ];
        let shown = shown_env(&env);
        assert!(!shown.contains("abc") && !shown.contains("hunter2") && !shown.contains("sk4f9a8b"), "{shown}");
        assert!(shown.contains("MY_API_TOKEN=<hidden>"));
        assert!(shown.contains("JAVA_TOOL_OPTIONS=-Dfile.encoding=UTF-8") && shown.contains("LIBGL_DIR=/usr/lib/x86_64-linux-gnu/dri"));
    }

    #[test]
    fn variables_are_substituted() {
        let vars = vec![("INST_NAME".to_string(), "My Pack".to_string()), ("GPU".to_string(), "1".to_string())];
        assert_eq!(substitute("echo $INST_NAME ${GPU}x $UNKNOWN $5", &vars), "echo My Pack 1x $UNKNOWN $5");
    }

    #[test]
    fn wrappers_place_the_java_command() {
        let args = vec!["-Xmx2G".to_string(), "net.minecraft.client.main.Main".to_string()];
        let (program, rest) = wrapped("prime-run", "/jdk/bin/java", &args, &[]);
        let (with_slot, slot_rest) = wrapped("obs-gamecapture {java} --after", "/jdk/bin/java", &args, &[]);
        #[cfg(not(windows))]
        {
            assert_eq!(program, "sh");
            assert_eq!(rest[1], "prime-run \"$@\"");
            assert_eq!(rest[2..], ["resincraft", "/jdk/bin/java", "-Xmx2G", "net.minecraft.client.main.Main"]);
            assert_eq!((with_slot.as_str(), slot_rest[1].as_str()), ("sh", "obs-gamecapture \"$@\" --after"));
        }
        #[cfg(windows)]
        {
            assert_eq!(program, "prime-run");
            assert_eq!(rest, ["/jdk/bin/java", "-Xmx2G", "net.minecraft.client.main.Main"]);
            assert_eq!(with_slot, "obs-gamecapture");
            assert_eq!(slot_rest, ["/jdk/bin/java", "-Xmx2G", "net.minecraft.client.main.Main", "--after"]);
        }
    }

    #[tokio::test]
    async fn commands_run_through_the_shell() {
        let dir = std::env::temp_dir();
        let lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let push = {
            let lines = lines.clone();
            move |l: LogLine| lines.lock().unwrap().push(l.text)
        };
        let vars = vec![("RESIN_TEST_VAR".to_string(), "hello".to_string())];
        let done = run("echo got $RESIN_TEST_VAR&& exit 3", &vars, &dir, Duration::from_secs(30), push.clone()).await.unwrap().unwrap();
        assert_eq!(done.code, Some(3));
        assert!(lines.lock().unwrap().iter().any(|l| l.trim() == "got hello"), "{:?}", lines.lock().unwrap());
        // Too slow: stopped, with nothing to report.
        let slow = if cfg!(windows) { "ping -n 30 127.0.0.1 >NUL" } else { "sleep 30" };
        let started = std::time::Instant::now();
        assert!(run(slow, &[], &dir, Duration::from_millis(500), push).await.unwrap().is_none());
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}
