//! The public booking flow (/b/{slug}) and the staff's bookings list.

use askama::Template;
use axum::Form;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use chrono::{DateTime, Datelike, NaiveDate, TimeDelta, Utc};
use chrono_tz::Tz;
use serde::Deserialize;
use sqlx::Acquire;
use uuid::Uuid;

use crate::AppState;
use crate::auth::CurrentUser;
use crate::availability::{self, Busy, Slot};
use crate::catalog::format_price;
use crate::error::{AppError, AppResult};
use crate::forms;
use crate::jobs::{self, Job};
use crate::orgs::Org;
use crate::web::{is_htmx, render};

/// Customers can't book less than an hour ahead...
pub const MIN_NOTICE: TimeDelta = TimeDelta::minutes(60);
/// ...or more than 60 days ahead.
pub const MAX_DAYS_AHEAD: i64 = 60;
/// Offer a start time every 15 minutes.
pub const STEP: TimeDelta = TimeDelta::minutes(15);

pub struct PublicOrg {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub tz: Tz,
}

pub struct PublicService {
    pub id: Uuid,
    pub name: String,
    pub duration_minutes: i32,
    pub price_cents: i32,
}

impl PublicService {
    pub fn price(&self) -> String {
        format_price(self.price_cents)
    }
}

/// "test@example.com" becomes "t***@example.com": enough for the customer to
/// recognise, useless to anyone else who sees the link.
pub fn mask_email(email: &str) -> String {
    match email.split_once('@') {
        Some((local, domain)) => {
            let first = local.chars().next().unwrap_or('*');
            format!("{first}***@{domain}")
        }
        None => "***".into(),
    }
}

/// "Tue 1 Dec, 9:30 am" in the business's time zone.
pub fn local_label(t: DateTime<Utc>, tz: Tz) -> String {
    t.with_timezone(&tz)
        .format("%a %-d %b, %-I:%M %P")
        .to_string()
}

