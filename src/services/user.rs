use anyhow::anyhow;
use tower_sessions::Session;

use crate::{
    clients::{clirelay::CliRelayApiKeyEntry, github},
    crypto,
    db::{self, ApiKeyUpdate, GitHubProfileUpdate, NewUser, User},
    error::{AppError, AppResult},
    state::AppState,
};

const SESSION_USER_ID: &str = "user_id";
const MAX_RECONCILE_ATTEMPTS: usize = 3;

pub async fn current_user(state: &AppState, session: &Session) -> AppResult<Option<User>> {
    let Some(user_id) = session.get::<i64>(SESSION_USER_ID).await? else {
        return Ok(None);
    };

    let Some(user) = db::get_user_by_id(&state.db, user_id).await? else {
        session.flush().await?;
        return Ok(None);
    };

    Ok(Some(reconcile_api_key(state, user).await?))
}

pub async fn remember_current_user(session: &Session, user_id: i64) -> AppResult<()> {
    session.insert(SESSION_USER_ID, user_id).await?;
    Ok(())
}

pub fn decrypt_api_key(state: &AppState, user: &User) -> AppResult<Option<String>> {
    match (&user.api_key_ciphertext, &user.api_key_nonce) {
        (Some(ciphertext), Some(nonce)) => state
            .crypto
            .decrypt(ciphertext, nonce)
            .map(Some)
            .map_err(AppError::from),
        (None, None) => Ok(None),
        _ => Err(AppError::Other(anyhow!(
            "stored API key ciphertext and nonce are inconsistent"
        ))),
    }
}

pub async fn reconcile_api_key(state: &AppState, mut user: User) -> AppResult<User> {
    for _ in 0..MAX_RECONCILE_ATTEMPTS {
        let entries = state.clirelay.api_key_entries().await?;
        let current_key = decrypt_api_key(state, &user)?;
        let remote = matching_remote_entry(&user, current_key.as_deref(), &entries)?;

        if let Some(entry) = remote {
            validate_remote_entry(entry)?;
            if user.clirelay_api_key_id.as_deref() == Some(entry.id.as_str())
                && current_key.as_deref() == Some(entry.key.as_str())
            {
                return Ok(user);
            }
        } else if user.clirelay_api_key_id.is_none() && current_key.is_none() {
            return Ok(user);
        }

        let encrypted = remote
            .map(|entry| state.crypto.encrypt(&entry.key))
            .transpose()?;
        let update = ApiKeyUpdate {
            clirelay_api_key_id: remote.map(|entry| entry.id.as_str()),
            ciphertext: encrypted
                .as_ref()
                .map(|secret| secret.ciphertext.as_slice()),
            nonce: encrypted.as_ref().map(|secret| secret.nonce.as_slice()),
        };
        if let Some(updated) =
            db::compare_and_set_user_api_key(&state.db, user.id, user.api_key_version, update)
                .await?
        {
            return Ok(updated);
        }

        user = db::get_user_by_id(&state.db, user.id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
    }

    Err(AppError::Other(anyhow!(
        "API key changed concurrently during reconciliation"
    )))
}

pub async fn create_api_key(state: &AppState, user: User) -> AppResult<User> {
    if decrypt_api_key(state, &user)?.is_some() {
        return Ok(user);
    }

    let api_key = crypto::generate_api_key(state.config.security.api_key_prefix.as_ref())?;
    let entry = state
        .clirelay
        .provision_api_key(&api_key, &api_key_name(&user.github_login, user.github_id))
        .await?;
    store_api_key(state, user.id, &entry.id, &entry.key).await
}

pub async fn rotate_api_key(
    state: &AppState,
    user: User,
    expected_key_hash: &str,
) -> AppResult<User> {
    let (id, _) = require_expected_api_key(state, &user, expected_key_hash)?;
    let api_key = crypto::generate_api_key(state.config.security.api_key_prefix.as_ref())?;
    state.clirelay.rotate_api_key(id, &api_key).await?;
    store_api_key(state, user.id, id, &api_key).await
}

pub async fn delete_api_key(
    state: &AppState,
    user: User,
    expected_key_hash: &str,
) -> AppResult<User> {
    let (id, _) = require_expected_api_key(state, &user, expected_key_hash)?;
    state.clirelay.delete_api_key(id).await?;
    Ok(db::set_user_api_key(
        &state.db,
        user.id,
        ApiKeyUpdate {
            clirelay_api_key_id: None,
            ciphertext: None,
            nonce: None,
        },
    )
    .await?)
}

fn matching_remote_entry<'a>(
    user: &User,
    current_key: Option<&str>,
    entries: &'a [CliRelayApiKeyEntry],
) -> AppResult<Option<&'a CliRelayApiKeyEntry>> {
    if let Some(id) = user.clirelay_api_key_id.as_deref() {
        return Ok(entries.iter().find(|entry| entry.id == id));
    }
    if let Some(api_key) = current_key
        && let Some(entry) = entries.iter().find(|entry| entry.key == api_key)
    {
        return Ok(Some(entry));
    }

    let name = api_key_name(&user.github_login, user.github_id);
    let mut matches = entries.iter().filter(|entry| entry.name == name);
    let first = matches.next();
    if matches.next().is_some() {
        return Err(AppError::Upstream(format!(
            "multiple CliRelay API keys match {name}"
        )));
    }
    Ok(first)
}

