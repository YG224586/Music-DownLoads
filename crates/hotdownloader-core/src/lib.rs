//! 可由 Tauri 客户端及独立 Rust 服务复用的任务业务核心。
//! 原生窗口、Android SAF、系统通知和 IPC 适配保留在各自的运行时项目中。

pub mod adapters;
pub mod download;
pub mod platforms;
pub mod settings;
pub mod task;
