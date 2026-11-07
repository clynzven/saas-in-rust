//! Businesses ("organizations"), membership roles and the `Org` extractor.
//!
//! Every page under /app/{slug}/ takes an `Org` argument. The extractor checks
//! that the logged-in user belongs to that business, so a handler can't forget.

use std::collections::HashMap;

use askama::Template;
use axum::Form;
use axum::extract::{FromRequestParts, Path, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::accounts::is_unique_violation;
use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};
use crate::forms;
use crate::web::render;

#[derive(Clone, Copy, Debug, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "member_role", rename_all = "lowercase")]
pub enum Role {
    Owner,
    Staff,
}

/// The current business, for a user who is a member of it.
#[derive(Clone, Debug)]
pub struct Org {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub timezone: String,
    pub role: Role,
    pub user: CurrentUser,
}

impl Org {
    pub fn is_owner(&self) -> bool {
        self.role == Role::Owner
    }

    pub fn require_owner(&self) -> AppResult<()> {
        if self.is_owner() {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }
}

impl FromRequestParts<AppState> for Org {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        let user = CurrentUser::from_request_parts(parts, state).await?;
        let Path(params) = Path::<HashMap<String, String>>::from_request_parts(parts, state)
            .await
            .map_err(|_| AppError::NotFound)?;
        let slug = params.get("slug").ok_or(AppError::NotFound)?;

        let row = sqlx::query!(
            r#"select o.id, o.name, o.slug, o.timezone, m.role as "role: Role"
                 from organizations o join memberships m on m.org_id = o.id
                where o.slug = $1 and m.user_id = $2"#,
            slug,
            user.id,
        )
        .fetch_optional(&state.db)
        .await?
        // Not a member? Say "not found", not "forbidden": we don't confirm
        // that someone else's business exists.
        .ok_or(AppError::NotFound)?;

        Ok(Org {
            id: row.id,
            name: row.name,
            slug: row.slug,
            timezone: row.timezone,
            role: row.role,
            user,
        })
    }
}

// ---------------------------------------------------------------- create

pub const TIMEZONES: &[&str] = &[
    "Pacific/Auckland",
    "Australia/Sydney",
    "Asia/Manila",
    "Asia/Singapore",
    "Asia/Tokyo",
    "Europe/London",
    "Europe/Berlin",
    "America/New_York",
    "America/Los_Angeles",
    "UTC",
];

#[derive(Template, Default)]
#[template(path = "org_new.html")]
struct NewOrgPage {
    error: Option<String>,
    name: String,
    slug: String,
    timezone: String,
    timezones: &'static [&'static str],
}

impl NewOrgPage {
    fn is_selected(&self, tz: &str) -> bool {
        self.timezone == tz
    }
}

#[derive(Deserialize)]
pub struct NewOrgForm {
    name: String,
    slug: String,
    timezone: String,
}

/// Taking `CurrentUser` means only logged-in users see this page.
pub async fn new_page(_user: CurrentUser) -> AppResult<Html<String>> {
    render(NewOrgPage {
        timezones: TIMEZONES,
        timezone: "Pacific/Auckland".into(),
        ..Default::default()
    })
}

pub fn valid_slug(s: &str) -> bool {
    (3..=40).contains(&s.len())
        && s.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

pub async fn create(
    State(state): State<AppState>,
    user: CurrentUser,
    Form(f): Form<NewOrgForm>,
) -> AppResult<Response> {
    let slug = f.slug.trim().to_lowercase();
    let page = |error: &str| NewOrgPage {
        error: Some(error.to_string()),
        name: f.name.clone(),
        slug: slug.clone(),
        timezone: f.timezone.clone(),
        timezones: TIMEZONES,
    };
    let name = match forms::text(&f.name, "a business name", 80) {
        Ok(n) => n,
        Err(e) => return Ok((StatusCode::UNPROCESSABLE_ENTITY, render(page(&e))?).into_response()),
    };
    if !valid_slug(&slug) {
        let e = "Use 3 to 40 lowercase letters, digits and single dashes, like kape-barbers.";
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, render(page(e))?).into_response());
    }
    if f.timezone.parse::<chrono_tz::Tz>().is_err() {
        return Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            render(page("Please choose a time zone."))?,
        )
            .into_response());
    }

    // The business and its first owner are created together, or not at all.
    let mut tx = state.db.begin().await?;
    let org_id = match sqlx::query_scalar!(
        "insert into organizations (name, slug, timezone) values ($1, $2, $3) returning id",
        name,
        slug,
        f.timezone,
    )
    .fetch_one(&mut *tx)
    .await
    {
        Ok(id) => id,
        Err(e) if is_unique_violation(&e, "organizations_slug_key") => {
            let e = "That address is taken. Try adding your town, like kape-barbers-wellington.";
            return Ok((StatusCode::UNPROCESSABLE_ENTITY, render(page(e))?).into_response());
        }
        Err(e) => return Err(e.into()),
    };
    sqlx::query!(
        "insert into memberships (org_id, user_id, role) values ($1, $2, 'owner')",
        org_id,
        user.id,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    tracing::info!(%org_id, %slug, "business created");
    Ok(Redirect::to(&format!("/app/{slug}")).into_response())
}

