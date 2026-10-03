//! 平台能力门控：IPC 层统一的「暂不支持」判定与文案。
//!
//! 前端的 `app/src/config/platforms.ts` 只负责隐藏入口，真正的拒绝必须发生在后端：
//! 任何绕过 UI 的调用都要拿到明确错误，而不是空结果或假装成功。

use hotdownloader_core::platforms::Platform;

/// 脚本音源不支持该操作（沿用既有文案，避免前端提示回归）。
pub const SCRIPT_UNSUPPORTED: &str = "脚本音源不支持该操作";

/// 平台的中文完整名，用于错误与提示文案。
pub fn platform_name(platform: Platform) -> &'static str {
    match platform {
        Platform::QqMusic => "QQ 音乐",
        Platform::Kuwo => "酷我音乐",
        Platform::Kugou => "酷狗音乐",
        Platform::Netease => "网易云音乐",
        Platform::Bilibili => "哔哩哔哩",
        Platform::Migu => "咪咕音乐",
        Platform::Script(_) => "自定义音源",
    }
}

/// 统一的「该平台暂不支持某能力」错误。
///
/// `capability` 用界面上的说法（如「专辑」「歌手」「歌词」），拼出的文案直接展示给用户。
pub fn unsupported(platform: Platform, capability: &str) -> String {
    format!("{}暂不支持{capability}", platform_name(platform))
}
