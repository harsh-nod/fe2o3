use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::path::Path;

use fe2o3_artifacts::DigestAlgorithm;

use crate::{AuthenticatedProofExecutionIdentityV1, Digest};

#[cfg(target_os = "linux")]
#[path = "persistent_freshness_linux.rs"]
mod linux;

pub const PERSISTENT_FRESHNESS_VERSION_V1: u16 = 1;
pub const PERSISTENT_FRESHNESS_STATE_MAGIC_V1: [u8; 8] = *b"FE2PFLD\0";
pub const PERSISTENT_FRESHNESS_INTENT_MAGIC_V1: [u8; 8] = *b"FE2PFTX\0";
pub const MAX_PERSISTENT_FRESHNESS_ENTRIES_V1: usize = 65_536;

const DIGEST_ALGORITHM_SHA256_V1: u16 = 1;
const STATE_HEADER_BYTES_V1: usize = 64;
const IDENTITY_BYTES_V1: usize = 96;
const CHECKSUM_BYTES_V1: usize = 32;
const INTENT_BYTES_V1: usize = 260;
const STATE_CHECKSUM_DOMAIN_V1: [u8; 8] = *b"FE2PFSC\0";
const STATE_IDENTITY_DOMAIN_V1: [u8; 8] = *b"FE2PFSI\0";
const INTENT_CHECKSUM_DOMAIN_V1: [u8; 8] = *b"FE2PFTC\0";

pub const MAX_PERSISTENT_FRESHNESS_STATE_BYTES_V1: usize = STATE_HEADER_BYTES_V1
    + MAX_PERSISTENT_FRESHNESS_ENTRIES_V1 * IDENTITY_BYTES_V1
    + CHECKSUM_BYTES_V1;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PersistentFreshnessIdentityV1 {
    challenge: Digest,
    transcript: Digest,
    result: Digest,
}

impl PersistentFreshnessIdentityV1 {
    pub const fn challenge(self) -> Digest {
        self.challenge
    }

    pub const fn transcript(self) -> Digest {
        self.transcript
    }

    pub const fn result(self) -> Digest {
        self.result
    }

    fn validate(self) -> Result<(), PersistentFreshnessRecordErrorV1> {
        for (field, digest) in [
            (
                PersistentFreshnessIdentityFieldV1::Challenge,
                self.challenge,
            ),
            (
                PersistentFreshnessIdentityFieldV1::Transcript,
                self.transcript,
            ),
            (PersistentFreshnessIdentityFieldV1::Result, self.result),
        ] {
            if digest.as_bytes().iter().all(|byte| *byte == 0) {
                return Err(PersistentFreshnessRecordErrorV1::ZeroIdentity { field });
            }
        }
        Ok(())
    }

