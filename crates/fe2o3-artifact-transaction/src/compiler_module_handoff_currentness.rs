//! One pinned currentness implementation for strict semantic handoff schemas.
use super::*;

type Result<T> = std::result::Result<T, HandoffEngineError>;
const STREAM_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Binding {
    pub(super) sha256: [u8; 32],
    pub(super) byte_len: u64,
}

pub(super) trait Schema: HandoffSchema<Binding = Binding> {
    type Receipt: Copy + Eq;
    const TRANSACTION_DOMAIN: &'static [u8];
    fn receipt_fields(receipt: Self::Receipt) -> PublishedHandoff<Self>;
    fn receipt(fields: PublishedHandoff<Self>, payload: &Self::Payload) -> Self::Receipt;
    fn payload_binding(payload: &Self::Payload) -> Binding;

    fn transaction_hasher(
        producer: [u8; 32],
        slot: [u8; 32],
        attempt: BuildAttempt,
        binding: Binding,
        length: usize,
    ) -> Sha256 {
        let mut digest = Sha256::new();
        digest.update(Self::TRANSACTION_DOMAIN);
        digest.update(binding.sha256);
        digest.update(binding.byte_len.to_le_bytes());
        digest.update(slot);
        digest.update(producer);
        digest.update(attempt.generation().to_le_bytes());
        digest.update(attempt.session().as_bytes());
        digest.update(attempt.invocation().as_bytes());
        digest.update((length as u64).to_le_bytes());
        digest
    }
}

pub(super) struct Current<S: Schema> {
    pub(super) output: PinnedOutput,
    pub(super) producer: ProducerIdentity,
    pub(super) producer_identity: [u8; 32],
    pub(super) parent: PinnedDirectory,
    pub(super) slot_directory: PinnedDirectory,
    pub(super) ready_file: PinnedFile,
    pub(super) payload_file: PinnedFile,
    pub(super) receipt: S::Receipt,
    pub(super) slot_identity: [u8; 32],
    pub(super) committed_generation: u64,
}

pub(super) struct PinnedFile {
    pub(super) file: fs::File,
    pub(super) identity: FileIdentity,
}

pub(super) fn shape<S: Schema>(slot: &PinnedDirectory) -> Result<()> {
    let entries = slot_entries(slot)?;
    if entries.iter().any(|entry| entry == CONSUMED_ENTRY) {
        return Err(CompilerModuleHandoffErrorV1::AlreadyConsumed.into());
    }
    let ready = entries.iter().any(|entry| entry == READY_ENTRY);
    let payload = entries.iter().any(|entry| entry == PAYLOAD_ENTRY);
    let sidecar =
        S::COMMITTED_SIDECAR_ENTRY.is_some_and(|name| entries.iter().any(|entry| entry == name));
    if ready && payload && entries.len() == 2 + usize::from(sidecar) {
        return Ok(());
    }
    if !ready {
        return Err(CompilerModuleHandoffErrorV1::NotPublished.into());
    }
    Err(invalid_slot(
        &slot.path,
        "current slot must contain only ready, payload, and its schema's optional sidecar",
    )
    .into())
}

pub(super) fn pin(slot: &PinnedDirectory, entry: &str, length: usize) -> Result<PinnedFile> {
    let fd = openat(
        &slot.fd,
        entry,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        invalid_slot(
            &slot.path.join(entry),
            std::io::Error::from(error).to_string(),
        )
    })?;
    let file = fs::File::from(fd);
    let opened = fstat(&file).map_err(std::io::Error::from)?;
    let named = statat(&slot.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    if !same_private_file(&opened, &named, length) {
        return Err(invalid_slot(
            &slot.path.join(entry),
            "file does not match its pinned private descriptor",
        )
        .into());
    }
    Ok(PinnedFile {
        file,
        identity: FileIdentity::from_stat(&opened),
    })
}

