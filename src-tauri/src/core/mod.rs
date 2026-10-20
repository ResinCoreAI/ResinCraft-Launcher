//! Shared foundation used by every feature group: context, events/tasks, HTTP + downloads,
//! paths, settings, instances, OS integration and the IPC router.

pub mod art;
pub mod ctx;
pub mod events;
pub mod fsutil;
pub mod instances;
pub mod net;
pub mod paths;
pub mod router;
pub mod settings;
pub mod shell;
pub mod storage;
pub mod text;
pub mod types;

pub use router::Args;
