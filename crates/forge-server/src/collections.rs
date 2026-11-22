//! Collections (Peak C1): ordered, owner-curated lists of
//! snippets, local to one registry. A collection NEVER widens
//! access - its read endpoint filters every item through the
//! snippet visibility rules, so a public collection containing a
//! private snippet shows that snippet to nobody but its owner
//! (and to the collection owner only while they can read it too).
//!
//! Endpoints:
//! - `POST   /collections`                                create
//! - `GET    /me/collections`                             own list
//! - `GET    /collections/{owner}/{name}`                 read
//! - `PATCH  /collections/{owner}/{name}`                 edit
//! - `DELETE /collections/{owner}/{name}`                 delete
//! - `PUT    /collections/{owner}/{name}/items/{so}/{sn}` add
//! - `DELETE /collections/{owner}/{name}/items/{so}/{sn}` remove
//! - `POST   /collections/{owner}/{name}/reorder`         set order

use crate::auth::{body, CurrentUser, RequireUser};
use crate::db::now_iso;
use crate::error::ApiError;
use crate::repo::{CollectionItemRow, CollectionRow, UserRow};
use crate::snippets::{check_write_scope_and_csrf, resolve_readable, validate_visibility};
use crate::state::AppState;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Json, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;

/// Plan limits (C1): plenty for a curated runbook, small enough
/// to keep every read one page.
pub(crate) const MAX_COLLECTIONS_PER_USER: i64 = 100;
/// Plan limit per collection.
pub(crate) const MAX_ITEMS_PER_COLLECTION: i64 = 200;

fn collection_not_found() -> ApiError {
    ApiError::not_found("collection_not_found", "Collection not found")
}

// ------------------------------------------------------------ shapes

fn collection_json(
    row: &CollectionRow,
    owner_username: &str,
    item_count: i64,
) -> serde_json::Value {
    serde_json::json!({
        "owner": owner_username,
        "name": row.name,
        "description": row.description,
        "visibility": row.visibility,
        "item_count": item_count,
        "created_at": row.created_at,
        "updated_at": row.updated_at,
    })
}

fn item_json(item: &CollectionItemRow) -> serde_json::Value {
    serde_json::json!({
        "owner": item.owner_username,
        "name": item.name,
        "description": item.description,
        "visibility": item.visibility,
        "latest_version": item.latest_version,
        "position": item.position,
    })
}

/// The snippet read rules applied to a joined item row (mirrors
/// `snippets::can_read`, plus soft-deleted snippets are gone for
/// everyone): public/unlisted for all, private for the snippet's
/// owner only, admin-hidden for the owner only.
fn item_readable(item: &CollectionItemRow, viewer: Option<&UserRow>) -> bool {
    if item.deleted_at.is_some() {
        return false;
    }
    if viewer.is_some_and(|u| u.id == item.snippet_owner_id) {
        return true;
    }
    if item.hidden_at.is_some() {
        return false;
    }
    item.visibility == "public" || item.visibility == "unlisted"
}

/// Collection read access: the owner, or anyone when the
/// collection is public/unlisted. Private collections answer
/// 404 to everyone else so their existence never leaks.
fn collection_readable(row: &CollectionRow, viewer: Option<&UserRow>) -> bool {
    if viewer.is_some_and(|u| u.id == row.owner_id) {
        return true;
    }
    row.visibility == "public" || row.visibility == "unlisted"
}

// ------------------------------------------------------------ resolve

/// Resolve owner username + collection name to a row, 404 for
/// unknown users and unknown collections alike.
async fn resolve_collection(
    state: &AppState,
    owner: &str,
    name: &str,
) -> Result<(UserRow, CollectionRow), ApiError> {
    let owner_row = state
        .repos
        .users
        .by_username(owner)
        .await?
        .ok_or_else(collection_not_found)?;
    let row = state
        .repos
        .collections
        .by_owner_name(owner_row.id, name)
        .await?
        .ok_or_else(collection_not_found)?;
    Ok((owner_row, row))
}

/// Resolve a collection the caller OWNS (mutations). Any other
/// outcome - someone else's collection included - is the same
/// 404, so write attempts never confirm existence.
async fn resolve_own_collection(
    state: &AppState,
    ctx: &RequireUser,
    owner: &str,
    name: &str,
) -> Result<CollectionRow, ApiError> {
    if owner != ctx.0.user.username {
        return Err(collection_not_found());
    }
    state
        .repos
        .collections
        .by_owner_name(ctx.0.user.id, name)
        .await?
        .ok_or_else(collection_not_found)
}

// ------------------------------------------------------------ handlers

#[derive(Deserialize)]
struct CollectionInput {
    name: String,
    description: Option<String>,
    visibility: Option<String>,
}

