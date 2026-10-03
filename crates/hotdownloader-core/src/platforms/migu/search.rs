//! 咪咕音乐歌曲搜索模块。
//!
//! 调用咪咕 App 搜索接口
//! `https://app.c.nf.migu.cn/MIGUM2.0/v1.0/content/search_all.do`。
//! 实测（`_dev/probe/migu-probe-e1.log`）：该接口**不需要签名**，
//! 但缺 `channel` 请求头会被网关拒绝；keyword=晴天 返回 20 条，
//! `songResultData.totalCount` 为字符串 `"216"`。
//!
//! 关键点：
//! - `page` 语义与前端一致（从 1 开始），直接作为咪咕的 `pageNo`；
//! - `searchSwitch` 只打开 `song` 与 `bestShow`，避免其它类型改变响应结构；
//! - 搜索结果**不逐首追加请求**（避免风控），封面直接取结果里的 `imgItems`。

use reqwest::Url;
use serde_json::{json, Value};

use super::parser::{number, parse_song};
use crate::platforms::CLIENT;

/// 搜索接口地址（MIGUM2.0 实测可用；MIGUM3.0 同路径表现一致）。
const SEARCH_URL: &str = "https://app.c.nf.migu.cn/MIGUM2.0/v1.0/content/search_all.do";

/// 只开歌曲与最佳匹配的 `searchSwitch`。
const SEARCH_SWITCH: &str = r#"{"song":1,"album":0,"singer":0,"tagSong":0,"mvSong":0,"bestShow":1,"songlist":0,"lyric":0,"program":0,"dt":0}"#;

/// 每页条数的兜底值（咪咕默认一页 20 条）。
const DEFAULT_LIMIT: u32 = 20;

/// 搜索歌曲，返回 JSON 字符串，包含 `songs` 和 `has_more`。
///
/// # 参数
/// - `separator`：多歌手连接符。
/// - `keyword`：搜索关键字。
/// - `page`：页码（从 1 开始）。
/// - `limit`：每页歌曲数量。
///
/// # 返回
/// - `Ok(String)`：`{"songs": [...], "has_more": bool}`。
/// - `Err(String)`：错误信息（网络类错误带契约 §4 规定的可重试前缀）。
pub async fn search_songs(
    separator: &str,
    keyword: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    let page = page.max(1);
    let limit = if limit == 0 { DEFAULT_LIMIT } else { limit };

    let data = search_request(&keyword, page, limit).await?;

    let code = data.get("code").and_then(Value::as_str).unwrap_or("000000");
    if code != "000000" {
        let info = data
            .get("info")
            .and_then(Value::as_str)
            .unwrap_or("未知错误");
        return Err(format!("咪咕搜索失败: {}（code={}）", info, code));
    }

    let items = data
        .get("songResultData")
        .and_then(|result| result.get("result"))
        .and_then(Value::as_array)
        .ok_or("未找到歌曲列表（songResultData.result 字段缺失）")?;

    let songs: Vec<Value> = items
        .iter()
        .filter_map(|item| parse_song(item, separator))
        .collect();

    // 分页判断：`totalCount` 是字符串总数，offset 用本页原始条数（而非解析后条数）计算，
    // 避免个别条目缺少 contentId 被跳过后误判为最后一页。
    let total = data
        .get("songResultData")
        .and_then(|result| result.get("totalCount"))
        .map(number)
        .unwrap_or(0);
    let returned = items.len() as u64;
    let offset = u64::from(page.saturating_sub(1)) * u64::from(limit);
    let has_more = if total > 0 {
        offset + returned < total
    } else {
        !items.is_empty() && returned >= u64::from(limit)
    };

    let payload = json!({
        "songs": songs,
        "has_more": has_more
    });

    serde_json::to_string(&payload).map_err(|e| format!("序列化结果失败: {}", e))
}

/// 发起一次搜索请求并解析为 JSON。
async fn search_request(keyword: &str, page: u32, limit: u32) -> Result<Value, String> {
    let url = search_url(keyword, page, limit)?;

    let resp = super::app_request(CLIENT.get(url))
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;

    serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))
}

/// 构造搜索 URL（参数与实测脚本 `_dev/probe/migu-probe.mjs` 完全一致）。
fn search_url(keyword: &str, page: u32, limit: u32) -> Result<Url, String> {
    let mut url = Url::parse(SEARCH_URL).map_err(|e| format!("URL 构建失败: {}", e))?;
    {
        let mut params = url.query_pairs_mut();
        params.append_pair("text", keyword);
        params.append_pair("pageNo", &page.to_string());
        params.append_pair("pageSize", &limit.to_string());
        params.append_pair("isCopyright", "1");
        params.append_pair("sort", "1");
        params.append_pair("searchSwitch", SEARCH_SWITCH);
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_url_follows_measured_parameters() {
        let url = search_url("周杰伦 & Jay", 2, 30).unwrap();
        let params: std::collections::HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(url.path(), "/MIGUM2.0/v1.0/content/search_all.do");
        assert_eq!(params["text"], "周杰伦 & Jay");
        assert_eq!(params["pageNo"], "2");
        assert_eq!(params["pageSize"], "30");
        assert_eq!(params["isCopyright"], "1");
        assert_eq!(params["sort"], "1");
        assert!(params["searchSwitch"].contains("\"song\":1"));
        assert!(params["searchSwitch"].contains("\"bestShow\":1"));
    }
}
