//! 普通文件系统的 LRC 与音频标签写入，以及完整下载收尾实现。

use std::path::Path;

use futures_util::future::BoxFuture;
use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::tag::{ItemKey, Tag, TagType};

use crate::download::context::TaskContext;
use crate::download::ports::{DownloadPostprocessor, DownloadProgressSink, PostprocessRequest};
use crate::download::postprocess::{prepare_assets, PostprocessAssets};
use crate::platforms::lyric::LyricData;
/// 将普通 LRC 写在音频旁边；无歌词或写入失败都不改变音频下载结果。
pub fn write_local_lrc(audio_path: &str, lyric: &LyricData) -> Option<String> {
    let content = lyric
        .lrc
        .as_deref()
        .filter(|value| !value.trim().is_empty())?;
    let audio = Path::new(audio_path);
    let stem = audio
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown");
    let parent = audio.parent().unwrap_or_else(|| Path::new("."));
    let lrc_path = parent.join(format!("{stem}.lrc"));

    match std::fs::write(&lrc_path, content) {
        Ok(()) => {
            log::info!("LRC 歌词文件已保存: {}", lrc_path.display());
            Some(lrc_path.to_string_lossy().to_string())
        }
        Err(error) => {
            log::warn!("写入 LRC 歌词文件失败 {}: {error}", lrc_path.display());
            None
        }
    }
}

/// 写入本地音频标签。返回值表示是否有歌词或封面需要写入。
/// 保持已有行为：无歌词和封面时不覆盖音频原有标题、歌手及专辑。
pub fn write_audio_metadata(
    path: &Path,
    context: &TaskContext,
    assets: &PostprocessAssets,
) -> Result<bool, String> {
    let lyric_text = assets.lyric.as_ref().and_then(|lyric| {
        lyric
            .elrc
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                lyric
                    .lrc
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
            })
    });
    if lyric_text.is_none() && assets.cover_bytes.is_none() {
        log::info!("无可用歌词或封面，跳过 metadata 写入");
        return Ok(false);
    }

    let mut tagged_file =
        lofty::read_from_path(path).map_err(|error| format!("读取音频文件失败: {error}"))?;

    // lofty 0.22 写 ID3v1 时可能在多字节字符中间按字节截断，触发 panic。
    // 主标签保存完整字段，移除 128 字节的旧标签不会丢掉本次写入的信息。
    if tagged_file.remove(TagType::Id3v1).is_some() {
        log::info!("已移除 ID3v1 标签，避免多字节字符截断引发崩溃");
    }

    let tag_type = tagged_file.primary_tag_type();
    if tagged_file.primary_tag().is_none() {
        tagged_file.insert_tag(Tag::new(tag_type));
    }
    let tag = tagged_file
        .primary_tag_mut()
        .ok_or_else(|| "无法获取音频标签".to_string())?;

    // 标签字段来自任务上下文。先清掉旧值，避免空字段保留下载源中的错误信息。
    for (key, value) in [
        (ItemKey::TrackTitle, &context.song_info.title),
        (ItemKey::TrackArtist, &context.song_info.artist),
        (ItemKey::AlbumTitle, &context.song_info.album),
    ] {
        tag.remove_key(&key);
        if !value.is_empty() {
            tag.insert_text(key, value.clone());
        }
    }
    if let Some(lyric) = lyric_text {
        tag.remove_key(&ItemKey::Lyrics);
        tag.insert_text(ItemKey::Lyrics, lyric.to_string());
    }
    if let Some(bytes) = &assets.cover_bytes {
        let picture = Picture::new_unchecked(
            PictureType::CoverFront,
            Some(MimeType::Jpeg),
            None,
            bytes.clone(),
        );
        tag.remove_picture_type(PictureType::CoverFront);
        tag.push_picture(picture);
    }

    tagged_file
        .save_to_path(path, WriteOptions::default())
        .map_err(|error| format!("保存 metadata 失败: {error}"))?;
    log::info!("metadata 已写入: {}", path.display());
    Ok(true)
}

/// 普通文件系统的完整收尾实现，供独立进程持有。
pub struct LocalDownloadPostprocessor;

impl DownloadPostprocessor for LocalDownloadPostprocessor {
    fn process<'a>(
        &'a self,
        request: PostprocessRequest<'a>,
        progress_sink: &'a dyn DownloadProgressSink,
    ) -> BoxFuture<'a, Option<String>> {
        Box::pin(async move {
            let assets = prepare_assets(
                request.context,
                request.config.write_metadata,
                request.config.download_lrc,
            )
            .await;

            // 先写音频标签，再处理独立 LRC；标签失败只发警告，仍继续尝试歌词文件。
            if request.config.write_metadata {
                if let Err(error) =
                    write_audio_metadata(Path::new(request.audio_path), request.context, &assets)
                {
                    log::warn!("{}: {error}", request.context.task_id);
                    progress_sink.metadata_error(&request.context.task_id, &error);
                }
            }

            if request.config.download_lrc {
                if let Some(lyric) = assets.lyric.as_ref() {
                    // 独立歌词是可选产物，写入失败不会回退已经完成的音频下载。
                    return write_local_lrc(request.audio_path, lyric);
                }
                log::info!("无普通歌词，跳过 LRC 文件创建");
            }
            None
        })
    }
}

#[cfg(test)]
mod tests {
    use super::write_local_lrc;
    use crate::platforms::lyric::LyricData;

    #[test]
    fn ordinary_lrc_uses_audio_stem_and_preserves_content() {
        let directory =
            std::env::temp_dir().join(format!("hotdownloader-lrc-{:x}", rand::random::<u64>()));
        std::fs::create_dir_all(&directory).unwrap();
        let audio = directory.join("歌名.flac");
        let lyric = LyricData {
            lrc: Some("[00:01.00]歌词\n".into()),
            ..LyricData::default()
        };

        let written = write_local_lrc(audio.to_str().unwrap(), &lyric).unwrap();
        assert_eq!(std::path::Path::new(&written), directory.join("歌名.lrc"));
        assert_eq!(
            std::fs::read_to_string(&written).unwrap(),
            "[00:01.00]歌词\n"
        );

        std::fs::remove_file(written).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
