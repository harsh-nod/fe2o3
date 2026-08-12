use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::time::Instant;

use crate::PublisherError;
use crate::bounds::{
    MAX_CLOCK_SKEW_SECS, MAX_JSON_STRING_BYTES, MAX_JWKS_BYTES, MAX_JWKS_KEYS, MAX_JWT_BYTES,
    MAX_JWT_SEGMENT_BYTES, MAX_OIDC_LIFETIME_SECS,
};
use crate::canonical::{CanonicalError, parse_unique};
use crate::config::ServiceConfig;
use crate::jwks::JwksProvider;

const POLICY_ID: &str = "fe2o3-protected-local-merge-group-v3";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PublisherRequest {
    pub archive_sha256: String,
    pub baseline_status_sha256: String,
    pub candidate_head: String,
    pub candidate_status_sha256: String,
    pub default_tip: String,
    pub hardware_lane: String,
    pub logical_destination: String,
    pub manifest_baseline_commit: String,
    pub manifest_path: String,
    pub manifest_sha256: String,
    pub oidc_authorization: Value,
    pub request_domain: String,
    pub schema_version: u32,
    pub source_commit: String,
    pub source_tree: String,
    pub target: String,
    pub workflow: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct Authorization {
    pub replay_identity: String,
    pub issued_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct JwksDocument {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Jwk {
    alg: String,
    e: String,
    kid: String,
    kty: String,
    n: String,
    #[serde(rename = "use")]
    use_: String,
    #[serde(default)]
    x5c: Vec<String>,
    #[serde(default)]
    x5t: Option<String>,
}

pub async fn authenticate(
    config: &ServiceConfig,
    provider: Arc<dyn JwksProvider>,
    request: &PublisherRequest,
    token: &str,
    deadline: Instant,
) -> Result<Authorization, PublisherError> {
    validate_request_shape(request)?;
    if token.is_empty() || token.len() > MAX_JWT_BYTES || !token.is_ascii() {
        return Err(PublisherError::Authentication);
    }
    let segments: Vec<_> = token.split('.').collect();
    if segments.len() != 3
        || segments.iter().any(|segment| {
            segment.is_empty()
                || segment.len() > MAX_JWT_SEGMENT_BYTES
                || !segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        })
    {
        return Err(PublisherError::Authentication);
    }

    let header = decode_segment(segments[0])?;
    let claims = decode_segment(segments[1])?;
    let header_map = header.as_object().ok_or(PublisherError::Authentication)?;
    let allowed_header_sets = [
        BTreeSet::from(["alg", "kid", "typ"]),
        BTreeSet::from(["alg", "kid", "typ", "x5t"]),
    ];
    let actual_header_set = header_map
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if !allowed_header_sets.contains(&actual_header_set)
        || string(&header, "alg")? != "RS256"
        || string(&header, "typ")? != "JWT"
    {
        return Err(PublisherError::Authentication);
    }
    let kid = string(&header, "kid")?;
    if !safe_text(kid) {
        return Err(PublisherError::Authentication);
    }

    let jwks_raw = provider.fetch(deadline).await?;
    let jwks_value = parse_unique(&jwks_raw, MAX_JWKS_BYTES).map_err(|_| PublisherError::Jwks)?;
    let jwks: JwksDocument =
        serde_json::from_value(jwks_value).map_err(|_| PublisherError::Jwks)?;
    if jwks.keys.is_empty() || jwks.keys.len() > MAX_JWKS_KEYS {
        return Err(PublisherError::Jwks);
    }
    let matching: Vec<_> = jwks.keys.iter().filter(|key| key.kid == kid).collect();
    if matching.len() != 1 {
        return Err(PublisherError::Authentication);
    }
    let key = matching[0];
    validate_jwk(key)?;
    if let Some(x5t) = header.get("x5t").and_then(Value::as_str)
        && (key.x5t.as_deref() != Some(x5t) || !valid_thumbprint(x5t))
    {
        return Err(PublisherError::Authentication);
    }
    let decoding_key = DecodingKey::from_rsa_components(&key.n, &key.e)
        .map_err(|_| PublisherError::Authentication)?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.required_spec_claims.clear();
    validation.validate_exp = false;
    validation.validate_nbf = false;
    validation.validate_aud = false;
    let decoded = jsonwebtoken::decode::<Value>(token, &decoding_key, &validation)
        .map_err(|_| PublisherError::Authentication)?;
    if decoded.header.alg != Algorithm::RS256 || decoded.claims != claims {
        return Err(PublisherError::Authentication);
    }

    validate_claims(config, request, &header, &claims)
}

fn decode_segment(segment: &str) -> Result<Value, PublisherError> {
    let padding = "=".repeat((4 - segment.len() % 4) % 4);
    let encoded = format!("{segment}{padding}");
    let raw = base64::engine::general_purpose::URL_SAFE
        .decode(encoded.as_bytes())
        .map_err(|_| PublisherError::Authentication)?;
    if base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&raw) != segment {
        return Err(PublisherError::Authentication);
    }
    parse_unique(&raw, MAX_JWT_SEGMENT_BYTES)
        .map_err(|_: CanonicalError| PublisherError::Authentication)
}

fn validate_jwk(key: &Jwk) -> Result<(), PublisherError> {
    if key.alg != "RS256"
        || key.kty != "RSA"
        || key.use_ != "sig"
        || !safe_text(&key.kid)
        || key.n.is_empty()
        || key.n.len() > 2048
        || key.e.is_empty()
        || key.e.len() > 16
        || key.x5c.len() > 4
        || key.x5c.iter().any(|value| value.len() > 8192)
        || key
            .x5t
            .as_ref()
            .is_some_and(|value| !valid_thumbprint(value))
    {
        return Err(PublisherError::Jwks);
    }
    Ok(())
}

fn valid_thumbprint(value: &str) -> bool {
    if value.len() != 27
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return false;
    }
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .is_ok_and(|decoded| decoded.len() == 20)
}

fn validate_request_shape(request: &PublisherRequest) -> Result<(), PublisherError> {
    let hashes = [
        &request.archive_sha256,
        &request.baseline_status_sha256,
        &request.candidate_status_sha256,
        &request.manifest_sha256,
    ];
    let commits = [
        &request.candidate_head,
        &request.default_tip,
        &request.manifest_baseline_commit,
        &request.source_commit,
        &request.source_tree,
    ];
    let bounded = [
        &request.hardware_lane,
        &request.logical_destination,
        &request.manifest_path,
        &request.target,
    ];
    if request.schema_version != 1
        || request.request_domain != "fe2o3-protected-publisher-request-v1"
        || hashes.iter().any(|value| !hex(value, 64))
        || commits.iter().any(|value| !hex(value, 40))
        || bounded.iter().any(|value| !safe_text(value))
        || !(request.logical_destination == "docs/parity-evidence/archive"
            || request
                .logical_destination
                .starts_with("docs/parity-evidence/archive/"))
    {
        return Err(PublisherError::Request);
    }
    Ok(())
}

fn validate_claims(
    config: &ServiceConfig,
    request: &PublisherRequest,
    header: &Value,
    claims: &Value,
) -> Result<Authorization, PublisherError> {
    let expected_claim_keys = BTreeSet::from([
        "actor_id",
        "aud",
        "base_ref",
        "check_run_id",
        "event_name",
        "environment",
        "exp",
        "head_ref",
        "iat",
        "iss",
        "job_workflow_ref",
        "job_workflow_sha",
        "jti",
        "nbf",
        "ref",
        "repository",
        "repository_id",
        "repository_owner",
        "repository_owner_id",
        "run_attempt",
        "run_id",
        "run_number",
        "runner_environment",
        "sha",
        "sub",
        "workflow",
        "workflow_ref",
        "workflow_sha",
    ]);
    let claim_map = claims.as_object().ok_or(PublisherError::Authentication)?;
    if claim_map
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != expected_claim_keys
    {
        return Err(PublisherError::Authentication);
    }

    let workflow = &request.workflow;
    let expected_workflow_keys = BTreeSet::from([
        "github_repository",
        "github_repository_id",
        "fe2o3_publisher_repository_owner_id",
        "github_run_id",
        "github_run_attempt",
        "github_run_number",
        "github_workflow_ref",
        "github_workflow_sha",
        "github_workflow",
        "github_job",
        "github_event_name",
        "github_ref",
        "github_sha",
        "github_actor_id",
        "fe2o3_publisher_github_environment",
        "fe2o3_publisher_default_branch",
    ]);
    if workflow.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected_workflow_keys {
        return Err(PublisherError::Authentication);
    }
    let field = |name: &str| {
        workflow
            .get(name)
            .map(String::as_str)
            .ok_or(PublisherError::Authentication)
    };
    let reference = string(claims, "ref")?;
    if !reference.starts_with(&config.queue_prefix())
        || field("github_repository")? != config.repository
        || field("github_repository_id")? != config.repository_id
        || field("fe2o3_publisher_repository_owner_id")? != config.repository_owner_id
        || field("fe2o3_publisher_github_environment")? != config.environment
        || field("fe2o3_publisher_default_branch")? != config.default_branch
        || field("github_job")? != "gate"
        || field("github_event_name")? != "merge_group"
        || field("github_ref")? != reference
        || field("github_sha")? != request.candidate_head
        || field("github_workflow_sha")? != request.candidate_head
        || field("github_workflow")? != "Protected parity promotion"
        || !config
            .allowed_actor_ids
            .iter()
            .any(|actor| actor == field("github_actor_id").unwrap_or(""))
    {
        return Err(PublisherError::Authentication);
    }

    let owner = config
        .repository
        .split_once('/')
        .ok_or(PublisherError::Config)?
        .0;
    let caller_ref = format!(
        "{}/{path}@{reference}",
        config.repository,
        path = config.caller_workflow_path
    );
    let job_ref = format!(
        "{}/{path}@{reference}",
        config.repository,
        path = config.protected_workflow_path
    );
    let subject = format!(
        "repo:{}:environment:{}",
        config.repository.replace(':', "%3A"),
        config.environment.replace(':', "%3A")
    );
    let exact = [
        ("actor_id", field("github_actor_id")?),
        ("aud", config.audience.as_str()),
        ("base_ref", ""),
        ("event_name", "merge_group"),
        ("environment", config.environment.as_str()),
        ("head_ref", ""),
        ("iss", config.issuer.as_str()),
        ("job_workflow_ref", job_ref.as_str()),
        ("job_workflow_sha", request.candidate_head.as_str()),
        ("ref", reference),
        ("repository", config.repository.as_str()),
        ("repository_id", config.repository_id.as_str()),
        ("repository_owner", owner),
        ("repository_owner_id", config.repository_owner_id.as_str()),
        ("run_attempt", field("github_run_attempt")?),
        ("run_id", field("github_run_id")?),
        ("run_number", field("github_run_number")?),
        ("runner_environment", "github-hosted"),
        ("sha", request.candidate_head.as_str()),
        ("sub", subject.as_str()),
        ("workflow", "Protected parity promotion"),
        ("workflow_ref", caller_ref.as_str()),
        ("workflow_sha", request.candidate_head.as_str()),
    ];
    if exact
        .iter()
        .any(|(name, expected)| string(claims, name).ok() != Some(*expected))
        || field("github_workflow_ref")? != caller_ref
        || !positive_decimal(string(claims, "check_run_id")?)
    {
        return Err(PublisherError::Authentication);
    }
    for name in ["run_attempt", "run_id", "run_number", "actor_id"] {
        if !positive_decimal(string(claims, name)?) {
            return Err(PublisherError::Authentication);
        }
    }

    let issued_at = integer(claims, "iat")?;
    let not_before = integer(claims, "nbf")?;
    let expires_at = integer(claims, "exp")?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| PublisherError::Authentication)?
        .as_secs() as i64;
    if issued_at <= 0
        || not_before <= 0
        || expires_at <= issued_at
        || not_before > issued_at
        || expires_at - issued_at > MAX_OIDC_LIFETIME_SECS
        || issued_at > now + MAX_CLOCK_SKEW_SECS
        || not_before > now + MAX_CLOCK_SKEW_SECS
        || expires_at < now
    {
        return Err(PublisherError::Authentication);
    }

    let expected_authorization = authorization_value(header, claims, field("github_job")?);
    if request.oidc_authorization != expected_authorization {
        return Err(PublisherError::Authentication);
    }
    let jti = string(claims, "jti")?;
    if !safe_text(jti) {
        return Err(PublisherError::Authentication);
    }
    Ok(Authorization {
        replay_identity: jti.to_owned(),
        issued_at,
    })
}

