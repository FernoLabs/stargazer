use crate::{
    context::SharedState,
    database::{EventListFilters, ProgramVersionByteSide},
    link_preview,
    models::{
        AdminUsersResponse, ChainEvent, CreateMetricAlertRuleRequest, CreatePaymentLinkRequest,
        CreatePromoCodeRequest, CreateTelegramSubscriptionRequest, DefiProtocolsResponse,
        EventScopeCounts, EventsResponse, EventsSummary, FirewatchUser,
        GrantUserEntitlementRequest, LiveStatusResponse, MetricAlertRule, MetricAlertsResponse,
        PaymentLink, PaymentLinkCheckoutResponse, ProgramDisassemblyResponse,
        ProgramIdlVersionsResponse, ProgramSignerStatsResponse, ProgramVersionCatalogSummary,
        ProgramVersionSummariesResponse, ProgramVersionsResponse, PromoCode, PromoCodesResponse,
        SaveTelegramConfigRequest, SaveTelegramConfigResponse, TelegramConfigSessionResponse,
        TelegramSubscription, TelegramSubscriptionSummaryResponse, TelegramSubscriptionsResponse,
        UpsertFirewatchUserRequest,
    },
    name_service, protocols, telegram,
};
use axum::{
    Json, Router,
    extract::{Path, Query, Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header},
    middleware::{Next, from_fn, from_fn_with_state},
    response::{IntoResponse, Response},
    routing::{delete, get},
};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use solana_sdk::pubkey::Pubkey;
use std::{collections::HashSet, str::FromStr, time::Instant};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{debug, warn};

const ESTIMATED_SLOT_TIME_MS: i64 = 400;
const CACHE_LIVE_STATUS: &str = "public, max-age=1, s-maxage=2, stale-while-revalidate=5";
const CACHE_STATUS: &str = "public, max-age=5, s-maxage=10, stale-while-revalidate=30";
const CACHE_EVENTS_FRESH: &str = "public, max-age=5, s-maxage=15, stale-while-revalidate=60";
const CACHE_EVENTS_FILTERED: &str = "public, max-age=10, s-maxage=60, stale-while-revalidate=300";
const CACHE_EVENT_DETAIL: &str = "public, max-age=300, s-maxage=3600, stale-while-revalidate=86400";
const CACHE_PREVIEW_CARD: &str =
    "public, max-age=300, s-maxage=86400, stale-while-revalidate=604800";
const CACHE_PROTOCOLS: &str = "public, max-age=600, s-maxage=21600, stale-while-revalidate=86400";
const CACHE_PROGRAM_STATS: &str =
    "public, max-age=300, s-maxage=1800, stale-while-revalidate=21600";
const CACHE_PROGRAM_LISTS: &str =
    "public, max-age=300, s-maxage=3600, stale-while-revalidate=86400";
const CACHE_IMMUTABLE: &str = "public, max-age=31536000, immutable";
const CACHE_NO_STORE: &str = "private, no-store";

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
        .route("/api/events/{id}/preview-card.png", get(event_preview_card))
        .route("/api/events/{id}", get(get_event))
        .route("/api/status", get(status))
        .route("/api/live-status", get(live_status))
        .route("/api/protocols", get(list_protocols))
        .route(
            "/api/programs/version-summaries",
            get(program_version_summaries),
        )
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
            "/api/promo-codes",
            get(list_promo_codes).post(create_promo_code),
        )
        .route("/api/promo-codes/{id}", delete(delete_promo_code))
        .route("/api/users", get(list_users).post(upsert_user))
        .route(
            "/api/users/entitlements",
            axum::routing::post(grant_user_entitlement),
        )
        .route(
            "/api/payment-links",
            get(list_payment_links).post(create_payment_link),
        )
        .route(
            "/api/payment-links/by-token/{token}",
            get(get_payment_link_checkout),
        )
        .route(
            "/api/config-sessions/by-token/{token}",
            get(get_config_session),
        )
        .route(
            "/api/config-sessions/by-token/{token}/save",
            axum::routing::post(save_config_session),
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
        .layer(from_fn(apply_cache_control))
        .layer(from_fn_with_state(state.clone(), require_origin_token));

    Ok(Router::new()
        .route("/healthz", get(health))
        .merge(api_router)
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http()))
}

