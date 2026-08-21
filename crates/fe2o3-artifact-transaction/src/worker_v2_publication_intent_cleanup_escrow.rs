const CLEANUP_ESCROW_CAPSULE_MAGIC_V1: &[u8] =
    b"FE2O3-WORKER-V2-INTENT-CLEANUP-ESCROW-CAPSULE-V1\0";
const CLEANUP_ESCROW_CAPSULE_VERSION_V1: u16 = 1;
const CLEANUP_ESCROW_CAPSULE_CHECKSUM_DOMAIN_V1: &[u8] =
    b"fe2o3.worker-v2-intent-cleanup-escrow.capsule-checksum.v1\0";
const CLEANUP_ESCROW_CAPSULE_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.worker-v2-intent-cleanup-escrow.capsule-identity.v1\0";
const CLEANUP_ESCROW_RECEIPT_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.worker-v2-intent-cleanup-escrow.receipt-identity.v1\0";
const CLEANUP_ESCROW_MANIFEST_MAGIC_V1: &[u8] =
    b"FE2O3-WORKER-V2-INTENT-CLEANUP-ESCROW-MANIFEST-V1\0";
const CLEANUP_ESCROW_MANIFEST_VERSION_V1: u16 = 1;
const CLEANUP_ESCROW_MANIFEST_CHECKSUM_DOMAIN_V1: &[u8] =
    b"fe2o3.worker-v2-intent-cleanup-escrow.manifest-checksum.v1\0";
const CLEANUP_ESCROW_NAME_DOMAIN_V1: &[u8] = b"fe2o3.worker-v2-intent-cleanup-escrow.name.v1\0";
const CLEANUP_ESCROW_PREFIX_V1: &str = ".fe2o3-worker-v2-intent-cleanup-escrow-v1-";
const CLEANUP_ESCROW_MANIFEST_SUFFIX_V1: &str = ".manifest";
const CLEANUP_ESCROW_RECORD_SUFFIX_V1: &str = ".record.quarantine";
const CLEANUP_ESCROW_OUTPUT_SUFFIX_V1: &str = ".output.quarantine";
const CLEANUP_ESCROW_TEMP_SUFFIX_V1: &str = ".manifest.tmp-";
const CLEANUP_ESCROW_MAX_TEMPS_V1: usize = 64;

// device, inode, byte length, mode, and link count.
const CLEANUP_ESCROW_FILE_SNAPSHOT_BYTES_V1: usize = 5 * 8;
const CLEANUP_ESCROW_RECEIPT_BYTES_V1: usize = (7 * 32) + COMPILER_CLOSURE_BYTES_V2;
const CLEANUP_ESCROW_CAPSULE_BODY_BYTES_V1: usize = CLEANUP_ESCROW_CAPSULE_MAGIC_V1.len()
    + 2
    + 32
    + 32
    + 8
    + 16
    + 32
    + 32
    + 32
    + 32
    + 32
    + 32
    + CLEANUP_ESCROW_RECEIPT_BYTES_V1
    + (2 * CLEANUP_ESCROW_FILE_SNAPSHOT_BYTES_V1);
/// Exact upper bound for one opaque V2 cleanup escrow capsule.
pub const MAX_WORKER_V2_PUBLICATION_INTENT_CLEANUP_ESCROW_CAPSULE_BYTES_V1: usize =
    CLEANUP_ESCROW_CAPSULE_BODY_BYTES_V1 + 32;
const CLEANUP_ESCROW_MANIFEST_BYTES_V1: usize = CLEANUP_ESCROW_MANIFEST_MAGIC_V1.len()
    + 2
    + 1
    + MAX_WORKER_V2_PUBLICATION_INTENT_CLEANUP_ESCROW_CAPSULE_BYTES_V1
    + 32;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerV2PublicationIntentCleanupEscrowIdentityV1([u8; 32]);

impl WorkerV2PublicationIntentCleanupEscrowIdentityV1 {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CleanupEscrowFileSnapshotV1 {
    device: u64,
    inode: u64,
    byte_len: u64,
    mode: u64,
    links: u64,
}

impl CleanupEscrowFileSnapshotV1 {
    fn from_stat(stat: &rustix::fs::Stat) -> Result<Self, &'static str> {
        if !is_private_file(stat) {
            return Err("escrow entry is not a private single-link regular file");
        }
        Ok(Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            byte_len: u64::try_from(stat.st_size).map_err(|_| "escrow entry size is invalid")?,
            mode: u64::from(stat.st_mode),
            links: stat.st_nlink,
        })
    }

    fn encode(self, bytes: &mut Vec<u8>) {
        for field in [
            self.device,
            self.inode,
            self.byte_len,
            self.mode,
            self.links,
        ] {
            bytes.extend_from_slice(&field.to_le_bytes());
        }
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, &'static str> {
        Ok(Self {
            device: decoder.u64()?,
            inode: decoder.u64()?,
            byte_len: decoder.u64()?,
            mode: decoder.u64()?,
            links: decoder.u64()?,
        })
    }

    fn matches(self, stat: &rustix::fs::Stat) -> bool {
        self.device == stat.st_dev
            && self.inode == stat.st_ino
            && u64::try_from(stat.st_size) == Ok(self.byte_len)
            && u64::from(stat.st_mode) == self.mode
            && stat.st_nlink == self.links
            && is_private_file(stat)
    }

    fn is_canonical_private_file(self) -> bool {
        FileType::from_raw_mode(self.mode as _) == FileType::RegularFile
            && self.links == 1
            && self.mode & 0o777 == 0o600
    }
}

/// Small artifact-owned receipt for one quarantined exact V2 publication intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV2PublicationIntentCleanupEscrowV1 {
    package: [u8; 32],
    producer_identity: [u8; 32],
    attempt: BuildAttempt,
    slot: [u8; 32],
    intent: WorkerV2PublicationIntentIdentityV2,
    record_identity: [u8; 32],
    output_identity: [u8; 32],
    receipt_identity: [u8; 32],
    receipt: crate::BackendPublicationReceiptV2,
    record_file: CleanupEscrowFileSnapshotV1,
    output_file: CleanupEscrowFileSnapshotV1,
    identity: WorkerV2PublicationIntentCleanupEscrowIdentityV1,
}

impl WorkerV2PublicationIntentCleanupEscrowV1 {
    fn new(
        package: [u8; 32],
        record: WorkerV2PublicationIntentRecordV2,
        receipt: crate::BackendPublicationReceiptV2,
        record_file: CleanupEscrowFileSnapshotV1,
        output_file: CleanupEscrowFileSnapshotV1,
    ) -> Result<Self, &'static str> {
        let mut capsule = Self {
            package,
            producer_identity: record.producer_identity(),
            attempt: record.attempt(),
            slot: record.slot,
            intent: record.identity(),
            record_identity: record.identity().as_bytes(),
            output_identity: *record.output_identity().as_bytes(),
            receipt_identity: cleanup_escrow_receipt_identity_v1(receipt),
            receipt,
            record_file,
            output_file,
            identity: WorkerV2PublicationIntentCleanupEscrowIdentityV1([0; 32]),
        };
        capsule.validate()?;
        capsule.identity = WorkerV2PublicationIntentCleanupEscrowIdentityV1(sha256_parts(&[
            CLEANUP_ESCROW_CAPSULE_IDENTITY_DOMAIN_V1,
            &capsule.encode_body(),
        ]));
        Ok(capsule)
    }

    pub const fn identity(self) -> WorkerV2PublicationIntentCleanupEscrowIdentityV1 {
        self.identity
    }

    pub const fn attempt(self) -> BuildAttempt {
        self.attempt
    }

    pub const fn intent(self) -> WorkerV2PublicationIntentIdentityV2 {
        self.intent
    }

    pub const fn package_identity(self) -> [u8; 32] {
        self.package
    }

    pub const fn producer_identity(self) -> [u8; 32] {
        self.producer_identity
    }

    pub const fn receipt(self) -> crate::BackendPublicationReceiptV2 {
        self.receipt
    }

    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    pub const fn grants_load_authority(self) -> bool {
        false
    }

    pub const fn grants_launch_authority(self) -> bool {
        false
    }

    pub fn to_bytes(self) -> Vec<u8> {
        let mut bytes = self.encode_body();
        bytes.extend_from_slice(&sha256_parts(&[
            CLEANUP_ESCROW_CAPSULE_CHECKSUM_DOMAIN_V1,
            &bytes,
        ]));
        debug_assert_eq!(
            bytes.len(),
            MAX_WORKER_V2_PUBLICATION_INTENT_CLEANUP_ESCROW_CAPSULE_BYTES_V1
        );
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != MAX_WORKER_V2_PUBLICATION_INTENT_CLEANUP_ESCROW_CAPSULE_BYTES_V1 {
            return Err("cleanup escrow capsule has a noncanonical length");
        }
        let (body, checksum) = bytes.split_at(bytes.len() - 32);
        if sha256_parts(&[CLEANUP_ESCROW_CAPSULE_CHECKSUM_DOMAIN_V1, body]).as_slice() != checksum {
            return Err("cleanup escrow capsule checksum mismatch");
        }
        let mut decoder = Decoder::new(body);
        if decoder.take(CLEANUP_ESCROW_CAPSULE_MAGIC_V1.len())? != CLEANUP_ESCROW_CAPSULE_MAGIC_V1 {
            return Err("cleanup escrow capsule magic mismatch");
        }
        if decoder.u16()? != CLEANUP_ESCROW_CAPSULE_VERSION_V1 {
            return Err("unsupported cleanup escrow capsule version");
        }
        let package = decoder.array()?;
        let producer_identity = decoder.array()?;
        let generation = decoder.u64()?;
        let session = BuildSession::from_bytes(decoder.array()?);
        let invocation = crate::BuildInvocation::from_bytes(decoder.array()?);
        let attempt = BuildAttempt::from_env_value(&format!(
            "{generation}:{}:{}",
            session.to_hex(),
            invocation.to_hex()
        ))
        .map_err(|_| "cleanup escrow capsule contains an invalid attempt")?;
        let slot = decoder.array()?;
        let intent = WorkerV2PublicationIntentIdentityV2::from_bytes(decoder.array()?);
        let record_identity = decoder.array()?;
        let output_identity = decoder.array()?;
        let receipt_identity = decoder.array()?;
        let receipt = decode_cleanup_escrow_receipt_v1(&mut decoder)?;
        let record_file = CleanupEscrowFileSnapshotV1::decode(&mut decoder)?;
        let output_file = CleanupEscrowFileSnapshotV1::decode(&mut decoder)?;
        if !decoder.finished() {
            return Err("cleanup escrow capsule has trailing bytes");
        }
        let mut capsule = Self {
            package,
            producer_identity,
            attempt,
            slot,
            intent,
            record_identity,
            output_identity,
            receipt_identity,
            receipt,
            record_file,
            output_file,
            identity: WorkerV2PublicationIntentCleanupEscrowIdentityV1([0; 32]),
        };
        capsule.validate()?;
        capsule.identity = WorkerV2PublicationIntentCleanupEscrowIdentityV1(sha256_parts(&[
            CLEANUP_ESCROW_CAPSULE_IDENTITY_DOMAIN_V1,
            body,
        ]));
        Ok(capsule)
    }

    fn validate(self) -> Result<(), &'static str> {
        if self.slot != slot_identity_v2(self.producer_identity, self.attempt)
            || self.record_identity != self.intent.as_bytes()
            || self.receipt_identity != cleanup_escrow_receipt_identity_v1(self.receipt)
            || self.receipt.attempt_identity()
                != backend_publication_receipt_attempt_identity_v2(self.attempt)
            || self.receipt.scope_identity() == [0; 32]
            || self.receipt.finalized_output_identity() != self.output_identity
            || self.record_file.byte_len
                != u64::try_from(RECORD_BYTES_V2).expect("record bound fits u64")
            || self.output_file.byte_len == 0
            || self.output_file.byte_len
                > u64::try_from(MAX_WORKER_V2_PUBLICATION_INTENT_OUTPUT_BYTES_V2)
                    .expect("output bound fits u64")
            || !self.record_file.is_canonical_private_file()
            || !self.output_file.is_canonical_private_file()
            || (self.record_file.device == self.output_file.device
                && self.record_file.inode == self.output_file.inode)
        {
            return Err("cleanup escrow capsule fields are inconsistent");
        }
        Ok(())
    }

    fn encode_body(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(CLEANUP_ESCROW_CAPSULE_BODY_BYTES_V1);
        bytes.extend_from_slice(CLEANUP_ESCROW_CAPSULE_MAGIC_V1);
        bytes.extend_from_slice(&CLEANUP_ESCROW_CAPSULE_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&self.package);
        bytes.extend_from_slice(&self.producer_identity);
        bytes.extend_from_slice(&self.attempt.generation().to_le_bytes());
        bytes.extend_from_slice(self.attempt.session().as_bytes());
        bytes.extend_from_slice(self.attempt.invocation().as_bytes());
        bytes.extend_from_slice(&self.slot);
        bytes.extend_from_slice(&self.intent.as_bytes());
        bytes.extend_from_slice(&self.record_identity);
        bytes.extend_from_slice(&self.output_identity);
        bytes.extend_from_slice(&self.receipt_identity);
        encode_cleanup_escrow_receipt_v1(self.receipt, &mut bytes);
        self.record_file.encode(&mut bytes);
        self.output_file.encode(&mut bytes);
        debug_assert_eq!(bytes.len(), CLEANUP_ESCROW_CAPSULE_BODY_BYTES_V1);
        bytes
    }
}

