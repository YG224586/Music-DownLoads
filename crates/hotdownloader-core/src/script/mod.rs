//! 自定义音源脚本（用户粘贴一段 JS 即可新增音源）。
//!
//! 契约：`_dev/script-spec/API.md`。本模块只做三件事：
//! 1. 安装校验（`source.name` / `search` / `getUrl`）与进程内注册表；
//! 2. 把脚本调用搬进 `spawn_blocking`（boa 是同步引擎，且内部用 reqwest blocking）；
//! 3. 归一化脚本返回值（音质、扩展名、稳定数字 id）。

mod engine;
pub mod limits;
mod model;
mod registry;

pub use engine::ScriptDescriptor;
pub use model::{
    ScriptCallError, ScriptSong, ScriptSource, ScriptSourceItem, ScriptSourceState,
    ScriptTestReport, ScriptUrl,
};

use std::sync::Arc;

use crate::task::contract::QualityItem;

/// 音质 → 落盘扩展名（必须与 `prelude.js` 的 `QUALITY_EXTENSIONS` 保持一致）。
pub fn quality_extension(quality: &str) -> Option<&'static str> {
    Some(match quality.trim() {
        "48kaac" => "m4a",
        "96kogg" => "ogg",
        "128kmp3" => "mp3",
        "192kaac" => "m4a",
        "192kogg" => "ogg",
        "320kmp3" => "mp3",
        "flac" => "flac",
        "hires" => "flac",
        "臻品母带" => "flac",
        _ => return None,
    })
}

/// 落盘文件名：`{音质}.{扩展名}`；下载链路靠它反推请求音质。
pub fn quality_filename(quality: &str) -> Option<String> {
    quality_extension(quality).map(|extension| format!("{}.{extension}", quality.trim()))
}

/// 从落盘文件名反推音质（扩展名必须与音质匹配，否则视为不识别）。
pub fn quality_from_filename(filename: &str) -> Option<String> {
    let (quality, extension) = filename.rsplit_once('.')?;
    let expected = quality_extension(quality)?;
    extension
        .eq_ignore_ascii_case(expected)
        .then(|| quality.trim().to_string())
}

/// 音质高低排序，用于「脚本返回值不得高于请求音质」的校验。
fn quality_rank(quality: &str) -> Option<u32> {
    Some(match quality.trim() {
        "48kaac" => 48,
        "96kogg" => 96,
        "128kmp3" => 128,
        "192kaac" | "192kogg" => 192,
        "320kmp3" => 320,
        "flac" => 1000,
        "hires" => 2000,
        "臻品母带" => 3000,
        _ => return None,
    })
}

/// 脚本歌曲 id → 前端需要的数字 id（能解析就用原值，否则用 FNV-1a 稳定哈希）。
pub fn song_numeric_id(song_id: &str) -> u64 {
    crate::task::contract::song_id_to_u64(song_id)
}

/// 脚本音质的可下载品质列表（`size` 未知，交给下载链路按响应头校正）。
pub fn quality_items(qualities: &[String]) -> Vec<QualityItem> {
    qualities
        .iter()
        .filter_map(|quality| {
            quality_filename(quality).map(|filename| QualityItem {
                quality: quality.trim().to_string(),
                filename,
                size: 0,
            })
        })
        .collect()
}

/// 当前所有音源（供 `list_script_sources`；不回传脚本正文）。
pub fn list_sources() -> ScriptSourceState {
    registry::snapshot()
}

/// 持久化快照（`store_wrapper` 键 `scriptSources`）。
pub fn sources_json() -> String {
    let records: Vec<ScriptSource> = registry::list()
        .into_iter()
        .map(|source| (*source).clone())
        .collect();
    serde_json::to_string(&records).unwrap_or_else(|_| "[]".to_string())
}

