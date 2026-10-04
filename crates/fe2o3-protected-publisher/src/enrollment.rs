use std::os::fd::RawFd;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

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
    crate::process_security::harden_process_for_secrets()?;
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
    crate::process_security::harden_process_for_secrets()?;
    if artifact_path != config.enrollment_artifact_path {
        return Err(PublisherError::Config);
    }
    let mut token_bytes = read_nonregular_token_fd(token_fd, config.network_deadline())?;
    while token_bytes
        .last()
        .is_some_and(|byte| *byte == b'\r' || *byte == b'\n')
    {
        token_bytes.pop();
    }
    let token = std::str::from_utf8(&token_bytes).map_err(|_| PublisherError::Authentication)?;
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

fn read_nonregular_token_fd(
    fd: RawFd,
    timeout: Duration,
) -> Result<Zeroizing<Vec<u8>>, PublisherError> {
    read_nonregular_token_fd_with_hooks(fd, timeout, |_| {}, || {}, |_| {})
}

#[cfg_attr(test, allow(clippy::too_many_arguments))]
pub(crate) fn read_nonregular_token_fd_with_hooks(
    fd: RawFd,
    timeout: Duration,
    mut after_readiness: impl FnMut(RawFd),
    mut after_would_block: impl FnMut(),
    mut after_scratch_clear: impl FnMut(&[u8]),
) -> Result<Zeroizing<Vec<u8>>, PublisherError> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(PublisherError::Authentication)?;
    let before = fd_identity(fd)?;
    let kind = before.st_mode & libc::S_IFMT;
    if kind == libc::S_IFREG || !(kind == libc::S_IFIFO || kind == libc::S_IFSOCK) {
        return Err(PublisherError::Config);
    }
    if kind == libc::S_IFSOCK {
        require_unix_socket(fd)?;
    }
    require_nonblocking(fd)?;
    let duplicated = DescriptorDuplicate::new(fd)?;
    let opened = fd_identity(duplicated.fd)?;
    if before.st_dev != opened.st_dev
        || before.st_ino != opened.st_ino
        || before.st_mode != opened.st_mode
        || before.st_uid != opened.st_uid
        || before.st_gid != opened.st_gid
    {
        return Err(PublisherError::Config);
    }
    if kind == libc::S_IFSOCK {
        require_unix_socket(duplicated.fd)?;
    }
    require_nonblocking(duplicated.fd)?;
    let result = read_token_until(
        duplicated.fd,
        deadline,
        &mut after_readiness,
        &mut after_would_block,
        &mut after_scratch_clear,
    );
    require_nonblocking(duplicated.fd)?;
    result
}

fn read_token_until(
    fd: RawFd,
    deadline: Instant,
    after_readiness: &mut impl FnMut(RawFd),
    after_would_block: &mut impl FnMut(),
    after_scratch_clear: &mut impl FnMut(&[u8]),
) -> Result<Zeroizing<Vec<u8>>, PublisherError> {
    let mut bytes = Zeroizing::new(Vec::with_capacity(MAX_JWT_BYTES + 1));
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(PublisherError::Authentication);
        }
        let timeout_milliseconds = remaining
            .as_millis()
            .saturating_add(1)
            .min(libc::c_int::MAX as u128) as libc::c_int;
        let mut poll = libc::pollfd {
            fd,
            events: libc::POLLIN | libc::POLLHUP,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut poll, 1, timeout_milliseconds) };
        if ready < 0 {
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(PublisherError::Authentication);
        }
        if ready == 0 || poll.revents & (libc::POLLERR | libc::POLLNVAL) != 0 {
            return Err(PublisherError::Authentication);
        }
        if poll.revents & (libc::POLLIN | libc::POLLHUP) == 0 {
            continue;
        }
        if Instant::now() >= deadline {
            return Err(PublisherError::Authentication);
        }
        after_readiness(fd);
        if Instant::now() >= deadline {
            return Err(PublisherError::Authentication);
        }
        require_nonblocking(fd).map_err(|_| PublisherError::Authentication)?;
        let mut chunk = Zeroizing::new([0u8; 4096]);
        let capacity = (MAX_JWT_BYTES + 1 - bytes.len()).min(chunk.len());
        let count = unsafe { libc::read(fd, chunk.as_mut_ptr().cast(), capacity) };
        if count < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::WouldBlock {
                after_would_block();
                continue;
            }
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(PublisherError::Authentication);
        }
        let count = count as usize;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..count]);
        chunk[..count].zeroize();
        after_scratch_clear(&chunk[..count]);
        if bytes.len() > MAX_JWT_BYTES {
            return Err(PublisherError::Authentication);
        }
    }
    if bytes.is_empty() {
        return Err(PublisherError::Authentication);
    }
    Ok(bytes)
}

struct DescriptorDuplicate {
    fd: RawFd,
}

impl DescriptorDuplicate {
    fn new(fd: RawFd) -> Result<Self, PublisherError> {
        let duplicated = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
        if duplicated < 0 {
            return Err(PublisherError::Config);
        }
        Ok(Self { fd: duplicated })
    }
}

