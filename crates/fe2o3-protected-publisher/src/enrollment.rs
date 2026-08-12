use std::fs::File;
use std::io::Read;
use std::os::fd::{FromRawFd, RawFd};
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::PublisherError;
use crate::bounds::{ENROLLMENT_VALIDITY_SECS, MAX_ENROLLMENT_ARTIFACT_BYTES, MAX_JWT_BYTES};
use crate::canonical::{canonical_bytes, parse_canonical};
use crate::config::ServiceConfig;
use crate::jwks::{HttpsJwksProvider, JwksProvider};
use crate::oidc::{
    EnrollmentProjection, validate_enrollment_projection, validate_enrollment_token,
};
use crate::secure_fs::{read_owner_only, write_new_owner_only};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EnrollmentArtifact {
    artifact_domain: String,
    claim_profile_sha256: String,
    config_sha256: String,
    enrolled_at_unix: i64,
    expires_at_unix: i64,
    projection: EnrollmentProjection,
    schema_version: u32,
    token_sha256: String,
}

impl EnrollmentArtifact {
    fn validate(&self, config: &ServiceConfig, now: i64) -> Result<(), PublisherError> {
        if self.schema_version != 2
            || self.artifact_domain != "fe2o3-protected-publisher-enrollment-v2"
            || self.config_sha256 != config.config_sha256()?
            || self.enrolled_at_unix <= 0
            || self.expires_at_unix <= self.enrolled_at_unix
            || self.expires_at_unix - self.enrolled_at_unix != ENROLLMENT_VALIDITY_SECS
            || now < self.enrolled_at_unix
            || now >= self.expires_at_unix
            || !is_hex(&self.token_sha256)
            || !is_hex(&self.claim_profile_sha256)
        {
            return Err(PublisherError::Config);
        }
        let projection = canonical_bytes(
            &serde_json::to_value(&self.projection).map_err(|_| PublisherError::Config)?,
        )
        .map_err(|_| PublisherError::Config)?;
        if sha256(&projection) != self.claim_profile_sha256 {
            return Err(PublisherError::Config);
        }
        validate_enrollment_projection(config, &self.projection, self.enrolled_at_unix)
            .map_err(|_| PublisherError::Config)
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
    token_fd: RawFd,
    artifact_path: &Path,
) -> Result<String, PublisherError> {
    config.validate()?;
    let provider = Arc::new(HttpsJwksProvider::new(
        &config.jwks_url,
        config.network_deadline(),
        config.jwks_cache_ttl(),
    )?);
    enroll_token_with_provider(config, provider, token_fd, artifact_path).await
}

async fn enroll_token_with_provider(
    config: &ServiceConfig,
    provider: Arc<dyn JwksProvider>,
    token_fd: RawFd,
    artifact_path: &Path,
) -> Result<String, PublisherError> {
    if artifact_path != config.enrollment_artifact_path {
        return Err(PublisherError::Config);
    }
    let token_bytes = read_nonregular_token_fd(token_fd)?;
    let token = std::str::from_utf8(&token_bytes)
        .map_err(|_| PublisherError::Authentication)?
        .trim_end_matches(['\r', '\n']);
    let projection = validate_enrollment_token(
        config,
        provider,
        token,
        tokio::time::Instant::now() + config.network_deadline(),
    )
    .await?;
    let projection_bytes =
        canonical_bytes(&serde_json::to_value(&projection).map_err(|_| PublisherError::Config)?)
            .map_err(|_| PublisherError::Config)?;
    let enrolled_at = now();
    let artifact = EnrollmentArtifact {
        artifact_domain: "fe2o3-protected-publisher-enrollment-v2".into(),
        claim_profile_sha256: sha256(&projection_bytes),
        config_sha256: config.config_sha256()?,
        enrolled_at_unix: enrolled_at,
        expires_at_unix: enrolled_at
            .checked_add(ENROLLMENT_VALIDITY_SECS)
            .ok_or(PublisherError::Config)?,
        projection,
        schema_version: 2,
        token_sha256: sha256(token.as_bytes()),
    };
    artifact.validate(config, enrolled_at)?;
    let bytes =
        canonical_bytes(&serde_json::to_value(&artifact).map_err(|_| PublisherError::Config)?)
            .map_err(|_| PublisherError::Config)?;
    write_new_owner_only(artifact_path, &bytes, MAX_ENROLLMENT_ARTIFACT_BYTES)?;
    Ok(artifact.claim_profile_sha256)
}

fn read_nonregular_token_fd(fd: RawFd) -> Result<Zeroizing<Vec<u8>>, PublisherError> {
    let before = fd_identity(fd)?;
    let kind = before.st_mode & libc::S_IFMT;
    if kind == libc::S_IFREG || !(kind == libc::S_IFIFO || kind == libc::S_IFSOCK) {
        return Err(PublisherError::Config);
    }
    let duplicated = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
    if duplicated < 0 {
        return Err(PublisherError::Config);
    }
    let opened = match fd_identity(duplicated) {
        Ok(identity) => identity,
        Err(error) => {
            unsafe {
                libc::close(duplicated);
            }
            return Err(error);
        }
    };
    if before.st_dev != opened.st_dev
        || before.st_ino != opened.st_ino
        || before.st_mode != opened.st_mode
        || before.st_uid != opened.st_uid
        || before.st_gid != opened.st_gid
    {
        unsafe {
            libc::close(duplicated);
        }
        return Err(PublisherError::Config);
    }
    let mut file = unsafe { File::from_raw_fd(duplicated) };
    let mut bytes = Zeroizing::new(Vec::new());
    Read::by_ref(&mut file)
        .take(MAX_JWT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| PublisherError::Authentication)?;
    if bytes.is_empty() || bytes.len() > MAX_JWT_BYTES {
        return Err(PublisherError::Authentication);
    }
    Ok(bytes)
}

fn fd_identity(fd: RawFd) -> Result<libc::stat, PublisherError> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if fd < 0 || unsafe { libc::fstat(fd, stat.as_mut_ptr()) } != 0 {
        return Err(PublisherError::Config);
    }
    Ok(unsafe { stat.assume_init() })
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
    use std::io::Write;
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixStream;