async fn apply_cache_control(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let query = request.uri().query().map(str::to_owned);
    let mut response = next.run(request).await;

    if !response.status().is_success() || response.headers().contains_key(header::CACHE_CONTROL) {
        return response;
    }

    let cache_control =
        cache_control_for_api_path(&method, &path, query.as_deref()).unwrap_or(CACHE_NO_STORE);
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(cache_control),
    );

    response
}

fn cache_control_for_api_path(
    method: &Method,
    path: &str,
    query: Option<&str>,
) -> Option<&'static str> {
    if *method != Method::GET && *method != Method::HEAD {
        return None;
    }

    match path {
        "/api/live-status" => return Some(CACHE_LIVE_STATUS),
        "/api/status" => return Some(CACHE_STATUS),
        "/api/events" => return Some(event_list_cache_control(query)),
        "/api/protocols" => return Some(CACHE_PROTOCOLS),
        "/api/programs/version-summaries" => return Some(CACHE_PROGRAM_STATS),
        _ => {}
    }

    let segments = path
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    match segments.as_slice() {
        ["api", "events", _id] => Some(CACHE_EVENT_DETAIL),
        ["api", "events", _id, "preview-card.png"] => Some(CACHE_PREVIEW_CARD),
        ["api", "programs", _address, "signer-stats"] => Some(CACHE_PROGRAM_STATS),
        ["api", "programs", _address, versions_or_idls]
            if matches!(*versions_or_idls, "versions" | "idls") =>
        {
            Some(CACHE_PROGRAM_LISTS)
        }
        [
            "api",
            "programs",
            _address,
            versions_or_idls,
            _version_id,
            bytes_or_text,
        ] if matches!(*versions_or_idls, "versions" | "idls")
            && matches!(*bytes_or_text, "bytes" | "text") =>
        {
            Some(CACHE_IMMUTABLE)
        }
        [
            "api",
            "programs",
            _address,
            "versions",
            _version_id,
            "disassembly",
        ] => Some(CACHE_IMMUTABLE),
        _ => None,
    }
}

fn event_list_cache_control(query: Option<&str>) -> &'static str {
    let offset = query_value(query, "offset")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0);
    let has_focused_filter = [
        "protocol",
        "target_address",
        "scope",
        "severity",
        "coverage",
        "tag",
        "search",
    ]
    .iter()
    .any(|key| query_value(query, key).is_some_and(|value| !value.is_empty()));

    if offset > 0 || has_focused_filter {
        CACHE_EVENTS_FILTERED
    } else {
        CACHE_EVENTS_FRESH
    }
}

fn query_value<'a>(query: Option<&'a str>, key: &str) -> Option<&'a str> {
    query?.split('&').find_map(|pair| {
        let (pair_key, value) = pair.split_once('=').unwrap_or((pair, ""));
        (pair_key == key).then_some(value)
    })
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

    let (items, scope_counts, summary) = tokio::try_join!(
        state.db.list_events_filtered(limit, offset, filters),
        state.db.event_scope_counts(filters),
        state.db.summary()
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let total_count = count_for_scope(normalized_scope, &scope_counts);
    let hydrated_items = name_service::hydrate_events_cached(&state, items).await;
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

async fn get_event(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> ApiResult<ChainEvent> {
    let id = id.trim();
    if id.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Event not found" })),
        ));
    }

    let event = state.db.event_by_id(id).await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Could not load event" })),
        )
    })?;

    match event {
        Some(event) => Ok(Json(
            name_service::hydrate_event_cached(&state, event).await,
        )),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Event not found" })),
        )),
    }
}

async fn event_preview_card(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let id = id.trim();
    if id.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Event not found" })),
        ));
    }

    let event = state.db.event_by_id(id).await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Could not load event" })),
        )
    })?;

    let Some(event) = event else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Event not found" })),
        ));
    };

    let event = name_service::hydrate_event_cached(&state, event).await;
    let png = link_preview::render_event_preview_card(&state, &event)
        .await
        .map_err(internal_error)?;

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(
            "public, max-age=300, s-maxage=86400, stale-while-revalidate=604800",
        ),
    );

    Ok((headers, png))
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

