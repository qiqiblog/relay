//! Enroll RPC service (M4.2).
//!
//! Runs on a dedicated TLS listener (`MASTER_ENROLL_ADDR`, default `:7444`)
//! using the master's CA-signed server cert but **without** requiring a
//! client cert — the client (a fresh node) hasn't been issued one yet.
//!
//! Flow:
//!   1. Operator creates a node row in the master DB; `enrollment_token`
//!      is a random secret returned exactly once to the operator.
//!   2. Node generates a local keypair, builds a CSR, calls Enroll with
//!      its `node_id`, `enrollment_token`, and `csr_pem`.
//!   3. Master atomically clears the token (`UPDATE ... WHERE token=$1
//!      RETURNING ...`) — the swap only fires for the node row that owned
//!      the matching token, so brute force / replay are limited to a
//!      single attempt per token value, and a token only grants one cert.
//!   4. Master signs a client cert for the node, persists fingerprint /
//!      serial / not_after on the row, returns cert + ca_cert.

use std::net::SocketAddr;
use std::sync::Arc;

use tonic::transport::{Identity, Server, ServerTlsConfig};
use tonic::{Request, Response, Status};

use relay_proto::v1::{
    enroll_service_server::{EnrollService, EnrollServiceServer},
    EnrollRequest, EnrollResponse,
};

use crate::pki::{is_valid_node_id, Pki};
use crate::state::AppState;

pub async fn serve(addr: SocketAddr, state: AppState, pki: Arc<Pki>) -> anyhow::Result<()> {
    let identity = Identity::from_pem(&pki.server_cert_pem, &pki.server_key_pem);
    let tls = ServerTlsConfig::new().identity(identity);

    let svc = EnrollSvc { state, pki };
    Server::builder()
        .tls_config(tls)?
        .add_service(EnrollServiceServer::new(svc))
        .serve(addr)
        .await?;
    Ok(())
}

#[derive(Clone)]
struct EnrollSvc {
    state: AppState,
    pki: Arc<Pki>,
}

#[tonic::async_trait]
impl EnrollService for EnrollSvc {
    async fn enroll(
        &self,
        req: Request<EnrollRequest>,
    ) -> Result<Response<EnrollResponse>, Status> {
        let req = req.into_inner();

        if !is_valid_node_id(&req.node_id) {
            return Err(Status::invalid_argument("invalid node_id"));
        }
        if req.enrollment_token.is_empty() {
            return Err(Status::invalid_argument("enrollment_token required"));
        }
        if req.csr_pem.is_empty() {
            return Err(Status::invalid_argument("csr_pem required"));
        }

        // Atomic single-use token consume. The UPDATE only matches when the
        // submitted token matches the row's current token AND the row hasn't
        // already been enrolled (token NOT NULL). On hit we clear the token
        // in the same statement so two racing requests can't both succeed.
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
        .fetch_optional(&self.state.db)
        .await
        .map_err(|e| Status::internal(format!("db: {e}")))?;

        if consumed.is_none() {
            // Either: node_id unknown, token wrong, or token already used.
            // Don't leak which to avoid enumeration.
            return Err(Status::permission_denied("enrollment denied"));
        }

        let signed = self
            .pki
            .sign_node_cert(&req.node_id, &req.csr_pem)
            .map_err(|e| Status::invalid_argument(format!("sign failed: {e}")))?;

        // Persist cert binding for revocation / handshake check.
        let not_after_chrono =
            chrono::DateTime::<chrono::Utc>::from_timestamp(signed.not_after.unix_timestamp(), 0)
                .ok_or_else(|| Status::internal("not_after timestamp"))?;

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
        .execute(&self.state.db)
        .await
        .map_err(|e| Status::internal(format!("db: {e}")))?;

        tracing::info!(
            node = %req.node_id,
            fingerprint = %signed.fingerprint_hex,
            "node enrolled, cert issued"
        );

        Ok(Response::new(EnrollResponse {
            node_cert_pem: signed.cert_pem,
            ca_cert_pem: self.pki.ca_cert_pem.clone(),
            not_after_unix_ms: signed.not_after.unix_timestamp() * 1_000,
        }))
    }
}
