//! 哔哩哔哩取直链：`view`(bvid→cid) → `playurl`(fnval=16) → DASH 音轨。

use super::{fetch_buvid_cookie, http_get_json_with};

/// 取下载直链：`song_mid` = bvid。
///
/// 链路：`view`(bvid → cid，防占位数据校验) → `playurl(fnval=16)` →
/// `dash.audio` 选 id=30280（192kbps AAC）→ `baseUrl` 即直链。
/// B 站 DASH 音轨不加密，key 恒为空串。
pub async fn get_download_link(
    client: &reqwest::Client,
    song_mid: &str,
    filename: &str,
) -> Result<(String, String), String> {
    let _ = filename; // B 站只有一个音质档（192kaac.m4a），filename 不参与选轨
    let bvid = song_mid.trim();
    if bvid.is_empty() {
        return Err("歌曲标识为空".to_string());
    }
    let cookie = fetch_buvid_cookie().await?;

    // 1. view 拿 cid。注意 B 站对无效 bvid 会返回 code=0 + 占位 data（aid=2、
    //    pic=transparent.png），必须校验 cid > 0 才认。
    let view_url = format!(
        "https://api.bilibili.com/x/web-interface/view?bvid={}",
        bvid
    );
    let view = http_get_json_with(client, &view_url, &cookie).await?;
    if view["code"].as_i64() != Some(0) {
        return Err(format!(
            "获取视频信息失败: {}",
            view["message"].as_str().unwrap_or("未知错误")
        ));
    }
    let cid = view["data"]["cid"].as_i64().unwrap_or(0);
    if cid <= 0 {
        return Err("视频不存在或已下架".to_string());
    }

    // 2. playurl 取 DASH 音轨。
    let play_url = format!(
        "https://api.bilibili.com/x/player/playurl?bvid={}&cid={}&qn=0&fnval=16",
        bvid, cid
    );
    let play = http_get_json_with(client, &play_url, &cookie).await?;
    if play["code"].as_i64() != Some(0) {
        return Err(format!(
            "获取播放地址失败: {}",
            play["message"].as_str().unwrap_or("未知错误")
        ));
    }
    let audio = play["data"]["dash"]["audio"]
        .as_array()
        .ok_or("解析响应失败: 响应中没有 DASH 音轨（可能是纯图文或已下架内容）")?;
    let track = audio
        .iter()
        .find(|a| a["id"].as_i64() == Some(30_280))
        .ok_or("该视频没有 192kbps 音轨，暂不支持下载")?;
    let base_url = track["baseUrl"]
        .as_str()
        .ok_or("解析响应失败: 音轨缺少 baseUrl")?;
    Ok((base_url.to_string(), String::new()))
}
