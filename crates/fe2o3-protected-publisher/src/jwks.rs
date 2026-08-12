use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE};
use tokio::sync::{Mutex, RwLock, Semaphore};
use tokio::time::Instant;

use crate::PublisherError;
use crate::bounds::{
    JWKS_FORCED_REFRESH_FLOOR_SECS, JWKS_FORCED_REFRESH_MAX_BACKOFF_SECS,
    MAX_CACHED_JWKS_KID_BYTES, MAX_JWKS_BYTES, MAX_NEGATIVE_JWKS_KIDS, MAX_RESPONSE_BYTES,
};

#[derive(Clone, Debug)]
pub struct JwksSnapshot {
    pub bytes: Vec<u8>,
    pub generation: u64,
}

pub trait JwksProvider: Send + Sync {
    fn fetch<'a>(
        &'a self,
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<JwksSnapshot, PublisherError>> + Send + 'a>>;

    fn refresh<'a>(
        &'a self,
        deadline: Instant,
        observed_generation: u64,
        unknown_kid: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<JwksSnapshot, PublisherError>> + Send + 'a>>;
}

pub struct HttpsJwksProvider {
    client: reqwest::Client,
    url: reqwest::Url,
    cache_ttl: Duration,
    cache: RwLock<Option<CachedJwks>>,
    refresh: Mutex<()>,
    outbound: Semaphore,
    negative_kids: Mutex<NegativeKidState>,
    maximum_backoff: Duration,
}

struct CachedJwks {
    bytes: Vec<u8>,
    fresh_until: Instant,
    generation: u64,
}

struct NegativeKidState {
    entries: VecDeque<NegativeKid>,
    next_forced_refresh: Option<Instant>,
    backoff: Duration,
}

struct NegativeKid {
    generation: u64,
    kid: String,
    retry_at: Instant,
}

impl HttpsJwksProvider {
    pub fn new(url: &str, timeout: Duration, cache_ttl: Duration) -> Result<Self, PublisherError> {
        Self::build(
            url,
            timeout,
            cache_ttl,
            None,
            Duration::from_secs(JWKS_FORCED_REFRESH_FLOOR_SECS),
            Duration::from_secs(JWKS_FORCED_REFRESH_MAX_BACKOFF_SECS),
        )
    }

    fn build(
        url: &str,
        timeout: Duration,
        cache_ttl: Duration,
        extra_root: Option<reqwest::Certificate>,
        refresh_floor: Duration,
        maximum_backoff: Duration,
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
            .no_proxy()
            .connect_timeout(timeout)
            .timeout(timeout)
            .user_agent("fe2o3-protected-publisher/1");
        if let Some(root) = extra_root {
            builder = builder.add_root_certificate(root);
        }
        let client = builder.build().map_err(|_| PublisherError::Config)?;
        if cache_ttl.is_zero()
            || cache_ttl > Duration::from_secs(3_600)
            || refresh_floor.is_zero()
            || maximum_backoff < refresh_floor
            || maximum_backoff > Duration::from_secs(JWKS_FORCED_REFRESH_MAX_BACKOFF_SECS)
        {
            return Err(PublisherError::Config);
        }
        Ok(Self {
            client,
            url,
            cache_ttl,
            cache: RwLock::new(None),
            refresh: Mutex::new(()),
            outbound: Semaphore::new(1),
            negative_kids: Mutex::new(NegativeKidState {
                entries: VecDeque::new(),
                next_forced_refresh: None,
                backoff: refresh_floor,
            }),
            maximum_backoff,
        })
    }

    #[cfg(test)]
    fn with_test_root(
        url: &str,
        timeout: Duration,
        cache_ttl: Duration,
        root_pem: &[u8],
    ) -> Result<Self, PublisherError> {
        let root = reqwest::Certificate::from_pem(root_pem).map_err(|_| PublisherError::Config)?;
        Self::build(
            url,
            timeout,
            cache_ttl,
            Some(root),
            Duration::from_millis(25),
            Duration::from_millis(100),
        )
    }

