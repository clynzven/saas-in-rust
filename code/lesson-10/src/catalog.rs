//! What a business offers (services) and when it's open (opening hours).
//!
//! These pages use HTMX: forms post as usual, but when the request comes from
//! HTMX we return just the changed fragment and the browser swaps it in.
//! Without JavaScript, the same forms still work with full page loads.

use std::collections::HashMap;

use askama::Template;
use axum::Form;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use chrono::NaiveTime;
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};
use crate::forms;
use crate::orgs::Org;
use crate::web::{is_htmx, render};

// ---------------------------------------------------------------- services

pub struct Service {
    pub id: Uuid,
    pub name: String,
    pub duration_minutes: i32,
    pub price_cents: i32,
    pub active: bool,
}

impl Service {
    pub fn price(&self) -> String {
        format_price(self.price_cents)
    }
}

pub fn format_price(cents: i32) -> String {
    format!("${}.{:02}", cents / 100, cents % 100)
}

/// "35", "35.5" and "$35.50" all become 3550 cents.
pub fn parse_price(raw: &str) -> Result<i32, String> {
    let s = raw.trim().trim_start_matches('$');
    let err = || "Enter a price like 35 or 35.50.".to_string();
    let (whole, frac) = s.split_once('.').unwrap_or((s, ""));
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || frac.len() > 2
        || !frac.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(err());
    }
    let whole: i32 = whole.parse().map_err(|_| err())?;
    let frac: i32 = format!("{frac:0<2}").parse().map_err(|_| err())?;
    whole
        .checked_mul(100)
        .and_then(|c| c.checked_add(frac))
        .filter(|c| *c <= 1_000_000)
        .ok_or_else(err)
}

pub const DURATIONS: &[i32] = &[15, 30, 45, 60, 90, 120];

#[derive(Template)]
#[template(path = "services.html")]
struct ServicesPage {
    user: CurrentUser,
    org: Org,
    services: Vec<Service>,
    error: Option<String>,
    durations: &'static [i32],
}

/// The same template, but only its `section` block: what HTMX swaps in.
#[derive(Template)]
#[template(path = "services.html", block = "section")]
struct ServicesSection {
    org: Org,
    services: Vec<Service>,
    error: Option<String>,
    durations: &'static [i32],
}

#[derive(Template)]
#[template(path = "service_row.html")]
struct ServiceRow {
    org: Org,
    s: Service,
}

async fn load_services(state: &AppState, org_id: Uuid) -> AppResult<Vec<Service>> {
    Ok(sqlx::query_as!(
        Service,
        "select id, name, duration_minutes, price_cents, active
           from services where org_id = $1 order by active desc, name",
        org_id,
    )
    .fetch_all(&state.db)
    .await?)
}

pub async fn services(State(state): State<AppState>, org: Org) -> AppResult<Response> {
    let services = load_services(&state, org.id).await?;
    Ok(render(ServicesPage {
        user: org.user.clone(),
        org,
        services,
        error: None,
        durations: DURATIONS,
    })?
    .into_response())
}

#[derive(Deserialize)]
pub struct ServiceForm {
    name: String,
    duration_minutes: i32,
    price: String,
}

pub async fn add_service(
    State(state): State<AppState>,
    headers: HeaderMap,
    org: Org,
    Form(f): Form<ServiceForm>,
) -> AppResult<Response> {
    org.require_owner()?;
    let checked = forms::text(&f.name, "a service name", 80).and_then(|name| {
        if !DURATIONS.contains(&f.duration_minutes) {
            return Err("Choose one of the listed durations.".to_string());
        }
        Ok((name, parse_price(&f.price)?))
    });
    let error = match checked {
        Ok((name, price_cents)) => {
            sqlx::query!(
                "insert into services (org_id, name, duration_minutes, price_cents) values ($1, $2, $3, $4)",
                org.id,
                name,
                f.duration_minutes,
                price_cents,
            )
            .execute(&state.db)
            .await?;
            None
        }
        Err(e) => Some(e),
    };

    if !is_htmx(&headers) {
        return match error {
            None => Ok(Redirect::to(&format!("/app/{}/services", org.slug)).into_response()),
            Some(e) => Err(AppError::BadRequest(e)),
        };
    }
    let services = load_services(&state, org.id).await?;
    let status = if error.is_some() {
        StatusCode::UNPROCESSABLE_ENTITY
    } else {
        StatusCode::OK
    };
    Ok((
        status,
        render(ServicesSection {
            org,
            services,
            error,
            durations: DURATIONS,
        })?,
    )
        .into_response())
}

