use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;
use std::path::Path;
use std::time::Instant;

#[cfg(test)]
use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::PublisherError;
use crate::ServiceConfig;
use crate::bounds::{
    MAX_LEDGER_BYTES, MAX_LEDGER_RECORD_BYTES, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES,
    MAX_STORE_RECEIPTS, MIN_LEDGER_BYTES,
};
use crate::canonical::{canonical_bytes, parse_canonical};
use crate::oidc::PublisherRequest;
use crate::receipt::{
    ReceiptArtifact, ReceiptSigner, build_artifact, raw_request_sha256, request_identity,
};
use crate::secure_fs::{FileIdentity, SecureLocation};

const LEDGER_MAGIC: &[u8] = b"fe2o3-protected-publisher-ledger-v1\0";
const FRAME_MAGIC: &[u8; 8] = b"F2O3REC1";
const FRAME_VERSION: u32 = 1;
const FRAME_PREFIX_BYTES: usize = 8 + 4 + 4 + 8 + 32;
const FRAME_HASH_BYTES: usize = 32;
const ZERO_HASH: [u8; 32] = [0; 32];

pub struct DurableStore {
    file: File,
    policy: StorePolicy,
    location: SecureLocation,
    identity: FileIdentity,
    header_len: u64,
    tail_offset: u64,
    next_sequence: u64,
    tail_hash: [u8; 32],
    by_request_key: HashMap<String, EntryIndex>,
    request_identities: HashSet<String>,
    request_digests: HashSet<String>,
    evidence_identities: HashSet<String>,
    poisoned: bool,
    #[cfg(test)]
    commit_delay: Duration,
    #[cfg(test)]
    maximum_write_chunk: usize,
    #[cfg(test)]
    fail_write_after: Option<usize>,
    #[cfg(test)]
    after_sync: Option<Box<dyn FnOnce() + Send>>,
}

#[derive(Clone)]
struct EntryIndex {
    frame_offset: u64,
    frame_length: u64,
    request_identity: String,
    request_sha256: String,
    stable_authorization_sha256: String,
}

#[derive(Clone)]
pub(crate) struct StorePolicy {
    max_receipts: u64,
    max_ledger_bytes: u64,
    ledger_domain: String,
}

impl StorePolicy {
    pub(crate) fn from_config(config: &ServiceConfig) -> Result<Self, PublisherError> {
        let policy = Self {
            max_receipts: config.max_receipts,
            max_ledger_bytes: config.max_ledger_bytes,
            ledger_domain: config.service_identity()?,
        };
        policy.validate()?;
        Ok(policy)
    }

    fn validate(&self) -> Result<(), PublisherError> {
        if self.max_receipts == 0
            || self.max_receipts > MAX_STORE_RECEIPTS
            || !(MIN_LEDGER_BYTES..=MAX_LEDGER_BYTES).contains(&self.max_ledger_bytes)
            || self.ledger_domain.len() != 64
            || !self.ledger_domain.is_ascii()
        {
            return Err(PublisherError::Config);
        }
        Ok(())
    }

    #[cfg(test)]
    fn test_default() -> Self {
        Self {
            max_receipts: 4096,
            max_ledger_bytes: 64 * 1024 * 1024,
            ledger_domain: format!("t{}", "1".repeat(63)),
        }
    }
}

pub(crate) struct IssueInput<'a> {
    pub request_key_sha256: &'a str,
    pub stable_authorization_sha256: &'a str,
    pub request_identity: &'a str,
    pub request_sha256: &'a str,
    pub request_body: &'a [u8],
    pub request: &'a PublisherRequest,
    pub issued_at: i64,
    pub signature_domain: &'a str,
    pub signer: &'a dyn ReceiptSigner,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LedgerRecord {
    evidence_identity: String,
    issued_at: i64,
    record_domain: String,
    request_body_base64: String,
    request_identity: String,
    request_key_sha256: String,
    request_sha256: String,
    response_body_base64: String,
    schema_version: u32,
    stable_authorization_sha256: String,
}

