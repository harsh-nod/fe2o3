//! Complete inert scalar rounds; framing never reconstructs optimizer witnesses.
use crate::{
    CanonicalPolicy3ExecutionReceiptErrorV1, CheckedScalarFixedPointErrorV1,
    CheckedScalarFixedPointOwnerV1 as Core, InertCanonicalPolicy3ExecutionReceiptV1,
    MAX_CANONICAL_POLICY3_EXECUTION_RECEIPT_BYTES_V1, MAX_LOOP_UNROLL_HISTORY_STORAGE_V1,
    SCALAR_FIXED_POINT_EXECUTION_BYTES_V1, SCALAR_FIXED_POINT_MAX_ROUNDS_V1,
    encode_checked_canonical_policy3_execution_receipt_v1,
    private_cell_promotion_resources_v1 as resources,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayAdmissionErrorV12,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKirTransitionReceiptErrorV1,
    InertCanonicalKirTransitionReceiptV1 as Transition,
    MAX_CANONICAL_KIR_TRANSITION_RECEIPT_BYTES_V1, MAX_MODULE_BYTES_V1,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{error::Error as StdError, fmt, mem::size_of};

pub const SCALAR_FIXED_POINT_HISTORY_MAGIC_V1: [u8; 8] = *b"F2SPH1\0\0";
/// Reserved content domain; hashing belongs to the paid publication layer.
pub const SCALAR_FIXED_POINT_HISTORY_DOMAIN_V1: &[u8] = b"FE2O3/SCALAR-FIXED-POINT-HISTORY/V1\0";
pub(super) const HEADER: usize = 32 + SCALAR_FIXED_POINT_EXECUTION_BYTES_V1;
pub(super) const ROUND_HEADER: usize = 48;
pub(super) const FIELDS: usize = 5;
pub(super) const SCRATCH: usize = size_of::<[usize; FIELDS]>() + 2 * size_of::<usize>();
/// Checked sum of unchanged child bounds, not a promise of publication admission.
pub const MAX_SCALAR_FIXED_POINT_HISTORY_BYTES_V1: usize = {
    let per = MAX_MODULE_BYTES_V1
        .checked_mul(2)
        .expect("fixed graph sum")
        .checked_add(fe2o3_pliron::INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1)
        .expect("fixed claim")
        .checked_add(MAX_CANONICAL_KIR_TRANSITION_RECEIPT_BYTES_V1)
        .expect("fixed transition")
        .checked_add(MAX_CANONICAL_POLICY3_EXECUTION_RECEIPT_BYTES_V1)
        .expect("fixed policy3")
        .checked_add(ROUND_HEADER)
        .expect("fixed round");
    HEADER
        .checked_add(
            per.checked_mul(SCALAR_FIXED_POINT_MAX_ROUNDS_V1)
                .expect("fixed rounds"),
        )
        .expect("fixed history")
};

#[derive(Debug)]
pub enum ScalarFixedPointHistoryErrorV1 {
    Resource(Resource),
    Header,
    Length,
    Reserved,
    Limit,
    Round {
        ordinal: usize,
    },
    Field {
        ordinal: usize,
        field: usize,
    },
    Composition,
    Terminal {
        ordinal: usize,
    },
    Core(CheckedScalarFixedPointErrorV1),
    Transition(CanonicalKirTransitionReceiptErrorV1),
    Policy3(CanonicalPolicy3ExecutionReceiptErrorV1),
    Admission {
        ordinal: usize,
        integer: bool,
        error: CanonicalKernelIrReplayAdmissionErrorV12,
    },
    IntegerClaim(fe2o3_pliron::IntegerContinuationClaimErrorV1),
    Semantic(crate::KernelIrCheckedOptimizationReceiptErrorV1),
    Panicked,
}
pub(super) type Error = ScalarFixedPointHistoryErrorV1;
pub(super) type Result<T> = std::result::Result<T, Error>;
pub(super) type Meter<'a, 'w> = resources::Meter<'a, 'w, Error>;
impl From<Resource> for Error {
    fn from(v: Resource) -> Self {
        Self::Resource(v)
    }
}
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "scalar history: {self:?}")
    }
}
impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Core(e) => Some(e),
            Self::Transition(e) => Some(e),
            Self::Policy3(e) => Some(e),
            Self::Admission { error, .. } => Some(error),
            Self::IntegerClaim(e) => Some(e),
            Self::Semantic(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarFixedPointHistoryStorageV1(pub(super) usize);
impl ScalarFixedPointHistoryStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy)]
pub struct InertScalarFixedPointRoundRefV1<'w> {
    pub(super) fields: [&'w [u8]; FIELDS],
}
impl<'w> InertScalarFixedPointRoundRefV1<'w> {
    pub fn integer_output_bytes(&self) -> &'w [u8] {
        self.fields[0]
    }
    pub fn output_bytes(&self) -> &'w [u8] {
        self.fields[1]
    }
    pub fn integer_record(&self) -> &'w [u8] {
        self.fields[2]
    }
    pub fn integer_transition(&self) -> &'w [u8] {
        self.fields[3]
    }
    pub fn policy3_receipt(&self) -> &'w [u8] {
        self.fields[4]
    }
}
/// Outer closed framing only; children remain untrusted until independent replay.
pub struct InertScalarFixedPointHistoryRefV1<'w> {
    wire: &'w [u8],
    execution: &'w [u8; SCALAR_FIXED_POINT_EXECUTION_BYTES_V1],
    count: usize,
    rounds: [InertScalarFixedPointRoundRefV1<'w>; SCALAR_FIXED_POINT_MAX_ROUNDS_V1],
    final_graph: std::ops::Range<usize>,
}
impl<'w> InertScalarFixedPointHistoryRefV1<'w> {
    pub const fn canonical_bytes(&self) -> &'w [u8] {
        self.wire
    }
    pub const fn execution_claim(&self) -> &'w [u8; SCALAR_FIXED_POINT_EXECUTION_BYTES_V1] {
        self.execution
    }
    pub fn rounds(&self) -> &[InertScalarFixedPointRoundRefV1<'w>] {
        &self.rounds[..self.count]
    }
    pub fn output_bytes(&self) -> &'w [u8] {
        self.rounds[self.count - 1].output_bytes()
    }
    /// Absolute canonical-wire range selected by the closed last-round field-1
    /// directory, not a caller-provided substring. Framing alone proves no semantics.
    pub fn final_graph_range(&self) -> std::ops::Range<usize> {
        self.final_graph.clone()
    }
    pub const fn storage(&self) -> ScalarFixedPointHistoryStorageV1 {
        ScalarFixedPointHistoryStorageV1(size_of::<Self>())
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}
pub struct InertScalarFixedPointHistoryBytesV1 {
    bytes: Vec<u8>,
    storage: ScalarFixedPointHistoryStorageV1,
}
impl InertScalarFixedPointHistoryBytesV1 {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub const fn storage(&self) -> ScalarFixedPointHistoryStorageV1 {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}
pub(super) fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or_else(|| Resource::Arithmetic.into())
}
pub(super) fn configured(b: &Budget<'_>) -> Result<()> {
    if b.storage_limit() > MAX_LOOP_UNROLL_HISTORY_STORAGE_V1 {
        Err(Error::Limit)
    } else {
        Ok(())
    }
}
pub(super) fn same(a: &[u8], b: &[u8], meter: &mut Meter<'_, '_>) -> Result<bool> {
    meter.work(add(add(a.len(), b.len())?, 1)?)?;
    Ok(a == b)
}
fn wide(bytes: &[u8]) -> Result<usize> {
    usize::try_from(u64::from_le_bytes(
        bytes.try_into().map_err(|_| Error::Length)?,
    ))
    .map_err(|_| Resource::Arithmetic.into())
}
fn valid_execution(
    bytes: &[u8; SCALAR_FIXED_POINT_EXECUTION_BYTES_V1],
    count: usize,
) -> Result<()> {
    if bytes[..8] != *b"F2SFP1\0\0"
        || bytes[8..12] != [1, 0, 16, 0]
        || bytes[12..14] != (count as u16).to_le_bytes()
        || bytes[14..16] != [2, 0]
        || bytes[96..104] != [6, 0, 2, 0, 0, 0, 0, 0]
        || bytes[104..112] != [3, 0, 8, 0, 1, 0, 0, 0]
    {
        return Err(Error::Composition);
    }
    if bytes[112..] != [0; 16] {
        return Err(Error::Reserved);
    }
    Ok(())
}
fn parse<'w>(
    wire: &'w [u8],
    meter: &mut Meter<'_, '_>,
) -> Result<InertScalarFixedPointHistoryRefV1<'w>> {
    // The public reader establishes HEADER before this bounded fixed indexing.
    meter.work(HEADER)?;
    if wire[..8] != SCALAR_FIXED_POINT_HISTORY_MAGIC_V1
        || wire[8..12] != [1, 0, 1, 0]
        || wire[12..16] != (HEADER as u32).to_le_bytes()
    {
        return Err(Error::Header);
    }
    if wide(&wire[16..24])? != wire.len() {
        return Err(Error::Length);
    }
    if wire[26..32] != [0; 6] {
        return Err(Error::Reserved);
    }
    let count = usize::from(u16::from_le_bytes(
        wire[24..26].try_into().map_err(|_| Error::Length)?,
    ));
    if count == 0 || count > SCALAR_FIXED_POINT_MAX_ROUNDS_V1 {
        return Err(Error::Limit);
    }
    let execution = wire[32..HEADER].try_into().map_err(|_| Error::Length)?;
    valid_execution(execution, count)?;
    let table = count
        .checked_mul(ROUND_HEADER)
        .ok_or(Resource::Arithmetic)?;
    let mut at = add(HEADER, table)?;
    if at > wire.len() {
        return Err(Error::Length);
    }
    meter.work(table)?;
    let mut rounds = [InertScalarFixedPointRoundRefV1 {
        fields: [&[]; FIELDS],
    }; SCALAR_FIXED_POINT_MAX_ROUNDS_V1];
    let mut final_graph = 0..0;
    for (ordinal, round) in rounds[..count].iter_mut().enumerate() {
        let header = HEADER + ordinal * ROUND_HEADER;
        if wire[header..header + 2] != (ordinal as u16).to_le_bytes()
            || wire[header + 2..header + 4] != [1, 0]
        {
            return Err(Error::Round { ordinal });
        }
        if wire[header + 4..header + 8] != [0; 4] {
            return Err(Error::Reserved);
        }
        for field in 0..FIELDS {
            let len = wide(&wire[header + 8 + 8 * field..header + 16 + 8 * field])?;
            let valid = match field {
                0 | 1 => (1..=MAX_MODULE_BYTES_V1).contains(&len),
                2 => len == fe2o3_pliron::INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1,
                3 => (1..=MAX_CANONICAL_KIR_TRANSITION_RECEIPT_BYTES_V1).contains(&len),
                4 => (crate::CANONICAL_POLICY3_EXECUTION_RECEIPT_HEADER_V1
                    ..=MAX_CANONICAL_POLICY3_EXECUTION_RECEIPT_BYTES_V1)
                    .contains(&len),
                _ => false,
            };
            if !valid {
                return Err(Error::Field { ordinal, field });
            }
            let end = add(at, len)?;
            round.fields[field] = wire.get(at..end).ok_or(Error::Length)?;
            if ordinal + 1 == count && field == 1 {
                final_graph = at..end;
            }
            at = end;
        }
    }
    if at != wire.len() {
        return Err(Error::Length);
    }
    meter.work(1)?;
    Ok(InertScalarFixedPointHistoryRefV1 {
        wire,
        execution,
        count,
        rounds,
        final_graph,
    })
}
/// Caller prepays complete wire. Returned fixed frame is unreserved; no child
/// graph or opaque execution/transition record is admitted by this reader.
pub fn read_scalar_fixed_point_history_v1<'w>(
    wire: &'w [u8],
    budget: &mut Budget<'_>,
) -> Result<InertScalarFixedPointHistoryRefV1<'w>> {
    configured(budget)?;
    if wire.len() > MAX_SCALAR_FIXED_POINT_HISTORY_BYTES_V1 {
        return Err(Error::Limit);
    }
    if wire.len() < HEADER {
        return Err(Error::Length);
    }
    if budget.storage() < wire.len() {
        return Err(Resource::Accounting.into());
    }
    resources::scoped(budget, |meter| {
        meter.reserve(add(
            size_of::<InertScalarFixedPointHistoryRefV1<'_>>(),
            SCRATCH,
        )?)?;
        parse(wire, meter)
    })
}