async fn public_org(state: &AppState, slug: &str) -> AppResult<PublicOrg> {
    let r = sqlx::query!(
        "select id, name, slug, timezone from organizations where slug = $1",
        slug
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let tz = r
        .timezone
        .parse()
        .map_err(|_| anyhow::anyhow!("bad time zone {}", r.timezone))?;
    Ok(PublicOrg {
        id: r.id,
        name: r.name,
        slug: r.slug,
        tz,
    })
}

async fn public_service(state: &AppState, org_id: Uuid, id: Uuid) -> AppResult<PublicService> {
    sqlx::query_as!(
        PublicService,
        "select id, name, duration_minutes, price_cents from services where id = $1 and org_id = $2 and active",
        id,
        org_id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)
}

/// Gather everything the availability engine needs for one day and run it.
pub async fn slots_for(
    state: &AppState,
    org: &PublicOrg,
    service: &PublicService,
    date: NaiveDate,
) -> AppResult<Vec<Slot>> {
    let weekday = date.weekday().num_days_from_monday() as i16;
    let hours = sqlx::query!(
        "select opens, closes from opening_hours where org_id = $1 and weekday = $2",
        org.id,
        weekday
    )
    .fetch_optional(&state.db)
    .await?
    .map(|r| (r.opens, r.closes));
    let staff = sqlx::query_scalar!(
        "select user_id from memberships where org_id = $1 order by created_at",
        org.id
    )
    .fetch_all(&state.db)
    .await?;
    // Bookings anywhere near this day (a day either side covers every time zone).
    let from = date
        .pred_opt()
        .unwrap_or(date)
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();
    let to = from + TimeDelta::days(3);
    let busy = sqlx::query_as!(
        Busy,
        "select staff_id, starts_at as start, ends_at as end from bookings
          where org_id = $1 and status = 'confirmed' and starts_at < $3 and ends_at > $2",
        org.id,
        from,
        to,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(availability::slots(&availability::Query {
        date,
        tz: org.tz,
        hours,
        duration: TimeDelta::minutes(service.duration_minutes.into()),
        step: STEP,
        not_before: Utc::now() + MIN_NOTICE,
        staff: &staff,
        busy: &busy,
    }))
}

fn today_in(tz: Tz) -> NaiveDate {
    Utc::now().with_timezone(&tz).date_naive()
}

/// Clamp a requested date into the bookable window.
fn bookable_date(tz: Tz, requested: Option<NaiveDate>) -> NaiveDate {
    let today = today_in(tz);
    requested
        .unwrap_or(today)
        .clamp(today, today + TimeDelta::days(MAX_DAYS_AHEAD))
}

// ---------------------------------------------------------------- pages

#[derive(Template)]
#[template(path = "public_org.html")]
struct OrgPage {
    org: PublicOrg,
    services: Vec<PublicService>,
}

pub async fn org_page(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<Response> {
    let org = public_org(&state, &slug).await?;
    let services = sqlx::query_as!(
        PublicService,
        "select id, name, duration_minutes, price_cents from services where org_id = $1 and active order by name",
        org.id,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(render(OrgPage { org, services })?.into_response())
}

pub struct SlotView {
    pub start: String,
    pub label: String,
}

#[derive(Template)]
#[template(path = "public_service.html")]
struct ServicePage {
    org: PublicOrg,
    service: PublicService,
    date: NaiveDate,
    min_date: NaiveDate,
    max_date: NaiveDate,
    slots: Vec<SlotView>,
}

#[derive(Template)]
#[template(path = "public_service.html", block = "slots")]
struct SlotsFragment {
    org: PublicOrg,
    service: PublicService,
    date: NaiveDate,
    slots: Vec<SlotView>,
}

#[derive(Deserialize)]
pub struct DateQuery {
    date: Option<NaiveDate>,
}

pub async fn service_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((slug, service_id)): Path<(String, Uuid)>,
    Query(q): Query<DateQuery>,
) -> AppResult<Response> {
    let org = public_org(&state, &slug).await?;
    let service = public_service(&state, org.id, service_id).await?;
    let date = bookable_date(org.tz, q.date);
    let slots = slots_for(&state, &org, &service, date)
        .await?
        .into_iter()
        .map(|s| SlotView {
            start: s.start.to_rfc3339(),
            label: s
                .start
                .with_timezone(&org.tz)
                .format("%-I:%M %P")
                .to_string(),
        })
        .collect();
    if is_htmx(&headers) {
        // The date picker changed: send back just the list of times.
        return Ok(render(SlotsFragment {
            org,
            service,
            date,
            slots,
        })?
        .into_response());
    }
    let min_date = today_in(org.tz);
    let max_date = min_date + TimeDelta::days(MAX_DAYS_AHEAD);
    Ok(render(ServicePage {
        org,
        service,
        date,
        min_date,
        max_date,
        slots,
    })?
    .into_response())
}

#[derive(Template)]
#[template(path = "public_book.html")]
struct BookPage {
    org: PublicOrg,
    service: PublicService,
    start: String,
    when: String,
    error: Option<String>,
    name: String,
    email: String,
}

#[derive(Deserialize)]
pub struct StartQuery {
    start: DateTime<Utc>,
}

pub async fn book_page(
    State(state): State<AppState>,
    Path((slug, service_id)): Path<(String, Uuid)>,
    Query(q): Query<StartQuery>,
) -> AppResult<Response> {
    let org = public_org(&state, &slug).await?;
    let service = public_service(&state, org.id, service_id).await?;
    let when = local_label(q.start, org.tz);
    let page = BookPage {
        org,
        service,
        start: q.start.to_rfc3339(),
        when,
        error: None,
        name: String::new(),
        email: String::new(),
    };
    Ok(render(page)?.into_response())
}

#[derive(Deserialize)]
pub struct BookForm {
    start: DateTime<Utc>,
    name: String,
    email: String,
}

/// Book a slot. Safe under concurrency: the database's exclusion constraint is
/// the final judge, so two customers can never get the same staff member.
pub async fn book(
    State(state): State<AppState>,
    Path((slug, service_id)): Path<(String, Uuid)>,
    Form(f): Form<BookForm>,
) -> AppResult<Response> {
    let org = public_org(&state, &slug).await?;
    let service = public_service(&state, org.id, service_id).await?;

    let checked =
        forms::text(&f.name, "your name", 80).and_then(|n| Ok((n, forms::email(&f.email)?)));
    let (name, email) = match checked {
        Ok(v) => v,
        Err(e) => {
            let when = local_label(f.start, org.tz);
            let page = BookPage {
                org,
                service,
                start: f.start.to_rfc3339(),
                when,
                error: Some(e),
                name: f.name,
                email: f.email,
            };
            return Ok((StatusCode::UNPROCESSABLE_ENTITY, render(page)?).into_response());
        }
    };

    // Re-check availability now: the page may have been open for a while.
    let date = f.start.with_timezone(&org.tz).date_naive();
    let slot = slots_for(&state, &org, &service, date)
        .await?
        .into_iter()
        .find(|s| s.start == f.start)
        .ok_or_else(|| {
            AppError::Conflict(
                "Sorry, that time is no longer available. Please pick another.".into(),
            )
        })?;

    let (cancel_token, cancel_hash) = crate::auth::new_token();
    let mut tx = state.db.begin().await?;
    let mut booked = None;
    for staff_id in &slot.free_staff {
        // A savepoint per attempt: if this staff member was just taken, we roll
        // back to here and try the next one, without losing the transaction.
        let mut attempt = (&mut *tx).begin().await?;
        // Take a lock on this staff member until the transaction ends. Two
        // inserts racing on the exclusion constraint can deadlock in Postgres;
        // with the lock they queue up, and the loser gets a clean violation.
        // Every request locks staff in the same order, so locks can't cycle.
        sqlx::query!(
            "select pg_advisory_xact_lock(hashtextextended($1::text, 0))",
            staff_id.to_string()
        )
        .execute(&mut *attempt)
        .await?;
        let inserted = sqlx::query_scalar!(
            "insert into bookings (org_id, service_id, staff_id, customer_name, customer_email,
                                   starts_at, ends_at, cancel_token_hash)
             values ($1, $2, $3, $4, $5, $6, $7, $8) returning id",
            org.id,
            service.id,
            staff_id,
            name,
            email,
            slot.start,
            slot.end,
            cancel_hash,
        )
        .fetch_one(&mut *attempt)
        .await;
        match inserted {
            Ok(id) => {
                attempt.commit().await?;
                booked = Some(id);
                break;
            }
            Err(e) if crate::accounts::is_unique_violation(&e, "no_double_booking") => {
                attempt.rollback().await?;
                tracing::info!(%staff_id, "staff member was just booked by someone else; trying the next");
            }
            Err(e) => return Err(e.into()),
        }
    }
    let Some(booking_id) = booked else {
        return Err(AppError::Conflict(
            "Sorry, someone booked that time a moment ago. Please pick another.".into(),
        ));
    };
    // Same transaction: the booking and its confirmation email commit together.
    jobs::enqueue(
        &mut tx,
        &Job::BookingConfirmation {
            booking_id,
            cancel_token,
        },
    )
    .await?;
    tx.commit().await?;

    tracing::info!(%booking_id, org = %org.slug, "booking confirmed");
    Ok(Redirect::to(&format!("/b/{}/booked/{booking_id}", org.slug)).into_response())
}

#[derive(Template)]
#[template(path = "public_booked.html")]
struct BookedPage {
    org: PublicOrg,
    service_name: String,
    when: String,
    email: String,
}

pub async fn booked(
    State(state): State<AppState>,
    Path((slug, id)): Path<(String, Uuid)>,
) -> AppResult<Response> {
    let org = public_org(&state, &slug).await?;
    let b = sqlx::query!(
        "select s.name as service_name, b.starts_at, b.customer_email
           from bookings b join services s on s.id = b.service_id
          where b.id = $1 and b.org_id = $2",
        id,
        org.id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let when = local_label(b.starts_at, org.tz);
    Ok(render(BookedPage {
        org,
        service_name: b.service_name,
        when,
        email: mask_email(&b.customer_email),
    })?
    .into_response())
}

// ---------------------------------------------------------------- staff view

pub struct BookingRow {
    pub id: Uuid,
    pub starts_at: DateTime<Utc>,
    pub service: String,
    pub staff: String,
    pub customer_name: String,
    pub customer_email: String,
    pub status: String,
    /// Still in the future, so staff can cancel it.
    pub upcoming: bool,
}

#[derive(Template)]
#[template(path = "bookings.html")]
struct BookingsPage {
    user: CurrentUser,
    org: Org,
    tz: Tz,
    rows: Vec<BookingRow>,
}

#[derive(Template)]
#[template(path = "booking_row.html")]
struct BookingRowFragment {
    org: Org,
    tz: Tz,
    b: BookingRow,
}

impl BookingsPage {
    fn when(&self, t: &DateTime<Utc>) -> String {
        local_label(*t, self.tz)
    }
}

impl BookingRowFragment {
    fn when(&self, t: &DateTime<Utc>) -> String {
        local_label(*t, self.tz)
    }
}

pub async fn bookings(State(state): State<AppState>, org: Org) -> AppResult<Response> {
    let tz: Tz = org
        .timezone
        .parse()
        .map_err(|_| anyhow::anyhow!("bad time zone"))?;
    let rows = sqlx::query_as!(
        BookingRow,
        "select b.id, b.starts_at, s.name as service, u.name as staff, b.customer_name, b.customer_email, b.status,
                b.starts_at > now() as \"upcoming!\"
           from bookings b join services s on s.id = b.service_id join users u on u.id = b.staff_id
          where b.org_id = $1 and b.ends_at > now() - interval '1 day'
          order by b.starts_at
          limit 200",
        org.id,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(render(BookingsPage {
        user: org.user.clone(),
        org,
        tz,
        rows,
    })?
    .into_response())
}

pub async fn cancel_by_staff(
    State(state): State<AppState>,
    headers: HeaderMap,
    org: Org,
    Path((_, id)): Path<(String, Uuid)>,
) -> AppResult<Response> {
    let tz: Tz = org
        .timezone
        .parse()
        .map_err(|_| anyhow::anyhow!("bad time zone"))?;
    let mut tx = state.db.begin().await?;
    let b = sqlx::query_as!(
        BookingRow,
        "with updated as (
           update bookings set status = 'cancelled'
            where id = $1 and org_id = $2 and status = 'confirmed' and starts_at > now()
           returning *
         )
         select b.id, b.starts_at, s.name as service, u.name as staff, b.customer_name, b.customer_email, b.status,
                b.starts_at > now() as \"upcoming!\"
           from updated b join services s on s.id = b.service_id join users u on u.id = b.staff_id",
        id,
        org.id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    jobs::enqueue(&mut tx, &Job::BookingCancelled { booking_id: id }).await?;
    tx.commit().await?;
    tracing::info!(booking_id = %id, by = %org.user.id, "booking cancelled by staff");
    if is_htmx(&headers) {
        Ok(render(BookingRowFragment { org, tz, b })?.into_response())
    } else {
        Ok(Redirect::to(&format!("/app/{}/bookings", org.slug)).into_response())
    }
}
