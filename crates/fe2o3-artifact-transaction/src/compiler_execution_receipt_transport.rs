//! Shared sidecar custody. Version adapters own subject and wire semantics.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;

#[path = "compiler_execution_receipt_envelope.rs"]
pub(super) mod envelope;
#[path = "compiler_execution_receipt_native.rs"]
pub(super) mod native;

pub(super) struct Coordinates<S: currentness::Schema> {
    pub attempt: BuildAttempt,
    pub slot: S::Slot,
    pub outer: currentness::Binding,
    pub transaction: [u8; 32],
}

pub(super) trait Subject {
    type Error;
    type Schema: currentness::Schema;
    type Postcheck;
    const ENTRY: &'static str;
    const MAX_BYTES: usize;
    fn coordinates(&self) -> Coordinates<Self::Schema>;
    fn validate_payload(
        &self,
        record: &HandoffRecord<Self::Schema>,
        bytes: Vec<u8>,
        resources: &mut Resources<'_, '_>,
    ) -> Result<(), Self::Error>;
    fn prepare_postcheck(
        &self,
        output: &PinnedOutput,
        producer: &ProducerIdentity,
        slot: &PinnedDirectory,
        resources: &mut Resources<'_, '_>,
    ) -> Result<Self::Postcheck, Self::Error>;
    /// No native allocation or budget refusal may first occur after commit.
    fn postcheck(
        &self,
        output: &PinnedOutput,
        producer: &ProducerIdentity,
        slot: &PinnedDirectory,
        prepared: Self::Postcheck,
    ) -> Result<(), Self::Error>;
}

#[derive(Debug)]
pub(super) enum Failure<E = crate::CompilerExecutionSubjectErrorV2> {
    Handoff(HandoffEngineError),
    Subject(E),
    InvalidSize { actual: usize, maximum: usize },
    NotPublished,
    Conflict,
    Mismatch,
}
pub(super) type Result<T, E = crate::CompilerExecutionSubjectErrorV2> =
    std::result::Result<T, Failure<E>>;

// The legacy specialization remains layout-identical: version adapters bill
// these error/result headers in their existing fixed scratch allowance.
const _: () = {
    #[allow(dead_code)]
    enum Before {
        Handoff(HandoffEngineError),
        Subject(crate::CompilerExecutionSubjectErrorV2),
        InvalidSize { actual: usize, maximum: usize },
        NotPublished,
        Conflict,
        Mismatch,
    }
    use std::mem::{align_of, size_of};
    assert!(size_of::<Failure>() == size_of::<Before>());
    assert!(align_of::<Failure>() == align_of::<Before>());
    assert!(size_of::<Result<()>>() == size_of::<std::result::Result<(), Before>>());
    assert!(
        size_of::<std::thread::Result<Result<Vec<u8>>>>()
            == size_of::<std::thread::Result<std::result::Result<Vec<u8>, Before>>>()
    );
};

impl<E> From<HandoffEngineError> for Failure<E> {
    fn from(e: HandoffEngineError) -> Self {
        Self::Handoff(e)
    }
}
impl<E> From<CompilerModuleHandoffErrorV1> for Failure<E> {
    fn from(e: CompilerModuleHandoffErrorV1) -> Self {
        Self::Handoff(e.into())
    }
}
impl<E> From<std::io::Error> for Failure<E> {
    fn from(e: std::io::Error) -> Self {
        Self::Handoff(e.into())
    }
}
impl<E> From<EmitError> for Failure<E> {
    fn from(e: EmitError) -> Self {
        Self::Handoff(e.into())
    }
}
impl<E> From<Resource> for Failure<E> {
    fn from(e: Resource) -> Self {
        Self::Handoff(e.into())
    }
}

pub(super) fn size<E>(length: usize, maximum: usize) -> Result<(), E> {
    if length == 0 || length > maximum {
        return Err(Failure::<E>::InvalidSize {
            actual: length,
            maximum,
        });
    }
    Ok(())
}

pub(super) fn authorize_recovery<E>(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    allow_consumed: bool,
) -> Result<(), E> {
    if !allow_consumed {
        return authorize(output, producer, attempt).map_err(Into::into);
    }
    if attempt.session() == BuildSession::DIRECT {
        return Err(attempt_error(
            "direct compiler attempts cannot own a compiler-execution receipt sidecar",
        )
        .into());
    }
    let attempts = read_attempt_registry(output)?;
    let record = attempts
        .record_exact(&producer.stable_source, attempt)
        .map_err(|e| attempt_error(e.to_string()))?;
    if record.crate_name != producer.crate_name {
        return Err(
            attempt_error("build attempt crate name does not match the receipt producer").into(),
        );
    }
    if !matches!(
        (record.phase, record.backend_receipt),
        (AttemptPhase::Building, None)
            | (
                AttemptPhase::BackendClaimed | AttemptPhase::Completed,
                Some(_)
            )
    ) {
        return Err(
            attempt_error("build attempt is not in a receipt-sidecar recovery phase").into(),
        );
    }
    Ok(())
}