/// Turn a service on or off. Bookings keep pointing at it either way.
pub async fn toggle_service(
    State(state): State<AppState>,
    headers: HeaderMap,
    org: Org,
    Path((_, id)): Path<(String, Uuid)>,
) -> AppResult<Response> {
    org.require_owner()?;
    // `org_id = $2` keeps this inside the current business.
    let s = sqlx::query_as!(
        Service,
        "update services set active = not active where id = $1 and org_id = $2
         returning id, name, duration_minutes, price_cents, active",
        id,
        org.id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    if is_htmx(&headers) {
        Ok(render(ServiceRow { org, s })?.into_response())
    } else {
        Ok(Redirect::to(&format!("/app/{}/services", org.slug)).into_response())
    }
}

// ---------------------------------------------------------------- opening hours

pub const WEEKDAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

pub struct DayHours {
    pub weekday: usize,
    pub name: &'static str,
    pub open: bool,
    pub opens: String,
    pub closes: String,
}

#[derive(Template)]
#[template(path = "hours.html")]
struct HoursPage {
    user: CurrentUser,
    org: Org,
    days: Vec<DayHours>,
    message: Option<(bool, String)>,
}

#[derive(Template)]
#[template(path = "hours.html", block = "section")]
struct HoursSection {
    org: Org,
    days: Vec<DayHours>,
    message: Option<(bool, String)>,
}

async fn load_hours(state: &AppState, org_id: Uuid) -> AppResult<Vec<DayHours>> {
    let rows = sqlx::query!(
        "select weekday, opens, closes from opening_hours where org_id = $1",
        org_id
    )
    .fetch_all(&state.db)
    .await?;
    Ok((0..7)
        .map(|d| {
            let row = rows.iter().find(|r| r.weekday as usize == d);
            DayHours {
                weekday: d,
                name: WEEKDAYS[d],
                open: row.is_some(),
                opens: row.map_or("09:00".into(), |r| r.opens.format("%H:%M").to_string()),
                closes: row.map_or("17:00".into(), |r| r.closes.format("%H:%M").to_string()),
            }
        })
        .collect())
}

pub async fn hours(State(state): State<AppState>, org: Org) -> AppResult<Response> {
    let days = load_hours(&state, org.id).await?;
    Ok(render(HoursPage {
        user: org.user.clone(),
        org,
        days,
        message: None,
    })?
    .into_response())
}

/// The form sends open_0, opens_0, closes_0 ... for each weekday 0-6.
pub fn parse_hours(
    form: &HashMap<String, String>,
) -> Result<Vec<(i16, NaiveTime, NaiveTime)>, String> {
    let mut out = Vec::new();
    for (d, day) in WEEKDAYS.iter().enumerate() {
        if form.get(&format!("open_{d}")).is_none() {
            continue;
        }
        let time = |key: &str| {
            form.get(&format!("{key}_{d}"))
                .and_then(|v| NaiveTime::parse_from_str(v, "%H:%M").ok())
                .ok_or_else(|| format!("{day}: enter times like 09:00."))
        };
        let (opens, closes) = (time("opens")?, time("closes")?);
        if opens >= closes {
            return Err(format!(
                "{}: closing time must be after opening time.",
                WEEKDAYS[d]
            ));
        }
        out.push((d as i16, opens, closes));
    }
    Ok(out)
}

pub async fn save_hours(
    State(state): State<AppState>,
    headers: HeaderMap,
    org: Org,
    Form(form): Form<HashMap<String, String>>,
) -> AppResult<Response> {
    org.require_owner()?;
    let message = match parse_hours(&form) {
        Ok(days) => {
            // Replace the whole week in one transaction: readers never see half a week.
            let mut tx = state.db.begin().await?;
            sqlx::query!("delete from opening_hours where org_id = $1", org.id)
                .execute(&mut *tx)
                .await?;
            for (weekday, opens, closes) in days {
                sqlx::query!(
                    "insert into opening_hours (org_id, weekday, opens, closes) values ($1, $2, $3, $4)",
                    org.id,
                    weekday,
                    opens,
                    closes,
                )
                .execute(&mut *tx)
                .await?;
            }
            tx.commit().await?;
            (true, "Opening hours saved.".to_string())
        }
        Err(e) => (false, e),
    };
    if !is_htmx(&headers) {
        return if message.0 {
            Ok(Redirect::to(&format!("/app/{}/hours", org.slug)).into_response())
        } else {
            Err(AppError::BadRequest(message.1))
        };
    }
    let days = load_hours(&state, org.id).await?;
    let status = if message.0 {
        StatusCode::OK
    } else {
        StatusCode::UNPROCESSABLE_ENTITY
    };
    Ok((
        status,
        render(HoursSection {
            org,
            days,
            message: Some(message),
        })?,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prices() {
        assert_eq!(parse_price("35"), Ok(3500));
        assert_eq!(parse_price("$35.5"), Ok(3550));
        assert_eq!(parse_price(" 0.99 "), Ok(99));
        for bad in ["", "abc", "1.234", "-5", "1,50", "999999999"] {
            assert!(parse_price(bad).is_err(), "{bad:?}");
        }
        assert_eq!(format_price(3550), "$35.50");
    }

    #[test]
    fn hours_form() {
        let form: HashMap<String, String> = [
            ("open_0", "on"),
            ("opens_0", "09:00"),
            ("closes_0", "17:30"),
            ("opens_1", "09:00"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let days = parse_hours(&form).unwrap();
        assert_eq!(days.len(), 1, "only days ticked as open are saved");
        assert_eq!(days[0].2, NaiveTime::from_hms_opt(17, 30, 0).unwrap());

        let mut bad = form.clone();
        bad.insert("closes_0".into(), "08:00".into());
        assert!(parse_hours(&bad).unwrap_err().contains("after opening"));
    }
}
