use std::fs::Permissions;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};

use crate::PublisherError;
use crate::ServiceConfig;
use crate::bounds::{
    MAX_DATABASE_BYTES, MAX_STORE_RECEIPTS, MIN_DATABASE_BYTES, RECEIPT_LIFETIME_SECS,
};
use crate::oidc::PublisherRequest;
use crate::receipt::{ReceiptArtifact, ReceiptSigner, build_artifact};

pub struct DurableStore {
    connection: Connection,
    policy: StorePolicy,
}

#[derive(Clone, Copy)]
pub(crate) struct StorePolicy {
    max_receipts: u64,
    max_database_bytes: u64,
    retention_seconds: i64,
    busy_timeout: Duration,
}

impl StorePolicy {
    pub(crate) fn from_config(config: &ServiceConfig) -> Result<Self, PublisherError> {
        let retention_seconds =
            i64::try_from(config.receipt_retention_seconds).map_err(|_| PublisherError::Config)?;
        let policy = Self {
            max_receipts: config.max_receipts,
            max_database_bytes: config.max_database_bytes,
            retention_seconds,
            busy_timeout: Duration::from_millis(config.sqlite_busy_timeout_milliseconds),
        };
        policy.validate()?;
        Ok(policy)
    }

    fn validate(self) -> Result<(), PublisherError> {
        if self.max_receipts == 0
            || self.max_receipts > MAX_STORE_RECEIPTS
            || !(MIN_DATABASE_BYTES..=MAX_DATABASE_BYTES).contains(&self.max_database_bytes)
            || self.retention_seconds < RECEIPT_LIFETIME_SECS
            || self.busy_timeout.is_zero()
            || self.busy_timeout > Duration::from_secs(1)
        {
            return Err(PublisherError::Config);
        }
        Ok(())
    }

    #[cfg(test)]
    fn test_default() -> Self {
        Self {
            max_receipts: 4096,
            max_database_bytes: 64 * 1024 * 1024,
            retention_seconds: 24 * 60 * 60,
            busy_timeout: Duration::from_secs(1),
        }
    }
}

pub(crate) struct IssueInput<'a> {
    pub replay_identity: &'a str,
    pub request_identity: &'a str,
    pub request_sha256: &'a str,
    pub request_body: &'a [u8],
    pub request: &'a PublisherRequest,
    pub issued_at: i64,
    pub observed_at: i64,
    pub signature_domain: &'a str,
    pub signer: &'a dyn ReceiptSigner,
}

impl DurableStore {
    #[cfg(test)]
    pub fn open(path: &Path) -> Result<Self, PublisherError> {
        Self::open_with_policy(path, StorePolicy::test_default())
    }