fn open<T: Subject>(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    subject: &T,
    allow_consumed: bool,
    resources: &mut Resources<'_, '_>,
) -> Result<(PinnedDirectory, PinnedDirectory), T::Error> {
    output.verify_path_identity()?;
    let c = subject.coordinates();
    authorize_recovery::<T::Error>(output, producer, c.attempt, allow_consumed)?;
    let producer_id = producer_identity_for::<T::Schema>(producer);
    let slot_id = slot_identity_for::<T::Schema>(producer_id, c.attempt, c.slot);
    let parent = open_private_directory(
        &output.fd,
        &output.display_path,
        format!("{}{}", T::Schema::PARENT_PREFIX, hex(&producer_id)),
    )?
    .ok_or(CompilerModuleHandoffErrorV1::NotPublished)?;
    cleanup_stale_slots::<T::Schema>(&parent, producer_id, c.attempt)?;
    let slot = open_private_directory(
        &parent.fd,
        &parent.path,
        format!("{}{}", T::Schema::SLOT_PREFIX, hex(&slot_id)),
    )?
    .ok_or(CompilerModuleHandoffErrorV1::NotPublished)?;
    recover_slot::<T::Schema>(&slot, resources)?;
    validate(
        output,
        producer,
        subject,
        &slot,
        allow_consumed,
        true,
        resources,
    )?;
    parent.verify()?;
    Ok((parent, slot))
}

pub(super) fn validate<T: Subject>(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    subject: &T,
    slot: &PinnedDirectory,
    allow_consumed: bool,
    payload: bool,
    resources: &mut Resources<'_, '_>,
) -> Result<(), T::Error> {
    resources.scoped(|r| {
        Ok(validate_inner(
            output,
            producer,
            subject,
            slot,
            allow_consumed,
            payload,
            r,
        ))
    })?
}

fn validate_inner<T: Subject>(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    subject: &T,
    slot: &PinnedDirectory,
    allow_consumed: bool,
    payload: bool,
    resources: &mut Resources<'_, '_>,
) -> Result<(), T::Error> {
    output.verify_path_identity()?;
    let c = subject.coordinates();
    authorize_recovery::<T::Error>(output, producer, c.attempt, allow_consumed)?;
    slot.verify()?;
    let entries = slot_entries(slot)?;
    let record_entry = match (
        entries.iter().any(|e| e == READY_ENTRY),
        entries.iter().any(|e| e == CONSUMED_ENTRY),
    ) {
        (true, false) => READY_ENTRY,
        (false, true) if allow_consumed => CONSUMED_ENTRY,
        (false, true) => return Err(CompilerModuleHandoffErrorV1::AlreadyConsumed.into()),
        (false, false) => return Err(CompilerModuleHandoffErrorV1::NotPublished.into()),
        (true, true) => return Err(Failure::<T::Error>::Mismatch),
    };
    let bytes = if T::Schema::METERED {
        let pinned = currentness::pin(slot, record_entry, T::Schema::RECORD_BYTES)?;
        currentness::read_file(
            slot,
            record_entry,
            &pinned,
            T::Schema::RECORD_BYTES,
            T::Schema::RECORD_BYTES,
            resources,
        )?
    } else {
        read_private_file(slot, record_entry, T::Schema::RECORD_BYTES)?
            .ok_or(Failure::<T::Error>::Mismatch)?
    };
    let record = matching_record::<T::Schema, T::Error>(&bytes, producer, &c, resources)?;
    if T::Schema::METERED && !payload && record_entry == READY_ENTRY {
        let pinned = pin_payload::<T::Schema, T::Error>(slot, &record)?;
        if allow_consumed {
            currentness::prepay_payload_stream(&record, resources)?;
            currentness::stream_payload(slot, &pinned, &record)?;
        }
    }
    if payload && record_entry == READY_ENTRY {
        let bytes = read_payload::<T::Schema>(
            slot,
            &record,
            T::Schema::MAX_DECODE_WORKING_SET_BYTES,
            resources,
        )?;
        subject.validate_payload(&record, bytes, resources)?;
    }
    output.verify_path_identity()?;
    authorize_recovery::<T::Error>(output, producer, c.attempt, allow_consumed)?;
    slot.verify()?;
    Ok(())
}

