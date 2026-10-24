//! Events to the UI ("task", "game", "log", "login") and long-running tasks shown in the task dock. Most tasks
//! can be cancelled from the dock (see `Task::run`).

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use tauri::Emitter;
use tokio_util::sync::CancellationToken;

use super::ctx;
use super::text::{text_of, Text};
use super::types::TaskInfo;

/// Running tasks that can be cancelled, by task id.
static CANCELS: Mutex<Option<HashMap<String, CancellationToken>>> = Mutex::new(None);

/// Stops a running task. Its work is dropped right away: downloads stop (a `.part` file stays for the next try to
/// overwrite), installer processes are killed, and the task reports "cancelled". False when no such task runs.
pub fn cancel(id: &str) -> bool {
    let token = CANCELS.lock().unwrap().as_mut().and_then(|m| m.remove(id));
    token.map(|t| t.cancel()).is_some()
}

/// How many cancellable tasks (downloads, installs, imports…) are running.
pub fn running_tasks() -> usize {
    CANCELS.lock().unwrap().as_ref().map_or(0, |m| m.len())
}

/// The error a cancelled task's work ends with; callers clean up as for any other error.
pub fn cancelled_error() -> anyhow::Error {
    crate::err_tr!("task.cancelled", "Cancelled")
}

pub fn is_cancelled(err: &anyhow::Error) -> bool {
    text_of(err).is_some_and(|t| t.key == "task.cancelled")
}

/// Ends a task's registration; if its work was dropped before it reported back (a task it belongs to was
/// cancelled), it reports "cancelled" too.
struct Finish {
    task: Arc<Task>,
    done: bool,
}

impl Drop for Finish {
    fn drop(&mut self) {
        let id = self.task.info.lock().unwrap().id.clone();
        if let Some(m) = CANCELS.lock().unwrap().as_mut() {
            m.remove(&id);
        }
        if !self.done {
            self.task.cancelled();
        }
    }
}

type Sink = Box<dyn Fn(&str, Value) + Send + Sync>;
static SINK: OnceLock<Sink> = OnceLock::new();

/// Headless runs (examples/tests) can receive events here instead of the UI.
pub fn set_sink(f: impl Fn(&str, Value) + Send + Sync + 'static) {
    let _ = SINK.set(Box::new(f));
}

/// Sends `payload` to the UI as `resin:<channel>`.
pub fn emit<S: Serialize + Clone>(channel: &str, payload: S) {
    if let Some(app) = ctx::app() {
        let _ = app.emit(&format!("resin:{channel}"), payload);
    } else if let Some(sink) = SINK.get() {
        sink(channel, serde_json::to_value(payload).unwrap_or(Value::Null));
    }
}

/// Tells the UI a list changed on disk ("instances" or "servers"): an install made a new entry, or one went. The UI
/// fetches the list again.
pub fn list_changed(what: &'static str) {
    emit("listChanged", what);
}

/// A long-running job shown in the UI's task dock. Progress updates are throttled to ~10/s.
pub struct Task {
    info: Mutex<TaskInfo>,
    last: Mutex<Instant>,
}

impl Task {
    pub fn new(title: impl Into<Text>, instance_id: Option<String>) -> Arc<Task> {
        Task::create(title, instance_id, false)
    }

    fn create(title: impl Into<Text>, instance_id: Option<String>, cancellable: bool) -> Arc<Task> {
        let title = title.into();
        let info = TaskInfo {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.text.clone(),
            detail: None,
            progress: None,
            state: "running",
            error: None,
            instance_id,
            title_text: (!title.is_plain()).then_some(title),
            detail_text: None,
            error_text: None,
            cancellable,
        };
        emit("task", info.clone());
        Arc::new(Task { info: Mutex::new(info), last: Mutex::new(Instant::now() - Duration::from_secs(1)) })
    }

    pub fn update(&self, detail: impl Into<Text>, progress: Option<f64>) {
        let detail = detail.into();
        let snapshot = {
            let mut info = self.info.lock().unwrap();
            info.detail = Some(detail.text.clone());
            info.detail_text = (!detail.is_plain()).then_some(detail);
            info.progress = progress;
            let mut last = self.last.lock().unwrap();
            if last.elapsed() < Duration::from_millis(100) && progress != Some(1.0) {
                return;
            }
            *last = Instant::now();
            info.clone()
        };
        emit("task", snapshot);
    }

