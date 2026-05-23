use crate::{
    context::SharedState,
    database::{EventListFilters, ProgramVersionByteSide},
    models::{
        CreateMetricAlertRuleRequest, CreateTelegramSubscriptionRequest, DefiProtocolsResponse,
        EventScopeCounts, EventsResponse, EventsSummary, LiveStatusResponse, MetricAlertRule,
        MetricAlertsResponse, ProgramDisassemblyResponse, ProgramIdlVersionsResponse,
        ProgramSignerStatsResponse, ProgramVersionsResponse, TelegramSubscription,
        TelegramSubscriptionSummaryResponse, TelegramSubscriptionsResponse,
    },
    name_service, protocols, telegram,
};
use axum::{
    Json, Router,
    extract::{Path, Query, Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header},
    middleware::{Next, from_fn_with_state},
    response::{IntoResponse, Response},
    routing::{delete, get},
};
use serde::Deserialize;
use serde_json::json;
use solana_sdk::pubkey::Pubkey;
use std::{str::FromStr, time::Instant};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{debug, warn};

const ESTIMATED_SLOT_TIME_MS: i64 = 400;

pub fn router(state: SharedState) -> anyhow::Result<Router> {
    let cors = CorsLayer::new()
        .allow_origin(HeaderValue::from_str(&state.config.cors_allowed_origin)?)
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            HeaderName::from_static("x-firewatch-origin-token"),
        ]);

    let api_router = Router::new()
        .route("/api/events", get(list_events))
        .route("/api/status", get(status))
        .route("/api/live-status", get(live_status))
        .route("/api/protocols", get(list_protocols))
        .route(
            "/api/programs/{address}/signer-stats",
            get(program_signer_stats),
        )
        .route("/api/programs/{address}/versions", get(program_versions))
        .route("/api/programs/{address}/idls", get(program_idl_versions))
        .route(
            "/api/programs/{address}/idls/{version_id}/bytes",
            get(program_idl_version_bytes),
        )
        .route(
            "/api/programs/{address}/idls/{version_id}/text",
            get(program_idl_version_text),
        )
        .route(
            "/api/programs/{address}/versions/{version_id}/bytes",
            get(program_version_bytes),
        )
        .route(
            "/api/programs/{address}/versions/{version_id}/disassembly",
            get(program_version_disassembly),
        )
        .route("/api/metric-alerts", get(list_metric_alerts))
        .route(
            "/api/metric-alert-rules",
            axum::routing::post(create_metric_alert_rule),
        )
        .route(
            "/api/metric-alert-rules/{id}",
            delete(delete_metric_alert_rule),
        )
        .route(
            "/api/telegram-subscriptions",
            get(list_telegram_subscriptions).post(create_telegram_subscription),
        )
        .route(
            "/api/telegram-subscriptions/summary",
            get(telegram_subscriptions_summary),
        )
        .route(
            "/api/telegram-subscriptions/{id}",
            delete(delete_telegram_subscription),
        )
        .with_state(state.clone())
        .layer(from_fn_with_state(state.clone(), require_origin_token));

    Ok(Router::new()
        .route("/healthz", get(health))
        .merge(api_router)
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http()))
}

#[derive(Deserialize)]
struct EventsQuery {
    limit: Option<i64>,
    offset: Option<i64>,
    protocol: Option<String>,
    target_address: Option<String>,
    scope: Option<String>,
    search: Option<String>,
    severity: Option<String>,
    coverage: Option<String>,
    tag: Option<String>,
    include_related_multisigs: Option<bool>,
}

#[derive(Deserialize)]
struct ProgramVersionBytesQuery {
    side: Option<String>,
}

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

