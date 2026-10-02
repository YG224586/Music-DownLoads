//! 音乐源配置：解析 QingMusic 风格的 music.json，
//! 把其中的音源条目映射到本应用内置的平台引擎。
//!
//! 配置只控制内置引擎（qqmusic / kuwo）的启用状态和对外展示的音质档位，
//! 不会下载或执行任何远程脚本 —— `searchApi` / `detailApi` 字段是
//! QingMusic 内部的函数名，这里仅保留原文用于展示，不参与分发。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// music.json 中的一条音源配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicSourceLine {
    /// QingMusic 音源标识：tx / kw / kg / wy / mg / bili 等。
    pub id: String,
    /// 展示名，如「小Q源」。
    pub name: String,
    /// 配置里的初始开关；用户在本应用内的切换会覆盖它。
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// QingMusic 内部的搜索函数名（仅展示保留，不执行）。
    #[serde(default)]
    pub search_api: String,
    /// QingMusic 内部的详情函数名（仅展示保留，不执行）。
    #[serde(default)]
    pub detail_api: Option<String>,
    /// 配置声明的音质档位，如 standard / exhigh / lossless。
    #[serde(default)]
    pub levels: Vec<String>,
}

/// music.json 顶层结构：`{ "lines": [ ... ] }`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicSourceConfig {
    #[serde(default)]
    pub lines: Vec<MusicSourceLine>,
}

/// 交给前端的单个音源状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceMapping {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    /// 能映射到的内置平台 key（qqmusic / kuwo），None 表示暂不支持。
    pub platform: Option<String>,
    /// 映射后的允许音质列表；空 = 不限制。
    pub levels: Vec<String>,
    /// 配置原始 searchApi，便于展示来源信息。
    #[serde(default)]
    pub search_api: String,
}

/// 持久化的音源配置状态（data.json 的 `sourceConfig` 键）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceConfigState {
    /// 最近一次成功导入的 URL；未导入为 None。
    #[serde(default)]
    pub url: Option<String>,
    /// 是否导入过外部配置。
    pub imported: bool,
    pub sources: Vec<SourceMapping>,
}

fn default_true() -> bool {
    true
}

/// QingMusic 音源 id → 本应用平台 key。
/// 只有 tx（QQ）和 kw（酷我）有内置引擎，其余返回 None。
pub fn platform_for_source(id: &str) -> Option<&'static str> {
    match id {
        "tx" => Some("qqmusic"),
        "kw" => Some("kuwo"),
        _ => None,
    }
}

/// QingMusic 音质档位 → 本应用音质标识。
/// 未知档位返回 None（跳过，不视为错误）。
pub fn map_level(level: &str) -> Option<&'static str> {
    match level {
        "standard" => Some("128kmp3"),
        "exhigh" => Some("320kmp3"),
        "lossless" => Some("flac"),
        "hires" => Some("hires"),
        "master" | "jymaster" => Some("臻品母带"),
        _ => None,
    }
}

/// 未导入配置时的默认状态：两个内置平台全部启用、音质不限制。
pub fn default_source_state() -> SourceConfigState {
    SourceConfigState {
        url: None,
        imported: false,
        sources: vec![
            SourceMapping {
                id: "qqmusic".to_string(),
                name: "QQ音乐".to_string(),
                enabled: true,
                platform: Some("qqmusic".to_string()),
                levels: Vec::new(),
                search_api: String::new(),
            },
            SourceMapping {
                id: "kuwo".to_string(),
                name: "酷我音乐".to_string(),
                enabled: true,
                platform: Some("kuwo".to_string()),
                levels: Vec::new(),
                search_api: String::new(),
            },
        ],
    }
}

/// 解析并校验 music.json 文本。
pub fn parse_source_config(json: &str) -> Result<MusicSourceConfig, String> {
    let config: MusicSourceConfig =
        serde_json::from_str(json).map_err(|error| format!("音源配置格式不正确: {error}"))?;
    if config.lines.is_empty() {
        return Err("音源配置里没有任何音源".to_string());
    }
    for line in &config.lines {
        if line.id.trim().is_empty() {
            return Err("音源配置里存在缺少 id 的条目".to_string());
        }
    }
    Ok(config)
}