    fn from_recorder_report_identity(
        identity: &AuthenticatedProofExecutionIdentityV1,
    ) -> Result<Self, PersistentFreshnessLedgerErrorV1> {
        let value = Self {
            challenge: identity.challenge(),
            transcript: identity.transcript_digest(),
            result: identity.result().digest(),
        };
        value
            .validate()
            .map_err(|error| PersistentFreshnessLedgerErrorV1::Record {
                file: PersistentFreshnessLedgerFileV1::State,
                error,
            })?;
        Ok(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistentFreshnessStateInspectionV1 {
    namespace: Digest,
    generation: u64,
    consumed_count: u32,
    state_identity: Digest,
}

impl PersistentFreshnessStateInspectionV1 {
    pub const fn namespace(self) -> Digest {
        self.namespace
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }

    pub const fn consumed_count(self) -> u32 {
        self.consumed_count
    }

    pub const fn state_identity(self) -> Digest {
        self.state_identity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistentFreshnessIntentInspectionV1 {
    namespace: Digest,
    previous_generation: u64,
    next_generation: u64,
    previous_state_identity: Digest,
    next_state_identity: Digest,
    identity: PersistentFreshnessIdentityV1,
}

impl PersistentFreshnessIntentInspectionV1 {
    pub const fn namespace(self) -> Digest {
        self.namespace
    }

    pub const fn previous_generation(self) -> u64 {
        self.previous_generation
    }

    pub const fn next_generation(self) -> u64 {
        self.next_generation
    }

    pub const fn previous_state_identity(self) -> Digest {
        self.previous_state_identity
    }

    pub const fn next_state_identity(self) -> Digest {
        self.next_state_identity
    }

    pub const fn identity(self) -> PersistentFreshnessIdentityV1 {
        self.identity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistentFreshnessRecoveryV1 {
    Initialized,
    Clean,
    DiscardedUncommittedIntent,
    AppliedPendingIntent,
    FinalizedPendingIntent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistentFreshnessReceiptV1 {
    identity: PersistentFreshnessIdentityV1,
    namespace: Digest,
    previous_state_identity: Digest,
    generation: u64,
    state_identity: Digest,
}

impl PersistentFreshnessReceiptV1 {
    pub const fn identity(self) -> PersistentFreshnessIdentityV1 {
        self.identity
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }

    pub const fn namespace(self) -> Digest {
        self.namespace
    }

    pub const fn previous_state_identity(self) -> Digest {
        self.previous_state_identity
    }

    pub const fn state_identity(self) -> Digest {
        self.state_identity
    }

    pub const fn grants_runtime_authority(self) -> bool {
        false
    }
}

/// A Linux persistent replay ledger rooted at one retained directory
/// descriptor. Every transaction opens and locks a fresh nofollow lock-file
/// descriptor and rejects use from a process other than its creator.
///
/// Construction is intentionally separate from the process-local
/// `AuthenticatedExecutionFreshnessV1`. The value is neither `Clone` nor a
/// source of runtime authority. The caller must provision an owner-controlled
/// local filesystem directory whose identity remains trustworthy across
/// process restarts; a pathname is not treated as an authenticated identity.
pub struct PersistentProofFreshnessLedgerV1 {
    #[cfg(target_os = "linux")]
    inner: linux::LinuxLedger,
}

impl fmt::Debug for PersistentProofFreshnessLedgerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PersistentProofFreshnessLedgerV1")
            .finish_non_exhaustive()
    }
}

impl PersistentProofFreshnessLedgerV1 {
    /// Creates and durably initializes a ledger in a directory that has never
    /// contained one. Existing ledger files are rejected.
    pub fn create_new(
        directory: impl AsRef<Path>,
    ) -> Result<(Self, PersistentFreshnessRecoveryV1), PersistentFreshnessLedgerErrorV1> {
        #[cfg(target_os = "linux")]
        {
            let (inner, recovery) = linux::LinuxLedger::create_new(directory.as_ref())?;
            Ok((Self { inner }, recovery))
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = directory;
            Err(PersistentFreshnessLedgerErrorV1::UnsupportedPlatform)
        }
    }

    /// Opens and recovers an initialized ledger without creating missing
    /// state. A deleted state file fails closed.
    pub fn open_existing(
        directory: impl AsRef<Path>,
    ) -> Result<(Self, PersistentFreshnessRecoveryV1), PersistentFreshnessLedgerErrorV1> {
        #[cfg(target_os = "linux")]
        {
            let (inner, recovery) = linux::LinuxLedger::open_existing(directory.as_ref())?;
            Ok((Self { inner }, recovery))
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = directory;
            Err(PersistentFreshnessLedgerErrorV1::UnsupportedPlatform)
        }
    }

    pub fn try_begin_exclusive(
        &mut self,
    ) -> Result<PersistentProofFreshnessTransactionV1<'_>, PersistentFreshnessLedgerErrorV1> {
        #[cfg(target_os = "linux")]
        {
            Ok(PersistentProofFreshnessTransactionV1 {
                inner: self.inner.try_begin_exclusive()?,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(PersistentFreshnessLedgerErrorV1::UnsupportedPlatform)
        }
    }

    /// Durably consumes the replay identity of an authenticated recorder report.
    ///
    /// This records challenge/transcript/result digests only. It does not show
    /// that a verifier or solver ran and grants no proof authority.
    pub fn consume_authenticated_recorder_output(
        &mut self,
        identity: &AuthenticatedProofExecutionIdentityV1,
    ) -> Result<PersistentFreshnessReceiptV1, PersistentFreshnessLedgerErrorV1> {
        self.try_begin_exclusive()?.consume(identity)
    }

    #[deprecated(
        note = "use consume_authenticated_recorder_output(); the identity is a recorder report"
    )]
    pub fn consume_authenticated_execution(
        &mut self,
        identity: &AuthenticatedProofExecutionIdentityV1,
    ) -> Result<PersistentFreshnessReceiptV1, PersistentFreshnessLedgerErrorV1> {
        self.consume_authenticated_recorder_output(identity)
    }

    pub fn inspect(
        &mut self,
    ) -> Result<PersistentFreshnessStateInspectionV1, PersistentFreshnessLedgerErrorV1> {
        Ok(self.try_begin_exclusive()?.state())
    }
}

pub struct PersistentProofFreshnessTransactionV1<'a> {
    #[cfg(target_os = "linux")]
    inner: linux::LinuxTransaction<'a>,
}

impl fmt::Debug for PersistentProofFreshnessTransactionV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PersistentProofFreshnessTransactionV1")
            .finish_non_exhaustive()
    }
}

impl PersistentProofFreshnessTransactionV1<'_> {
    pub fn recovery(&self) -> PersistentFreshnessRecoveryV1 {
        #[cfg(target_os = "linux")]
        {
            self.inner.recovery()
        }
        #[cfg(not(target_os = "linux"))]
        {
            unreachable!("persistent freshness transactions require Linux")
        }
    }

    pub fn state(&self) -> PersistentFreshnessStateInspectionV1 {
        #[cfg(target_os = "linux")]
        {
            self.inner.state()
        }
        #[cfg(not(target_os = "linux"))]
        {
            unreachable!("persistent freshness transactions require Linux")
        }
    }

    /// Consumes one recorder-report replay identity in this transaction.
    ///
    /// The input does not authenticate verifier or solver execution.
    pub fn consume(
        &mut self,
        identity: &AuthenticatedProofExecutionIdentityV1,
    ) -> Result<PersistentFreshnessReceiptV1, PersistentFreshnessLedgerErrorV1> {
        let identity = PersistentFreshnessIdentityV1::from_recorder_report_identity(identity)?;
        #[cfg(target_os = "linux")]
        {
            self.inner.consume(identity)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = identity;
            Err(PersistentFreshnessLedgerErrorV1::UnsupportedPlatform)
        }
    }
}

pub fn inspect_persistent_freshness_state_v1(
    bytes: &[u8],
) -> Result<PersistentFreshnessStateInspectionV1, PersistentFreshnessRecordErrorV1> {
    let state = FreshnessStateV1::decode(bytes)?;
    Ok(PersistentFreshnessStateInspectionV1 {
        namespace: state.namespace,
        generation: state.generation,
        consumed_count: state.entries.len() as u32,
        state_identity: state.identity(),
    })
}

pub fn inspect_persistent_freshness_intent_v1(
    bytes: &[u8],
) -> Result<PersistentFreshnessIntentInspectionV1, PersistentFreshnessRecordErrorV1> {
    let intent = FreshnessIntentV1::decode(bytes)?;
    Ok(PersistentFreshnessIntentInspectionV1 {
        namespace: intent.namespace,
        previous_generation: intent.previous_generation,
        next_generation: intent.next_generation,
        previous_state_identity: intent.previous_state_identity,
        next_state_identity: intent.next_state_identity,
        identity: intent.identity,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FreshnessStateV1 {
    namespace: Digest,
    generation: u64,
    entries: Vec<PersistentFreshnessIdentityV1>,
}

impl FreshnessStateV1 {
    fn empty(namespace: Digest) -> Self {
        Self {
            namespace,
            generation: 0,
            entries: Vec::new(),
        }
    }

    fn inspection(&self) -> PersistentFreshnessStateInspectionV1 {
        PersistentFreshnessStateInspectionV1 {
            namespace: self.namespace,
            generation: self.generation,
            consumed_count: self.entries.len() as u32,
            state_identity: self.identity(),
        }
    }

    fn with_consumed(
        &self,
        identity: PersistentFreshnessIdentityV1,
    ) -> Result<Self, PersistentFreshnessLedgerErrorV1> {
        identity
            .validate()
            .map_err(|error| PersistentFreshnessLedgerErrorV1::Record {
                file: PersistentFreshnessLedgerFileV1::State,
                error,
            })?;
        if self.entries.len() == MAX_PERSISTENT_FRESHNESS_ENTRIES_V1 {
            return Err(PersistentFreshnessLedgerErrorV1::Full {
                max: MAX_PERSISTENT_FRESHNESS_ENTRIES_V1,
            });
        }
        for (field, replayed) in [
            (
                PersistentFreshnessIdentityFieldV1::Challenge,
                self.entries
                    .iter()
                    .any(|entry| entry.challenge == identity.challenge),
            ),
            (
                PersistentFreshnessIdentityFieldV1::Transcript,
                self.entries
                    .iter()
                    .any(|entry| entry.transcript == identity.transcript),
            ),
            (
                PersistentFreshnessIdentityFieldV1::Result,
                self.entries
                    .iter()
                    .any(|entry| entry.result == identity.result),
            ),
        ] {
            if replayed {
                return Err(PersistentFreshnessLedgerErrorV1::Replay { field });
            }
        }
        let mut entries = self.entries.clone();
        let insertion = entries
            .binary_search(&identity)
            .expect_err("an exact duplicate must share all independently checked identities");
        entries.insert(insertion, identity);
        Ok(Self {
            namespace: self.namespace,
            generation: self.generation + 1,
            entries,
        })
    }

    fn contains(&self, identity: PersistentFreshnessIdentityV1) -> bool {
        self.entries.binary_search(&identity).is_ok()
    }

    fn validate(&self) -> Result<(), PersistentFreshnessRecordErrorV1> {
        if self.namespace.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(PersistentFreshnessRecordErrorV1::ZeroRecordIdentity {
                field: "ledger namespace",
            });
        }
        if self.entries.len() > MAX_PERSISTENT_FRESHNESS_ENTRIES_V1 {
            return Err(PersistentFreshnessRecordErrorV1::TooManyEntries {
                count: self.entries.len() as u64,
                max: MAX_PERSISTENT_FRESHNESS_ENTRIES_V1,
            });
        }
        if self.generation != self.entries.len() as u64 {
            return Err(PersistentFreshnessRecordErrorV1::GenerationCountMismatch {
                generation: self.generation,
                count: self.entries.len() as u32,
            });
        }

        let mut challenges = BTreeSet::new();
        let mut transcripts = BTreeSet::new();
        let mut results = BTreeSet::new();
        let mut previous = None;
        for entry in &self.entries {
            entry.validate()?;
            if previous.is_some_and(|previous| previous >= *entry) {
                return Err(PersistentFreshnessRecordErrorV1::NonCanonical);
            }
            previous = Some(*entry);
            for (field, was_new) in [
                (
                    PersistentFreshnessIdentityFieldV1::Challenge,
                    challenges.insert(entry.challenge),
                ),
                (
                    PersistentFreshnessIdentityFieldV1::Transcript,
                    transcripts.insert(entry.transcript),
                ),
                (
                    PersistentFreshnessIdentityFieldV1::Result,
                    results.insert(entry.result),
                ),
            ] {
                if !was_new {
                    return Err(PersistentFreshnessRecordErrorV1::DuplicateIdentity { field });
                }
            }
        }
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>, PersistentFreshnessRecordErrorV1> {
        self.validate()?;
        let total_len = STATE_HEADER_BYTES_V1
            .checked_add(self.entries.len() * IDENTITY_BYTES_V1)
            .and_then(|length| length.checked_add(CHECKSUM_BYTES_V1))
            .ok_or(PersistentFreshnessRecordErrorV1::LengthOverflow)?;
        let total_len = u32::try_from(total_len)
            .map_err(|_| PersistentFreshnessRecordErrorV1::LengthOverflow)?;
        let mut writer = Writer::new();
        writer.bytes(&PERSISTENT_FRESHNESS_STATE_MAGIC_V1);
        writer.u16(PERSISTENT_FRESHNESS_VERSION_V1);
        writer.u16(0);
        writer.u32(total_len);
        writer.u64(self.generation);
        writer.u32(self.entries.len() as u32);
        writer.u16(DIGEST_ALGORITHM_SHA256_V1);
        writer.u16(0);
        writer.digest(self.namespace);
        for entry in &self.entries {
            writer.identity(*entry);
        }
        let checksum = domain_digest(STATE_CHECKSUM_DOMAIN_V1, &writer.bytes);
        writer.digest(checksum);
        Ok(writer.bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self, PersistentFreshnessRecordErrorV1> {
        if bytes.len() > MAX_PERSISTENT_FRESHNESS_STATE_BYTES_V1 {
            return Err(PersistentFreshnessRecordErrorV1::TooLarge {
                max: MAX_PERSISTENT_FRESHNESS_STATE_BYTES_V1,
            });
        }
        let mut reader = Reader::new(bytes);
        if reader.fixed::<8>()? != PERSISTENT_FRESHNESS_STATE_MAGIC_V1 {
            return Err(PersistentFreshnessRecordErrorV1::InvalidMagic);
        }
        require_version(reader.u16()?)?;
        require_zero(reader.u16()?, "state flags")?;
        require_exact_length(reader.u32()?, bytes.len())?;
        let generation = reader.u64()?;
        let count = reader.count_u32()?;
        require_algorithm(reader.u16()?)?;
        require_zero(reader.u16()?, "state reserved")?;
        let namespace = reader.digest()?;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            entries.push(reader.identity()?);
        }
        let checksum_offset = reader.offset;
        let checksum = reader.digest()?;
        if !reader.is_finished() {
            return Err(PersistentFreshnessRecordErrorV1::TrailingBytes);
        }
        if checksum != domain_digest(STATE_CHECKSUM_DOMAIN_V1, &bytes[..checksum_offset]) {
            return Err(PersistentFreshnessRecordErrorV1::ChecksumMismatch);
        }
        let state = Self {
            namespace,
            generation,
            entries,
        };
        state.validate()?;
        if state.encode()?.as_slice() != bytes {
            return Err(PersistentFreshnessRecordErrorV1::NonCanonical);
        }
        Ok(state)
    }

    fn identity(&self) -> Digest {
        domain_digest(
            STATE_IDENTITY_DOMAIN_V1,
            &self
                .encode()
                .expect("a decoded state remains canonically encodable"),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FreshnessIntentV1 {
    namespace: Digest,
    previous_generation: u64,
    next_generation: u64,
    previous_state_identity: Digest,
    next_state_identity: Digest,
    identity: PersistentFreshnessIdentityV1,
}

impl FreshnessIntentV1 {
    fn new(
        previous: &FreshnessStateV1,
        next: &FreshnessStateV1,
        identity: PersistentFreshnessIdentityV1,
    ) -> Result<Self, PersistentFreshnessRecordErrorV1> {
        let value = Self {
            namespace: previous.namespace,
            previous_generation: previous.generation,
            next_generation: next.generation,
            previous_state_identity: previous.identity(),
            next_state_identity: next.identity(),
            identity,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(self) -> Result<(), PersistentFreshnessRecordErrorV1> {
        if self.next_generation == 0
            || self.previous_generation.checked_add(1) != Some(self.next_generation)
        {
            return Err(PersistentFreshnessRecordErrorV1::InvalidIntentGeneration {
                previous: self.previous_generation,
                next: self.next_generation,
            });
        }
        for (field, digest) in [
            ("ledger namespace", self.namespace),
            ("previous state identity", self.previous_state_identity),
            ("next state identity", self.next_state_identity),
        ] {
            if digest.as_bytes().iter().all(|byte| *byte == 0) {
                return Err(PersistentFreshnessRecordErrorV1::ZeroRecordIdentity { field });
            }
        }
        self.identity.validate()
    }

    fn encode(self) -> Result<Vec<u8>, PersistentFreshnessRecordErrorV1> {
        self.validate()?;
        let mut writer = Writer::new();
        writer.bytes(&PERSISTENT_FRESHNESS_INTENT_MAGIC_V1);
        writer.u16(PERSISTENT_FRESHNESS_VERSION_V1);
        writer.u16(0);
        writer.u32(INTENT_BYTES_V1 as u32);
        writer.u64(self.previous_generation);
        writer.u64(self.next_generation);
        writer.u16(DIGEST_ALGORITHM_SHA256_V1);
        writer.u16(0);
        writer.digest(self.namespace);
        writer.digest(self.previous_state_identity);
        writer.digest(self.next_state_identity);
        writer.identity(self.identity);
        let checksum = domain_digest(INTENT_CHECKSUM_DOMAIN_V1, &writer.bytes);
        writer.digest(checksum);
        debug_assert_eq!(writer.bytes.len(), INTENT_BYTES_V1);
        Ok(writer.bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self, PersistentFreshnessRecordErrorV1> {
        if bytes.len() > INTENT_BYTES_V1 {
            return Err(PersistentFreshnessRecordErrorV1::TooLarge {
                max: INTENT_BYTES_V1,
            });
        }
        let mut reader = Reader::new(bytes);
        if reader.fixed::<8>()? != PERSISTENT_FRESHNESS_INTENT_MAGIC_V1 {
            return Err(PersistentFreshnessRecordErrorV1::InvalidMagic);
        }
        require_version(reader.u16()?)?;
        require_zero(reader.u16()?, "intent flags")?;
        require_exact_length(reader.u32()?, bytes.len())?;
        let previous_generation = reader.u64()?;
        let next_generation = reader.u64()?;
        require_algorithm(reader.u16()?)?;
        require_zero(reader.u16()?, "intent reserved")?;
        let namespace = reader.digest()?;
        let previous_state_identity = reader.digest()?;
        let next_state_identity = reader.digest()?;
        let identity = reader.identity()?;
        let checksum_offset = reader.offset;
        let checksum = reader.digest()?;
        if !reader.is_finished() {
            return Err(PersistentFreshnessRecordErrorV1::TrailingBytes);
        }
        if checksum != domain_digest(INTENT_CHECKSUM_DOMAIN_V1, &bytes[..checksum_offset]) {
            return Err(PersistentFreshnessRecordErrorV1::ChecksumMismatch);
        }
        let intent = Self {
            namespace,
            previous_generation,
            next_generation,
            previous_state_identity,
            next_state_identity,
            identity,
        };
        intent.validate()?;
        if intent.encode()?.as_slice() != bytes {
            return Err(PersistentFreshnessRecordErrorV1::NonCanonical);
        }
        Ok(intent)
    }
}

fn require_version(version: u16) -> Result<(), PersistentFreshnessRecordErrorV1> {
    if version == PERSISTENT_FRESHNESS_VERSION_V1 {
        Ok(())
    } else {
        Err(PersistentFreshnessRecordErrorV1::UnknownVersion(version))
    }
}

fn require_zero(value: u16, field: &'static str) -> Result<(), PersistentFreshnessRecordErrorV1> {
    if value == 0 {
        Ok(())
    } else {
        Err(PersistentFreshnessRecordErrorV1::UnknownField { field, value })
    }
}

fn require_algorithm(algorithm: u16) -> Result<(), PersistentFreshnessRecordErrorV1> {
    if algorithm == DIGEST_ALGORITHM_SHA256_V1 {
        Ok(())
    } else {
        Err(PersistentFreshnessRecordErrorV1::UnknownDigestAlgorithm(
            algorithm,
        ))
    }
}

fn require_exact_length(
    declared: u32,
    actual: usize,
) -> Result<(), PersistentFreshnessRecordErrorV1> {
    let declared =
        usize::try_from(declared).map_err(|_| PersistentFreshnessRecordErrorV1::LengthOverflow)?;
    if declared > actual {
        Err(PersistentFreshnessRecordErrorV1::Truncated)
    } else if declared < actual {
        Err(PersistentFreshnessRecordErrorV1::TrailingBytes)
    } else {
        Ok(())
    }
}

fn domain_digest(domain: [u8; 8], bytes: &[u8]) -> Digest {
    let mut payload = Vec::with_capacity(domain.len() + 4 + bytes.len());
    payload.extend_from_slice(&domain);
    payload.extend_from_slice(&PERSISTENT_FRESHNESS_VERSION_V1.to_le_bytes());
    payload.extend_from_slice(&0_u16.to_le_bytes());
    payload.extend_from_slice(bytes);
    let digest = DigestAlgorithm::Sha256.calculate(&payload);
    Digest::from_bytes(*digest.bytes().as_bytes())
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn digest(&mut self, digest: Digest) {
        self.bytes.extend_from_slice(digest.as_bytes());
    }

    fn identity(&mut self, identity: PersistentFreshnessIdentityV1) {
        self.digest(identity.challenge);
        self.digest(identity.transcript);
        self.digest(identity.result);
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], PersistentFreshnessRecordErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(PersistentFreshnessRecordErrorV1::LengthOverflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(PersistentFreshnessRecordErrorV1::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], PersistentFreshnessRecordErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| PersistentFreshnessRecordErrorV1::Truncated)
    }

    fn u16(&mut self) -> Result<u16, PersistentFreshnessRecordErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, PersistentFreshnessRecordErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, PersistentFreshnessRecordErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn digest(&mut self) -> Result<Digest, PersistentFreshnessRecordErrorV1> {
        Ok(Digest::from_bytes(self.fixed()?))
    }

    fn identity(
        &mut self,
    ) -> Result<PersistentFreshnessIdentityV1, PersistentFreshnessRecordErrorV1> {
        Ok(PersistentFreshnessIdentityV1 {
            challenge: self.digest()?,
            transcript: self.digest()?,
            result: self.digest()?,
        })
    }

    fn count_u32(&mut self) -> Result<usize, PersistentFreshnessRecordErrorV1> {
        let raw = self.u32()?;
        let count =
            usize::try_from(raw).map_err(|_| PersistentFreshnessRecordErrorV1::TooManyEntries {
                count: u64::from(raw),
                max: MAX_PERSISTENT_FRESHNESS_ENTRIES_V1,
            })?;
        if count > MAX_PERSISTENT_FRESHNESS_ENTRIES_V1 {
            return Err(PersistentFreshnessRecordErrorV1::TooManyEntries {
                count: u64::from(raw),
                max: MAX_PERSISTENT_FRESHNESS_ENTRIES_V1,
            });
        }
        Ok(count)
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistentFreshnessLedgerFileV1 {
    Directory,
    Lock,
    State,
    StateTemporary,
    Intent,
    IntentTemporary,
}

impl PersistentFreshnessLedgerFileV1 {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Directory => "directory",
            Self::Lock => "lock file",
            Self::State => "state file",
            Self::StateTemporary => "temporary state file",
            Self::Intent => "intent file",
            Self::IntentTemporary => "temporary intent file",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistentFreshnessLedgerOperationV1 {
    Open,
    Inspect,
    Lock,
    Read,
    Create,
    Write,
    Sync,
    Rename,
    Remove,
}

impl PersistentFreshnessLedgerOperationV1 {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Inspect => "inspect",
            Self::Lock => "lock",
            Self::Read => "read",
            Self::Create => "create",
            Self::Write => "write",
            Self::Sync => "sync",
            Self::Rename => "rename",
            Self::Remove => "remove",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PersistentFreshnessLedgerErrorV1 {
    UnsupportedPlatform,
    InvalidDirectoryPath,
    LedgerAlreadyExists,
    MissingState,
    ZeroNamespace,
    Io {
        operation: PersistentFreshnessLedgerOperationV1,
        file: PersistentFreshnessLedgerFileV1,
        kind: io::ErrorKind,
    },
    NotDirectory,
    InsecureDirectoryOwner,
    InsecureDirectoryPermissions,
    FileNotRegular {
        file: PersistentFreshnessLedgerFileV1,
    },
    FileHasMultipleLinks {
        file: PersistentFreshnessLedgerFileV1,
        links: u64,
    },
    FileOwnerMismatch {
        file: PersistentFreshnessLedgerFileV1,
    },
    FilePermissionsTooBroad {
        file: PersistentFreshnessLedgerFileV1,
    },
    FileTooLarge {
        file: PersistentFreshnessLedgerFileV1,
        max: usize,
    },
    FileChangedDuringRead {
        file: PersistentFreshnessLedgerFileV1,
    },
    LockBusy,
    ForkDetected {
        owner: u32,
        current: u32,
    },
    LockFileSubstituted,
    UnexpectedRecoveryFile {
        file: PersistentFreshnessLedgerFileV1,
    },
    AmbiguousRecovery,
    RecoveryConflict,
    Record {
        file: PersistentFreshnessLedgerFileV1,
        error: PersistentFreshnessRecordErrorV1,
    },
    Replay {
        field: PersistentFreshnessIdentityFieldV1,
    },
    Full {
        max: usize,
    },
}

impl fmt::Display for PersistentFreshnessLedgerErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => {
                formatter.write_str("persistent proof freshness requires Linux")
            }
            Self::InvalidDirectoryPath => {
                formatter.write_str("persistent freshness directory path is invalid")
            }
            Self::LedgerAlreadyExists => {
                formatter.write_str("persistent freshness ledger already exists")
            }
            Self::MissingState => formatter.write_str("persistent freshness state is missing"),
            Self::ZeroNamespace => formatter.write_str("persistent freshness namespace is zero"),
            Self::Io {
                operation,
                file,
                kind,
            } => write!(
                formatter,
                "cannot {} persistent freshness {}: {kind}",
                operation.as_str(),
                file.as_str()
            ),
            Self::NotDirectory => {
                formatter.write_str("persistent freshness root is not a directory")
            }
            Self::InsecureDirectoryOwner => formatter
                .write_str("persistent freshness directory is not owned by the effective user"),
            Self::InsecureDirectoryPermissions => formatter
                .write_str("persistent freshness directory is writable by group or other users"),
            Self::FileNotRegular { file } => {
                write!(
                    formatter,
                    "persistent freshness {} is not regular",
                    file.as_str()
                )
            }
            Self::FileHasMultipleLinks { file, links } => write!(
                formatter,
                "persistent freshness {} has {links} hard links",
                file.as_str()
            ),
            Self::FileOwnerMismatch { file } => write!(
                formatter,
                "persistent freshness {} owner does not match",
                file.as_str()
            ),
            Self::FilePermissionsTooBroad { file } => write!(
                formatter,
                "persistent freshness {} permissions are too broad",
                file.as_str()
            ),
            Self::FileTooLarge { file, max } => write!(
                formatter,
                "persistent freshness {} exceeds {max} bytes",
                file.as_str()
            ),
            Self::FileChangedDuringRead { file } => write!(
                formatter,
                "persistent freshness {} changed while being read",
                file.as_str()
            ),
            Self::LockBusy => formatter.write_str("persistent freshness ledger is locked"),
            Self::ForkDetected { owner, current } => write!(
                formatter,
                "persistent freshness transaction belongs to process {owner}, not {current}"
            ),
            Self::LockFileSubstituted => {
                formatter.write_str("persistent freshness lock file was substituted")
            }
            Self::UnexpectedRecoveryFile { file } => write!(
                formatter,
                "persistent freshness {} has no valid recovery role",
                file.as_str()
            ),
            Self::AmbiguousRecovery => {
                formatter.write_str("persistent freshness recovery state is ambiguous")
            }
            Self::RecoveryConflict => {
                formatter.write_str("persistent freshness intent conflicts with durable state")
            }
            Self::Record { file, error } => {
                write!(
                    formatter,
                    "invalid persistent freshness {}: {error}",
                    file.as_str()
                )
            }
            Self::Replay { field } => write!(
                formatter,
                "persistent freshness {} identity was already consumed",
                field.as_str()
            ),
            Self::Full { max } => {
                write!(
                    formatter,
                    "persistent freshness ledger reached {max} entries"
                )
            }
        }
    }
}

impl std::error::Error for PersistentFreshnessLedgerErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Record { error, .. } => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistentFreshnessIdentityFieldV1 {
    Challenge,
    Transcript,
    Result,
}

impl PersistentFreshnessIdentityFieldV1 {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Challenge => "challenge",
            Self::Transcript => "transcript",
            Self::Result => "result",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PersistentFreshnessRecordErrorV1 {
    TooLarge {
        max: usize,
    },
    Truncated,
    TrailingBytes,
    InvalidMagic,
    UnknownVersion(u16),
    UnknownField {
        field: &'static str,
        value: u16,
    },
    UnknownDigestAlgorithm(u16),
    LengthOverflow,
    ChecksumMismatch,
    NonCanonical,
    TooManyEntries {
        count: u64,
        max: usize,
    },
    GenerationCountMismatch {
        generation: u64,
        count: u32,
    },
    InvalidIntentGeneration {
        previous: u64,
        next: u64,
    },
    ZeroIdentity {
        field: PersistentFreshnessIdentityFieldV1,
    },
    ZeroRecordIdentity {
        field: &'static str,
    },
    DuplicateIdentity {
        field: PersistentFreshnessIdentityFieldV1,
    },
}

impl fmt::Display for PersistentFreshnessRecordErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => write!(formatter, "freshness record exceeds {max} bytes"),
            Self::Truncated => formatter.write_str("freshness record is truncated"),
            Self::TrailingBytes => formatter.write_str("freshness record has trailing bytes"),
            Self::InvalidMagic => formatter.write_str("freshness record magic is invalid"),
            Self::UnknownVersion(version) => {
                write!(
                    formatter,
                    "freshness record version {version} is unsupported"
                )
            }
            Self::UnknownField { field, value } => {
                write!(
                    formatter,
                    "freshness {field} field {value:#x} is unsupported"
                )
            }
            Self::UnknownDigestAlgorithm(algorithm) => write!(
                formatter,
                "freshness digest algorithm {algorithm} is unsupported"
            ),
            Self::LengthOverflow => formatter.write_str("freshness record length overflows"),
            Self::ChecksumMismatch => {
                formatter.write_str("freshness record checksum does not match")
            }
            Self::NonCanonical => formatter.write_str("freshness record is not canonical"),
            Self::TooManyEntries { count, max } => {
                write!(formatter, "freshness entry count {count} exceeds {max}")
            }
            Self::GenerationCountMismatch { generation, count } => write!(
                formatter,
                "freshness generation {generation} does not match entry count {count}"
            ),
            Self::InvalidIntentGeneration { previous, next } => write!(
                formatter,
                "freshness intent generation {previous} -> {next} is invalid"
            ),
            Self::ZeroIdentity { field } => {
                write!(formatter, "freshness {} identity is zero", field.as_str())
            }
            Self::ZeroRecordIdentity { field } => {
                write!(formatter, "freshness {field} is zero")
            }
            Self::DuplicateIdentity { field } => write!(
                formatter,
                "freshness {} identity is duplicated",
                field.as_str()
            ),
        }
    }
}

impl std::error::Error for PersistentFreshnessRecordErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(seed: u8) -> Digest {
        Digest::from_bytes([seed; 32])
    }

    fn identity(seed: u8) -> PersistentFreshnessIdentityV1 {
        PersistentFreshnessIdentityV1 {
            challenge: digest(seed),
            transcript: digest(seed.wrapping_add(1)),
            result: digest(seed.wrapping_add(2)),
        }
    }

    fn state(entries: Vec<PersistentFreshnessIdentityV1>) -> FreshnessStateV1 {
        FreshnessStateV1 {
            namespace: digest(0xf0),
            generation: entries.len() as u64,
            entries,
        }
    }

    fn intent() -> FreshnessIntentV1 {
        let previous = state(vec![identity(1)]);
        let next = state(vec![identity(1), identity(4)]);
        FreshnessIntentV1 {
            namespace: previous.namespace,
            previous_generation: 1,
            next_generation: 2,
            previous_state_identity: previous.identity(),
            next_state_identity: next.identity(),
            identity: identity(4),
        }
    }

    fn repair_checksum(bytes: &mut [u8], domain: [u8; 8]) {
        let checksum_offset = bytes.len() - CHECKSUM_BYTES_V1;
        let checksum = domain_digest(domain, &bytes[..checksum_offset]);
        bytes[checksum_offset..].copy_from_slice(checksum.as_bytes());
    }

    #[test]
    fn state_and_intent_have_stable_canonical_shapes() {
        let state = state(vec![identity(1), identity(4)]);
        let state_bytes = state.encode().unwrap();
        assert_eq!(&state_bytes[..8], b"FE2PFLD\0");
        assert_eq!(state_bytes.len(), STATE_HEADER_BYTES_V1 + 2 * 96 + 32);
        assert_eq!(FreshnessStateV1::decode(&state_bytes).unwrap(), state);
        let inspected = inspect_persistent_freshness_state_v1(&state_bytes).unwrap();
        assert_eq!(inspected.namespace(), digest(0xf0));
        assert_eq!(inspected.generation(), 2);
        assert_eq!(inspected.consumed_count(), 2);
        assert_eq!(inspected.state_identity(), state.identity());

        let intent = intent();
        let intent_bytes = intent.encode().unwrap();
        assert_eq!(&intent_bytes[..8], b"FE2PFTX\0");
        assert_eq!(intent_bytes.len(), INTENT_BYTES_V1);
        assert_eq!(FreshnessIntentV1::decode(&intent_bytes).unwrap(), intent);
        let inspected = inspect_persistent_freshness_intent_v1(&intent_bytes).unwrap();
        assert_eq!(inspected.namespace(), digest(0xf0));
        assert_eq!(inspected.previous_generation(), 1);
        assert_eq!(inspected.next_generation(), 2);
        assert_eq!(inspected.identity(), identity(4));
    }

    #[test]
    fn every_truncation_trailing_byte_and_single_bit_mutation_is_rejected() {
        for (bytes, decode) in [
            (
                state(vec![identity(1), identity(4)]).encode().unwrap(),
                FreshnessStateV1::decode as fn(&[u8]) -> Result<_, _>,
            ),
            (intent().encode().unwrap(), |bytes| {
                FreshnessIntentV1::decode(bytes).map(|_| FreshnessStateV1 {
                    namespace: digest(0xf0),
                    generation: 0,
                    entries: vec![],
                })
            }),
        ] {
            for length in 0..bytes.len() {
                assert!(
                    decode(&bytes[..length]).is_err(),
                    "accepted length {length}"
                );
            }
            let mut trailing = bytes.clone();
            trailing.push(0);
            assert!(decode(&trailing).is_err());
            for bit in 0..bytes.len() * 8 {
                let mut mutated = bytes.clone();
                mutated[bit / 8] ^= 1 << (bit % 8);
                assert!(decode(&mutated).is_err(), "accepted mutation bit {bit}");
            }
        }
    }

    #[test]
    fn duplicate_zero_and_noncanonical_state_identities_are_rejected() {
        let original = state(vec![identity(1), identity(4)]).encode().unwrap();
        let mut zero_namespace = original.clone();
        zero_namespace[32..64].fill(0);
        repair_checksum(&mut zero_namespace, STATE_CHECKSUM_DOMAIN_V1);
        assert_eq!(
            FreshnessStateV1::decode(&zero_namespace),
            Err(PersistentFreshnessRecordErrorV1::ZeroRecordIdentity {
                field: "ledger namespace",
            })
        );

        for (field, destination_offset, source_offset) in [
            (PersistentFreshnessIdentityFieldV1::Challenge, 160, 64),
            (PersistentFreshnessIdentityFieldV1::Transcript, 192, 96),
            (PersistentFreshnessIdentityFieldV1::Result, 224, 128),
        ] {
            let mut duplicate = original.clone();
            duplicate.copy_within(source_offset..source_offset + 32, destination_offset);
            repair_checksum(&mut duplicate, STATE_CHECKSUM_DOMAIN_V1);
            assert_eq!(
                FreshnessStateV1::decode(&duplicate),
                Err(PersistentFreshnessRecordErrorV1::DuplicateIdentity { field })
            );
        }

        for (field, offset) in [
            (PersistentFreshnessIdentityFieldV1::Challenge, 64),
            (PersistentFreshnessIdentityFieldV1::Transcript, 96),
            (PersistentFreshnessIdentityFieldV1::Result, 128),
        ] {
            let mut zero = original.clone();
            zero[offset..offset + 32].fill(0);
            repair_checksum(&mut zero, STATE_CHECKSUM_DOMAIN_V1);
            assert_eq!(
                FreshnessStateV1::decode(&zero),
                Err(PersistentFreshnessRecordErrorV1::ZeroIdentity { field })
            );
        }

        let mut reordered = original;
        let first = reordered[64..160].to_vec();
        let second = reordered[160..256].to_vec();
        reordered[64..160].copy_from_slice(&second);
        reordered[160..256].copy_from_slice(&first);
        repair_checksum(&mut reordered, STATE_CHECKSUM_DOMAIN_V1);
        assert_eq!(
            FreshnessStateV1::decode(&reordered),
            Err(PersistentFreshnessRecordErrorV1::NonCanonical)
        );
    }

    #[test]
    fn unknown_fields_counts_and_generations_fail_closed() {
        let original = state(vec![identity(1)]).encode().unwrap();
        for (offset, value, expected) in [
            (
                8,
                2_u16,
                PersistentFreshnessRecordErrorV1::UnknownVersion(2),
            ),
            (
                10,
                1,
                PersistentFreshnessRecordErrorV1::UnknownField {
                    field: "state flags",
                    value: 1,
                },
            ),
            (
                28,
                0,
                PersistentFreshnessRecordErrorV1::UnknownDigestAlgorithm(0),
            ),
        ] {
            let mut changed = original.clone();
            changed[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
            assert_eq!(FreshnessStateV1::decode(&changed), Err(expected));
        }

        let mut too_many = original.clone();
        too_many[24..28]
            .copy_from_slice(&((MAX_PERSISTENT_FRESHNESS_ENTRIES_V1 as u32) + 1).to_le_bytes());
        assert_eq!(
            FreshnessStateV1::decode(&too_many),
            Err(PersistentFreshnessRecordErrorV1::TooManyEntries {
                count: MAX_PERSISTENT_FRESHNESS_ENTRIES_V1 as u64 + 1,
                max: MAX_PERSISTENT_FRESHNESS_ENTRIES_V1,
            })
        );

        let mut wrong_generation = original;
        wrong_generation[16..24].copy_from_slice(&2_u64.to_le_bytes());
        repair_checksum(&mut wrong_generation, STATE_CHECKSUM_DOMAIN_V1);
        assert_eq!(
            FreshnessStateV1::decode(&wrong_generation),
            Err(PersistentFreshnessRecordErrorV1::GenerationCountMismatch {
                generation: 2,
                count: 1,
            })
        );
    }

    #[test]
    fn intent_rejects_zero_unknown_and_discontinuous_fields() {
        let original = intent().encode().unwrap();
        let mut zero_namespace = original.clone();
        zero_namespace[36..68].fill(0);
        repair_checksum(&mut zero_namespace, INTENT_CHECKSUM_DOMAIN_V1);
        assert_eq!(
            FreshnessIntentV1::decode(&zero_namespace),
            Err(PersistentFreshnessRecordErrorV1::ZeroRecordIdentity {
                field: "ledger namespace",
            })
        );

        let mut zero_state = original.clone();
        zero_state[68..100].fill(0);
        repair_checksum(&mut zero_state, INTENT_CHECKSUM_DOMAIN_V1);
        assert_eq!(
            FreshnessIntentV1::decode(&zero_state),
            Err(PersistentFreshnessRecordErrorV1::ZeroRecordIdentity {
                field: "previous state identity",
            })
        );

        let mut zero_result = original.clone();
        zero_result[196..228].fill(0);
        repair_checksum(&mut zero_result, INTENT_CHECKSUM_DOMAIN_V1);
        assert_eq!(
            FreshnessIntentV1::decode(&zero_result),
            Err(PersistentFreshnessRecordErrorV1::ZeroIdentity {
                field: PersistentFreshnessIdentityFieldV1::Result,
            })
        );

        let mut discontinuous = original.clone();
        discontinuous[24..32].copy_from_slice(&3_u64.to_le_bytes());
        repair_checksum(&mut discontinuous, INTENT_CHECKSUM_DOMAIN_V1);
        assert_eq!(
            FreshnessIntentV1::decode(&discontinuous),
            Err(PersistentFreshnessRecordErrorV1::InvalidIntentGeneration {
                previous: 1,
                next: 3,
            })
        );
    }
}
