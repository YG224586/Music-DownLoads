//! 酷狗音乐搜索模块。
//!
//! 实测结论（`_dev/probe/kugou-probe.mjs` / `kugou-probe.log`）：
//! - `https://searchcdn.kugou.com`：域名不存在（`ENOTFOUND`）。
//! - `https://msearchcdn.kugou.com`：证书与域名不匹配（`ERR_TLS_CERT_ALTNAME_INVALID`）。
//! - 只有 `http://mobilecdn.kugou.com/api/v3/search/song` 可用，返回 `content-type: text/html`
//!   但响应体是 JSON（Android 已开启 `usesCleartextTraffic`，http 可用）。
//!
//! 搜索结果**不逐首再发请求**：酷狗搜索响应里已带各档 `hash` 与大小，直接归一化即可。

use reqwest::Url;
use serde_json::{json, Value};

use super::parser::parse_song;
use crate::platforms::CLIENT;

/// 实测可用的搜索接口（只支持 http）。
const SEARCH_ENDPOINT: &str = "http://mobilecdn.kugou.com/api/v3/search/song";

/// 探测脚本实测使用的 UA（该接口不校验 Referer）。
const SEARCH_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// 搜索酷狗歌曲，返回契约 JSON 字符串 `{"songs":[…],"has_more":bool}`。
///
/// # 参数
/// - `separator`：歌手名分隔符（来自设置），用于把酷狗合并的歌手名尽量拆开。
/// - `keyword`：搜索关键词。
/// - `page`：页码，从 1 开始。
/// - `limit`：每页条数。
pub async fn search_songs(
    separator: &str,
    keyword: String,
    page: u32,
    limit: u32,
) -> Result<String, String> {
    if keyword.trim().is_empty() {
        return Err("搜索关键词不能为空".into());
    }

    let data = search_request(&keyword, page, limit).await?;

    if let Some(status) = data["status"].as_i64() {
        if status != 1 {
            return Err(format!(
                "酷狗接口错误: status={}, errcode={}",
                status,
                data["errcode"].as_i64().unwrap_or_default()
            ));
        }
    }

    let list = data["data"]["info"]
        .as_array()
        .ok_or_else(|| "酷狗搜索响应缺少 data.info 字段".to_string())?;
    let songs: Vec<Value> = list
        .iter()
        .filter_map(|item| parse_song(item, separator))
        .collect();

    let total = data["data"]["total"].as_u64().unwrap_or(0);
    let has_more = compute_has_more(total, page, limit, songs.len());

    Ok(json!({ "songs": songs, "has_more": has_more }).to_string())
}

/// 发起搜索请求；错误分级与契约一致（只有 `网络错误: `/`读取响应失败: `/`解析响应失败: ` 可重试）。
async fn search_request(keyword: &str, page: u32, limit: u32) -> Result<Value, String> {
    let url = search_url(keyword, page, limit)?;
    let response = CLIENT
        .get(url)
        .header("User-Agent", SEARCH_USER_AGENT)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;
    let text = response
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))
}

/// 构造搜索 URL，参数与探测脚本实测通过的一致。
fn search_url(keyword: &str, page: u32, limit: u32) -> Result<Url, String> {
    let mut url = Url::parse(SEARCH_ENDPOINT).map_err(|e| format!("URL 构建失败: {}", e))?;
    url.query_pairs_mut()
        .append_pair("format", "json")
        .append_pair("keyword", keyword)
        .append_pair("page", page.max(1).to_string().as_str())
        .append_pair("pagesize", limit.to_string().as_str())
        .append_pair("showtype", "1");
    Ok(url)
}

/// 分页判断：接口给了 `total` 时用 `offset + 本页数 < total`，否则退化为「本页是否装满」。
fn compute_has_more(total: u64, page: u32, limit: u32, returned: usize) -> bool {
    if total > 0 {
        let offset = u64::from(page.saturating_sub(1)) * u64::from(limit);
        offset + (returned as u64) < total
    } else {
        returned as u64 == u64::from(limit)
    }
}

#[cfg(test)]
mod tests {
    use super::{compute_has_more, search_url};

    #[test]
    fn search_url_matches_probed_request() {
        let url = search_url("晴天", 1, 20).unwrap();
        assert_eq!(url.scheme(), "http");
        assert_eq!(url.host_str(), Some("mobilecdn.kugou.com"));
        assert_eq!(url.path(), "/api/v3/search/song");

        let pairs: Vec<(String, String)> = url
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        for expected in [
            ("format", "json"),
            ("keyword", "晴天"),
            ("page", "1"),
            ("pagesize", "20"),
            ("showtype", "1"),
        ] {
            assert!(
                pairs
                    .iter()
                    .any(|(key, value)| key == expected.0 && value == expected.1),
                "缺少参数 {}={}：{:?}",
                expected.0,
                expected.1,
                pairs
            );
        }
    }

    #[test]
    fn page_zero_is_clamped_to_first_page() {
        let url = search_url("x", 0, 20).unwrap();
        assert!(url
            .query_pairs()
            .any(|(key, value)| key.as_ref() == "page" && value.as_ref() == "1"));
    }

    #[test]
    fn has_more_prefers_total_and_falls_back_to_page_size() {
        // 实测 total=480：第 1 页还有下一页，第 24 页刚好到底
        assert!(compute_has_more(480, 1, 20, 20));
        assert!(!compute_has_more(480, 24, 20, 20));
        // 最后一页不足一页
        assert!(!compute_has_more(15, 1, 20, 15));
        // 无 total 时退化为「本页是否装满」
        assert!(compute_has_more(0, 1, 20, 20));
        assert!(!compute_has_more(0, 1, 20, 7));
    }
}
