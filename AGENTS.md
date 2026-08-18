# AGENTS.md

## Project
- Lightweight self-service user layer for CPA: GitHub login identifies users, then the service creates or reuses CPA API keys and stores encrypted user/API-key state in SQLite.

## Commands
| Task | Command |
|------|---------|
| Format check | `cargo +nightly fmt --all -- --check` |
| Clippy | `cargo clippy --all-targets --all-features` |

## Structure
- `src/`: Axum service code for GitHub OAuth, CPA management calls, SQLite access, config, crypto, routes, and templates; it does not proxy model requests.
- `templates/`: Askama templates compiled into the Rust binary.
- `migrations/`: SQLx migrations embedded by `sqlx::migrate!("./migrations")`.
- `.github/workflows/`: CI checks and GHCR container publishing.

## Rules
- Project positioning: CPA remains the API proxy and management system; this service only handles user login, API-key self-service, and local encrypted state.
- Use nightly rustfmt; `rustfmt.toml` enables unstable formatting options.
- Configuration loads optional `config.*`, then required `CPA_PORTAL_CONFIG` when set, then `CPA_PORTAL__...` environment overrides.
- Keep `config.example.toml` in sync when adding or renaming configuration keys.
- Treat `migrations/` as a persistence boundary; edits affect existing SQLite deployments.
- Verify template field changes against `src/templates.rs`; Askama checks `templates/` at compile time.
- Do not write unit tests.
- Docker builds with `cargo build --release --locked` and runs `cpa-portal` with `/app/data` available for SQLite data.