async fn list_events(
    State(state): State<SharedState>,
    Query(query): Query<EventsQuery>,
) -> Result<Json<EventsResponse>, StatusCode> {
    let started_at = Instant::now();
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let offset = query.offset.unwrap_or(0).max(0);
    let normalized_protocol = query
        .protocol
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(protocols::curated_protocol_descriptor)
        .map(|descriptor| descriptor.slug);
    let related_multisigs = if query.include_related_multisigs.unwrap_or(false) {
        match normalized_protocol.as_deref() {
            Some(protocol_slug) => state
                .db
                .protocol_related_multisigs(protocol_slug)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
            None => Vec::new(),
        }
    } else {
        Vec::new()
    };
    let normalized_target_address = query
        .target_address
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let normalized_scope = query
        .scope
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let normalized_search = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let normalized_severity = query
        .severity
        .as_deref()
        .map(str::trim)
        .filter(|value| matches!(*value, "critical" | "high" | "standard"));
    let known_defi_only = query
        .coverage
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| value == "known");
    let normalized_tag = query
        .tag
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let filters = EventListFilters {
        protocol_slug: normalized_protocol.as_deref(),
        related_multisigs: &related_multisigs,
        target_address: normalized_target_address,
        scope: normalized_scope,
        search: normalized_search,
        severity: normalized_severity,
        known_defi_only,
        tag: normalized_tag,
    };

    let items = state
        .db
        .list_events_filtered(limit, offset, filters)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let scope_counts = state
        .db
        .event_scope_counts(filters)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let total_count = count_for_scope(normalized_scope, &scope_counts);
    let mut hydrated_items = Vec::with_capacity(items.len());
    for event in items {
        hydrated_items.push(name_service::hydrate_event_cached(&state, event).await);
    }
    let summary = state
        .db
        .summary()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let next_offset = (offset + (hydrated_items.len() as i64) < total_count)
        .then_some(offset + hydrated_items.len() as i64);

    debug!(
        limit,
        offset,
        item_count = hydrated_items.len(),
        total_count,
        elapsed_ms = started_at.elapsed().as_millis() as u64,
        protocol = normalized_protocol.as_deref().unwrap_or(""),
        target_address = normalized_target_address.unwrap_or(""),
        scope = normalized_scope.unwrap_or(""),
        search = normalized_search.unwrap_or(""),
        severity = normalized_severity.unwrap_or(""),
        tag = normalized_tag.unwrap_or(""),
        "served events page",
    );

    Ok(Json(EventsResponse {
        items: hydrated_items,
        summary,
        total_count,
        scope_counts,
        next_offset,
    }))
}

fn count_for_scope(scope: Option<&str>, counts: &EventScopeCounts) -> i64 {
    match scope {
        Some("critical") => counts.critical,
        Some("defi") => counts.defi,
        Some("programs") => counts.programs,
        Some("idl") => counts.idl,
        Some("multisig") => counts.multisig,
        _ => counts.all,
    }
}

async fn list_telegram_subscriptions(
    State(state): State<SharedState>,
) -> ApiResult<TelegramSubscriptionsResponse> {
    let items = state
        .db
        .list_telegram_subscriptions()
        .await
        .map_err(internal_error)?;

    Ok(Json(TelegramSubscriptionsResponse {
        items,
        telegram_enabled: state.config.telegram_bot_token.is_some(),
    }))
}

async fn telegram_subscriptions_summary(
    State(state): State<SharedState>,
) -> ApiResult<TelegramSubscriptionSummaryResponse> {
    let subscription_count = state
        .db
        .telegram_subscription_count()
        .await
        .map_err(internal_error)?;

    Ok(Json(TelegramSubscriptionSummaryResponse {
        subscription_count,
        telegram_enabled: state.config.telegram_bot_token.is_some(),
    }))
}

async fn list_protocols(State(state): State<SharedState>) -> ApiResult<DefiProtocolsResponse> {
    let mut items = protocols::curated_protocol_descriptors();
    let program_ids = items
        .iter()
        .flat_map(|protocol| protocol.programs.iter())
        .map(|program| program.program_id.clone())
        .collect::<Vec<_>>();
    let risk_summaries = state
        .db
        .program_risk_summaries(&program_ids)
        .await
        .map_err(internal_error)?;

    for protocol in &mut items {
        for program in &mut protocol.programs {
            if let Some(summary) = risk_summaries.get(&program.program_id) {
                program.risk_summary = summary.clone();
            }
        }
    }

    Ok(Json(DefiProtocolsResponse { items }))
}