fn authorization_value(header: &Value, claims: &Value, job: &str) -> Value {
    let mut output = claims.as_object().cloned().unwrap_or_else(Map::new);
    output.insert("alg".into(), Value::String("RS256".into()));
    output.insert("job".into(), Value::String(job.into()));
    output.insert(
        "kid".into(),
        Value::String(string(header, "kid").unwrap_or_default().into()),
    );
    output.insert("policy_id".into(), Value::String(POLICY_ID.into()));
    output.insert("schema_version".into(), Value::Number(1.into()));
    if let Ok(x5t) = string(header, "x5t") {
        output.insert("x5t".into(), Value::String(x5t.into()));
    }
    Value::Object(output)
}

fn string<'a>(value: &'a Value, name: &str) -> Result<&'a str, PublisherError> {
    let value = value
        .get(name)
        .and_then(Value::as_str)
        .ok_or(PublisherError::Authentication)?;
    if value.len() > MAX_JSON_STRING_BYTES
        || !value.is_ascii()
        || value.bytes().any(|byte| !(0x20..=0x7e).contains(&byte))
    {
        return Err(PublisherError::Authentication);
    }
    Ok(value)
}

fn integer(value: &Value, name: &str) -> Result<i64, PublisherError> {
    value
        .get(name)
        .and_then(Value::as_i64)
        .ok_or(PublisherError::Authentication)
}