fn validate_remote_entry(entry: &CliRelayApiKeyEntry) -> AppResult<()> {
    if entry.id.trim().is_empty() || entry.key.trim().is_empty() {
        return Err(AppError::Upstream(
            "CliRelay returned an API key without a stable identity or value".into(),
        ));
    }
    Ok(())
}

fn require_expected_api_key<'a>(
    state: &AppState,
    user: &'a User,
    expected_key_hash: &str,
) -> AppResult<(&'a str, String)> {
    let id = user.clirelay_api_key_id.as_deref().ok_or_else(|| {
        AppError::BadRequest("API key no longer exists; reload the dashboard".into())
    })?;
    let api_key = decrypt_api_key(state, user)?.ok_or_else(|| {
        AppError::BadRequest("API key no longer exists; reload the dashboard".into())
    })?;
    if crypto::hash_secret(&api_key) != expected_key_hash {
        return Err(AppError::BadRequest(
            "API key changed; reload the dashboard before trying again".into(),
        ));
    }
    Ok((id, api_key))
}

async fn store_api_key(
    state: &AppState,
    user_id: i64,
    clirelay_api_key_id: &str,
    api_key: &str,
) -> AppResult<User> {
    if clirelay_api_key_id.trim().is_empty() || api_key.trim().is_empty() {
        return Err(AppError::Upstream(
            "CliRelay returned an invalid API key".into(),
        ));
    }
    let encrypted = state.crypto.encrypt(api_key)?;
    Ok(db::set_user_api_key(
        &state.db,
        user_id,
        ApiKeyUpdate {
            clirelay_api_key_id: Some(clirelay_api_key_id),
            ciphertext: Some(&encrypted.ciphertext),
            nonce: Some(&encrypted.nonce),
        },
    )
    .await?)
}

fn api_key_name(github_login: &str, github_id: i64) -> String {
    format!("github:{github_login}#{github_id}")
}

pub async fn find_or_create_user(
    state: &AppState,
    identity: github::GitHubIdentity,
) -> AppResult<User> {
    let _mutation_guard = state.api_key_mutations.lock().await;
    if find_existing_user(state, identity.id).await?.is_some() {
        return update_existing_user(state, identity).await;
    }

    create_user(state, identity).await
}

async fn find_existing_user(state: &AppState, github_id: i64) -> AppResult<Option<User>> {
    Ok(db::get_user_by_github_id(&state.db, github_id).await?)
}

async fn update_existing_user(
    state: &AppState,
    identity: github::GitHubIdentity,
) -> AppResult<User> {
    let update = GitHubProfileUpdate {
        github_id: identity.id,
        github_login: identity.login,
        github_name: identity.name,
        github_email: identity.email,
        avatar_url: identity.avatar_url,
    };

    Ok(db::update_existing_user(&state.db, &update).await?)
}

async fn create_user(state: &AppState, identity: github::GitHubIdentity) -> AppResult<User> {
    let github_id = identity.id;
    let api_key = crypto::generate_api_key(state.config.security.api_key_prefix.as_ref())?;
    let entry = state
        .clirelay
        .provision_api_key(&api_key, &api_key_name(&identity.login, github_id))
        .await?;
    let encrypted = match state.crypto.encrypt(&entry.key) {
        Ok(encrypted) => encrypted,
        Err(error) => {
            cleanup_provisioned_api_key(state, &entry.id).await;
            return Err(AppError::from(error));
        },
    };
    let new_user = NewUser {
        github_id,
        github_login: identity.login,
        github_name: identity.name,
        github_email: identity.email,
        avatar_url: identity.avatar_url,
        api_key_ciphertext: encrypted.ciphertext,
        api_key_nonce: encrypted.nonce,
        clirelay_api_key_id: entry.id,
    };

    insert_user_with_race_retry(state, new_user, github_id).await
}

async fn insert_user_with_race_retry(
    state: &AppState,
    new_user: NewUser,
    github_id: i64,
) -> AppResult<User> {
    match db::insert_user(&state.db, &new_user).await {
        Ok(user) => Ok(user),
        Err(sqlx::Error::Database(err)) if err.is_unique_violation() => {
            cleanup_provisioned_api_key(state, &new_user.clirelay_api_key_id).await;
            db::get_user_by_github_id(&state.db, github_id)
                .await?
                .ok_or(sqlx::Error::RowNotFound)
                .map_err(AppError::from)
        },
        Err(err) => {
            cleanup_provisioned_api_key(state, &new_user.clirelay_api_key_id).await;
            Err(AppError::from(err))
        },
    }
}

async fn cleanup_provisioned_api_key(state: &AppState, clirelay_api_key_id: &str) {
    if let Err(error) = state.clirelay.delete_api_key(clirelay_api_key_id).await {
        tracing::warn!(
            error = %error,
            "failed to roll back CliRelay API key after local user creation failure"
        );
    }
}