    pub(crate) fn open_with_policy(
        path: &Path,
        policy: StorePolicy,
    ) -> Result<Self, PublisherError> {
        policy.validate()?;
        let parent = path.parent().ok_or(PublisherError::Config)?;
        let canonical_parent = parent.canonicalize().map_err(|_| PublisherError::Config)?;
        let parent_metadata =
            std::fs::symlink_metadata(parent).map_err(|_| PublisherError::Config)?;
        if canonical_parent != parent
            || !parent_metadata.file_type().is_dir()
            || parent_metadata.uid() != unsafe { libc::geteuid() }
            || parent_metadata.mode() & 0o077 != 0
        {
            return Err(PublisherError::Config);
        }
        if let Ok(metadata) = std::fs::symlink_metadata(path)
            && (!metadata.file_type().is_file()
                || metadata.uid() != unsafe { libc::geteuid() }
                || metadata.nlink() != 1)
        {
            return Err(PublisherError::Config);
        }
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|_| PublisherError::Store)?;
        std::fs::set_permissions(path, Permissions::from_mode(0o600))
            .map_err(|_| PublisherError::Store)?;
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL;
                 PRAGMA synchronous=FULL;
                 PRAGMA foreign_keys=ON;
                 PRAGMA trusted_schema=OFF;",
            )
            .map_err(|_| PublisherError::Store)?;
        connection
            .busy_timeout(policy.busy_timeout)
            .map_err(|_| PublisherError::Store)?;
        let page_size: u64 = connection
            .pragma_query_value(None, "page_size", |row| row.get(0))
            .map_err(|_| PublisherError::Store)?;
        let max_pages = policy
            .max_database_bytes
            .checked_div(page_size)
            .filter(|pages| *pages > 0)
            .ok_or(PublisherError::Config)?;
        connection
            .pragma_update(None, "max_page_count", max_pages)
            .map_err(|_| PublisherError::Store)?;
        let applied_pages: u64 = connection
            .pragma_query_value(None, "max_page_count", |row| row.get(0))
            .map_err(|_| PublisherError::Store)?;
        if applied_pages > max_pages {
            return Err(PublisherError::Store);
        }
        let mut store = Self { connection, policy };
        store.initialize()?;
        std::fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| PublisherError::Store)?;
        Ok(store)
    }

    fn initialize(&mut self) -> Result<(), PublisherError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| PublisherError::Store)?;
        let version: i64 = transaction
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|_| PublisherError::Store)?;
        if version == 0 {
            transaction
                .execute_batch(
                    "CREATE TABLE receipts (
                         request_identity TEXT PRIMARY KEY NOT NULL CHECK(length(request_identity) = 64),
                         replay_identity TEXT UNIQUE NOT NULL CHECK(length(replay_identity) BETWEEN 1 AND 4096),
                         request_sha256 TEXT UNIQUE NOT NULL CHECK(length(request_sha256) = 64),
                         request_body BLOB CHECK(request_body IS NULL OR length(request_body) <= 65536),
                         evidence_identity TEXT UNIQUE NOT NULL CHECK(length(evidence_identity) = 64),
                         response_body BLOB CHECK(response_body IS NULL OR length(response_body) <= 524288),
                         issued_at INTEGER NOT NULL CHECK(issued_at > 0),
                         retired_at INTEGER,
                         CHECK (
                           (request_body IS NOT NULL AND response_body IS NOT NULL AND retired_at IS NULL)
                           OR
                           (request_body IS NULL AND response_body IS NULL AND retired_at IS NOT NULL AND retired_at >= issued_at)
                         )
                     ) STRICT;
                     CREATE INDEX receipts_retention
                       ON receipts(issued_at) WHERE retired_at IS NULL;
                     PRAGMA user_version=2;",
                )
                .map_err(|_| PublisherError::Store)?;
        } else if version == 1 {
            transaction
                .execute_batch(
                    "ALTER TABLE receipts RENAME TO receipts_v1;
                     CREATE TABLE receipts (
                         request_identity TEXT PRIMARY KEY NOT NULL CHECK(length(request_identity) = 64),
                         replay_identity TEXT UNIQUE NOT NULL CHECK(length(replay_identity) BETWEEN 1 AND 4096),
                         request_sha256 TEXT UNIQUE NOT NULL CHECK(length(request_sha256) = 64),
                         request_body BLOB CHECK(request_body IS NULL OR length(request_body) <= 65536),
                         evidence_identity TEXT UNIQUE NOT NULL CHECK(length(evidence_identity) = 64),
                         response_body BLOB CHECK(response_body IS NULL OR length(response_body) <= 524288),
                         issued_at INTEGER NOT NULL CHECK(issued_at > 0),
                         retired_at INTEGER,
                         CHECK (
                           (request_body IS NOT NULL AND response_body IS NOT NULL AND retired_at IS NULL)
                           OR
                           (request_body IS NULL AND response_body IS NULL AND retired_at IS NOT NULL AND retired_at >= issued_at)
                         )
                     ) STRICT;
                     INSERT INTO receipts (
                       request_identity, replay_identity, request_sha256, request_body,
                       evidence_identity, response_body, issued_at, retired_at
                     ) SELECT
                       request_identity, replay_identity, request_sha256, request_body,
                       evidence_identity, response_body, issued_at, NULL
                     FROM receipts_v1;
                     DROP TABLE receipts_v1;
                     CREATE INDEX receipts_retention
                       ON receipts(issued_at) WHERE retired_at IS NULL;
                     PRAGMA user_version=2;",
                )
                .map_err(|_| PublisherError::Store)?;
        } else if version != 2 {
            return Err(PublisherError::Store);
        }
        transaction.commit().map_err(|_| PublisherError::Store)
    }

    pub(crate) fn issue(&mut self, input: IssueInput<'_>) -> Result<Vec<u8>, PublisherError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| PublisherError::Store)?;
        let retire_before = input
            .observed_at
            .checked_sub(self.policy.retention_seconds)
            .ok_or(PublisherError::Store)?;
        transaction
            .execute(
                "UPDATE receipts
                    SET request_body = NULL, response_body = NULL, retired_at = ?1
                  WHERE request_identity IN (
                    SELECT request_identity FROM receipts
                     WHERE retired_at IS NULL AND issued_at <= ?2
                     ORDER BY issued_at, request_identity
                     LIMIT 64
                  )",
                params![input.observed_at, retire_before],
            )
            .map_err(|_| PublisherError::Store)?;
        let replay = transaction
            .query_row(
                "SELECT request_identity, request_sha256, request_body, response_body
                   FROM receipts WHERE replay_identity = ?1",
                [input.replay_identity],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<Vec<u8>>>(2)?,
                        row.get::<_, Option<Vec<u8>>>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| PublisherError::Store)?;
        if let Some((stored_identity, stored_sha256, stored_body, response)) = replay {
            if stored_identity != input.request_identity || stored_sha256 != input.request_sha256 {
                return Err(PublisherError::ReplayConflict);
            }
            let (Some(stored_body), Some(response)) = (stored_body, response) else {
                return Err(PublisherError::ReplayConflict);
            };
            if stored_body != input.request_body {
                return Err(PublisherError::ReplayConflict);
            }
            transaction.commit().map_err(|_| PublisherError::Store)?;
            return Ok(response);
        }

        let existing_identity: Option<String> = transaction
            .query_row(
                "SELECT replay_identity FROM receipts
                  WHERE request_identity = ?1 OR request_sha256 = ?2
                  LIMIT 1",
                params![input.request_identity, input.request_sha256],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| PublisherError::Store)?;
        if existing_identity.is_some() {
            return Err(PublisherError::ReplayConflict);
        }
        let count: u64 = transaction
            .query_row("SELECT count(*) FROM receipts", [], |row| row.get(0))
            .map_err(|_| PublisherError::Store)?;
        if count >= self.policy.max_receipts {
            return Err(PublisherError::Store);
        }

        let ReceiptArtifact {
            evidence_identity,
            response,
        } = build_artifact(
            input.request,
            input.request_identity,
            input.request_sha256,
            input.issued_at,
            input.signature_domain,
            input.signer,
        )?;
        transaction
            .execute(
                "INSERT INTO receipts (
                     request_identity, replay_identity, request_sha256, request_body,
                     evidence_identity, response_body, issued_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    input.request_identity,
                    input.replay_identity,
                    input.request_sha256,
                    input.request_body,
                    evidence_identity,
                    response,
                    input.issued_at,
                ],
            )
            .map_err(|error| {
                if error
                    .sqlite_error_code()
                    .is_some_and(|code| code == rusqlite::ErrorCode::ConstraintViolation)
                {
                    PublisherError::ReplayConflict
                } else {
                    PublisherError::Store
                }
            })?;
        transaction.commit().map_err(|_| PublisherError::Store)?;
        self.connection
            .execute_batch("PRAGMA wal_checkpoint(PASSIVE);")
            .map_err(|_| PublisherError::Store)?;
        Ok(response)
    }

    #[cfg(test)]
    pub(crate) fn count(&self) -> usize {
        self.connection
            .query_row("SELECT count(*) FROM receipts", [], |row| row.get(0))
            .unwrap()
    }

    #[cfg(test)]
    pub(crate) fn retired_count(&self) -> usize {
        self.connection
            .query_row(
                "SELECT count(*) FROM receipts WHERE retired_at IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[cfg(test)]
    pub(crate) fn break_for_test(&self) {
        self.connection
            .execute_batch("DROP TABLE receipts;")
            .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::sync::{Arc, Barrier};
    use std::thread;

    use super::*;
    use crate::receipt::{TestSigner, raw_request_sha256, request_identity};
    use crate::test_support::{fixture, secure_tempdir};

    fn issue(
        store: &mut DurableStore,
        replay: &str,
        body: &[u8],
        signer: &dyn ReceiptSigner,
    ) -> Result<Vec<u8>, PublisherError> {
        issue_at(store, replay, body, signer, 1_800_000_000, 1_800_000_000)
    }

    fn issue_at(
        store: &mut DurableStore,
        replay: &str,
        body: &[u8],
        signer: &dyn ReceiptSigner,
        issued_at: i64,
        observed_at: i64,
    ) -> Result<Vec<u8>, PublisherError> {
        let fixture = fixture();
        let identity = request_identity(body);
        let sha256 = raw_request_sha256(body);
        store.issue(IssueInput {
            replay_identity: replay,
            request_identity: &identity,
            request_sha256: &sha256,
            request_body: body,
            request: &fixture.request,
            issued_at,
            observed_at,
            signature_domain: "test",
            signer,
        })
    }

    #[test]
    fn duplicate_is_stable_and_conflicting_replay_rejects() {
        let temp = secure_tempdir();
        let mut store = DurableStore::open(&temp.path().join("publisher.db")).unwrap();
        let signer = TestSigner::new("test-publisher-v1");
        let body = fixture().request_body;
        let first = issue(&mut store, "jti-1", &body, &signer).unwrap();
        let duplicate = issue(&mut store, "jti-1", &body, &signer).unwrap();
        assert_eq!(first, duplicate);
        assert_eq!(store.count(), 1);
        let mut changed = body.clone();
        changed.push(b' ');
        assert!(matches!(
            issue(&mut store, "jti-1", &changed, &signer),
            Err(PublisherError::ReplayConflict)
        ));
        assert_eq!(store.count(), 1);
    }

    #[test]
    fn signing_failure_rolls_back() {
        let temp = secure_tempdir();
        let mut store = DurableStore::open(&temp.path().join("publisher.db")).unwrap();
        let body = fixture().request_body;
        assert!(matches!(
            issue(
                &mut store,
                "jti-fail",
                &body,
                &TestSigner::failing("test-publisher-v1")
            ),
            Err(PublisherError::Signing)
        ));
        assert_eq!(store.count(), 0);
        assert!(
            issue(
                &mut store,
                "jti-fail",
                &body,
                &TestSigner::new("test-publisher-v1")
            )
            .is_ok()
        );
    }

    #[test]
    fn concurrent_duplicates_return_identical_committed_bytes() {
        const THREADS: usize = 16;
        let temp = secure_tempdir();
        let path = Arc::new(temp.path().join("publisher.db"));
        DurableStore::open(path.as_path()).unwrap();
        let barrier = Arc::new(Barrier::new(THREADS));
        let body = Arc::new(fixture().request_body);
        let handles = (0..THREADS)
            .map(|_| {
                let path = path.clone();
                let barrier = barrier.clone();
                let body = body.clone();
                thread::spawn(move || {
                    let mut store = DurableStore::open(path.as_path()).unwrap();
                    barrier.wait();
                    issue(
                        &mut store,
                        "same-jti",
                        &body,
                        &TestSigner::new("test-publisher-v1"),
                    )
                    .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let responses = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert!(responses.windows(2).all(|pair| pair[0] == pair[1]));
        assert_eq!(DurableStore::open(path.as_path()).unwrap().count(), 1);
    }

    #[test]
    fn key_rotation_keeps_old_receipt_and_uses_new_identity() {
        use base64::Engine;
        let temp = secure_tempdir();
        let mut store = DurableStore::open(&temp.path().join("publisher.db")).unwrap();
        let first_body = fixture().request_body;
        let first = issue(
            &mut store,
            "jti-v1",
            &first_body,
            &TestSigner::new("publisher-v1"),
        )
        .unwrap();
        let mut second_body = first_body.clone();
        second_body.extend_from_slice(b"rotation");
        let second = issue(
            &mut store,
            "jti-v2",
            &second_body,
            &TestSigner::new("publisher-v2"),
        )
        .unwrap();
        let decode_receipt = |response: Vec<u8>| {
            let response: serde_json::Value = serde_json::from_slice(&response).unwrap();
            String::from_utf8(
                base64::engine::general_purpose::STANDARD
                    .decode(response["publisher_receipt_base64"].as_str().unwrap())
                    .unwrap(),
            )
            .unwrap()
        };
        assert!(decode_receipt(first).contains("\nsigning_key_id\tpublisher-v1\n"));
        assert!(decode_receipt(second).contains("\nsigning_key_id\tpublisher-v2\n"));
        assert_eq!(store.count(), 2);
    }

    #[test]
    fn retention_leaves_permanent_tombstones_and_capacity_fails_closed() {
        let temp = secure_tempdir();
        let policy = StorePolicy {
            max_receipts: 2,
            max_database_bytes: 4 * 1024 * 1024,
            retention_seconds: RECEIPT_LIFETIME_SECS,
            busy_timeout: Duration::from_millis(100),
        };
        let mut store =
            DurableStore::open_with_policy(&temp.path().join("publisher.db"), policy).unwrap();
        let signer = TestSigner::new("test-publisher-v1");
        let first_body = fixture().request_body;
        let first = issue_at(
            &mut store,
            "jti-first",
            &first_body,
            &signer,
            1_800_000_000,
            1_800_000_000,
        )
        .unwrap();
        let mut second_body = first_body.clone();
        second_body.extend_from_slice(b"second");
        let retire_time = 1_800_000_000 + RECEIPT_LIFETIME_SECS + 1;
        let second = issue_at(
            &mut store,
            "jti-second",
            &second_body,
            &signer,
            retire_time,
            retire_time,
        )
        .unwrap();
        assert_ne!(first, second);
        assert_eq!(store.count(), 2);
        assert_eq!(store.retired_count(), 1);

        assert!(matches!(
            issue_at(
                &mut store,
                "jti-first",
                &first_body,
                &signer,
                retire_time,
                retire_time,
            ),
            Err(PublisherError::ReplayConflict)
        ));
        assert!(matches!(
            issue_at(
                &mut store,
                "new-jti-same-request",
                &first_body,
                &signer,
                retire_time,
                retire_time,
            ),
            Err(PublisherError::ReplayConflict)
        ));

        let mut third_body = first_body;
        third_body.extend_from_slice(b"third");
        assert!(matches!(
            issue_at(
                &mut store,
                "jti-third",
                &third_body,
                &signer,
                retire_time,
                retire_time,
            ),
            Err(PublisherError::Store)
        ));
        assert_eq!(
            issue_at(
                &mut store,
                "jti-second",
                &second_body,
                &signer,
                retire_time,
                retire_time,
            )
            .unwrap(),
            second
        );
    }

    #[test]
    fn database_page_limit_fails_closed_and_keeps_committed_replays() {
        let temp = secure_tempdir();
        let policy = StorePolicy {
            max_receipts: 1000,
            max_database_bytes: MIN_DATABASE_BYTES,
            retention_seconds: 24 * 60 * 60,
            busy_timeout: Duration::from_millis(100),
        };
        let path = temp.path().join("publisher.db");
        let mut store = DurableStore::open_with_policy(&path, policy).unwrap();
        let signer = TestSigner::new("test-publisher-v1");
        let first_body = fixture().request_body;
        let first = issue(&mut store, "jti-page-first", &first_body, &signer).unwrap();
        let mut failed_closed = false;
        for index in 0..128 {
            let mut body = vec![b'x'; 60 * 1024];
            body.extend_from_slice(format!("{index:08x}").as_bytes());
            if matches!(
                issue(&mut store, &format!("jti-page-{index}"), &body, &signer),
                Err(PublisherError::Store)
            ) {
                failed_closed = true;
                break;
            }
        }
        assert!(
            failed_closed,
            "configured SQLite page ceiling was not reached"
        );
        assert_eq!(
            issue(&mut store, "jti-page-first", &first_body, &signer).unwrap(),
            first
        );
    }

    #[test]
    fn sqlite_busy_wait_is_bounded() {
        let temp = secure_tempdir();
        let policy = StorePolicy {
            max_receipts: 16,
            max_database_bytes: 4 * 1024 * 1024,
            retention_seconds: RECEIPT_LIFETIME_SECS,
            busy_timeout: Duration::from_millis(25),
        };
        let path = temp.path().join("publisher.db");
        let locker = DurableStore::open_with_policy(&path, policy).unwrap();
        let mut contender = DurableStore::open_with_policy(&path, policy).unwrap();
        locker.connection.execute_batch("BEGIN IMMEDIATE;").unwrap();
        let start = std::time::Instant::now();
        assert!(matches!(
            issue(
                &mut contender,
                "jti-busy",
                &fixture().request_body,
                &TestSigner::new("test-publisher-v1")
            ),
            Err(PublisherError::Store)
        ));
        assert!(start.elapsed() < Duration::from_millis(500));
        locker.connection.execute_batch("ROLLBACK;").unwrap();
    }

    #[test]
    fn schema_v1_migrates_transactionally_to_tombstone_schema() {
        let temp = secure_tempdir();
        let path = temp.path().join("publisher.db");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE receipts (
                   request_identity TEXT PRIMARY KEY NOT NULL CHECK(length(request_identity) = 64),
                   replay_identity TEXT UNIQUE NOT NULL CHECK(length(replay_identity) BETWEEN 1 AND 4096),
                   request_sha256 TEXT UNIQUE NOT NULL CHECK(length(request_sha256) = 64),
                   request_body BLOB NOT NULL CHECK(length(request_body) <= 65536),
                   evidence_identity TEXT UNIQUE NOT NULL CHECK(length(evidence_identity) = 64),
                   response_body BLOB NOT NULL CHECK(length(response_body) <= 524288),
                   issued_at INTEGER NOT NULL CHECK(issued_at > 0)
                 ) STRICT;
                 PRAGMA user_version=1;",
            )
            .unwrap();
        drop(connection);

        let store = DurableStore::open_with_policy(&path, StorePolicy::test_default()).unwrap();
        let version: i64 = store
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 2);
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn crash_before_commit_recovers_without_partial_row() {
        let temp = secure_tempdir();
        let path = temp.path().join("publisher.db");
        DurableStore::open(&path).unwrap();
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "store::tests::crash_child", "--nocapture"])
            .env("FE2O3_TEST_CRASH_DB", &path)
            .status()
            .unwrap();
        assert!(!status.success());
        assert_eq!(DurableStore::open(&path).unwrap().count(), 0);
    }

    #[test]
    fn crash_child() {
        let Some(path) = std::env::var_os("FE2O3_TEST_CRASH_DB") else {
            return;
        };
        let store = DurableStore::open(Path::new(&path)).unwrap();
        store
            .connection
            .execute_batch(
                "BEGIN IMMEDIATE;
             INSERT INTO receipts (
               request_identity, replay_identity, request_sha256, request_body,
               evidence_identity, response_body, issued_at, retired_at
             ) VALUES (
               'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
               'crash-jti',
               'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
               X'7B7D0A',
               'cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc',
               X'7B7D0A',
               1800000000,
               NULL
             );",
            )
            .unwrap();
        std::process::abort();
    }
}
