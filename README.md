# CPA Portal

CPA Portal 为 CPA 增加基于 GitHub 登录的多用户自助服务。
它为每位用户创建并保存一个加密的 CPA API Key，并提供登录页和用户面板。
模型请求、路由和管理功能仍由 CPA 处理；CPA Portal 不代理模型请求。

## 功能

- GitHub OAuth 登录与会话管理
- 按 GitHub 用户绑定 CPA API Key
- 通过 CPA Management API 创建 API Key
- SQLite 持久化用户、API Key 和 OAuth 状态
- ChaCha20-Poly1305 加密存储 API Key
- 同源登录 CPA Usage Keeper

## 路由

CPA Portal 只注册以下路由：

- `GET /`：登录页或用户面板
- `GET /auth/github/start`：开始 GitHub OAuth 登录
- `GET /auth/github/callback`：处理 GitHub OAuth 回调
- `POST /logout`：退出当前会话

`/usage` 和 `/usage/*` 应转发到 CPA Usage Keeper，其余路径应转发到 CPA。

## 配置

复制 `config.example.toml`，或创建 `cpa-portal.toml`：

```toml
[server]
listen = "0.0.0.0:8080"
public_base_url = "https://ai.example.com"
site_name = "CPA Portal"

[server.session]
cookie_name = "cpa_portal"
secure = true
ttl_seconds = 2592000

[github]
client_id = "YOUR_GITHUB_CLIENT_ID"
client_secret = "YOUR_GITHUB_CLIENT_SECRET"

[cpa]
public_base_url = "https://ai.example.com"
internal_base_url = "http://cpa:8317"
management_key = "YOUR_CPA_MANAGEMENT_KEY"

[database]
url = "sqlite://./data/cpa-portal.db"

[security]
api_key_encryption_key = "YOUR_BASE64_32_BYTE_KEY"
api_key_prefix = "sk-ghu-"
```

`security.api_key_encryption_key` 必须是 32 字节随机值的 Base64 或 Base64URL 编码。
丢失该值后，数据库中已加密的 API Key 无法恢复。

```bash
openssl rand -base64 32
```

默认加载可选的 `config.*`。
设置 `CPA_PORTAL_CONFIG` 可指定必需的配置文件，环境变量 `CPA_PORTAL__...` 可覆盖配置项，例如：

```text
CPA_PORTAL_CONFIG=/app/config.toml
CPA_PORTAL__SERVER__PUBLIC_BASE_URL=https://ai.example.com
CPA_PORTAL__CPA__INTERNAL_BASE_URL=http://cpa:8317
```

## GitHub OAuth App

在 GitHub 创建 OAuth App，并设置：

```text
Homepage URL: https://ai.example.com
Authorization callback URL: https://ai.example.com/auth/github/callback
```

## Docker

镜像通过当前 GitHub 仓库名发布到 GHCR：

```yaml
services:
  cpa-portal:
    image: ghcr.io/sbga-tech/cpa-portal:latest
    restart: unless-stopped
    expose:
      - "8080"
    environment:
      CPA_PORTAL_CONFIG: /app/config.toml
    volumes:
      - ./cpa-portal.toml:/app/config.toml:ro
      - ./cpa-portal-data:/app/data
```

## 反向代理

CPA Portal、CPA Usage Keeper 和 CPA 应位于同一站点下。
以下 Caddy 配置让 Portal 处理登录路由，让 Keeper 处理用量页面，并将其他请求交给 CPA：

```caddyfile
ai.example.com {
	@portal path / /auth/* /logout
	handle @portal {
		reverse_proxy cpa-portal:8080
	}

	@usage path /usage /usage/*
	handle @usage {
		reverse_proxy cpa-usage-keeper:8080
	}

	handle {
		reverse_proxy cpa:8317
	}
}
```

用户在面板点击 **Query usages** 后，浏览器会向
`/usage/api/v1/auth/api-key-login` 发送同源 `POST` 请求。
登录成功后，浏览器跳转到 `/usage/`；API Key 不会出现在 URL 中。

## 相关链接

- CPA Portal 镜像：<https://github.com/sbga-tech/cpa-portal/pkgs/container/cpa-portal>
- GitHub OAuth App 文档：<https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/creating-an-oauth-app>
- Caddy `reverse_proxy`：<https://caddyserver.com/docs/caddyfile/directives/reverse_proxy>