fn matching_record<S: currentness::Schema, E>(
    bytes: &[u8],
    producer: &ProducerIdentity,
    c: &Coordinates<S>,
    resources: &mut Resources<'_, '_>,
) -> Result<HandoffRecord<S>, E> {
    resources.reserve(std::mem::size_of::<HandoffRecord<S>>() + std::mem::size_of::<Sha256>())?;
    resources.work(S::RECORD_BYTES * 4)?;
    let record = HandoffRecord::<S>::decode(bytes).map_err(|_| Failure::<E>::Mismatch)?;
    let producer_id = producer_identity_for::<S>(producer);
    if record.producer != producer_id
        || record.slot != slot_identity_for::<S>(producer_id, c.attempt, c.slot)
        || record.attempt != c.attempt
        || record.binding != c.outer
        || record.identity != c.transaction
        || record.length as u64 != c.outer.byte_len
    {
        return Err(Failure::<E>::Mismatch);
    }
    Ok(record)
}

pub(super) fn read<T: Subject>(
    slot: &PinnedDirectory,
    resources: &mut Resources<'_, '_>,
) -> Result<Option<Vec<u8>>, T::Error> {
    let stat = match statat(&slot.fd, T::ENTRY, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(e) if e == rustix::io::Errno::NOENT => return Ok(None),
        Err(e) => return Err(std::io::Error::from(e).into()),
    };
    if !is_private_file(&stat) {
        return Err(Failure::<T::Error>::Mismatch);
    }
    let n = usize::try_from(stat.st_size).map_err(|_| Failure::<T::Error>::InvalidSize {
        actual: usize::MAX,
        maximum: T::MAX_BYTES,
    })?;
    size::<T::Error>(n, T::MAX_BYTES)?;
    if T::Schema::METERED {
        let pinned = currentness::pin(slot, T::ENTRY, n)?;
        Ok(Some(currentness::read_file(
            slot,
            T::ENTRY,
            &pinned,
            n,
            T::MAX_BYTES,
            resources,
        )?))
    } else {
        Ok(Some(
            read_private_file(slot, T::ENTRY, n)?.ok_or(Failure::<T::Error>::NotPublished)?,
        ))
    }
}

pub(super) fn publish<T: Subject>(
    output_dir: &Path,
    producer: &ProducerIdentity,
    subject: &T,
    wire: &[u8],
    resources: &mut Resources<'_, '_>,
    hooks: &mut impl HandoffHooks,
) -> Result<(), T::Error> {
    size::<T::Error>(wire.len(), T::MAX_BYTES)?;
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    let (parent, slot) = open(&output, producer, subject, false, resources)?;
    if let Some(existing) = read::<T>(&slot, resources)? {
        resources.work(2 * wire.len().max(existing.len()) + 64)?;
        if existing != wire {
            return Err(Failure::<T::Error>::Conflict);
        }
        let prepared = subject.prepare_postcheck(&output, producer, &slot, resources)?;
        fsync(&slot.fd).map_err(std::io::Error::from)?;
        subject.postcheck(&output, producer, &slot, prepared)?;
        parent.verify()?;
        return Ok(());
    }
    resources.work(wire.len())?;
    let (name, mut temporary) = create_temp(&slot, T::ENTRY)?;
    hooks.hit(FaultPoint::PayloadCreated)?;
    temporary.write_all(wire)?;
    hooks.hit(FaultPoint::PayloadWritten)?;
    temporary.sync_all()?;
    hooks.hit(FaultPoint::PayloadSynced)?;
    let stat = fstat(&temporary).map_err(std::io::Error::from)?;
    if !is_private_file(&stat) || usize::try_from(stat.st_size).ok() != Some(wire.len()) {
        return Err(Failure::<T::Error>::Mismatch);
    }
    validate(&output, producer, subject, &slot, false, false, resources)?;
    let prepared = subject.prepare_postcheck(&output, producer, &slot, resources)?;
    let readback = if T::Schema::METERED {
        resources.work(3 * wire.len() + 65)?;
        let mut bytes = resources.buffer(wire.len())?;
        bytes.resize(wire.len(), 0);
        Some(bytes)
    } else {
        None
    };
    parent.verify()?;
    // All native read-back/validation buffers and work are now prepaid.
    match renameat_with(&slot.fd, &name, &slot.fd, T::ENTRY, RenameFlags::NOREPLACE) {
        Ok(()) => {}
        Err(e) if e == rustix::io::Errno::EXIST => return Err(Failure::<T::Error>::Conflict),
        Err(e) => return Err(std::io::Error::from(e).into()),
    }
    hooks.hit(FaultPoint::RecordRenamed)?;
    fsync(&slot.fd).map_err(std::io::Error::from)?;
    hooks.hit(FaultPoint::PublishedSynced)?;
    slot.verify()?;
    let committed = match readback {
        Some(mut bytes) => {
            let pinned = currentness::pin(&slot, T::ENTRY, bytes.len())?;
            currentness::read_file_into(&slot, T::ENTRY, &pinned, &mut bytes)?;
            bytes
        }
        None => read::<T>(&slot, resources)?.ok_or(Failure::<T::Error>::NotPublished)?,
    };
    if committed != wire {
        return Err(Failure::<T::Error>::Conflict);
    }
    subject.postcheck(&output, producer, &slot, prepared)?;
    parent.verify()?;
    Ok(())
}