async fn program_version_summaries(
    State(state): State<SharedState>,
) -> ApiResult<ProgramVersionSummariesResponse> {
    let mut seen = HashSet::new();
    let program_ids = protocols::curated_protocol_descriptors()
        .into_iter()
        .flat_map(|protocol| protocol.programs)
        .filter_map(|program| {
            if seen.insert(program.program_id.clone()) {
                Some(program.program_id)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let mut items = state
        .db
        .program_version_catalog_summaries(&program_ids)
        .await
        .map_err(internal_error)?;
    let summarized = items
        .iter()
        .map(|summary| summary.program_id.clone())
        .collect::<HashSet<_>>();

    for program_id in program_ids {
        if summarized.contains(&program_id) {
            continue;
        }

        items.push(ProgramVersionCatalogSummary {
            program_id,
            version_count: 0,
            diffable_count: 0,
            incomplete_count: 0,
            latest: None,
        });
    }

    Ok(Json(ProgramVersionSummariesResponse { items }))
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

async fn list_promo_codes(State(state): State<SharedState>) -> ApiResult<PromoCodesResponse> {
    let items = state.db.list_promo_codes().await.map_err(internal_error)?;

    Ok(Json(PromoCodesResponse { items }))
}

async fn create_promo_code(
    State(state): State<SharedState>,
    Json(payload): Json<CreatePromoCodeRequest>,
) -> ApiResult<PromoCode> {
    let label = payload.label.trim();
    let audience = payload.audience.trim();
    let plan_id = payload.plan_id.trim();

    if label.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "A promo label is required.",
        ));
    }

    if audience.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "A promo audience is required.",
        ));
    }

    if plan_id.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "A promo plan id is required.",
        ));
    }

    if !(1..=36).contains(&payload.duration_months) {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Promo duration must be between 1 and 36 months.",
        ));
    }

    if payload.max_redemptions.is_some_and(|value| value <= 0) {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Max redemptions must be positive when provided.",
        ));
    }

    if let (Some(starts_at), Some(expires_at)) = (payload.starts_at, payload.expires_at) {
        if expires_at <= starts_at {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "Promo expiration must be after its start time.",
            ));
        }
    }

    let code_seed = payload
        .code
        .as_deref()
        .map(str::trim)
        .filter(|code| !code.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(generate_superteam_promo_code);
    let code = normalize_promo_code(&code_seed)?;
    let existing_codes = state.db.list_promo_codes().await.map_err(internal_error)?;
    if existing_codes.iter().any(|promo| promo.code == code) {
        return Err(error_response(
            StatusCode::CONFLICT,
            "A promo code with that code already exists.",
        ));
    }

    let promo_code = state
        .db
        .create_promo_code(
            &uuid::Uuid::new_v4().to_string(),
            &code,
            label,
            audience,
            plan_id,
            payload.duration_months,
            payload.max_redemptions,
            payload.starts_at,
            payload.expires_at,
        )
        .await
        .map_err(internal_error)?;

    Ok(Json(promo_code))
}

async fn delete_promo_code(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let deleted = state
        .db
        .delete_promo_code(&id)
        .await
        .map_err(internal_error)?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(error_response(
            StatusCode::NOT_FOUND,
            "Promo code not found.",
        ))
    }
}

async fn list_users(State(state): State<SharedState>) -> ApiResult<AdminUsersResponse> {
    let users = state
        .db
        .list_firewatch_users()
        .await
        .map_err(internal_error)?;
    let entitlements = state
        .db
        .list_user_entitlements()
        .await
        .map_err(internal_error)?;
    let payment_links = state
        .db
        .list_payment_links()
        .await
        .map_err(internal_error)?;

    Ok(Json(AdminUsersResponse {
        users,
        entitlements,
        payment_links,
    }))
}

