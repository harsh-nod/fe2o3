use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::PublisherError;
use crate::bounds::{
    MAX_CONFIG_BYTES, MAX_INFLIGHT_REQUESTS, MAX_JSON_STRING_BYTES, MAX_LEDGER_BYTES,
    MAX_STORE_RECEIPTS, MIN_LEDGER_BYTES,
};
use crate::canonical::{canonical_bytes, parse_canonical};
use crate::secure_fs::read_owner_only;

pub const GITHUB_ISSUER: &str = "https://token.actions.githubusercontent.com";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceConfig {
    pub schema_version: u32,
    pub listen: SocketAddr,
    pub ledger_path: PathBuf,
    pub enrollment_artifact_path: PathBuf,
    pub signing_key_id: String,
    pub signing_key_path: PathBuf,
    pub signature_domain: String,
    pub issuer: String,
    pub jwks_url: String,
    pub audience: String,
    pub repository: String,
    pub repository_id: String,
    pub repository_owner_id: String,
    pub environment: String,
    pub default_branch: String,
    pub caller_workflow_path: String,
    pub protected_workflow_path: String,
    pub allowed_actor_ids: Vec<String>,
    pub request_deadline_milliseconds: u64,
    pub max_inflight_requests: u32,
    pub network_deadline_milliseconds: u64,
    pub jwks_cache_seconds: u64,
    pub max_receipts: u64,
    pub max_ledger_bytes: u64,
}

impl ServiceConfig {
    pub fn load(path: &Path) -> Result<Self, PublisherError> {
        let raw = read_owner_only(path, MAX_CONFIG_BYTES)?;
        let value = parse_canonical(&raw, MAX_CONFIG_BYTES).map_err(|_| PublisherError::Config)?;
        let config: Self = serde_json::from_value(value).map_err(|_| PublisherError::Config)?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), PublisherError> {
        let bounded = [
            &self.signing_key_id,
            &self.signature_domain,
            &self.issuer,
            &self.jwks_url,
            &self.audience,
            &self.repository,
            &self.repository_id,
            &self.repository_owner_id,
            &self.environment,
            &self.default_branch,
            &self.caller_workflow_path,
            &self.protected_workflow_path,
        ];
        if self.schema_version != 2
            || bounded
                .iter()
                .any(|value| value.is_empty() || value.len() > MAX_JSON_STRING_BYTES)
            || self.issuer != GITHUB_ISSUER
            || self.jwks_url != format!("{GITHUB_ISSUER}/.well-known/jwks")
            || self.signature_domain != "production"
            || self.allowed_actor_ids.is_empty()
            || self.allowed_actor_ids.len() > 64
            || self.network_deadline_milliseconds == 0
            || self.network_deadline_milliseconds > 10_000
            || self.request_deadline_milliseconds == 0
            || self.request_deadline_milliseconds > 30_000
            || self.network_deadline_milliseconds > self.request_deadline_milliseconds
            || self.max_inflight_requests == 0
            || self.max_inflight_requests > MAX_INFLIGHT_REQUESTS
            || self.jwks_cache_seconds == 0
            || self.jwks_cache_seconds > 3_600
            || self.max_receipts == 0
            || self.max_receipts > MAX_STORE_RECEIPTS
            || !(MIN_LEDGER_BYTES..=MAX_LEDGER_BYTES).contains(&self.max_ledger_bytes)
            || !valid_id(&self.signing_key_id)
            || !self.repository.contains('/')
            || !self.repository_id.bytes().all(|byte| byte.is_ascii_digit())
            || !self
                .repository_owner_id
                .bytes()
                .all(|byte| byte.is_ascii_digit())
            || self.default_branch != "main"
            || self.environment != "protected-publisher"
            || !self.listen.ip().is_loopback()
            || !self.ledger_path.is_absolute()
            || !self.enrollment_artifact_path.is_absolute()
            || !self.signing_key_path.is_absolute()
            || self.enrollment_artifact_path == self.ledger_path
            || self.enrollment_artifact_path == self.signing_key_path
            || self.ledger_path == self.signing_key_path
        {
            return Err(PublisherError::Config);
        }
        let actors: BTreeSet<_> = self.allowed_actor_ids.iter().collect();
        if actors.len() != self.allowed_actor_ids.len()
            || self.allowed_actor_ids.iter().any(|value| {
                value.is_empty()
                    || value.len() > 32
                    || !value.bytes().all(|byte| byte.is_ascii_digit())
            })
        {
            return Err(PublisherError::Config);
        }
        Ok(())
    }

    pub fn network_deadline(&self) -> Duration {
        Duration::from_millis(self.network_deadline_milliseconds)
    }

    pub fn request_deadline(&self) -> Duration {
        Duration::from_millis(self.request_deadline_milliseconds)
    }

    pub fn minimum_token_remaining_seconds(&self) -> i64 {
        let deadline_seconds = self.request_deadline_milliseconds.div_ceil(1_000) as i64;
        deadline_seconds + crate::bounds::TOKEN_RECOVERY_GRACE_SECS
    }

    pub fn jwks_cache_ttl(&self) -> Duration {
        Duration::from_secs(self.jwks_cache_seconds)
    }

    pub fn queue_prefix(&self) -> String {
        format!("refs/heads/gh-readonly-queue/{}/", self.default_branch)
    }