    use super::*;
    use crate::config::GITHUB_ISSUER;
    use crate::jwks::StaticJwksProvider;
    use crate::test_support::{config, fixture, fixture_with, jwks, secure_tempdir};

    fn production_config(root: &Path) -> ServiceConfig {
        let mut config = config(root.join("publisher.ledger"));
        config.enrollment_artifact_path = root.join("enrollment.json");
        config.signing_key_path = root.join("publisher.pem");
        config.signature_domain = "production".into();
        config.jwks_url = format!("{GITHUB_ISSUER}/.well-known/jwks");
        config
    }

    #[tokio::test]
    async fn enrollment_binds_exact_runtime_projection_without_raw_token() {
        let temp = secure_tempdir();
        let config = production_config(temp.path());
        let fixture = fixture();
        let (mut writer, reader) = UnixStream::pair().unwrap();
        writer.write_all(fixture.token.as_bytes()).unwrap();
        writer.shutdown(std::net::Shutdown::Write).unwrap();
        let digest = enroll_token_with_provider(
            &config,
            Arc::new(StaticJwksProvider::new(jwks("fixture-key"))),
            reader.as_raw_fd(),
            &config.enrollment_artifact_path,
        )
        .await
        .unwrap();
        assert_eq!(digest.len(), 64);
        require_enrollment(&config).unwrap();
        let artifact = std::fs::read_to_string(&config.enrollment_artifact_path).unwrap();
        assert!(!artifact.contains(&fixture.token));
        assert!(!artifact.contains("token-file"));
        assert!(artifact.contains("\"ephemeral\""));
        assert!(artifact.contains("\"stable\""));

        let mut changed = config.clone();
        changed.allowed_actor_ids = vec!["202".into()];
        assert!(require_enrollment(&changed).is_err());
    }

    #[tokio::test]
    async fn regular_file_token_descriptor_is_rejected() {
        let temp = secure_tempdir();
        let config = production_config(temp.path());
        let fixture = fixture();
        let path = temp.path().join("forbidden-token.jwt");
        std::fs::write(&path, fixture.token.as_bytes()).unwrap();
        let file = File::open(path).unwrap();
        assert!(
            enroll_token_with_provider(
                &config,
                Arc::new(StaticJwksProvider::new(jwks("fixture-key"))),
                std::os::fd::AsRawFd::as_raw_fd(&file),
                &config.enrollment_artifact_path,
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn enrollment_rejects_workflow_substrings_and_ephemeral_substitution() {
        let temp = secure_tempdir();
        let config = production_config(temp.path());
        let cases = [
            fixture_with(|claims| {
                let original = claims["workflow_ref"].as_str().unwrap();
                claims.insert(
                    "workflow_ref".into(),
                    format!("attacker-prefix/{original}").into(),
                );
            }),
            fixture_with(|claims| {
                let original = claims["job_workflow_ref"].as_str().unwrap();
                claims.insert(
                    "job_workflow_ref".into(),
                    format!("attacker-prefix/{original}").into(),
                );
            }),
            fixture_with(|claims| {
                claims.insert("check_run_id".into(), "not-a-number".into());
            }),
            fixture_with(|claims| {
                claims.insert("jti".into(), "contains whitespace".into());
            }),
        ];
        for fixture in cases {
            let (mut writer, reader) = UnixStream::pair().unwrap();
            writer.write_all(fixture.token.as_bytes()).unwrap();
            writer.shutdown(std::net::Shutdown::Write).unwrap();
            assert!(
                enroll_token_with_provider(
                    &config,
                    Arc::new(StaticJwksProvider::new(jwks("fixture-key"))),
                    reader.as_raw_fd(),
                    &config.enrollment_artifact_path,
                )
                .await
                .is_err()
            );
        }
    }

    #[test]
    fn shipped_cli_has_no_token_path_or_token_value_option() {
        let main = include_str!("main.rs");
        assert!(!main.contains("--token-file"));
        assert!(!main.contains("--token="));
        assert!(main.contains("--token-fd"));
    }
}
