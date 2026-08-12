use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderMap, Method, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use tokio::sync::Mutex;

use crate::bounds::{
    MAX_HTTP_HEADER_BYTES, MAX_HTTP_HEADERS, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES,
};
use crate::canonical::parse_canonical;
use crate::jwks::{HttpsJwksProvider, JwksProvider};
use crate::oidc::{PublisherRequest, authenticate};
use crate::receipt::{FileReceiptSigner, ReceiptSigner, raw_request_sha256, request_identity};
use crate::store::{DurableStore, IssueInput};
use crate::{PublisherError, ServiceConfig};

#[derive(Clone, Debug)]
pub struct PublisherResponse {
    pub body: Vec<u8>,
}

pub struct Publisher {
    config: Arc<ServiceConfig>,
    jwks: Arc<dyn JwksProvider>,
    store: Mutex<DurableStore>,
    signer: Arc<dyn ReceiptSigner>,
}

impl Publisher {
    pub fn open(config: ServiceConfig) -> Result<Arc<Self>, PublisherError> {
        config.validate()?;
        let service_identity = config.service_identity()?;
        let jwks = Arc::new(HttpsJwksProvider::new(
            &config.jwks_url,
            config.network_deadline(),
        )?);
        let signer = Arc::new(FileReceiptSigner::load(
            service_identity,
            &config.signing_key_path,
        )?);
        let store = DurableStore::open(&config.database_path)?;
        Ok(Arc::new(Self {
            config: Arc::new(config),
            jwks,
            store: Mutex::new(store),
            signer,
        }))
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        config: ServiceConfig,
        jwks: Arc<dyn JwksProvider>,
        store: DurableStore,
        signer: Arc<dyn ReceiptSigner>,
    ) -> Arc<Self> {
        Arc::new(Self {
            config: Arc::new(config),
            jwks,
            store: Mutex::new(store),
            signer,
        })
    }

    pub async fn issue(
        &self,
        body: &[u8],
        bearer: &str,
    ) -> Result<PublisherResponse, PublisherError> {
        let value =
            parse_canonical(body, MAX_REQUEST_BYTES).map_err(|_| PublisherError::Request)?;
        let request: PublisherRequest =
            serde_json::from_value(value).map_err(|_| PublisherError::Request)?;
        let deadline = tokio::time::Instant::now() + self.config.network_deadline();
        let authorization =
            authenticate(&self.config, self.jwks.clone(), &request, bearer, deadline).await?;
        let identity = request_identity(body);
        let sha256 = raw_request_sha256(body);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| PublisherError::Signing)?
            .as_secs() as i64;
        let issued_at = now.max(authorization.issued_at);
        let body = self.store.lock().await.issue(IssueInput {
            replay_identity: &authorization.replay_identity,
            request_identity: &identity,
            request_sha256: &sha256,
            request_body: body,
            request: &request,
            issued_at,
            signature_domain: &self.config.signature_domain,
            signer: self.signer.as_ref(),
        })?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(PublisherError::Store);
        }
        Ok(PublisherResponse { body })
    }
}

pub fn router(publisher: Arc<Publisher>) -> Router {
    Router::new()
        .route("/v1/receipts", post(receipts))
        .fallback(not_found)
        .with_state(publisher)
}

async fn receipts(State(publisher): State<Arc<Publisher>>, request: Request<Body>) -> Response {
    match bounded_http_request(request).await {
        Ok((body, bearer)) => match publisher.issue(&body, &bearer).await {
            Ok(response) => (
                StatusCode::OK,
                [(CONTENT_TYPE, "application/json")],
                response.body,
            )
                .into_response(),
            Err(error) => error_response(&error),
        },
        Err(error) => error_response(&error),
    }
}

async fn bounded_http_request(request: Request<Body>) -> Result<(Vec<u8>, String), PublisherError> {
    if request.method() != Method::POST
        || request.uri().path() != "/v1/receipts"
        || request.uri().query().is_some()
    {
        return Err(PublisherError::Request);
    }
    validate_headers(request.headers())?;
    let content_type = request
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    if content_type != Some("application/json") {
        return Err(PublisherError::Request);
    }
    let authorization = request.headers().get_all(AUTHORIZATION);
    if authorization.iter().count() != 1 {
        return Err(PublisherError::Authentication);
    }
    let authorization = authorization
        .iter()
        .next()
        .and_then(|value| value.to_str().ok())
        .ok_or(PublisherError::Authentication)?;
    let bearer = authorization
        .strip_prefix("Bearer ")
        .filter(|value| !value.is_empty() && !value.contains(char::is_whitespace))
        .ok_or(PublisherError::Authentication)?
        .to_owned();
    let body = to_bytes(request.into_body(), MAX_REQUEST_BYTES)
        .await
        .map_err(|_| PublisherError::Request)?;
    Ok((body.to_vec(), bearer))
}