fn encode_cleanup_escrow_receipt_v1(
    receipt: crate::BackendPublicationReceiptV2,
    bytes: &mut Vec<u8>,
) {
    for identity in [
        receipt.attempt_identity(),
        receipt.producer_identity(),
        receipt.scope_identity(),
        receipt.plan_commitment(),
        receipt.upstream_evidence_identity(),
        receipt.finalized_output_identity(),
        receipt.publication_identity(),
    ] {
        bytes.extend_from_slice(&identity);
    }
    encode_compiler_closure_v2(receipt.compiler_closure(), bytes);
}

fn decode_cleanup_escrow_receipt_v1(
    decoder: &mut Decoder<'_>,
) -> Result<crate::BackendPublicationReceiptV2, &'static str> {
    Ok(crate::BackendPublicationReceiptV2::new(
        decoder.array()?,
        decoder.array()?,
        decoder.array()?,
        decoder.array()?,
        decoder.array()?,
        decoder.array()?,
        decoder.array()?,
        decode_compiler_closure_v2(decoder)?,
    ))
}

fn cleanup_escrow_receipt_identity_v1(receipt: crate::BackendPublicationReceiptV2) -> [u8; 32] {
    let mut bytes = Vec::with_capacity(CLEANUP_ESCROW_RECEIPT_BYTES_V1);
    encode_cleanup_escrow_receipt_v1(receipt, &mut bytes);
    sha256_parts(&[CLEANUP_ESCROW_RECEIPT_IDENTITY_DOMAIN_V1, &bytes])
}

/// Durable artifact-owned state of one cleanup escrow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerV2PublicationIntentCleanupEscrowStateV1 {
    Prepared,
    Committed,
}

impl WorkerV2PublicationIntentCleanupEscrowStateV1 {
    fn tag(self) -> u8 {
        match self {
            Self::Prepared => 1,
            Self::Committed => 2,
        }
    }

    fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Prepared),
            2 => Some(Self::Committed),
            _ => None,
        }
    }
}

/// Bounded evidence that one predecessor escrow was committed while an exact newer V2 intent
/// remained pinned under the same artifact-directory lock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV2PublicationIntentCleanupEscrowCommitEvidenceV2 {
    predecessor: WorkerV2PublicationIntentCleanupEscrowIdentityV1,
    successor: WorkerV2PublicationIntentRecordV2,
}

impl WorkerV2PublicationIntentCleanupEscrowCommitEvidenceV2 {
    /// Identity of the exact predecessor escrow consumed by the atomic operation.
    pub const fn predecessor(self) -> WorkerV2PublicationIntentCleanupEscrowIdentityV1 {
        self.predecessor
    }

    /// Complete bounded successor record revalidated after predecessor commit.
    pub const fn successor(self) -> WorkerV2PublicationIntentRecordV2 {
        self.successor
    }

    /// This cleanup evidence does not grant publication authority.
    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    /// This cleanup evidence does not grant load authority.
    pub const fn grants_load_authority(self) -> bool {
        false
    }