async fn upsert_user(
    State(state): State<SharedState>,
    Json(payload): Json<UpsertFirewatchUserRequest>,
) -> ApiResult<FirewatchUser> {
    let username = payload
        .telegram_username
        .as_deref()
        .map(normalize_telegram_username)
        .filter(|value| !value.is_empty());
    let label = payload
        .label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);

    if payload.telegram_user_id.is_none() && username.is_none() && label.is_none() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Provide a Telegram handle, Telegram user id, or label.",
        ));
    }

    let user = state
        .db
        .upsert_firewatch_user(
            payload.telegram_user_id,
            username.as_deref(),
            label.as_deref(),
        )
        .await
        .map_err(internal_error)?;

    Ok(Json(user))
}

async fn grant_user_entitlement(
    State(state): State<SharedState>,
    Json(payload): Json<GrantUserEntitlementRequest>,
) -> ApiResult<crate::models::UserEntitlement> {
    let user_id = payload.user_id.trim();
    let plan_id = payload.plan_id.trim();
    let source = payload.source.trim();

    if state
        .db
        .firewatch_user_by_id(user_id)
        .await
        .map_err(internal_error)?
        .is_none()
    {
        return Err(error_response(StatusCode::NOT_FOUND, "User not found."));
    }

    if plan_id.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Plan id is required.",
        ));
    }

    if source.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Source is required.",
        ));
    }

    if payload
        .duration_months
        .is_some_and(|value| !(1..=36).contains(&value))
    {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Duration must be between 1 and 36 months.",
        ));
    }

    let entitlement = state
        .db
        .grant_user_entitlement(
            user_id,
            plan_id,
            source,
            payload.duration_months,
            payload.expires_at,
        )
        .await
        .map_err(internal_error)?;

    Ok(Json(entitlement))
}

async fn list_payment_links(State(state): State<SharedState>) -> ApiResult<serde_json::Value> {
    let items = state
        .db
        .list_payment_links()
        .await
        .map_err(internal_error)?;
    Ok(Json(json!({ "items": items })))
}

async fn create_payment_link(
    State(state): State<SharedState>,
    Json(payload): Json<CreatePaymentLinkRequest>,
) -> ApiResult<PaymentLink> {
    let user = resolve_payment_link_user(&state, &payload).await?;
    let plan_id = payload
        .plan_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("paid");
    let amount = payload
        .amount
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .or_else(|| configured_payment_amount_for_plan(&state, plan_id))
        .ok_or_else(|| {
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "SOLANA_PAY_CUSTOM_PLAN_MONTHLY_AMOUNT, SOLANA_PAY_CUSTOM_PLAN_AMOUNT, or SOLANA_PAY_AMOUNT is not configured.",
            )
        })?;

    let payment_link = build_and_store_payment_link(&state, &user, plan_id, &amount).await?;
    Ok(Json(payment_link))
}

async fn get_payment_link_checkout(
    State(state): State<SharedState>,
    Path(token): Path<String>,
) -> ApiResult<PaymentLinkCheckoutResponse> {
    let token = token.trim();
    if token.is_empty() {
        return Err(error_response(
            StatusCode::NOT_FOUND,
            "Payment link not found.",
        ));
    }

    let token_hash = token_hash(token);
    let Some((payment_link, user)) = state
        .db
        .payment_link_by_token_hash(&token_hash)
        .await
        .map_err(internal_error)?
    else {
        return Err(error_response(
            StatusCode::NOT_FOUND,
            "Payment link not found.",
        ));
    };

    Ok(Json(PaymentLinkCheckoutResponse { payment_link, user }))
}

async fn get_config_session(
    State(state): State<SharedState>,
    Path(token): Path<String>,
) -> ApiResult<TelegramConfigSessionResponse> {
    let token = token.trim();
    if token.is_empty() {
        return Err(error_response(
            StatusCode::NOT_FOUND,
            "Config session not found.",
        ));
    }

    let token_hash = token_hash(token);
    let Some((session, user)) = state
        .db
        .telegram_config_session_by_token_hash(&token_hash, crate::models::now_ts())
        .await
        .map_err(internal_error)?
    else {
        return Err(error_response(
            StatusCode::NOT_FOUND,
            "Config session not found or expired.",
        ));
    };

    Ok(Json(TelegramConfigSessionResponse { session, user }))
}