    async fn fresh_cache(&self, now: Instant) -> Option<JwksSnapshot> {
        self.cache
            .read()
            .await
            .as_ref()
            .filter(|entry| now < entry.fresh_until)
            .map(snapshot)
    }

    async fn fetch_cached(&self, deadline: Instant) -> Result<JwksSnapshot, PublisherError> {
        if let Some(snapshot) = self.fresh_cache(Instant::now()).await {
            return Ok(snapshot);
        }
        let _refresh = tokio::time::timeout_at(deadline, self.refresh.lock())
            .await
            .map_err(|_| PublisherError::Jwks)?;
        if let Some(snapshot) = self.fresh_cache(Instant::now()).await {
            return Ok(snapshot);
        }
        self.refresh_locked(deadline).await
    }

    async fn refresh_after(
        &self,
        deadline: Instant,
        observed_generation: u64,
        unknown_kid: &str,
    ) -> Result<JwksSnapshot, PublisherError> {
        let _refresh = tokio::time::timeout_at(deadline, self.refresh.lock())
            .await
            .map_err(|_| PublisherError::Jwks)?;
        if let Some(entry) = self.cache.read().await.as_ref()
            && entry.generation != observed_generation
        {
            return Ok(snapshot(entry));
        }
        let now = Instant::now();
        let mut negative = self.negative_kids.lock().await;
        if negative.entries.iter().any(|entry| {
            entry.kid == unknown_kid
                && entry.generation == observed_generation
                && now < entry.retry_at
        }) || negative
            .next_forced_refresh
            .is_some_and(|retry_at| now < retry_at)
        {
            remember_negative(&mut negative, unknown_kid, observed_generation, now);
            drop(negative);
            return self
                .cache
                .read()
                .await
                .as_ref()
                .map(snapshot)
                .ok_or(PublisherError::Jwks);
        }
        let backoff = negative.backoff;
        negative.next_forced_refresh = now.checked_add(backoff);
        negative.backoff = backoff
            .checked_mul(2)
            .unwrap_or(self.maximum_backoff)
            .min(self.maximum_backoff);
        drop(negative);
        let refreshed = self.refresh_locked(deadline).await;
        let mut negative = self.negative_kids.lock().await;
        negative.next_forced_refresh = Instant::now().checked_add(backoff);
        if let Ok(refreshed) = &refreshed {
            remember_negative(
                &mut negative,
                unknown_kid,
                refreshed.generation,
                Instant::now(),
            );
        }
        refreshed
    }

    async fn refresh_locked(&self, deadline: Instant) -> Result<JwksSnapshot, PublisherError> {
        let _outbound = tokio::time::timeout_at(deadline, self.outbound.acquire())
            .await
            .map_err(|_| PublisherError::Jwks)?
            .map_err(|_| PublisherError::Jwks)?;
        let bytes = self.fetch_inner(deadline).await?;
        let fresh_until = Instant::now()
            .checked_add(self.cache_ttl)
            .ok_or(PublisherError::Jwks)?;
        let mut cache = self.cache.write().await;
        let generation = cache
            .as_ref()
            .map_or(1, |entry| entry.generation.saturating_add(1));
        if generation == u64::MAX {
            return Err(PublisherError::Jwks);
        }
        *cache = Some(CachedJwks {
            bytes: bytes.clone(),
            fresh_until,
            generation,
        });
        Ok(JwksSnapshot { bytes, generation })
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

    #[cfg(test)]
    async fn negative_kid_names(&self) -> Vec<String> {
        self.negative_kids
            .lock()
            .await
            .entries
            .iter()
            .map(|entry| entry.kid.clone())
            .collect()
    }
}

impl JwksProvider for HttpsJwksProvider {
    fn fetch<'a>(
        &'a self,
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<JwksSnapshot, PublisherError>> + Send + 'a>> {
        Box::pin(self.fetch_cached(deadline))
    }

