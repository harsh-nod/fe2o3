use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde_json::{Map, Value, json};

use crate::canonical::canonical_bytes;
use crate::config::{GITHUB_ISSUER, ServiceConfig};
use crate::oidc::PublisherRequest;

pub(crate) fn secure_tempdir() -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().unwrap();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    directory
}

pub(crate) const RSA_PRIVATE_KEY: &[u8] = include_bytes!("../tests/fixtures/github-test-rsa.pem");
pub(crate) const RSA_MODULUS: &str = "uaTz1mW6Z6HS5kDdG-0E4rM9YlVXzEOetxC8TlGfTqN3k8DigVbR_Ix0kirK-vRVxZltPYRRu1gWtweq2HhaNRoF2edQsMCVOWJwY_w8BD75rsH977JEQivPlRyha7hrVq2UpTH5j6A84FjMgzUFDj2y8BlQSSKYW2EAU6aRVRn04-6uKLisdU8gifZuxBgAFV0dLB_PBWbjnAWy3gcUPwF4-LgT0X_IsNw_paz2eE0C_NgY1MDf0IsJSy70BTAQkOrzZLYLEp62Q-YghpLXB36Fa3ry0RfshiHq6XuvZdTr0VORnAoyUP5civX4ECxCAPtiNwGExd77RIjHxXH7zQ";

pub(crate) fn config(database_path: PathBuf) -> ServiceConfig {
    ServiceConfig {
        schema_version: 1,
        listen: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        database_path,
        signing_key_id: "test-publisher-v1".into(),
        signing_key_path: PathBuf::from("/test-only/not-loaded.pem"),
        signature_domain: "test".into(),
        issuer: GITHUB_ISSUER.into(),
        jwks_url: "https://issuer.test/jwks".into(),
        audience: "https://publisher.example.invalid/github-actions".into(),
        repository: "powderluv/fe2o3".into(),
        repository_id: "1233498266".into(),
        repository_owner_id: "74956".into(),
        environment: "protected-publisher".into(),
        default_branch: "main".into(),
        caller_workflow_path: ".github/workflows/parity-promotion.yml".into(),
        protected_workflow_path: ".github/workflows/parity-publisher-gate.yml".into(),
        allowed_actor_ids: vec!["101".into()],
        request_deadline_milliseconds: 2000,
        max_inflight_requests: 16,
        network_deadline_milliseconds: 1000,
        jwks_cache_seconds: 300,
        max_receipts: 4096,
        max_database_bytes: 64 * 1024 * 1024,
        receipt_retention_seconds: 24 * 60 * 60,
        sqlite_busy_timeout_milliseconds: 1000,
    }
}

pub(crate) fn jwks(kid: &str) -> Vec<u8> {
    format!(
        "{{\"keys\":[{{\"alg\":\"RS256\",\"e\":\"AQAB\",\"kid\":\"{kid}\",\"kty\":\"RSA\",\"n\":\"{RSA_MODULUS}\",\"use\":\"sig\",\"x5c\":[],\"x5t\":\"Zml4dHVyZS10aHVtYnByaW50ISE\"}}]}}\n"
    )
    .into_bytes()
}

pub(crate) struct Fixture {
    pub request: PublisherRequest,
    pub request_body: Vec<u8>,
    pub token: String,
}

pub(crate) fn fixture() -> Fixture {
    fixture_with(|_| {})
}

pub(crate) fn fixture_with(mutator: impl FnOnce(&mut Map<String, Value>)) -> Fixture {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let reference = "refs/heads/gh-readonly-queue/main/pr-1";
    let candidate = "4".repeat(40);
    let mut claims = json!({
        "actor_id": "101",
        "aud": "https://publisher.example.invalid/github-actions",
        "base_ref": "",
        "check_run_id": "505",
        "event_name": "merge_group",
        "environment": "protected-publisher",
        "exp": now + 300,
        "head_ref": "",
        "iat": now,
        "iss": GITHUB_ISSUER,
        "job_workflow_ref": format!("powderluv/fe2o3/.github/workflows/parity-publisher-gate.yml@{reference}"),
        "job_workflow_sha": candidate,
        "jti": "fixture-jti-001",
        "nbf": now,
        "ref": reference,
        "repository": "powderluv/fe2o3",
        "repository_id": "1233498266",
        "repository_owner": "powderluv",
        "repository_owner_id": "74956",
        "run_attempt": "1",
        "run_id": "303",
        "run_number": "404",
        "runner_environment": "github-hosted",
        "sha": candidate,
        "sub": "repo:powderluv/fe2o3:environment:protected-publisher",
        "workflow": "Protected parity promotion",
        "workflow_ref": format!("powderluv/fe2o3/.github/workflows/parity-promotion.yml@{reference}"),
        "workflow_sha": candidate,
    });
    mutator(claims.as_object_mut().unwrap());
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("fixture-key".into());
    let token = jsonwebtoken::encode(
        &header,
        &claims,
        &EncodingKey::from_rsa_pem(RSA_PRIVATE_KEY).unwrap(),
    )
    .unwrap();
    let mut authorization = claims.as_object().unwrap().clone();
    authorization.insert("alg".into(), Value::String("RS256".into()));
    authorization.insert("job".into(), Value::String("gate".into()));
    authorization.insert("kid".into(), Value::String("fixture-key".into()));
    authorization.insert(
        "policy_id".into(),
        Value::String("fe2o3-protected-local-merge-group-v3".into()),
    );
    authorization.insert("schema_version".into(), Value::Number(1.into()));

    let workflow = BTreeMap::from([
        ("fe2o3_publisher_default_branch".into(), "main".into()),
        (
            "fe2o3_publisher_github_environment".into(),
            "protected-publisher".into(),
        ),
        ("fe2o3_publisher_repository_owner_id".into(), "74956".into()),
        ("github_actor_id".into(), "101".into()),
        ("github_event_name".into(), "merge_group".into()),
        ("github_job".into(), "gate".into()),
        ("github_ref".into(), reference.into()),
        ("github_repository".into(), "powderluv/fe2o3".into()),
        ("github_repository_id".into(), "1233498266".into()),
        ("github_run_attempt".into(), "1".into()),
        ("github_run_id".into(), "303".into()),
        ("github_run_number".into(), "404".into()),
        ("github_sha".into(), candidate.clone()),
        (
            "github_workflow".into(),
            "Protected parity promotion".into(),
        ),
        (
            "github_workflow_ref".into(),
            format!("powderluv/fe2o3/.github/workflows/parity-promotion.yml@{reference}"),
        ),
        ("github_workflow_sha".into(), candidate.clone()),
    ]);
    let request = PublisherRequest {
        archive_sha256: "a".repeat(64),
        baseline_status_sha256: "b".repeat(64),
        candidate_head: candidate,
        candidate_status_sha256: "c".repeat(64),
        default_tip: "3".repeat(40),
        hardware_lane: "mi300x-gfx942-test".into(),
        logical_destination: "docs/parity-evidence/archive".into(),
        manifest_baseline_commit: "1".repeat(40),
        manifest_path: "promotion.tsv".into(),
        manifest_sha256: "d".repeat(64),
        oidc_authorization: Value::Object(authorization),
        request_domain: "fe2o3-protected-publisher-request-v1".into(),
        schema_version: 1,
        source_commit: "2".repeat(40),
        source_tree: "6".repeat(40),
        target: "gfx942".into(),
        workflow,
    };
    let request_value = serde_json::to_value(&request).unwrap();
    let request_body = canonical_bytes(&request_value).unwrap();
    Fixture {
        request,
        request_body,
        token,
    }
}