    /// This cleanup evidence does not grant launch authority.
    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CleanupEscrowManifestV1 {
    state: WorkerV2PublicationIntentCleanupEscrowStateV1,
    capsule: WorkerV2PublicationIntentCleanupEscrowV1,
}

impl CleanupEscrowManifestV1 {
    fn to_bytes(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(CLEANUP_ESCROW_MANIFEST_BYTES_V1);
        bytes.extend_from_slice(CLEANUP_ESCROW_MANIFEST_MAGIC_V1);
        bytes.extend_from_slice(&CLEANUP_ESCROW_MANIFEST_VERSION_V1.to_le_bytes());
        bytes.push(self.state.tag());
        bytes.extend_from_slice(&self.capsule.to_bytes());
        bytes.extend_from_slice(&sha256_parts(&[
            CLEANUP_ESCROW_MANIFEST_CHECKSUM_DOMAIN_V1,
            &bytes,
        ]));
        debug_assert_eq!(bytes.len(), CLEANUP_ESCROW_MANIFEST_BYTES_V1);
        bytes
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != CLEANUP_ESCROW_MANIFEST_BYTES_V1 {
            return Err("cleanup escrow manifest has a noncanonical length");
        }
        let (body, checksum) = bytes.split_at(bytes.len() - 32);
        if sha256_parts(&[CLEANUP_ESCROW_MANIFEST_CHECKSUM_DOMAIN_V1, body]).as_slice() != checksum
        {
            return Err("cleanup escrow manifest checksum mismatch");
        }
        let mut decoder = Decoder::new(body);
        if decoder.take(CLEANUP_ESCROW_MANIFEST_MAGIC_V1.len())? != CLEANUP_ESCROW_MANIFEST_MAGIC_V1
        {
            return Err("cleanup escrow manifest magic mismatch");
        }
        if decoder.u16()? != CLEANUP_ESCROW_MANIFEST_VERSION_V1 {
            return Err("unsupported cleanup escrow manifest version");
        }
        let state = WorkerV2PublicationIntentCleanupEscrowStateV1::from_tag(decoder.take(1)?[0])
            .ok_or("cleanup escrow manifest state is invalid")?;
        let capsule = WorkerV2PublicationIntentCleanupEscrowV1::from_bytes(
            decoder.take(MAX_WORKER_V2_PUBLICATION_INTENT_CLEANUP_ESCROW_CAPSULE_BYTES_V1)?,
        )?;
        if !decoder.finished() {
            return Err("cleanup escrow manifest has trailing bytes");
        }
        Ok(Self { state, capsule })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerV2PublicationIntentCleanupEscrowBoundaryV1 {
    RenameRecordToQuarantine,
    SyncQuarantinedRecordName,
    RenameOutputToQuarantine,
    SyncQuarantinedOutputName,
    CreateManifestTemp,
    WriteManifestTemp,
    SyncManifestTemp,
    RenameManifest,
    SyncManifestName,
    UnlinkQuarantinedRecord,
    SyncQuarantinedRecordDeletion,
    UnlinkQuarantinedOutput,
    SyncQuarantinedOutputDeletion,
    UnlinkCommittedManifest,
    SyncCommittedManifestDeletion,
    RenameOutputToCanonical,
    SyncCanonicalOutputName,
    RenameRecordToCanonical,
    SyncCanonicalRecordName,
    UnlinkRolledBackManifest,
    SyncRolledBackManifestDeletion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerV2PublicationIntentCleanupEscrowFaultTimingV1 {
    Before,
    After,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV2PublicationIntentCleanupEscrowFaultPointV1 {
    pub boundary: WorkerV2PublicationIntentCleanupEscrowBoundaryV1,
    pub timing: WorkerV2PublicationIntentCleanupEscrowFaultTimingV1,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkerV2PublicationIntentCleanupEscrowOptionsV1 {
    fault: Option<WorkerV2PublicationIntentCleanupEscrowFaultPointV1>,
}

impl WorkerV2PublicationIntentCleanupEscrowOptionsV1 {
    pub const fn inject_crash(fault: WorkerV2PublicationIntentCleanupEscrowFaultPointV1) -> Self {
        Self { fault: Some(fault) }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV2PublicationIntentCleanupEscrowErrorV1 {
    Intent(WorkerV2PublicationIntentErrorV2),
    Io(std::io::Error),
    InvalidEscrow {
        path: PathBuf,
        reason: String,
    },
    ConflictingEscrow,
    CapsuleMismatch,
    InvalidTransition,
    /// Explicit successor fields disagree with the supplied exact recovered snapshot.
    SuccessorExpectationMismatch,
    /// The proposed successor generation is not strictly newer than the predecessor.
    SuccessorGenerationNotNewer {
        /// Durable predecessor generation.
        predecessor: u64,
        /// Proposed successor generation.
        successor: u64,
    },
    InjectedCrash {
        point: WorkerV2PublicationIntentCleanupEscrowFaultPointV1,
    },
}

impl fmt::Display for WorkerV2PublicationIntentCleanupEscrowErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Intent(error) => write!(formatter, "cleanup escrow rejected intent: {error}"),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::InvalidEscrow { path, reason } => {
                write!(
                    formatter,
                    "invalid cleanup escrow {}: {reason}",
                    path.display()
                )
            }
            Self::ConflictingEscrow => formatter.write_str(
                "a different protected cleanup escrow occupies the producer/package slot",
            ),
            Self::CapsuleMismatch => {
                formatter.write_str("cleanup escrow capsule does not match durable state")
            }
            Self::InvalidTransition => {
                formatter.write_str("invalid protected cleanup escrow state transition")
            }
            Self::SuccessorExpectationMismatch => formatter.write_str(
                "exact successor inputs do not describe one canonical V2 publication intent",
            ),
            Self::SuccessorGenerationNotNewer {
                predecessor,
                successor,
            } => write!(
                formatter,
                "successor generation {successor} is not newer than predecessor generation {predecessor}",
            ),
            Self::InjectedCrash { point } => {
                write!(formatter, "injected cleanup escrow crash at {point:?}")
            }
        }
    }
}

impl std::error::Error for WorkerV2PublicationIntentCleanupEscrowErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Intent(error) => Some(error),
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<WorkerV2PublicationIntentErrorV2> for WorkerV2PublicationIntentCleanupEscrowErrorV1 {
    fn from(error: WorkerV2PublicationIntentErrorV2) -> Self {
        Self::Intent(error)
    }
}

impl From<std::io::Error> for WorkerV2PublicationIntentCleanupEscrowErrorV1 {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

struct CleanupEscrowFaultInjectorV1 {
    fault: Option<WorkerV2PublicationIntentCleanupEscrowFaultPointV1>,
    fired: bool,
}

impl CleanupEscrowFaultInjectorV1 {
    fn new(options: WorkerV2PublicationIntentCleanupEscrowOptionsV1) -> Self {
        Self {
            fault: options.fault,
            fired: false,
        }
    }

    fn hit(
        &mut self,
        boundary: WorkerV2PublicationIntentCleanupEscrowBoundaryV1,
        timing: WorkerV2PublicationIntentCleanupEscrowFaultTimingV1,
    ) -> Result<(), WorkerV2PublicationIntentCleanupEscrowErrorV1> {
        let point = WorkerV2PublicationIntentCleanupEscrowFaultPointV1 { boundary, timing };
        if !self.fired && self.fault == Some(point) {
            self.fired = true;
            Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::InjectedCrash { point })
        } else {
            Ok(())
        }
    }

    fn around(
        &mut self,
        boundary: WorkerV2PublicationIntentCleanupEscrowBoundaryV1,
        operation: impl FnOnce() -> Result<(), WorkerV2PublicationIntentCleanupEscrowErrorV1>,
    ) -> Result<(), WorkerV2PublicationIntentCleanupEscrowErrorV1> {
        self.hit(
            boundary,
            WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::Before,
        )?;
        operation()?;
        self.hit(
            boundary,
            WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::After,
        )
    }
}

struct CleanupEscrowNamesV1 {
    manifest: String,
    record: String,
    output: String,
    temp_prefix: String,
}

impl CleanupEscrowNamesV1 {
    fn new(
        producer_identity: [u8; 32],
        package: [u8; 32],
        slot: [u8; 32],
        intent: WorkerV2PublicationIntentIdentityV2,
    ) -> Self {
        let namespace = sha256_parts(&[
            CLEANUP_ESCROW_NAME_DOMAIN_V1,
            &producer_identity,
            &package,
            &slot,
            &intent.as_bytes(),
        ]);
        let base = format!("{CLEANUP_ESCROW_PREFIX_V1}{}", hex(&namespace));
        Self {
            manifest: format!("{base}{CLEANUP_ESCROW_MANIFEST_SUFFIX_V1}"),
            record: format!("{base}{CLEANUP_ESCROW_RECORD_SUFFIX_V1}"),
            output: format!("{base}{CLEANUP_ESCROW_OUTPUT_SUFFIX_V1}"),
            temp_prefix: format!("{base}{CLEANUP_ESCROW_TEMP_SUFFIX_V1}"),
        }
    }
}

fn cleanup_escrow_invalid_v1(
    output: &PinnedOutput,
    entry: &str,
    reason: impl Into<String>,
) -> WorkerV2PublicationIntentCleanupEscrowErrorV1 {
    WorkerV2PublicationIntentCleanupEscrowErrorV1::InvalidEscrow {
        path: output.display_path.join(entry),
        reason: reason.into(),
    }
}

fn cleanup_escrow_entry_exists_v1(
    output: &PinnedOutput,
    entry: &str,
) -> Result<bool, WorkerV2PublicationIntentCleanupEscrowErrorV1> {
    match statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) if is_private_file(&stat) => Ok(true),
        Ok(_) => Err(cleanup_escrow_invalid_v1(
            output,
            entry,
            "entry is not a private single-link regular file",
        )),
        Err(error) if error == rustix::io::Errno::NOENT => Ok(false),
        Err(error) => Err(std::io::Error::from(error).into()),
    }
}

struct PinnedCleanupEscrowEntryV1 {
    file: fs::File,
    stat: rustix::fs::Stat,
    name: String,
}

/// Proof that the caller holds the crate's exclusive lock for this pinned directory.
///
/// Destructive helpers require this token so cooperating code cannot accidentally bypass the
/// filesystem concurrency contract. The token cannot constrain same-UID code outside this crate.
struct CleanupEscrowExclusiveDirectoryV1<'a> {
    output: &'a PinnedOutput,
    _lock: &'a OutputLock,
}

impl<'a> CleanupEscrowExclusiveDirectoryV1<'a> {
    fn new(
        output: &'a PinnedOutput,
        lock: &'a OutputLock,
    ) -> Result<Self, WorkerV2PublicationIntentCleanupEscrowErrorV1> {
        let lock_fd = lock.fd.as_ref().ok_or_else(|| {
            cleanup_escrow_invalid_v1(output, crate::LOCK_FILE, "artifact lock was released")
        })?;
        let pinned = fstat(lock_fd).map_err(std::io::Error::from)?;
        let named = statat(&output.fd, crate::LOCK_FILE, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(std::io::Error::from)?;
        if !is_private_file(&pinned)
            || pinned.st_dev != named.st_dev
            || pinned.st_ino != named.st_ino
            || !is_private_file(&named)
        {
            return Err(cleanup_escrow_invalid_v1(
                output,
                crate::LOCK_FILE,
                "held artifact lock does not belong to the pinned output directory",
            ));
        }
        Ok(Self {
            output,
            _lock: lock,
        })
    }
}

impl PinnedCleanupEscrowEntryV1 {
    fn open_and_read(
        output: &PinnedOutput,
        name: &str,
        exact_length: usize,
    ) -> Result<(Self, Vec<u8>), WorkerV2PublicationIntentCleanupEscrowErrorV1> {
        let descriptor = openat(
            &output.fd,
            name,
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| {
            cleanup_escrow_invalid_v1(output, name, std::io::Error::from(error).to_string())
        })?;
        let mut file = fs::File::from(descriptor);
        let before = fstat(&file).map_err(std::io::Error::from)?;
        let named =
            statat(&output.fd, name, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
        if !same_private_file(&before, &named, exact_length) {
            return Err(cleanup_escrow_invalid_v1(
                output,
                name,
                "entry does not match its pinned private inode",
            ));
        }
        let mut bytes = Vec::with_capacity(exact_length);
        Read::by_ref(&mut file)
            .take((exact_length + 1) as u64)
            .read_to_end(&mut bytes)?;
        let after = fstat(&file).map_err(std::io::Error::from)?;
        let named_after =
            statat(&output.fd, name, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
        if bytes.len() != exact_length
            || !same_private_file(&before, &after, exact_length)
            || !same_private_file(&before, &named_after, exact_length)
        {
            return Err(cleanup_escrow_invalid_v1(
                output,
                name,
                "entry changed while its pinned descriptor was read",
            ));
        }
        Ok((
            Self {
                file,
                stat: before,
                name: name.to_owned(),
            },
            bytes,
        ))
    }

    fn require_snapshot(
        &self,
        output: &PinnedOutput,
        expected: CleanupEscrowFileSnapshotV1,
    ) -> Result<(), WorkerV2PublicationIntentCleanupEscrowErrorV1> {
        if !expected.matches(&self.stat) {
            return Err(cleanup_escrow_invalid_v1(
                output,
                &self.name,
                "pinned entry differs from the opaque escrow capsule",
            ));
        }
        Ok(())
    }

    fn snapshot(
        &self,
        output: &PinnedOutput,
    ) -> Result<CleanupEscrowFileSnapshotV1, WorkerV2PublicationIntentCleanupEscrowErrorV1> {
        CleanupEscrowFileSnapshotV1::from_stat(&self.stat)
            .map_err(|reason| cleanup_escrow_invalid_v1(output, &self.name, reason))
    }

    fn require_named_as(
        &self,
        output: &PinnedOutput,
        name: &str,
    ) -> Result<(), WorkerV2PublicationIntentCleanupEscrowErrorV1> {
        let named =
            statat(&output.fd, name, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
        let expected = self.snapshot(output)?;
        if !expected.matches(&named) {
            return Err(cleanup_escrow_invalid_v1(
                output,
                name,
                "renamed entry does not match its pinned escrow inode",
            ));
        }
        Ok(())
    }

    fn require_unlinked(
        &self,
        output: &PinnedOutput,
    ) -> Result<(), WorkerV2PublicationIntentCleanupEscrowErrorV1> {
        let after = fstat(&self.file).map_err(std::io::Error::from)?;
        let name_absent = match statat(&output.fd, &self.name, AtFlags::SYMLINK_NOFOLLOW) {
            Err(error) if error == rustix::io::Errno::NOENT => true,
            Ok(_) => false,
            Err(error) => return Err(std::io::Error::from(error).into()),
        };
        if after.st_dev != self.stat.st_dev
            || after.st_ino != self.stat.st_ino
            || after.st_mode != self.stat.st_mode
            || after.st_size != self.stat.st_size
            || self.stat.st_nlink != 1
            || after.st_nlink != 0
            || !name_absent
        {
            return Err(cleanup_escrow_invalid_v1(
                output,
                &self.name,
                "unlink did not remove the pinned escrow inode exactly",
            ));
        }
        Ok(())
    }

    fn unlink_and_sync(
        self,
        exclusive: &CleanupEscrowExclusiveDirectoryV1<'_>,
        unlink_boundary: WorkerV2PublicationIntentCleanupEscrowBoundaryV1,
        sync_boundary: WorkerV2PublicationIntentCleanupEscrowBoundaryV1,
        faults: &mut CleanupEscrowFaultInjectorV1,
    ) -> Result<(), WorkerV2PublicationIntentCleanupEscrowErrorV1> {
        let output = exclusive.output;
        let named = statat(&output.fd, &self.name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(std::io::Error::from)?;
        if !same_private_file(
            &self.stat,
            &named,
            usize::try_from(self.stat.st_size).map_err(|_| {
                cleanup_escrow_invalid_v1(output, &self.name, "pinned entry size is invalid")
            })?,
        ) {
            return Err(cleanup_escrow_invalid_v1(
                output,
                &self.name,
                "entry was replaced before unlink",
            ));
        }
        faults.hit(
            unlink_boundary,
            WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::Before,
        )?;
        unlinkat(&output.fd, &self.name, AtFlags::empty()).map_err(std::io::Error::from)?;
        self.require_unlinked(output)?;
        faults.hit(
            unlink_boundary,
            WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::After,
        )?;
        faults.around(sync_boundary, || {
            fsync(&output.fd)
                .map_err(std::io::Error::from)
                .map_err(Into::into)
        })?;
        self.require_unlinked(output)
    }
}

fn cleanup_escrow_read_manifest_v1(
    output: &PinnedOutput,
    names: &CleanupEscrowNamesV1,
) -> Result<Option<CleanupEscrowManifestV1>, WorkerV2PublicationIntentCleanupEscrowErrorV1> {
    if !cleanup_escrow_entry_exists_v1(output, &names.manifest)? {
        return Ok(None);
    }
    let bytes = read_private_file::<PublicationIntentSchemaV2>(
        output,
        &names.manifest,
        CLEANUP_ESCROW_MANIFEST_BYTES_V1,
    )?;
    CleanupEscrowManifestV1::from_bytes(&bytes)
        .map(Some)
        .map_err(|reason| cleanup_escrow_invalid_v1(output, &names.manifest, reason))
}

fn cleanup_escrow_pin_manifest_v1(
    output: &PinnedOutput,
    names: &CleanupEscrowNamesV1,
) -> Result<
    (PinnedCleanupEscrowEntryV1, CleanupEscrowManifestV1),
    WorkerV2PublicationIntentCleanupEscrowErrorV1,
> {
    let (pinned, bytes) = PinnedCleanupEscrowEntryV1::open_and_read(
        output,
        &names.manifest,
        CLEANUP_ESCROW_MANIFEST_BYTES_V1,
    )?;
    let manifest = CleanupEscrowManifestV1::from_bytes(&bytes)
        .map_err(|reason| cleanup_escrow_invalid_v1(output, &names.manifest, reason))?;
    Ok((pinned, manifest))
}

fn cleanup_escrow_cleanup_temps_v1(
    exclusive: &CleanupEscrowExclusiveDirectoryV1<'_>,
    names: &CleanupEscrowNamesV1,
) -> Result<(), WorkerV2PublicationIntentCleanupEscrowErrorV1> {
    let output = exclusive.output;
    let descriptor =
        rustix::io::fcntl_dupfd_cloexec(&output.fd, 0).map_err(std::io::Error::from)?;
    let mut directory = rustix::fs::Dir::read_from(&descriptor).map_err(std::io::Error::from)?;
    let mut temps = Vec::new();
    for entry in &mut directory {
        let entry = entry.map_err(std::io::Error::from)?;
        let name = entry.file_name().to_string_lossy();
        if !name.starts_with(&names.temp_prefix) {
            continue;
        }
        if temps.len() == CLEANUP_ESCROW_MAX_TEMPS_V1 {
            return Err(cleanup_escrow_invalid_v1(
                output,
                &names.temp_prefix,
                "too many package-owned escrow manifest temporary entries",
            ));
        }
        let stat = statat(&output.fd, name.as_ref(), AtFlags::SYMLINK_NOFOLLOW)
            .map_err(std::io::Error::from)?;
        if !is_private_file(&stat) {
            return Err(cleanup_escrow_invalid_v1(
                output,
                name.as_ref(),
                "escrow manifest temporary entry is not private",
            ));
        }
        temps.push(name.into_owned());
    }
    if !temps.is_empty() {
        for name in temps {
            unlinkat(&output.fd, &name, AtFlags::empty()).map_err(std::io::Error::from)?;
        }
        fsync(&output.fd).map_err(std::io::Error::from)?;
    }
    Ok(())
}

fn cleanup_escrow_write_manifest_v1(
    exclusive: &CleanupEscrowExclusiveDirectoryV1<'_>,
    names: &CleanupEscrowNamesV1,
    manifest: CleanupEscrowManifestV1,
    replace: bool,
    faults: &mut CleanupEscrowFaultInjectorV1,
) -> Result<(), WorkerV2PublicationIntentCleanupEscrowErrorV1> {
    let output = exclusive.output;
    let bytes = manifest.to_bytes();
    let start = NEXT_TEMP_ID.fetch_add(MAX_TEMP_ATTEMPTS, Ordering::Relaxed);
    let mut reserved = None;
    for offset in 0..MAX_TEMP_ATTEMPTS {
        let temp_name = format!(
            "{}{}-{}",
            names.temp_prefix,
            std::process::id(),
            start.wrapping_add(offset)
        );
        faults.hit(
            WorkerV2PublicationIntentCleanupEscrowBoundaryV1::CreateManifestTemp,
            WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::Before,
        )?;
        match openat(
            &output.fd,
            &temp_name,
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        ) {
            Ok(descriptor) => {
                faults.hit(
                    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::CreateManifestTemp,
                    WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::After,
                )?;
                reserved = Some((temp_name, fs::File::from(descriptor)));
                break;
            }
            Err(error) if error == rustix::io::Errno::EXIST => {}
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
    }
    let (temp_name, mut temp) = reserved.ok_or_else(|| {
        cleanup_escrow_invalid_v1(
            output,
            &names.temp_prefix,
            "could not reserve a private escrow manifest temporary entry",
        )
    })?;
    let result = (|| {
        faults.around(
            WorkerV2PublicationIntentCleanupEscrowBoundaryV1::WriteManifestTemp,
            || temp.write_all(&bytes).map_err(Into::into),
        )?;
        faults.around(
            WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncManifestTemp,
            || temp.sync_all().map_err(Into::into),
        )?;
        faults.hit(
            WorkerV2PublicationIntentCleanupEscrowBoundaryV1::RenameManifest,
            WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::Before,
        )?;
        if replace {
            renameat(&output.fd, &temp_name, &output.fd, &names.manifest)
                .map_err(std::io::Error::from)?;
        } else {
            renameat_with(
                &output.fd,
                &temp_name,
                &output.fd,
                &names.manifest,
                RenameFlags::NOREPLACE,
            )
            .map_err(std::io::Error::from)?;
        }
        faults.hit(
            WorkerV2PublicationIntentCleanupEscrowBoundaryV1::RenameManifest,
            WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::After,
        )?;
        validate_renamed_file::<PublicationIntentSchemaV2>(
            output,
            &names.manifest,
            &temp,
            bytes.len(),
        )?;
        faults.around(
            WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncManifestName,
            || {
                fsync(&output.fd)
                    .map_err(std::io::Error::from)
                    .map_err(Into::into)
            },
        )?;
        let published = cleanup_escrow_read_manifest_v1(output, names)?.ok_or_else(|| {
            cleanup_escrow_invalid_v1(
                output,
                &names.manifest,
                "escrow manifest disappeared after publication",
            )
        })?;
        if published != manifest {
            return Err(cleanup_escrow_invalid_v1(
                output,
                &names.manifest,
                "escrow manifest changed after publication",
            ));
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = unlinkat(&output.fd, &temp_name, AtFlags::empty());
    }
    result
}

fn cleanup_escrow_expected_receipt_v1(
    producer: &ProducerIdentity,
    record: WorkerV2PublicationIntentRecordV2,
) -> crate::BackendPublicationReceiptV2 {
    publication_receipt_v2(
        producer,
        record.attempt(),
        record.plan(),
        record.upstream_evidence(),
        record.compiler_closure(),
    )
}

fn cleanup_escrow_pin_record_v1(
    output: &PinnedOutput,
    canonical_names: &IntentNames,
    entry: &str,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    compiler_closure: CompilerClosureV2,
    identity: WorkerV2PublicationIntentIdentityV2,
) -> Result<
    (
        PinnedCleanupEscrowEntryV1,
        WorkerV2PublicationIntentRecordV2,
    ),
    WorkerV2PublicationIntentCleanupEscrowErrorV1,
> {
    let (pinned, bytes) =
        PinnedCleanupEscrowEntryV1::open_and_read(output, entry, RECORD_BYTES_V2)?;
    let record = WorkerV2PublicationIntentRecordV2::decode(&bytes)
        .map_err(|reason| cleanup_escrow_invalid_v1(output, entry, reason))?;
    let expected_producer = producer_identity_v2(producer);
    if record.producer_identity() != expected_producer
        || record.slot != slot_identity_v2(expected_producer, attempt)
        || record.attempt() != attempt
        || record.plan().attempt() != attempt
        || canonical_names.base
            != IntentNames::new::<PublicationIntentSchemaV2>(expected_producer, record.slot).base
        || record.compiler_closure() != compiler_closure
        || record.identity() != identity
    {
        return Err(cleanup_escrow_invalid_v1(
            output,
            entry,
            "pinned escrow record differs from its exact owner binding",
        ));
    }
    Ok((pinned, record))
}

fn cleanup_escrow_pin_output_v1(
    output: &PinnedOutput,
    entry: &str,
    exact_length: usize,
    exact_identity: [u8; 32],
) -> Result<PinnedCleanupEscrowEntryV1, WorkerV2PublicationIntentCleanupEscrowErrorV1> {
    let (pinned, bytes) = PinnedCleanupEscrowEntryV1::open_and_read(output, entry, exact_length)?;
    if sha256(&bytes) != exact_identity {
        return Err(cleanup_escrow_invalid_v1(
            output,
            entry,
            "pinned escrow output digest mismatch",
        ));
    }
    Ok(pinned)
}

fn cleanup_escrow_validate_capsule_context_v1(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    capsule: WorkerV2PublicationIntentCleanupEscrowV1,
) -> Result<CleanupEscrowNamesV1, WorkerV2PublicationIntentCleanupEscrowErrorV1> {
    capsule
        .validate()
        .map_err(|reason| cleanup_escrow_invalid_v1(output, "capsule", reason))?;
    let producer_identity = producer_identity_v2(producer);
    let package = *crate::producer_package_identity_v1(producer).as_bytes();
    if capsule.producer_identity != producer_identity || capsule.package != package {
        return Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::CapsuleMismatch);
    }
    Ok(CleanupEscrowNamesV1::new(
        producer_identity,
        package,
        capsule.slot,
        capsule.intent,
    ))
}

fn cleanup_escrow_validate_prepared_payload_v1(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    capsule: WorkerV2PublicationIntentCleanupEscrowV1,
    names: &CleanupEscrowNamesV1,
) -> Result<(), WorkerV2PublicationIntentCleanupEscrowErrorV1> {
    cleanup_escrow_pin_prepared_payload_v1(output, producer, capsule, names).map(|_| ())
}

fn cleanup_escrow_pin_prepared_payload_v1(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    capsule: WorkerV2PublicationIntentCleanupEscrowV1,
    names: &CleanupEscrowNamesV1,
) -> Result<
    (
        PinnedCleanupEscrowEntryV1,
        WorkerV2PublicationIntentRecordV2,
        PinnedCleanupEscrowEntryV1,
    ),
    WorkerV2PublicationIntentCleanupEscrowErrorV1,
> {
    let canonical_names =
        IntentNames::new::<PublicationIntentSchemaV2>(capsule.producer_identity, capsule.slot);
    let (pinned_record, record) = cleanup_escrow_pin_record_v1(
        output,
        &canonical_names,
        &names.record,
        producer,
        capsule.attempt,
        capsule.receipt.compiler_closure(),
        capsule.intent,
    )?;
    pinned_record.require_snapshot(output, capsule.record_file)?;
    let pinned_output = cleanup_escrow_pin_output_v1(
        output,
        &names.output,
        record.output_length(),
        *record.output_identity().as_bytes(),
    )?;
    pinned_output.require_snapshot(output, capsule.output_file)?;
    if cleanup_escrow_expected_receipt_v1(producer, record) != capsule.receipt {
        return Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::CapsuleMismatch);
    }
    cleanup_escrow_validate_durable_receipt_v1(output, producer, capsule)?;
    Ok((pinned_record, record, pinned_output))
}

fn cleanup_escrow_validate_durable_receipt_v1(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    capsule: WorkerV2PublicationIntentCleanupEscrowV1,
) -> Result<(), WorkerV2PublicationIntentCleanupEscrowErrorV1> {
    let attempts = read_attempt_registry(output).map_err(WorkerV2PublicationIntentErrorV2::from)?;
    let current = attempts.record(&producer.stable_source).ok_or_else(|| {
        cleanup_escrow_invalid_v1(
            output,
            "attempt registry",
            "cleanup escrow producer has no durable build attempt",
        )
    })?;
    let exact_attempt = current.generation == capsule.attempt.generation()
        && current.session == capsule.attempt.session()
        && current.invocation == capsule.attempt.invocation();
    let exact_receipt = exact_attempt
        && matches!(
            current.phase,
            AttemptPhase::BackendClaimed | AttemptPhase::Completed
        )
        && current.backend_receipt == Some(BackendReceiptV1::ProvenanceV2(capsule.receipt));
    let superseded_by_same_producer = current.crate_name == producer.crate_name
        && current.generation > capsule.attempt.generation();
    if current.crate_name != producer.crate_name || (!exact_receipt && !superseded_by_same_producer)
    {
        return Err(cleanup_escrow_invalid_v1(
            output,
            "attempt registry",
            "cleanup escrow lacks its exact durable V2 receipt or a strictly newer same-producer attempt",
        ));
    }
    Ok(())
}

fn cleanup_escrow_all_intent_paths_absent_v1(
    output: &PinnedOutput,
    names: &CleanupEscrowNamesV1,
    capsule: WorkerV2PublicationIntentCleanupEscrowV1,
) -> Result<bool, WorkerV2PublicationIntentCleanupEscrowErrorV1> {
    let canonical_names =
        IntentNames::new::<PublicationIntentSchemaV2>(capsule.producer_identity, capsule.slot);
    Ok(
        !entry_exists::<PublicationIntentSchemaV2>(output, &canonical_names.record)?
            && !entry_exists::<PublicationIntentSchemaV2>(output, &canonical_names.redo)?
            && !entry_exists::<PublicationIntentSchemaV2>(output, &canonical_names.output)?
            && !cleanup_escrow_entry_exists_v1(output, &names.record)?
            && !cleanup_escrow_entry_exists_v1(output, &names.output)?,
    )
}

/// Moves one exact receipt-authorized V2 intent into an artifact-owned durable cleanup escrow.
#[allow(clippy::too_many_arguments)]
pub fn prepare_worker_v2_publication_intent_cleanup_escrow_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    compiler_closure: CompilerClosureV2,
    identity: WorkerV2PublicationIntentIdentityV2,
    receipt: crate::BackendPublicationReceiptV2,
) -> Result<WorkerV2PublicationIntentCleanupEscrowV1, WorkerV2PublicationIntentCleanupEscrowErrorV1>
{
    prepare_worker_v2_publication_intent_cleanup_escrow_v1_with_options(
        output_dir,
        producer,
        attempt,
        compiler_closure,
        identity,
        receipt,
        WorkerV2PublicationIntentCleanupEscrowOptionsV1::default(),
    )
}

/// Fault-injectable form of [`prepare_worker_v2_publication_intent_cleanup_escrow_v1`].
#[allow(clippy::too_many_arguments)]
pub fn prepare_worker_v2_publication_intent_cleanup_escrow_v1_with_options(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    compiler_closure: CompilerClosureV2,
    identity: WorkerV2PublicationIntentIdentityV2,
    receipt: crate::BackendPublicationReceiptV2,
    options: WorkerV2PublicationIntentCleanupEscrowOptionsV1,
) -> Result<WorkerV2PublicationIntentCleanupEscrowV1, WorkerV2PublicationIntentCleanupEscrowErrorV1>
{
    let output =
        PinnedOutput::open_existing(output_dir).map_err(WorkerV2PublicationIntentErrorV2::from)?;
    let lock = output
        .lock()
        .map_err(WorkerV2PublicationIntentErrorV2::from)?;
    let exclusive = CleanupEscrowExclusiveDirectoryV1::new(&output, &lock)?;
    output
        .verify_path_identity()
        .map_err(WorkerV2PublicationIntentErrorV2::from)?;
    let producer_identity = producer_identity_v2(producer);
    let package = *crate::producer_package_identity_v1(producer).as_bytes();
    let slot = slot_identity_v2(producer_identity, attempt);
    let escrow_names = CleanupEscrowNamesV1::new(producer_identity, package, slot, identity);
    cleanup_escrow_cleanup_temps_v1(&exclusive, &escrow_names)?;
    let mut faults = CleanupEscrowFaultInjectorV1::new(options);

    if cleanup_escrow_entry_exists_v1(&output, &escrow_names.manifest)? {
        let (pinned_manifest, existing) = cleanup_escrow_pin_manifest_v1(&output, &escrow_names)?;
        cleanup_escrow_validate_capsule_context_v1(&output, producer, existing.capsule)?;
        cleanup_escrow_validate_durable_receipt_v1(&output, producer, existing.capsule)?;
        if existing.capsule.attempt == attempt
            && existing.capsule.intent == identity
            && existing.capsule.receipt == receipt
            && existing.capsule.receipt.compiler_closure() == compiler_closure
        {
            if existing.state == WorkerV2PublicationIntentCleanupEscrowStateV1::Prepared {
                cleanup_escrow_validate_prepared_payload_v1(
                    &output,
                    producer,
                    existing.capsule,
                    &escrow_names,
                )?;
            }
            fsync(&output.fd).map_err(std::io::Error::from)?;
            pinned_manifest.require_named_as(&output, &escrow_names.manifest)?;
            return Ok(existing.capsule);
        }
        return Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::ConflictingEscrow);
    }

    let canonical_names = IntentNames::new::<PublicationIntentSchemaV2>(producer_identity, slot);
    cleanup_temps::<PublicationIntentSchemaV2>(&output, &canonical_names)?;
    let quarantined_record = cleanup_escrow_entry_exists_v1(&output, &escrow_names.record)?;
    let canonical_record =
        entry_exists::<PublicationIntentSchemaV2>(&output, &canonical_names.record)?;
    let redo_record = entry_exists::<PublicationIntentSchemaV2>(&output, &canonical_names.redo)?;
    if canonical_record && redo_record {
        return Err(cleanup_escrow_invalid_v1(
            &output,
            &canonical_names.record,
            "canonical and redo records coexist",
        ));
    }
    let canonical_record_entry = if canonical_record {
        Some(canonical_names.record.clone())
    } else if redo_record {
        Some(canonical_names.redo.clone())
    } else {
        None
    };
    if quarantined_record && canonical_record_entry.is_some() {
        return Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::ConflictingEscrow);
    }
    let (pinned_record, record, record_source) = if quarantined_record {
        let (pinned, record) = cleanup_escrow_pin_record_v1(
            &output,
            &canonical_names,
            &escrow_names.record,
            producer,
            attempt,
            compiler_closure,
            identity,
        )?;
        (pinned, record, None)
    } else {
        let source = canonical_record_entry.ok_or_else(|| {
            WorkerV2PublicationIntentCleanupEscrowErrorV1::Intent(
                WorkerV2PublicationIntentErrorV2::NotFound,
            )
        })?;
        let (pinned, record) = cleanup_escrow_pin_record_v1(
            &output,
            &canonical_names,
            &source,
            producer,
            attempt,
            compiler_closure,
            identity,
        )?;
        (pinned, record, Some(source))
    };
    authorize_clear::<PublicationIntentSchemaV2>(&output, producer, attempt, record)?;
    if cleanup_escrow_expected_receipt_v1(producer, record) != receipt {
        return Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::CapsuleMismatch);
    }
    if *record.plan().scope().package().as_bytes() != package {
        return Err(cleanup_escrow_invalid_v1(
            &output,
            &escrow_names.manifest,
            "intent publication scope differs from the producer package",
        ));
    }

    if let Some(source) = record_source {
        faults.around(
            WorkerV2PublicationIntentCleanupEscrowBoundaryV1::RenameRecordToQuarantine,
            || {
                renameat_with(
                    &output.fd,
                    &source,
                    &output.fd,
                    &escrow_names.record,
                    RenameFlags::NOREPLACE,
                )
                .map_err(std::io::Error::from)
                .map_err(Into::into)
            },
        )?;
        pinned_record.require_named_as(&output, &escrow_names.record)?;
        faults.around(
            WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncQuarantinedRecordName,
            || {
                fsync(&output.fd)
                    .map_err(std::io::Error::from)
                    .map_err(Into::into)
            },
        )?;
        pinned_record.require_named_as(&output, &escrow_names.record)?;
    }

    let quarantined_output = cleanup_escrow_entry_exists_v1(&output, &escrow_names.output)?;
    let canonical_output =
        entry_exists::<PublicationIntentSchemaV2>(&output, &canonical_names.output)?;
    if quarantined_output == canonical_output {
        return Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::ConflictingEscrow);
    }
    let (pinned_output, output_source) = if quarantined_output {
        (
            cleanup_escrow_pin_output_v1(
                &output,
                &escrow_names.output,
                record.output_length(),
                *record.output_identity().as_bytes(),
            )?,
            None,
        )
    } else {
        (
            cleanup_escrow_pin_output_v1(
                &output,
                &canonical_names.output,
                record.output_length(),
                *record.output_identity().as_bytes(),
            )?,
            Some(canonical_names.output.clone()),
        )
    };
    if let Some(source) = output_source {
        faults.around(
            WorkerV2PublicationIntentCleanupEscrowBoundaryV1::RenameOutputToQuarantine,
            || {
                renameat_with(
                    &output.fd,
                    &source,
                    &output.fd,
                    &escrow_names.output,
                    RenameFlags::NOREPLACE,
                )
                .map_err(std::io::Error::from)
                .map_err(Into::into)
            },
        )?;
        pinned_output.require_named_as(&output, &escrow_names.output)?;
        faults.around(
            WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncQuarantinedOutputName,
            || {
                fsync(&output.fd)
                    .map_err(std::io::Error::from)
                    .map_err(Into::into)
            },
        )?;
        pinned_output.require_named_as(&output, &escrow_names.output)?;
    }
    let capsule = WorkerV2PublicationIntentCleanupEscrowV1::new(
        package,
        record,
        receipt,
        pinned_record.snapshot(&output)?,
        pinned_output.snapshot(&output)?,
    )
    .map_err(|reason| cleanup_escrow_invalid_v1(&output, &escrow_names.manifest, reason))?;
    cleanup_escrow_write_manifest_v1(
        &exclusive,
        &escrow_names,
        CleanupEscrowManifestV1 {
            state: WorkerV2PublicationIntentCleanupEscrowStateV1::Prepared,
            capsule,
        },
        false,
        &mut faults,
    )?;
    pinned_record.require_named_as(&output, &escrow_names.record)?;
    pinned_output.require_named_as(&output, &escrow_names.output)?;
    Ok(capsule)
}

pub fn recover_worker_v2_publication_intent_cleanup_escrow_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    capsule: WorkerV2PublicationIntentCleanupEscrowV1,
) -> Result<
    WorkerV2PublicationIntentCleanupEscrowStateV1,
    WorkerV2PublicationIntentCleanupEscrowErrorV1,
> {
    let output =
        PinnedOutput::open_existing(output_dir).map_err(WorkerV2PublicationIntentErrorV2::from)?;
    let lock = output
        .lock()
        .map_err(WorkerV2PublicationIntentErrorV2::from)?;
    let exclusive = CleanupEscrowExclusiveDirectoryV1::new(&output, &lock)?;
    output
        .verify_path_identity()
        .map_err(WorkerV2PublicationIntentErrorV2::from)?;
    let names = cleanup_escrow_validate_capsule_context_v1(&output, producer, capsule)?;
    cleanup_escrow_cleanup_temps_v1(&exclusive, &names)?;
    cleanup_escrow_validate_durable_receipt_v1(&output, producer, capsule)?;
    if !cleanup_escrow_entry_exists_v1(&output, &names.manifest)? {
        if !cleanup_escrow_all_intent_paths_absent_v1(&output, &names, capsule)? {
            return Err(cleanup_escrow_invalid_v1(
                &output,
                &names.manifest,
                "manifest is absent while canonical or quarantined intent state remains",
            ));
        }
        fsync(&output.fd).map_err(std::io::Error::from)?;
        return Ok(WorkerV2PublicationIntentCleanupEscrowStateV1::Committed);
    }
    let (pinned_manifest, manifest) = cleanup_escrow_pin_manifest_v1(&output, &names)?;
    if manifest.capsule != capsule {
        return Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::CapsuleMismatch);
    }
    let (pinned_record, pinned_output) = match manifest.state {
        WorkerV2PublicationIntentCleanupEscrowStateV1::Prepared => {
            let (record, _, output_entry) =
                cleanup_escrow_pin_prepared_payload_v1(&output, producer, capsule, &names)?;
            (Some(record), Some(output_entry))
        }
        WorkerV2PublicationIntentCleanupEscrowStateV1::Committed => {
            let canonical_names = IntentNames::new::<PublicationIntentSchemaV2>(
                capsule.producer_identity,
                capsule.slot,
            );
            if entry_exists::<PublicationIntentSchemaV2>(&output, &canonical_names.record)?
                || entry_exists::<PublicationIntentSchemaV2>(&output, &canonical_names.redo)?
                || entry_exists::<PublicationIntentSchemaV2>(&output, &canonical_names.output)?
            {
                return Err(cleanup_escrow_invalid_v1(
                    &output,
                    &names.manifest,
                    "committed escrow coexists with canonical intent state",
                ));
            }
            let record = if cleanup_escrow_entry_exists_v1(&output, &names.record)? {
                let (pinned, record) = cleanup_escrow_pin_record_v1(
                    &output,
                    &canonical_names,
                    &names.record,
                    producer,
                    capsule.attempt,
                    capsule.receipt.compiler_closure(),
                    capsule.intent,
                )?;
                pinned.require_snapshot(&output, capsule.record_file)?;
                Some((pinned, record))
            } else {
                None
            };
            let output_entry = if cleanup_escrow_entry_exists_v1(&output, &names.output)? {
                let length = usize::try_from(capsule.output_file.byte_len).map_err(|_| {
                    cleanup_escrow_invalid_v1(
                        &output,
                        &names.output,
                        "capsule output length is invalid",
                    )
                })?;
                let pinned = cleanup_escrow_pin_output_v1(
                    &output,
                    &names.output,
                    length,
                    capsule.output_identity,
                )?;
                pinned.require_snapshot(&output, capsule.output_file)?;
                if let Some((_, record)) = &record
                    && (record.output_length() != length
                        || *record.output_identity().as_bytes() != capsule.output_identity)
                {
                    return Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::CapsuleMismatch);
                }
                Some(pinned)
            } else {
                None
            };
            (record.map(|(pinned, _)| pinned), output_entry)
        }
    };
    fsync(&output.fd).map_err(std::io::Error::from)?;
    pinned_manifest.require_named_as(&output, &names.manifest)?;
    if let Some(pinned) = &pinned_record {
        pinned.require_named_as(&output, &names.record)?;
    }
    if let Some(pinned) = &pinned_output {
        pinned.require_named_as(&output, &names.output)?;
    }
    Ok(manifest.state)
}