    /// Ties the task to the instance (or server) it is making, once that has an id: the UI shows the task's progress
    /// on its card, which appears at once (see `list_changed`).
    pub fn set_instance(&self, id: &str) {
        let snapshot = {
            let mut info = self.info.lock().unwrap();
            info.instance_id = Some(id.to_string());
            info.clone()
        };
        emit("task", snapshot);
    }

    /// A progress callback for `net::download_all`: "Label (done/total)".
    pub fn counter(self: &Arc<Self>, label: impl Into<Text>) -> impl Fn(usize, usize) + Send + Sync + 'static {
        let task = self.clone();
        let label = label.into();
        move |done, total| {
            let p = if total == 0 { 1.0 } else { done as f64 / total as f64 };
            task.update(crate::tr!("task.progress", "{label} ({done}/{total})", label = label, done = done, total = total), Some(p));
        }
    }

    pub fn done(&self) {
        let snapshot = {
            let mut info = self.info.lock().unwrap();
            info.state = "done";
            info.progress = Some(1.0);
            info.clone()
        };
        emit("task", snapshot);
    }

    pub fn cancelled(&self) {
        let snapshot = {
            let mut info = self.info.lock().unwrap();
            if info.state != "running" {
                return;
            }
            info.state = "cancelled";
            info.clone()
        };
        emit("task", snapshot);
    }

    pub fn fail(&self, err: &anyhow::Error) {
        let snapshot = {
            let mut info = self.info.lock().unwrap();
            info.state = "error";
            info.error = Some(err.to_string());
            info.error_text = text_of(err).cloned();
            info.clone()
        };
        emit("task", snapshot);
    }

    /// Runs `f` as a task, marking it done, failed or cancelled. The dock offers Cancel: that drops `f`'s work and
    /// returns `cancelled_error()`, so whatever the caller does after a failure (removing a half-made server…) runs.
    pub async fn run<T, F, Fut>(title: impl Into<Text>, instance_id: Option<String>, f: F) -> anyhow::Result<T>
    where
        F: FnOnce(Arc<Task>) -> Fut,
        Fut: Future<Output = anyhow::Result<T>>,
    {
        Task::run_with(title, instance_id, true, f).await
    }

    /// Like `run`, without Cancel: for work that can't stop halfway (a blocking thread writing a zip).
    pub async fn run_to_end<T, F, Fut>(title: impl Into<Text>, instance_id: Option<String>, f: F) -> anyhow::Result<T>
    where
        F: FnOnce(Arc<Task>) -> Fut,
        Fut: Future<Output = anyhow::Result<T>>,
    {
        Task::run_with(title, instance_id, false, f).await
    }

    async fn run_with<T, F, Fut>(title: impl Into<Text>, instance_id: Option<String>, cancellable: bool, f: F) -> anyhow::Result<T>
    where
        F: FnOnce(Arc<Task>) -> Fut,
        Fut: Future<Output = anyhow::Result<T>>,
    {
        let task = Task::create(title, instance_id, cancellable);
        let token = CancellationToken::new();
        if cancellable {
            let id = task.info.lock().unwrap().id.clone();
            CANCELS.lock().unwrap().get_or_insert_with(HashMap::new).insert(id, token.clone());
        }
        let mut finish = Finish { task: task.clone(), done: false };
        let work = f(task.clone());
        tokio::pin!(work);
        let result = tokio::select! {
            r = &mut work => r,
            _ = token.cancelled() => Err(cancelled_error()),
        };
        finish.done = true;
        drop(finish);
        match result {
            Ok(v) => {
                task.done();
                Ok(v)
            }
            // Cancelled here, or a task this one waited for was.
            Err(e) if is_cancelled(&e) => {
                task.cancelled();
                Err(e)
            }
            Err(e) => {
                task.fail(&e);
                Err(e)
            }
        }
    }
}
