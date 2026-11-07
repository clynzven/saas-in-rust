//! Background jobs: a small, reliable queue on top of Postgres.
//!
//! * Jobs are rows, so they survive restarts and deploys.
//! * Workers claim jobs with `FOR UPDATE SKIP LOCKED`, so several workers (or
//!   several app instances) never grab the same job.
//! * Claiming pushes `run_at` forward: if a worker crashes mid-job, the job
//!   simply becomes due again later. Failures retry with exponential backoff.

use std::time::Duration;

use anyhow::{Context, Result};
use chrono::TimeDelta;
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::AppState;
use crate::booking::local_label;
use crate::mail::Email;

pub const MAX_ATTEMPTS: i32 = 6;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Job {
    BookingConfirmation {
        booking_id: Uuid,
        cancel_token: String,
    },
    BookingCancelled {
        booking_id: Uuid,
    },
}

impl Job {
    fn kind(&self) -> &'static str {
        match self {
            Job::BookingConfirmation { .. } => "booking_confirmation",
            Job::BookingCancelled { .. } => "booking_cancelled",
        }
    }
}

/// Queue a job inside the caller's transaction.
pub async fn enqueue(conn: &mut PgConnection, job: &Job) -> Result<()> {
    sqlx::query!(
        "insert into jobs (kind, payload) values ($1, $2)",
        job.kind(),
        serde_json::to_value(job)?
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Wait 30s, 1m, 2m, 4m, 8m between attempts.
pub fn backoff(attempts: i32) -> TimeDelta {
    TimeDelta::seconds(30 * 2i64.pow((attempts - 1).clamp(0, 10) as u32))
}

/// Claim and run every job that is due. Returns how many ran.
pub async fn run_due(state: &AppState) -> Result<usize> {
    let claimed = sqlx::query!(
        "update jobs set attempts = attempts + 1, run_at = now() + interval '5 minutes'
          where id in (
            select id from jobs
             where done_at is null and run_at <= now() and attempts < $1
             order by run_at
             limit 10
             for update skip locked)
         returning id, payload, attempts",
        MAX_ATTEMPTS,
    )
    .fetch_all(&state.db)
    .await?;

    for row in &claimed {
        let result = match serde_json::from_value::<Job>(row.payload.clone()) {
            Ok(job) => run(state, &job).await,
            Err(e) => Err(e.into()),
        };
        match result {
            Ok(()) => {
                // Done. Also drop the secret cancel token from the stored payload.
                sqlx::query!("update jobs set done_at = now(), payload = payload - 'cancel_token' where id = $1", row.id)
                    .execute(&state.db)
                    .await?;
            }
            Err(e) => {
                let gave_up = row.attempts >= MAX_ATTEMPTS;
                tracing::warn!(job = row.id, attempts = row.attempts, gave_up, error = %e, "job failed");
                // Giving up: the email will never go out, so its secret goes too.
                sqlx::query!(
                    "update jobs set last_error = $2, run_at = now() + make_interval(secs => $3),
                            payload = case when $4 then payload - 'cancel_token' else payload end
                      where id = $1",
                    row.id,
                    format!("{e:#}"),
                    backoff(row.attempts).num_seconds() as f64,
                    gave_up,
                )
                .execute(&state.db)
                .await?;
            }
        }
    }
    Ok(claimed.len())
}

/// Poll for work until `shutdown` is cancelled.
pub async fn worker(state: AppState, shutdown: CancellationToken) {
    tracing::info!("job worker started");
    loop {
        let idle = match run_due(&state).await {
            Ok(n) => n == 0,
            Err(e) => {
                tracing::error!(error = %e, "job worker error");
                true
            }
        };
        if idle {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(2)) => {}
                _ = shutdown.cancelled() => break,
            }
        } else if shutdown.is_cancelled() {
            break;
        }
    }
    tracing::info!("job worker stopped");
}

struct BookingInfo {
    customer_name: String,
    customer_email: String,
    starts_at: chrono::DateTime<chrono::Utc>,
    service: String,
    org_name: String,
    org_slug: String,
    timezone: String,
}

async fn booking_info(state: &AppState, id: Uuid) -> Result<BookingInfo> {
    sqlx::query_as!(
        BookingInfo,
        "select b.customer_name, b.customer_email, b.starts_at, s.name as service,
                o.name as org_name, o.slug as org_slug, o.timezone
           from bookings b join services s on s.id = b.service_id join organizations o on o.id = b.org_id
          where b.id = $1",
        id,
    )
    .fetch_one(&state.db)
    .await
    .context("booking not found")
}

async fn run(state: &AppState, job: &Job) -> Result<()> {
    let email = match job {
        Job::BookingConfirmation {
            booking_id,
            cancel_token,
        } => {
            let b = booking_info(state, *booking_id).await?;
            let tz: Tz = b
                .timezone
                .parse()
                .map_err(|_| anyhow::anyhow!("bad time zone"))?;
            let when = local_label(b.starts_at, tz);
            Email {
                to: b.customer_email,
                subject: format!("Booked: {} at {}, {when}", b.service, b.org_name),
                text: format!(
                    "Hi {},\n\nYou're booked in for {} at {} on {when}.\n\n\
                     Can't make it? Cancel here: {}/cancel/{cancel_token}\n\n\
                     Book again any time: {}/b/{}\n",
                    b.customer_name,
                    b.service,
                    b.org_name,
                    state.config.base_url,
                    state.config.base_url,
                    b.org_slug
                ),
            }
        }
        Job::BookingCancelled { booking_id } => {
            let b = booking_info(state, *booking_id).await?;
            let tz: Tz = b
                .timezone
                .parse()
                .map_err(|_| anyhow::anyhow!("bad time zone"))?;
            let when = local_label(b.starts_at, tz);
            Email {
                to: b.customer_email,
                subject: format!("Cancelled: {} at {}, {when}", b.service, b.org_name),
                text: format!(
                    "Hi {},\n\nYour booking for {} at {} on {when} has been cancelled.\n\n\
                     To book another time: {}/b/{}\n",
                    b.customer_name, b.service, b.org_name, state.config.base_url, b.org_slug
                ),
            }
        }
    };
    state.mailer.send(&email).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles() {
        let mins: Vec<i64> = (1..=5).map(|a| backoff(a).num_seconds()).collect();
        assert_eq!(mins, [30, 60, 120, 240, 480]);
    }

    #[test]
    fn jobs_serialise_with_a_kind_tag() {
        let id = Uuid::nil();
        let v = serde_json::to_value(Job::BookingCancelled { booking_id: id }).unwrap();
        assert_eq!(v["kind"], "booking_cancelled");
    }
}