    pub fn config_sha256(&self) -> Result<String, PublisherError> {
        let value = serde_json::to_value(self).map_err(|_| PublisherError::Config)?;
        let bytes = canonical_bytes(&value).map_err(|_| PublisherError::Config)?;
        let digest = Sha256::digest(bytes);
        Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    pub fn service_identity(&self) -> Result<String, PublisherError> {
        let digest = self.config_sha256()?;
        Ok(format!("c{}", &digest[..63]))
    }
}

pub fn valid_id(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(first) if first.is_ascii_lowercase())
        && value.len() <= 64
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canonical::canonical_bytes;
    use crate::test_support::{config, secure_tempdir};
    use std::fs::hard_link;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn production_config(root: &Path) -> ServiceConfig {
        let mut config = config(root.join("publisher.ledger"));
        config.signing_key_path = root.join("publisher.pem");
        config.signature_domain = "production".into();
        config.jwks_url = format!("{GITHUB_ISSUER}/.well-known/jwks");
        config
    }

    fn write_secure(path: &Path, bytes: &[u8]) {
        std::fs::write(path, bytes).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[test]
    fn canonical_production_config_loads_exactly() {
        let temp = secure_tempdir();
        let config = production_config(temp.path());
        assert!(config.validate().is_ok());
        let path = temp.path().join("config.json");
        let value = serde_json::to_value(&config).unwrap();
        write_secure(&path, &canonical_bytes(&value).unwrap());
        let loaded = ServiceConfig::load(&path).unwrap();
        assert_eq!(loaded.repository, "powderluv/fe2o3");
        assert_eq!(loaded.listen.ip(), std::net::Ipv4Addr::LOCALHOST);
        assert_eq!(loaded.config_sha256().unwrap().len(), 64);
        assert_eq!(loaded.service_identity().unwrap().len(), 64);
        assert!(valid_id(&loaded.service_identity().unwrap()));
    }

    #[test]
    fn deployment_boundary_mutations_reject() {
        let temp = secure_tempdir();
        let base = production_config(temp.path());

        let mut config = base.clone();
        config.listen = "0.0.0.0:9443".parse().unwrap();
        assert!(config.validate().is_err());
        let mut config = base.clone();
        config.jwks_url = "https://attacker.invalid/jwks".into();
        assert!(config.validate().is_err());
        let mut config = base.clone();
        config.issuer = "https://attacker.invalid".into();
        assert!(config.validate().is_err());
        let mut config = base.clone();
        config.signature_domain = "test".into();
        assert!(config.validate().is_err());
        let mut config = base.clone();
        config.allowed_actor_ids = vec!["101".into(), "101".into()];
        assert!(config.validate().is_err());
        let mut config = base;
        config.ledger_path = "relative.ledger".into();
        assert!(config.validate().is_err());
        let mut config = production_config(temp.path());
        config.enrollment_artifact_path = "relative-enrollment.json".into();
        assert!(config.validate().is_err());

        let base = production_config(temp.path());
        let mut config = base.clone();
        config.max_receipts = 0;
        assert!(config.validate().is_err());
        let mut config = base.clone();
        config.max_ledger_bytes = MIN_LEDGER_BYTES - 1;
        assert!(config.validate().is_err());
    }

    #[test]
    fn noncanonical_and_duplicate_config_json_rejects() {
        let temp = secure_tempdir();
        let path = temp.path().join("config.json");
        write_secure(&path, b"{\"schema_version\":1,\"schema_version\":1}\n");
        assert!(ServiceConfig::load(&path).is_err());
        write_secure(&path, b"{ \"schema_version\": 1 }\n");
        assert!(ServiceConfig::load(&path).is_err());
    }

    #[test]
    fn config_file_must_be_owner_only_single_link_and_not_a_symlink() {
        let temp = secure_tempdir();
        let config = production_config(temp.path());
        let bytes = canonical_bytes(&serde_json::to_value(config).unwrap()).unwrap();
        let path = temp.path().join("config.json");
        std::fs::write(&path, &bytes).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
        assert!(ServiceConfig::load(&path).is_err());

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let symlink_path = temp.path().join("config-symlink.json");
        symlink(&path, &symlink_path).unwrap();
        assert!(ServiceConfig::load(&symlink_path).is_err());

        let hardlink_path = temp.path().join("config-hardlink.json");
        hard_link(&path, &hardlink_path).unwrap();
        assert!(ServiceConfig::load(&path).is_err());
        assert!(ServiceConfig::load(&hardlink_path).is_err());
    }

    #[test]
    fn every_authority_config_mutation_changes_service_identity() {
        let temp = secure_tempdir();
        let base = production_config(temp.path());
        let identity = base.service_identity().unwrap();
        let mut mutations = Vec::new();

        let mut changed = base.clone();
        changed.audience.push_str("/changed");
        mutations.push(changed);
        let mut changed = base.clone();
        changed.repository_id.push('7');
        mutations.push(changed);
        let mut changed = base.clone();
        changed.allowed_actor_ids = vec!["202".into()];
        mutations.push(changed);
        let mut changed = base.clone();
        changed.caller_workflow_path.push_str(".changed");
        mutations.push(changed);
        let mut changed = base;
        changed.signing_key_path = temp.path().join("rotated.pem");
        mutations.push(changed);
        let mut changed = production_config(temp.path());
        changed.enrollment_artifact_path = temp.path().join("rotated-enrollment.json");
        mutations.push(changed);

        assert!(
            mutations
                .iter()
                .all(|changed| changed.service_identity().unwrap() != identity)
        );
    }
}