pub(super) fn validate_file(
    slot: &PinnedDirectory,
    entry: &str,
    pinned: &PinnedFile,
) -> Result<()> {
    let opened = fstat(&pinned.file).map_err(std::io::Error::from)?;
    let named = statat(&slot.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
        invalid_slot(
            &slot.path.join(entry),
            std::io::Error::from(error).to_string(),
        )
    })?;
    if !pinned.identity.matches(&opened) || !pinned.identity.matches(&named) {
        return Err(invalid_slot(
            &slot.path.join(entry),
            "file no longer matches the publication lease",
        )
        .into());
    }
    Ok(())
}

fn validate_renamed(slot: &PinnedDirectory, pinned: &PinnedFile) -> Result<()> {
    let opened = fstat(&pinned.file).map_err(std::io::Error::from)?;
    let named = statat(&slot.fd, CONSUMED_ENTRY, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
        invalid_slot(
            &slot.path.join(CONSUMED_ENTRY),
            std::io::Error::from(error).to_string(),
        )
    })?;
    let length = usize::try_from(pinned.identity.length)
        .map_err(|_| invalid_slot(&slot.path, "pinned record length is invalid"))?;
    if pinned.identity.device != opened.st_dev
        || pinned.identity.inode != opened.st_ino
        || !same_private_file(&opened, &named, length)
    {
        return Err(invalid_slot(
            &slot.path.join(CONSUMED_ENTRY),
            "consumed record is not the exact renamed ready record",
        )
        .into());
    }
    Ok(())
}