pub fn commit_worker_v2_publication_intent_cleanup_escrow_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    capsule: WorkerV2PublicationIntentCleanupEscrowV1,
) -> Result<(), WorkerV2PublicationIntentCleanupEscrowErrorV1> {
    commit_worker_v2_publication_intent_cleanup_escrow_v1_with_options(
        output_dir,
        producer,
        capsule,
        WorkerV2PublicationIntentCleanupEscrowOptionsV1::default(),
    )
}

pub fn commit_worker_v2_publication_intent_cleanup_escrow_v1_with_options(
    output_dir: &Path,
    producer: &ProducerIdentity,
    capsule: WorkerV2PublicationIntentCleanupEscrowV1,
    options: WorkerV2PublicationIntentCleanupEscrowOptionsV1,
) -> Result<(), WorkerV2PublicationIntentCleanupEscrowErrorV1> {
    let output =
        PinnedOutput::open_existing(output_dir).map_err(WorkerV2PublicationIntentErrorV2::from)?;
    let lock = output
        .lock()
        .map_err(WorkerV2PublicationIntentErrorV2::from)?;
    let exclusive = CleanupEscrowExclusiveDirectoryV1::new(&output, &lock)?;
    commit_worker_v2_publication_intent_cleanup_escrow_locked_v1(
        &exclusive, producer, capsule, options,
    )
}

