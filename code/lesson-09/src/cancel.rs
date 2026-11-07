//! Customers cancel their own booking with the secret link from their email.

use askama::Template;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use chrono_tz::Tz;

use crate::AppState;
use crate::auth::token_hash;
use crate::booking::local_label;
use crate::error::{AppError, AppResult};
use crate::jobs::{self, Job};
use crate::web::render;

#[derive(Template)]
#[template(path = "cancel.html")]
struct CancelPage {
    token: String,
    org_name: String,
    org_slug: String,
    service: String,
    when: String,
    state: CancelState,
}

#[derive(PartialEq, Eq)]
enum CancelState {
    CanCancel,
    AlreadyCancelled,
    InThePast,
    JustCancelled,
}

struct Found {
    id: uuid::Uuid,
    status: String,
    starts_at: DateTime<Utc>,
    service: String,
    org_name: String,
    org_slug: String,
    timezone: String,
}

async fn find(state: &AppState, token: &str) -> AppResult<Found> {
    sqlx::query_as!(
        Found,
        "select b.id, b.status, b.starts_at, s.name as service, o.name as org_name, o.slug as org_slug, o.timezone
           from bookings b join services s on s.id = b.service_id join organizations o on o.id = b.org_id
          where b.cancel_token_hash = $1",
        token_hash(token),
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)
}

fn page(token: String, b: Found, state: CancelState) -> AppResult<Response> {
    let tz: Tz = b
        .timezone
        .parse()
        .map_err(|_| anyhow::anyhow!("bad time zone"))?;
    let when = local_label(b.starts_at, tz);
    Ok(render(CancelPage {
        token,
        org_name: b.org_name,
        org_slug: b.org_slug,
        service: b.service,
        when,
        state,
    })?
    .into_response())
}

pub async fn show(State(state): State<AppState>, Path(token): Path<String>) -> AppResult<Response> {
    let b = find(&state, &token).await?;
    let s = if b.status == "cancelled" {
        CancelState::AlreadyCancelled
    } else if b.starts_at <= Utc::now() {
        CancelState::InThePast
    } else {
        CancelState::CanCancel
    };
    page(token, b, s)
}

pub async fn confirm(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> AppResult<Response> {
    let b = find(&state, &token).await?;
    let mut tx = state.db.begin().await?;
    // The conditions live in the UPDATE itself, so a double click can't cancel twice.
    let updated = sqlx::query!(
        "update bookings set status = 'cancelled'
          where id = $1 and status = 'confirmed' and starts_at > now()",
        b.id,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated == 0 {
        drop(tx);
        return show(State(state), Path(token)).await;
    }
    jobs::enqueue(&mut tx, &Job::BookingCancelled { booking_id: b.id }).await?;
    tx.commit().await?;
    tracing::info!(booking_id = %b.id, "booking cancelled by customer");
    page(token, b, CancelState::JustCancelled)
}
