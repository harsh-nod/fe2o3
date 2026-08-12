use std::fs::Permissions;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};

use crate::PublisherError;
use crate::oidc::PublisherRequest;
use crate::receipt::{ReceiptArtifact, ReceiptSigner, build_artifact};

pub struct DurableStore {
    connection: Connection,
}

pub(crate) struct IssueInput<'a> {
    pub replay_identity: &'a str,
    pub request_identity: &'a str,
    pub request_sha256: &'a str,
    pub request_body: &'a [u8],
    pub request: &'a PublisherRequest,
    pub issued_at: i64,
    pub signature_domain: &'a str,
    pub signer: &'a dyn ReceiptSigner,
}

impl DurableStore {
    pub fn open(path: &Path) -> Result<Self, PublisherError> {
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
                 PRAGMA trusted_schema=OFF;
                 PRAGMA busy_timeout=10000;",
            )
            .map_err(|_| PublisherError::Store)?;
        let mut store = Self { connection };
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
                         request_body BLOB NOT NULL CHECK(length(request_body) <= 65536),
                         evidence_identity TEXT UNIQUE NOT NULL CHECK(length(evidence_identity) = 64),
                         response_body BLOB NOT NULL CHECK(length(response_body) <= 524288),
                         issued_at INTEGER NOT NULL CHECK(issued_at > 0)
                     ) STRICT;
                     PRAGMA user_version=1;",
                )
                .map_err(|_| PublisherError::Store)?;
        } else if version != 1 {
            return Err(PublisherError::Store);
        }
        transaction.commit().map_err(|_| PublisherError::Store)
    }

    pub(crate) fn issue(&mut self, input: IssueInput<'_>) -> Result<Vec<u8>, PublisherError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
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
                        row.get::<_, Vec<u8>>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| PublisherError::Store)?;
        if let Some((stored_identity, stored_sha256, stored_body, response)) = replay {
            if stored_identity != input.request_identity
                || stored_sha256 != input.request_sha256
                || stored_body != input.request_body
            {
                return Err(PublisherError::ReplayConflict);
            }
            transaction.commit().map_err(|_| PublisherError::Store)?;
            return Ok(response);
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
        let fixture = fixture();
        let identity = request_identity(body);
        let sha256 = raw_request_sha256(body);
        store.issue(IssueInput {
            replay_identity: replay,
            request_identity: &identity,
            request_sha256: &sha256,
            request_body: body,
            request: &fixture.request,
            issued_at: 1_800_000_000,
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
             INSERT INTO receipts VALUES (
               'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
               'crash-jti',
               'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
               X'7B7D0A',
               'cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc',
               X'7B7D0A',
               1800000000
             );",
            )
            .unwrap();
        std::process::abort();
    }
}