fn commit_worker_v2_publication_intent_cleanup_escrow_locked_v1(
    exclusive: &CleanupEscrowExclusiveDirectoryV1<'_>,
    producer: &ProducerIdentity,
    capsule: WorkerV2PublicationIntentCleanupEscrowV1,
    options: WorkerV2PublicationIntentCleanupEscrowOptionsV1,
) -> Result<(), WorkerV2PublicationIntentCleanupEscrowErrorV1> {
    let output = exclusive.output;
    output
        .verify_path_identity()
        .map_err(WorkerV2PublicationIntentErrorV2::from)?;
    let names = cleanup_escrow_validate_capsule_context_v1(output, producer, capsule)?;
    cleanup_escrow_cleanup_temps_v1(exclusive, &names)?;
    cleanup_escrow_validate_durable_receipt_v1(output, producer, capsule)?;
    let Some(manifest) = cleanup_escrow_read_manifest_v1(output, &names)? else {
        if !cleanup_escrow_all_intent_paths_absent_v1(output, &names, capsule)? {
            return Err(cleanup_escrow_invalid_v1(
                output,
                &names.manifest,
                "manifest is absent while canonical or quarantined intent state remains",
            ));
        }
        fsync(&output.fd).map_err(std::io::Error::from)?;
        return Ok(());
    };
    if manifest.capsule != capsule {
        return Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::CapsuleMismatch);
    }
    let mut faults = CleanupEscrowFaultInjectorV1::new(options);
    let prepared_payload = if manifest.state
        == WorkerV2PublicationIntentCleanupEscrowStateV1::Prepared
    {
        let payload = cleanup_escrow_pin_prepared_payload_v1(output, producer, capsule, &names)?;
        cleanup_escrow_write_manifest_v1(
            exclusive,
            &names,
            CleanupEscrowManifestV1 {
                state: WorkerV2PublicationIntentCleanupEscrowStateV1::Committed,
                capsule,
            },
            true,
            &mut faults,
        )?;
        Some(payload)
    } else {
        None
    };

    let (pinned_manifest, committed_manifest) = cleanup_escrow_pin_manifest_v1(output, &names)?;
    if committed_manifest
        != (CleanupEscrowManifestV1 {
            state: WorkerV2PublicationIntentCleanupEscrowStateV1::Committed,
            capsule,
        })
    {
        return Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::CapsuleMismatch);
    }

    let canonical_names =
        IntentNames::new::<PublicationIntentSchemaV2>(capsule.producer_identity, capsule.slot);
    if entry_exists::<PublicationIntentSchemaV2>(output, &canonical_names.record)?
        || entry_exists::<PublicationIntentSchemaV2>(output, &canonical_names.redo)?
        || entry_exists::<PublicationIntentSchemaV2>(output, &canonical_names.output)?
    {
        return Err(cleanup_escrow_invalid_v1(
            output,
            &names.manifest,
            "commit refuses canonical intent state beside its escrow",
        ));
    }
    let (prepared_record, prepared_output) =
        if let Some((record, value, output_entry)) = prepared_payload {
            record.require_named_as(output, &names.record)?;
            output_entry.require_named_as(output, &names.output)?;
            (Some((record, value)), Some(output_entry))
        } else {
            (None, None)
        };
    let pinned_record = if let Some(record) = prepared_record {
        Some(record)
    } else if cleanup_escrow_entry_exists_v1(output, &names.record)? {
        let (pinned, record) = cleanup_escrow_pin_record_v1(
            output,
            &canonical_names,
            &names.record,
            producer,
            capsule.attempt,
            capsule.receipt.compiler_closure(),
            capsule.intent,
        )?;
        pinned.require_snapshot(output, capsule.record_file)?;
        Some((pinned, record))
    } else {
        None
    };
    let pinned_output = if let Some(output_entry) = prepared_output {
        Some(output_entry)
    } else if cleanup_escrow_entry_exists_v1(output, &names.output)? {
        let length = usize::try_from(capsule.output_file.byte_len).map_err(|_| {
            cleanup_escrow_invalid_v1(output, &names.output, "capsule output length is invalid")
        })?;
        let pinned =
            cleanup_escrow_pin_output_v1(output, &names.output, length, capsule.output_identity)?;
        pinned.require_snapshot(output, capsule.output_file)?;
        if let Some((_, record)) = &pinned_record
            && (record.output_length() != length
                || *record.output_identity().as_bytes() != capsule.output_identity)
        {
            return Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::CapsuleMismatch);
        }
        Some(pinned)
    } else {
        None
    };
    if let Some((pinned, _)) = pinned_record {
        pinned.unlink_and_sync(
            exclusive,
            WorkerV2PublicationIntentCleanupEscrowBoundaryV1::UnlinkQuarantinedRecord,
            WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncQuarantinedRecordDeletion,
            &mut faults,
        )?;
    }
    if let Some(pinned) = pinned_output {
        pinned.unlink_and_sync(
            exclusive,
            WorkerV2PublicationIntentCleanupEscrowBoundaryV1::UnlinkQuarantinedOutput,
            WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncQuarantinedOutputDeletion,
            &mut faults,
        )?;
    }
    pinned_manifest.unlink_and_sync(
        exclusive,
        WorkerV2PublicationIntentCleanupEscrowBoundaryV1::UnlinkCommittedManifest,
        WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncCommittedManifestDeletion,
        &mut faults,
    )?;
    if !cleanup_escrow_all_intent_paths_absent_v1(output, &names, capsule)?
        || cleanup_escrow_entry_exists_v1(output, &names.manifest)?
    {
        return Err(cleanup_escrow_invalid_v1(
            output,
            &names.manifest,
            "committed escrow deletion left durable state behind",
        ));
    }
    Ok(())
}

