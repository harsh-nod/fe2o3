use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::PublisherError;
use crate::bounds::{ENROLLMENT_VALIDITY_SECS, MAX_ENROLLMENT_ARTIFACT_BYTES};
use crate::canonical::{canonical_bytes, parse_canonical};
use crate::config::ServiceConfig;
use crate::jwks::{HttpsJwksProvider, JwksProvider};
use crate::secure_fs::{read_owner_only, write_new_owner_only};

pub const ENROLLMENT_REQUIRED_CLAIMS: [&str; 28] = [
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
];

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EnrollmentArtifact {
    artifact_domain: String,
    claim_profile_sha256: String,
    config_sha256: String,
    enrolled_at_unix: i64,
    expires_at_unix: i64,
    observed_claims: BTreeMap<String, String>,
    schema_version: u32,
    token_sha256: String,
}

impl EnrollmentArtifact {
    fn validate(&self, config: &ServiceConfig, now: i64) -> Result<(), PublisherError> {
        if self.schema_version != 1
            || self.artifact_domain != "fe2o3-protected-publisher-enrollment-v1"
            || self.config_sha256 != config.config_sha256()?
            || self.enrolled_at_unix <= 0
            || self.expires_at_unix <= self.enrolled_at_unix
            || self.expires_at_unix - self.enrolled_at_unix != ENROLLMENT_VALIDITY_SECS
            || now < self.enrolled_at_unix
            || now >= self.expires_at_unix
            || !is_hex(&self.token_sha256)
            || !is_hex(&self.claim_profile_sha256)
            || self.observed_claims.len() != ENROLLMENT_REQUIRED_CLAIMS.len()
            || ENROLLMENT_REQUIRED_CLAIMS
                .iter()
                .any(|claim| !self.observed_claims.contains_key(*claim))
            || self.observed_claims.values().any(|value| {
                value.len() > 4096
                    || !value.is_ascii()
                    || value.bytes().any(|byte| !(0x20..=0x7e).contains(&byte))
            })
        {
            return Err(PublisherError::Config);
        }
        let claims = canonical_bytes(
            &serde_json::to_value(&self.observed_claims).map_err(|_| PublisherError::Config)?,
        )
        .map_err(|_| PublisherError::Config)?;
        if sha256(&claims) != self.claim_profile_sha256 {
            return Err(PublisherError::Config);
        }
        validate_claim_profile(config, &self.observed_claims)
    }
}

pub(crate) fn require_enrollment(config: &ServiceConfig) -> Result<(), PublisherError> {
    let bytes = read_owner_only(
        &config.enrollment_artifact_path,
        MAX_ENROLLMENT_ARTIFACT_BYTES,
    )?;
    let value = parse_canonical(&bytes, MAX_ENROLLMENT_ARTIFACT_BYTES)
        .map_err(|_| PublisherError::Config)?;
    let artifact: EnrollmentArtifact =
        serde_json::from_value(value).map_err(|_| PublisherError::Config)?;
    artifact.validate(config, now())
}

pub async fn enroll_token(
    config: &ServiceConfig,
    token_path: &Path,
    artifact_path: &Path,
) -> Result<String, PublisherError> {
    config.validate()?;
    let provider = Arc::new(HttpsJwksProvider::new(
        &config.jwks_url,
        config.network_deadline(),
        config.jwks_cache_ttl(),
    )?);
    enroll_token_with_provider(config, provider, token_path, artifact_path).await
}