// ---------------------------------------------------------------- overview

#[derive(Template)]
#[template(path = "org_home.html")]
struct OrgHome {
    user: CurrentUser,
    org: Org,
    members: i64,
}

pub async fn home(State(state): State<AppState>, org: Org) -> AppResult<Html<String>> {
    let members = sqlx::query_scalar!(
        r#"select count(*) as "n!" from memberships where org_id = $1"#,
        org.id
    )
    .fetch_one(&state.db)
    .await?;
    render(OrgHome {
        user: org.user.clone(),
        org,
        members,
    })
}

// ---------------------------------------------------------------- team

pub struct Member {
    pub user_id: Uuid,
    pub name: String,
    pub email: String,
    pub role: Role,
}

#[derive(Template)]
#[template(path = "team.html")]
struct TeamPage {
    user: CurrentUser,
    org: Org,
    members: Vec<Member>,
    error: Option<String>,
}

async fn team_page(state: &AppState, org: Org, error: Option<String>) -> AppResult<Response> {
    let members = sqlx::query_as!(
        Member,
        r#"select u.id as user_id, u.name, u.email, m.role as "role: Role"
             from memberships m join users u on u.id = m.user_id
            where m.org_id = $1
            order by m.role, u.name"#,
        org.id,
    )
    .fetch_all(&state.db)
    .await?;
    let status = if error.is_some() {
        StatusCode::UNPROCESSABLE_ENTITY
    } else {
        StatusCode::OK
    };
    Ok((
        status,
        render(TeamPage {
            user: org.user.clone(),
            org,
            members,
            error,
        })?,
    )
        .into_response())
}

pub async fn team(State(state): State<AppState>, org: Org) -> AppResult<Response> {
    team_page(&state, org, None).await
}

#[derive(Deserialize)]
pub struct AddMemberForm {
    email: String,
}

/// Owners add staff by the email they signed up with.
pub async fn add_member(
    State(state): State<AppState>,
    org: Org,
    Form(f): Form<AddMemberForm>,
) -> AppResult<Response> {
    org.require_owner()?;
    // The answer says whether an email has an account, so don't let anyone
    // test thousands of addresses this way.
    if !state
        .ip_limiter
        .allow(&format!("add-member {}", org.user.id))
    {
        let msg =
            "You've added a lot of people in a short time. Please wait a few minutes.".to_string();
        return team_page(&state, org, Some(msg)).await;
    }
    let email = f.email.trim().to_lowercase();
    let Some(user_id) = sqlx::query_scalar!("select id from users where lower(email) = $1", email)
        .fetch_optional(&state.db)
        .await?
    else {
        let msg =
            format!("No account uses {email}. Ask them to sign up first, then add them here.");
        return team_page(&state, org, Some(msg)).await;
    };
    sqlx::query!(
        "insert into memberships (org_id, user_id, role) values ($1, $2, 'staff') on conflict do nothing",
        org.id,
        user_id,
    )
    .execute(&state.db)
    .await?;
    Ok(Redirect::to(&format!("/app/{}/team", org.slug)).into_response())
}

pub async fn remove_member(
    State(state): State<AppState>,
    org: Org,
    Path((_, user_id)): Path<(String, Uuid)>,
) -> AppResult<Response> {
    org.require_owner()?;
    // Owners can't be removed here, so a business never loses its last owner.
    sqlx::query!(
        "delete from memberships where org_id = $1 and user_id = $2 and role = 'staff'",
        org.id,
        user_id
    )
    .execute(&state.db)
    .await?;
    Ok(Redirect::to(&format!("/app/{}/team", org.slug)).into_response())
}

#[cfg(test)]
mod tests {
    #[test]
    fn slugs() {
        for ok in ["kape-barbers", "abc", "studio-9", "a1-b2-c3"] {
            assert!(super::valid_slug(ok), "{ok}");
        }
        for bad in [
            "ab",
            "Kape",
            "kape--barbers",
            "-kape",
            "kape-",
            "kape barbers",
            "café",
        ] {
            assert!(!super::valid_slug(bad), "{bad}");
        }
    }
}