struct DecodedFrame {
    frame_hash: [u8; 32],
    frame_length: u64,
    record: LedgerRecord,
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
        let location = SecureLocation::open(path)?;
        let (mut file, identity, created) = location.open_or_create_ledger()?;
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(PublisherError::Store);
        }
        let header = ledger_header(&policy);
        if created {
            if identity.size != 0 {
                return Err(PublisherError::Store);
            }
            file.write_all(&header).map_err(|_| PublisherError::Store)?;
            file.sync_data().map_err(|_| PublisherError::Store)?;
            location.sync()?;
        } else if identity.size == 0 {
            return Err(PublisherError::Store);
        }

        let mut store = Self {
            file,
            policy,
            location,
            identity,
            header_len: header.len() as u64,
            tail_offset: header.len() as u64,
            next_sequence: 1,
            tail_hash: ZERO_HASH,
            by_request_key: HashMap::new(),
            request_identities: HashSet::new(),
            request_digests: HashSet::new(),
            evidence_identities: HashSet::new(),
            poisoned: false,
            #[cfg(test)]
            commit_delay: Duration::ZERO,
            #[cfg(test)]
            maximum_write_chunk: usize::MAX,
            #[cfg(test)]
            fail_write_after: None,
            #[cfg(test)]
            after_sync: None,
        };
        store.verify_identity()?;
        store.replay(&header)?;
        store.verify_identity()?;
        Ok(store)
    }

    fn replay(&mut self, expected_header: &[u8]) -> Result<(), PublisherError> {
        let length = self
            .file
            .metadata()
            .map_err(|_| PublisherError::Store)?
            .len();
        if length > self.policy.max_ledger_bytes || length < self.header_len {
            return Err(PublisherError::Store);
        }
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|_| PublisherError::Store)?;
        let mut header = vec![0; expected_header.len()];
        self.file
            .read_exact(&mut header)
            .map_err(|_| PublisherError::Store)?;
        if header != expected_header {
            return Err(PublisherError::Store);
        }

        let mut offset = self.header_len;
        while offset < length {
            let remaining = length - offset;
            if remaining < FRAME_PREFIX_BYTES as u64 {
                self.recover_torn_tail(offset)?;
                break;
            }
            let decoded = match self.read_frame(offset, remaining)? {
                Some(frame) => frame,
                None => {
                    self.recover_torn_tail(offset)?;
                    break;
                }
            };
            self.index_record(offset, decoded.frame_length, &decoded.record)?;
            self.tail_hash = decoded.frame_hash;
            self.next_sequence = self
                .next_sequence
                .checked_add(1)
                .ok_or(PublisherError::Store)?;
            offset = offset
                .checked_add(decoded.frame_length)
                .ok_or(PublisherError::Store)?;
            self.tail_offset = offset;
        }
        Ok(())
    }

    fn recover_torn_tail(&mut self, offset: u64) -> Result<(), PublisherError> {
        self.file
            .set_len(offset)
            .map_err(|_| PublisherError::Store)?;
        self.file.sync_data().map_err(|_| PublisherError::Store)?;
        self.tail_offset = offset;
        Ok(())
    }

    fn read_frame(
        &mut self,
        offset: u64,
        remaining: u64,
    ) -> Result<Option<DecodedFrame>, PublisherError> {
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(|_| PublisherError::Store)?;
        let mut prefix = [0u8; FRAME_PREFIX_BYTES];
        self.file
            .read_exact(&mut prefix)
            .map_err(|_| PublisherError::Store)?;
        if &prefix[..8] != FRAME_MAGIC {
            return Err(PublisherError::Store);
        }
        let version = u32::from_be_bytes(prefix[8..12].try_into().unwrap());
        let payload_length = u32::from_be_bytes(prefix[12..16].try_into().unwrap()) as usize;
        let sequence = u64::from_be_bytes(prefix[16..24].try_into().unwrap());
        let previous_hash: [u8; 32] = prefix[24..56].try_into().unwrap();
        if version != FRAME_VERSION
            || payload_length == 0
            || payload_length > MAX_LEDGER_RECORD_BYTES
            || sequence != self.next_sequence
            || previous_hash != self.tail_hash
        {
            return Err(PublisherError::Store);
        }
        let frame_length = FRAME_PREFIX_BYTES
            .checked_add(payload_length)
            .and_then(|value| value.checked_add(FRAME_HASH_BYTES))
            .ok_or(PublisherError::Store)? as u64;
        if remaining < frame_length {
            return Ok(None);
        }
        let mut payload = vec![0; payload_length];
        self.file
            .read_exact(&mut payload)
            .map_err(|_| PublisherError::Store)?;
        let mut stored_hash = [0u8; 32];
        self.file
            .read_exact(&mut stored_hash)
            .map_err(|_| PublisherError::Store)?;
        let computed_hash = frame_hash(&prefix, &payload);
        if stored_hash != computed_hash {
            return Err(PublisherError::Store);
        }
        let record = decode_record(&payload)?;
        Ok(Some(DecodedFrame {
            frame_hash: computed_hash,
            frame_length,
            record,
        }))
    }

    fn index_record(
        &mut self,
        frame_offset: u64,
        frame_length: u64,
        record: &LedgerRecord,
    ) -> Result<(), PublisherError> {
        if self.by_request_key.len() as u64 >= self.policy.max_receipts
            || self.by_request_key.contains_key(&record.request_key_sha256)
            || !self
                .request_identities
                .insert(record.request_identity.clone())
            || !self.request_digests.insert(record.request_sha256.clone())
            || !self
                .evidence_identities
                .insert(record.evidence_identity.clone())
        {
            return Err(PublisherError::Store);
        }
        self.by_request_key.insert(
            record.request_key_sha256.clone(),
            EntryIndex {
                frame_offset,
                frame_length,
                request_identity: record.request_identity.clone(),
                request_sha256: record.request_sha256.clone(),
                stable_authorization_sha256: record.stable_authorization_sha256.clone(),
            },
        );
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn issue(&mut self, input: IssueInput<'_>) -> Result<Vec<u8>, PublisherError> {
        self.issue_until(input, Instant::now() + Duration::from_secs(60))
    }

    pub(crate) fn issue_until(
        &mut self,
        input: IssueInput<'_>,
        deadline: Instant,
    ) -> Result<Vec<u8>, PublisherError> {
        if self.poisoned {
            return Err(PublisherError::Store);
        }
        check_deadline(deadline)?;
        self.verify_identity()?;
        validate_issue_input(&input)?;
        check_deadline(deadline)?;

        if let Some(existing) = self.by_request_key.get(input.request_key_sha256).cloned() {
            if existing.request_identity != input.request_identity
                || existing.request_sha256 != input.request_sha256
                || existing.stable_authorization_sha256 != input.stable_authorization_sha256
            {
                return Err(PublisherError::ReplayConflict);
            }
            let record = self.load_indexed(&existing)?;
            let request_body = decode_base64(&record.request_body_base64, MAX_REQUEST_BYTES)?;
            if request_body != input.request_body {
                return Err(PublisherError::ReplayConflict);
            }
            check_deadline(deadline)?;
            return decode_base64(&record.response_body_base64, MAX_RESPONSE_BYTES);
        }
        if self.request_identities.contains(input.request_identity)
            || self.request_digests.contains(input.request_sha256)
            || self.by_request_key.len() as u64 >= self.policy.max_receipts
        {
            return Err(PublisherError::ReplayConflict);
        }
        check_deadline(deadline)?;

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
        let record = LedgerRecord {
            evidence_identity,
            issued_at: input.issued_at,
            record_domain: "fe2o3-protected-publisher-ledger-record-v1".into(),
            request_body_base64: base64::engine::general_purpose::STANDARD
                .encode(input.request_body),
            request_identity: input.request_identity.into(),
            request_key_sha256: input.request_key_sha256.into(),
            request_sha256: input.request_sha256.into(),
            response_body_base64: base64::engine::general_purpose::STANDARD.encode(&response),
            schema_version: 1,
            stable_authorization_sha256: input.stable_authorization_sha256.into(),
        };
        validate_record(&record)?;
        if self.evidence_identities.contains(&record.evidence_identity) {
            return Err(PublisherError::ReplayConflict);
        }
        let payload =
            canonical_bytes(&serde_json::to_value(&record).map_err(|_| PublisherError::Store)?)
                .map_err(|_| PublisherError::Store)?;
        if payload.len() > MAX_LEDGER_RECORD_BYTES {
            return Err(PublisherError::Store);
        }
        let frame = encode_frame(self.next_sequence, self.tail_hash, &payload)?;
        let new_tail = self
            .tail_offset
            .checked_add(frame.len() as u64)
            .filter(|tail| *tail <= self.policy.max_ledger_bytes)
            .ok_or(PublisherError::Store)?;
        check_deadline(deadline)?;
        self.verify_identity()?;

        // Admission ends here. Synchronous writes and fdatasync are not cancellable.
        #[cfg(test)]
        std::thread::sleep(self.commit_delay);
        if self.append_authoritatively(&frame).is_err() {
            self.poisoned = true;
            return Err(PublisherError::Store);
        }
        let frame_hash: [u8; 32] = frame[frame.len() - FRAME_HASH_BYTES..].try_into().unwrap();
        let frame_offset = self.tail_offset;
        self.tail_offset = new_tail;
        self.tail_hash = frame_hash;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(PublisherError::Store)?;
        if self
            .index_record(frame_offset, frame.len() as u64, &record)
            .is_err()
        {
            self.poisoned = true;
            return Err(PublisherError::Store);
        }
        Ok(response)
    }

    fn append_authoritatively(&mut self, frame: &[u8]) -> Result<(), PublisherError> {
        self.file
            .seek(SeekFrom::Start(self.tail_offset))
            .map_err(|_| PublisherError::Store)?;
        let mut written = 0usize;
        while written < frame.len() {
            #[cfg(test)]
            if self
                .fail_write_after
                .is_some_and(|threshold| written >= threshold)
            {
                return Err(PublisherError::Store);
            }
            #[cfg(test)]
            let end = frame
                .len()
                .min(written.saturating_add(self.maximum_write_chunk));
            #[cfg(not(test))]
            let end = frame.len();
            let count = self
                .file
                .write(&frame[written..end])
                .map_err(|_| PublisherError::Store)?;
            if count == 0 {
                return Err(PublisherError::Store);
            }
            written = written.checked_add(count).ok_or(PublisherError::Store)?;
        }
        self.file.sync_data().map_err(|_| PublisherError::Store)?;
        #[cfg(test)]
        if let Some(hook) = self.after_sync.take() {
            hook();
        }
        self.verify_identity()
    }

    fn load_indexed(&mut self, index: &EntryIndex) -> Result<LedgerRecord, PublisherError> {
        let length = self
            .file
            .metadata()
            .map_err(|_| PublisherError::Store)?
            .len();
        if index
            .frame_offset
            .checked_add(index.frame_length)
            .is_none_or(|end| end > length)
        {
            return Err(PublisherError::Store);
        }
        let saved_sequence = self.next_sequence;
        let saved_hash = self.tail_hash;
        self.next_sequence = u64::from_be_bytes({
            self.file
                .seek(SeekFrom::Start(index.frame_offset + 16))
                .map_err(|_| PublisherError::Store)?;
            let mut value = [0u8; 8];
            self.file
                .read_exact(&mut value)
                .map_err(|_| PublisherError::Store)?;
            value
        });
        self.file
            .seek(SeekFrom::Start(index.frame_offset + 24))
            .map_err(|_| PublisherError::Store)?;
        self.file
            .read_exact(&mut self.tail_hash)
            .map_err(|_| PublisherError::Store)?;
        let decoded = self.read_frame(index.frame_offset, index.frame_length);
        self.next_sequence = saved_sequence;
        self.tail_hash = saved_hash;
        decoded?
            .ok_or(PublisherError::Store)
            .map(|frame| frame.record)
    }

    fn verify_identity(&self) -> Result<(), PublisherError> {
        self.location
            .verify_ledger_entry(self.identity)
            .map_err(|_| PublisherError::Store)?;
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        if unsafe { libc::fstat(self.file.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
            return Err(PublisherError::Store);
        }
        let stat = unsafe { stat.assume_init() };
        if stat.st_dev != self.identity.dev
            || stat.st_ino != self.identity.ino
            || stat.st_mode != self.identity.mode
            || stat.st_uid != self.identity.uid
            || stat.st_gid != self.identity.gid
            || stat.st_nlink != 1
        {
            return Err(PublisherError::Store);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn count(&self) -> usize {
        self.by_request_key.len()
    }

    #[cfg(test)]
    pub(crate) fn break_for_test(&mut self) {
        self.poisoned = true;
    }

    #[cfg(test)]
    pub(crate) fn set_commit_delay(&mut self, delay: Duration) {
        self.commit_delay = delay;
    }

    #[cfg(test)]
    fn set_maximum_write_chunk(&mut self, bytes: usize) {
        self.maximum_write_chunk = bytes.max(1);
    }

    #[cfg(test)]
    fn fail_write_after(&mut self, bytes: usize) {
        self.fail_write_after = Some(bytes);
    }

    #[cfg(test)]
    fn set_after_sync(&mut self, hook: impl FnOnce() + Send + 'static) {
        self.after_sync = Some(Box::new(hook));
    }
}

fn ledger_header(policy: &StorePolicy) -> Vec<u8> {
    let mut header = Vec::with_capacity(LEDGER_MAGIC.len() + policy.ledger_domain.len() + 1);
    header.extend_from_slice(LEDGER_MAGIC);
    header.extend_from_slice(policy.ledger_domain.as_bytes());
    header.push(b'\n');
    header
}

fn encode_frame(
    sequence: u64,
    previous_hash: [u8; 32],
    payload: &[u8],
) -> Result<Vec<u8>, PublisherError> {
    let payload_length = u32::try_from(payload.len()).map_err(|_| PublisherError::Store)?;
    let mut prefix = Vec::with_capacity(FRAME_PREFIX_BYTES);
    prefix.extend_from_slice(FRAME_MAGIC);
    prefix.extend_from_slice(&FRAME_VERSION.to_be_bytes());
    prefix.extend_from_slice(&payload_length.to_be_bytes());
    prefix.extend_from_slice(&sequence.to_be_bytes());
    prefix.extend_from_slice(&previous_hash);
    let hash = frame_hash(&prefix, payload);
    let mut frame = Vec::with_capacity(prefix.len() + payload.len() + hash.len());
    frame.extend_from_slice(&prefix);
    frame.extend_from_slice(payload);
    frame.extend_from_slice(&hash);
    Ok(frame)
}

fn frame_hash(prefix: &[u8], payload: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"fe2o3-protected-publisher-ledger-frame-v1\0");
    hash.update(prefix);
    hash.update(payload);
    hash.finalize().into()
}

fn decode_record(payload: &[u8]) -> Result<LedgerRecord, PublisherError> {
    let value =
        parse_canonical(payload, MAX_LEDGER_RECORD_BYTES).map_err(|_| PublisherError::Store)?;
    let record: LedgerRecord = serde_json::from_value(value).map_err(|_| PublisherError::Store)?;
    validate_record(&record)?;
    Ok(record)
}

fn validate_issue_input(input: &IssueInput<'_>) -> Result<(), PublisherError> {
    if !is_digest(input.request_key_sha256)
        || !is_digest(input.stable_authorization_sha256)
        || !is_digest(input.request_identity)
        || !is_digest(input.request_sha256)
        || input.request_body.len() > MAX_REQUEST_BYTES
        || request_identity(input.request_body) != input.request_identity
        || raw_request_sha256(input.request_body) != input.request_sha256
    {
        return Err(PublisherError::Store);
    }
    let request_value = parse_canonical(input.request_body, MAX_REQUEST_BYTES)
        .map_err(|_| PublisherError::Store)?;
    let parsed: PublisherRequest =
        serde_json::from_value(request_value).map_err(|_| PublisherError::Store)?;
    let parsed_bytes =
        canonical_bytes(&serde_json::to_value(&parsed).map_err(|_| PublisherError::Store)?)
            .map_err(|_| PublisherError::Store)?;
    let projection =
        canonical_bytes(&parsed.oidc_authorization).map_err(|_| PublisherError::Store)?;
    if parsed_bytes != input.request_body
        || sha256_hex(&projection) != input.stable_authorization_sha256
    {
        return Err(PublisherError::Store);
    }
    Ok(())
}

fn validate_record(record: &LedgerRecord) -> Result<(), PublisherError> {
    if record.schema_version != 1
        || record.record_domain != "fe2o3-protected-publisher-ledger-record-v1"
        || record.issued_at <= 0
        || !is_digest(&record.evidence_identity)
        || !is_digest(&record.request_identity)
        || !is_digest(&record.request_key_sha256)
        || !is_digest(&record.request_sha256)
        || !is_digest(&record.stable_authorization_sha256)
    {
        return Err(PublisherError::Store);
    }
    let request = decode_base64(&record.request_body_base64, MAX_REQUEST_BYTES)?;
    let response = decode_base64(&record.response_body_base64, MAX_RESPONSE_BYTES)?;
    if request_identity(&request) != record.request_identity
        || raw_request_sha256(&request) != record.request_sha256
    {
        return Err(PublisherError::Store);
    }
    let request_value =
        parse_canonical(&request, MAX_REQUEST_BYTES).map_err(|_| PublisherError::Store)?;
    let request: PublisherRequest =
        serde_json::from_value(request_value).map_err(|_| PublisherError::Store)?;
    let projection =
        canonical_bytes(&request.oidc_authorization).map_err(|_| PublisherError::Store)?;
    if sha256_hex(&projection) != record.stable_authorization_sha256 {
        return Err(PublisherError::Store);
    }
    let response_value =
        parse_canonical(&response, MAX_RESPONSE_BYTES).map_err(|_| PublisherError::Store)?;
    let response_object = response_value.as_object().ok_or(PublisherError::Store)?;
    if response_object.len() != 4
        || response_object
            .get("schema_version")
            .and_then(Value::as_u64)
            != Some(1)
        || response_object
            .get("request_sha256")
            .and_then(Value::as_str)
            != Some(&record.request_sha256)
    {
        return Err(PublisherError::Store);
    }
    let receipt = response_object
        .get("publisher_receipt_base64")
        .and_then(Value::as_str)
        .ok_or(PublisherError::Store)?;
    let receipt = decode_base64(receipt, crate::bounds::MAX_RECEIPT_BYTES)?;
    if domain_hash(
        b"fe2o3-protected-publisher-evidence-identity-v1\0",
        &receipt,
    ) != record.evidence_identity
    {
        return Err(PublisherError::Store);
    }
    Ok(())
}

fn decode_base64(value: &str, limit: usize) -> Result<Vec<u8>, PublisherError> {
    if value.len() > limit.saturating_mul(4).div_ceil(3).saturating_add(4) {
        return Err(PublisherError::Store);
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|_| PublisherError::Store)?;
    if decoded.len() > limit || base64::engine::general_purpose::STANDARD.encode(&decoded) != value
    {
        return Err(PublisherError::Store);
    }
    Ok(decoded)
}

fn domain_hash(domain: &[u8], value: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(value);
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn sha256_hex(value: &[u8]) -> String {
    Sha256::digest(value)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn check_deadline(deadline: Instant) -> Result<(), PublisherError> {
    if Instant::now() >= deadline {
        Err(PublisherError::Store)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{Permissions, hard_link};
    use std::os::unix::fs::PermissionsExt;
    use std::sync::{Arc, Barrier};
    use std::thread;

    use super::*;
    use crate::receipt::TestSigner;
    use crate::test_support::{fixture, secure_tempdir};

    const KEY_A: &str = "71a1de4805f764bdf13f374906476fbc60d23f0e4f93f6d63c33f2c4029d6605";
    const KEY_B: &str = "88578b21c1dbb86eace7b852723ab32b7564856518468168165f75890dd14b8e";

    fn key_digest(key: &str) -> String {
        domain_hash(
            b"fe2o3-protected-publisher-idempotency-key-v1\0",
            key.as_bytes(),
        )
    }

    fn projection_digest(request: &PublisherRequest) -> String {
        sha256_hex(&canonical_bytes(&request.oidc_authorization).unwrap())
    }

    fn issue_with(
        store: &mut DurableStore,
        key: &str,
        fixture: &crate::test_support::Fixture,
    ) -> Result<Vec<u8>, PublisherError> {
        let signer = TestSigner::new("test-publisher-v1");
        let identity = request_identity(&fixture.request_body);
        let request_sha = raw_request_sha256(&fixture.request_body);
        let auth = projection_digest(&fixture.request);
        store.issue(IssueInput {
            request_key_sha256: &key_digest(key),
            stable_authorization_sha256: &auth,
            request_identity: &identity,
            request_sha256: &request_sha,
            request_body: &fixture.request_body,
            request: &fixture.request,
            issued_at: 1_800_000_000,
            signature_domain: "test",
            signer: &signer,
        })
    }

    #[test]
    fn exact_retry_returns_identical_durable_bytes() {
        let temp = secure_tempdir();
        let path = temp.path().join("publisher.ledger");
        let mut store = DurableStore::open(&path).unwrap();
        let fixture = fixture();
        let first = issue_with(&mut store, KEY_A, &fixture).unwrap();
        let second = issue_with(&mut store, KEY_A, &fixture).unwrap();
        assert_eq!(first, second);
        assert_eq!(store.count(), 1);
        drop(store);
        let mut reopened = DurableStore::open(&path).unwrap();
        assert_eq!(issue_with(&mut reopened, KEY_A, &fixture).unwrap(), first);
    }

    #[test]
    fn key_body_authorization_and_request_substitution_fail_closed() {
        let temp = secure_tempdir();
        let path = temp.path().join("publisher.ledger");
        let mut store = DurableStore::open(&path).unwrap();
        let first = fixture();
        issue_with(&mut store, KEY_A, &first).unwrap();

        let mut changed_body = fixture();
        changed_body.request.archive_sha256 = "e".repeat(64);
        changed_body.request_body =
            canonical_bytes(&serde_json::to_value(&changed_body.request).unwrap()).unwrap();
        assert!(matches!(
            issue_with(&mut store, KEY_A, &changed_body),
            Err(PublisherError::ReplayConflict)
        ));
        assert!(matches!(
            issue_with(&mut store, KEY_B, &first),
            Err(PublisherError::ReplayConflict)
        ));

        let mut changed_auth = fixture();
        changed_auth.request.oidc_authorization["actor_id"] = Value::String("202".into());
        changed_auth.request_body =
            canonical_bytes(&serde_json::to_value(&changed_auth.request).unwrap()).unwrap();
        assert!(matches!(
            issue_with(&mut store, KEY_A, &changed_auth),
            Err(PublisherError::ReplayConflict)
        ));
    }

    #[test]
    fn complete_corruption_and_duplicate_frames_reject_on_restart() {
        let temp = secure_tempdir();
        let path = temp.path().join("publisher.ledger");
        let mut store = DurableStore::open(&path).unwrap();
        issue_with(&mut store, KEY_A, &fixture()).unwrap();
        let header_len = store.header_len;
        drop(store);
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[header_len as usize + FRAME_PREFIX_BYTES + 3] ^= 1;
        std::fs::write(&path, bytes).unwrap();
        assert!(DurableStore::open(&path).is_err());

        let duplicate_path = temp.path().join("duplicate.ledger");
        let mut store = DurableStore::open(&duplicate_path).unwrap();
        issue_with(&mut store, KEY_A, &fixture()).unwrap();
        let first_frame = {
            let bytes = std::fs::read(&duplicate_path).unwrap();
            bytes[store.header_len as usize..].to_vec()
        };
        let first_payload = &first_frame[FRAME_PREFIX_BYTES..first_frame.len() - FRAME_HASH_BYTES];
        let duplicate = encode_frame(2, store.tail_hash, first_payload).unwrap();
        drop(store);
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&duplicate_path)
            .unwrap();
        file.write_all(&duplicate).unwrap();
        file.sync_data().unwrap();
        assert!(DurableStore::open(&duplicate_path).is_err());
    }

    #[test]
    fn torn_tail_is_truncated_but_complete_frame_is_replayed() {
        let temp = secure_tempdir();
        let source = temp.path().join("source.ledger");
        let mut store = DurableStore::open(&source).unwrap();
        issue_with(&mut store, KEY_A, &fixture()).unwrap();
        let header_len = store.header_len as usize;
        drop(store);
        let bytes = std::fs::read(&source).unwrap();
        let header = &bytes[..header_len];
        let frame = &bytes[header_len..];
        for (index, cut) in [0, 1, 7, 31, 55, 56, 80, frame.len() / 2, frame.len() - 1]
            .into_iter()
            .enumerate()
        {
            let path = temp.path().join(format!("torn-{index}.ledger"));
            let mut partial = header.to_vec();
            partial.extend_from_slice(&frame[..cut]);
            std::fs::write(&path, partial).unwrap();
            std::fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
            let reopened = DurableStore::open(&path).unwrap();
            assert_eq!(reopened.count(), 0);
            assert_eq!(std::fs::metadata(&path).unwrap().len(), header_len as u64);
        }
        let complete = temp.path().join("complete.ledger");
        std::fs::write(&complete, bytes).unwrap();
        std::fs::set_permissions(&complete, Permissions::from_mode(0o600)).unwrap();
        assert_eq!(DurableStore::open(&complete).unwrap().count(), 1);
    }

    #[test]
    fn short_writes_complete_and_enospc_tail_recovers_without_acknowledgement() {
        let temp = secure_tempdir();
        let short = temp.path().join("short.ledger");
        let mut store = DurableStore::open(&short).unwrap();
        store.set_maximum_write_chunk(7);
        issue_with(&mut store, KEY_A, &fixture()).unwrap();
        drop(store);
        assert_eq!(DurableStore::open(&short).unwrap().count(), 1);

        let full = temp.path().join("full.ledger");
        let mut store = DurableStore::open(&full).unwrap();
        store.set_maximum_write_chunk(11);
        store.fail_write_after(33);
        assert!(matches!(
            issue_with(&mut store, KEY_A, &fixture()),
            Err(PublisherError::Store)
        ));
        assert!(matches!(
            issue_with(&mut store, KEY_A, &fixture()),
            Err(PublisherError::Store)
        ));
        drop(store);
        assert_eq!(DurableStore::open(&full).unwrap().count(), 0);
    }

    #[test]
    fn rename_unlink_hardlink_and_replacement_fail_stop() {
        for mode in ["rename", "unlink", "hardlink", "replace"] {
            let temp = secure_tempdir();
            let path = temp.path().join("publisher.ledger");
            let mut store = DurableStore::open(&path).unwrap();
            match mode {
                "rename" => std::fs::rename(&path, temp.path().join("moved")).unwrap(),
                "unlink" => std::fs::remove_file(&path).unwrap(),
                "hardlink" => hard_link(&path, temp.path().join("hard")).unwrap(),
                "replace" => {
                    std::fs::rename(&path, temp.path().join("old")).unwrap();
                    std::fs::write(&path, b"replacement").unwrap();
                    std::fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
                }
                _ => unreachable!(),
            }
            assert!(matches!(
                issue_with(&mut store, KEY_A, &fixture()),
                Err(PublisherError::Store)
            ));
        }
    }

    #[test]
    fn substitution_after_fsync_is_never_acknowledged() {
        let temp = secure_tempdir();
        let path = temp.path().join("publisher.ledger");
        let moved = temp.path().join("moved.ledger");
        let mut store = DurableStore::open(&path).unwrap();
        let path_for_hook = path.clone();
        store.set_after_sync(move || std::fs::rename(path_for_hook, moved).unwrap());
        assert!(matches!(
            issue_with(&mut store, KEY_A, &fixture()),
            Err(PublisherError::Store)
        ));
        assert!(matches!(
            issue_with(&mut store, KEY_A, &fixture()),
            Err(PublisherError::Store)
        ));
    }

    #[test]
    fn parent_directory_rename_and_replacement_fail_stop() {
        let temp = secure_tempdir();
        let state = temp.path().join("state");
        std::fs::create_dir(&state).unwrap();
        std::fs::set_permissions(&state, Permissions::from_mode(0o700)).unwrap();
        let path = state.join("publisher.ledger");
        let mut store = DurableStore::open(&path).unwrap();
        std::fs::rename(&state, temp.path().join("moved-state")).unwrap();
        std::fs::create_dir(&state).unwrap();
        std::fs::set_permissions(&state, Permissions::from_mode(0o700)).unwrap();
        assert!(matches!(
            issue_with(&mut store, KEY_A, &fixture()),
            Err(PublisherError::Store)
        ));
    }

    #[test]
    fn deadline_before_admission_rejects_and_admitted_commit_is_authoritative() {
        let temp = secure_tempdir();
        let path = temp.path().join("publisher.ledger");
        let mut store = DurableStore::open(&path).unwrap();
        let fixture = fixture();
        let signer = TestSigner::new("test-publisher-v1");
        let identity = request_identity(&fixture.request_body);
        let request_sha = raw_request_sha256(&fixture.request_body);
        let auth = projection_digest(&fixture.request);
        let request_key = key_digest(KEY_A);
        let make_input = || IssueInput {
            request_key_sha256: &request_key,
            stable_authorization_sha256: &auth,
            request_identity: &identity,
            request_sha256: &request_sha,
            request_body: &fixture.request_body,
            request: &fixture.request,
            issued_at: 1_800_000_000,
            signature_domain: "test",
            signer: &signer,
        };
        assert!(
            store
                .issue_until(make_input(), Instant::now() - Duration::from_millis(1))
                .is_err()
        );
        assert_eq!(store.count(), 0);
        store.set_commit_delay(Duration::from_millis(75));
        let deadline = Instant::now() + Duration::from_millis(20);
        let response = store.issue_until(make_input(), deadline).unwrap();
        assert!(Instant::now() >= deadline);
        assert_eq!(issue_with(&mut store, KEY_A, &fixture).unwrap(), response);
    }

    #[test]
    fn exclusive_lock_allows_only_one_writer() {
        let temp = secure_tempdir();
        let path = temp.path().join("publisher.ledger");
        let store = DurableStore::open(&path).unwrap();
        assert!(DurableStore::open(&path).is_err());
        drop(store);
        assert!(DurableStore::open(&path).is_ok());
    }

    #[test]
    fn concurrent_open_has_one_owner() {
        let temp = secure_tempdir();
        let path = Arc::new(temp.path().join("publisher.ledger"));
        let barrier = Arc::new(Barrier::new(8));
        let handles = (0..8)
            .map(|_| {
                let path = path.clone();
                let barrier = barrier.clone();
                thread::spawn(move || {
                    barrier.wait();
                    DurableStore::open(&path).is_ok()
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .filter(|opened| *opened)
                .count(),
            1
        );
    }
}
