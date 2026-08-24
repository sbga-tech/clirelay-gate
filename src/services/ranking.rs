use std::collections::HashMap;

use serde::Serialize;
use time::OffsetDateTime;

use crate::{
    clients::keeper::{LocalLeaderboardEntry, RankingMetric, RankingPeriod},
    db::{self, User},
    error::AppResult,
    services::user::decrypt_api_key,
    state::AppState,
};

#[derive(Debug, Serialize)]
pub struct PortalLeaderboard {
    pub period: RankingPeriod,
    pub period_key: String,
    pub metric: RankingMetric,
    #[serde(with = "time::serde::rfc3339")]
    pub generated_at: OffsetDateTime,
    pub stale: bool,
    pub entries: Vec<PortalLeaderboardEntry>,
}

#[derive(Debug, Serialize)]
pub struct PortalLeaderboardEntry {
    pub rank: usize,
    pub user: PortalRankingUser,
    pub value: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_numerator: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_denominator: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<PortalLeaderboardMetrics>,
}

#[derive(Debug, Serialize)]
pub struct PortalRankingUser {
    pub id: i64,
    pub github_id: i64,
    pub github_login: String,
    pub github_name: String,
    pub avatar_url: String,
}

#[derive(Debug, Serialize)]
pub struct PortalLeaderboardMetrics {
    pub total_tokens: i64,
    pub request_count: i64,
    pub cache_read_rate: i64,
    pub ttft_average: i64,
    pub latency_average: i64,
    pub peak_tpm: i64,
    pub peak_rpm: i64,
}

pub async fn local_leaderboard(
    state: &AppState,
    period: RankingPeriod,
    metric: RankingMetric,
) -> AppResult<PortalLeaderboard> {
    let (leaderboard, key_settings) = tokio::try_join!(
        state.keeper.local_leaderboard(period, metric),
        state.keeper.cpa_api_key_settings(),
    )?;
    let mut users_by_api_key = HashMap::new();
    for user in db::list_users(&state.db).await? {
        users_by_api_key.insert(decrypt_api_key(state, &user)?, user);
    }
    let users = key_settings
        .items
        .into_iter()
        .filter_map(|key| {
            users_by_api_key
                .remove(&key.api_key)
                .map(|user| (key.id, user))
        })
        .collect::<HashMap<_, _>>();

    let entries = leaderboard
        .entries
        .into_iter()
        .filter_map(|entry| portal_entry(entry, &users))
        .enumerate()
        .map(|(index, mut entry)| {
            entry.rank = index + 1;
            entry
        })
        .collect();

    Ok(PortalLeaderboard {
        period: leaderboard.period,
        period_key: leaderboard.period_key,
        metric: leaderboard.metric,
        generated_at: leaderboard.generated_at,
        stale: leaderboard.stale,
        entries,
    })
}

fn portal_entry(
    entry: LocalLeaderboardEntry,
    users: &HashMap<String, User>,
) -> Option<PortalLeaderboardEntry> {
    let user = users.get(&entry.participant_id)?;
    Some(PortalLeaderboardEntry {
        rank: 0,
        user: PortalRankingUser {
            id: user.id,
            github_id: user.github_id,
            github_login: user.github_login.clone(),
            github_name: user.github_name.clone(),
            avatar_url: user.avatar_url.clone(),
        },
        value: entry.value,
        rate_numerator: entry.rate_numerator,
        rate_denominator: entry.rate_denominator,
        metrics: entry.metrics.map(|metrics| PortalLeaderboardMetrics {
            total_tokens: metrics.total_tokens,
            request_count: metrics.request_count,
            cache_read_rate: metrics.cache_read_rate,
            ttft_average: metrics.ttft_average,
            latency_average: metrics.latency_average,
            peak_tpm: metrics.peak_tpm,
            peak_rpm: metrics.peak_rpm,
        }),
    })
}