fn validate_headers(headers: &HeaderMap) -> Result<(), PublisherError> {
    if headers.len() > MAX_HTTP_HEADERS {
        return Err(PublisherError::Request);
    }
    let total = headers.iter().try_fold(0usize, |total, (name, value)| {
        total
            .checked_add(name.as_str().len())
            .and_then(|value_total| value_total.checked_add(value.as_bytes().len()))
    });
    if total.is_none_or(|total| total > MAX_HTTP_HEADER_BYTES) {
        return Err(PublisherError::Request);
    }
    Ok(())
}

fn error_response(error: &PublisherError) -> Response {
    let status = match error {
        PublisherError::Request => StatusCode::BAD_REQUEST,
        PublisherError::Authentication | PublisherError::Jwks => StatusCode::UNAUTHORIZED,
        PublisherError::ReplayConflict => StatusCode::CONFLICT,
        PublisherError::Config | PublisherError::Store | PublisherError::Signing => {
            StatusCode::SERVICE_UNAVAILABLE
        }
    };
    let body = format!(
        "{{\"error\":\"{}\",\"schema_version\":1}}\n",
        error.public_code()
    );
    (status, [(CONTENT_TYPE, "application/json")], body).into_response()
}

async fn not_found() -> Response {
    error_response(&PublisherError::Request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use base64::Engine;
    use serde_json::Value;
    use sha2::{Digest, Sha256};
    use std::process::Command;
    use tower::ServiceExt;

    use crate::jwks::StaticJwksProvider;
    use crate::receipt::TestSigner;
    use crate::test_support::{config, fixture, jwks, secure_tempdir};

    fn test_publisher(
        temp: &tempfile::TempDir,
        provider: StaticJwksProvider,
        signer: Arc<TestSigner>,
    ) -> Arc<Publisher> {
        let config = config(temp.path().join("publisher.db"));
        let store = DurableStore::open(&config.database_path).unwrap();
        Publisher::for_test(config, Arc::new(provider), store, signer)
    }

    #[tokio::test]
    async fn full_service_path_emits_client_receipt_schema() {
        let temp = secure_tempdir();
        let signer = Arc::new(TestSigner::new("test-publisher-v1"));
        let publisher = test_publisher(&temp, StaticJwksProvider::new(jwks("fixture-key")), signer);
        let fixture = fixture();
        let first = publisher
            .issue(&fixture.request_body, &fixture.token)
            .await
            .unwrap();
        let second = publisher
            .issue(&fixture.request_body, &fixture.token)
            .await
            .unwrap();
        assert_eq!(first.body, second.body);
        let response: Value = serde_json::from_slice(&first.body).unwrap();
        assert_eq!(response["schema_version"], 1);
        assert_eq!(
            response["request_sha256"],
            raw_request_sha256(&fixture.request_body)
        );
        let receipt = base64::engine::general_purpose::STANDARD
            .decode(response["publisher_receipt_base64"].as_str().unwrap())
            .unwrap();
        let receipt = String::from_utf8(receipt).unwrap();
        assert!(receipt.starts_with("publisher_contract_receipt_schema_version\t2\n"));
        assert!(receipt.contains("\nsignature_algorithm\ted25519\n"));
        assert!(receipt.contains("\nsigning_key_id\ttest-publisher-v1\n"));
        assert!(receipt.ends_with('\n'));
    }

    #[tokio::test]
    async fn existing_python_client_accepts_service_receipt() {
        let temp = secure_tempdir();
        let signer = Arc::new(TestSigner::new("test-publisher-v1"));
        let publisher = test_publisher(
            &temp,
            StaticJwksProvider::new(jwks("fixture-key")),
            signer.clone(),
        );
        let fixture = fixture();
        let response = publisher
            .issue(&fixture.request_body, &fixture.token)
            .await
            .unwrap();
        let response_path = temp.path().join("response.json");
        std::fs::write(&response_path, &response.body).unwrap();
        let response_value: Value = serde_json::from_slice(&response.body).unwrap();

        let expected = serde_json::json!({
            "archive_sha256": fixture.request.archive_sha256,
            "baseline_status_sha256": fixture.request.baseline_status_sha256,
            "candidate_head": fixture.request.candidate_head,
            "candidate_status_sha256": fixture.request.candidate_status_sha256,
            "default_tip": fixture.request.default_tip,
            "hardware_lane": fixture.request.hardware_lane,
            "logical_destination": fixture.request.logical_destination,
            "manifest_path": fixture.request.manifest_path,
            "manifest_sha256": fixture.request.manifest_sha256,
            "source_commit": fixture.request.source_commit,
            "source_tree": fixture.request.source_tree,
            "target": fixture.request.target,
        });
        let expected_path = temp.path().join("expected.json");
        std::fs::write(&expected_path, serde_json::to_vec(&expected).unwrap()).unwrap();

        let trusted = temp.path().join("trusted");
        let keys = trusted.join("keys");
        std::fs::create_dir_all(&keys).unwrap();
        let public = signer.public_key_pem();
        std::fs::write(keys.join("test-publisher-v1.pem"), &public).unwrap();
        let digest = Sha256::digest(public.as_bytes());
        let digest = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let policy = trusted.join("trust.tsv");
        std::fs::write(
            &policy,
            format!(
                "parity_trust_policy_schema_version\t2\n\
trust_domain\ttest\n\
metadata_path_count\t0\n\
key_count\t1\n\
key\t0000\tpublisher\ttest-publisher-v1\tkeys/test-publisher-v1.pem\t{digest}\ted25519\n"
            ),
        )
        .unwrap();
        let runner = temp.path().join("runner");
        std::fs::create_dir(&runner).unwrap();
        assert_eq!(response_value["schema_version"], 1);
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let status = Command::new("python3")
            .arg(root.join("scripts/tests/protected-publisher-service-client.py"))
            .args(["--response", response_path.to_str().unwrap()])
            .args(["--expected", expected_path.to_str().unwrap()])
            .args(["--trusted-root", trusted.to_str().unwrap()])
            .args(["--trust-policy", policy.to_str().unwrap()])
            .args(["--runner-temp", runner.to_str().unwrap()])
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn http_surface_is_bounded_and_canonical() {
        let temp = secure_tempdir();
        let publisher = test_publisher(
            &temp,
            StaticJwksProvider::new(jwks("fixture-key")),
            Arc::new(TestSigner::new("test-publisher-v1")),
        );
        let fixture = fixture();
        let request = Request::builder()
            .method(Method::POST)
            .uri("/v1/receipts")
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, format!("Bearer {}", fixture.token))
            .body(Body::from(fixture.request_body))
            .unwrap();
        let response = router(publisher.clone()).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), MAX_RESPONSE_BYTES)
            .await
            .unwrap();
        assert!(body.ends_with(b"\n"));

        let oversized = Request::builder()
            .method(Method::POST)
            .uri("/v1/receipts")
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, "Bearer x.y.z")
            .body(Body::from(vec![b'a'; MAX_REQUEST_BYTES + 1]))
            .unwrap();
        let response = router(publisher).oneshot(oversized).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn outage_signing_and_database_failures_do_not_emit_receipts() {
        let fixture = fixture();
        let outage_temp = secure_tempdir();
        let outage = test_publisher(
            &outage_temp,
            StaticJwksProvider::outage(),
            Arc::new(TestSigner::new("test-publisher-v1")),
        );
        assert!(matches!(
            outage.issue(&fixture.request_body, &fixture.token).await,
            Err(PublisherError::Jwks)
        ));

        let signing_temp = secure_tempdir();
        let signing = test_publisher(
            &signing_temp,
            StaticJwksProvider::new(jwks("fixture-key")),
            Arc::new(TestSigner::failing("test-publisher-v1")),
        );
        assert!(matches!(
            signing.issue(&fixture.request_body, &fixture.token).await,
            Err(PublisherError::Signing)
        ));
        assert_eq!(signing.store.lock().await.count(), 0);

        let database_temp = secure_tempdir();
        let database = test_publisher(
            &database_temp,
            StaticJwksProvider::new(jwks("fixture-key")),
            Arc::new(TestSigner::new("test-publisher-v1")),
        );
        database.store.lock().await.break_for_test();
        assert!(matches!(
            database.issue(&fixture.request_body, &fixture.token).await,
            Err(PublisherError::Store)
        ));
    }

    #[tokio::test]
    async fn deterministic_hostile_body_corpus_is_panic_free() {
        let temp = secure_tempdir();
        let publisher = test_publisher(
            &temp,
            StaticJwksProvider::new(jwks("fixture-key")),
            Arc::new(TestSigner::new("test-publisher-v1")),
        );
        let token = fixture().token;
        let mut state = 0x9e37_79b9_u32;
        for length in 0..10_000usize {
            let mut body = Vec::with_capacity(length % 1024);
            for _ in 0..length % 1024 {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                body.push((state & 0xff) as u8);
            }
            let _ = publisher.issue(&body, &token).await;
        }
    }

    #[test]
    fn signing_material_is_not_debuggable_or_present_in_errors() {
        let signer = TestSigner::new("test-publisher-v1");
        let public = signer.public_key_pem();
        assert!(public.contains("BEGIN PUBLIC KEY"));
        for error in [
            PublisherError::Config,
            PublisherError::Signing,
            PublisherError::Store,
        ] {
            let text = format!("{error:?}: {error}");
            assert!(!text.contains("PRIVATE KEY"));
            assert!(!text.contains("fixture-jti"));
        }
    }
}
