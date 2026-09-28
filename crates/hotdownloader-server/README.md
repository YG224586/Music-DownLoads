# HotDownloader 独立服务

此 crate 组装共享 Rust 核心，管理任务队列、下载文件、QQ 凭据和进度广播。
浏览器重新连接时可获取当前任务快照，继续查看进度。

## Docker/Web 部署

### 镜像

- 地址：`ghcr.io/lerdb/hotdownloader`
- 架构：`linux/amd64`
- `latest`：随 `main` 分支发布更新
- 版本标签：`vX.Y.Z`、`X.Y.Z`、`X.Y` 和 `sha-<短提交号>`

### 启动容器

创建 `.env`，配置用户名和密码，以及容器内下载目录：

```dotenv
AUTH_USERNAME=admin
AUTH_PASSWORD=请替换为密码
HOTDOWNLOADER_DOWNLOAD_DIR=/downloads
```

拉取镜像并启动服务：

```bash
docker pull ghcr.io/lerdb/hotdownloader:latest
docker run -d --name hotdownloader --restart unless-stopped \
  --env-file .env -p 127.0.0.1:8787:8787 \
  -v hotdownloader-data:/data \
  -v hotdownloader-downloads:/downloads \
  ghcr.io/lerdb/hotdownloader:latest
```

在服务器本机访问 `http://127.0.0.1:8787`，输入 `.env` 中的用户名和密码。
也可只设置 `HOTDOWNLOADER_TOKEN`（至少 16 个字符），继续使用令牌登录。
若同时设置账号密码和令牌，服务只接受账号密码。
Docker 仅将端口绑定到宿主机回环地址，容器内服务仍监听 `0.0.0.0:8787`，以便端口映射正常工作。

从其他设备访问时，请在宿主机配置 HTTPS 反向代理，将请求转发到 `127.0.0.1:8787`，并通过 HTTPS 域名打开网页。
不要将访问凭据通过公网明文 HTTP 传输。

下载任务由服务进程持续执行。重新打开网页即可查看进度。
页面顶部显示服务连接状态和最近响应时间。

### 数据与下载目录

容器使用两个独立的卷：

- `hotdownloader-data` 挂载到 `/data`，保存设置、QQ 凭据和任务记录。
- `hotdownloader-downloads` 挂载到 `/downloads`，保存新下载的文件。

如需使用宿主机目录，将第二个 `-v` 改为：

```text
-v /srv/music:/downloads
```

`HOTDOWNLOADER_DOWNLOAD_DIR` 应与容器内的下载目录挂载目标一致。

从旧版 Docker 部署升级时，已有下载文件仍位于 `hotdownloader-data` 卷中的 `/data/downloads`，已有任务记录也继续引用该路径。
新任务将写入独立的下载卷。请保留原数据卷，以便访问历史文件。

### 使用 Compose

使用仓库根目录的 [`compose.yaml`](../../compose.yaml)。
在该目录保存 `.env` 后运行：

```bash
docker compose pull
docker compose up -d
```

Compose 默认使用独立的 `hotdownloader-downloads` 卷。
如需改用宿主机目录，在 `.env` 中加入：

```dotenv
HOTDOWNLOADER_DOWNLOAD_MOUNT=/srv/music
```

`HOTDOWNLOADER_DOWNLOAD_MOUNT` 控制挂载源。
如需更改容器内路径，再将 `HOTDOWNLOADER_DOWNLOAD_DIR` 设为绝对路径，Compose 会用它作为挂载目标。

### 更新镜像

Compose 部署：

```bash
docker compose pull
docker compose up -d
```

`docker run` 部署：拉取镜像，删除旧容器，再运行上面的启动命令。

```bash
docker pull ghcr.io/lerdb/hotdownloader:latest
docker rm -f hotdownloader
```

命名卷保留已有数据。

### 任务恢复与访问安全

服务进程重启后，先前未完成的任务显示在“已中断”标签中。
点击恢复后，任务重新入队。

远程访问请使用上述 HTTPS 反向代理，保护浏览器与服务之间的访问凭据。
当前部署使用一组账号密码或单个访问令牌，适合单管理员使用。

## 本机运行与配置

从仓库根目录运行：

```bash
cargo run --manifest-path crates/hotdownloader-server/Cargo.toml
```