async fn enroll_token_with_provider(
    config: &ServiceConfig,
    provider: Arc<dyn JwksProvider>,
    token_path: &Path,
    artifact_path: &Path,
) -> Result<String, PublisherError> {
    if artifact_path != config.enrollment_artifact_path {
        return Err(PublisherError::Config);
    }
    let token = read_owner_only(token_path, MAX_ENROLLMENT_ARTIFACT_BYTES)?;
    let token = std::str::from_utf8(&token)
        .map_err(|_| PublisherError::Authentication)?
        .trim_end_matches(['\r', '\n']);
    let claims = crate::oidc::validate_enrollment_token(
        config,
        provider,
        token,
        tokio::time::Instant::now() + config.network_deadline(),
    )
    .await?;
    let claims_bytes =
        canonical_bytes(&serde_json::to_value(&claims).map_err(|_| PublisherError::Config)?)
            .map_err(|_| PublisherError::Config)?;
    let enrolled_at = now();
    let artifact = EnrollmentArtifact {
        artifact_domain: "fe2o3-protected-publisher-enrollment-v1".into(),
        claim_profile_sha256: sha256(&claims_bytes),
        config_sha256: config.config_sha256()?,
        enrolled_at_unix: enrolled_at,
        expires_at_unix: enrolled_at
            .checked_add(ENROLLMENT_VALIDITY_SECS)
            .ok_or(PublisherError::Config)?,
        observed_claims: claims,
        schema_version: 1,
        token_sha256: sha256(token.as_bytes()),
    };
    artifact.validate(config, enrolled_at)?;
    let bytes =
        canonical_bytes(&serde_json::to_value(&artifact).map_err(|_| PublisherError::Config)?)
            .map_err(|_| PublisherError::Config)?;
    write_new_owner_only(artifact_path, &bytes, MAX_ENROLLMENT_ARTIFACT_BYTES)?;
    Ok(artifact.claim_profile_sha256)
}

pub(crate) fn validate_claim_profile(
    config: &ServiceConfig,
    claims: &BTreeMap<String, String>,
) -> Result<(), PublisherError> {
    let get = |name| claims.get(name).map(String::as_str).unwrap_or_default();
    let queue = config.queue_prefix();
    if get("iss") != config.issuer
        || get("aud") != config.audience
        || get("repository") != config.repository
        || get("repository_id") != config.repository_id
        || get("repository_owner_id") != config.repository_owner_id
        || get("environment") != config.environment
        || get("event_name") != "merge_group"
        || !get("ref").starts_with(&queue)
        || !get("base_ref").is_empty()
        || !get("head_ref").is_empty()
        || get("runner_environment") != "github-hosted"
        || get("workflow") != "Protected parity promotion"
        || get("workflow_sha") != get("sha")
        || get("job_workflow_sha") != get("sha")
        || !get("workflow_ref").contains(&format!("/{path}@", path = config.caller_workflow_path))
        || !get("job_workflow_ref")
            .contains(&format!("/{path}@", path = config.protected_workflow_path))
        || !config
            .allowed_actor_ids
            .iter()
            .any(|actor| actor == get("actor_id"))
    {
        return Err(PublisherError::Authentication);
    }
    Ok(())
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs() as i64)
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn is_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;
    use crate::config::GITHUB_ISSUER;
    use crate::jwks::StaticJwksProvider;
    use crate::test_support::{config, fixture, jwks, secure_tempdir};

    fn production_config(root: &Path) -> ServiceConfig {
        let mut config = config(root.join("publisher.db"));
        config.enrollment_artifact_path = root.join("enrollment.json");
        config.signing_key_path = root.join("publisher.pem");
        config.signature_domain = "production".into();
        config.jwks_url = format!("{GITHUB_ISSUER}/.well-known/jwks");
        config
    }

    #[tokio::test]
    async fn enrollment_binds_verified_profile_without_storing_token() {
        let temp = secure_tempdir();
        let config = production_config(temp.path());
        let fixture = fixture();
        let token_path = temp.path().join("token.jwt");
        std::fs::write(&token_path, &fixture.token).unwrap();
        std::fs::set_permissions(&token_path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let provider = Arc::new(StaticJwksProvider::new(jwks("fixture-key")));
        let digest = enroll_token_with_provider(
            &config,
            provider,
            &token_path,
            &config.enrollment_artifact_path,
        )
        .await
        .unwrap();
        assert_eq!(digest.len(), 64);
        require_enrollment(&config).unwrap();
        let artifact = std::fs::read_to_string(&config.enrollment_artifact_path).unwrap();
        assert!(!artifact.contains(&fixture.token));
        assert!(!artifact.contains("fixture-jti-001."));

        let mut changed = config.clone();
        changed.allowed_actor_ids = vec!["202".into()];
        assert!(require_enrollment(&changed).is_err());
        assert!(
            enroll_token_with_provider(
                &config,
                Arc::new(StaticJwksProvider::new(jwks("fixture-key"))),
                &token_path,
                &config.enrollment_artifact_path,
            )
            .await
            .is_err()
        );
    }
}
