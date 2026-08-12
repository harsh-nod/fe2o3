mod bounds;
mod canonical;
mod config;
mod enrollment;
mod jwks;
mod oidc;
mod process_security;
mod receipt;
mod secure_fs;
mod service;
mod store;
mod store_worker;
#[cfg(test)]
mod test_support;

pub use config::ServiceConfig;
pub use enrollment::enroll_token;
pub use jwks::{HttpsJwksProvider, JwksProvider, JwksSnapshot, StaticJwksProvider};
pub use process_security::harden_process_for_secrets;
pub use service::{Publisher, PublisherResponse, router};

#[derive(Debug, thiserror::Error)]
pub enum PublisherError {
    #[error("configuration is invalid")]
    Config,
    #[error("request is malformed or exceeds a bound")]
    Request,
    #[error("authentication failed closed")]
    Authentication,
    #[error("JWKS acquisition failed closed")]
    Jwks,
    #[error("replay conflicts with an existing request")]
    ReplayConflict,
    #[error("durable replay store failed closed")]
    Store,
    #[error("receipt signing failed closed")]
    Signing,
}

impl PublisherError {
    pub fn public_code(&self) -> &'static str {
        match self {
            Self::Request => "invalid_request",
            Self::Authentication | Self::Jwks => "unauthorized",
            Self::ReplayConflict => "replay_conflict",
            Self::Config | Self::Store | Self::Signing => "service_unavailable",
        }
    }
}