| 环境变量 | 用途与默认值 |
| --- | --- |
| `HOTDOWNLOADER_DATA_DIR` | 数据目录，默认 `./data`。 |
| `HOTDOWNLOADER_BIND` | 监听地址，默认 `127.0.0.1:8787`。 |
| `AUTH_USERNAME`、`AUTH_PASSWORD` | 对外监听时可同时设置，启用账号密码认证。`/api` 请求使用 HTTP Basic 认证；两者优先于令牌。不允许只设置其中一个，用户名不能包含冒号。 |
| `HOTDOWNLOADER_TOKEN` | 未设置账号密码时，对外监听需设置至少 16 个字符的访问令牌。`/api` 请求使用 `Authorization: Bearer <token>`。 |
| `HOTDOWNLOADER_WEB_DIR` | 前端构建产物目录，默认 `./dist`。 |
| `HOTDOWNLOADER_DOWNLOAD_DIR` | 下载文件的绝对目录。本机或直接使用镜像时默认是数据目录下的 `downloads`；仓库的 Compose 配置默认是 `/downloads`。 |
| `HOTDOWNLOADER_LOG_LEVEL` | 日志级别，可设为 `off`、`error`、`warn`、`info`、`debug` 或 `trace`，默认 `info`。 |

数据目录包含：

- `settings.json`：下载设置
- `qq-credentials.json`：QQ 登录凭据
- `tasks.json`：任务记录

首次启动使用默认设置和空任务列表。
请持久化整个数据目录并限制其访问权限，妥善保护 QQ 凭据和访问令牌。

## 服务接口

服务接口沿用共享核心的 `camelCase` JSON 契约。

### 任务与事件

- `GET /api/tasks`：当前完整任务快照。
- `POST /api/tasks`：提交 `CreateTaskRequest`，成功后返回 `CreateTaskResult`。
- `POST /api/tasks/{id}/pause|resume|retry|cancel|remove`：控制单个任务。取消和删除可传 `{"deleteFile":true}`。
- `POST /api/tasks/remove`：批量删除，传 `{"taskIds":["..."],"deleteFile":false}`。
- `GET /api/events`：SSE。连接后先发送任务与设置快照，再推送任务更新、设置更新和心跳。重连时通过快照同步视图。

事件类型包括 `task-updated`、`task-removed` 和 `settings-updated`。

### 设置与健康检查

- `GET /api/settings`：设置快照。
- `PATCH /api/settings`：字段级合并更新。提交 `changes` 与各字段的 `expected` 原值；同字段冲突返回 HTTP 409 和最新快照。
- `GET /api/settings/default-download-dir`：服务器下载目录。
- `GET /healthz`：服务健康检查，HTTP 服务可响应时返回 HTTP 200 与 `{"status":"ok"}`。

### 音乐与登录

- `POST /api/music/{action}`：查询歌曲、歌单、专辑、歌手、热搜、建议、封面和歌词。参数及结果沿用前端 API 契约。
- QQ 登录状态与凭据操作：
  - `GET /api/login/status`
  - `POST /api/login/qr`
  - `GET /api/login/qr/{id}`
  - `POST /api/login/manual`
  - `POST /api/login/logout`

静态前端资源由同一进程提供。
Web 端输入访问令牌后，通过 HTTP 操作任务，通过 SSE 接收快照和进度。
服务进程管理任务生命周期；进程重启后，未完成的任务进入 `interrupted` 状态，等待手动恢复。

## 运行状态与日志

Compose 已配置 `/healthz` 健康检查。查看 Compose 服务状态与日志：

```bash
docker compose ps
docker compose logs -f hotdownloader
```

使用 `docker run` 部署时查看容器与日志：

```bash
docker ps
docker logs -f hotdownloader
```

日志以单行 JSON 写入标准错误输出，包含时间、级别、模块和消息。
任务进度与结果也可在网页任务列表中查看。

## 本机前端联调

本机联调时，在两个终端分别运行服务与前端。

终端一：

```bash
cargo run --manifest-path crates/hotdownloader-server/Cargo.toml
```

终端二：

```bash
npm run dev
```

Vite 将浏览器的 `/api` 请求转发到本机 `8787` 端口。
前端生产构建由同一 Rust 进程提供。
