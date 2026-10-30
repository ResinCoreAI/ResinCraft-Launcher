//! Background tasks: Tauri's runtime in the app, plain Tokio in the panel.

use std::future::Future;

#[cfg(feature = "desktop")]
pub use tauri::async_runtime::JoinHandle;
#[cfg(not(feature = "desktop"))]
pub use tokio::task::JoinHandle;

pub fn spawn<F>(task: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    #[cfg(feature = "desktop")]
    return tauri::async_runtime::spawn(task);
    #[cfg(not(feature = "desktop"))]
    tokio::spawn(task)
}

pub fn is_finished(h: &JoinHandle<()>) -> bool {
    #[cfg(feature = "desktop")]
    return h.inner().is_finished();
    #[cfg(not(feature = "desktop"))]
    h.is_finished()
}
