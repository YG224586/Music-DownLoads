//! 下载后的歌词与封面获取，供普通文件和 SAF 收尾流程共用。

use crate::download::context::TaskContext;
use crate::platforms::lyric::LyricData;
use crate::platforms::Platform;

/// 一次获取的歌词与封面会同时供音频标签及独立 LRC 使用。
pub struct PostprocessAssets {
    pub lyric: Option<LyricData>,
    pub cover_bytes: Option<Vec<u8>>,
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