async fn save_config_session(
    State(state): State<SharedState>,
    Path(token): Path<String>,
    Json(payload): Json<SaveTelegramConfigRequest>,
) -> ApiResult<SaveTelegramConfigResponse> {
    if state.config.telegram_bot_token.is_none() {
        return Err(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "TELEGRAM_BOT_TOKEN is not configured on the backend.",
        ));
    }

    let token = token.trim();
    if token.is_empty() {
        return Err(error_response(
            StatusCode::NOT_FOUND,
            "Config session not found.",
        ));
    }

    let token_hash = token_hash(token);
    let Some((session, user)) = state
        .db
        .telegram_config_session_by_token_hash(&token_hash, crate::models::now_ts())
        .await
        .map_err(internal_error)?
    else {
        return Err(error_response(
            StatusCode::NOT_FOUND,
            "Config session not found or expired.",
        ));
    };

    let requested_rules = config_rules_from_request(&payload)?;
    if requested_rules.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Select at least one protocol, program, wallet, or .sol domain.",
        ));
    }

    let has_entitlement = state
        .db
        .user_has_active_custom_entitlement(&user.id, crate::models::now_ts())
        .await
        .map_err(internal_error)?;
    let status = if has_entitlement {
        "active"
    } else {
        "pending_payment"
    };

    let mut saved_count = 0usize;
    for rule in requested_rules {
        state
            .db
            .create_telegram_subscription(
                &uuid::Uuid::new_v4().to_string(),
                &rule.label,
                Some(&user.id),
                &session.chat_id,
                None,
                rule.scope,
                rule.target_address.as_deref(),
                &rule.required_tags,
                status,
                None,
            )
            .await
            .map_err(internal_error)?;
        saved_count += 1;
    }

    let payment_link = if has_entitlement {
        None
    } else {
        let plan_id = if payload.billing.as_deref() == Some("year") {
            "paid-year"
        } else {
            "paid"
        };
        Some(create_solana_payment_link_for_user(&state, &user, plan_id).await?)
    };

    Ok(Json(SaveTelegramConfigResponse {
        saved_count,
        status: status.to_owned(),
        payment_link,
    }))
}

struct ConfigRuleDraft {
    label: String,
    scope: crate::models::TelegramEventScope,
    target_address: Option<String>,
    required_tags: Vec<String>,
}

fn config_rules_from_request(
    payload: &SaveTelegramConfigRequest,
) -> Result<Vec<ConfigRuleDraft>, (StatusCode, Json<serde_json::Value>)> {
    let mut rules = Vec::new();
    let mut seen = HashSet::new();

    for raw_slug in &payload.protocol_slugs {
        let slug = raw_slug.trim();
        if slug.is_empty() {
            continue;
        }
        let Some(protocol) = protocols::curated_protocol_descriptor(slug) else {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "Unknown protocol selected.",
            ));
        };
        let key = format!("protocol:{}", protocol.slug);
        if seen.insert(key.clone()) {
            rules.push(ConfigRuleDraft {
                label: format!("Config /watch protocol - {}", protocol.name),
                scope: crate::models::TelegramEventScope::DefiWatch,
                target_address: None,
                required_tags: vec![key],
            });
        }
    }

    for raw_program in &payload.program_ids {
        let program = raw_program.trim();
        if program.is_empty() {
            continue;
        }
        let normalized = Pubkey::from_str(program)
            .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Program address is invalid."))?
            .to_string();
        let key = format!("program:{normalized}");
        if seen.insert(key) {
            rules.push(ConfigRuleDraft {
                label: format!("Config /watch program - {normalized}"),
                scope: crate::models::TelegramEventScope::ProgramUpdates,
                target_address: Some(normalized),
                required_tags: Vec::new(),
            });
        }
    }

    for raw_wallet in &payload.wallet_addresses {
        let wallet = raw_wallet.trim();
        if wallet.is_empty() {
            continue;
        }
        let normalized = Pubkey::from_str(wallet)
            .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Wallet address is invalid."))?
            .to_string();
        let key = format!("wallet:{normalized}");
        if seen.insert(key) {
            rules.push(ConfigRuleDraft {
                label: format!("Config /watch wallet - {normalized}"),
                scope: crate::models::TelegramEventScope::WalletActivity,
                target_address: Some(normalized),
                required_tags: Vec::new(),
            });
        }
    }

    for raw_domain in &payload.sol_domains {
        let domain = normalize_sol_domain(raw_domain);
        if domain.is_empty() {
            continue;
        }
        let key = format!("sol:{domain}");
        if !seen.insert(key.clone()) {
            continue;
        }
        if let Some(protocol) = protocols::protocol_for_sol_domain(&domain) {
            rules.push(ConfigRuleDraft {
                label: format!("Config /watch .sol - {domain}"),
                scope: crate::models::TelegramEventScope::DefiWatch,
                target_address: None,
                required_tags: vec![format!("protocol:{}", protocol.slug)],
            });
        } else {
            rules.push(ConfigRuleDraft {
                label: format!("Config /watch .sol - {domain}"),
                scope: crate::models::TelegramEventScope::DefiWatch,
                target_address: None,
                required_tags: vec![format!("sol_domain:{domain}")],
            });
        }
    }

    Ok(rules)
}