/// 启动时恢复（容忍任意损坏数据，返回恢复条数）。
pub fn restore_from_json(raw: &str) -> usize {
    if raw.trim().is_empty() {
        return 0;
    }
    let records = match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(serde_json::Value::Array(items)) => items,
        Ok(serde_json::Value::Object(mut object)) => match object.remove("sources") {
            Some(serde_json::Value::Array(items)) => items,
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };
    let mut sources = Vec::new();
    for item in records {
        match serde_json::from_value::<ScriptSource>(item) {
            Ok(source) if !source.script.trim().is_empty() => sources.push(source),
            _ => continue,
        }
    }
    registry::replace_all(sources)
}

pub fn set_enabled(id: u32, enabled: bool) -> Result<ScriptSourceState, String> {
    registry::set_enabled(id, enabled).ok_or_else(|| "音源不存在".to_string())?;
    Ok(registry::snapshot())
}

pub fn remove_source(id: u32) -> Result<ScriptSourceState, String> {
    registry::remove(id).ok_or_else(|| "音源不存在".to_string())?;
    Ok(registry::snapshot())
}

/// 分页参数归一化（契约 §3：`page` 从 1 开始，`limit` 默认 20 上限 50）。
pub fn normalize_page(page: u32) -> u32 {
    page.max(1)
}

pub fn normalize_limit(limit: u32) -> u32 {
    if limit == 0 {
        limits::DEFAULT_PAGE_LIMIT
    } else {
        limit.min(limits::MAX_PAGE_LIMIT)
    }
}

/// 安装（或按同名覆盖）一个音源。
pub async fn install_script(
    script: String,
    name: Option<String>,
    url: Option<String>,
) -> Result<ScriptSource, String> {
    run_blocking(move || {
        let url = url
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let (script, source_url) = if script.trim().is_empty() {
            match url {
                Some(url) => {
                    let text = engine::fetch_text(&url).map_err(|error| error.user_message())?;
                    (text, Some(url))
                }
                None => return Err("脚本内容为空".to_string()),
            }
        } else {
            (script, url)
        };

        let descriptor = engine::describe(&script).map_err(|error| error.user_message())?;
        validate_descriptor(&descriptor)?;
        let resolved_name = match name
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            Some(value) => model::normalize_name(&value)?,
            None => model::normalize_name(&descriptor.name)?,
        };
        let record = ScriptSource {
            id: registry::find_by_name(&resolved_name)
                .map(|source| source.id)
                .unwrap_or(0),
            name: resolved_name,
            description: descriptor
                .description
                .clone()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            qualities: descriptor.qualities.clone(),
            script,
            enabled: true,
            installed_at: model::iso8601_now(),
            source_url,
        };
        Ok((*registry::insert(record)).clone())
    })
    .await
}

/// 测试脚本：静态校验 + （可选）跑一次搜索。
pub async fn test_script(script: String, keyword: Option<String>) -> ScriptTestReport {
    match run_blocking(move || -> Result<ScriptTestReport, ScriptCallError> {
        let descriptor = match engine::describe(&script) {
            Ok(descriptor) => descriptor,
            Err(error) => return Ok(ScriptTestReport::failure(error.user_message())),
        };
        let name = descriptor.name.trim().to_string();
        let qualities = descriptor.qualities.clone();
        if let Err(message) = validate_descriptor(&descriptor) {
            return Ok(ScriptTestReport {
                ok: false,
                name: (!name.is_empty()).then_some(name),
                qualities: Some(qualities),
                sample: None,
                error: Some(message),
            });
        }
        let keyword = keyword.unwrap_or_default().trim().to_string();
        if keyword.is_empty() {
            return Ok(ScriptTestReport {
                ok: true,
                name: Some(name),
                qualities: Some(qualities),
                sample: None,
                error: None,
            });
        }
        match engine::search(&script, &keyword, 1, limits::DEFAULT_PAGE_LIMIT) {
            Ok(songs) => {
                let sample = songs.into_iter().next();
                Ok(ScriptTestReport {
                    ok: sample.is_some(),
                    name: Some(name),
                    qualities: Some(qualities),
                    sample,
                    error: None,
                })
            }
            Err(error) => Ok(ScriptTestReport {
                ok: false,
                name: Some(name),
                qualities: Some(qualities),
                sample: None,
                error: Some(error.user_message()),
            }),
        }
    })
    .await
    {
        Ok(report) => report,
        Err(error) => ScriptTestReport::failure(error.user_message()),
    }
}