/// Commits one exact predecessor escrow only while an exact newer V2 intent remains current.
///
/// This operation consumes an already acquired exact successor lease and performs no second lock
/// acquisition. Under that lease's retained lock it joins the explicit successor attempt,
/// compiler closure, and intent identity to `expected_successor`, compares the complete canonical
/// record and exact output bytes, revalidates their pinned inodes, commits the predecessor escrow,
/// and revalidates the same directory and file bindings again. The returned evidence is bounded
/// and inert; it contains no file payload or authority.
///
/// Production callers pass [`WorkerV2PublicationIntentCleanupEscrowOptionsV1::default`]. Fault
/// injection exists only to qualify every predecessor commit durability boundary.
#[allow(clippy::too_many_arguments)]
pub fn commit_worker_v2_publication_intent_cleanup_escrow_after_exact_successor_v2(
    successor_lease: WorkerV2PublicationIntentLeaseV2,
    producer: &ProducerIdentity,
    predecessor: WorkerV2PublicationIntentCleanupEscrowV1,
    successor_attempt: BuildAttempt,
    successor_compiler_closure: CompilerClosureV2,
    successor_intent: WorkerV2PublicationIntentIdentityV2,
    expected_successor: &RecoveredWorkerV2PublicationIntentV2,
    options: WorkerV2PublicationIntentCleanupEscrowOptionsV1,
) -> Result<
    WorkerV2PublicationIntentCleanupEscrowCommitEvidenceV2,
    WorkerV2PublicationIntentCleanupEscrowErrorV1,