fn positive_decimal(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.parse::<u64>().is_ok_and(|number| number > 0)
}

fn safe_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_JSON_STRING_BYTES
        && value.is_ascii()
        && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}

fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jwks::StaticJwksProvider;
    use crate::test_support::{config, fixture, fixture_with, jwks};
    use serde_json::json;

    async fn authenticate_fixture(
        fixture: &crate::test_support::Fixture,
        provider: StaticJwksProvider,
    ) -> Result<Authorization, PublisherError> {
        authenticate(
            &config("/tmp/not-opened.db".into()),
            Arc::new(provider),
            &fixture.request,
            &fixture.token,
            Instant::now() + std::time::Duration::from_secs(1),
        )
        .await
    }

    #[tokio::test]
    async fn authenticates_exact_github_profile() {
        let authorization =
            authenticate_fixture(&fixture(), StaticJwksProvider::new(jwks("fixture-key")))
                .await
                .unwrap();
        assert_eq!(authorization.replay_identity, "fixture-jti-001");
    }

    #[tokio::test]
    async fn stale_future_and_invalid_lifetimes_fail_closed() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let cases = [
            (
                "stale",
                json!(now - 1200),
                json!(now - 900),
                json!(now - 1200),
            ),
            (
                "future",
                json!(now + 301),
                json!(now + 600),
                json!(now + 301),
            ),
            (
                "long",
                json!(now),
                json!(now + MAX_OIDC_LIFETIME_SECS + 1),
                json!(now),
            ),
            (
                "nbf-after-iat",
                json!(now),
                json!(now + 200),
                json!(now + 1),
            ),
        ];
        for (_name, iat, exp, nbf) in cases {
            let fixture = fixture_with(|claims| {
                claims.insert("iat".into(), iat);
                claims.insert("exp".into(), exp);
                claims.insert("nbf".into(), nbf);
            });
            assert!(
                authenticate_fixture(&fixture, StaticJwksProvider::new(jwks("fixture-key")))
                    .await
                    .is_err()
            );
        }
    }

    #[tokio::test]
    async fn every_bound_identity_claim_is_reconstructed() {
        let replacements = [
            ("actor_id", json!("999")),
            ("aud", json!("https://attacker.invalid")),
            ("event_name", json!("push")),
            ("environment", json!("unprotected")),
            ("iss", json!("https://attacker.invalid")),
            (
                "job_workflow_ref",
                json!("powderluv/fe2o3/.github/workflows/other.yml@refs/heads/main"),
            ),
            ("ref", json!("refs/heads/main")),
            ("repository", json!("attacker/fe2o3")),
            ("repository_id", json!("1")),
            ("repository_owner_id", json!("1")),
            ("runner_environment", json!("self-hosted")),
            ("sha", json!("9".repeat(40))),
            (
                "sub",
                json!("repo:attacker/fe2o3:environment:protected-publisher"),
            ),
            (
                "workflow_ref",
                json!("powderluv/fe2o3/.github/workflows/other.yml@refs/heads/main"),
            ),
        ];
        for (name, replacement) in replacements {
            let fixture = fixture_with(|claims| {
                claims.insert(name.into(), replacement);
            });
            assert!(
                authenticate_fixture(&fixture, StaticJwksProvider::new(jwks("fixture-key")))
                    .await
                    .is_err(),
                "accepted replaced {name}"
            );
        }
    }

    #[tokio::test]
    async fn unknown_key_rotation_and_outage_are_closed() {
        let fixture = fixture();
        assert!(
            authenticate_fixture(&fixture, StaticJwksProvider::new(jwks("old-key")))
                .await
                .is_err()
        );
        assert!(
            authenticate_fixture(&fixture, StaticJwksProvider::outage())
                .await
                .is_err()
        );
        assert!(
            authenticate_fixture(&fixture, StaticJwksProvider::new(jwks("fixture-key")))
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn malformed_oversized_and_unknown_algorithm_tokens_reject() {
        let fixture = fixture();
        let provider = || StaticJwksProvider::new(jwks("fixture-key"));
        let mut malformed = fixture.token.clone();
        malformed.push('.');
        assert!(
            authenticate(
                &config("/tmp/not-opened.db".into()),
                Arc::new(provider()),
                &fixture.request,
                &malformed,
                Instant::now() + std::time::Duration::from_secs(1)
            )
            .await
            .is_err()
        );
        assert!(
            authenticate(
                &config("/tmp/not-opened.db".into()),
                Arc::new(provider()),
                &fixture.request,
                &"a".repeat(MAX_JWT_BYTES + 1),
                Instant::now() + std::time::Duration::from_secs(1)
            )
            .await
            .is_err()
        );

        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(br#"{"alg":"none","kid":"fixture-key","typ":"JWT"}"#);
        let claims = fixture.token.split('.').nth(1).unwrap();
        let none = format!("{header}.{claims}.x");
        assert!(
            authenticate(
                &config("/tmp/not-opened.db".into()),
                Arc::new(provider()),
                &fixture.request,
                &none,
                Instant::now() + std::time::Duration::from_secs(1)
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn jwks_key_count_and_depth_are_bounded() {
        let fixture = fixture();
        let key = String::from_utf8(jwks("fixture-key")).unwrap();
        let object = key
            .trim()
            .strip_prefix("{\"keys\":[")
            .unwrap()
            .strip_suffix("]}")
            .unwrap();
        let many = format!(
            "{{\"keys\":[{}]}}",
            std::iter::repeat_n(object, MAX_JWKS_KEYS + 1)
                .collect::<Vec<_>>()
                .join(",")
        );
        assert!(
            authenticate_fixture(&fixture, StaticJwksProvider::new(many.into_bytes()))
                .await
                .is_err()
        );
        let deep = format!(
            "{}0{}",
            "[".repeat(crate::bounds::MAX_JSON_DEPTH + 1),
            "]".repeat(crate::bounds::MAX_JSON_DEPTH + 1)
        );
        assert!(
            authenticate_fixture(&fixture, StaticJwksProvider::new(deep.into_bytes()))
                .await
                .is_err()
        );
    }
}
