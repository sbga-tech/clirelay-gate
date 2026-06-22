use std::{cmp::Ordering, collections::HashMap};

use serde::{Deserialize, Deserializer};

use crate::{db::User, error::AppResult, services::user::decrypt_api_key, state::AppState};

#[derive(Debug, Deserialize, Default)]
pub struct LeaderboardQuery {
    #[serde(default)]
    pub period: LeaderboardPeriod,
    #[serde(default)]
    pub metric: LeaderboardMetric,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum LeaderboardPeriod {
    Today,
    #[default]
    SevenDays,
    ThirtyDays,
}

impl<'de> Deserialize<'de> for LeaderboardPeriod {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "today" => Self::Today,
            "7d" => Self::SevenDays,
            "30d" => Self::ThirtyDays,
            _ => Self::default(),
        })
    }
}

impl LeaderboardPeriod {
    fn all() -> [Self; 3] {
        [Self::Today, Self::SevenDays, Self::ThirtyDays]
    }

    fn days(self) -> u16 {
        match self {
            Self::Today => 1,
            Self::SevenDays => 7,
            Self::ThirtyDays => 30,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::SevenDays => "Last 7 days",
            Self::ThirtyDays => "Last 30 days",
        }
    }

    fn short_label(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::SevenDays => "7 days",
            Self::ThirtyDays => "30 days",
        }
    }

    fn value(self) -> &'static str {
        match self {
            Self::Today => "today",
            Self::SevenDays => "7d",
            Self::ThirtyDays => "30d",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum LeaderboardMetric {
    #[default]
    Tokens,
    Requests,
}

impl<'de> Deserialize<'de> for LeaderboardMetric {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "tokens" => Self::Tokens,
            "requests" => Self::Requests,
            _ => Self::default(),
        })
    }
}

impl LeaderboardMetric {
    fn all() -> [Self; 2] {
        [Self::Tokens, Self::Requests]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Tokens => "Tokens",
            Self::Requests => "Requests",
        }
    }

    fn value(self) -> &'static str {
        match self {
            Self::Tokens => "tokens",
            Self::Requests => "requests",
        }
    }
}

#[derive(Debug)]
pub struct LeaderboardView {
    pub rows: Vec<LeaderboardRow>,
    pub period: LeaderboardPeriod,
    pub metric: LeaderboardMetric,
    pub period_choices: Vec<LeaderboardChoice>,
    pub metric_choices: Vec<LeaderboardChoice>,
}

#[derive(Debug)]
pub struct LeaderboardChoice {
    pub value: &'static str,
    pub label: &'static str,
    pub selected: bool,
}

#[derive(Debug)]
pub struct LeaderboardRow {
    pub rank: usize,
    pub github_login: String,
    pub github_name: String,
    pub avatar_url: String,
    pub requests: i64,
    pub tokens: i64,
}

impl LeaderboardRow {
    pub fn requests_label(&self) -> String {
        format_integer(self.requests)
    }

    pub fn tokens_label(&self) -> String {
        format_integer(self.tokens)
    }
}

#[derive(Debug, Clone, Copy)]
struct UsageTotals {
    requests: i64,
    tokens: i64,
}

pub async fn load_leaderboard(
    state: &AppState,
    period: LeaderboardPeriod,
    metric: LeaderboardMetric,
) -> AppResult<LeaderboardView> {
    let usage = state.clirelay.usage_chart_data(period.days()).await?;
    let users = crate::db::list_users(&state.db).await?;
    let usage_by_key = usage
        .apikey_distribution
        .into_iter()
        .map(|row| {
            (
                row.api_key,
                UsageTotals {
                    requests: row.requests,
                    tokens: row.tokens,
                },
            )
        })
        .collect::<HashMap<_, _>>();

    let mut rows = users
        .into_iter()
        .filter_map(|user| row_for_user(state, user, &usage_by_key).transpose())
        .collect::<AppResult<Vec<_>>>()?;

    sort_rows(&mut rows, metric);
    for (index, row) in rows.iter_mut().enumerate() {
        row.rank = index + 1;
    }

    Ok(LeaderboardView {
        rows,
        period,
        metric,
        period_choices: period_choices(period),
        metric_choices: metric_choices(metric),
    })
}

fn row_for_user(
    state: &AppState,
    user: User,
    usage_by_key: &HashMap<String, UsageTotals>,
) -> AppResult<Option<LeaderboardRow>> {
    let api_key = decrypt_api_key(state, &user)?;
    let Some(totals) = usage_by_key.get(&api_key).copied() else {
        return Ok(None);
    };
    if totals.requests == 0 && totals.tokens == 0 {
        return Ok(None);
    }

    Ok(Some(LeaderboardRow {
        rank: 0,
        github_login: user.github_login,
        github_name: user.github_name,
        avatar_url: user.avatar_url,
        requests: totals.requests,
        tokens: totals.tokens,
    }))
}

fn sort_rows(rows: &mut [LeaderboardRow], metric: LeaderboardMetric) {
    rows.sort_by(|left, right| {
        let primary = match metric {
            LeaderboardMetric::Tokens => right.tokens.cmp(&left.tokens),
            LeaderboardMetric::Requests => right.requests.cmp(&left.requests),
        };
        if primary != Ordering::Equal {
            return primary;
        }

        let secondary = match metric {
            LeaderboardMetric::Tokens => right.requests.cmp(&left.requests),
            LeaderboardMetric::Requests => right.tokens.cmp(&left.tokens),
        };
        if secondary != Ordering::Equal {
            return secondary;
        }

        left.github_login.cmp(&right.github_login)
    });
}

fn period_choices(selected: LeaderboardPeriod) -> Vec<LeaderboardChoice> {
    LeaderboardPeriod::all()
        .into_iter()
        .map(|period| LeaderboardChoice {
            value: period.value(),
            label: period.short_label(),
            selected: period == selected,
        })
        .collect()
}

fn metric_choices(selected: LeaderboardMetric) -> Vec<LeaderboardChoice> {
    LeaderboardMetric::all()
        .into_iter()
        .map(|metric| LeaderboardChoice {
            value: metric.value(),
            label: metric.label(),
            selected: metric == selected,
        })
        .collect()
}

fn format_integer(value: i64) -> String {
    let raw = value.unsigned_abs().to_string();
    let mut formatted = String::with_capacity(raw.len() + raw.len() / 3 + usize::from(value < 0));
    for (index, ch) in raw.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(ch);
    }
    if value < 0 {
        formatted.push('-');
    }
    formatted.chars().rev().collect()
}