async fn program_signer_stats(
    State(state): State<SharedState>,
    Path(address): Path<String>,
) -> ApiResult<ProgramSignerStatsResponse> {
    Pubkey::from_str(address.trim()).map_err(|_| {
        error_response(
            StatusCode::BAD_REQUEST,
            "Provide a valid Solana program address to load signer stats.",
        )
    })?;

    let mut items = state
        .db
        .program_signer_stats(address.trim(), 12)
        .await
        .map_err(internal_error)?;

    for item in &mut items {
        if let Some(label) = name_service::cached_address_label(&state, &item.member)
            .await
            .map_err(internal_error)?
        {
            item.member_label = Some(label.label().to_owned());
        }
    }

    Ok(Json(ProgramSignerStatsResponse { items }))
}

async fn program_versions(
    State(state): State<SharedState>,
    Path(address): Path<String>,
) -> ApiResult<ProgramVersionsResponse> {
    Pubkey::from_str(address.trim()).map_err(|_| {
        error_response(
            StatusCode::BAD_REQUEST,
            "Provide a valid Solana program address to load program versions.",
        )
    })?;

    let items = state
        .db
        .list_program_versions(address.trim(), 200)
        .await
        .map_err(internal_error)?;

    Ok(Json(ProgramVersionsResponse { items }))
}

async fn program_idl_versions(
    State(state): State<SharedState>,
    Path(address): Path<String>,
) -> ApiResult<ProgramIdlVersionsResponse> {
    Pubkey::from_str(address.trim()).map_err(|_| {
        error_response(
            StatusCode::BAD_REQUEST,
            "Provide a valid Solana program address to load IDL versions.",
        )
    })?;

    let items = state
        .db
        .list_program_idl_versions(address.trim(), 200)
        .await
        .map_err(internal_error)?;

    Ok(Json(ProgramIdlVersionsResponse { items }))
}

async fn program_idl_version_bytes(
    State(state): State<SharedState>,
    Path((address, version_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    Pubkey::from_str(address.trim()).map_err(|_| {
        error_response(
            StatusCode::BAD_REQUEST,
            "Provide a valid Solana program address to load IDL bytes.",
        )
    })?;

    let Some(bytes) = state
        .db
        .program_idl_version_bytes(address.trim(), &version_id)
        .await
        .map_err(internal_error)?
    else {
        return Err(error_response(
            StatusCode::NOT_FOUND,
            "IDL bytes are not available for this version.",
        ));
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&program_idl_filename(address.trim(), &version_id, "bin"))
            .map_err(internal_error)?,
    );

    Ok((headers, bytes))
}

async fn program_idl_version_text(
    State(state): State<SharedState>,
    Path((address, version_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    Pubkey::from_str(address.trim()).map_err(|_| {
        error_response(
            StatusCode::BAD_REQUEST,
            "Provide a valid Solana program address to load IDL text.",
        )
    })?;

    let Some(text) = state
        .db
        .program_idl_version_text(address.trim(), &version_id)
        .await
        .map_err(internal_error)?
    else {
        return Err(error_response(
            StatusCode::NOT_FOUND,
            "Decoded IDL text is not available for this version.",
        ));
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&program_idl_filename(address.trim(), &version_id, "json"))
            .map_err(internal_error)?,
    );

    Ok((headers, text))
}

async fn program_version_bytes(
    State(state): State<SharedState>,
    Path((address, version_id)): Path<(String, String)>,
    Query(query): Query<ProgramVersionBytesQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    Pubkey::from_str(address.trim()).map_err(|_| {
        error_response(
            StatusCode::BAD_REQUEST,
            "Provide a valid Solana program address to load program bytes.",
        )
    })?;

    let side = parse_program_version_side(query.side.as_deref())?;

    let Some(bytes) = state
        .db
        .program_version_bytes(address.trim(), &version_id, side)
        .await
        .map_err(internal_error)?
    else {
        return Err(error_response(
            StatusCode::NOT_FOUND,
            "Program bytes are not available for this version.",
        ));
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&program_bytes_filename(address.trim(), &version_id, side))
            .map_err(internal_error)?,
    );

    Ok((headers, bytes))
}

async fn program_version_disassembly(
    State(state): State<SharedState>,
    Path((address, version_id)): Path<(String, String)>,
    Query(query): Query<ProgramVersionBytesQuery>,
) -> ApiResult<ProgramDisassemblyResponse> {
    Pubkey::from_str(address.trim()).map_err(|_| {
        error_response(
            StatusCode::BAD_REQUEST,
            "Provide a valid Solana program address to load program disassembly.",
        )
    })?;

    let side = parse_program_version_side(query.side.as_deref())?;
    let Some(item) = state
        .db
        .program_version_disassembly(address.trim(), &version_id, side)
        .await
        .map_err(internal_error)?
    else {
        return Err(error_response(
            StatusCode::NOT_FOUND,
            "Program disassembly is not available for this version.",
        ));
    };

    Ok(Json(ProgramDisassemblyResponse { item }))
}

async fn list_metric_alerts(State(state): State<SharedState>) -> ApiResult<MetricAlertsResponse> {
    let presets = state
        .db
        .recent_protocol_metric_presets(250)
        .await
        .map_err(internal_error)?;
    let rules = state
        .db
        .list_metric_alert_rules()
        .await
        .map_err(internal_error)?;

    Ok(Json(MetricAlertsResponse { presets, rules }))
}

async fn create_metric_alert_rule(
    State(state): State<SharedState>,
    Json(payload): Json<CreateMetricAlertRuleRequest>,
) -> ApiResult<MetricAlertRule> {
    let label = payload.label.trim();
    if label.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "A metric rule label is required.",
        ));
    }

    if payload.min_value.is_none() && payload.max_value.is_none() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Set at least one threshold value for the metric rule.",
        ));
    }

    let presets = state
        .db
        .recent_protocol_metric_presets(250)
        .await
        .map_err(internal_error)?;
    let preset = presets.into_iter().find(|preset| {
        preset.protocol_slug == payload.protocol_slug
            && preset.instruction_name == payload.instruction_name
            && preset.metric_key == payload.metric_key
            && preset.subject_address == payload.subject_address
    });

    let Some(preset) = preset else {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Choose a metric from the recent observed presets so FireWatch can scope the rule safely.",
        ));
    };

    let rule = state
        .db
        .create_metric_alert_rule(
            &uuid::Uuid::new_v4().to_string(),
            label,
            &preset.protocol_slug,
            &preset.protocol_name,
            &preset.instruction_name,
            &preset.metric_key,
            &preset.metric_label,
            preset.subject_address.as_deref(),
            preset.subject_kind.as_deref(),
            preset.subject_label.as_deref(),
            payload.min_value,
            payload.max_value,
        )
        .await
        .map_err(internal_error)?;

    Ok(Json(rule))
}