fn normalize_sol_domain(value: &str) -> String {
    let raw = value.trim().trim_start_matches('@').to_ascii_lowercase();
    if raw.is_empty() {
        return String::new();
    }
    if raw.ends_with(".sol") {
        raw
    } else {
        format!("{raw}.sol")
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
            None,
            chat_id,
            payload.thread_id,
            payload.scope,
            normalized_target_address.as_deref(),
            &required_tags,
            "active",
            None,
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

fn normalize_promo_code(raw_code: &str) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    let code = raw_code.trim().to_ascii_uppercase();

    if code.is_empty() || code.len() > 64 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Promo code must be between 1 and 64 characters.",
        ));
    }

    if !code
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Promo code may only contain letters, numbers, '-' and '_'.",
        ));
    }

    Ok(code)
}

pub(crate) async fn create_solana_payment_link_for_user(
    state: &SharedState,
    user: &FirewatchUser,
    plan_id: &str,
) -> Result<PaymentLink, (StatusCode, Json<serde_json::Value>)> {
    let amount = configured_payment_amount_for_plan(state, plan_id)
        .ok_or_else(|| {
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "SOLANA_PAY_CUSTOM_PLAN_MONTHLY_AMOUNT, SOLANA_PAY_CUSTOM_PLAN_AMOUNT, or SOLANA_PAY_AMOUNT is not configured.",
            )
        })?;

    build_and_store_payment_link(state, user, plan_id, &amount).await
}

fn configured_payment_amount_for_plan(state: &SharedState, plan_id: &str) -> Option<String> {
    let monthly_amount = state.config.solana_pay_custom_plan_amount.as_deref()?;
    if plan_id != "paid-year" {
        return Some(monthly_amount.to_owned());
    }

    state
        .config
        .solana_pay_custom_plan_yearly_amount
        .clone()
        .or_else(|| multiply_sol_amount(monthly_amount, 11))
}

fn multiply_sol_amount(amount: &str, multiplier: u32) -> Option<String> {
    let value = amount.parse::<f64>().ok()? * f64::from(multiplier);
    if !value.is_finite() {
        return None;
    }

    Some(
        format!("{value:.9}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned(),
    )
}

async fn resolve_payment_link_user(
    state: &SharedState,
    payload: &CreatePaymentLinkRequest,
) -> Result<FirewatchUser, (StatusCode, Json<serde_json::Value>)> {
    if let Some(user_id) = payload
        .user_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return state
            .db
            .firewatch_user_by_id(user_id)
            .await
            .map_err(internal_error)?
            .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "User not found."));
    }

    let username = payload
        .telegram_username
        .as_deref()
        .map(normalize_telegram_username)
        .filter(|value| !value.is_empty());
    let label = payload
        .label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);

    if payload.telegram_user_id.is_none() && username.is_none() && label.is_none() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Provide a user id, Telegram handle, Telegram user id, or label.",
        ));
    }

    state
        .db
        .upsert_firewatch_user(
            payload.telegram_user_id,
            username.as_deref(),
            label.as_deref(),
        )
        .await
        .map_err(internal_error)
}

