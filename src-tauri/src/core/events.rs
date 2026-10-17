//! Events to the UI ("task", "game", "log", "login") and long-running tasks shown in the task dock.

use std::future::Future;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use tauri::Emitter;

use super::ctx;
use super::text::{text_of, Text};
use super::types::TaskInfo;

type Sink = Box<dyn Fn(&str, Value) + Send + Sync>;
static SINK: OnceLock<Sink> = OnceLock::new();

/// Headless runs (examples/tests) can receive events here instead of the UI.
pub fn set_sink(f: impl Fn(&str, Value) + Send + Sync + 'static) {
    let _ = SINK.set(Box::new(f));
}

/// Sends `payload` to the UI as `resin:<channel>`, the same events the Electron build sends.
pub fn emit<S: Serialize + Clone>(channel: &str, payload: S) {
    if let Some(app) = ctx::app() {
        let _ = app.emit(&format!("resin:{channel}"), payload);
    } else if let Some(sink) = SINK.get() {
        sink(channel, serde_json::to_value(payload).unwrap_or(Value::Null));
    }
}

/// A long-running job shown in the UI's task dock. Progress updates are throttled to ~10/s.
pub struct Task {
    info: Mutex<TaskInfo>,
    last: Mutex<Instant>,
}

impl Task {
    pub fn new(title: impl Into<Text>, instance_id: Option<String>) -> Arc<Task> {
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

    /// Runs `f` as a task, marking it done or failed.
    pub async fn run<T, F, Fut>(title: impl Into<Text>, instance_id: Option<String>, f: F) -> anyhow::Result<T>
    where
        F: FnOnce(Arc<Task>) -> Fut,
        Fut: Future<Output = anyhow::Result<T>>,
    {
        let task = Task::new(title, instance_id);
        match f(task.clone()).await {
            Ok(v) => {
                task.done();
                Ok(v)
            }
            Err(e) => {
                task.fail(&e);
                Err(e)
            }
        }
    }
}
