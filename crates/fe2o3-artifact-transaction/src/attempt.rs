use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

const ATTEMPT_MAGIC: &[u8] = b"FE2O3-ATTEMPTS-V1\0";
const MAX_ATTEMPT_RECORDS: usize = 1024;
pub(crate) const MAX_ATTEMPT_BYTES: usize = 1024 * 1024;
const MAX_STABLE_SOURCE_BYTES: usize = 4096;
const MAX_CRATE_NAME_BYTES: usize = 128;
const ATTEMPT_HEADER_BYTES: usize = ATTEMPT_MAGIC.len() + 8 + 4;
const ATTEMPT_RECORD_FIXED_BYTES: usize = 2 + 2 + 32 + 8 + 16 + 1 + 1;
const BACKEND_RECEIPT_NONE: u8 = 0;
const BACKEND_RECEIPT_LEGACY: u8 = 1;
const BACKEND_RECEIPT_PROVENANCE_V1: u8 = 2;
const BACKEND_RECEIPT_PENDING_PROVENANCE_V1: u8 = 3;
const BACKEND_PROVENANCE_RECEIPT_BYTES: usize = 7 * 32;

/// Process-independent identity shared by the cooperating processes in one build.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BuildSession([u8; 16]);

impl BuildSession {
    /// The all-zero session reserved for direct compiler invocations.
    pub const DIRECT: Self = Self([0; 16]);

    /// Constructs a session from its exact 128-bit representation.
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Returns the exact 128-bit session representation.
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Encodes the session as exactly 32 lowercase hexadecimal digits.
    pub fn to_hex(self) -> String {
        encode_hex(&self.0)
    }

    /// Decodes exactly 32 lowercase hexadecimal digits.
    pub fn from_hex(encoded: &str) -> Result<Self, AttemptCodecError> {
        if encoded.len() != 32
            || !encoded
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(AttemptCodecError::InvalidSessionEncoding);
        }

        let mut bytes = [0; 16];
        for (index, pair) in encoded.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = (hex_value(pair[0]) << 4) | hex_value(pair[1]);
        }
        Ok(Self(bytes))
    }
}

/// Exact identity of one rustc invocation and artifact output domain.
///
/// Callers should derive this value from a canonical fingerprint covering the package and target,
/// crate disambiguator, target architecture, relevant cfg/codegen inputs, and output domain. A
/// different invocation must use a different value even when it compiles the same source file.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BuildInvocation([u8; 32]);

impl BuildInvocation {
    pub(crate) const DIRECT: Self = Self([0; 32]);

    /// Constructs an invocation identity from its exact 256-bit fingerprint.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the exact 256-bit invocation fingerprint.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Encodes the invocation fingerprint as exactly 64 lowercase hexadecimal digits.
    pub fn to_hex(self) -> String {
        encode_hex(&self.0)
    }

    /// Decodes exactly 64 lowercase hexadecimal digits.
    pub fn from_hex(encoded: &str) -> Result<Self, AttemptCodecError> {
        if encoded.len() != 64 || !is_lower_hex(encoded) {
            return Err(AttemptCodecError::InvalidInvocationEncoding);
        }
        let mut bytes = [0; 32];
        decode_hex(encoded, &mut bytes);
        Ok(Self(bytes))
    }
}

impl fmt::Display for BuildInvocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

impl FromStr for BuildInvocation {
    type Err = AttemptCodecError;

    fn from_str(encoded: &str) -> Result<Self, Self::Err> {
        Self::from_hex(encoded)
    }
}

impl fmt::Display for BuildSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

impl FromStr for BuildSession {
    type Err = AttemptCodecError;

    fn from_str(encoded: &str) -> Result<Self, Self::Err> {
        Self::from_hex(encoded)
    }
}

/// Durable generation token authorizing one exact rustc invocation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BuildAttempt {
    generation: u64,
    session: BuildSession,
    invocation: BuildInvocation,
}

/// Durable provenance fields bound to one successful exact-byte backend publication.
///
/// This receipt is coordination evidence, not proof that the code object passed semantic, ABI,
/// memory-safety, or launch-safety admission. In particular, `upstream_evidence_identity` records
/// an identity supplied by the caller; this crate does not validate the evidence behind it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BackendPublicationReceiptV1 {
    attempt_identity: [u8; 32],
    producer_identity: [u8; 32],
    scope_identity: [u8; 32],
    plan_commitment: [u8; 32],
    upstream_evidence_identity: [u8; 32],
    finalized_output_identity: [u8; 32],
    publication_identity: [u8; 32],
}

impl BackendPublicationReceiptV1 {
    pub(crate) const fn new(
        attempt_identity: [u8; 32],
        producer_identity: [u8; 32],
        scope_identity: [u8; 32],
        plan_commitment: [u8; 32],
        upstream_evidence_identity: [u8; 32],
        finalized_output_identity: [u8; 32],
        publication_identity: [u8; 32],
    ) -> Self {
        Self {
            attempt_identity,
            producer_identity,
            scope_identity,
            plan_commitment,
            upstream_evidence_identity,
            finalized_output_identity,
            publication_identity,
        }
    }

    /// Returns the canonical identity of the exact build attempt.
    pub const fn attempt_identity(self) -> [u8; 32] {
        self.attempt_identity
    }

    /// Returns the canonical identity of the producer source and crate name.
    pub const fn producer_identity(self) -> [u8; 32] {
        self.producer_identity
    }

    /// Returns the canonical package, kernel-set, and target scope identity.
    pub const fn scope_identity(self) -> [u8; 32] {
        self.scope_identity
    }

    /// Returns the commitment to every field in the durable publication plan.
    pub const fn plan_commitment(self) -> [u8; 32] {
        self.plan_commitment
    }

    /// Returns the caller-supplied upstream evidence identity.
    pub const fn upstream_evidence_identity(self) -> [u8; 32] {
        self.upstream_evidence_identity
    }

    /// Returns the SHA-256 identity of the exact finalized bytes.
    pub const fn finalized_output_identity(self) -> [u8; 32] {
        self.finalized_output_identity
    }

    /// Returns the atomic publication identity committed by the durable plan.
    pub const fn publication_identity(self) -> [u8; 32] {
        self.publication_identity
    }
}

impl BuildAttempt {
    fn new(
        generation: u64,
        session: BuildSession,
        invocation: BuildInvocation,
    ) -> Result<Self, AttemptCodecError> {
        if generation == 0 {
            return Err(AttemptCodecError::ZeroGeneration);
        }
        if (session == BuildSession::DIRECT) != (invocation == BuildInvocation::DIRECT) {
            return Err(AttemptCodecError::InvalidAttemptEncoding);
        }
        Ok(Self {
            generation,
            session,
            invocation,
        })
    }

    /// Returns the nonzero durable generation number.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Returns the build session that owns this attempt.
    pub const fn session(self) -> BuildSession {
        self.session
    }

    /// Returns the exact rustc invocation identity authorized by this token.
    pub const fn invocation(self) -> BuildInvocation {
        self.invocation
    }

    /// Encodes the canonical environment value `generation:session:invocation`.
    pub fn to_env_value(self) -> String {
        format!("{}:{}:{}", self.generation, self.session, self.invocation)
    }

    /// Parses a canonical environment value `generation:session:invocation`.
    pub fn from_env_value(value: &str) -> Result<Self, AttemptCodecError> {
        let mut fields = value.split(':');
        let (Some(generation), Some(session), Some(invocation), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return Err(AttemptCodecError::InvalidAttemptEncoding);
        };
        if generation.is_empty()
            || (generation.len() > 1 && generation.starts_with('0'))
            || !generation.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(AttemptCodecError::InvalidAttemptEncoding);
        }
        let generation = generation
            .parse::<u64>()
            .map_err(|_| AttemptCodecError::InvalidAttemptEncoding)?;
        let session = BuildSession::from_hex(session)
            .map_err(|_| AttemptCodecError::InvalidAttemptEncoding)?;
        let invocation = BuildInvocation::from_hex(invocation)
            .map_err(|_| AttemptCodecError::InvalidAttemptEncoding)?;
        Self::new(generation, session, invocation)
            .map_err(|_| AttemptCodecError::InvalidAttemptEncoding)
    }
}

