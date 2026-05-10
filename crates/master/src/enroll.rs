use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use crate::http::{ApiError, ApiResult};
use crate::pki::is_valid_node_id;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct EnrollRequest {
    pub node_id: String,
    pub enrollment_token: String,
    pub csr_pem: String,
}

#[derive(Serialize)]
pub struct EnrollResponse {
    pub node_cert_pem: String,
    pub ca_cert_pem: String,
    pub not_after_unix_ms: i64,
}

pub async fn enroll_handler(
    State(s): State<AppState>,
    Json(req): Json<EnrollRequest>,
) -> ApiResult<Json<EnrollResponse>> {
    if !is_valid_node_id(&req.node_id) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "invalid node_id"));
    }
    if req.enrollment_token.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "enrollment_token required",
        ));
    }
    if req.csr_pem.is_empty() {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "csr_pem required"));
    }

    // Atomic single-use token consume — two racing requests can't both succeed.
    let consumed: Option<(String,)> = sqlx::query_as(
        "UPDATE nodes
            SET enrollment_token = NULL,
                updated_at = now()
          WHERE id = $1
            AND enrollment_token = $2
          RETURNING id",
    )
    .bind(&req.node_id)
    .bind(&req.enrollment_token)
    .fetch_optional(&s.db)
    .await
    .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;

    if consumed.is_none() {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "enrollment denied"));
    }

    let signed = s
        .pki
        .sign_node_cert(&req.node_id, &req.csr_pem)
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, format!("sign failed: {e}")))?;

    let not_after_chrono =
        chrono::DateTime::<chrono::Utc>::from_timestamp(signed.not_after.unix_timestamp(), 0)
            .ok_or_else(|| {
                ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "not_after timestamp")
            })?;

    sqlx::query(
        "UPDATE nodes
            SET cert_fingerprint = $2,
                cert_serial = $3,
                cert_not_after = $4,
                enrolled_at = COALESCE(enrolled_at, now()),
                updated_at = now()
          WHERE id = $1",
    )
    .bind(&req.node_id)
    .bind(&signed.fingerprint_hex)
    .bind(&signed.serial_hex)
    .bind(not_after_chrono)
    .execute(&s.db)
    .await
    .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;

    tracing::info!(
        node = %req.node_id,
        fingerprint = %signed.fingerprint_hex,
        "node enrolled, cert issued"
    );

    Ok(Json(EnrollResponse {
        node_cert_pem: signed.cert_pem,
        ca_cert_pem: s.pki.ca_cert_pem.clone(),
        not_after_unix_ms: signed.not_after.unix_timestamp() * 1_000,
    }))
}