/// 用某个已安装音源搜索。
pub async fn search_source(
    id: u32,
    keyword: String,
    page: u32,
    limit: u32,
) -> Result<Vec<ScriptSong>, String> {
    let source = active_source(id)?;
    let keyword = keyword.trim().to_string();
    if keyword.is_empty() {
        return Ok(Vec::new());
    }
    let page = normalize_page(page);
    let limit = normalize_limit(limit);
    let songs = run_blocking(move || engine::search(&source.script, &keyword, page, limit))
        .await
        .map_err(|error| error.user_message())?;
    // 记住本次搜索到的歌曲：下载链路只拿得到歌曲 id（任务里的 song_mid），
    // 必须靠这里的完整对象（含 raw 原始字段）才能让脚本的 getUrl 正常构造直链。
    for song in &songs {
        registry::remember_source_song(id, song);
    }
    Ok(songs)
}

/// 取直链（脚本歌曲对象由调用方给出）。
pub async fn fetch_song_url(
    source_id: u32,
    song: ScriptSong,
    quality: &str,
) -> Result<ScriptUrl, String> {
    let source = active_source(source_id)?;
    let requested = quality.trim().to_string();
    // 调用方给了完整歌曲对象，顺手记住，之后同 id 的下载任务也能命中线索缓存。
    registry::remember_source_song(source_id, &song);
    let url = run_blocking(move || engine::get_url(&source.script, &song, &requested))
        .await
        .map_err(|error| error.user_message())?;
    validate_returned_url(&url, quality)
}

/// 下载链路入口：只拿得到歌曲 id 与落盘文件名，歌曲对象从注册表线索缓存补全。
pub async fn fetch_url_for_link(
    source_id: u32,
    song_id: &str,
    quality_filename: &str,
) -> Result<ScriptUrl, String> {
    let quality = quality_from_filename(quality_filename)
        .ok_or_else(|| format!("无法识别音质: {quality_filename}"))?;
    let song = registry::recall_song(source_id, song_id).unwrap_or_else(|| {
        log::warn!("音源 {source_id} 找不到歌曲 {song_id} 的搜索线索，仅用 id 调用 getUrl");
        ScriptSong {
            id: song_id.to_string(),
            ..ScriptSong::default()
        }
    });
    fetch_song_url(source_id, song, &quality).await
}

fn active_source(id: u32) -> Result<Arc<ScriptSource>, String> {
    let source = registry::get(id).ok_or_else(|| "音源不存在".to_string())?;
    if !source.enabled {
        return Err("音源已停用".to_string());
    }
    Ok(source)
}

fn validate_descriptor(descriptor: &ScriptDescriptor) -> Result<(), String> {
    if !descriptor.has_source || descriptor.name.trim().is_empty() {
        return Err(limits::MISSING_NAME_MESSAGE.to_string());
    }
    if !descriptor.has_search || !descriptor.has_get_url {
        return Err(limits::MISSING_METHOD_MESSAGE.to_string());
    }
    Ok(())
}

/// 契约 §4：返回值音质必须合法、不得高于请求音质，且容器要与请求音质一致
/// （落盘文件名在取链前就定了，扩展名不能撒谎）。
fn validate_returned_url(url: &ScriptUrl, requested: &str) -> Result<ScriptUrl, String> {
    let requested = requested.trim();
    let returned = url
        .quality
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(requested)
        .to_string();
    let returned_extension = quality_extension(&returned)
        .ok_or_else(|| format!("脚本返回了不支持的音质: {returned}"))?;
    if let (Some(requested_rank), Some(returned_rank)) =
        (quality_rank(requested), quality_rank(&returned))
    {
        if returned_rank > requested_rank {
            return Err(format!(
                "脚本返回的音质高于请求（{returned} > {requested}）"
            ));
        }
    }
    if let Some(requested_extension) = quality_extension(requested) {
        if requested_extension != returned_extension {
            return Err(format!(
                "脚本返回的音频格式与请求音质不一致（{returned} / 请求 {requested}）"
            ));
        }
    }
    Ok(ScriptUrl {
        url: url.url.clone(),
        quality: Some(returned),
        headers: url.headers.clone(),
    })
}