impl fmt::Display for BuildAttempt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_env_value())
    }
}

impl FromStr for BuildAttempt {
    type Err = AttemptCodecError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::from_env_value(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AttemptPhase {
    Invalidating,
    Building,
    BackendClaimed,
    Failed,
    Completed,
}

impl AttemptPhase {
    fn encode(self) -> u8 {
        match self {
            Self::Invalidating => 0,
            Self::Building => 1,
            Self::Failed => 2,
            Self::Completed => 3,
            Self::BackendClaimed => 4,
        }
    }

    fn decode(value: u8) -> Result<Self, AttemptCodecError> {
        match value {
            0 => Ok(Self::Invalidating),
            1 => Ok(Self::Building),
            2 => Ok(Self::Failed),
            3 => Ok(Self::Completed),
            4 => Ok(Self::BackendClaimed),
            _ => Err(AttemptCodecError::InvalidPhase(value)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AttemptRecord {
    pub(crate) crate_name: String,
    pub(crate) invocation: BuildInvocation,
    pub(crate) generation: u64,
    pub(crate) session: BuildSession,
    pub(crate) phase: AttemptPhase,
    pub(crate) backend_receipt: Option<BackendReceiptV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackendReceiptV1 {
    LegacyCoordination,
    PendingProvenance(BackendPublicationReceiptV1),
    Provenance(BackendPublicationReceiptV1),
}

impl BackendReceiptV1 {
    pub(crate) const fn is_completed(self) -> bool {
        matches!(self, Self::LegacyCoordination | Self::Provenance(_))
    }
}

impl AttemptRecord {
    fn attempt(&self) -> BuildAttempt {
        BuildAttempt {
            generation: self.generation,
            session: self.session,
            invocation: self.invocation,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StartAttemptOutcome {
    New(BuildAttempt),
    ResumeInvalidating(BuildAttempt),
    ReuseBuilding(BuildAttempt),
}

/// A bounded registry error or noncanonical codec input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttemptCodecError {
    InvalidSessionEncoding,
    InvalidInvocationEncoding,
    InvalidAttemptEncoding,
    InvalidStableSource,
    InvalidCrateName,
    BadMagic,
    Truncated,
    TrailingBytes,
    InvalidUtf8,
    InvalidTextLength,
    InvalidPhase(u8),
    InvalidBackendReceiptTag(u8),
    ZeroGeneration,
    GenerationBeyondWatermark,
    DuplicateGeneration,
    DuplicateSource,
    TooManyRecords,
    RegistryTooLarge,
    NonCanonical,
    SameSessionFailed,
    SameSessionCompleted,
    BackendAlreadySeen,
    GenerationExhausted,
    MissingSource,
    AttemptMismatch,
    InvalidTransition,
}

impl fmt::Display for AttemptCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidSessionEncoding => {
                "build session must be exactly 32 lowercase hexadecimal digits"
            }
            Self::InvalidInvocationEncoding => {
                "build invocation must be exactly 64 lowercase hexadecimal digits"
            }
            Self::InvalidAttemptEncoding => {
                "build attempt must contain a canonical generation, session, and invocation"
            }
            Self::InvalidStableSource => "invalid stable producer source",
            Self::InvalidCrateName => "invalid diagnostic crate name",
            Self::BadMagic => "bad attempt registry magic",
            Self::Truncated => "truncated attempt registry",
            Self::TrailingBytes => "trailing attempt registry bytes",
            Self::InvalidUtf8 => "attempt registry text is not UTF-8",
            Self::InvalidTextLength => "invalid attempt registry text length",
            Self::InvalidPhase(_) => "invalid attempt phase",
            Self::InvalidBackendReceiptTag(_) => "invalid backend receipt tag",
            Self::ZeroGeneration => "attempt generation is zero",
            Self::GenerationBeyondWatermark => "attempt generation exceeds registry watermark",
            Self::DuplicateGeneration => "duplicate active attempt generation",
            Self::DuplicateSource => "duplicate stable producer source",
            Self::TooManyRecords => "too many active build attempts",
            Self::RegistryTooLarge => "canonical attempt registry exceeds its byte bound",
            Self::NonCanonical => "attempt registry is not canonical",
            Self::SameSessionFailed => "this build session already failed for the source",
            Self::SameSessionCompleted => "this build session already completed for the source",
            Self::BackendAlreadySeen => "the backend already published for this build attempt",
            Self::GenerationExhausted => "attempt generation space is exhausted",
            Self::MissingSource => "stable producer source has no build attempt",
            Self::AttemptMismatch => "build attempt token does not match the source",
            Self::InvalidTransition => "build attempt is in the wrong phase",
        };
        match self {
            Self::InvalidPhase(value) | Self::InvalidBackendReceiptTag(value) => {
                write!(formatter, "{message}: {value}")
            }
            _ => formatter.write_str(message),
        }
    }
}

impl std::error::Error for AttemptCodecError {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct AttemptRegistry {
    last_issued_generation: u64,
    records: BTreeMap<String, AttemptRecord>,
}

impl AttemptRegistry {
    pub(crate) fn encode(&self) -> Result<Vec<u8>, AttemptCodecError> {
        self.validate()?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(ATTEMPT_MAGIC);
        bytes.extend_from_slice(&self.last_issued_generation.to_le_bytes());
        push_u32(&mut bytes, self.records.len())?;
        for (source, record) in &self.records {
            push_text(&mut bytes, source)?;
            push_text(&mut bytes, &record.crate_name)?;
            bytes.extend_from_slice(record.invocation.as_bytes());
            bytes.extend_from_slice(&record.generation.to_le_bytes());
            bytes.extend_from_slice(record.session.as_bytes());
            bytes.push(record.phase.encode());
            match record.backend_receipt {
                None => bytes.push(BACKEND_RECEIPT_NONE),
                Some(BackendReceiptV1::LegacyCoordination) => {
                    bytes.push(BACKEND_RECEIPT_LEGACY);
                }
                Some(BackendReceiptV1::Provenance(receipt)) => {
                    bytes.push(BACKEND_RECEIPT_PROVENANCE_V1);
                    bytes.extend_from_slice(&receipt.attempt_identity);
                    bytes.extend_from_slice(&receipt.producer_identity);
                    bytes.extend_from_slice(&receipt.scope_identity);
                    bytes.extend_from_slice(&receipt.plan_commitment);
                    bytes.extend_from_slice(&receipt.upstream_evidence_identity);
                    bytes.extend_from_slice(&receipt.finalized_output_identity);
                    bytes.extend_from_slice(&receipt.publication_identity);
                }
                Some(BackendReceiptV1::PendingProvenance(receipt)) => {
                    bytes.push(BACKEND_RECEIPT_PENDING_PROVENANCE_V1);
                    bytes.extend_from_slice(&receipt.attempt_identity);
                    bytes.extend_from_slice(&receipt.producer_identity);
                    bytes.extend_from_slice(&receipt.scope_identity);
                    bytes.extend_from_slice(&receipt.plan_commitment);
                    bytes.extend_from_slice(&receipt.upstream_evidence_identity);
                    bytes.extend_from_slice(&receipt.finalized_output_identity);
                    bytes.extend_from_slice(&receipt.publication_identity);
                }
            }
        }
        if bytes.len() > MAX_ATTEMPT_BYTES {
            return Err(AttemptCodecError::RegistryTooLarge);
        }
        Ok(bytes)
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, AttemptCodecError> {
        if bytes.len() > MAX_ATTEMPT_BYTES {
            return Err(AttemptCodecError::RegistryTooLarge);
        }

        let mut decoder = AttemptDecoder::new(bytes);
        if decoder.take(ATTEMPT_MAGIC.len())? != ATTEMPT_MAGIC {
            return Err(AttemptCodecError::BadMagic);
        }
        let last_issued_generation = decoder.u64()?;
        let record_count = decoder.u32()? as usize;
        if record_count > MAX_ATTEMPT_RECORDS {
            return Err(AttemptCodecError::TooManyRecords);
        }

        let mut records = BTreeMap::new();
        for _ in 0..record_count {
            let source = decoder.text(MAX_STABLE_SOURCE_BYTES)?;
            validate_stable_source(&source)?;
            let crate_name = decoder.text(MAX_CRATE_NAME_BYTES)?;
            validate_crate_name(&crate_name)?;
            let mut invocation = [0; 32];
            invocation.copy_from_slice(decoder.take(32)?);
            let generation = decoder.u64()?;
            let mut session = [0; 16];
            session.copy_from_slice(decoder.take(16)?);
            let phase = AttemptPhase::decode(decoder.byte()?)?;
            let backend_receipt = match decoder.byte()? {
                BACKEND_RECEIPT_NONE => None,
                BACKEND_RECEIPT_LEGACY => Some(BackendReceiptV1::LegacyCoordination),
                BACKEND_RECEIPT_PROVENANCE_V1 => Some(BackendReceiptV1::Provenance(
                    BackendPublicationReceiptV1::new(
                        decoder.fixed()?,
                        decoder.fixed()?,
                        decoder.fixed()?,
                        decoder.fixed()?,
                        decoder.fixed()?,
                        decoder.fixed()?,
                        decoder.fixed()?,
                    ),
                )),
                BACKEND_RECEIPT_PENDING_PROVENANCE_V1 => Some(BackendReceiptV1::PendingProvenance(
                    BackendPublicationReceiptV1::new(
                        decoder.fixed()?,
                        decoder.fixed()?,
                        decoder.fixed()?,
                        decoder.fixed()?,
                        decoder.fixed()?,
                        decoder.fixed()?,
                        decoder.fixed()?,
                    ),
                )),
                value => return Err(AttemptCodecError::InvalidBackendReceiptTag(value)),
            };
            if records
                .insert(
                    source,
                    AttemptRecord {
                        crate_name,
                        invocation: BuildInvocation::from_bytes(invocation),
                        generation,
                        session: BuildSession::from_bytes(session),
                        phase,
                        backend_receipt,
                    },
                )
                .is_some()
            {
                return Err(AttemptCodecError::DuplicateSource);
            }
        }
        if !decoder.is_finished() {
            return Err(AttemptCodecError::TrailingBytes);
        }

        let registry = Self {
            last_issued_generation,
            records,
        };
        registry.validate()?;
        if registry.encode()? != bytes {
            return Err(AttemptCodecError::NonCanonical);
        }
        Ok(registry)
    }

    pub(crate) fn record(&self, stable_source: &str) -> Option<&AttemptRecord> {
        self.records.get(stable_source)
    }

    pub(crate) fn record_for_attempt(
        &self,
        attempt: BuildAttempt,
    ) -> Option<(&str, &AttemptRecord)> {
        self.records.iter().find_map(|(source, record)| {
            (record.attempt() == attempt).then_some((source.as_str(), record))
        })
    }

    pub(crate) fn start_or_resume(
        &mut self,
        stable_source: &str,
        crate_name: &str,
        invocation: BuildInvocation,
        session: BuildSession,
    ) -> Result<StartAttemptOutcome, AttemptCodecError> {
        validate_stable_source(stable_source)?;
        validate_crate_name(crate_name)?;
        if invocation == BuildInvocation::DIRECT {
            return Err(AttemptCodecError::InvalidInvocationEncoding);
        }

        if let Some(record) = self.records.get(stable_source)
            && record.session == session
            && record.invocation == invocation
        {
            let attempt = record.attempt();
            return match record.phase {
                AttemptPhase::Invalidating => Ok(StartAttemptOutcome::ResumeInvalidating(attempt)),
                AttemptPhase::Building => Ok(StartAttemptOutcome::ReuseBuilding(attempt)),
                AttemptPhase::BackendClaimed => Err(AttemptCodecError::BackendAlreadySeen),
                AttemptPhase::Failed => Err(AttemptCodecError::SameSessionFailed),
                AttemptPhase::Completed => Err(AttemptCodecError::SameSessionCompleted),
            };
        }

        let attempt = self.next_attempt(session, invocation)?;
        if !self.records.contains_key(stable_source) && self.records.len() == MAX_ATTEMPT_RECORDS {
            return Err(AttemptCodecError::TooManyRecords);
        }
        self.ensure_insert_fits(stable_source, crate_name)?;
        let record = AttemptRecord {
            crate_name: crate_name.to_string(),
            invocation,
            generation: attempt.generation,
            session,
            phase: AttemptPhase::Invalidating,
            backend_receipt: None,
        };
        self.last_issued_generation = attempt.generation;
        self.records.insert(stable_source.to_string(), record);
        Ok(StartAttemptOutcome::New(attempt))
    }

    pub(crate) fn transition_building(
        &mut self,
        stable_source: &str,
        attempt: BuildAttempt,
    ) -> Result<(), AttemptCodecError> {
        let record = self.exact_record_mut(stable_source, attempt)?;
        if record.phase != AttemptPhase::Invalidating || record.backend_receipt.is_some() {
            return Err(AttemptCodecError::InvalidTransition);
        }
        record.phase = AttemptPhase::Building;
        Ok(())
    }

    pub(crate) fn claim_backend(
        &mut self,
        stable_source: &str,
        attempt: BuildAttempt,
    ) -> Result<(), AttemptCodecError> {
        let record = self.exact_record_mut(stable_source, attempt)?;
        if record.backend_receipt.is_some() {
            return Err(AttemptCodecError::BackendAlreadySeen);
        }
        if record.phase != AttemptPhase::Building {
            return Err(AttemptCodecError::InvalidTransition);
        }
        record.phase = AttemptPhase::BackendClaimed;
        Ok(())
    }

    pub(crate) fn claim_backend_with_pending_receipt(
        &mut self,
        stable_source: &str,
        attempt: BuildAttempt,
        receipt: BackendPublicationReceiptV1,
    ) -> Result<(), AttemptCodecError> {
        let record = self.exact_record_mut(stable_source, attempt)?;
        if record.backend_receipt.is_some() {
            return Err(AttemptCodecError::BackendAlreadySeen);
        }
        if record.phase != AttemptPhase::Building {
            return Err(AttemptCodecError::InvalidTransition);
        }
        record.phase = AttemptPhase::BackendClaimed;
        record.backend_receipt = Some(BackendReceiptV1::PendingProvenance(receipt));
        Ok(())
    }

    pub(crate) fn record_legacy_backend_receipt(
        &mut self,
        stable_source: &str,
        attempt: BuildAttempt,
    ) -> Result<(), AttemptCodecError> {
        let record = self.exact_record_mut(stable_source, attempt)?;
        if record.backend_receipt.is_some() {
            return Err(AttemptCodecError::BackendAlreadySeen);
        }
        if record.phase != AttemptPhase::BackendClaimed {
            return Err(AttemptCodecError::InvalidTransition);
        }
        record.backend_receipt = Some(BackendReceiptV1::LegacyCoordination);
        Ok(())
    }

    pub(crate) fn record_backend_publication_receipt(
        &mut self,
        stable_source: &str,
        attempt: BuildAttempt,
        receipt: BackendPublicationReceiptV1,
    ) -> Result<(), AttemptCodecError> {
        let record = self.exact_record_mut(stable_source, attempt)?;
        match record.backend_receipt {
            Some(BackendReceiptV1::PendingProvenance(pending)) if pending == receipt => {
                record.backend_receipt = Some(BackendReceiptV1::Provenance(receipt));
                Ok(())
            }
            Some(BackendReceiptV1::Provenance(existing)) if existing == receipt => Ok(()),
            Some(_) => Err(AttemptCodecError::BackendAlreadySeen),
            None => Err(AttemptCodecError::InvalidTransition),
        }
    }

    pub(crate) fn mark_failed(
        &mut self,
        stable_source: &str,
        attempt: BuildAttempt,
    ) -> Result<(), AttemptCodecError> {
        let record = self.exact_record_mut(stable_source, attempt)?;
        if record.phase == AttemptPhase::Completed {
            return Err(AttemptCodecError::InvalidTransition);
        }
        if record.phase == AttemptPhase::Failed {
            return Ok(());
        }
        if matches!(
            record.backend_receipt,
            Some(BackendReceiptV1::PendingProvenance(_))
        ) {
            record.backend_receipt = None;
        }
        record.phase = AttemptPhase::Failed;
        Ok(())
    }

    pub(crate) fn mark_completed(
        &mut self,
        stable_source: &str,
        attempt: BuildAttempt,
    ) -> Result<(), AttemptCodecError> {
        let record = self.exact_record_mut(stable_source, attempt)?;
        if record.phase == AttemptPhase::Completed {
            return if record
                .backend_receipt
                .is_some_and(BackendReceiptV1::is_completed)
            {
                Ok(())
            } else {
                Err(AttemptCodecError::InvalidTransition)
            };
        }
        if record.phase != AttemptPhase::BackendClaimed
            || !record
                .backend_receipt
                .is_some_and(BackendReceiptV1::is_completed)
        {
            return Err(AttemptCodecError::InvalidTransition);
        }
        record.phase = AttemptPhase::Completed;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn remove_session(&mut self, session: BuildSession) -> usize {
        let previous_len = self.records.len();
        self.records.retain(|_, record| record.session != session);
        previous_len - self.records.len()
    }

    pub(crate) fn authorize_backend(
        &self,
        stable_source: &str,
        attempt: BuildAttempt,
    ) -> Result<&AttemptRecord, AttemptCodecError> {
        let record = self.record_exact(stable_source, attempt)?;
        if record.phase == AttemptPhase::BackendClaimed {
            return Err(AttemptCodecError::BackendAlreadySeen);
        }
        if record.phase != AttemptPhase::Building {
            return Err(AttemptCodecError::InvalidTransition);
        }
        if record.backend_receipt.is_some() {
            return Err(AttemptCodecError::BackendAlreadySeen);
        }
        Ok(record)
    }

    pub(crate) fn allocate_direct(
        &mut self,
        stable_source: &str,
        crate_name: &str,
    ) -> Result<BuildAttempt, AttemptCodecError> {
        validate_stable_source(stable_source)?;
        validate_crate_name(crate_name)?;
        let attempt = self.next_attempt(BuildSession::DIRECT, BuildInvocation::DIRECT)?;
        if !self.records.contains_key(stable_source) && self.records.len() == MAX_ATTEMPT_RECORDS {
            return Err(AttemptCodecError::TooManyRecords);
        }
        self.ensure_insert_fits(stable_source, crate_name)?;
        self.last_issued_generation = attempt.generation;
        self.records.insert(
            stable_source.to_string(),
            AttemptRecord {
                crate_name: crate_name.to_string(),
                invocation: BuildInvocation::DIRECT,
                generation: attempt.generation,
                session: BuildSession::DIRECT,
                phase: AttemptPhase::Invalidating,
                backend_receipt: None,
            },
        );
        Ok(attempt)
    }

    fn next_attempt(
        &self,
        session: BuildSession,
        invocation: BuildInvocation,
    ) -> Result<BuildAttempt, AttemptCodecError> {
        let generation = self
            .last_issued_generation
            .checked_add(1)
            .ok_or(AttemptCodecError::GenerationExhausted)?;
        BuildAttempt::new(generation, session, invocation)
    }

    fn ensure_insert_fits(
        &self,
        stable_source: &str,
        crate_name: &str,
    ) -> Result<(), AttemptCodecError> {
        let current_size = self.canonical_size()?;
        let replaced_size = self.records.get(stable_source).map_or(0, |record| {
            record_size(stable_source, &record.crate_name, record.backend_receipt)
        });
        let next_size = current_size
            .checked_sub(replaced_size)
            .and_then(|size| size.checked_add(record_size(stable_source, crate_name, None)))
            .ok_or(AttemptCodecError::RegistryTooLarge)?;
        if next_size > MAX_ATTEMPT_BYTES {
            return Err(AttemptCodecError::RegistryTooLarge);
        }
        Ok(())
    }

    fn canonical_size(&self) -> Result<usize, AttemptCodecError> {
        self.records
            .iter()
            .try_fold(ATTEMPT_HEADER_BYTES, |size, (source, record)| {
                size.checked_add(record_size(
                    source,
                    &record.crate_name,
                    record.backend_receipt,
                ))
                .ok_or(AttemptCodecError::RegistryTooLarge)
            })
    }

    pub(crate) fn record_exact(
        &self,
        stable_source: &str,
        attempt: BuildAttempt,
    ) -> Result<&AttemptRecord, AttemptCodecError> {
        let record = self
            .records
            .get(stable_source)
            .ok_or(AttemptCodecError::MissingSource)?;
        if record.generation != attempt.generation
            || record.session != attempt.session
            || record.invocation != attempt.invocation
        {
            return Err(AttemptCodecError::AttemptMismatch);
        }
        Ok(record)
    }

    fn exact_record_mut(
        &mut self,
        stable_source: &str,
        attempt: BuildAttempt,
    ) -> Result<&mut AttemptRecord, AttemptCodecError> {
        let record = self
            .records
            .get_mut(stable_source)
            .ok_or(AttemptCodecError::MissingSource)?;
        if record.generation != attempt.generation
            || record.session != attempt.session
            || record.invocation != attempt.invocation
        {
            return Err(AttemptCodecError::AttemptMismatch);
        }
        Ok(record)
    }

    fn validate(&self) -> Result<(), AttemptCodecError> {
        if self.records.len() > MAX_ATTEMPT_RECORDS {
            return Err(AttemptCodecError::TooManyRecords);
        }
        let mut generations = BTreeSet::new();
        for (source, record) in &self.records {
            validate_stable_source(source)?;
            validate_crate_name(&record.crate_name)?;
            if (record.session == BuildSession::DIRECT)
                != (record.invocation == BuildInvocation::DIRECT)
            {
                return Err(AttemptCodecError::InvalidTransition);
            }
            if record.generation == 0 {
                return Err(AttemptCodecError::ZeroGeneration);
            }
            if record.generation > self.last_issued_generation {
                return Err(AttemptCodecError::GenerationBeyondWatermark);
            }
            if !generations.insert(record.generation) {
                return Err(AttemptCodecError::DuplicateGeneration);
            }
            if ((record.phase == AttemptPhase::Invalidating
                || record.phase == AttemptPhase::Building)
                && record.backend_receipt.is_some())
                || (record.phase == AttemptPhase::Completed
                    && !record
                        .backend_receipt
                        .is_some_and(BackendReceiptV1::is_completed))
            {
                return Err(AttemptCodecError::InvalidTransition);
            }
        }
        if self.canonical_size()? > MAX_ATTEMPT_BYTES {
            return Err(AttemptCodecError::RegistryTooLarge);
        }
        Ok(())
    }
}

fn record_size(
    stable_source: &str,
    crate_name: &str,
    backend_receipt: Option<BackendReceiptV1>,
) -> usize {
    ATTEMPT_RECORD_FIXED_BYTES
        + stable_source.len()
        + crate_name.len()
        + usize::from(matches!(
            backend_receipt,
            Some(BackendReceiptV1::PendingProvenance(_) | BackendReceiptV1::Provenance(_))
        )) * BACKEND_PROVENANCE_RECEIPT_BYTES
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn is_lower_hex(encoded: &str) -> bool {
    encoded
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn decode_hex(encoded: &str, bytes: &mut [u8]) {
    for (index, pair) in encoded.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (hex_value(pair[0]) << 4) | hex_value(pair[1]);
    }
}

fn hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("hex input was validated"),
    }
}

fn validate_stable_source(source: &str) -> Result<(), AttemptCodecError> {
    if source.is_empty()
        || source.len() > MAX_STABLE_SOURCE_BYTES
        || !(source.starts_with("path:") || source.starts_with("crate:"))
        || source.ends_with(':')
        || source.as_bytes().contains(&0)
    {
        return Err(AttemptCodecError::InvalidStableSource);
    }
    Ok(())
}

fn validate_crate_name(crate_name: &str) -> Result<(), AttemptCodecError> {
    if crate_name.is_empty()
        || crate_name.len() > MAX_CRATE_NAME_BYTES
        || !crate_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(AttemptCodecError::InvalidCrateName);
    }
    Ok(())
}

fn push_u32(bytes: &mut Vec<u8>, value: usize) -> Result<(), AttemptCodecError> {
    let value = u32::try_from(value).map_err(|_| AttemptCodecError::TooManyRecords)?;
    bytes.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

fn push_text(bytes: &mut Vec<u8>, text: &str) -> Result<(), AttemptCodecError> {
    let length = u16::try_from(text.len()).map_err(|_| AttemptCodecError::InvalidTextLength)?;
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(text.as_bytes());
    Ok(())
}

struct AttemptDecoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> AttemptDecoder<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], AttemptCodecError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(AttemptCodecError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(AttemptCodecError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn byte(&mut self) -> Result<u8, AttemptCodecError> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, AttemptCodecError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, AttemptCodecError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], AttemptCodecError> {
        self.take(N)?
            .try_into()
            .map_err(|_| AttemptCodecError::Truncated)
    }

    fn text(&mut self, maximum: usize) -> Result<String, AttemptCodecError> {
        let length = u16::from_le_bytes(self.take(2)?.try_into().unwrap()) as usize;
        if length == 0 || length > maximum {
            return Err(AttemptCodecError::InvalidTextLength);
        }
        String::from_utf8(self.take(length)?.to_vec()).map_err(|_| AttemptCodecError::InvalidUtf8)
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SESSION_A: BuildSession = BuildSession::from_bytes([
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff,
    ]);
    const SESSION_B: BuildSession = BuildSession::from_bytes([0x5a; 16]);
    const INVOCATION_A: BuildInvocation = BuildInvocation::from_bytes([0x11; 32]);
    const INVOCATION_B: BuildInvocation = BuildInvocation::from_bytes([0x22; 32]);

    fn start(registry: &mut AttemptRegistry, source: &str, session: BuildSession) -> BuildAttempt {
        match registry
            .start_or_resume(source, "kernel_crate", INVOCATION_A, session)
            .unwrap()
        {
            StartAttemptOutcome::New(attempt) => attempt,
            outcome => panic!("unexpected start outcome: {outcome:?}"),
        }
    }

    fn one_record_bytes() -> Vec<u8> {
        let mut registry = AttemptRegistry::default();
        let attempt = start(&mut registry, "path:a", SESSION_A);
        registry.transition_building("path:a", attempt).unwrap();
        registry.claim_backend("path:a", attempt).unwrap();
        registry
            .record_legacy_backend_receipt("path:a", attempt)
            .unwrap();
        registry.encode().unwrap()
    }

    fn provenance_receipt(seed: u8) -> BackendPublicationReceiptV1 {
        BackendPublicationReceiptV1::new(
            [seed; 32],
            [seed.wrapping_add(1); 32],
            [seed.wrapping_add(2); 32],
            [seed.wrapping_add(3); 32],
            [seed.wrapping_add(4); 32],
            [seed.wrapping_add(5); 32],
            [seed.wrapping_add(6); 32],
        )
    }

    fn raw_registry(
        watermark: u64,
        records: &[(&str, &str, u64, BuildSession, u8, u8)],
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(ATTEMPT_MAGIC);
        bytes.extend_from_slice(&watermark.to_le_bytes());
        bytes.extend_from_slice(&(records.len() as u32).to_le_bytes());
        for (source, crate_name, generation, session, phase, backend_seen) in records {
            bytes.extend_from_slice(&(source.len() as u16).to_le_bytes());
            bytes.extend_from_slice(source.as_bytes());
            bytes.extend_from_slice(&(crate_name.len() as u16).to_le_bytes());
            bytes.extend_from_slice(crate_name.as_bytes());
            bytes.extend_from_slice(INVOCATION_A.as_bytes());
            bytes.extend_from_slice(&generation.to_le_bytes());
            bytes.extend_from_slice(session.as_bytes());
            bytes.push(*phase);
            bytes.push(*backend_seen);
        }
        bytes
    }

    #[test]
    fn build_session_is_strict_lowercase_hex() {
        let encoded = "00112233445566778899aabbccddeeff";
        assert_eq!(SESSION_A.to_hex(), encoded);
        assert_eq!(BuildSession::from_hex(encoded).unwrap(), SESSION_A);
        assert_eq!(encoded.parse::<BuildSession>().unwrap(), SESSION_A);
        assert_eq!(SESSION_A.to_string(), encoded);
        assert_eq!(
            SESSION_A.as_bytes(),
            &[
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff,
            ]
        );

        for invalid in [
            "",
            "00112233445566778899aabbccddeef",
            "00112233445566778899aabbccddeeff0",
            "00112233445566778899AABBCCDDEEFF",
            "00112233445566778899aabbccddee g",
            "00112233445566778899aabbccddee:f",
        ] {
            assert_eq!(
                BuildSession::from_hex(invalid),
                Err(AttemptCodecError::InvalidSessionEncoding)
            );
        }
    }

    #[test]
    fn build_invocation_is_strict_lowercase_hex() {
        let encoded = "1111111111111111111111111111111111111111111111111111111111111111";
        assert_eq!(INVOCATION_A.to_hex(), encoded);
        assert_eq!(BuildInvocation::from_hex(encoded).unwrap(), INVOCATION_A);
        assert_eq!(encoded.parse::<BuildInvocation>().unwrap(), INVOCATION_A);
        assert_eq!(INVOCATION_A.to_string(), encoded);
        assert_eq!(INVOCATION_A.as_bytes(), &[0x11; 32]);

        for invalid in [
            "",
            "111111111111111111111111111111111111111111111111111111111111111",
            "11111111111111111111111111111111111111111111111111111111111111111",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "111111111111111111111111111111111111111111111111111111111111111g",
        ] {
            assert_eq!(
                BuildInvocation::from_hex(invalid),
                Err(AttemptCodecError::InvalidInvocationEncoding)
            );
        }
    }

    #[test]
    fn build_attempt_env_value_is_strict_and_canonical() {
        let attempt = BuildAttempt::new(42, SESSION_A, INVOCATION_A).unwrap();
        let encoded = "42:00112233445566778899aabbccddeeff:1111111111111111111111111111111111111111111111111111111111111111";
        assert_eq!(attempt.to_env_value(), encoded);
        assert_eq!(BuildAttempt::from_env_value(encoded).unwrap(), attempt);
        assert_eq!(encoded.parse::<BuildAttempt>().unwrap(), attempt);
        assert_eq!(attempt.to_string(), encoded);
        assert_eq!(attempt.generation(), 42);
        assert_eq!(attempt.session(), SESSION_A);
        assert_eq!(attempt.invocation(), INVOCATION_A);
        assert_eq!(
            BuildAttempt::new(1, SESSION_A, BuildInvocation::DIRECT),
            Err(AttemptCodecError::InvalidAttemptEncoding)
        );
        assert_eq!(
            BuildAttempt::new(1, BuildSession::DIRECT, INVOCATION_A),
            Err(AttemptCodecError::InvalidAttemptEncoding)
        );

        for invalid in [
            "",
            ":00112233445566778899aabbccddeeff",
            "0:00112233445566778899aabbccddeeff",
            "01:00112233445566778899aabbccddeeff",
            "+1:00112233445566778899aabbccddeeff",
            "1:00112233445566778899AABBCCDDEEFF",
            "1:00112233445566778899aabbccddeeff",
            "1:00112233445566778899aabbccddeeff:extra",
            "1:00112233445566778899aabbccddeeff:1111111111111111111111111111111111111111111111111111111111111111:extra",
            "18446744073709551616:00112233445566778899aabbccddeeff",
        ] {
            assert_eq!(
                BuildAttempt::from_env_value(invalid),
                Err(AttemptCodecError::InvalidAttemptEncoding),
                "{invalid}"
            );
        }
    }

    #[test]
    fn codec_has_stable_golden_bytes() {
        let actual = one_record_bytes();
        let expected = b"FE2O3-ATTEMPTS-V1\0\
            \x01\x00\x00\x00\x00\x00\x00\x00\
            \x01\x00\x00\x00\
            \x06\x00path:a\
            \x0c\x00kernel_crate\
            \x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\
            \x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\
            \x01\x00\x00\x00\x00\x00\x00\x00\
            \x00\x11\x22\x33\x44\x55\x66\x77\x88\x99\xaa\xbb\xcc\xdd\xee\xff\
            \x04\x01";
        assert_eq!(actual, expected);
        assert_eq!(
            AttemptRegistry::decode(expected).unwrap().encode().unwrap(),
            expected
        );
    }

    #[test]
    fn completed_phase_has_stable_canonical_tag() {
        let expected = b"FE2O3-ATTEMPTS-V1\0\
            \x01\x00\x00\x00\x00\x00\x00\x00\
            \x01\x00\x00\x00\
            \x06\x00path:a\
            \x0c\x00kernel_crate\
            \x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\
            \x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\
            \x01\x00\x00\x00\x00\x00\x00\x00\
            \x00\x11\x22\x33\x44\x55\x66\x77\x88\x99\xaa\xbb\xcc\xdd\xee\xff\
            \x03\x01";
        let decoded = AttemptRegistry::decode(expected).unwrap();
        assert_eq!(
            decoded.record("path:a").unwrap().phase,
            AttemptPhase::Completed
        );
        assert_eq!(decoded.encode().unwrap(), expected);
    }

    #[test]
    fn codec_roundtrips_all_states_in_source_order() {
        let mut registry = AttemptRegistry::default();
        let z = start(&mut registry, "path:z", SESSION_A);
        let a = start(&mut registry, "crate:a", SESSION_B);
        let m = start(&mut registry, "path:m", SESSION_A);
        registry.transition_building("path:z", z).unwrap();
        registry.claim_backend("path:z", z).unwrap();
        registry.record_legacy_backend_receipt("path:z", z).unwrap();
        registry.mark_failed("path:z", z).unwrap();
        registry.transition_building("path:m", m).unwrap();
        registry.claim_backend("path:m", m).unwrap();
        registry.record_legacy_backend_receipt("path:m", m).unwrap();
        registry.mark_completed("path:m", m).unwrap();
        assert_eq!(a.generation(), 2);

        let bytes = registry.encode().unwrap();
        let decoded = AttemptRegistry::decode(&bytes).unwrap();
        assert_eq!(decoded, registry);
        assert!(
            bytes
                .windows(7)
                .position(|window| window == b"crate:a")
                .unwrap()
                < bytes
                    .windows(6)
                    .position(|window| window == b"path:z")
                    .unwrap()
        );
    }

    #[test]
    fn provenance_receipt_roundtrips_and_only_exact_pending_receipt_completes() {
        let mut registry = AttemptRegistry::default();
        let attempt = start(&mut registry, "path:receipt", SESSION_A);
        registry
            .transition_building("path:receipt", attempt)
            .unwrap();
        let receipt = provenance_receipt(0x30);
        registry
            .claim_backend_with_pending_receipt("path:receipt", attempt, receipt)
            .unwrap();

        let pending_bytes = registry.encode().unwrap();
        assert_eq!(AttemptRegistry::decode(&pending_bytes).unwrap(), registry);
        assert_eq!(
            registry.mark_completed("path:receipt", attempt),
            Err(AttemptCodecError::InvalidTransition)
        );
        assert_eq!(
            registry.record_backend_publication_receipt(
                "path:receipt",
                attempt,
                provenance_receipt(0x31),
            ),
            Err(AttemptCodecError::BackendAlreadySeen)
        );

        registry
            .record_backend_publication_receipt("path:receipt", attempt, receipt)
            .unwrap();
        let completed_bytes = registry.encode().unwrap();
        assert_ne!(pending_bytes, completed_bytes);
        assert_eq!(pending_bytes.len(), completed_bytes.len());
        assert_eq!(AttemptRegistry::decode(&completed_bytes).unwrap(), registry);
        registry.mark_completed("path:receipt", attempt).unwrap();
        assert_eq!(
            registry
                .record_exact("path:receipt", attempt)
                .unwrap()
                .backend_receipt,
            Some(BackendReceiptV1::Provenance(receipt))
        );
    }

    #[test]
    fn decoder_rejects_malformed_truncated_and_trailing_input() {
        let valid = one_record_bytes();
        assert_eq!(
            AttemptRegistry::decode(b"not an attempt registry"),
            Err(AttemptCodecError::BadMagic)
        );
        for length in 0..valid.len() {
            assert!(
                AttemptRegistry::decode(&valid[..length]).is_err(),
                "{length}"
            );
        }
        let mut trailing = valid;
        trailing.push(0);
        assert_eq!(
            AttemptRegistry::decode(&trailing),
            Err(AttemptCodecError::TrailingBytes)
        );
    }

    #[test]
    fn decoder_rejects_noncanonical_and_invalid_records() {
        let reversed = raw_registry(
            2,
            &[
                ("path:z", "z", 1, SESSION_A, 0, 0),
                ("path:a", "a", 2, SESSION_B, 0, 0),
            ],
        );
        assert_eq!(
            AttemptRegistry::decode(&reversed),
            Err(AttemptCodecError::NonCanonical)
        );
        assert_eq!(
            AttemptRegistry::decode(&raw_registry(
                2,
                &[
                    ("path:a", "a", 1, SESSION_A, 0, 0),
                    ("path:a", "b", 2, SESSION_B, 0, 0),
                ],
            )),
            Err(AttemptCodecError::DuplicateSource)
        );
        assert_eq!(
            AttemptRegistry::decode(&raw_registry(
                1,
                &[
                    ("path:a", "a", 1, SESSION_A, 0, 0),
                    ("path:b", "b", 1, SESSION_B, 0, 0),
                ],
            )),
            Err(AttemptCodecError::DuplicateGeneration)
        );
        assert_eq!(
            AttemptRegistry::decode(&raw_registry(1, &[("path:a", "a", 0, SESSION_A, 0, 0)])),
            Err(AttemptCodecError::ZeroGeneration)
        );
        assert_eq!(
            AttemptRegistry::decode(&raw_registry(1, &[("path:a", "a", 2, SESSION_A, 0, 0)])),
            Err(AttemptCodecError::GenerationBeyondWatermark)
        );
        assert_eq!(
            AttemptRegistry::decode(&raw_registry(1, &[("path:a", "a", 1, SESSION_A, 9, 0)])),
            Err(AttemptCodecError::InvalidPhase(9))
        );
        assert_eq!(
            AttemptRegistry::decode(&raw_registry(1, &[("path:a", "a", 1, SESSION_A, 0, 4)])),
            Err(AttemptCodecError::InvalidBackendReceiptTag(4))
        );
        assert_eq!(
            AttemptRegistry::decode(&raw_registry(1, &[("path:a", "a", 1, SESSION_A, 0, 1)])),
            Err(AttemptCodecError::InvalidTransition)
        );
        assert_eq!(
            AttemptRegistry::decode(&raw_registry(1, &[("path:a", "a", 1, SESSION_A, 3, 0)])),
            Err(AttemptCodecError::InvalidTransition)
        );
        assert_eq!(
            AttemptRegistry::decode(&raw_registry(1, &[("path:a", "a", 1, SESSION_A, 3, 1)]))
                .unwrap()
                .record("path:a")
                .unwrap()
                .phase,
            AttemptPhase::Completed
        );
    }

    #[test]
    fn decoder_rejects_text_and_registry_limits() {
        for source in [
            "",
            "relative",
            "path:",
            "crate:",
            "other:value",
            "path:a\0b",
        ] {
            assert_eq!(
                AttemptRegistry::decode(&raw_registry(1, &[(source, "a", 1, SESSION_A, 0, 0)])),
                Err(if source.is_empty() {
                    AttemptCodecError::InvalidTextLength
                } else {
                    AttemptCodecError::InvalidStableSource
                })
            );
        }
        for crate_name in ["", "has-dash", "snowman_\u{2603}"] {
            assert_eq!(
                AttemptRegistry::decode(&raw_registry(
                    1,
                    &[("path:a", crate_name, 1, SESSION_A, 0, 0)],
                )),
                Err(if crate_name.is_empty() {
                    AttemptCodecError::InvalidTextLength
                } else {
                    AttemptCodecError::InvalidCrateName
                })
            );
        }

        let long_source = format!("path:{}", "x".repeat(MAX_STABLE_SOURCE_BYTES));
        assert_eq!(
            AttemptRegistry::decode(&raw_registry(1, &[(&long_source, "a", 1, SESSION_A, 0, 0)],)),
            Err(AttemptCodecError::InvalidTextLength)
        );
        let long_crate = "x".repeat(MAX_CRATE_NAME_BYTES + 1);
        assert_eq!(
            AttemptRegistry::decode(&raw_registry(
                1,
                &[("path:a", &long_crate, 1, SESSION_A, 0, 0)],
            )),
            Err(AttemptCodecError::InvalidTextLength)
        );

        let mut too_many = Vec::new();
        too_many.extend_from_slice(ATTEMPT_MAGIC);
        too_many.extend_from_slice(&0_u64.to_le_bytes());
        too_many.extend_from_slice(&((MAX_ATTEMPT_RECORDS + 1) as u32).to_le_bytes());
        assert_eq!(
            AttemptRegistry::decode(&too_many),
            Err(AttemptCodecError::TooManyRecords)
        );
        assert_eq!(
            AttemptRegistry::decode(&vec![0; MAX_ATTEMPT_BYTES + 1]),
            Err(AttemptCodecError::RegistryTooLarge)
        );

        let mut invalid_utf8 = raw_registry(1, &[("path:a", "a", 1, SESSION_A, 0, 0)]);
        let source_offset = ATTEMPT_MAGIC.len() + 8 + 4 + 2;
        invalid_utf8[source_offset] = 0xff;
        assert_eq!(
            AttemptRegistry::decode(&invalid_utf8),
            Err(AttemptCodecError::InvalidUtf8)
        );
    }

    #[test]
    fn same_session_resumes_or_reuses_and_terminal_states_fail_closed() {
        let mut registry = AttemptRegistry::default();
        let attempt = start(&mut registry, "path:a", SESSION_A);
        assert_eq!(
            registry
                .start_or_resume("path:a", "renamed_crate", INVOCATION_A, SESSION_A)
                .unwrap(),
            StartAttemptOutcome::ResumeInvalidating(attempt)
        );
        assert_eq!(
            registry.record("path:a").unwrap().crate_name,
            "kernel_crate"
        );

        registry.transition_building("path:a", attempt).unwrap();
        assert_eq!(
            registry
                .start_or_resume("path:a", "kernel_crate", INVOCATION_A, SESSION_A)
                .unwrap(),
            StartAttemptOutcome::ReuseBuilding(attempt)
        );
        registry.mark_failed("path:a", attempt).unwrap();
        let before = registry.clone();
        assert_eq!(
            registry.start_or_resume("path:a", "kernel_crate", INVOCATION_A, SESSION_A),
            Err(AttemptCodecError::SameSessionFailed)
        );
        assert_eq!(registry, before);

        let completed = start(&mut registry, "path:b", SESSION_A);
        registry.transition_building("path:b", completed).unwrap();
        registry.claim_backend("path:b", completed).unwrap();
        registry
            .record_legacy_backend_receipt("path:b", completed)
            .unwrap();
        registry.mark_completed("path:b", completed).unwrap();
        let before = registry.clone();
        assert_eq!(
            registry.start_or_resume("path:b", "kernel_crate", INVOCATION_A, SESSION_A),
            Err(AttemptCodecError::SameSessionCompleted)
        );
        assert_eq!(registry, before);
    }

    #[test]
    fn different_session_supersedes_with_a_new_generation() {
        let mut registry = AttemptRegistry::default();
        let first = start(&mut registry, "path:a", SESSION_A);
        registry.transition_building("path:a", first).unwrap();
        registry.claim_backend("path:a", first).unwrap();
        registry
            .record_legacy_backend_receipt("path:a", first)
            .unwrap();

        let second = match registry
            .start_or_resume("path:a", "renamed_crate", INVOCATION_B, SESSION_B)
            .unwrap()
        {
            StartAttemptOutcome::New(attempt) => attempt,
            outcome => panic!("unexpected supersession outcome: {outcome:?}"),
        };
        assert_eq!(second.generation(), first.generation() + 1);
        let record = registry.record("path:a").unwrap();
        assert_eq!(record.crate_name, "renamed_crate");
        assert_eq!(record.phase, AttemptPhase::Invalidating);
        assert!(record.backend_receipt.is_none());
        assert_eq!(record.session, SESSION_B);
        assert_eq!(
            registry.authorize_backend("path:a", first),
            Err(AttemptCodecError::AttemptMismatch)
        );
    }

    #[test]
    fn transitions_and_authorization_require_the_exact_token() {
        let mut registry = AttemptRegistry::default();
        let attempt = start(&mut registry, "path:a", SESSION_A);
        let wrong_generation =
            BuildAttempt::new(attempt.generation() + 1, SESSION_A, INVOCATION_A).unwrap();
        let wrong_session =
            BuildAttempt::new(attempt.generation(), SESSION_B, INVOCATION_A).unwrap();
        let wrong_invocation =
            BuildAttempt::new(attempt.generation(), SESSION_A, INVOCATION_B).unwrap();

        assert_eq!(
            registry.authorize_backend("path:a", attempt),
            Err(AttemptCodecError::InvalidTransition)
        );
        for wrong in [wrong_generation, wrong_session, wrong_invocation] {
            assert_eq!(
                registry.transition_building("path:a", wrong),
                Err(AttemptCodecError::AttemptMismatch)
            );
            assert_eq!(
                registry.record_legacy_backend_receipt("path:a", wrong),
                Err(AttemptCodecError::AttemptMismatch)
            );
            assert_eq!(
                registry.mark_failed("path:a", wrong),
                Err(AttemptCodecError::AttemptMismatch)
            );
            assert_eq!(
                registry.mark_completed("path:a", wrong),
                Err(AttemptCodecError::AttemptMismatch)
            );
            assert_eq!(
                registry.record_exact("path:a", wrong),
                Err(AttemptCodecError::AttemptMismatch)
            );
        }
        assert_eq!(
            registry.authorize_backend("path:missing", attempt),
            Err(AttemptCodecError::MissingSource)
        );

        registry.transition_building("path:a", attempt).unwrap();
        assert_eq!(
            registry
                .authorize_backend("path:a", attempt)
                .unwrap()
                .generation,
            1
        );
        assert_eq!(
            registry.record_exact("path:a", attempt).unwrap().generation,
            1
        );
        registry.claim_backend("path:a", attempt).unwrap();
        registry
            .record_legacy_backend_receipt("path:a", attempt)
            .unwrap();
        let backend_receipt = registry.clone();
        assert_eq!(
            registry.record_legacy_backend_receipt("path:a", attempt),
            Err(AttemptCodecError::BackendAlreadySeen)
        );
        assert_eq!(registry, backend_receipt);
        assert_eq!(
            registry.authorize_backend("path:a", attempt),
            Err(AttemptCodecError::BackendAlreadySeen)
        );
        assert_eq!(
            registry.record("path:a").unwrap().backend_receipt,
            Some(BackendReceiptV1::LegacyCoordination)
        );
        registry.mark_completed("path:a", attempt).unwrap();
        registry.mark_completed("path:a", attempt).unwrap();
        assert_eq!(
            registry.mark_failed("path:a", attempt),
            Err(AttemptCodecError::InvalidTransition)
        );
        assert_eq!(
            registry.authorize_backend("path:a", attempt),
            Err(AttemptCodecError::InvalidTransition)
        );
        assert_eq!(
            registry.record_exact("path:a", attempt).unwrap().phase,
            AttemptPhase::Completed
        );

        let record = registry.records.get_mut("path:a").unwrap();
        record.backend_receipt = None;
        let invalid_completed = registry.clone();
        assert_eq!(
            registry.mark_completed("path:a", attempt),
            Err(AttemptCodecError::InvalidTransition)
        );
        assert_eq!(registry, invalid_completed);
        assert_eq!(registry.encode(), Err(AttemptCodecError::InvalidTransition));
    }

    #[test]
    fn failed_state_is_idempotent_and_cannot_complete_or_authorize() {
        let mut registry = AttemptRegistry::default();
        let attempt = start(&mut registry, "path:a", SESSION_A);
        registry.transition_building("path:a", attempt).unwrap();
        registry.claim_backend("path:a", attempt).unwrap();
        registry
            .record_legacy_backend_receipt("path:a", attempt)
            .unwrap();
        registry.mark_failed("path:a", attempt).unwrap();
        let failed = registry.clone();

        registry.mark_failed("path:a", attempt).unwrap();
        assert_eq!(registry, failed);
        assert_eq!(
            registry.record("path:a").unwrap().backend_receipt,
            Some(BackendReceiptV1::LegacyCoordination)
        );
        assert_eq!(
            registry.mark_completed("path:a", attempt),
            Err(AttemptCodecError::InvalidTransition)
        );
        assert_eq!(
            registry.authorize_backend("path:a", attempt),
            Err(AttemptCodecError::InvalidTransition)
        );
        assert_eq!(
            AttemptRegistry::decode(&registry.encode().unwrap()).unwrap(),
            registry
        );
    }

    #[test]
    fn direct_allocations_are_zero_session_and_always_new() {
        let mut registry = AttemptRegistry::default();
        let first = registry.allocate_direct("crate:a", "a").unwrap();
        let second = registry.allocate_direct("crate:a", "renamed").unwrap();
        assert_eq!(first.session(), BuildSession::DIRECT);
        assert_eq!(second.session(), BuildSession::DIRECT);
        assert_eq!(second.generation(), first.generation() + 1);
        assert_eq!(registry.record("crate:a").unwrap().crate_name, "renamed");
    }

    #[test]
    fn remove_session_only_removes_exact_session_and_keeps_watermark() {
        let mut registry = AttemptRegistry::default();
        let first = start(&mut registry, "path:a", SESSION_A);
        let second = start(&mut registry, "path:b", SESSION_B);
        let third = start(&mut registry, "path:c", SESSION_A);
        assert_eq!(registry.remove_session(SESSION_A), 2);
        assert!(registry.record("path:a").is_none());
        assert!(registry.record("path:c").is_none());
        assert_eq!(registry.record("path:b").unwrap().attempt(), second);
        assert_eq!(registry.remove_session(SESSION_A), 0);
        assert_eq!(registry.last_issued_generation, third.generation());
        assert_eq!(first.generation(), 1);
        assert_eq!(
            registry.encode().unwrap(),
            AttemptRegistry::decode(&registry.encode().unwrap())
                .unwrap()
                .encode()
                .unwrap()
        );
    }

    #[test]
    fn capacity_fails_closed_without_evicting_or_mutating() {
        let mut registry = AttemptRegistry::default();
        for index in 0..MAX_ATTEMPT_RECORDS {
            start(&mut registry, &format!("path:{index}"), SESSION_A);
        }
        let before = registry.clone();
        assert_eq!(
            registry.start_or_resume("path:overflow", "a", INVOCATION_B, SESSION_B),
            Err(AttemptCodecError::TooManyRecords)
        );
        assert_eq!(registry, before);

        let old = registry.record("path:0").unwrap().attempt();
        let replacement = match registry
            .start_or_resume("path:0", "replacement", INVOCATION_B, SESSION_B)
            .unwrap()
        {
            StartAttemptOutcome::New(attempt) => attempt,
            outcome => panic!("unexpected replacement outcome: {outcome:?}"),
        };
        assert_eq!(registry.records.len(), MAX_ATTEMPT_RECORDS);
        assert!(replacement.generation() > old.generation());
    }

    #[test]
    fn canonical_byte_limit_fails_closed_without_mutating() {
        let mut registry = AttemptRegistry::default();
        let crate_name = "x".repeat(MAX_CRATE_NAME_BYTES);
        let mut rejected = false;
        for index in 0..MAX_ATTEMPT_RECORDS {
            let prefix = format!("path:{index:04}:");
            let source = format!(
                "{prefix}{}",
                "s".repeat(MAX_STABLE_SOURCE_BYTES - prefix.len())
            );
            let before = registry.clone();
            match registry.start_or_resume(&source, &crate_name, INVOCATION_A, SESSION_A) {
                Ok(StartAttemptOutcome::New(_)) => {
                    assert!(registry.encode().unwrap().len() <= MAX_ATTEMPT_BYTES);
                }
                Err(AttemptCodecError::RegistryTooLarge) => {
                    assert_eq!(registry, before);
                    rejected = true;
                    break;
                }
                outcome => panic!("unexpected byte-bound outcome: {outcome:?}"),
            }
        }
        assert!(rejected);
        assert!(registry.records.len() < MAX_ATTEMPT_RECORDS);
    }

    #[test]
    fn generation_exhaustion_never_mutates_registry() {
        let mut registry = AttemptRegistry {
            last_issued_generation: u64::MAX,
            records: BTreeMap::new(),
        };
        let before = registry.clone();
        assert_eq!(
            registry.start_or_resume("path:a", "a", INVOCATION_A, SESSION_A),
            Err(AttemptCodecError::GenerationExhausted)
        );
        assert_eq!(registry, before);
        assert_eq!(
            registry.allocate_direct("path:a", "a"),
            Err(AttemptCodecError::GenerationExhausted)
        );
        assert_eq!(registry, before);
    }

    #[test]
    fn mutation_input_validation_is_fail_closed() {
        let mut registry = AttemptRegistry::default();
        for source in ["", "relative", "path:", "crate:", "path:a\0b"] {
            let before = registry.clone();
            assert_eq!(
                registry.start_or_resume(source, "a", INVOCATION_A, SESSION_A),
                Err(AttemptCodecError::InvalidStableSource)
            );
            assert_eq!(registry, before);
        }
        for crate_name in ["", "has-dash", "snowman_\u{2603}"] {
            let before = registry.clone();
            assert_eq!(
                registry.start_or_resume("path:a", crate_name, INVOCATION_A, SESSION_A),
                Err(AttemptCodecError::InvalidCrateName)
            );
            assert_eq!(registry, before);
        }
        let before = registry.clone();
        assert_eq!(
            registry.start_or_resume("path:a", "a", BuildInvocation::DIRECT, SESSION_A,),
            Err(AttemptCodecError::InvalidInvocationEncoding)
        );
        assert_eq!(registry, before);
    }
}
