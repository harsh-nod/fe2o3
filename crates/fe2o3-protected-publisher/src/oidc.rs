use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use tokio::time::Instant;

use crate::PublisherError;
use crate::bounds::{
    MAX_CLOCK_SKEW_SECS, MAX_JSON_STRING_BYTES, MAX_JWKS_BYTES, MAX_JWKS_KEYS, MAX_JWT_BYTES,
    MAX_JWT_SEGMENT_BYTES, MAX_OIDC_LIFETIME_SECS,
};
use crate::canonical::{CanonicalError, canonical_bytes, parse_unique};
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
    pub stable_projection_sha256: String,
    pub issued_at: i64,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct StableAuthorizationProjection<'a> {
    actor_id: &'a str,
    aud: &'a str,
    base_ref: &'a str,
    event_name: &'a str,
    environment: &'a str,
    head_ref: &'a str,
    iss: &'a str,
    job: &'a str,
    job_workflow_ref: &'a str,
    job_workflow_sha: &'a str,
    policy_id: &'static str,
    #[serde(rename = "ref")]
    reference: &'a str,
    repository: &'a str,
    repository_id: &'a str,
    repository_owner: &'a str,
    repository_owner_id: &'a str,
    run_attempt: &'a str,
    run_id: &'a str,
    run_number: &'a str,
    runner_environment: &'a str,
    schema_version: u32,
    sha: &'a str,
    sub: &'a str,
    workflow: &'a str,
    workflow_ref: &'a str,
    workflow_sha: &'a str,
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

#[derive(Debug, Deserialize)]
struct GithubClaims {
    actor_id: String,
    aud: String,
    base_ref: String,
    check_run_id: String,
    event_name: String,
    environment: String,
    exp: i64,
    head_ref: String,
    iat: i64,
    iss: String,
    job_workflow_ref: String,
    job_workflow_sha: String,
    jti: String,
    nbf: i64,
    #[serde(rename = "ref")]
    reference: String,
    repository: String,
    repository_id: String,
    repository_owner: String,
    repository_owner_id: String,
    run_attempt: String,
    run_id: String,
    run_number: String,
    runner_environment: String,
    sha: String,
    sub: String,
    workflow: String,
    workflow_ref: String,
    workflow_sha: String,
    #[serde(default, deserialize_with = "deserialize_optional_claim")]
    actor: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_claim")]
    ref_type: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_claim")]
    repository_visibility: Option<String>,
    #[serde(flatten)]
    provider_metadata: BTreeMap<String, Value>,
}

fn deserialize_optional_claim<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::String(value) => Ok(Some(value)),
        _ => Err(D::Error::custom("optional GitHub claim must be a string")),
    }
}

impl GithubClaims {
    fn validate_shape(&self) -> Result<(), PublisherError> {
        let required_text = [
            &self.actor_id,
            &self.aud,
            &self.base_ref,
            &self.check_run_id,
            &self.event_name,
            &self.environment,
            &self.head_ref,
            &self.iss,
            &self.job_workflow_ref,
            &self.job_workflow_sha,
            &self.jti,
            &self.reference,
            &self.repository,
            &self.repository_id,
            &self.repository_owner,
            &self.repository_owner_id,
            &self.run_attempt,
            &self.run_id,
            &self.run_number,
            &self.runner_environment,
            &self.sha,
            &self.sub,
            &self.workflow,
            &self.workflow_ref,
            &self.workflow_sha,
        ];
        let optional_text = [
            self.actor.as_deref(),
            self.ref_type.as_deref(),
            self.repository_visibility.as_deref(),
        ];
        const RESERVED: [&str; 5] = ["alg", "job", "kid", "policy_id", "schema_version"];
        if required_text.iter().any(|value| !bounded_claim_text(value))
            || optional_text
                .iter()
                .flatten()
                .any(|value| !bounded_claim_text(value))
            || self.provider_metadata.keys().any(|name| {
                name.is_empty()
                    || name.len() > 128
                    || !name
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                    || RESERVED.contains(&name.as_str())
            })
        {
            return Err(PublisherError::Authentication);
        }
        Ok(())
    }
}