struct EncodedRound {
    integer: Transition,
    scalar: InertCanonicalPolicy3ExecutionReceiptV1,
}
/// Serializes only a genuine complete owner after fresh replay. Input/core and
/// siblings stay prepaid; all intermediate receipts coexist on this ledger.
/// Result transfers its full header/capacity unreserved, never execution authority.
pub fn encode_scalar_fixed_point_history_v1(
    input: &Owner,
    core: &Core,
    budget: &mut Budget<'_>,
) -> Result<InertScalarFixedPointHistoryBytesV1> {
    configured(budget)?;
    if budget.storage()
        < add(
            input.canonical().canonical_bytes().len(),
            core.retained_storage(),
        )?
    {
        return Err(Resource::Accounting.into());
    }
    resources::scoped(budget, |meter| {
        meter.reserve(add(
            size_of::<Vec<EncodedRound>>(),
            size_of::<[InertScalarFixedPointRoundRefV1<'_>; SCALAR_FIXED_POINT_MAX_ROUNDS_V1]>(),
        )?)?;
        meter.derive(|b| core.replay_against(input, b).map_err(Error::Core))?;
        let count = core.rounds().len();
        let (mut encoded, _) = meter.table::<EncodedRound>(count)?;
        let mut before = input;
        for round in core.rounds() {
            meter.work(1)?;
            let (integer, receipt) = meter.derive(|b| {
                Transition::from_candidate_with_budget(
                    before.canonical().identity(),
                    round.integer().owner().canonical().identity(),
                    round.integer().occurrences().candidate(),
                    b,
                )
                .map_err(Error::Transition)
            })?;
            meter.reserve(receipt.retained_storage())?;
            let scalar = meter.derive(|b| {
                encode_checked_canonical_policy3_execution_receipt_v1(
                    round.integer().owner(),
                    round.scalar(),
                    b,
                )
                .map_err(Error::Policy3)
            })?;
            meter.reserve(scalar.storage().retained_storage())?;
            meter.push(&mut encoded, EncodedRound { integer, scalar })?;
            before = round.output();
        }
        let mut rounds = [InertScalarFixedPointRoundRefV1 {
            fields: [&[]; FIELDS],
        }; SCALAR_FIXED_POINT_MAX_ROUNDS_V1];
        for ((fields, round), encoded) in
            rounds[..count].iter_mut().zip(core.rounds()).zip(&encoded)
        {
            fields.fields = [
                round.integer().owner().canonical().canonical_bytes(),
                round.output().canonical().canonical_bytes(),
                round.integer().execution().canonical_bytes(),
                encoded.integer.canonical_bytes(),
                encoded.scalar.canonical_bytes(),
            ];
        }
        write_fields(core.execution().canonical_bytes(), &rounds[..count], meter)
    })
}

/// Canonically rewrites every parsed field without constructing optimizer owners.
/// Wire/frame remain prepaid; the full new byte owner transfers unreserved.
pub fn reencode_scalar_fixed_point_history_v1(
    frame: &InertScalarFixedPointHistoryRefV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<InertScalarFixedPointHistoryBytesV1> {
    configured(budget)?;
    if budget.storage() < add(frame.canonical_bytes().len(), frame.storage().0)? {
        return Err(Resource::Accounting.into());
    }
    resources::scoped(budget, |meter| {
        write_fields(frame.execution_claim(), frame.rounds(), meter)
    })
}
fn write_fields(
    execution: &[u8; SCALAR_FIXED_POINT_EXECUTION_BYTES_V1],
    rounds: &[InertScalarFixedPointRoundRefV1<'_>],
    meter: &mut Meter<'_, '_>,
) -> Result<InertScalarFixedPointHistoryBytesV1> {
    meter.reserve(add(
        size_of::<InertScalarFixedPointHistoryBytesV1>(),
        SCRATCH,
    )?)?;
    let count = rounds.len();
    meter.work(add(
        count.checked_mul(FIELDS).ok_or(Resource::Arithmetic)?,
        1,
    )?)?;
    if count == 0 || count > SCALAR_FIXED_POINT_MAX_ROUNDS_V1 {
        return Err(Error::Limit);
    }
    let mut total = add(
        HEADER,
        count
            .checked_mul(ROUND_HEADER)
            .ok_or(Resource::Arithmetic)?,
    )?;
    for round in rounds {
        for field in round.fields {
            total = add(total, field.len())?;
        }
    }
    if total > MAX_SCALAR_FIXED_POINT_HISTORY_BYTES_V1 {
        return Err(Error::Limit);
    }
    let (mut bytes, capacity) = meter.table::<u8>(total)?;
    meter.work(total)?;
    bytes.resize(total, 0);
    meter.work(total)?;
    bytes[..8].copy_from_slice(&SCALAR_FIXED_POINT_HISTORY_MAGIC_V1);
    bytes[8..12].copy_from_slice(&[1, 0, 1, 0]);
    bytes[12..16].copy_from_slice(&(HEADER as u32).to_le_bytes());
    bytes[16..24].copy_from_slice(
        &u64::try_from(total)
            .map_err(|_| Resource::Arithmetic)?
            .to_le_bytes(),
    );
    bytes[24..26].copy_from_slice(&(count as u16).to_le_bytes());
    bytes[32..HEADER].copy_from_slice(execution);
    let mut at = HEADER + count * ROUND_HEADER;
    for (ordinal, round) in rounds.iter().enumerate() {
        let header = HEADER + ordinal * ROUND_HEADER;
        bytes[header..header + 2].copy_from_slice(&(ordinal as u16).to_le_bytes());
        bytes[header + 2..header + 4].copy_from_slice(&1u16.to_le_bytes());
        for (field, data) in round.fields.into_iter().enumerate() {
            bytes[header + 8 + field * 8..header + 16 + field * 8].copy_from_slice(
                &u64::try_from(data.len())
                    .map_err(|_| Resource::Arithmetic)?
                    .to_le_bytes(),
            );
            let end = add(at, data.len())?;
            bytes[at..end].copy_from_slice(data);
            at = end;
        }
    }
    if at != total {
        return Err(Error::Length);
    }
    let frame = meter.derive(|b| read_scalar_fixed_point_history_v1(&bytes, b))?;
    let paid = frame.storage().retained_storage();
    meter.reserve(paid)?;
    meter.work(1)?;
    drop(frame);
    meter.release(paid)?;
    Ok(InertScalarFixedPointHistoryBytesV1 {
        bytes,
        storage: ScalarFixedPointHistoryStorageV1(add(
            size_of::<InertScalarFixedPointHistoryBytesV1>(),
            capacity,
        )?),
    })
}

#[cfg(test)]
#[path = "scalar_fixed_point_history_v1_tests.rs"]
pub(crate) mod tests;