pub(super) fn recover<T: Subject>(
    output_dir: &Path,
    producer: &ProducerIdentity,
    subject: &T,
    resources: &mut Resources<'_, '_>,
) -> Result<Vec<u8>, T::Error> {
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    let (parent, slot) = open(&output, producer, subject, true, resources)?;
    let bytes = read::<T>(&slot, resources)?.ok_or(Failure::<T::Error>::NotPublished)?;
    validate(&output, producer, subject, &slot, true, false, resources)?;
    parent.verify()?;
    Ok(bytes)
}

/// Immutable ready-record custody plus separately prepaid read-back scratch.
pub(super) struct PreparedPostcheck<S: currentness::Schema> {
    file: currentness::PinnedFile,
    payload: currentness::PinnedFile,
    original: Vec<u8>,
    readback: Vec<u8>,
    record: HandoffRecord<S>,
}
impl<S: currentness::Schema> PreparedPostcheck<S> {
    pub(super) fn new<E>(
        slot: &PinnedDirectory,
        producer: &ProducerIdentity,
        c: &Coordinates<S>,
        resources: &mut Resources<'_, '_>,
    ) -> Result<Self, E> {
        resources.reserve(std::mem::size_of::<Self>())?;
        let file = currentness::pin(slot, READY_ENTRY, S::RECORD_BYTES)?;
        let original = currentness::read_file(
            slot,
            READY_ENTRY,
            &file,
            S::RECORD_BYTES,
            S::RECORD_BYTES,
            resources,
        )?;
        let record = matching_record::<S, E>(&original, producer, c, resources)?;
        let payload = pin_payload::<S, E>(slot, &record)?;
        resources.scoped(|r| {
            currentness::prepay_payload_stream(&record, r)?;
            currentness::stream_payload(slot, &payload, &record)
        })?;
        currentness::prepay_payload_stream(&record, resources)?;
        resources.work(3 * S::RECORD_BYTES + 65)?;
        let mut readback = resources.buffer(S::RECORD_BYTES)?;
        readback.resize(S::RECORD_BYTES, 0);
        Ok(Self {
            file,
            payload,
            original,
            readback,
            record,
        })
    }
    pub(super) fn finish<E>(
        mut self,
        output: &PinnedOutput,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        slot: &PinnedDirectory,
    ) -> Result<(), E> {
        output.verify_path_identity()?;
        authorize_recovery::<E>(output, producer, attempt, false)?;
        slot.verify()?;
        currentness::shape::<S>(slot)?;
        currentness::validate_file(slot, PAYLOAD_ENTRY, &self.payload)?;
        currentness::read_file_into(slot, READY_ENTRY, &self.file, &mut self.readback)?;
        if self.original != self.readback {
            return Err(Failure::<E>::Mismatch);
        }
        currentness::stream_payload(slot, &self.payload, &self.record)?;
        output.verify_path_identity()?;
        authorize_recovery::<E>(output, producer, attempt, false)?;
        currentness::validate_file(slot, PAYLOAD_ENTRY, &self.payload)?;
        slot.verify()?;
        Ok(())
    }
}

fn pin_payload<S: HandoffSchema, E>(
    slot: &PinnedDirectory,
    record: &HandoffRecord<S>,
) -> Result<currentness::PinnedFile, E> {
    let pinned = currentness::pin(slot, PAYLOAD_ENTRY, record.length)?;
    if pinned.identity != record.file {
        return Err(invalid_slot(
            &slot.path.join(PAYLOAD_ENTRY),
            "payload identity metadata mismatch",
        )
        .into());
    }
    Ok(pinned)
}
