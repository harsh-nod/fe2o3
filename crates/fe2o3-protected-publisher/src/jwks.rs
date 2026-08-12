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
        Self::build(url, timeout, None)
    }

    fn build(
        url: &str,
        timeout: Duration,
        extra_root: Option<reqwest::Certificate>,
    ) -> Result<Self, PublisherError> {
        let url = reqwest::Url::parse(url).map_err(|_| PublisherError::Config)?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || url.username() != ""
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(PublisherError::Config);
        }
        let mut builder = reqwest::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(timeout)
            .timeout(timeout)
            .user_agent("fe2o3-protected-publisher/1");
        if let Some(root) = extra_root {
            builder = builder.add_root_certificate(root);
        }
        let client = builder.build().map_err(|_| PublisherError::Config)?;
        Ok(Self { client, url })
    }

    #[cfg(test)]
    fn with_test_root(
        url: &str,
        timeout: Duration,
        root_pem: &[u8],
    ) -> Result<Self, PublisherError> {
        let root = reqwest::Certificate::from_pem(root_pem).map_err(|_| PublisherError::Config)?;
        Self::build(url, timeout, Some(root))
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

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Stdio};
    use std::thread;

    use super::*;
    use crate::test_support::secure_tempdir;

    const CERT: &[u8] = include_bytes!("../tests/fixtures/mock-issuer-ca.pem");

    struct MockIssuer {
        child: Child,
        _directory: tempfile::TempDir,
        url: String,
    }

    impl MockIssuer {
        fn start(mode: &str) -> Self {
            let directory = secure_tempdir();
            let port_file = directory.path().join("port");
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
            let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
            let mut child = Command::new("python3")
                .arg(root.join("scripts/tests/mock-publisher-issuer.py"))
                .args([
                    "--cert",
                    fixture.join("mock-issuer-cert.pem").to_str().unwrap(),
                ])
                .args([
                    "--key",
                    fixture.join("mock-issuer-key.pem").to_str().unwrap(),
                ])
                .args(["--port-file", port_file.to_str().unwrap()])
                .args(["--mode", mode])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let port = wait_for_port(&port_file, &mut child);
            Self {
                child,
                _directory: directory,
                url: format!("https://localhost:{port}/jwks"),
            }
        }
    }

    impl Drop for MockIssuer {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    fn wait_for_port(path: &PathBuf, child: &mut Child) -> u16 {
        for _ in 0..200 {
            if let Ok(value) = std::fs::read_to_string(path) {
                return value.trim().parse().unwrap();
            }
            assert!(
                child.try_wait().unwrap().is_none(),
                "mock issuer exited early"
            );
            thread::sleep(Duration::from_millis(5));
        }
        panic!("mock issuer did not publish its port");
    }

    #[tokio::test]
    async fn validated_https_fetches_bounded_jwks() {
        let issuer = MockIssuer::start("jwks");
        let provider =
            HttpsJwksProvider::with_test_root(&issuer.url, Duration::from_secs(1), CERT).unwrap();
        let body = provider
            .fetch(Instant::now() + Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(body, b"{\"keys\":[]}\n");
    }

    #[tokio::test]
    async fn untrusted_cert_redirect_and_oversize_fail_closed() {
        let untrusted = MockIssuer::start("jwks");
        let provider = HttpsJwksProvider::new(&untrusted.url, Duration::from_secs(1)).unwrap();
        assert!(
            provider
                .fetch(Instant::now() + Duration::from_secs(1))
                .await
                .is_err()
        );

        for mode in ["redirect", "oversize"] {
            let issuer = MockIssuer::start(mode);
            let provider =
                HttpsJwksProvider::with_test_root(&issuer.url, Duration::from_secs(1), CERT)
                    .unwrap();
            assert!(
                provider
                    .fetch(Instant::now() + Duration::from_secs(1))
                    .await
                    .is_err()
            );
        }
    }

    #[tokio::test]
    async fn slow_response_obeys_one_absolute_deadline() {
        let issuer = MockIssuer::start("slow");
        let provider =
            HttpsJwksProvider::with_test_root(&issuer.url, Duration::from_millis(75), CERT)
                .unwrap();
        let start = Instant::now();
        assert!(
            provider
                .fetch(start + Duration::from_millis(75))
                .await
                .is_err()
        );
        assert!(start.elapsed() < Duration::from_millis(500));
    }
}
