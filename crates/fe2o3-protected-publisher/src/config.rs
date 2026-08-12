use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use crate::PublisherError;
use crate::bounds::{MAX_CONFIG_BYTES, MAX_JSON_STRING_BYTES};
use crate::canonical::parse_canonical;

pub const GITHUB_ISSUER: &str = "https://token.actions.githubusercontent.com";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceConfig {
    pub schema_version: u32,
    pub listen: SocketAddr,
    pub database_path: PathBuf,
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
    pub network_deadline_milliseconds: u64,
}

impl ServiceConfig {
    pub fn load(path: &Path) -> Result<Self, PublisherError> {
        let metadata = std::fs::symlink_metadata(path).map_err(|_| PublisherError::Config)?;
        if !metadata.file_type().is_file() || metadata.len() > MAX_CONFIG_BYTES as u64 {
            return Err(PublisherError::Config);
        }
        let raw = std::fs::read(path).map_err(|_| PublisherError::Config)?;
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
        if self.schema_version != 1
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
            || !self.database_path.is_absolute()
            || !self.signing_key_path.is_absolute()
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

    pub fn queue_prefix(&self) -> String {
        format!("refs/heads/gh-readonly-queue/{}/", self.default_branch)
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