    fn refresh<'a>(
        &'a self,
        deadline: Instant,
        observed_generation: u64,
        unknown_kid: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<JwksSnapshot, PublisherError>> + Send + 'a>> {
        Box::pin(self.refresh_after(deadline, observed_generation, unknown_kid))
    }
}

fn snapshot(entry: &CachedJwks) -> JwksSnapshot {
    JwksSnapshot {
        bytes: entry.bytes.clone(),
        generation: entry.generation,
    }
}

fn remember_negative(state: &mut NegativeKidState, kid: &str, generation: u64, now: Instant) {
    if kid.len() > MAX_CACHED_JWKS_KID_BYTES {
        return;
    }
    state.entries.retain(|entry| entry.kid != kid);
    state.entries.push_back(NegativeKid {
        generation,
        kid: kid.into(),
        retry_at: state.next_forced_refresh.unwrap_or(now),
    });
    while state.entries.len() > MAX_NEGATIVE_JWKS_KIDS {
        state.entries.pop_front();
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
    ) -> Pin<Box<dyn Future<Output = Result<JwksSnapshot, PublisherError>> + Send + 'a>> {
        Box::pin(async move {
            if self.failure {
                Err(PublisherError::Jwks)
            } else {
                Ok(JwksSnapshot {
                    bytes: self.bytes.as_ref().clone(),
                    generation: 1,
                })
            }
        })
    }

    fn refresh<'a>(
        &'a self,
        deadline: Instant,
        _observed_generation: u64,
        _unknown_kid: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<JwksSnapshot, PublisherError>> + Send + 'a>> {
        self.fetch(deadline)
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
        directory: tempfile::TempDir,
        url: String,
    }

    impl MockIssuer {
        fn start(mode: &str) -> Self {
            let directory = secure_tempdir();
            let port_file = directory.path().join("port");
            let count_file = directory.path().join("count");
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
                .args(["--count-file", count_file.to_str().unwrap()])
                .args(["--mode", mode])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let port = wait_for_port(&port_file, &mut child);
            Self {
                child,
                directory,
                url: format!("https://localhost:{port}/jwks"),
            }
        }

        fn request_count(&self) -> usize {
            std::fs::read_to_string(self.directory.path().join("count"))
                .unwrap_or_else(|_| "0".into())
                .trim()
                .parse()
                .unwrap()
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
            if let Ok(value) = std::fs::read_to_string(path)
                && let Ok(port) = value.trim().parse()
            {
                return port;
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
        let provider = HttpsJwksProvider::with_test_root(
            &issuer.url,
            Duration::from_secs(1),
            Duration::from_secs(60),
            CERT,
        )
        .unwrap();
        let body = provider
            .fetch(Instant::now() + Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(body.bytes, b"{\"keys\":[]}\n");
    }

    #[tokio::test]
    async fn untrusted_cert_redirect_and_oversize_fail_closed() {
        let untrusted = MockIssuer::start("jwks");
        let provider = HttpsJwksProvider::new(
            &untrusted.url,
            Duration::from_secs(1),
            Duration::from_secs(60),
        )
        .unwrap();
        assert!(
            provider
                .fetch(Instant::now() + Duration::from_secs(1))
                .await
                .is_err()
        );

        for mode in ["redirect", "oversize"] {
            let issuer = MockIssuer::start(mode);
            let provider = HttpsJwksProvider::with_test_root(
                &issuer.url,
                Duration::from_secs(1),
                Duration::from_secs(60),
                CERT,
            )
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
        let provider = HttpsJwksProvider::with_test_root(
            &issuer.url,
            Duration::from_millis(75),
            Duration::from_secs(60),
            CERT,
        )
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

    #[tokio::test]
    async fn concurrent_cache_misses_singleflight_and_refresh_after_expiry() {
        let issuer = MockIssuer::start("jwks");
        let provider = Arc::new(
            HttpsJwksProvider::with_test_root(
                &issuer.url,
                Duration::from_secs(1),
                Duration::from_millis(25),
                CERT,
            )
            .unwrap(),
        );
        let calls = (0..64)
            .map(|_| {
                let provider = provider.clone();
                tokio::spawn(async move {
                    provider
                        .fetch(Instant::now() + Duration::from_secs(1))
                        .await
                        .unwrap()
                        .bytes
                })
            })
            .collect::<Vec<_>>();
        for call in calls {
            assert_eq!(call.await.unwrap(), b"{\"keys\":[]}\n");
        }
        assert_eq!(issuer.request_count(), 1);
        tokio::time::sleep(Duration::from_millis(35)).await;
        provider
            .fetch(Instant::now() + Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(issuer.request_count(), 2);
    }

    #[tokio::test]
    async fn concurrent_forced_refreshes_singleflight_by_generation() {
        let issuer = MockIssuer::start("jwks");
        let provider = Arc::new(
            HttpsJwksProvider::with_test_root(
                &issuer.url,
                Duration::from_secs(1),
                Duration::from_secs(60),
                CERT,
            )
            .unwrap(),
        );
        let first = provider
            .fetch(Instant::now() + Duration::from_secs(1))
            .await
            .unwrap();
        let calls = (0..64)
            .map(|_| {
                let provider = provider.clone();
                tokio::spawn(async move {
                    provider
                        .refresh(
                            Instant::now() + Duration::from_secs(2),
                            first.generation,
                            "attacker-kid",
                        )
                        .await
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        for call in calls {
            let refreshed = call.await.unwrap();
            assert_eq!(refreshed.generation, 2);
            assert_eq!(refreshed.bytes, b"{\"keys\":[]}\n");
        }
        assert_eq!(issuer.request_count(), 2);
    }

    #[tokio::test]
    async fn sequential_unknown_kid_waves_are_throttled_and_cache_is_bounded() {
        let issuer = MockIssuer::start("jwks");
        let provider = HttpsJwksProvider::with_test_root(
            &issuer.url,
            Duration::from_secs(1),
            Duration::from_secs(60),
            CERT,
        )
        .unwrap();
        let mut snapshot = provider
            .fetch(Instant::now() + Duration::from_secs(1))
            .await
            .unwrap();
        for index in 0..MAX_NEGATIVE_JWKS_KIDS + 17 {
            snapshot = provider
                .refresh(
                    Instant::now() + Duration::from_secs(1),
                    snapshot.generation,
                    &format!("attacker-kid-{index}"),
                )
                .await
                .unwrap();
        }
        assert_eq!(issuer.request_count(), 2);
        let names = provider.negative_kid_names().await;
        assert_eq!(names.len(), MAX_NEGATIVE_JWKS_KIDS);
        assert!(!names.iter().any(|kid| kid == "attacker-kid-0"));
        assert!(
            names
                .iter()
                .any(|kid| kid == &format!("attacker-kid-{}", MAX_NEGATIVE_JWKS_KIDS + 16))
        );
    }

    #[tokio::test]
    async fn legitimate_rotation_recovers_after_bounded_refresh_floor() {
        let issuer = MockIssuer::start("rotate");
        let provider = HttpsJwksProvider::with_test_root(
            &issuer.url,
            Duration::from_secs(1),
            Duration::from_secs(60),
            CERT,
        )
        .unwrap();
        let initial = provider
            .fetch(Instant::now() + Duration::from_secs(1))
            .await
            .unwrap();
        let first = provider
            .refresh(
                Instant::now() + Duration::from_secs(1),
                initial.generation,
                "missing-one",
            )
            .await
            .unwrap();
        assert_eq!(first.bytes, b"{\"keys\":[]}\n");
        let suppressed = provider
            .refresh(
                Instant::now() + Duration::from_secs(1),
                first.generation,
                "rotated-key",
            )
            .await
            .unwrap();
        assert_eq!(suppressed.generation, first.generation);
        assert_eq!(issuer.request_count(), 2);
        tokio::time::sleep(Duration::from_millis(30)).await;
        let rotated = provider
            .refresh(
                Instant::now() + Duration::from_secs(1),
                first.generation,
                "rotated-key",
            )
            .await
            .unwrap();
        assert_eq!(rotated.bytes, b"{\"keys\":[{\"kid\":\"rotated-key\"}]}\n");
        assert_eq!(issuer.request_count(), 3);
    }

    #[tokio::test]
    async fn forced_refresh_wait_uses_original_deadline() {
        let issuer = MockIssuer::start("jwks");
        let provider = HttpsJwksProvider::with_test_root(
            &issuer.url,
            Duration::from_secs(1),
            Duration::from_secs(60),
            CERT,
        )
        .unwrap();
        let initial = provider
            .fetch(Instant::now() + Duration::from_secs(1))
            .await
            .unwrap();
        let guard = provider.refresh.lock().await;
        let start = Instant::now();
        assert!(
            provider
                .refresh(
                    start + Duration::from_millis(25),
                    initial.generation,
                    "blocked-kid",
                )
                .await
                .is_err()
        );
        assert!(start.elapsed() < Duration::from_millis(200));
        drop(guard);
        assert_eq!(issuer.request_count(), 1);
    }

    #[tokio::test]
    async fn cancelled_forced_refresh_releases_singleflight_and_preserves_floor() {
        let issuer = MockIssuer::start("slow-second");
        let provider = Arc::new(
            HttpsJwksProvider::with_test_root(
                &issuer.url,
                Duration::from_secs(3),
                Duration::from_secs(60),
                CERT,
            )
            .unwrap(),
        );
        let initial = provider
            .fetch(Instant::now() + Duration::from_secs(1))
            .await
            .unwrap();
        let task = {
            let provider = provider.clone();
            tokio::spawn(async move {
                provider
                    .refresh(
                        Instant::now() + Duration::from_secs(3),
                        initial.generation,
                        "cancelled-kid",
                    )
                    .await
            })
        };
        for _ in 0..100 {
            if issuer.request_count() >= 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        assert_eq!(issuer.request_count(), 2);
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());

        let suppressed = provider
            .refresh(
                Instant::now() + Duration::from_secs(1),
                initial.generation,
                "next-attacker-kid",
            )
            .await
            .unwrap();
        assert_eq!(suppressed.generation, initial.generation);
        assert_eq!(issuer.request_count(), 2);
        tokio::time::sleep(Duration::from_millis(30)).await;
        let recovered = provider
            .refresh(
                Instant::now() + Duration::from_secs(1),
                initial.generation,
                "legitimate-rotation-kid",
            )
            .await
            .unwrap();
        assert!(recovered.generation > initial.generation);
        assert_eq!(issuer.request_count(), 3);
    }

    #[test]
    fn system_proxy_environment_is_ignored() {
        let issuer = MockIssuer::start("jwks");
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "jwks::tests::proxy_environment_child",
                "--nocapture",
            ])
            .env("FE2O3_TEST_PROXY_JWKS_URL", &issuer.url)
            .env("HTTPS_PROXY", "http://127.0.0.1:9")
            .env("ALL_PROXY", "http://127.0.0.1:9")
            .env("NO_PROXY", "")
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn proxy_environment_child() {
        let Ok(url) = std::env::var("FE2O3_TEST_PROXY_JWKS_URL") else {
            return;
        };
        let provider = HttpsJwksProvider::with_test_root(
            &url,
            Duration::from_secs(1),
            Duration::from_secs(60),
            CERT,
        )
        .unwrap();
        assert_eq!(
            provider
                .fetch(Instant::now() + Duration::from_secs(1))
                .await
                .unwrap()
                .bytes,
            b"{\"keys\":[]}\n"
        );
    }
}