/// 由导入的配置 + 用户历史开关偏好，构造前端可见状态。
/// `overrides` 以音源 id 为键，记录用户在本应用内做过的开关选择。
pub fn build_state(
    url: Option<String>,
    config: &MusicSourceConfig,
    overrides: &HashMap<String, bool>,
) -> SourceConfigState {
    let sources = config
        .lines
        .iter()
        .map(|line| {
            let enabled = overrides.get(&line.id).copied().unwrap_or(line.enabled);
            let levels = line
                .levels
                .iter()
                .filter_map(|level| map_level(level))
                .map(str::to_string)
                .collect();
            SourceMapping {
                id: line.id.clone(),
                name: line.name.clone(),
                enabled: enabled && platform_for_source(&line.id).is_some(),
                platform: platform_for_source(&line.id).map(str::to_string),
                levels,
                search_api: line.search_api.clone(),
            }
        })
        .collect();
    SourceConfigState {
        url,
        imported: true,
        sources,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "lines": [
            {
                "id": "kw",
                "name": "小窝源",
                "enabled": true,
                "searchApi": "fetchSearchMusic",
                "detailApi": "fetchMusicDetail",
                "levels": ["standard", "exhigh", "lossless", "hires", "master"]
            },
            {
                "id": "tx",
                "name": "小Q源",
                "enabled": true,
                "searchApi": "txSearchMusic",
                "levels": ["standard", "exhigh", "lossless"]
            },
            {
                "id": "kg",
                "name": "小狗源",
                "enabled": true,
                "searchApi": "kgSearchMusic",
                "levels": ["standard"]
            },
            {
                "id": "wy",
                "name": "小云源",
                "enabled": false,
                "searchApi": "wySearchMusic",
                "levels": ["standard", "jymaster"]
            }
        ]
    }"#;

    #[test]
    fn parse_accepts_real_shaped_config() {
        let config = parse_source_config(SAMPLE).unwrap();
        assert_eq!(config.lines.len(), 4);
        assert_eq!(config.lines[0].id, "kw");
        assert_eq!(config.lines[1].search_api, "txSearchMusic");
        assert_eq!(config.lines[3].enabled, false);
    }

    #[test]
    fn parse_rejects_empty_and_malformed() {
        assert!(parse_source_config("{}").is_err());
        assert!(parse_source_config("{\"lines\": []}").is_err());
        assert!(parse_source_config("not json").is_err());
        assert!(parse_source_config("{\"lines\": [{\"name\": \"x\"}]}").is_err());
    }

    #[test]
    fn build_state_maps_platforms_and_levels() {
        let config = parse_source_config(SAMPLE).unwrap();
        let state = build_state(None, &config, &HashMap::new());
        assert!(state.imported);
        let kw = state.sources.iter().find(|s| s.id == "kw").unwrap();
        assert_eq!(kw.platform.as_deref(), Some("kuwo"));
        assert_eq!(
            kw.levels,
            vec!["128kmp3", "320kmp3", "flac", "hires", "臻品母带"]
        );
        let tx = state.sources.iter().find(|s| s.id == "tx").unwrap();
        assert_eq!(tx.platform.as_deref(), Some("qqmusic"));
        assert_eq!(tx.levels, vec!["128kmp3", "320kmp3", "flac"]);
        let kg = state.sources.iter().find(|s| s.id == "kg").unwrap();
        assert!(kg.platform.is_none());
        assert!(!kg.enabled, "暂不支持的平台一律视为未启用");
    }

    #[test]
    fn build_state_respects_user_overrides() {
        let config = parse_source_config(SAMPLE).unwrap();
        let mut overrides = HashMap::new();
        overrides.insert("tx".to_string(), false);
        overrides.insert("wy".to_string(), true);
        let state = build_state(None, &config, &overrides);
        let tx = state.sources.iter().find(|s| s.id == "tx").unwrap();
        assert!(!tx.enabled);
        let wy = state.sources.iter().find(|s| s.id == "wy").unwrap();
        assert!(!wy.enabled, "无内置引擎的平台即使配置开启也不可用");
    }

    #[test]
    fn map_level_covers_qingmusic_levels() {
        assert_eq!(map_level("standard"), Some("128kmp3"));
        assert_eq!(map_level("exhigh"), Some("320kmp3"));
        assert_eq!(map_level("lossless"), Some("flac"));
        assert_eq!(map_level("hires"), Some("hires"));
        assert_eq!(map_level("master"), Some("臻品母带"));
        assert_eq!(map_level("jymaster"), Some("臻品母带"));
        assert_eq!(map_level("bili192"), None);
    }

    #[test]
    fn default_state_enables_both_platforms_without_restriction() {
        let state = default_source_state();
        assert!(!state.imported);
        assert_eq!(state.sources.len(), 2);
        assert!(state
            .sources
            .iter()
            .all(|s| s.enabled && s.levels.is_empty()));
        assert_eq!(platform_for_source("tx"), Some("qqmusic"));
        assert_eq!(platform_for_source("kw"), Some("kuwo"));
        assert_eq!(platform_for_source("kg"), None);
    }
}