async fn create_collection(
    State(state): State<AppState>,
    ctx: RequireUser,
    headers: HeaderMap,
    result: Result<Json<CollectionInput>, JsonRejection>,
) -> Result<Response, ApiError> {
    check_write_scope_and_csrf(&ctx, &headers)?;
    let req = body(result)?;
    let name = req.name.trim().to_string();
    forge_core::names::validate_snippet_name(&name)
        .map_err(|e| ApiError::validation(e.to_string()))?;
    let visibility = req.visibility.as_deref().unwrap_or("private");
    validate_visibility(visibility)?;
    let description = req.description.as_deref().unwrap_or("");
    if description.chars().count() > 280 {
        return Err(ApiError::validation(
            "Description must be at most 280 characters",
        ));
    }
    let count = state
        .repos
        .collections
        .count_for_owner(ctx.0.user.id)
        .await?;
    if count >= MAX_COLLECTIONS_PER_USER {
        return Err(ApiError::validation(format!(
            "Collection limit reached ({MAX_COLLECTIONS_PER_USER} per user)"
        )));
    }
    let now = now_iso();
    let row = state
        .repos
        .collections
        .create(ctx.0.user.id, &name, description, visibility, &now)
        .await
        .map_err(|e| match e {
            crate::repo::RepoError::Conflict(_) => ApiError::new(
                StatusCode::CONFLICT,
                "collection_exists",
                "You already have a collection with that name",
            ),
            other => ApiError::from(other),
        })?;
    Ok((
        StatusCode::CREATED,
        Json(collection_json(&row, &ctx.0.user.username, 0)),
    )
        .into_response())
}

async fn my_collections(
    State(state): State<AppState>,
    ctx: RequireUser,
) -> Result<Response, ApiError> {
    if !ctx.0.kind.has_scope("read") {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "insufficient_scope",
            "This token cannot read",
        ));
    }
    let rows = state
        .repos
        .collections
        .list_for_owner(ctx.0.user.id)
        .await?;
    let items: Vec<serde_json::Value> = rows
        .iter()
        .map(|(row, count)| collection_json(row, &ctx.0.user.username, *count))
        .collect();
    Ok(Json(serde_json::json!({ "collections": items })).into_response())
}

async fn get_collection(
    State(state): State<AppState>,
    current: CurrentUser,
    Path((owner, name)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let viewer = current.0.as_ref().map(|c| &c.user);
    let (owner_row, row) = resolve_collection(&state, &owner, &name).await?;
    if !collection_readable(&row, viewer) {
        return Err(collection_not_found());
    }
    let all_items = state.repos.collections.items(row.id).await?;
    let total = i64::try_from(all_items.len()).unwrap_or(i64::MAX);
    let visible: Vec<&CollectionItemRow> = all_items
        .iter()
        .filter(|item| item_readable(item, viewer))
        .collect();
    let mut out = collection_json(
        &row,
        &owner_row.username,
        i64::try_from(visible.len()).unwrap_or(i64::MAX),
    );
    out["items"] = serde_json::Value::Array(visible.iter().map(|i| item_json(i)).collect());
    if viewer.is_some_and(|u| u.id == row.owner_id) {
        out["total_items"] = serde_json::json!(total);
    }
    Ok(Json(out).into_response())
}

#[derive(Deserialize)]
struct CollectionPatch {
    description: Option<String>,
    visibility: Option<String>,
}

async fn patch_collection(
    State(state): State<AppState>,
    ctx: RequireUser,
    headers: HeaderMap,
    Path((owner, name)): Path<(String, String)>,
    result: Result<Json<CollectionPatch>, JsonRejection>,
) -> Result<Response, ApiError> {
    check_write_scope_and_csrf(&ctx, &headers)?;
    let req = body(result)?;
    let row = resolve_own_collection(&state, &ctx, &owner, &name).await?;
    if let Some(visibility) = &req.visibility {
        validate_visibility(visibility)?;
    }
    if let Some(description) = &req.description {
        if description.chars().count() > 280 {
            return Err(ApiError::validation(
                "Description must be at most 280 characters",
            ));
        }
    }
    let now = now_iso();
    let updated = state
        .repos
        .collections
        .update(
            row.id,
            req.description.as_deref(),
            req.visibility.as_deref(),
            &now,
        )
        .await?;
    let count = state.repos.collections.item_count(row.id).await?;
    Ok(Json(collection_json(&updated, &ctx.0.user.username, count)).into_response())
}

async fn delete_collection(
    State(state): State<AppState>,
    ctx: RequireUser,
    headers: HeaderMap,
    Path((owner, name)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    check_write_scope_and_csrf(&ctx, &headers)?;
    let row = resolve_own_collection(&state, &ctx, &owner, &name).await?;
    state.repos.collections.delete(row.id).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn add_item(
    State(state): State<AppState>,
    ctx: RequireUser,
    headers: HeaderMap,
    Path((owner, name, sowner, sname)): Path<(String, String, String, String)>,
) -> Result<Response, ApiError> {
    check_write_scope_and_csrf(&ctx, &headers)?;
    let row = resolve_own_collection(&state, &ctx, &owner, &name).await?;
    // You can only add what you can read - adding someone else's
    // private snippet is impossible even by guessing its name,
    // because resolve_readable answers the same 404.
    let resolved = resolve_readable(&state, &sowner, &sname, Some(&ctx.0.user)).await?;
    let count = state.repos.collections.item_count(row.id).await?;
    if count >= MAX_ITEMS_PER_COLLECTION {
        return Err(ApiError::validation(format!(
            "Collection item limit reached ({MAX_ITEMS_PER_COLLECTION} per collection)"
        )));
    }
    let now = now_iso();
    state
        .repos
        .collections
        .add_item(row.id, resolved.snippet.id, &now)
        .await
        .map_err(|e| match e {
            crate::repo::RepoError::Conflict(_) => ApiError::new(
                StatusCode::CONFLICT,
                "already_in_collection",
                "That snippet is already in the collection",
            ),
            other => ApiError::from(other),
        })?;
    state.repos.collections.touch(row.id, &now).await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "owner": sowner,
            "name": resolved.snippet.name,
        })),
    )
        .into_response())
}

