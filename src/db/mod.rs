use std::{path::PathBuf, str::FromStr};

use anyhow::{Context, Result};
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use time::OffsetDateTime;

use crate::crypto::hash_secret;

#[derive(Debug, Clone)]
pub struct User {
    pub id: i64,
    pub github_id: i64,
    pub github_login: String,
    pub github_name: String,
    pub github_email: String,
    pub avatar_url: String,
    pub api_key_ciphertext: Vec<u8>,
    pub api_key_nonce: Vec<u8>,
    pub created_at: i64,
    pub last_login_at: i64,
}

#[derive(Debug, Clone)]
pub struct NewUser {
    pub github_id: i64,
    pub github_login: String,
    pub github_name: String,
    pub github_email: String,
    pub avatar_url: String,
    pub api_key_ciphertext: Vec<u8>,
    pub api_key_nonce: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct GitHubProfileUpdate {
    pub github_id: i64,
    pub github_login: String,
    pub github_name: String,
    pub github_email: String,
    pub avatar_url: String,
}

pub async fn connect(database_url: &str) -> Result<SqlitePool> {
    if let Some(path) = sqlite_file_path(database_url)
        && let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
    {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("create database directory {}", parent.display()))?;
    }

    let options = SqliteConnectOptions::from_str(database_url)
        .with_context(|| format!("parse database URL {database_url}"))?
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .with_context(|| format!("connect database {database_url}"))?;

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

fn sqlite_file_path(database_url: &str) -> Option<PathBuf> {
    let without_query = database_url
        .split_once('?')
        .map_or(database_url, |(left, _)| left);
    let path = without_query
        .strip_prefix("sqlite://")
        .or_else(|| without_query.strip_prefix("sqlite:"))?;
    if path == ":memory:" || path.is_empty() {
        return None;
    }
    Some(PathBuf::from(path))
}

pub async fn get_user_by_id(pool: &SqlitePool, id: i64) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query("SELECT * FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    row.map(row_to_user).transpose()
}

pub async fn get_user_by_github_id(
    pool: &SqlitePool,
    github_id: i64,
) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query("SELECT * FROM users WHERE github_id = ?")
        .bind(github_id)
        .fetch_optional(pool)
        .await?;
    row.map(row_to_user).transpose()
}

pub async fn list_users(pool: &SqlitePool) -> Result<Vec<User>, sqlx::Error> {
    let rows = sqlx::query("SELECT * FROM users ORDER BY github_login")
        .fetch_all(pool)
        .await?;

    rows.into_iter().map(row_to_user).collect()
}

pub async fn insert_user(pool: &SqlitePool, user: NewUser) -> Result<User, sqlx::Error> {
    let now = now_unix();
    let result = sqlx::query(
        r#"INSERT INTO users (
            github_id, github_login, github_name, github_email, avatar_url,
            api_key_ciphertext, api_key_nonce, created_at, last_login_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user.github_id)
    .bind(user.github_login)
    .bind(user.github_name)
    .bind(user.github_email)
    .bind(user.avatar_url)
    .bind(user.api_key_ciphertext)
    .bind(user.api_key_nonce)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    get_user_by_id(pool, result.last_insert_rowid())
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn update_existing_user(
    pool: &SqlitePool,
    update: &GitHubProfileUpdate,
) -> Result<User, sqlx::Error> {
    let now = now_unix();
    sqlx::query(
        r#"UPDATE users
           SET github_login = ?, github_name = ?, github_email = ?, avatar_url = ?,
               last_login_at = ?
           WHERE github_id = ?"#,
    )
    .bind(&update.github_login)
    .bind(&update.github_name)
    .bind(&update.github_email)
    .bind(&update.avatar_url)
    .bind(now)
    .bind(update.github_id)
    .execute(pool)
    .await?;

    get_user_by_github_id(pool, update.github_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn delete_expired_oauth_states(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM oauth_states WHERE expires_at <= ?")
        .bind(now_unix())
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub async fn insert_oauth_state(
    pool: &SqlitePool,
    state: &str,
    pkce_verifier: &str,
    ttl_seconds: i64,
) -> Result<(), sqlx::Error> {
    let now = now_unix();
    sqlx::query(
        "INSERT INTO oauth_states (state_hash, pkce_verifier, expires_at) VALUES (?, ?, ?)",
    )
    .bind(hash_secret(state))
    .bind(pkce_verifier)
    .bind(now + ttl_seconds)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn consume_oauth_state(
    pool: &SqlitePool,
    state: &str,
) -> Result<Option<String>, sqlx::Error> {
    let hash = hash_secret(state);
    let mut tx = pool.begin().await?;
    let row = sqlx::query(
        "SELECT pkce_verifier FROM oauth_states WHERE state_hash = ? AND expires_at > ?",
    )
    .bind(&hash)
    .bind(now_unix())
    .fetch_optional(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM oauth_states WHERE state_hash = ?")
        .bind(&hash)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    row.map(|row| row.try_get("pkce_verifier")).transpose()
}

fn now_unix() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

fn row_to_user(row: sqlx::sqlite::SqliteRow) -> Result<User, sqlx::Error> {
    Ok(User {
        id: row.try_get("id")?,
        github_id: row.try_get("github_id")?,
        github_login: row.try_get("github_login")?,
        github_name: row.try_get("github_name")?,
        github_email: row.try_get("github_email")?,
        avatar_url: row.try_get("avatar_url")?,
        api_key_ciphertext: row.try_get("api_key_ciphertext")?,
        api_key_nonce: row.try_get("api_key_nonce")?,
        created_at: row.try_get("created_at")?,
        last_login_at: row.try_get("last_login_at")?,
    })
}
