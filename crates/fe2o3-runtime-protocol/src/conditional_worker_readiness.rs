//! Actual conditional replay custody without a nominal V89 or host-load cast.
use crate::conditional_worker_readiness_codec::{
    self as codec, ConditionalWorkerReadinessCodecErrorV5 as CodecError,
    InertConditionalWorkerReadinessWireV5 as Wire,
};
use fe2o3_artifact_transaction::{
    CompilerExecutionSubjectErrorV3, InertCompilerExecutionSubjectV3 as Subject,
    MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V3 as CLAIM_BYTES,
    MAX_WORKER_V3_PUBLICATION_INTENT_RECORD_BYTES_V1 as RECORD_BYTES,
    MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1 as PROVIDERS, ProducerIdentity,
    VerifiedWorkerV3LoadEnvelopeAuthorityV1 as Authority, WorkerV3LoadEnvelopeBindingV1 as Binding,
    WorkerV3LoadReadinessErrorV1, WorkerV3LoadReadinessResultV1 as Readiness,
    publish_worker_v3_load_readiness_v1,
};
use fe2o3_compiler_execution_protocol::CompilerExecutionReceiptCarriageV3 as Carriage;
use fe2o3_hsaco_finalize::{
    ConditionalWorkerOutputErrorV5, ConditionalWorkerReplayPreimagesV5 as Preimages,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{fmt, mem::size_of, path::Path};

const WINDOW: usize = fe2o3_compiler_ffi::MAX_INERT_REFINED_FORWARDING_STORAGE_V1;
const FRAME: usize = 4 * size_of::<ConditionalWorkerReadinessEnvelopeV5<'static, 'static>>()
    + 2 * (CLAIM_BYTES + RECORD_BYTES)
    + PROVIDERS * size_of::<&[u8]>()
    + 4096;

#[derive(Debug)]
pub enum ConditionalWorkerReadinessErrorV5 {
    Resource(Resource),
    Output(ConditionalWorkerOutputErrorV5),
    Subject(CompilerExecutionSubjectErrorV3),
    Codec(CodecError),
    Readiness(WorkerV3LoadReadinessErrorV1),
    Mismatch(&'static str),
}
type Error = ConditionalWorkerReadinessErrorV5;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<CodecError> for Error {
    fn from(e: CodecError) -> Self {
        Self::Codec(e)
    }
}
impl From<ConditionalWorkerOutputErrorV5> for Error {
    fn from(e: ConditionalWorkerOutputErrorV5) -> Self {
        Self::Output(e)
    }
}
impl From<CompilerExecutionSubjectErrorV3> for Error {
    fn from(e: CompilerExecutionSubjectErrorV3) -> Self {
        Self::Subject(e)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "conditional replay custody: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Complete encoded replay preimages with their original published source and
/// signed-carriage owners still borrowed. Root approval/execution authentication
/// remains the enclosing parent's responsibility. This is neither an old-family
/// envelope nor a protected semantic-to-machine proof or host load capability.
/// ```compile_fail
/// use fe2o3_runtime_protocol::{ConditionalWorkerReadinessEnvelopeV5 as C, WorkerV3LoadEnvelopeV2 as V};
/// fn downgrade(value: C<'_, '_>) -> V { value.into() }
/// ```
pub struct ConditionalWorkerReadinessEnvelopeV5<'a, 'p> {
    replay: &'a Preimages<'p>,
    carriage: &'a Carriage,
    exact: Vec<u8>,
    binding: Binding,
    retained_storage: usize,
}

impl<'a, 'p> ConditionalWorkerReadinessEnvelopeV5<'a, 'p> {
    /// Returns this envelope's additional complete storage unreserved. Both
    /// borrowed owners and the publication behind `replay` remain prepaid.
    pub fn in_original_account(
        replay: &'a Preimages<'p>,
        carriage: &'a Carriage,
        producer: &ProducerIdentity,
        b: &mut Budget<'_>,
    ) -> Result<Self> {
        let inputs = input_storage(replay, carriage)?;
        let outside = b.storage();
        let overlap = inputs
            .checked_add(FRAME)
            .and_then(|n| n.checked_add(Budget::STORAGE_WINDOW_SCRATCH_V1))
            .ok_or(Resource::Arithmetic)?;
        b.with_additional_storage_window_v1(WINDOW, |b| {
            b.with_prepaid_scope(outside.max(inputs), 8, 8, overlap, |b| {
                replay.revalidate(producer, b)?;
                require_subject(replay, carriage, b)?;
                let record = replay
                    .record()
                    .encode_canonical()
                    .map_err(|_| Error::Mismatch("canonical intent record"))?;
                let claim = replay
                    .published_claim()
                    .encode_canonical()
                    .map_err(|_| Error::Mismatch("canonical physical claim"))?;
                let fields = [
                    record.as_slice(),
                    claim.as_slice(),
                    replay.outer_handoff(),
                    replay.transcript(),
                    carriage.canonical_bytes().as_slice(),
                ];
                let count = replay.providers().len();
                if count > PROVIDERS {
                    return Err(Resource::Accounting.into());
                }
                let length = codec::encoded_length(
                    fields.map(<[u8]>::len),
                    count,
                    replay.providers().payload_length(),
                )?;
                // Copy, canonical checksum, framing recheck and exact binding
                // each traverse at most one full wire. Cached provider archive
                // identities are not rehashed without an additional debit.
                b.charge_work(
                    length
                        .checked_mul(4)
                        .and_then(|n| n.checked_add(FRAME))
                        .ok_or(Resource::Arithmetic)?,
                )?;
                b.reserve_storage(length)?;
                let mut providers = [&[][..]; PROVIDERS];
                for (slot, bytes) in providers.iter_mut().zip(replay.providers().iter()) {
                    *slot = bytes;
                }
                let exact = codec::encode(fields, providers[..count].iter().copied())?;
                if exact.capacity() > length {
                    b.reserve_storage(exact.capacity() - length)?;
                }
                Wire::decode(&exact, length)?;
                let binding = Binding::from_exact_bytes(&exact)
                    .map_err(|_| Error::Mismatch("exact envelope binding"))?;
                replay.revalidate(producer, b)?;
                let retained_storage = size_of::<Self>()
                    .checked_add(exact.capacity())
                    .ok_or(Resource::Arithmetic)?;
                b.reserve_storage(size_of::<Self>())?;
                Ok(Self {
                    replay,
                    carriage,
                    exact,
                    binding,
                    retained_storage,
                })
            })
        })
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.exact
    }
    pub const fn required_retained_storage(&self) -> usize {
        self.retained_storage
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    /// Same-account prepublication validation. The exact wire is private and
    /// immutable; this checks original source/lease and signed Subject again.
    pub fn revalidate(&self, producer: &ProducerIdentity, b: &mut Budget<'_>) -> Result<()> {
        let inputs = input_storage(self.replay, self.carriage)?
            .checked_add(self.retained_storage)
            .ok_or(Resource::Arithmetic)?;
        let outside = b.storage();
        let overlap = inputs
            .checked_add(FRAME)
            .and_then(|n| n.checked_add(Budget::STORAGE_WINDOW_SCRATCH_V1))
            .ok_or(Resource::Arithmetic)?;
        b.with_additional_storage_window_v1(WINDOW, |b| {
            b.with_prepaid_scope(outside.max(inputs), 8, 8, overlap, |b| {
                self.replay.revalidate(producer, b)?;
                require_subject(self.replay, self.carriage, b)?;
                b.charge_work(self.exact.len())?;
                if Binding::from_exact_bytes(&self.exact)
                    .map_err(|_| Error::Mismatch("exact envelope binding"))?
                    != self.binding
                {
                    return Err(Error::Mismatch("retained envelope bytes"));
                }
                Ok(())
            })
        })
    }

    /// Runs the schema-neutral durable custody transaction. The actual source
    /// and carriage remain borrowed; successful output grants only replay-file
    /// retirement, never HSA load. The returned full result charge is unreserved.
    /// Do not enclose this terminal call in a blanket-refund scope: error or
    /// unwind retains partial reservations; only known success releases scratch.
    pub fn persist(
        self,
        output: &Path,
        producer: &ProducerIdentity,
        b: &mut Budget<'_>,
    ) -> Result<(Readiness, usize)> {
        let inputs = input_storage(self.replay, self.carriage)?
            .checked_add(self.retained_storage)
            .ok_or(Resource::Arithmetic)?;
        b.charge_work(8)?;
        if b.storage() < inputs {
            return Err(Resource::Accounting.into());
        }
        let overlap = inputs
            .checked_add(FRAME)
            .and_then(|n| n.checked_add(Budget::STORAGE_WINDOW_SCRATCH_V1))
            .ok_or(Resource::Arithmetic)?;
        b.with_additional_storage_window_v1(WINDOW, |b| {
            b.reserve_storage(overlap)?;
            self.revalidate(producer, b)?;
            let authority = self.custody_authority()?;
            let readiness = publish_worker_v3_load_readiness_v1(
                output,
                self.replay.published_claim(),
                authority,
                self.exact,
            )
            .map_err(Error::Readiness)?;
            let storage = readiness
                .retained_rust_storage()
                .ok_or(Resource::Arithmetic)?;
            b.reserve_storage(storage)?;
            self.replay.revalidate(producer, b)?;
            b.charge_work(readiness.exact_envelope_bytes().len())?;
            if readiness.receipt().envelope_binding() != self.binding
                || readiness.published_claim() != self.replay.published_claim()
                || Binding::from_exact_bytes(readiness.exact_envelope_bytes())
                    .map_err(|_| Error::Mismatch("persisted envelope binding"))?
                    != self.binding
            {
                return Err(Error::Mismatch("persisted exact replay custody"));
            }
            b.release_storage(overlap.checked_add(storage).ok_or(Resource::Arithmetic)?)?;
            Ok((readiness, storage))
        })
    }

    #[allow(
        unsafe_code,
        reason = "one private conditional bridge follows actual replay, exact Subject, physical lease and immutable wire validation"
    )]
    fn custody_authority(&self) -> Result<Authority> {
        // SAFETY: only persist calls this after revalidating this private owner.
        // Construction borrowed an actual independently replayed conditional
        // publication; Preimages re-extracted every provider with the canonical
        // Worker parser and matched the entire intent. Our canonical wire retains
        // every exact outer/provider/transcript byte and matching signed Subject.
        // The same physical current claim owns the artifact; publication itself
        // checks that claim under its lock. This bridge permits only durable
        // replay custody and duplicate-file retirement, not semantic load.
        unsafe {
            Authority::from_complete_compact_replay_preimages_unchecked(
                self.binding,
                self.replay.published_claim(),
            )
        }
        .map_err(|_| Error::Mismatch("replay custody physical claim"))
    }
}

fn input_storage(replay: &Preimages<'_>, carriage: &Carriage) -> Result<usize> {
    replay
        .publication()
        .required_retained_storage()
        .checked_add(replay.required_retained_storage())
        .and_then(|n| n.checked_add(carriage.retained_storage()))
        .ok_or(Resource::Arithmetic.into())
}
fn require_subject(replay: &Preimages<'_>, carriage: &Carriage, b: &mut Budget<'_>) -> Result<()> {
    let source = replay
        .publication()
        .recovered_evidence()
        .finalized()
        .source();
    let (subject, storage) = Subject::from_publication_in_original_account_v3(
        source.binding().receipt(),
        source.recovered_handoff().handoff(),
        b,
    )?;
    b.reserve_storage(storage.retained_storage())?;
    b.charge_work(subject.canonical_bytes().len())?;
    let matches = subject.canonical_bytes() == carriage.request().subject().canonical_bytes();
    drop(subject);
    b.release_storage(storage.retained_storage())?;
    if !matches {
        return Err(Error::Mismatch("original compiler-execution Subject"));
    }
    Ok(())
}
