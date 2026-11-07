//! The logged-in home page.

use askama::Template;
use axum::extract::State;
use axum::response::Html;

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::AppResult;
use crate::web::render;

pub struct OrgSummary {
    pub name: String,
    pub slug: String,
    pub role: String,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct Dashboard {
    user: CurrentUser,
    orgs: Vec<OrgSummary>,
}

pub async fn dashboard(
    State(state): State<AppState>,
    user: CurrentUser,
) -> AppResult<Html<String>> {
    let orgs = sqlx::query_as!(
        OrgSummary,
        r#"select o.name, o.slug, m.role::text as "role!"
             from memberships m join organizations o on o.id = m.org_id
            where m.user_id = $1
            order by o.name"#,
        user.id,
    )
    .fetch_all(&state.db)
    .await?;
    render(Dashboard { user, orgs })
}
