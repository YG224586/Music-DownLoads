//! 下载后的歌词、封面获取与独立歌词命名，供普通文件和 SAF 收尾流程共用。

use std::path::Path;

use crate::download::context::TaskContext;
use crate::platforms::lyric::LyricData;
use crate::platforms::Platform;

/// 一次获取的歌词与封面会同时供音频标签及独立 LRC 使用。
pub struct PostprocessAssets {
    pub lyric: Option<LyricData>,
    pub cover_bytes: Option<Vec<u8>>,
}

/// 独立歌词沿用音频文件名的 stem；普通路径与 Android SAF 使用同一命名规则。
/// 文件名无法转换为 UTF-8 时沿用 `unknown.lrc`，保持已有下载结果格式。
pub fn lrc_file_name(audio_path: &str) -> String {
    let stem = Path::new(audio_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown");
    format!("{stem}.lrc")
}

/// 根据任务配置准备收尾数据。平台接口失败只影响对应的可选内容。
pub async fn prepare_assets(
    context: &TaskContext,
    write_metadata: bool,
    download_lrc: bool,
) -> PostprocessAssets {
    let lyric = if write_metadata || download_lrc {
        let result = match context.platform {
            Platform::QqMusic => {
                crate::platforms::qqmusic::lyrics::get_lyric_by_id(context.song_id).await
            }
            Platform::Kuwo => {
                crate::platforms::kuwo::lyrics::get_lyric_by_id(context.song_id).await
            }
            // 自定义音源脚本没有歌词接口，只影响可选的歌词写入。
            Platform::Script(_) => Err("自定义音源暂不支持歌词".to_string()),
        };
        match result {
            Ok(lyric) => Some(lyric),
            Err(error) => {
                log::warn!("获取歌词失败: {error}");
                None
            }
        }
    } else {
        None
    };

    // 酷我搜索结果可能缺封面。只有需要写标签时才补取，避免无用的网络请求。
    let mut cover_url = context.song_info.cover_url.clone();
    if write_metadata && cover_url.is_empty() && matches!(context.platform, Platform::Kuwo) {
        match crate::platforms::kuwo::cover::fetch_cover(context.song_id).await {
            Ok(url) => cover_url = url,
            Err(error) => log::warn!("任务 {} 获取酷我封面失败: {error}", context.task_id),
        }
    }

    let cover_bytes = if write_metadata && !cover_url.is_empty() {
        match crate::platforms::CLIENT.get(&cover_url).send().await {
            Ok(response) if response.status().is_success() => {
                response.bytes().await.ok().map(|bytes| bytes.to_vec())
            }
            _ => None,
        }
    } else {
        None
    };

    PostprocessAssets { lyric, cover_bytes }
}