async fn build_and_store_payment_link(
    state: &SharedState,
    user: &FirewatchUser,
    plan_id: &str,
    amount: &str,
) -> Result<PaymentLink, (StatusCode, Json<serde_json::Value>)> {
    let recipient = state
        .config
        .solana_pay_recipient
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "SOLANA_PAY_RECIPIENT is not configured.",
            )
        })?;
    let recipient = validate_pubkey_string(recipient, "SOLANA_PAY_RECIPIENT")?;
    let spl_token = state
        .config
        .solana_pay_spl_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| validate_pubkey_string(value, "SOLANA_PAY_SPL_TOKEN"))
        .transpose()?;
    validate_amount_string(amount)?;

    let token = generate_payment_token();
    let hash = token_hash(&token);
    let reference = payment_reference(&token);
    let public_url = format!(
        "{}/pay/{}",
        state.config.firewatch_web_base_url.trim_end_matches('/'),
        token
    );
    let username = user
        .telegram_username
        .as_ref()
        .map(|username| format!("@{username}"))
        .or_else(|| user.label.clone())
        .unwrap_or_else(|| "FireWatch user".to_owned());
    let label = state.config.solana_pay_label.clone();
    let message = format!("FireWatch Custom Plan for {username}");
    let memo = format!("firewatch:{}:{}", user.id, &reference[..8]);
    let solana_pay_url = solana_pay_url(
        &recipient,
        amount,
        spl_token.as_deref(),
        &reference,
        &label,
        &message,
        &memo,
    );
    let expires_at = crate::models::now_ts() + state.config.solana_pay_link_ttl_hours * 60 * 60;

    state
        .db
        .create_payment_link(
            &user.id,
            plan_id,
            &hash,
            &public_url,
            &solana_pay_url,
            &reference,
            &recipient,
            amount,
            spl_token.as_deref(),
            &label,
            &message,
            &memo,
            expires_at,
        )
        .await
        .map_err(internal_error)
}

fn normalize_telegram_username(value: &str) -> String {
    value.trim().trim_start_matches('@').to_ascii_lowercase()
}

fn validate_pubkey_string(
    value: &str,
    label: &str,
) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    Pubkey::from_str(value)
        .map(|pubkey| pubkey.to_string())
        .map_err(|_| {
            error_response(
                StatusCode::BAD_REQUEST,
                &format!("{label} is not a valid Solana pubkey."),
            )
        })
}

fn validate_amount_string(amount: &str) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let amount = amount.trim();
    if amount.is_empty()
        || !amount
            .chars()
            .all(|character| character.is_ascii_digit() || character == '.')
        || amount.matches('.').count() > 1
    {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Solana Pay amount must be a positive decimal string.",
        ));
    }

    let parsed = amount.parse::<f64>().map_err(|_| {
        error_response(
            StatusCode::BAD_REQUEST,
            "Solana Pay amount must be a positive decimal string.",
        )
    })?;
    if parsed <= 0.0 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Solana Pay amount must be greater than zero.",
        ));
    }

    Ok(())
}

fn generate_payment_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn token_hash(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    hex::encode(digest)
}

fn payment_reference(token: &str) -> String {
    let digest = Sha256::digest(format!("firewatch-solana-pay:{token}").as_bytes());
    bs58::encode(digest).into_string()
}

fn solana_pay_url(
    recipient: &str,
    amount: &str,
    spl_token: Option<&str>,
    reference: &str,
    label: &str,
    message: &str,
    memo: &str,
) -> String {
    let mut params = vec![
        format!("amount={}", percent_encode(amount)),
        format!("reference={}", percent_encode(reference)),
        format!("label={}", percent_encode(label)),
        format!("message={}", percent_encode(message)),
        format!("memo={}", percent_encode(memo)),
    ];
    if let Some(spl_token) = spl_token {
        params.insert(1, format!("spl-token={}", percent_encode(spl_token)));
    }

    format!("solana:{recipient}?{}", params.join("&"))
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        let character = byte as char;
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '~') {
            encoded.push(character);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn generate_superteam_promo_code() -> String {
    let suffix = uuid::Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect::<String>()
        .to_ascii_uppercase();
    format!("SUPERTEAM-{suffix}")
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