impl Drop for DescriptorDuplicate {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

fn require_nonblocking(fd: RawFd) -> Result<(), PublisherError> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || flags & libc::O_NONBLOCK == 0 {
        return Err(PublisherError::Config);
    }
    Ok(())
}

fn require_unix_socket(fd: RawFd) -> Result<(), PublisherError> {
    let mut address = std::mem::MaybeUninit::<libc::sockaddr_storage>::zeroed();
    let mut length = std::mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
    if unsafe {
        libc::getsockname(
            fd,
            address.as_mut_ptr().cast::<libc::sockaddr>(),
            &mut length,
        )
    } != 0
        || length < std::mem::size_of::<libc::sa_family_t>() as libc::socklen_t
    {
        return Err(PublisherError::Config);
    }
    let address = unsafe { address.assume_init() };
    if address.ss_family as libc::c_int != libc::AF_UNIX {
        return Err(PublisherError::Config);
    }
    Ok(())
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
    use std::fs::{File, OpenOptions};
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::net::UnixStream;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc as StdArc, Barrier, mpsc};
    use std::thread;

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

    fn make_nonblocking(stream: &UnixStream) {
        stream.set_nonblocking(true).unwrap();
    }

    #[tokio::test]
    async fn enrollment_binds_exact_runtime_projection_without_raw_token() {
        let temp = secure_tempdir();
        let config = production_config(temp.path());
        let fixture = fixture();
        let (mut writer, reader) = UnixStream::pair().unwrap();
        make_nonblocking(&reader);
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

    #[test]
    fn unix_socketpair_is_accepted_without_status_flag_mutation() {
        let fixture = fixture();
        let (mut writer, reader) = UnixStream::pair().unwrap();
        make_nonblocking(&reader);
        let original_flags = unsafe { libc::fcntl(reader.as_raw_fd(), libc::F_GETFL) };
        let observed = reader.try_clone().unwrap();
        let stop = StdArc::new(AtomicBool::new(false));
        let reading = StdArc::new(AtomicBool::new(false));
        let changed = StdArc::new(AtomicBool::new(false));
        let samples = StdArc::new(AtomicUsize::new(0));
        let barrier = StdArc::new(Barrier::new(2));
        let (sampled_tx, sampled_rx) = mpsc::sync_channel(1);
        let observer = {
            let stop = stop.clone();
            let reading = reading.clone();
            let changed = changed.clone();
            let samples = samples.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                let mut notified = false;
                while !stop.load(Ordering::Acquire) {
                    let flags = unsafe { libc::fcntl(observed.as_raw_fd(), libc::F_GETFL) };
                    if flags != original_flags {
                        changed.store(true, Ordering::Release);
                    }
                    if reading.load(Ordering::Acquire) {
                        samples.fetch_add(1, Ordering::Relaxed);
                        if !notified {
                            sampled_tx.send(()).unwrap();
                            notified = true;
                        }
                    }
                }
            })
        };
        let token = fixture.token.clone();
        let writer = thread::spawn(move || {
            sampled_rx.recv_timeout(Duration::from_secs(1)).unwrap();
            writer.write_all(token.as_bytes()).unwrap();
            writer.shutdown(std::net::Shutdown::Write).unwrap();
        });
        barrier.wait();
        reading.store(true, Ordering::Release);
        let bytes = read_nonregular_token_fd(reader.as_raw_fd(), Duration::from_secs(1)).unwrap();
        reading.store(false, Ordering::Release);
        stop.store(true, Ordering::Release);
        writer.join().unwrap();
        observer.join().unwrap();
        assert_eq!(&*bytes, fixture.token.as_bytes());
        assert!(samples.load(Ordering::Relaxed) > 0);
        assert!(!changed.load(Ordering::Acquire));
        assert_eq!(
            unsafe { libc::fcntl(reader.as_raw_fd(), libc::F_GETFL) },
            original_flags
        );
    }

    #[test]
    fn blocking_descriptor_is_rejected_without_flag_mutation() {
        let (_writer, reader) = UnixStream::pair().unwrap();
        let original_flags = unsafe { libc::fcntl(reader.as_raw_fd(), libc::F_GETFL) };
        assert_eq!(original_flags & libc::O_NONBLOCK, 0);
        assert!(matches!(
            read_nonregular_token_fd(reader.as_raw_fd(), Duration::from_millis(20)),
            Err(PublisherError::Config)
        ));
        assert_eq!(
            unsafe { libc::fcntl(reader.as_raw_fd(), libc::F_GETFL) },
            original_flags
        );
    }

    #[test]
    fn nonblocking_fifo_is_accepted() {
        let temp = secure_tempdir();
        let path = temp.path().join("token.fifo");
        let path_bytes = std::os::unix::ffi::OsStrExt::as_bytes(path.as_os_str());
        let path_c = std::ffi::CString::new(path_bytes).unwrap();
        assert_eq!(unsafe { libc::mkfifo(path_c.as_ptr(), 0o600) }, 0);
        let reader = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC | libc::O_NOFOLLOW)
            .open(&path)
            .unwrap();
        let mut writer = OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC | libc::O_NOFOLLOW)
            .open(&path)
            .unwrap();
        let fixture = fixture();
        writer.write_all(fixture.token.as_bytes()).unwrap();
        drop(writer);
        let bytes = read_nonregular_token_fd(reader.as_raw_fd(), Duration::from_secs(1)).unwrap();
        assert_eq!(&*bytes, fixture.token.as_bytes());
    }

    #[test]
    fn tcp_socket_descriptor_is_rejected_before_read() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        assert!(matches!(
            read_nonregular_token_fd(client.as_raw_fd(), Duration::from_millis(50)),
            Err(PublisherError::Config)
        ));
        assert!(matches!(
            read_nonregular_token_fd(server.as_raw_fd(), Duration::from_millis(50)),
            Err(PublisherError::Config)
        ));
    }

    #[test]
    fn competing_reader_and_repeated_eagain_cannot_escape_deadline_loop() {
        let fixture = fixture();
        let expected = fixture.token.into_bytes();
        let token = expected.clone();
        let (mut writer, reader) = UnixStream::pair().unwrap();
        make_nonblocking(&reader);
        let competitor = reader.try_clone().unwrap();
        let (drained_tx, drained_rx) = mpsc::sync_channel(0);
        let producer = thread::spawn(move || {
            for byte in [b'a', b'b', b'c'] {
                writer.write_all(&[byte]).unwrap();
                drained_rx.recv().unwrap();
            }
            for chunk in token.chunks(37) {
                writer.write_all(chunk).unwrap();
                thread::sleep(Duration::from_millis(1));
            }
            writer.shutdown(std::net::Shutdown::Write).unwrap();
        });
        let mut drained = 0usize;
        let bytes = read_nonregular_token_fd_with_hooks(
            reader.as_raw_fd(),
            Duration::from_secs(2),
            |_| {
                if drained < 3 {
                    let mut scratch = [0u8; 16];
                    let count = unsafe {
                        libc::read(
                            competitor.as_raw_fd(),
                            scratch.as_mut_ptr().cast(),
                            scratch.len(),
                        )
                    };
                    assert_eq!(count, 1);
                    drained += 1;
                }
            },
            || drained_tx.send(()).unwrap(),
            |scratch| assert!(scratch.iter().all(|byte| *byte == 0)),
        )
        .unwrap();
        producer.join().unwrap();
        assert_eq!(drained, 3);
        assert_eq!(&*bytes, expected.as_slice());
    }

    #[test]
    fn partial_chunks_eof_and_total_byte_bound_are_exact() {
        let fixture = fixture();
        let token = fixture.token.clone().into_bytes();
        let (mut writer, reader) = UnixStream::pair().unwrap();
        make_nonblocking(&reader);
        let producer = thread::spawn(move || {
            for chunk in token.chunks(11) {
                writer.write_all(chunk).unwrap();
                thread::sleep(Duration::from_millis(1));
            }
            writer.shutdown(std::net::Shutdown::Write).unwrap();
        });
        let bytes = read_nonregular_token_fd(reader.as_raw_fd(), Duration::from_secs(2)).unwrap();
        producer.join().unwrap();
        assert_eq!(&*bytes, fixture.token.as_bytes());

        let (writer, reader) = UnixStream::pair().unwrap();
        make_nonblocking(&reader);
        writer.shutdown(std::net::Shutdown::Write).unwrap();
        assert!(matches!(
            read_nonregular_token_fd(reader.as_raw_fd(), Duration::from_millis(100)),
            Err(PublisherError::Authentication)
        ));

        let (mut writer, reader) = UnixStream::pair().unwrap();
        make_nonblocking(&reader);
        writer.write_all(&vec![b'a'; MAX_JWT_BYTES + 1]).unwrap();
        writer.shutdown(std::net::Shutdown::Write).unwrap();
        assert!(matches!(
            read_nonregular_token_fd(reader.as_raw_fd(), Duration::from_millis(100)),
            Err(PublisherError::Authentication)
        ));
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
    async fn stalled_token_pipe_obeys_enrollment_deadline() {
        let temp = secure_tempdir();
        let mut config = production_config(temp.path());
        config.network_deadline_milliseconds = 40;
        let (_writer, reader) = UnixStream::pair().unwrap();
        make_nonblocking(&reader);
        let start = Instant::now();
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
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(25));
        assert!(elapsed < Duration::from_millis(500));
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
            make_nonblocking(&reader);
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
        let docs = include_str!("../../../docs/protected-publisher-service-v1.md");
        assert!(!main.contains("--token-file"));
        assert!(!main.contains("--token="));
        assert!(main.contains("--token-fd"));
        let forbidden_flag_mutation = ["F_SET", "FL"].concat();
        assert!(!include_str!("enrollment.rs").contains(&forbidden_flag_mutation));
        assert!(!docs.contains("--token-file"));
        assert!(docs.contains("--token-fd"));
        assert!(docs.contains(&format!("never calls\n`{forbidden_flag_mutation}`")));
    }
}
