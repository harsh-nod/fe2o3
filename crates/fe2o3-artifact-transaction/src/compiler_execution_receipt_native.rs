//! Closed native sidecar flow, shared by V2/V3 without any proof-owner callback.
use super::{envelope, *};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;

pub(in crate::compiler_module_handoff) trait NativeSubject:
    Subject + Sized
{
    const WIRE: envelope::Schema;
    fn canonical_bytes(&self) -> &[u8; envelope::SUBJECT_BYTES];
    fn reconstruct(
        &self,
        payload: &<Self::Schema as HandoffSchema>::Payload,
        budget: &mut Budget<'_>,
    ) -> std::result::Result<(Self, usize), Self::Error>;
    fn from_receipt(
        receipt: <Self::Schema as currentness::Schema>::Receipt,
        payload: &<Self::Schema as HandoffSchema>::Payload,
        budget: &mut Budget<'_>,
    ) -> std::result::Result<(Self, usize), Self::Error>;
}

pub(in crate::compiler_module_handoff) struct Recovered {
    pub wire: Vec<u8>,
    pub digest: [u8; 32],
    pub length: usize,
    pub storage: usize,
}

pub(in crate::compiler_module_handoff) fn validate_payload<T: NativeSubject>(
    subject: &T,
    record: &HandoffRecord<T::Schema>,
    bytes: Vec<u8>,
    r: &mut Resources<'_, '_>,
) -> Result<(), T::Error> {
    let handoff = T::Schema::decode_payload(record.binding, bytes, r)?;
    let (actual, storage) = subject
        .reconstruct(&handoff, r.budget()?)
        .map_err(Failure::<T::Error>::Subject)?;
    r.reserve(storage)?;
    compare(subject, &actual, r)
}

fn compare<T: NativeSubject>(
    expected: &T,
    actual: &T,
    r: &mut Resources<'_, '_>,
) -> Result<(), T::Error> {
    r.work(2 * envelope::SUBJECT_BYTES + 64)?;
    if actual.canonical_bytes() != expected.canonical_bytes() {
        return Err(Failure::Mismatch);
    }
    Ok(())
}

pub(in crate::compiler_module_handoff) fn publish<T: NativeSubject, E>(
    output: &Path,
    producer: &ProducerIdentity,
    subject: &T,
    body: &[u8],
    r: &mut Resources<'_, '_>,
    hooks: &mut impl HandoffHooks,
) -> std::result::Result<([u8; 32], usize), E>
where
    E: envelope::Error + From<Failure<T::Error>>,
{
    let wire = T::WIRE.encode::<E>(subject.canonical_bytes(), body, r)?;
    let receipt = T::WIRE.inspect::<E>(&wire, subject.canonical_bytes(), r)?;
    super::publish(output, producer, subject, &wire, r, hooks)?;
    Ok(receipt)
}

pub(in crate::compiler_module_handoff) fn recovered<T: NativeSubject, E>(
    wire: Vec<u8>,
    subject: &T,
    headers: usize,
    r: &mut Resources<'_, '_>,
) -> std::result::Result<Recovered, E>
where
    E: envelope::Error,
{
    let (digest, length) = T::WIRE.inspect::<E>(&wire, subject.canonical_bytes(), r)?;
    r.reserve(headers)?;
    let storage = wire
        .capacity()
        .checked_add(headers)
        .ok_or(HandoffEngineError::Resource(Resource::Arithmetic))?;
    Ok(Recovered {
        wire,
        digest,
        length,
        storage,
    })
}

/// Caller has validated the exact raw lease/token. Reconstruct only inert cached
/// content, with metadata checks on both sides of sidecar inspection.
pub(in crate::compiler_module_handoff) fn recover_locked<T: NativeSubject, E>(
    binding: &currentness::Current<T::Schema>,
    payload: &<T::Schema as HandoffSchema>::Payload,
    expected: &T,
    headers: usize,
    r: &mut Resources<'_, '_>,
) -> std::result::Result<Recovered, E>
where
    E: envelope::Error + From<Failure<T::Error>> + From<T::Error>,
{
    recover_locked_using(binding, payload, expected, headers, r, T::from_receipt)
}

// Only fixed internal subject codecs select this adapter. No opaque proof or
// currentness callback enters the refundable native transport scope.
pub(in crate::compiler_module_handoff) fn recover_locked_using<T: NativeSubject, E>(
    binding: &currentness::Current<T::Schema>,
    payload: &<T::Schema as HandoffSchema>::Payload,
    expected: &T,
    headers: usize,
    r: &mut Resources<'_, '_>,
    reconstruct: impl FnOnce(
        <T::Schema as currentness::Schema>::Receipt,
        &<T::Schema as HandoffSchema>::Payload,
        &mut Budget<'_>,
    ) -> std::result::Result<(T, usize), T::Error>,
) -> std::result::Result<Recovered, E>
where
    E: envelope::Error + From<Failure<T::Error>> + From<T::Error>,
{
    currentness::metadata(binding, r)?;
    // Derive the actual occurrence from the token, never from the expectation.
    let (actual, storage) = reconstruct(binding.receipt, payload, r.budget()?)?;
    r.reserve(storage)?;
    compare(expected, &actual, r)?;
    let wire =
        super::read::<T>(&binding.slot_directory, r)?.ok_or(Failure::<T::Error>::NotPublished)?;
    let result = recovered::<T, E>(wire, expected, headers, r)?;
    currentness::metadata(binding, r)?;
    Ok(result)
}
