use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE};
use tokio::time::Instant;

use crate::PublisherError;
use crate::bounds::{MAX_JWKS_BYTES, MAX_RESPONSE_BYTES};

pub trait JwksProvider: Send + Sync {
    fn fetch<'a>(
        &'a self,
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, PublisherError>> + Send + 'a>>;
}

pub struct HttpsJwksProvider {
    client: reqwest::Client,
    url: reqwest::Url,
}

impl HttpsJwksProvider {
    pub fn new(url: &str, timeout: Duration) -> Result<Self, PublisherError> {
        let url = reqwest::Url::parse(url).map_err(|_| PublisherError::Config)?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || url.username() != ""
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(PublisherError::Config);
        }
        let client = reqwest::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(timeout)
            .timeout(timeout)
            .user_agent("fe2o3-protected-publisher/1")
            .build()
            .map_err(|_| PublisherError::Config)?;
        Ok(Self { client, url })
    }

    async fn fetch_inner(&self, deadline: Instant) -> Result<Vec<u8>, PublisherError> {
        let response = tokio::time::timeout_at(deadline, self.client.get(self.url.clone()).send())
            .await
            .map_err(|_| PublisherError::Jwks)?
            .map_err(|_| PublisherError::Jwks)?;
        if response.status() != reqwest::StatusCode::OK || response.url() != &self.url {
            return Err(PublisherError::Jwks);
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(str::trim);
        if content_type != Some("application/json") {
            return Err(PublisherError::Jwks);
        }
        if let Some(length) = response.headers().get(CONTENT_LENGTH) {
            let length = length
                .to_str()
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .ok_or(PublisherError::Jwks)?;
            if length > MAX_JWKS_BYTES {
                return Err(PublisherError::Jwks);
            }
        }

        let mut response = response;
        let mut body = Vec::new();
        body.try_reserve(MAX_JWKS_BYTES.min(4096))
            .map_err(|_| PublisherError::Jwks)?;
        loop {
            let chunk = tokio::time::timeout_at(deadline, response.chunk())
                .await
                .map_err(|_| PublisherError::Jwks)?
                .map_err(|_| PublisherError::Jwks)?;
            let Some(chunk) = chunk else { break };
            let new_length = body
                .len()
                .checked_add(chunk.len())
                .ok_or(PublisherError::Jwks)?;
            if new_length > MAX_JWKS_BYTES || new_length > MAX_RESPONSE_BYTES {
                return Err(PublisherError::Jwks);
            }
            body.try_reserve(chunk.len())
                .map_err(|_| PublisherError::Jwks)?;
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }
}

impl JwksProvider for HttpsJwksProvider {
    fn fetch<'a>(
        &'a self,
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, PublisherError>> + Send + 'a>> {
        Box::pin(self.fetch_inner(deadline))
    }
}

#[derive(Clone)]
pub struct StaticJwksProvider {
    bytes: Arc<Vec<u8>>,
    failure: bool,
}

impl StaticJwksProvider {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Arc::new(bytes),
            failure: false,
        }
    }

    pub fn outage() -> Self {
        Self {
            bytes: Arc::new(Vec::new()),
            failure: true,
        }
    }
}

impl JwksProvider for StaticJwksProvider {
    fn fetch<'a>(
        &'a self,
        _deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, PublisherError>> + Send + 'a>> {
        Box::pin(async move {
            if self.failure {
                Err(PublisherError::Jwks)
            } else {
                Ok(self.bytes.as_ref().clone())
            }
        })
    }
}
