#[path = "../db.rs"]
mod db;
#[path = "../model.rs"]
mod model;
#[path = "../repository.rs"]
mod repository;
#[path = "../auth.rs"]
mod auth;

use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use anyhow::Context;
use auth::{AuthUser, HasClientId};
use model::{Category, EXPENSE_CATEGORIES};

#[derive(Clone)]
struct AppState {
    conn: libsql::Connection,
    client_id: String,
}

impl HasClientId for AppState {
    fn client_id(&self) -> &str {
        &self.client_id
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let conn = db::open().await?;

    // リリースビルドでは GOOGLE_CLIENT_ID を必須とする。
    // デバッグビルドでは未設定時は空文字（SKIP_AUTH=true で開発する想定）。
    #[cfg(not(debug_assertions))]
    let client_id = std::env::var("GOOGLE_CLIENT_ID")
        .context("GOOGLE_CLIENT_ID 環境変数が設定されていません")?;
    #[cfg(debug_assertions)]
    let client_id = std::env::var("GOOGLE_CLIENT_ID").unwrap_or_default();

    let state = AppState { conn, client_id };

    let app = Router::new()
        .route(
            "/api/v1/transactions",
            post(add_transaction).get(list_transactions),
        )
        .route(
            "/api/v1/transactions/{id}",
            patch(edit_transaction).delete(delete_transaction),
        )
        .route("/api/v1/summary", get(get_summary))
        .route("/api/v1/budgets", post(set_budget_handler).get(get_budgets))
        .route("/api/v1/progress", get(get_progress))
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| anyhow::anyhow!("ポート {addr} のバインドに失敗しました: {e}"))?;
    println!("listening on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

// ─── Response types ──────────────────────────────────────────────────────────

/// API レスポンス用取引データ（user_id を除外）。
#[derive(Serialize)]
struct TransactionResponse {
    id: i64,
    name: String,
    amount: i64,
    date: String,
    category: Category,
    memo: Option<String>,
    created_at: String,
}

impl From<model::Transaction> for TransactionResponse {
    fn from(tx: model::Transaction) -> Self {
        Self {
            id: tx.id,
            name: tx.name,
            amount: tx.amount,
            date: tx.date,
            category: tx.category,
            memo: tx.memo,
            created_at: tx.created_at,
        }
    }
}

/// API レスポンス用予算データ（user_id を除外）。
#[derive(Serialize)]
struct BudgetResponse {
    id: i64,
    month: Option<String>,
    category: Option<Category>,
    amount: i64,
}

impl From<model::Budget> for BudgetResponse {
    fn from(b: model::Budget) -> Self {
        Self {
            id: b.id,
            month: b.month,
            category: b.category,
            amount: b.amount,
        }
    }
}

// ─── Error helpers ────────────────────────────────────────────────────────────

fn bad_request(msg: &'static str) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({"error": msg}))).into_response()
}

fn not_found(msg: &'static str) -> Response {
    (StatusCode::NOT_FOUND, Json(json!({"error": msg}))).into_response()
}

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, Json(json!({"error": "Forbidden"}))).into_response()
}

fn internal_server_error() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "Internal server error"})),
    )
    .into_response()
}

// ─── Shared helpers ───────────────────────────────────────────────────────────

fn build_category_totals(transactions: &[model::Transaction]) -> HashMap<Category, i64> {
    let mut totals: HashMap<Category, i64> = HashMap::new();
    for tx in transactions {
        *totals.entry(tx.category).or_insert(0) += tx.amount;
    }
    totals
}

fn prev_month(month: &str) -> anyhow::Result<String> {
    let mut parts = month.splitn(2, '-');
    let year: i32 = parts
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("invalid month: {month}"))?;
    let m: u32 = parts
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("invalid month: {month}"))?;
    if m == 1 {
        Ok(format!("{:04}-12", year - 1))
    } else {
        Ok(format!("{:04}-{:02}", year, m - 1))
    }
}