async fn delete_metric_alert_rule(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let deleted = state
        .db
        .delete_metric_alert_rule(&id)
        .await
        .map_err(internal_error)?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(error_response(
            StatusCode::NOT_FOUND,
            "Metric alert rule not found.",
        ))
    }
}

async fn create_telegram_subscription(
    State(state): State<SharedState>,
    Json(payload): Json<CreateTelegramSubscriptionRequest>,
) -> ApiResult<TelegramSubscription> {
    if state.config.telegram_bot_token.is_none() {
        return Err(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "TELEGRAM_BOT_TOKEN is not configured on the backend.",
        ));
    }

    let label = payload.label.trim();
    let chat_id = payload.chat_id.trim();

    if label.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "A label is required.",
        ));
    }

    if chat_id.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "A Telegram chat id is required.",
        ));
    }

    let normalized_target_address = match payload
        .target_address
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(address) => Some(
            Pubkey::from_str(address)
                .map_err(|_| {
                    error_response(
                        StatusCode::BAD_REQUEST,
                        "Target address is not a valid Solana pubkey.",
                    )
                })?
                .to_string(),
        ),
        None => None,
    };
    let required_tags = normalize_required_tags(payload.required_tags)?;

    let subscription = state
        .db
        .create_telegram_subscription(
            &uuid::Uuid::new_v4().to_string(),
            label,
            chat_id,
            payload.thread_id,
            payload.scope,
            normalized_target_address.as_deref(),
            &required_tags,
        )
        .await
        .map_err(internal_error)?;

    Ok(Json(subscription))
}

async fn delete_telegram_subscription(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let deleted = state
        .db
        .delete_telegram_subscription(&id)
        .await
        .map_err(internal_error)?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(error_response(
            StatusCode::NOT_FOUND,
            "Telegram subscription not found.",
        ))
    }
}