fn read_file(
    slot: &PinnedDirectory,
    entry: &str,
    pinned: &PinnedFile,
    length: usize,
    maximum: usize,
    resources: &mut Resources<'_, '_>,
) -> Result<Vec<u8>> {
    if length == 0 || length > maximum {
        return Err(CompilerModuleHandoffErrorV1::InvalidHandoffSize {
            actual: length,
            maximum,
        }
        .into());
    }
    resources.work(
        length
            .checked_add(1)
            .ok_or(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
    )?;
    validate_file(slot, entry, pinned)?;
    let mut bytes = resources.buffer(length)?;
    bytes.resize(length, 0);
    pinned.file.read_exact_at(&mut bytes, 0)?;
    let mut trailing = [0_u8; 1];
    if pinned.file.read_at(&mut trailing, length as u64)? != 0 {
        return Err(invalid_slot(
            &slot.path.join(entry),
            "pinned file grew beyond its committed length",
        )
        .into());
    }
    validate_file(slot, entry, pinned)?;
    Ok(bytes)
}

fn read_record<S: Schema>(
    slot: &PinnedDirectory,
    file: &PinnedFile,
    resources: &mut Resources<'_, '_>,
) -> Result<HandoffRecord<S>> {
    resources.scoped(|resources| {
        resources.require::<S>()?;
        let bytes = read_file(
            slot,
            READY_ENTRY,
            file,
            S::RECORD_BYTES,
            S::RECORD_BYTES,
            resources,
        )?;
        decode_record::<S>(slot, &bytes, resources)
    })
}

fn decode_record<S: Schema>(
    slot: &PinnedDirectory,
    bytes: &[u8],
    resources: &mut Resources<'_, '_>,
) -> Result<HandoffRecord<S>> {
    resources.scoped(|resources| {
        resources.require::<S>()?;
        resources
            .reserve(std::mem::size_of::<Sha256>() + std::mem::size_of::<HandoffRecord<S>>())?;
        resources.work(S::RECORD_BYTES * 4)?;
        HandoffRecord::<S>::decode(bytes)
            .map_err(|reason| invalid_slot(&slot.path.join(READY_ENTRY), reason).into())
    })
}

pub(super) fn record<S: Schema>(
    binding: &Current<S>,
    resources: &mut Resources<'_, '_>,
) -> Result<HandoffRecord<S>> {
    resources.require::<S>()?;
    let receipt = S::receipt_fields(binding.receipt);
    binding.output.verify_path_identity()?;
    authorize_for_custody(
        &binding.output,
        &binding.producer,
        receipt.attempt,
        S::ATTEMPT_CUSTODY,
    )?;
    if binding.committed_generation != receipt.attempt.generation() {
        return Err(
            attempt_error("lease generation no longer matches its committed attempt").into(),
        );
    }
    binding.parent.verify()?;
    binding.slot_directory.verify()?;
    shape::<S>(&binding.slot_directory)?;
    validate_file(&binding.slot_directory, READY_ENTRY, &binding.ready_file)?;
    validate_file(
        &binding.slot_directory,
        PAYLOAD_ENTRY,
        &binding.payload_file,
    )?;
    let record = read_record::<S>(&binding.slot_directory, &binding.ready_file, resources)?;
    if record.attempt != receipt.attempt
        || record.attempt.generation() != binding.committed_generation
        || record.producer != binding.producer_identity
        || record.slot != binding.slot_identity
    {
        return Err(invalid_slot(
            &binding.slot_directory.path.join(READY_ENTRY),
            "record no longer matches the lease attempt, producer, slot, or generation",
        )
        .into());
    }
    if record.binding != receipt.binding {
        return Err(HandoffEngineError::WrongBinding);
    }
    if record.identity != receipt.identity || record.length != receipt.length {
        return Err(CompilerModuleHandoffErrorV1::DigestMismatch.into());
    }
    if record.file != binding.payload_file.identity {
        return Err(invalid_slot(
            &binding.slot_directory.path.join(PAYLOAD_ENTRY),
            "payload metadata no longer matches the committed record",
        )
        .into());
    }
    Ok(record)
}

pub(super) fn metadata<S: Schema>(
    binding: &Current<S>,
    resources: &mut Resources<'_, '_>,
) -> Result<()> {
    record(binding, resources)?;
    binding.output.verify_path_identity()?;
    binding.parent.verify()?;
    binding.slot_directory.verify()?;
    Ok(())
}

pub(super) fn load<S: Schema>(
    binding: &Current<S>,
    resources: &mut Resources<'_, '_>,
) -> Result<S::Payload> {
    resources.require::<S>()?;
    resources.reserve(std::mem::size_of::<Sha256>())?;
    let record = record(binding, resources)?;
    validate_decode_working_set::<S>(record.length, S::MAX_DECODE_WORKING_SET_BYTES)?;
    let bytes = read_file(
        &binding.slot_directory,
        PAYLOAD_ENTRY,
        &binding.payload_file,
        record.length,
        S::MAX_HANDOFF_BYTES,
        resources,
    )?;
    resources.work(
        record
            .length
            .checked_add(256)
            .ok_or(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
    )?;
    if S::derive_identity(
        record.producer,
        record.slot,
        record.attempt,
        record.binding,
        &bytes,
    ) != record.identity
    {
        return Err(CompilerModuleHandoffErrorV1::DigestMismatch.into());
    }
    let handoff = S::decode_payload(record.binding, bytes, resources)?;
    if S::payload_binding(&handoff) != S::receipt_fields(binding.receipt).binding {
        return Err(HandoffEngineError::PayloadBindingMismatch);
    }
    metadata(binding, resources)?;
    Ok(handoff)
}

pub(super) fn stream<S: Schema>(
    binding: &Current<S>,
    resources: &mut Resources<'_, '_>,
) -> Result<()> {
    resources.scoped(|resources| {
        let record = record(binding, resources)?;
        validate_file(
            &binding.slot_directory,
            PAYLOAD_ENTRY,
            &binding.payload_file,
        )?;
        resources.reserve(STREAM_BYTES + std::mem::size_of::<Sha256>())?;
        resources.work(
            record
                .length
                .checked_mul(2)
                .and_then(|n| n.checked_add(257))
                .ok_or(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
        )?;
        let mut digest = S::transaction_hasher(
            record.producer,
            record.slot,
            record.attempt,
            record.binding,
            record.length,
        );
        let mut offset = 0_u64;
        let mut remaining = record.length;
        let mut buffer = [0_u8; STREAM_BYTES];
        while remaining != 0 {
            let requested = remaining.min(buffer.len());
            let read = binding
                .payload_file
                .file
                .read_at(&mut buffer[..requested], offset)?;
            if read == 0 {
                return Err(invalid_slot(
                    &binding.slot_directory.path.join(PAYLOAD_ENTRY),
                    "pinned payload ended before its committed length",
                )
                .into());
            }
            digest.update(&buffer[..read]);
            remaining -= read;
            offset += read as u64;
        }
        let mut trailing = [0_u8; 1];
        if binding.payload_file.file.read_at(&mut trailing, offset)? != 0 {
            return Err(invalid_slot(
                &binding.slot_directory.path.join(PAYLOAD_ENTRY),
                "pinned payload grew beyond its committed length",
            )
            .into());
        }
        validate_file(
            &binding.slot_directory,
            PAYLOAD_ENTRY,
            &binding.payload_file,
        )?;
        let identity: [u8; 32] = digest.finalize().into();
        if identity != record.identity || identity != S::receipt_fields(binding.receipt).identity {
            return Err(CompilerModuleHandoffErrorV1::DigestMismatch.into());
        }
        metadata(binding, resources)
    })
}

pub(super) fn mint<S: Schema>(
    output_dir: &Path,
    producer: &ProducerIdentity,
    receipt: S::Receipt,
    resources: &mut Resources<'_, '_>,
) -> Result<Arc<Current<S>>> {
    resources.require::<S>()?;
    let fields = S::receipt_fields(receipt);
    if !S::binding_matches_length(fields.binding, fields.length) {
        return Err(HandoffEngineError::PayloadBindingMismatch);
    }
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.try_lock()?.ok_or(HandoffEngineError::Busy)?;
    output.verify_path_identity()?;
    authorize_for_custody(&output, producer, fields.attempt, S::ATTEMPT_CUSTODY)?;
    let producer_identity = producer_identity_for::<S>(producer);
    let slot_identity = slot_identity_for::<S>(producer_identity, fields.attempt, fields.slot);
    let parent = open_private_directory(
        &output.fd,
        &output.display_path,
        format!("{}{}", S::PARENT_PREFIX, hex(&producer_identity)),
    )?
    .ok_or(CompilerModuleHandoffErrorV1::NotPublished)?;
    cleanup_stale_slots::<S>(&parent, producer_identity, fields.attempt)?;
    let slot_directory = open_private_directory(
        &parent.fd,
        &parent.path,
        format!("{}{}", S::SLOT_PREFIX, hex(&slot_identity)),
    )?
    .ok_or(CompilerModuleHandoffErrorV1::NotPublished)?;
    recover_slot::<S>(&slot_directory)?;
    shape::<S>(&slot_directory)?;
    let ready_file = pin(&slot_directory, READY_ENTRY, S::RECORD_BYTES)?;
    let payload_file = pin(&slot_directory, PAYLOAD_ENTRY, fields.length)?;
    let binding = Arc::new(Current::<S> {
        output,
        producer: producer.clone(),
        producer_identity,
        parent,
        slot_directory,
        ready_file,
        payload_file,
        receipt,
        slot_identity,
        committed_generation: fields.attempt.generation(),
    });
    stream(&binding, resources)?;
    Ok(binding)
}

pub(super) fn recover<S: Schema>(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    slot: S::Slot,
    resources: &mut Resources<'_, '_>,
) -> Result<S::Receipt> {
    resources.scoped(|resources| {
        resources.require::<S>()?;
        resources.reserve(std::mem::size_of::<Sha256>())?;
        let output = PinnedOutput::open_existing(output_dir)?;
        let _lock = output.lock()?;
        output.verify_path_identity()?;
        authorize_for_custody(&output, producer, attempt, S::ATTEMPT_CUSTODY)?;
        let producer_identity = producer_identity_for::<S>(producer);
        let slot_identity = slot_identity_for::<S>(producer_identity, attempt, slot);
        let parent = open_private_directory(
            &output.fd,
            &output.display_path,
            format!("{}{}", S::PARENT_PREFIX, hex(&producer_identity)),
        )?
        .ok_or(CompilerModuleHandoffErrorV1::NotPublished)?;
        cleanup_stale_slots::<S>(&parent, producer_identity, attempt)?;
        let directory = open_private_directory(
            &parent.fd,
            &parent.path,
            format!("{}{}", S::SLOT_PREFIX, hex(&slot_identity)),
        )?
        .ok_or(CompilerModuleHandoffErrorV1::NotPublished)?;
        recover_slot::<S>(&directory)?;
        shape::<S>(&directory)?;
        let ready = pin(&directory, READY_ENTRY, S::RECORD_BYTES)?;
        let record_bytes = read_file(
            &directory,
            READY_ENTRY,
            &ready,
            S::RECORD_BYTES,
            S::RECORD_BYTES,
            resources,
        )?;
        let record = decode_record::<S>(&directory, &record_bytes, resources)?;
        if record.producer != producer_identity
            || record.attempt != attempt
            || record.slot != slot_identity
        {
            return Err(invalid_slot(
                &directory.path.join(READY_ENTRY),
                "record binding does not match the requested producer, attempt, and slot",
            )
            .into());
        }
        let payload = pin(&directory, PAYLOAD_ENTRY, record.length)?;
        if record.file != payload.identity {
            return Err(invalid_slot(
                &directory.path.join(PAYLOAD_ENTRY),
                "payload metadata does not match the durable ready record",
            )
            .into());
        }
        validate_decode_working_set::<S>(record.length, S::MAX_DECODE_WORKING_SET_BYTES)?;
        let bytes = read_file(
            &directory,
            PAYLOAD_ENTRY,
            &payload,
            record.length,
            S::MAX_HANDOFF_BYTES,
            resources,
        )?;
        resources.work(
            record
                .length
                .checked_add(256)
                .ok_or(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
        )?;
        let identity = S::derive_identity(
            record.producer,
            record.slot,
            record.attempt,
            record.binding,
            &bytes,
        );
        if identity != record.identity {
            return Err(CompilerModuleHandoffErrorV1::DigestMismatch.into());
        }
        let handoff = S::decode_payload(record.binding, bytes, resources)?;
        if S::payload_binding(&handoff) != record.binding {
            return Err(HandoffEngineError::PayloadBindingMismatch);
        }
        authorize_for_custody(&output, producer, attempt, S::ATTEMPT_CUSTODY)?;
        output.verify_path_identity()?;
        parent.verify()?;
        directory.verify()?;
        shape::<S>(&directory)?;
        let final_record_bytes = read_file(
            &directory,
            READY_ENTRY,
            &ready,
            S::RECORD_BYTES,
            S::RECORD_BYTES,
            resources,
        )?;
        resources.work(S::RECORD_BYTES)?;
        if final_record_bytes != record_bytes {
            return Err(invalid_slot(
                &directory.path.join(READY_ENTRY),
                "ready record changed while its exact payload was validated",
            )
            .into());
        }
        validate_file(&directory, PAYLOAD_ENTRY, &payload)?;
        Ok(S::receipt(
            PublishedHandoff {
                attempt,
                slot,
                binding: record.binding,
                identity,
                length: record.length,
            },
            &handoff,
        ))
    })
}

/// The caller retains the exact token's cooperative lock through this function.
pub(super) fn consume<S: Schema>(
    binding: &Current<S>,
    resources: &mut Resources<'_, '_>,
    hooks: &mut impl HandoffHooks,
) -> Result<()> {
    metadata(binding, resources)?;
    let directory = &binding.slot_directory;
    directory.verify()?;
    hooks.hit(FaultPoint::PayloadValidated)?;
    renameat_with(
        &directory.fd,
        READY_ENTRY,
        &directory.fd,
        CONSUMED_ENTRY,
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    hooks.hit(FaultPoint::ConsumedRenamed)?;
    fsync(&directory.fd).map_err(std::io::Error::from)?;
    hooks.hit(FaultPoint::ConsumedSynced)?;
    directory.verify()?;
    validate_renamed(directory, &binding.ready_file)?;
    validate_file(directory, PAYLOAD_ENTRY, &binding.payload_file)?;
    cleanup_consumed_payload(directory);
    Ok(())
}