fn validate_month(month: &str) -> bool {
    chrono::NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d").is_ok()
}

fn current_month() -> String {
    chrono::Local::now().format("%Y-%m").to_string()
}

fn calc_percentage(current: i64, base: i64) -> f64 {
    if base == 0 {
        return 0.0;
    }
    (current as f64 / base as f64) * 100.0
}

// ─── POST /api/v1/transactions ────────────────────────────────────────────────

#[derive(Deserialize)]
struct AddTransactionRequest {
    name: String,
    amount: i64,
    date: String,
    category: Category,
    memo: Option<String>,
}

async fn add_transaction(
    State(state): State<AppState>,
    AuthUser { user_id }: AuthUser,
    Json(req): Json<AddTransactionRequest>,
) -> Response {
    if req.name.is_empty() {
        return bad_request("Invalid request: name is required");
    }
    if req.amount <= 0 {
        return bad_request("Invalid request: amount must be a positive integer");
    }
    if chrono::NaiveDate::parse_from_str(&req.date, "%Y-%m-%d").is_err() {
        return bad_request("Invalid request: date must be YYYY-MM-DD");
    }
    let new_tx = repository::NewTransaction {
        user_id,
        name: req.name,
        amount: req.amount,
        date: req.date,
        category: req.category,
        memo: req.memo,
    };
    match repository::add(&state.conn, &new_tx).await {
        Ok(tx) => (
            StatusCode::CREATED,
            Json(json!({"data": TransactionResponse::from(tx)})),
        )
            .into_response(),
        Err(_) => internal_server_error(),
    }
}

// ─── GET /api/v1/transactions ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct ListTransactionsQuery {
    month: Option<String>,
    category: Option<Category>,
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn list_transactions(
    State(state): State<AppState>,
    AuthUser { user_id }: AuthUser,
    Query(query): Query<ListTransactionsQuery>,
) -> Response {
    let month = query.month.or_else(|| Some(current_month()));
    if let Some(ref m) = month {
        if !validate_month(m) {
            return bad_request("Invalid request: month must be YYYY-MM");
        }
    }
    let filter = repository::TransactionFilter {
        user_id,
        month,
        category: query.category,
    };
    let all = match repository::list(&state.conn, &filter).await {
        Ok(txs) => txs,
        Err(_) => return internal_server_error(),
    };
    let total = all.len();
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(50);
    let transactions: Vec<TransactionResponse> = all
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(TransactionResponse::from)
        .collect();
    Json(json!({"data": {"transactions": transactions, "total": total}})).into_response()
}

// ─── PATCH /api/v1/transactions/:id ──────────────────────────────────────────

#[derive(Deserialize)]
struct EditTransactionRequest {
    name: Option<String>,
    amount: Option<i64>,
    date: Option<String>,
    category: Option<Category>,
    memo: Option<String>,
}

async fn edit_transaction(
    State(state): State<AppState>,
    AuthUser { user_id }: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<EditTransactionRequest>,
) -> Response {
    if let Some(amount) = req.amount {
        if amount <= 0 {
            return bad_request("Invalid request: amount must be a positive integer");
        }
    }
    if let Some(ref date) = req.date {
        if chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
            return bad_request("Invalid request: date must be YYYY-MM-DD");
        }
    }
    let update = repository::TransactionUpdate {
        name: req.name,
        amount: req.amount,
        date: req.date,
        category: req.category,
        memo: req.memo,
    };
    match repository::edit(&state.conn, id, &update, &user_id).await {
        Ok(tx) => Json(json!({"data": TransactionResponse::from(tx)})).into_response(),
        Err(_) => resolve_tx_error(&state.conn, id).await,
    }
}

// ─── DELETE /api/v1/transactions/{id} ────────────────────────────────────────

async fn delete_transaction(
    State(state): State<AppState>,
    AuthUser { user_id }: AuthUser,
    Path(id): Path<i64>,
) -> Response {
    match repository::delete(&state.conn, id, &user_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => resolve_tx_error(&state.conn, id).await,
    }
}