async fn status(State(state): State<SharedState>) -> Result<Json<EventsSummary>, StatusCode> {
    let summary = state
        .db
        .summary()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(summary))
}

async fn health(State(state): State<SharedState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let summary = state
        .db
        .summary()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "ok": true,
        "last_processed_slot": summary.last_processed_slot,
        "pending_delivery_count": summary.pending_delivery_count,
        "telegram_enabled": state.config.telegram_bot_token.is_some(),
    })))
}

async fn live_status(State(state): State<SharedState>) -> ApiResult<LiveStatusResponse> {
    let summary = state.db.summary().await.map_err(internal_error)?;
    let telegram_bot = match telegram::bot_profile(&state).await {
        Ok(profile) => profile,
        Err(error) => {
            warn!(error = %error, "failed to load telegram bot profile for live status");
            None
        }
    };

    Ok(Json(LiveStatusResponse {
        summary,
        telegram_enabled: state.config.telegram_bot_token.is_some(),
        telegram_bot,
        sampled_at_ms: crate::models::now_ts_ms(),
        estimated_slot_time_ms: ESTIMATED_SLOT_TIME_MS,
    }))
}

async fn require_origin_token(
    State(state): State<SharedState>,
    request: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let Some(expected_token) = state.config.origin_api_token.as_deref() else {
        return Ok(next.run(request).await);
    };

    let provided_token = request
        .headers()
        .get("x-firewatch-origin-token")
        .and_then(|value| value.to_str().ok());

    if provided_token.is_some_and(|token| secure_eq(token, expected_token)) {
        return Ok(next.run(request).await);
    }

    Err(error_response(
        StatusCode::UNAUTHORIZED,
        "Provide a valid FireWatch origin token.",
    ))
}

fn secure_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    let mut diff = left.len() ^ right.len();
    let len = left.len().max(right.len());

    for index in 0..len {
        diff |= left.get(index).copied().unwrap_or(0) as usize
            ^ right.get(index).copied().unwrap_or(0) as usize;
    }

    diff == 0
}

fn error_response(status: StatusCode, message: &str) -> (StatusCode, Json<serde_json::Value>) {
    (status, Json(json!({ "error": message })))
}

fn normalize_required_tags(
    raw_tags: Option<Vec<String>>,
) -> Result<Vec<String>, (StatusCode, Json<serde_json::Value>)> {
    let mut normalized = raw_tags
        .unwrap_or_default()
        .into_iter()
        .map(|tag| tag.trim().to_ascii_lowercase())
        .filter(|tag| !tag.is_empty())
        .collect::<Vec<_>>();

    for tag in &normalized {
        if !tag.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, ':' | '_' | '-')
        }) {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "Required tags may only contain lowercase letters, digits, ':', '_' and '-'.",
            ));
        }
    }

    normalized.sort();
    normalized.dedup();

    Ok(normalized)
}

fn program_bytes_filename(address: &str, version_id: &str, side: ProgramVersionByteSide) -> String {
    let version_fragment = version_id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(12)
        .collect::<String>();
    let side = match side {
        ProgramVersionByteSide::Before => "before",
        ProgramVersionByteSide::After => "after",
    };

    format!("attachment; filename=\"program-{address}-{version_fragment}-{side}.so\"")
}

fn program_idl_filename(address: &str, version_id: &str, extension: &str) -> String {
    let version_fragment = version_id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(12)
        .collect::<String>();

    format!("attachment; filename=\"idl-{address}-{version_fragment}.{extension}\"")
}

fn parse_program_version_side(
    side: Option<&str>,
) -> Result<ProgramVersionByteSide, (StatusCode, Json<serde_json::Value>)> {
    match side.unwrap_or("after") {
        "before" => Ok(ProgramVersionByteSide::Before),
        "after" => Ok(ProgramVersionByteSide::After),
        _ => Err(error_response(
            StatusCode::BAD_REQUEST,
            "Program version side must be either before or after.",
        )),
    }
}

fn internal_error(error: impl std::fmt::Display) -> (StatusCode, Json<serde_json::Value>) {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        &format!("internal server error: {error}"),
    )
}