> {
    if successor_attempt.generation() <= predecessor.attempt().generation() {
        return Err(
            WorkerV2PublicationIntentCleanupEscrowErrorV1::SuccessorGenerationNotNewer {
                predecessor: predecessor.attempt().generation(),
                successor: successor_attempt.generation(),
            },
        );
    }
    let expected_record = expected_successor.record();
    if successor_lease.producer != *producer
        || successor_lease.attempt != successor_attempt
        || successor_lease.compiler_closure != successor_compiler_closure
        || successor_lease.recovered.record().identity() != successor_intent
        || successor_lease.recovered.record() != expected_record
        || successor_lease.recovered.exact_output() != expected_successor.exact_output()
        || expected_record.attempt() != successor_attempt
        || expected_record.compiler_closure() != successor_compiler_closure
        || expected_record.identity() != successor_intent
    {
        return Err(
            WorkerV2PublicationIntentCleanupEscrowErrorV1::SuccessorExpectationMismatch,
        );
    }
    let successor = WorkerV2PublicationIntentLeaseSnapshotV2 {
        recovered: successor_lease.recovered.clone(),
        record_file: successor_lease.record_file,
        output_file: successor_lease.output_file,
    };
    let exclusive = CleanupEscrowExclusiveDirectoryV1::new(
        &successor_lease.output,
        &successor_lease._lock,
    )?;
    successor_lease
        .output
        .verify_path_identity()
        .map_err(WorkerV2PublicationIntentErrorV2::from)?;
    cleanup_escrow_validate_capsule_context_v1(
        &successor_lease.output,
        producer,
        predecessor,
    )?;
    revalidate_worker_v2_publication_intent_locked_v2(
        &successor_lease.output,
        producer,
        successor_attempt,
        successor_compiler_closure,
        &successor,
    )?;
    commit_worker_v2_publication_intent_cleanup_escrow_locked_v1(
        &exclusive,
        producer,
        predecessor,
        options,
    )?;
    revalidate_worker_v2_publication_intent_locked_v2(
        &successor_lease.output,
        producer,
        successor_attempt,
        successor_compiler_closure,
        &successor,
    )?;
    CleanupEscrowExclusiveDirectoryV1::new(
        &successor_lease.output,
        &successor_lease._lock,
    )?;

    Ok(WorkerV2PublicationIntentCleanupEscrowCommitEvidenceV2 {
        predecessor: predecessor.identity(),
        successor: successor.recovered.record(),
    })
}