/// 取引が存在するなら 403、存在しないなら 404、DB エラーなら 500 を返す。
async fn resolve_tx_error(conn: &libsql::Connection, id: i64) -> Response {
    match repository::transaction_exists(conn, id).await {
        Ok(true) => forbidden(),
        Ok(false) => not_found("Transaction not found"),
        Err(_) => internal_server_error(),
    }
}

// ─── GET /api/v1/summary ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SummaryQuery {
    month: Option<String>,
    by_category: Option<bool>,
}

async fn get_summary(
    State(state): State<AppState>,
    AuthUser { user_id }: AuthUser,
    Query(query): Query<SummaryQuery>,
) -> Response {
    let month = query.month.unwrap_or_else(current_month);
    if !validate_month(&month) {
        return bad_request("Invalid request: month must be YYYY-MM");
    }
    let filter = repository::TransactionFilter {
        user_id,
        month: Some(month.clone()),
        category: None,
    };
    let transactions = match repository::list(&state.conn, &filter).await {
        Ok(txs) => txs,
        Err(_) => return internal_server_error(),
    };
    let totals = build_category_totals(&transactions);
    let income: i64 = totals
        .iter()
        .filter(|(c, _)| c.is_income())
        .map(|(_, &v)| v)
        .sum();
    let expense: i64 = totals
        .iter()
        .filter(|(c, _)| !c.is_income())
        .map(|(_, &v)| v)
        .sum();
    let balance = income - expense;

    let by_category_data: Vec<serde_json::Value> = if query.by_category.unwrap_or(false) {
        let mut cats: Vec<Category> = vec![Category::Income];
        cats.extend_from_slice(EXPENSE_CATEGORIES);
        cats.iter()
            .filter_map(|c| {
                totals
                    .get(c)
                    .map(|&amount| json!({"category": c.to_string(), "amount": amount}))
            })
            .collect()
    } else {
        vec![]
    };

    Json(json!({
        "data": {
            "month": month,
            "income": income,
            "expense": expense,
            "balance": balance,
            "by_category": by_category_data,
        }
    }))
    .into_response()
}

// ─── POST /api/v1/budgets ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SetBudgetRequest {
    month: Option<String>,
    category: Option<Category>,
    amount: i64,
}

async fn set_budget_handler(
    State(state): State<AppState>,
    AuthUser { user_id }: AuthUser,
    Json(req): Json<SetBudgetRequest>,
) -> Response {
    if req.amount <= 0 {
        return bad_request("Invalid request: amount must be a positive integer");
    }
    if req.month.is_some() {
        return bad_request("Invalid request: month-specific budgets are not yet supported");
    }
    let new_budget = repository::NewBudget {
        user_id,
        category: req.category,
        amount: req.amount,
    };
    match repository::set_budget(&state.conn, &new_budget).await {
        Ok(budget) => (
            StatusCode::CREATED,
            Json(json!({"data": BudgetResponse::from(budget)})),
        )
            .into_response(),
        Err(_) => internal_server_error(),
    }
}

// ─── GET /api/v1/budgets ──────────────────────────────────────────────────────

async fn get_budgets(
    State(state): State<AppState>,
    AuthUser { user_id }: AuthUser,
) -> Response {
    match repository::list_budgets(&state.conn, &user_id).await {
        Ok(budgets) => {
            let responses: Vec<BudgetResponse> =
                budgets.into_iter().map(BudgetResponse::from).collect();
            Json(json!({"data": {"budgets": responses}})).into_response()
        }
        Err(_) => internal_server_error(),
    }
}

// ─── GET /api/v1/progress ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ProgressQuery {
    month: Option<String>,
    mode: Option<String>,
    scope: Option<String>,
}