async fn remove_item(
    State(state): State<AppState>,
    ctx: RequireUser,
    headers: HeaderMap,
    Path((owner, name, sowner, sname)): Path<(String, String, String, String)>,
) -> Result<Response, ApiError> {
    check_write_scope_and_csrf(&ctx, &headers)?;
    let row = resolve_own_collection(&state, &ctx, &owner, &name).await?;
    // Removal must work even when the snippet has become
    // unreadable (went private, was renamed) - resolve by id
    // without the read filter, but only ever within this
    // collection's own item list.
    let snippet_owner = state
        .repos
        .users
        .by_username(&sowner)
        .await?
        .ok_or_else(collection_not_found)?;
    let snippet = state
        .repos
        .snippets
        .by_owner_name(snippet_owner.id, &sname)
        .await?
        .ok_or_else(collection_not_found)?;
    let removed = state
        .repos
        .collections
        .remove_item(row.id, snippet.id)
        .await?;
    if !removed {
        return Err(collection_not_found());
    }
    let now = now_iso();
    state.repos.collections.touch(row.id, &now).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[derive(Deserialize)]
struct ReorderInput {
    items: Vec<String>,
}

async fn reorder_items(
    State(state): State<AppState>,
    ctx: RequireUser,
    headers: HeaderMap,
    Path((owner, name)): Path<(String, String)>,
    result: Result<Json<ReorderInput>, JsonRejection>,
) -> Result<Response, ApiError> {
    check_write_scope_and_csrf(&ctx, &headers)?;
    let req = body(result)?;
    let row = resolve_own_collection(&state, &ctx, &owner, &name).await?;
    let current = state.repos.collections.items(row.id).await?;
    if req.items.len() != current.len() {
        return Err(ApiError::validation(
            "Reorder must list every current item exactly once",
        ));
    }
    let mut ids: Vec<i64> = Vec::with_capacity(req.items.len());
    for refr in &req.items {
        let (so, sn) = refr.split_once('/').ok_or_else(|| {
            ApiError::validation(format!("Item `{refr}` must be in owner/name form"))
        })?;
        let item = current
            .iter()
            .find(|i| i.owner_username == so && i.name == sn)
            .ok_or_else(|| {
                ApiError::validation(format!("Item `{refr}` is not in the collection"))
            })?;
        if ids.contains(&item.snippet_id) {
            return Err(ApiError::validation(format!(
                "Item `{refr}` is listed twice"
            )));
        }
        ids.push(item.snippet_id);
    }
    state.repos.collections.set_order(row.id, &ids).await?;
    let now = now_iso();
    state.repos.collections.touch(row.id, &now).await?;
    let updated = state
        .repos
        .collections
        .by_owner_name(ctx.0.user.id, &row.name)
        .await?
        .ok_or_else(collection_not_found)?;
    let count = state.repos.collections.item_count(row.id).await?;
    Ok(Json(collection_json(&updated, &ctx.0.user.username, count)).into_response())
}

/// Collection routes (merged into the API router).
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/collections", post(create_collection))
        .route("/me/collections", get(my_collections))
        .route(
            "/collections/{owner}/{name}",
            get(get_collection)
                .patch(patch_collection)
                .delete(delete_collection),
        )
        .route(
            "/collections/{owner}/{name}/items/{sowner}/{sname}",
            axum::routing::put(add_item).delete(remove_item),
        )
        .route("/collections/{owner}/{name}/reorder", post(reorder_items))
}