pub fn rollback_worker_v2_publication_intent_cleanup_escrow_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    capsule: WorkerV2PublicationIntentCleanupEscrowV1,
) -> Result<(), WorkerV2PublicationIntentCleanupEscrowErrorV1> {
    rollback_worker_v2_publication_intent_cleanup_escrow_v1_with_options(
        output_dir,
        producer,
        capsule,
        WorkerV2PublicationIntentCleanupEscrowOptionsV1::default(),
    )
}

pub fn rollback_worker_v2_publication_intent_cleanup_escrow_v1_with_options(
    output_dir: &Path,
    producer: &ProducerIdentity,
    capsule: WorkerV2PublicationIntentCleanupEscrowV1,
    options: WorkerV2PublicationIntentCleanupEscrowOptionsV1,
) -> Result<(), WorkerV2PublicationIntentCleanupEscrowErrorV1> {
    let output =
        PinnedOutput::open_existing(output_dir).map_err(WorkerV2PublicationIntentErrorV2::from)?;
    let lock = output
        .lock()
        .map_err(WorkerV2PublicationIntentErrorV2::from)?;
    let exclusive = CleanupEscrowExclusiveDirectoryV1::new(&output, &lock)?;
    output
        .verify_path_identity()
        .map_err(WorkerV2PublicationIntentErrorV2::from)?;
    let names = cleanup_escrow_validate_capsule_context_v1(&output, producer, capsule)?;
    cleanup_escrow_cleanup_temps_v1(&exclusive, &names)?;
    let Some(manifest) = cleanup_escrow_read_manifest_v1(&output, &names)? else {
        let canonical_names =
            IntentNames::new::<PublicationIntentSchemaV2>(capsule.producer_identity, capsule.slot);
        let recovered = recover_locked::<PublicationIntentSchemaV2>(
            &output,
            &canonical_names,
            producer,
            capsule.attempt,
        )?
        .ok_or_else(|| {
            cleanup_escrow_invalid_v1(
                &output,
                &names.manifest,
                "neither durable escrow nor rolled-back canonical intent exists",
            )
        })?;
        if recovered.record.identity() != capsule.intent
            || recovered.record.compiler_closure() != capsule.receipt.compiler_closure()
            || *recovered.record.output_identity().as_bytes() != capsule.output_identity
        {
            return Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::CapsuleMismatch);
        }
        fsync(&output.fd).map_err(std::io::Error::from)?;
        return Ok(());
    };
    if manifest.capsule != capsule {
        return Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::CapsuleMismatch);
    }
    if manifest.state != WorkerV2PublicationIntentCleanupEscrowStateV1::Prepared {
        return Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::InvalidTransition);
    }
    let (pinned_manifest, pinned_manifest_value) = cleanup_escrow_pin_manifest_v1(&output, &names)?;
    if pinned_manifest_value != manifest {
        return Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::CapsuleMismatch);
    }
    cleanup_escrow_validate_durable_receipt_v1(&output, producer, capsule)?;
    let canonical_names =
        IntentNames::new::<PublicationIntentSchemaV2>(capsule.producer_identity, capsule.slot);
    let mut faults = CleanupEscrowFaultInjectorV1::new(options);

    let quarantined_output = cleanup_escrow_entry_exists_v1(&output, &names.output)?;
    let canonical_output =
        entry_exists::<PublicationIntentSchemaV2>(&output, &canonical_names.output)?;
    if quarantined_output == canonical_output {
        return Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::ConflictingEscrow);
    }
    let pinned_output = cleanup_escrow_pin_output_v1(
        &output,
        if quarantined_output {
            &names.output
        } else {
            &canonical_names.output
        },
        usize::try_from(capsule.output_file.byte_len).map_err(|_| {
            cleanup_escrow_invalid_v1(&output, &names.output, "capsule output length is invalid")
        })?,
        capsule.output_identity,
    )?;
    pinned_output.require_snapshot(&output, capsule.output_file)?;
    if quarantined_output {
        faults.around(
            WorkerV2PublicationIntentCleanupEscrowBoundaryV1::RenameOutputToCanonical,
            || {
                renameat_with(
                    &output.fd,
                    &names.output,
                    &output.fd,
                    &canonical_names.output,
                    RenameFlags::NOREPLACE,
                )
                .map_err(std::io::Error::from)
                .map_err(Into::into)
            },
        )?;
        pinned_output.require_named_as(&output, &canonical_names.output)?;
        faults.around(
            WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncCanonicalOutputName,
            || {
                fsync(&output.fd)
                    .map_err(std::io::Error::from)
                    .map_err(Into::into)
            },
        )?;
        pinned_output.require_named_as(&output, &canonical_names.output)?;
    }

    let quarantined_record = cleanup_escrow_entry_exists_v1(&output, &names.record)?;
    let canonical_record =
        entry_exists::<PublicationIntentSchemaV2>(&output, &canonical_names.record)?;
    let redo_record = entry_exists::<PublicationIntentSchemaV2>(&output, &canonical_names.redo)?;
    if redo_record {
        return Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::ConflictingEscrow);
    }
    if quarantined_record == canonical_record {
        return Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::ConflictingEscrow);
    }
    let (pinned_record, pinned_record_value) = cleanup_escrow_pin_record_v1(
        &output,
        &canonical_names,
        if quarantined_record {
            &names.record
        } else {
            &canonical_names.record
        },
        producer,
        capsule.attempt,
        capsule.receipt.compiler_closure(),
        capsule.intent,
    )?;
    pinned_record.require_snapshot(&output, capsule.record_file)?;
    if *pinned_record_value.output_identity().as_bytes() != capsule.output_identity {
        return Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::CapsuleMismatch);
    }
    if quarantined_record {
        faults.around(
            WorkerV2PublicationIntentCleanupEscrowBoundaryV1::RenameRecordToCanonical,
            || {
                renameat_with(
                    &output.fd,
                    &names.record,
                    &output.fd,
                    &canonical_names.record,
                    RenameFlags::NOREPLACE,
                )
                .map_err(std::io::Error::from)
                .map_err(Into::into)
            },
        )?;
        pinned_record.require_named_as(&output, &canonical_names.record)?;
        faults.around(
            WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncCanonicalRecordName,
            || {
                fsync(&output.fd)
                    .map_err(std::io::Error::from)
                    .map_err(Into::into)
            },
        )?;
        pinned_record.require_named_as(&output, &canonical_names.record)?;
    }

    let recovered = recover_locked::<PublicationIntentSchemaV2>(
        &output,
        &canonical_names,
        producer,
        capsule.attempt,
    )?
    .ok_or_else(|| {
        cleanup_escrow_invalid_v1(
            &output,
            &canonical_names.record,
            "rollback did not restore the canonical intent",
        )
    })?;
    if recovered.record.identity() != capsule.intent
        || recovered.record.compiler_closure() != capsule.receipt.compiler_closure()
        || *recovered.record.output_identity().as_bytes() != capsule.output_identity
    {
        return Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::CapsuleMismatch);
    }
    pinned_record.require_named_as(&output, &canonical_names.record)?;
    pinned_output.require_named_as(&output, &canonical_names.output)?;
    pinned_manifest.unlink_and_sync(
        &exclusive,
        WorkerV2PublicationIntentCleanupEscrowBoundaryV1::UnlinkRolledBackManifest,
        WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncRolledBackManifestDeletion,
        &mut faults,
    )?;
    pinned_record.require_named_as(&output, &canonical_names.record)?;
    pinned_output.require_named_as(&output, &canonical_names.output)?;
    if cleanup_escrow_entry_exists_v1(&output, &names.record)?
        || cleanup_escrow_entry_exists_v1(&output, &names.output)?
        || cleanup_escrow_entry_exists_v1(&output, &names.manifest)?
    {
        return Err(cleanup_escrow_invalid_v1(
            &output,
            &names.manifest,
            "rollback retained artifact-owned escrow state",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod cleanup_escrow_private_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn pinned_unlink_rejects_a_preexisting_replacement_under_the_protocol_lock() {
        let directory = std::env::temp_dir().join(format!(
            "fe2o3-cleanup-escrow-pinned-race-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        let victim = directory.join("victim");
        let displaced = directory.join("displaced");
        fs::write(&victim, b"original").unwrap();
        fs::set_permissions(&victim, fs::Permissions::from_mode(0o600)).unwrap();

        let output = PinnedOutput::open_existing(&directory).unwrap();
        let lock = output.lock().unwrap();
        let exclusive = CleanupEscrowExclusiveDirectoryV1::new(&output, &lock).unwrap();
        let (pinned, bytes) =
            PinnedCleanupEscrowEntryV1::open_and_read(&output, "victim", 8).unwrap();
        assert_eq!(bytes, b"original");
        fs::rename(&victim, &displaced).unwrap();
        fs::write(&victim, b"replaced").unwrap();
        fs::set_permissions(&victim, fs::Permissions::from_mode(0o600)).unwrap();

        let error = pinned
            .unlink_and_sync(
                &exclusive,
                WorkerV2PublicationIntentCleanupEscrowBoundaryV1::UnlinkCommittedManifest,
                WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncCommittedManifestDeletion,
                &mut CleanupEscrowFaultInjectorV1::new(
                    WorkerV2PublicationIntentCleanupEscrowOptionsV1::default(),
                ),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            WorkerV2PublicationIntentCleanupEscrowErrorV1::InvalidEscrow { .. }
        ));
        assert_eq!(fs::read(&victim).unwrap(), b"replaced");
        assert_eq!(fs::read(&displaced).unwrap(), b"original");
        fs::remove_dir_all(directory).unwrap();
    }
}
