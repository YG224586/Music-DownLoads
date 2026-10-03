//! 各平台「是否已配置账号」的进程级状态。
//!
//! 音质档位是在**搜索结果**里声明的（各平台 `parser::build_qualities`），而档位能不能
//! 真拿到取决于账号权益，且匿名行为各不相同（实测见各平台模块文档）：
//! - 网易云匿名请求无损会被**静默降级**成 320k mp3，所以匿名不声明 `2000.flac`；
//! - 咪咕匿名**忽略 `toneFlag`**，任何档位都给 128k mp3，所以匿名只声明 `PQ.mp3`；
//! - 酷狗档位由搜索响应里是否存在对应 `hash` 决定，与账号无关。
//!
//! core 不持有设置文件，因此由上层（Tauri）在启动读设置后与保存平台账号后调用
//! [`set_platform_account`] 写入状态，parser 侧读取。只保存布尔，**不接触 Cookie 内容**；
//! 未被写入的平台一律按「未配置账号」处理，退回到上面的匿名能力边界。

use std::sync::atomic::{AtomicU64, Ordering};

use super::Platform;

/// 位表：每个平台一位。
const KUGOU: u64 = 1 << 0;
const NETEASE: u64 = 1 << 1;
const MIGU: u64 = 1 << 2;

static CONFIGURED: AtomicU64 = AtomicU64::new(0);

fn bit_of(platform: Platform) -> u64 {
    match platform {
        Platform::Kugou => KUGOU,
        Platform::Netease => NETEASE,
        Platform::Migu => MIGU,
        // 其它平台（QQ 音乐走凭据文件、哔哩哔哩/酷我/脚本音源用不到账号）与账号状态无关。
        _ => 0,
    }
}

/// 记录某平台是否已配置账号；与账号状态无关的平台是空操作。
pub fn set_platform_account(platform: Platform, configured: bool) {
    let bit = bit_of(platform);
    if bit == 0 {
        return;
    }
    if configured {
        CONFIGURED.fetch_or(bit, Ordering::Relaxed);
    } else {
        CONFIGURED.fetch_and(!bit, Ordering::Relaxed);
    }
}

/// 该平台是否已配置账号。
pub fn platform_account_configured(platform: Platform) -> bool {
    let bit = bit_of(platform);
    bit != 0 && CONFIGURED.load(Ordering::Relaxed) & bit != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_state_is_tracked_per_platform() {
        // 只翻动酷狗的位：parser 的单元测试会读网易云/咪咕的位，测试并行跑时不能互相干扰。
        assert!(!platform_account_configured(Platform::Kugou));

        set_platform_account(Platform::Kugou, true);
        assert!(platform_account_configured(Platform::Kugou));
        // 各平台互不影响。
        assert!(!platform_account_configured(Platform::Netease));
        assert!(!platform_account_configured(Platform::Migu));

        set_platform_account(Platform::Kugou, false);
        assert!(!platform_account_configured(Platform::Kugou));
    }

    #[test]
    fn platforms_without_account_state_stay_anonymous() {
        set_platform_account(Platform::Kuwo, true);
        assert!(!platform_account_configured(Platform::Kuwo));

        let script: Platform = "script:7".parse().expect("script platform");
        set_platform_account(script, true);
        assert!(!platform_account_configured(script));
    }
}