async fn get_progress(
    State(state): State<AppState>,
    AuthUser { user_id }: AuthUser,
    Query(query): Query<ProgressQuery>,
) -> Response {
    let month = query.month.unwrap_or_else(current_month);
    if !validate_month(&month) {
        return bad_request("Invalid request: month must be YYYY-MM");
    }
    let mode = query.mode.as_deref().unwrap_or("budget");
    if !matches!(mode, "budget" | "last_month") {
        return bad_request("Invalid request: mode must be 'budget' or 'last_month'");
    }
    let scope = query.scope.as_deref().unwrap_or("both");
    if !matches!(scope, "total" | "by_category" | "both") {
        return bad_request("Invalid request: scope must be 'total', 'by_category', or 'both'");
    }
    let show_total = matches!(scope, "total" | "both");
    let show_by_category = matches!(scope, "by_category" | "both");

    let current_txs = match repository::list(
        &state.conn,
        &repository::TransactionFilter {
            user_id: user_id.clone(),
            month: Some(month.clone()),
            category: None,
        },
    )
    .await
    {
        Ok(txs) => txs,
        Err(_) => return internal_server_error(),
    };
    let current_totals = build_category_totals(&current_txs);
    let current_expense: i64 = current_totals
        .iter()
        .filter(|(c, _)| !c.is_income())
        .map(|(_, &v)| v)
        .sum();

    let mut data = json!({"month": month, "mode": mode});

    match mode {
        "budget" => {
            let budgets = match repository::list_budgets(&state.conn, &user_id).await {
                Ok(b) => b,
                Err(_) => return internal_server_error(),
            };
            let total_budget_amt = budgets
                .iter()
                .find(|b| b.category.is_none())
                .map(|b| b.amount)
                .unwrap_or(0);
            let category_budgets: HashMap<Category, i64> = budgets
                .iter()
                .filter_map(|b| b.category.map(|c| (c, b.amount)))
                .collect();

            if show_total {
                data["total"] = json!({
                    "base": total_budget_amt,
                    "current": current_expense,
                    "percentage": calc_percentage(current_expense, total_budget_amt),
                });
            }
            if show_by_category {
                let by_cat: Vec<serde_json::Value> = EXPENSE_CATEGORIES
                    .iter()
                    .filter_map(|c| {
                        let base = *category_budgets.get(c)?;
                        let current = current_totals.get(c).copied().unwrap_or(0);
                        Some(json!({
                            "category": c.to_string(),
                            "base": base,
                            "current": current,
                            "percentage": calc_percentage(current, base),
                        }))
                    })
                    .collect();
                data["by_category"] = json!(by_cat);
            }
        }
        _ => {
            // last_month
            let last = match prev_month(&month) {
                Ok(m) => m,
                Err(_) => return bad_request("Invalid request: month must be YYYY-MM"),
            };
            let last_txs = match repository::list(
                &state.conn,
                &repository::TransactionFilter {
                    user_id: user_id.clone(),
                    month: Some(last),
                    category: None,
                },
            )
            .await
            {
                Ok(txs) => txs,
                Err(_) => return internal_server_error(),
            };
            let last_totals = build_category_totals(&last_txs);
            let last_expense: i64 = last_totals
                .iter()
                .filter(|(c, _)| !c.is_income())
                .map(|(_, &v)| v)
                .sum();

            if show_total {
                data["total"] = json!({
                    "base": last_expense,
                    "current": current_expense,
                    "percentage": calc_percentage(current_expense, last_expense),
                });
            }
            if show_by_category {
                let by_cat: Vec<serde_json::Value> = EXPENSE_CATEGORIES
                    .iter()
                    .filter_map(|c| {
                        let base = last_totals.get(c).copied().unwrap_or(0);
                        let current = current_totals.get(c).copied().unwrap_or(0);
                        if base == 0 && current == 0 {
                            return None;
                        }
                        Some(json!({
                            "category": c.to_string(),
                            "base": base,
                            "current": current,
                            "percentage": calc_percentage(current, base),
                        }))
                    })
                    .collect();
                data["by_category"] = json!(by_cat);
            }
        }
    }

    Json(json!({"data": data})).into_response()
}