async fn run_blocking<T, E, F>(job: F) -> Result<T, E>
where
    F: FnOnce() -> Result<T, E> + Send + 'static,
    T: Send + 'static,
    E: Send + 'static + From<String>,
{
    match tokio::task::spawn_blocking(job).await {
        Ok(result) => result,
        Err(error) => Err(E::from(format!("脚本执行线程异常: {error}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(quality: Option<&str>) -> ScriptUrl {
        ScriptUrl {
            url: "https://cdn.example.com/a.mp3".into(),
            quality: quality.map(str::to_string),
            headers: Vec::new(),
        }
    }

    #[test]
    fn quality_extension_matches_prelude_table() {
        assert_eq!(quality_extension("128kmp3"), Some("mp3"));
        assert_eq!(quality_extension("320kmp3"), Some("mp3"));
        assert_eq!(quality_extension("flac"), Some("flac"));
        assert_eq!(quality_extension("192kogg"), Some("ogg"));
        assert_eq!(quality_extension("48kaac"), Some("m4a"));
        assert_eq!(quality_extension("臻品母带"), Some("flac"));
        assert_eq!(quality_extension("unknown"), None);
    }

    #[test]
    fn filename_round_trip_recovers_quality() {
        let filename = quality_filename("320kmp3").unwrap();
        assert_eq!(filename, "320kmp3.mp3");
        assert_eq!(quality_from_filename(&filename).as_deref(), Some("320kmp3"));
        assert_eq!(quality_from_filename("320kmp3.flac"), None);
        assert_eq!(quality_from_filename("晴天.mp3"), None);
        assert_eq!(quality_from_filename("noextension"), None);
    }

    #[test]
    fn numeric_song_id_prefers_parse_then_stable_hash() {
        assert_eq!(song_numeric_id("12345"), 12345);
        let first = song_numeric_id("M800003Qui1q2u1Zho");
        assert_eq!(first, song_numeric_id("M800003Qui1q2u1Zho"));
        assert_ne!(first, song_numeric_id("M800003Qui1q2u1Zhp"));
        assert_ne!(first, 0);
    }

    #[test]
    fn returned_quality_must_not_exceed_request() {
        assert!(validate_returned_url(&url(Some("flac")), "320kmp3").is_err());
        assert!(validate_returned_url(&url(Some("320kmp3")), "320kmp3").is_ok());
        assert!(validate_returned_url(&url(Some("128kmp3")), "320kmp3").is_ok());
        assert!(validate_returned_url(&url(None), "flac").is_ok());
    }

    #[test]
    fn returned_container_must_match_requested_quality() {
        // 同容器、音质更低：放行。
        assert!(validate_returned_url(&url(Some("128kmp3")), "320kmp3").is_ok());
        // 请求无损却返回 mp3：容器不一致，拒绝。
        assert!(validate_returned_url(&url(Some("320kmp3")), "flac").is_err());
        assert!(validate_returned_url(&url(Some("96kogg")), "128kmp3").is_err());
        assert!(validate_returned_url(&url(Some("unknown")), "128kmp3").is_err());
        // flac 家族的其它档位容器相同，但音质高于请求，同样拒绝。
        assert!(validate_returned_url(&url(Some("hires")), "flac").is_err());
    }

    #[test]
    fn pagination_is_clamped_to_contract_limits() {
        assert_eq!(normalize_page(0), 1);
        assert_eq!(normalize_page(3), 3);
        assert_eq!(normalize_limit(0), limits::DEFAULT_PAGE_LIMIT);
        assert_eq!(normalize_limit(1_000), limits::MAX_PAGE_LIMIT);
    }
}