pub async fn authenticate(
    config: &ServiceConfig,
    provider: Arc<dyn JwksProvider>,
    request: &PublisherRequest,
    token: &str,
    deadline: Instant,
) -> Result<Authorization, PublisherError> {
    authenticate_at(config, provider, request, token, deadline, unix_now()?).await
}

async fn authenticate_at(
    config: &ServiceConfig,
    provider: Arc<dyn JwksProvider>,
    request: &PublisherRequest,
    token: &str,
    deadline: Instant,
    observed_at: i64,
) -> Result<Authorization, PublisherError> {
    validate_request_shape(request)?;
    let (_header, claims) = verify_token(provider, token, deadline).await?;
    validate_claims(config, request, &claims, observed_at)
}

async fn verify_token(
    provider: Arc<dyn JwksProvider>,
    token: &str,
    deadline: Instant,
) -> Result<(Value, Value), PublisherError> {
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

    let initial = provider.fetch(deadline).await?;
    let mut jwks = parse_jwks(&initial.bytes)?;
    let mut matching = matching_keys(&jwks, kid);
    if matching.is_empty() {
        let refreshed = provider.refresh(deadline, initial.generation).await?;
        jwks = parse_jwks(&refreshed.bytes)?;
        matching = matching_keys(&jwks, kid);
    }
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

    Ok((header, claims))
}

fn parse_jwks(raw: &[u8]) -> Result<JwksDocument, PublisherError> {
    let value = parse_unique(raw, MAX_JWKS_BYTES).map_err(|_| PublisherError::Jwks)?;
    let document: JwksDocument = serde_json::from_value(value).map_err(|_| PublisherError::Jwks)?;
    if document.keys.is_empty() || document.keys.len() > MAX_JWKS_KEYS {
        return Err(PublisherError::Jwks);
    }
    Ok(document)
}

fn matching_keys<'a>(document: &'a JwksDocument, kid: &str) -> Vec<&'a Jwk> {
    document.keys.iter().filter(|key| key.kid == kid).collect()
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
    claims: &Value,
    observed_at: i64,
) -> Result<Authorization, PublisherError> {
    let projected: GithubClaims =
        serde_json::from_value(claims.clone()).map_err(|_| PublisherError::Authentication)?;
    projected.validate_shape()?;

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
    let reference = projected.reference.as_str();
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
        (projected.actor_id.as_str(), field("github_actor_id")?),
        (projected.aud.as_str(), config.audience.as_str()),
        (projected.base_ref.as_str(), ""),
        (projected.event_name.as_str(), "merge_group"),
        (projected.environment.as_str(), config.environment.as_str()),
        (projected.head_ref.as_str(), ""),
        (projected.iss.as_str(), config.issuer.as_str()),
        (projected.job_workflow_ref.as_str(), job_ref.as_str()),
        (
            projected.job_workflow_sha.as_str(),
            request.candidate_head.as_str(),
        ),
        (projected.reference.as_str(), reference),
        (projected.repository.as_str(), config.repository.as_str()),
        (
            projected.repository_id.as_str(),
            config.repository_id.as_str(),
        ),
        (projected.repository_owner.as_str(), owner),
        (
            projected.repository_owner_id.as_str(),
            config.repository_owner_id.as_str(),
        ),
        (projected.run_attempt.as_str(), field("github_run_attempt")?),
        (projected.run_id.as_str(), field("github_run_id")?),
        (projected.run_number.as_str(), field("github_run_number")?),
        (projected.runner_environment.as_str(), "github-hosted"),
        (projected.sha.as_str(), request.candidate_head.as_str()),
        (projected.sub.as_str(), subject.as_str()),
        (projected.workflow.as_str(), "Protected parity promotion"),
        (projected.workflow_ref.as_str(), caller_ref.as_str()),
        (
            projected.workflow_sha.as_str(),
            request.candidate_head.as_str(),
        ),
    ];
    if exact.iter().any(|(actual, expected)| actual != expected)
        || field("github_workflow_ref")? != caller_ref
        || !positive_decimal(&projected.check_run_id)
    {
        return Err(PublisherError::Authentication);
    }
    for value in [
        &projected.run_attempt,
        &projected.run_id,
        &projected.run_number,
        &projected.actor_id,
    ] {
        if !positive_decimal(value) {
            return Err(PublisherError::Authentication);
        }
    }

    let issued_at = projected.iat;
    let expires_at = projected.exp;
    validate_times(
        issued_at,
        projected.nbf,
        expires_at,
        config.minimum_token_remaining_seconds(),
        observed_at,
    )?;

    let expected_authorization = stable_authorization_value(&projected, field("github_job")?)?;
    if request.oidc_authorization != expected_authorization {
        return Err(PublisherError::Authentication);
    }
    if !safe_text(&projected.jti) {
        return Err(PublisherError::Authentication);
    }
    let projection =
        canonical_bytes(&expected_authorization).map_err(|_| PublisherError::Authentication)?;
    Ok(Authorization {
        stable_projection_sha256: sha256(&projection),
        issued_at,
    })
}

