use crate::store::{NewPairingClaim, PairingClaimStatus, Store};
use crate::{
    normalize_wallet_address, pairing_token_hash, unix_timestamp, validate_bot_username,
    BillingMode, PolicyState, PAIRING_TOKEN_BYTES,
};
use anyhow::{Context, Result};
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
pub struct AlertApiState {
    store: Store,
    bot_username: Arc<str>,
    bot_polling_configured: bool,
}

impl AlertApiState {
    pub fn new(store: Store, bot_username: &str, bot_polling_configured: bool) -> Result<Self> {
        Ok(Self {
            store,
            bot_username: validate_bot_username(bot_username)?.into(),
            bot_polling_configured,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreatePairingRequest {
    wallet_address: String,
    critical_alerts: bool,
    non_critical_alerts: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum PairingStatus {
    PendingTelegram,
    Paired,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum PaymentState {
    Unconfigured,
    NotRequired,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum EntitlementState {
    NotConfigured,
    GrantedOnPairing,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PolicyView {
    critical_alerts: bool,
    non_critical_alerts: bool,
    state: PolicyState,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BillingView {
    mode: BillingMode,
    payment_state: PaymentState,
    entitlement_state: EntitlementState,
}

impl BillingView {
    fn from_mode(mode: BillingMode) -> Self {
        match mode {
            BillingMode::Unconfigured => Self {
                mode,
                payment_state: PaymentState::Unconfigured,
                entitlement_state: EntitlementState::NotConfigured,
            },
            BillingMode::Pilot => Self {
                mode,
                payment_state: PaymentState::NotRequired,
                entitlement_state: EntitlementState::GrantedOnPairing,
            },
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreatePairingResponse {
    claim_token: String,
    telegram_url: String,
    expires_at: i64,
    status: PairingStatus,
    wallet_address: String,
    policy: PolicyView,
    billing: BillingView,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PairingStatusResponse {
    expires_at: i64,
    status: PairingStatus,
    wallet_address: String,
    policy: PolicyView,
    billing: BillingView,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    ok: bool,
    service: &'static str,
    telegram_polling_configured: bool,
    alert_delivery_configured: bool,
    billing: BillingView,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code,
            message: message.into(),
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "invalid_pairing_bearer",
            message: message.into(),
        }
    }

    fn not_found() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "pairing_claim_not_found",
            message: "Pairing claim not found.".to_owned(),
        }
    }

    fn unavailable(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code,
            message: message.into(),
        }
    }

    fn internal(error: anyhow::Error) -> Self {
        tracing::error!(error = %error, "alert-control request failed");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            message: "The alert-control service could not complete the request.".to_owned(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorEnvelope {
                error: ErrorBody {
                    code: self.code,
                    message: self.message,
                },
            }),
        )
            .into_response()
    }
}

pub fn router(state: AlertApiState, cors_origins: &[String]) -> Result<Router> {
    let cors = cors_layer(cors_origins)?;
    Ok(Router::new()
        .route("/health", get(health))
        .route("/api/alert-pairings", post(create_pairing))
        .route("/api/alert-pairings/status", get(pairing_status))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(cors)
        .layer(middleware::from_fn(security_headers))
        .with_state(state))
}

async fn health(State(state): State<AlertApiState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        service: "stargazer-alert-control",
        telegram_polling_configured: state.bot_polling_configured,
        alert_delivery_configured: false,
        billing: BillingView::from_mode(state.store.billing_mode()),
    })
}

async fn create_pairing(
    State(state): State<AlertApiState>,
    Json(request): Json<CreatePairingRequest>,
) -> Result<(StatusCode, Json<CreatePairingResponse>), ApiError> {
    if !state.bot_polling_configured {
        return Err(ApiError::unavailable(
            "telegram_bot_unavailable",
            "Telegram pairing is not available because the bot poller is not configured.",
        ));
    }
    if !request.critical_alerts && !request.non_critical_alerts {
        return Err(ApiError::bad_request(
            "empty_alert_policy",
            "Enable critical alerts, non-critical alerts, or both.",
        ));
    }
    let wallet_address = normalize_wallet_address(&request.wallet_address)
        .map_err(|error| ApiError::bad_request("invalid_wallet_address", error.to_string()))?;
    let created_at = unix_timestamp().map_err(ApiError::internal)?;
    let mut random = [0_u8; PAIRING_TOKEN_BYTES];
    OsRng.fill_bytes(&mut random);
    let claim_token = hex::encode(random);
    let token_hash = pairing_token_hash(&claim_token).map_err(ApiError::internal)?;
    let claim = NewPairingClaim {
        token_hash,
        wallet_address: wallet_address.clone(),
        critical_alerts: request.critical_alerts,
        non_critical_alerts: request.non_critical_alerts,
        created_at,
    };
    let store = state.store.clone();
    let expires_at = tokio::task::spawn_blocking(move || store.create_pairing_claim(&claim))
        .await
        .context("pairing claim worker stopped")
        .and_then(|result| result)
        .map_err(ApiError::internal)?;
    let telegram_url = format!("https://t.me/{}?start={}", state.bot_username, claim_token);
    Ok((
        StatusCode::CREATED,
        Json(CreatePairingResponse {
            claim_token,
            telegram_url,
            expires_at,
            status: PairingStatus::PendingTelegram,
            wallet_address,
            policy: PolicyView {
                critical_alerts: request.critical_alerts,
                non_critical_alerts: request.non_critical_alerts,
                state: PolicyState::PendingTelegram,
            },
            billing: BillingView::from_mode(state.store.billing_mode()),
        }),
    ))
}

async fn pairing_status(
    State(state): State<AlertApiState>,
    headers: HeaderMap,
) -> Result<Json<PairingStatusResponse>, ApiError> {
    let claim_token = bearer_token(&headers)?;
    let now = unix_timestamp().map_err(ApiError::internal)?;
    let store = state.store.clone();
    let status = tokio::task::spawn_blocking(move || store.pairing_claim_status(&claim_token, now))
        .await
        .context("pairing status worker stopped")
        .and_then(|result| result)
        .map_err(|error| {
            if error.to_string() == "invalid pairing token" {
                ApiError::unauthorized("Invalid pairing bearer token.")
            } else {
                ApiError::internal(error)
            }
        })?
        .ok_or_else(ApiError::not_found)?;
    Ok(Json(status_response(status, state.store.billing_mode())))
}

fn status_response(claim: PairingClaimStatus, billing_mode: BillingMode) -> PairingStatusResponse {
    let (status, policy_state) = if claim.claimed_at.is_some() {
        (
            PairingStatus::Paired,
            claim
                .policy_state
                .unwrap_or(PolicyState::AwaitingEntitlement),
        )
    } else {
        (PairingStatus::PendingTelegram, PolicyState::PendingTelegram)
    };
    PairingStatusResponse {
        expires_at: claim.expires_at,
        status,
        wallet_address: claim.wallet_address,
        policy: PolicyView {
            critical_alerts: claim.critical_alerts,
            non_critical_alerts: claim.non_critical_alerts,
            state: policy_state,
        },
        billing: BillingView::from_mode(billing_mode),
    }
}

fn bearer_token(headers: &HeaderMap) -> Result<String, ApiError> {
    let value = headers
        .get(header::AUTHORIZATION)
        .ok_or_else(|| ApiError::unauthorized("Missing pairing bearer token."))?
        .to_str()
        .map_err(|_| ApiError::unauthorized("Invalid pairing bearer token."))?;
    let (scheme, token) = value
        .split_once(' ')
        .ok_or_else(|| ApiError::unauthorized("Invalid pairing bearer token."))?;
    if !scheme.eq_ignore_ascii_case("bearer") || token.contains(char::is_whitespace) {
        return Err(ApiError::unauthorized("Invalid pairing bearer token."));
    }
    pairing_token_hash(token)
        .map_err(|_| ApiError::unauthorized("Invalid pairing bearer token."))?;
    Ok(token.to_owned())
}

fn cors_layer(origins: &[String]) -> Result<CorsLayer> {
    let mut cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);
    if origins.iter().any(|origin| origin == "*") {
        return Ok(cors.allow_origin(Any));
    }
    let values = origins
        .iter()
        .map(|origin| {
            origin
                .parse::<HeaderValue>()
                .with_context(|| format!("invalid CORS origin {origin}"))
        })
        .collect::<Result<Vec<_>>>()?;
    cors = cors.allow_origin(values);
    Ok(cors)
}

async fn security_headers(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    response
        .headers_mut()
        .insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use serde_json::{json, Value};
    use stellar_xdr::{MuxedAccount, Uint256};
    use tempfile::TempDir;
    use tower::ServiceExt;

    fn fixture() -> (TempDir, Router, String, Store) {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::initialize(
            temp.path().join("control.sqlite"),
            BillingMode::Unconfigured,
            30,
        )
        .unwrap();
        let state = AlertApiState::new(store.clone(), "stargazer_test_bot", true).unwrap();
        let app = router(state, &["http://localhost:5173".to_owned()]).unwrap();
        let wallet = MuxedAccount::Ed25519(Uint256([23; 32])).to_string();
        (temp, app, wallet, store)
    }

    #[tokio::test]
    async fn create_and_poll_a_pairing_claim_without_putting_secret_in_status_url() {
        let (_temp, app, wallet, store) = fixture();
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/alert-pairings")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "walletAddress": wallet,
                            "criticalAlerts": true,
                            "nonCriticalAlerts": true
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "no-store, max-age=0"
        );
        assert_eq!(response.headers()["referrer-policy"], "no-referrer");
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        let token = body["claimToken"].as_str().unwrap();
        assert_eq!(token.len(), 64);
        assert_eq!(body["status"], "pendingTelegram");
        assert_eq!(body["billing"]["paymentState"], "unconfigured");

        let status = app
            .clone()
            .oneshot(
                Request::get("/api/alert-pairings/status")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let body: Value =
            serde_json::from_slice(&status.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(body["status"], "pendingTelegram");
        assert!(body.get("claimToken").is_none());

        let claim_token = token.to_owned();
        tokio::task::spawn_blocking(move || {
            store.claim_pairing(
                &claim_token,
                &crate::store::TelegramIdentity {
                    chat_id: 77,
                    user_id: 77,
                    username: Some("paired_user".to_owned()),
                },
                unix_timestamp().unwrap(),
            )
        })
        .await
        .unwrap()
        .unwrap();
        let paired = app
            .oneshot(
                Request::get("/api/alert-pairings/status")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body: Value =
            serde_json::from_slice(&paired.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(body["status"], "paired");
        assert_eq!(body["policy"]["state"], "awaitingEntitlement");
    }

    #[tokio::test]
    async fn expired_status_bearer_discloses_no_claim_or_policy_metadata() {
        let (_temp, app, wallet, store) = fixture();
        let token = "ab".repeat(PAIRING_TOKEN_BYTES);
        let created_at = unix_timestamp().unwrap() - crate::PAIRING_TTL_SECONDS - 10;
        store
            .create_pairing_claim(&NewPairingClaim {
                token_hash: pairing_token_hash(&token).unwrap(),
                wallet_address: wallet.clone(),
                critical_alerts: true,
                non_critical_alerts: true,
                created_at,
            })
            .unwrap();
        store
            .claim_pairing(
                &token,
                &crate::store::TelegramIdentity {
                    chat_id: 78,
                    user_id: 78,
                    username: Some("expired_user".to_owned()),
                },
                created_at + 1,
            )
            .unwrap();

        let response = app
            .oneshot(
                Request::get("/api/alert-pairings/status")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_text = std::str::from_utf8(&body).unwrap();
        assert!(!body_text.contains(&wallet));
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["error"]["code"], "pairing_claim_not_found");
        assert!(body.get("policy").is_none());
    }

    #[tokio::test]
    async fn rejects_invalid_or_empty_policies() {
        let (_temp, app, _wallet, _store) = fixture();
        let response = app
            .oneshot(
                Request::post("/api/alert-pairings")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "walletAddress": "not-a-wallet",
                            "criticalAlerts": false,
                            "nonCriticalAlerts": false
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(body["error"]["code"], "empty_alert_policy");
    }

    #[tokio::test]
    async fn botless_api_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        let store =
            Store::initialize(temp.path().join("control.sqlite"), BillingMode::Pilot, 30).unwrap();
        let app = router(
            AlertApiState::new(store, "stargazer_test_bot", false).unwrap(),
            &[],
        )
        .unwrap();
        let response = app
            .oneshot(
                Request::post("/api/alert-pairings")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "walletAddress": MuxedAccount::Ed25519(Uint256([1; 32])).to_string(),
                            "criticalAlerts": true,
                            "nonCriticalAlerts": false
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn health_never_claims_alert_delivery_is_connected() {
        let (_temp, app, _wallet, _store) = fixture();
        let response = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(body["telegramPollingConfigured"], true);
        assert_eq!(body["alertDeliveryConfigured"], false);
    }
}
