# CliRelay Gate

CliRelay Gate 是给 CliRelay 增加多用户自助使用能力的轻量服务。CliRelay 本身专注代理与管理能力，没有完整用户系统；本项目用 GitHub 登录识别用户，自动为用户分配 API Key，并把密钥加密保存到本地 SQLite。

它不代理模型请求，也不替代 CliRelay。实际 API 请求、模型路由、权限配置和用量统计仍由 CliRelay 处理。

## 功能

- GitHub OAuth 登录与会话管理
- 按 GitHub 用户绑定 CliRelay API Key
- 自动调用 CliRelay Management API 创建 API Key
- 支持配置默认 permission profile 和 channel groups
- SQLite 持久化用户、API Key 和会话数据
- API Key 加密存储
- 简单的登录页与用户面板

## 部署

目前只支持 Docker 部署。推荐将 CliRelay、CliRelay Gate 放在同一个 Docker Compose 项目中，并通过 Caddy 进行反向代理：

- `/` 转发到 CliRelay Gate 首页，`/gate` 和 `/gate/*` 转发到 CliRelay Gate 的登录与会话路径
- 其他所有路径转发到 CliRelay，保留原来的兼容 API、管理接口和 Amp 路由
- CliRelay Gate 通过 Docker 内网访问 CliRelay Management API

### 1. 准备 GitHub OAuth App

在 GitHub 创建 OAuth App，并设置：

```text
Homepage URL: https://ai.example.com
Authorization callback URL: https://ai.example.com/gate/auth/github/callback
```

保存 `Client ID` 和 `Client Secret`。

### 2. 生成加密密钥

```bash
openssl rand -base64 32
```

这个值用于 `security.api_key_encryption_key`。丢失后，数据库中已加密的 API Key 无法恢复。

### 3. 配置 CliRelay Gate

创建 `clirelay-gate.toml`：

```toml
[server]
listen = "0.0.0.0:8080"
public_base_url = "https://ai.example.com"
site_name = "CliRelay Gate"

[server.session]
cookie_name = "clirelay_gate"
secure = true
ttl_seconds = 2592000

[github]
client_id = "YOUR_GITHUB_CLIENT_ID"
client_secret = "YOUR_GITHUB_CLIENT_SECRET"

[clirelay]
public_base_url = "https://ai.example.com"
internal_base_url = "http://cli-proxy-api:8317"
management_key = "YOUR_CLIRELAY_MANAGEMENT_KEY"
default_permission_profile_id = ""
default_allowed_channel_groups = []

[database]
url = "sqlite://./data/clirelay-gate.db"

[security]
api_key_encryption_key = "YOUR_BASE64_32_BYTE_KEY"
api_key_prefix = "sk-ghu-"
```

也可以用环境变量覆盖配置，例如 `CLIRELAY_GATE__SERVER__PUBLIC_BASE_URL`。

### 4. 配置 Caddy

创建 `Caddyfile`：

```caddyfile
ai.example.com {
	@gate_paths path / /gate /gate/*
	handle @gate_paths {
		reverse_proxy clirelay-gate:8080
	}

	handle {
		reverse_proxy cli-proxy-api:8317
	}
}
```

### 5. 配置 Docker Compose

创建 `compose.yaml`：

```yaml
name: clirelay

services:
  caddy:
    image: caddy:2
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile:ro
      - caddy-data:/data
      - caddy-config:/config
    depends_on:
      - cli-proxy-api
      - clirelay-gate

  cli-proxy-api:
    image: ghcr.io/kittors/clirelay:latest
    restart: unless-stopped
    expose:
      - "8317"
    volumes:
      - ./clirelay.yaml:/CLIProxyAPI/config.yaml:ro
      - ./auths:/CLIProxyAPI/auths
      - ./logs:/CLIProxyAPI/logs
      - ./clirelay-data:/CLIProxyAPI/data

  clirelay-gate:
    image: ghcr.io/sbga-tech/clirelay-gate:latest
    restart: unless-stopped
    expose:
      - "8080"
    volumes:
      - ./clirelay-gate.toml:/app/config.toml:ro
      - ./clirelay-gate-data:/app/data

volumes:
  caddy-data:
  caddy-config:
```

CliRelay 的配置步骤这里略去，请参考 CliRelay 官方文档。

### 6. 启动服务

```bash
docker compose pull
docker compose up -d
```

访问 `https://ai.example.com/` 登录。API 客户端继续使用同一个域名访问 CliRelay。

## 相关链接

- CliRelay Gate 镜像：<https://github.com/sbga-tech/clirelay-gate/pkgs/container/clirelay-gate>
- CliRelay：<https://github.com/kittors/CliRelay>
- GitHub OAuth App 文档：<https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/creating-an-oauth-app>
- Caddy reverse_proxy：<https://caddyserver.com/docs/caddyfile/directives/reverse_proxy>