fn stable_authorization_value(claims: &GithubClaims, job: &str) -> Result<Value, PublisherError> {
    serde_json::to_value(StableAuthorizationProjection {
        actor_id: &claims.actor_id,
        aud: &claims.aud,
        base_ref: &claims.base_ref,
        event_name: &claims.event_name,
        environment: &claims.environment,
        head_ref: &claims.head_ref,
        iss: &claims.iss,
        job,
        job_workflow_ref: &claims.job_workflow_ref,
        job_workflow_sha: &claims.job_workflow_sha,
        policy_id: POLICY_ID,
        reference: &claims.reference,
        repository: &claims.repository,
        repository_id: &claims.repository_id,
        repository_owner: &claims.repository_owner,
        repository_owner_id: &claims.repository_owner_id,
        run_attempt: &claims.run_attempt,
        run_id: &claims.run_id,
        run_number: &claims.run_number,
        runner_environment: &claims.runner_environment,
        schema_version: 1,
        sha: &claims.sha,
        sub: &claims.sub,
        workflow: &claims.workflow,
        workflow_ref: &claims.workflow_ref,
        workflow_sha: &claims.workflow_sha,
    })
    .map_err(|_| PublisherError::Authentication)
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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

fn positive_decimal(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.parse::<u64>().is_ok_and(|number| number > 0)
}

fn bounded_claim_text(value: &str) -> bool {
    value.len() <= MAX_JSON_STRING_BYTES
        && value.is_ascii()
        && value.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
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

pub(crate) async fn validate_enrollment_token(
    config: &ServiceConfig,
    provider: Arc<dyn JwksProvider>,
    token: &str,
    deadline: Instant,
) -> Result<BTreeMap<String, String>, PublisherError> {
    let (_header, claims) = verify_token(provider, token, deadline).await?;
    let projected: GithubClaims =
        serde_json::from_value(claims).map_err(|_| PublisherError::Authentication)?;
    projected.validate_shape()?;
    validate_times(
        projected.iat,
        projected.nbf,
        projected.exp,
        config.minimum_token_remaining_seconds(),
        unix_now()?,
    )?;
    let values = BTreeMap::from([
        ("actor_id".into(), projected.actor_id),
        ("aud".into(), projected.aud),
        ("base_ref".into(), projected.base_ref),
        ("check_run_id".into(), projected.check_run_id),
        ("event_name".into(), projected.event_name),
        ("environment".into(), projected.environment),
        ("exp".into(), projected.exp.to_string()),
        ("head_ref".into(), projected.head_ref),
        ("iat".into(), projected.iat.to_string()),
        ("iss".into(), projected.iss),
        ("job_workflow_ref".into(), projected.job_workflow_ref),
        ("job_workflow_sha".into(), projected.job_workflow_sha),
        ("jti".into(), projected.jti),
        ("nbf".into(), projected.nbf.to_string()),
        ("ref".into(), projected.reference),
        ("repository".into(), projected.repository),
        ("repository_id".into(), projected.repository_id),
        ("repository_owner".into(), projected.repository_owner),
        ("repository_owner_id".into(), projected.repository_owner_id),
        ("run_attempt".into(), projected.run_attempt),
        ("run_id".into(), projected.run_id),
        ("run_number".into(), projected.run_number),
        ("runner_environment".into(), projected.runner_environment),
        ("sha".into(), projected.sha),
        ("sub".into(), projected.sub),
        ("workflow".into(), projected.workflow),
        ("workflow_ref".into(), projected.workflow_ref),
        ("workflow_sha".into(), projected.workflow_sha),
    ]);
    crate::enrollment::validate_claim_profile(config, &values)?;
    Ok(values)
}

fn validate_times(
    issued_at: i64,
    not_before: i64,
    expires_at: i64,
    minimum_remaining_seconds: i64,
    now: i64,
) -> Result<(), PublisherError> {
    if issued_at <= 0
        || not_before <= 0
        || expires_at <= issued_at
        || not_before > issued_at
        || expires_at - issued_at > MAX_OIDC_LIFETIME_SECS
        || issued_at > now + MAX_CLOCK_SKEW_SECS
        || not_before > now + MAX_CLOCK_SKEW_SECS
        || expires_at < now
        || expires_at - now < minimum_remaining_seconds
    {
        return Err(PublisherError::Authentication);
    }
    Ok(())
}

fn unix_now() -> Result<i64, PublisherError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| PublisherError::Authentication)
        .map(|duration| duration.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::jwks::{JwksSnapshot, StaticJwksProvider};
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
        assert_eq!(authorization.stable_projection_sha256.len(), 64);
    }

    #[tokio::test]
    async fn expired_original_and_fresh_jti_share_only_stable_authorization() {
        let now = unix_now().unwrap();
        let original = fixture_with(|claims| {
            claims.insert("iat".into(), json!(now));
            claims.insert("nbf".into(), json!(now));
            claims.insert("exp".into(), json!(now + 300));
            claims.insert("jti".into(), json!("original-jti"));
        });
        let fresh = fixture_with(|claims| {
            claims.insert("iat".into(), json!(now + 301));
            claims.insert("nbf".into(), json!(now + 301));
            claims.insert("exp".into(), json!(now + 601));
            claims.insert("jti".into(), json!("fresh-jti"));
        });
        assert_ne!(original.token, fresh.token);
        assert_eq!(original.request_body, fresh.request_body);
        let provider = || Arc::new(StaticJwksProvider::new(jwks("fixture-key")));
        let initial = authenticate_at(
            &config("/tmp/not-opened.ledger".into()),
            provider(),
            &original.request,
            &original.token,
            Instant::now() + std::time::Duration::from_secs(1),
            now,
        )
        .await
        .unwrap();
        assert!(
            authenticate_at(
                &config("/tmp/not-opened.ledger".into()),
                provider(),
                &original.request,
                &original.token,
                Instant::now() + std::time::Duration::from_secs(1),
                now + 301,
            )
            .await
            .is_err()
        );
        let recovered = authenticate_at(
            &config("/tmp/not-opened.ledger".into()),
            provider(),
            &fresh.request,
            &fresh.token,
            Instant::now() + std::time::Duration::from_secs(1),
            now + 301,
        )
        .await
        .unwrap();
        assert_eq!(
            initial.stable_projection_sha256,
            recovered.stable_projection_sha256
        );
    }

    #[tokio::test]
    async fn token_must_outlive_request_deadline_and_recovery_grace() {
        let config = config("/tmp/not-opened.ledger".into());
        let now = unix_now().unwrap();
        let remaining = config.minimum_token_remaining_seconds() - 1;
        let fixture = fixture_with(|claims| {
            claims.insert("iat".into(), json!(now));
            claims.insert("nbf".into(), json!(now));
            claims.insert("exp".into(), json!(now + remaining));
        });
        assert!(
            authenticate_at(
                &config,
                Arc::new(StaticJwksProvider::new(jwks("fixture-key"))),
                &fixture.request,
                &fixture.token,
                Instant::now() + std::time::Duration::from_secs(1),
                now,
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn documented_and_future_provider_metadata_is_tolerated_but_not_authoritative() {
        let fixture = fixture_with(|claims| {
            claims.insert("actor".into(), json!("powderluv"));
            claims.insert("ref_type".into(), json!("branch"));
            claims.insert("repository_visibility".into(), json!("public"));
            claims.insert("enterprise_id".into(), json!("909"));
            claims.insert("provider_feature".into(), json!({"version": 2}));
        });
        assert!(
            authenticate_fixture(&fixture, StaticJwksProvider::new(jwks("fixture-key")))
                .await
                .is_ok()
        );

        for (name, replacement) in [
            ("actor", json!(7)),
            ("ref_type", json!(["branch"])),
            ("repository_visibility", Value::Null),
            ("policy_id", json!("attacker-policy")),
        ] {
            let fixture = fixture_with(|claims| {
                claims.insert(name.into(), replacement);
            });
            assert!(
                authenticate_fixture(&fixture, StaticJwksProvider::new(jwks("fixture-key")))
                    .await
                    .is_err(),
                "accepted invalid provider claim {name}"
            );
        }
    }

    #[tokio::test]
    async fn malformed_time_claim_types_and_duplicate_json_reject() {
        for (name, replacement) in [
            ("exp", json!("1800000000")),
            ("iat", json!(1.5)),
            ("nbf", json!(u64::MAX)),
            ("exp", Value::Null),
            ("iat", json!(true)),
        ] {
            let fixture = fixture_with(|claims| {
                claims.insert(name.into(), replacement);
            });
            assert!(
                authenticate_fixture(&fixture, StaticJwksProvider::new(jwks("fixture-key")))
                    .await
                    .is_err(),
                "accepted malformed {name}"
            );
        }

        let duplicate =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"exp":1,"exp":2}"#);
        assert!(decode_segment(&duplicate).is_err());
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
                json!(now + MAX_CLOCK_SKEW_SECS + 10),
                json!(now + MAX_CLOCK_SKEW_SECS + 100),
                json!(now + MAX_CLOCK_SKEW_SECS + 10),
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
        for (name, iat, exp, nbf) in cases {
            let fixture = fixture_with(|claims| {
                claims.insert("iat".into(), iat);
                claims.insert("exp".into(), exp);
                claims.insert("nbf".into(), nbf);
            });
            assert!(
                authenticate_fixture(&fixture, StaticJwksProvider::new(jwks("fixture-key")))
                    .await
                    .is_err(),
                "accepted invalid lifetime case {name}"
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

    struct RotatingProvider {
        refreshes: AtomicUsize,
    }

    impl JwksProvider for RotatingProvider {
        fn fetch<'a>(
            &'a self,
            _deadline: Instant,
        ) -> Pin<Box<dyn Future<Output = Result<JwksSnapshot, PublisherError>> + Send + 'a>>
        {
            Box::pin(async {
                Ok(JwksSnapshot {
                    bytes: jwks("old-key"),
                    generation: 1,
                })
            })
        }

        fn refresh<'a>(
            &'a self,
            _deadline: Instant,
            observed_generation: u64,
        ) -> Pin<Box<dyn Future<Output = Result<JwksSnapshot, PublisherError>> + Send + 'a>>
        {
            Box::pin(async move {
                assert_eq!(observed_generation, 1);
                self.refreshes.fetch_add(1, Ordering::SeqCst);
                Ok(JwksSnapshot {
                    bytes: jwks("fixture-key"),
                    generation: 2,
                })
            })
        }
    }

    #[tokio::test]
    async fn unknown_kid_retries_once_against_rotated_document() {
        let fixture = fixture();
        let provider = Arc::new(RotatingProvider {
            refreshes: AtomicUsize::new(0),
        });
        let authorization = authenticate(
            &config("/tmp/not-opened.db".into()),
            provider.clone(),
            &fixture.request,
            &fixture.token,
            Instant::now() + std::time::Duration::from_secs(1),
        )
        .await
        .unwrap();
        assert_eq!(authorization.stable_projection_sha256.len(), 64);
        assert_eq!(provider.refreshes.load(Ordering::SeqCst), 1);
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
