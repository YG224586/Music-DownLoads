//! 可独立运行的下载服务。默认监听本机；容器部署时同时提供 Web 页面和任务 API。

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

use hotdownloader_server::auth::AccessAuth;
use hotdownloader_server::http;
use hotdownloader_server::logging;
use hotdownloader_server::runtime::ServerRuntime;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    logging::initialize();
    // 数据目录和绑定地址由部署环境指定。对外监听时必须配置一种访问认证。
    let data_dir = std::env::var_os("HOTDOWNLOADER_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./data"));
    let bind = std::env::var("HOTDOWNLOADER_BIND").unwrap_or_else(|_| "127.0.0.1:8787".to_string());
    let address: SocketAddr = bind.parse()?;
    let auth = AccessAuth::from_env()?;
    if !address.ip().is_loopback() && !auth.valid_for_external_access() {
        return Err("对外监听时须同时设置 AUTH_USERNAME 和 AUTH_PASSWORD，或设置至少 16 个字符的 HOTDOWNLOADER_TOKEN".into());
    }
    let runtime = ServerRuntime::start(data_dir, auth).map_err(std::io::Error::other)?;
    let listener = TcpListener::bind(address).await?;
    log::info!("任务服务已监听 http://{bind}");

    loop {
        let (stream, _) = listener.accept().await?;
        let runtime: Arc<ServerRuntime> = runtime.clone();
        tokio::spawn(async move {
            let service = service_fn(move |request| http::handle(request, runtime.clone()));
            if let Err(error) = http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .await
            {
                log::warn!("HTTP 连接结束: {error}");
            }
        });
    }
}
