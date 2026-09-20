//! Inert fixed-P8 history framing. Nested prefix and raw pool admission is pending.
use crate::POLICY8_COMMUTATIVE_PASS_NAME_V1;
use fe2o3_kernel_analysis::CanonicalKirLoadForwardingRowV1 as LoadRow;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirOccurrenceRowsRefV1 as RowView, CanonicalKirTransitionReceiptErrorV1 as RowError,
    InertCanonicalKirOccurrenceRowBytesV1 as RowBytes, MAX_MODULE_BYTES_V1,
    VerifiedCanonicalKernelIrModuleV12 as Owner, read_canonical_kir_occurrence_row_bytes_v1,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

/// Distinct inert fixed-P8 container, not F2NTR policy1 or an execution seal.
pub const CANONICAL_POLICY8_HISTORY_MAGIC_V1: [u8; 8] = *b"F2HET1\0\0";
/// Complete aggregate ceiling; nested sections receive no additional allowance.
pub const MAX_CANONICAL_POLICY8_HISTORY_BYTES_V1: usize = 4 * 1024 * 1024;
const HEADER: usize = 56;
const ROLE_BYTES: usize = 44;
const ROLES: usize = 7;
const FIELDS: usize = 8;
const START: usize = HEADER + ROLES * ROLE_BYTES;
const EXTERNAL: u32 = u32::MAX;
const TAIL_HEADER: usize = 84;

/// Fixed endpoint roles, not a producer pass or runtime policy selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalPolicy8HistoryRoleV1 {
    /// Target-bound input.
    B,
    /// Scalar/CFG prefix output.
    C,
    /// Store-forwarding output.
    S,
    /// Load-forwarding output.
    O,
    /// Integer-continuation output.
    I,
    /// Redundant-store output.
    J,
    /// Actual external commutative output.
    K,
}

/// Framing/resource failure only, never a semantic optimizer verdict.
#[derive(Debug)]
pub enum CanonicalPolicy8HistoryErrorV1 {
    /// Shared caller resource refusal.
    Resource(Resource),
    /// Neutral row syntax or partition refusal.
    Rows(RowError),
    /// Invalid fixed header or schedule.
    Header,
    /// Checked count/extent/cap failed.
    Limit,
    /// Inconsistent role locator or external K.
    Role,
    /// Noncanonical pool reference, sharing, or unused payload.
    Pool,
    /// Invalid section-level structure; opaque contents remain unvalidated.
    Field(&'static str),
    /// Local backing was dropped after an unwind.
    Panicked,
}
type Error = CanonicalPolicy8HistoryErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<RowError> for Error {
    fn from(value: RowError) -> Self {
        Self::Rows(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "inert Policy8 history: {self:?}")
    }
}
impl std::error::Error for Error {}

/// A raw claimed graph locator. Pool digests have NOT been verified by decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertPolicy8HistoryGraphLocatorV1 {
    digest: [u8; 32],
    length: u64,
    reference: u32,
}
impl InertPolicy8HistoryGraphLocatorV1 {
    /// Claimed digest, not a newly admitted graph identity.
    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }
    /// Claimed canonical byte length, checked against the referenced slice.
    pub const fn canonical_length(self) -> u64 {
        self.length
    }
    /// Whether the role names the actual external final K.
    pub const fn is_external_output(self) -> bool {
        self.reference == EXTERNAL
    }
    /// Pool ordinal if not the external final output.
    pub const fn pool_index(self) -> Option<u32> {
        if self.reference == EXTERNAL {
            None
        } else {
            Some(self.reference)
        }
    }
}

/// New logical header/byte capacity or fixed borrowed-view storage, unreserved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalPolicy8HistoryStorageV1(usize);
impl CanonicalPolicy8HistoryStorageV1 {
    /// Reserve while the returned value lives before subsequent controlled work.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Encoder inputs use actual owner identities, but remain unauthenticated claims.
/// Borrowed inputs, including the row-body owner, stay reserved once or external.
pub struct CanonicalPolicy8HistoryEncodingInputsV1<'a> {
    /// Fixed B/C/S/O/I/J/K owners; no caller-supplied role hashes or pool indices.
    pub roles: [&'a Owner; ROLES],
    /// Opaque bounded complete P4 claim, not checked by this serializer.
    pub policy4_wire: &'a [u8],
    /// Opaque exactly 200-byte P5 claim.
    pub policy5_record: &'a [u8],
    /// Inert first/load coordinates; memory equality remains unproved.
    pub load_rows: &'a [LoadRow],
    /// Opaque exactly 416-byte integer claim.
    pub integer_record: &'a [u8],
    /// Opaque bounded O/I transition frame.
    pub transition_wire: &'a [u8],
    /// Opaque P7 claim, at least 384 bytes; its inline P6 remains unvalidated.
    pub policy7_record: &'a [u8],
    /// Neutral row bytes encoded directly from typed candidates, never policy1.
    pub tail_rows: &'a RowBytes,
}

/// Owns only inert container bytes, not graphs, checked history or producer seal.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::InertCanonicalPolicy8HistoryV1;
/// fn duplicate(value: InertCanonicalPolicy8HistoryV1) { let _ = value.clone(); }
/// ```
pub struct InertCanonicalPolicy8HistoryV1 {
    bytes: Vec<u8>,
    storage: CanonicalPolicy8HistoryStorageV1,
}
impl InertCanonicalPolicy8HistoryV1 {
    /// Complete inert container bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Exact new logical header and observed Vec capacity.
    pub const fn storage(&self) -> CanonicalPolicy8HistoryStorageV1 {
        self.storage
    }
    /// Serialization does not authenticate execution.
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
    /// No source, artifact, proof or launch authority is granted.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Allocation-free container view with fully parsed neutral P8 row syntax only.
/// Pool graph hashes/grammar and all historical nested formats remain unverified.
/// K is the actual supplied owner; equal bytes do not authenticate producer custody.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::{InertPolicy8HistoryRefV1, read_inert_policy8_history_v1 as read};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn escape<'k>(wire: Vec<u8>, k: &'k Owner, b: &mut Budget<'_>) -> InertPolicy8HistoryRefV1<'static, 'k> {
///     read(&wire, k, b).unwrap()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::{InertPolicy8HistoryRefV1, read_inert_policy8_history_v1 as read};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn escape<'a>(wire: &'a [u8], k: Owner, b: &mut Budget<'_>) -> InertPolicy8HistoryRefV1<'a, 'static> {
///     read(wire, &k, b).unwrap()
/// }
/// ```
pub struct InertPolicy8HistoryRefV1<'wire, 'k> {
    wire: &'wire [u8],
    output: &'k Owner,
    roles: [InertPolicy8HistoryGraphLocatorV1; ROLES],
    pool: [[usize; 2]; ROLES - 1],
    pool_count: usize,
    fields: [[usize; 2]; FIELDS],
    tail: RowView<'wire>,
    storage: CanonicalPolicy8HistoryStorageV1,
}
impl<'wire, 'k> InertPolicy8HistoryRefV1<'wire, 'k> {
    fn field(&self, index: usize) -> &'wire [u8] {
        let [start, end] = self.fields[index];
        &self.wire[start..end]
    }
    /// Original container backing; still not a semantic receipt.
    pub const fn canonical_bytes(&self) -> &'wire [u8] {
        self.wire
    }
    /// The exact externally supplied final output owner.
    pub const fn external_output(&self) -> &'k Owner {
        self.output
    }
    /// Explicit role locator, including unverified pool digest claims.
    pub fn role(&self, role: CanonicalPolicy8HistoryRoleV1) -> InertPolicy8HistoryGraphLocatorV1 {
        self.roles[role as usize]
    }
    /// Borrowed graph bytes; a pool payload is NOT an admitted V12 module.
    pub fn graph_bytes(&self, role: CanonicalPolicy8HistoryRoleV1) -> &[u8] {
        let reference = self.role(role).reference;
        if reference == EXTERNAL {
            self.output.canonical().canonical_bytes()
        } else {
            let [start, end] = self.pool[reference as usize];
            &self.wire[start..end]
        }
    }
    /// Number of distinct non-K stored payloads.
    pub const fn stored_graph_count(&self) -> usize {
        self.pool_count
    }
    /// Raw pool bytes by canonical first-use ordinal. No graph grammar, hash or
    /// verification is established; the borrow cannot outlive container backing.
    pub fn unverified_pool_graph_bytes(&self, index: usize) -> Option<&'wire [u8]> {
        if index >= self.pool_count {
            return None;
        }
        let [start, end] = self.pool[index];
        Some(&self.wire[start..end])
    }
    /// Opaque P4/P3/C/S claim bytes.
    pub fn policy4_wire(&self) -> &'wire [u8] {
        self.field(1)
    }
    /// Opaque P5 claim bytes.
    pub fn policy5_record(&self) -> &'wire [u8] {
        self.field(2)
    }
    /// Count and coordinate bytes only, not checked forwarding semantics.
    pub fn load_row_bytes(&self) -> &'wire [u8] {
        self.field(3)
    }
    /// Opaque integer-continuation claim bytes.
    pub fn integer_record(&self) -> &'wire [u8] {
        self.field(4)
    }
    /// Opaque O/I bytes, requiring the historical F2NTR decoder/checker.
    pub fn transition_wire(&self) -> &'wire [u8] {
        self.field(5)
    }
    /// Opaque P7 bytes, requiring its existing strict row decoder and checker.
    pub fn policy7_record(&self) -> &'wire [u8] {
        self.field(6)
    }
    /// Unvalidated inline candidate P6 slice, NOT an admitted record.
    pub fn unvalidated_policy6_record(&self) -> &'wire [u8] {
        &self.field(6)[16..272]
    }
    /// P8 row syntax/partitions only, without typed semantic admission.
    pub const fn tail_rows(&self) -> &RowView<'wire> {
        &self.tail
    }
    /// Fixed view plus its embedded neutral-row header, counted exactly once.
    pub const fn storage(&self) -> CanonicalPolicy8HistoryStorageV1 {
        self.storage
    }
    /// No producer execution is authenticated by container parsing.
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
    /// No source, proof, artifact or launch authority is granted.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn scoped<'work, T>(
    budget: &mut Budget<'work>,
    run: impl FnOnce(&mut Budget<'work>) -> Result<T>,
) -> Result<T> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(value) => value,
        Err(payload) => {
            payloads[0] = Some(payload);
            Err(Error::Panicked)
        }
    };
    let cleanup = if ledger != budget.work_ledger_identity_v1() {
        Err(Resource::Accounting)
    } else {
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)
            .and_then(|bytes| budget.release_storage(bytes))
    };
    if let Err(error) = cleanup {
        let rejected = std::mem::replace(&mut result, Err(error.into()));
        payloads[1] = catch_unwind(AssertUnwindSafe(|| drop(rejected))).err();
    }
    drop(payloads);
    result
}
fn word(bytes: &[u8], at: usize) -> Result<u32> {
    let end = at.checked_add(4).ok_or(Error::Limit)?;
    Ok(u32::from_le_bytes(
        bytes
            .get(at..end)
            .ok_or(Error::Limit)?
            .try_into()
            .map_err(|_| Error::Limit)?,
    ))
}
fn length(value: u32) -> Result<usize> {
    usize::try_from(value).map_err(|_| Error::Limit)
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or(Error::Limit)
}
fn reconcile_capacity(requested: usize, observed: usize, budget: &mut Budget<'_>) -> Result<()> {
    budget.reserve_storage(
        observed
            .checked_sub(requested)
            .ok_or(Resource::Accounting)?,
    )?;
    Ok(())
}
fn same_bytes(a: &[u8], b: &[u8], budget: &mut Budget<'_>) -> Result<bool> {
    budget.charge_work(1)?;
    if a.len() != b.len() {
        return Ok(false);
    }
    budget.charge_work(add(a.len(), b.len())?)?;
    Ok(a == b)
}
fn prefix_lengths(lengths: [usize; FIELDS]) -> Result<()> {
    if lengths[1] == 0
        || lengths[2] != 200
        || lengths[4] != 416
        || lengths[5] == 0
        || lengths[6] < 384
    {
        return Err(Error::Field("opaque prefix lengths"));
    }
    Ok(())
}
fn load_coordinate(bytes: &[u8]) -> Result<[u32; 3]> {
    Ok([word(bytes, 12)?, word(bytes, 16)?, word(bytes, 20)?])
}
fn check_load_bytes(bytes: &[u8], budget: &mut Budget<'_>) -> Result<()> {
    budget.charge_work(5)?;
    let count = length(word(bytes, 0)?)?;
    if count.checked_mul(24).and_then(|n| n.checked_add(4)) != Some(bytes.len()) {
        return Err(Error::Field("P5 row extent"));
    }
    budget.charge_work(add(bytes.len(), count)?)?;
    let mut previous = None;
    for bytes in bytes[4..].chunks_exact(24) {
        let load = load_coordinate(bytes)?;
        if previous.is_some_and(|prior| prior >= load) {
            return Err(Error::Field("P5 row order"));
        }
        previous = Some(load);
    }
    Ok(())
}

/// Reads only this container and its new tail grammar. Existing nested prefix
/// frames and raw pool hashes remain opaque; later decoders/admission MUST run.
/// Exact K identity, complete byte sharing, bounds and first-use order are checked.
/// All input backing is borrowed/external or separately reserved once. The
/// returned view is allocation-free with exact fixed logical storage unreserved.
pub fn read_inert_policy8_history_v1<'wire, 'k>(
    wire: &'wire [u8],
    output: &'k Owner,
    budget: &mut Budget<'_>,
) -> Result<InertPolicy8HistoryRefV1<'wire, 'k>> {
    scoped(budget, |budget| {
        budget.charge_work(1)?;
        if !(START..=MAX_CANONICAL_POLICY8_HISTORY_BYTES_V1).contains(&wire.len()) {
            return Err(Error::Limit);
        }
        budget.charge_work(START + FIELDS + ROLES)?;
        if wire[..8] != CANONICAL_POLICY8_HISTORY_MAGIC_V1
            || wire[8..10] != 1u16.to_le_bytes()
            || wire[10..12] != 8u16.to_le_bytes()
            || length(word(wire, 12)?)? != wire.len()
            || word(wire, 16)? != ROLES as u32
        {
            return Err(Error::Header);
        }
        let pool_count = length(word(wire, 20)?)?;
        if pool_count >= ROLES {
            return Err(Error::Pool);
        }
        let mut fields = [[0; 2]; FIELDS];
        let mut lengths = [0; FIELDS];
        let mut end = START;
        for (index, field) in fields.iter_mut().enumerate() {
            lengths[index] = length(word(wire, 24 + index * 4)?)?;
            let next = add(end, lengths[index])?;
            if next > wire.len() {
                return Err(Error::Limit);
            }
            *field = [end, next];
            end = next;
        }
        if end != wire.len() {
            return Err(Error::Limit);
        }
        prefix_lengths(lengths)?;
        let wrapper = size_of::<InertPolicy8HistoryRefV1<'_, '_>>()
            .checked_sub(size_of::<RowView<'_>>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(wrapper)?;
        let empty = InertPolicy8HistoryGraphLocatorV1 {
            digest: [0; 32],
            length: 0,
            reference: EXTERNAL,
        };
        let mut roles = [empty; ROLES];
        for (index, role) in roles.iter_mut().enumerate() {
            let at = HEADER + index * ROLE_BYTES;
            role.digest.copy_from_slice(&wire[at..at + 32]);
            role.length =
                u64::from_le_bytes(wire[at + 32..at + 40].try_into().map_err(|_| Error::Role)?);
            role.reference = word(wire, at + 40)?;
            if role.length == 0
                || usize::try_from(role.length)
                    .ok()
                    .is_none_or(|n| n > MAX_MODULE_BYTES_V1)
            {
                return Err(Error::Role);
            }
        }
        let mut pool = [[0; 2]; ROLES - 1];
        let mut cursor = fields[0][0];
        budget.charge_work((ROLES - 1) * 5)?;
        for slot in pool.iter_mut().take(pool_count) {
            let bytes = length(word(wire.get(..fields[0][1]).ok_or(Error::Pool)?, cursor)?)?;
            cursor = add(cursor, 4)?;
            if bytes == 0 || bytes > MAX_MODULE_BYTES_V1 {
                return Err(Error::Pool);
            }
            let next = add(cursor, bytes)?;
            if next > fields[0][1] {
                return Err(Error::Pool);
            }
            *slot = [cursor, next];
            cursor = next;
        }
        if cursor != fields[0][1] {
            return Err(Error::Pool);
        }
        if roles[6].reference != EXTERNAL {
            return Err(Error::Role);
        }
        let k = output.canonical();
        let mut next_pool = 0;
        for (index, role) in roles.iter().enumerate() {
            budget.charge_work(2)?;
            if role.reference == EXTERNAL {
                budget.charge_work(80)?;
                if role.digest != *k.identity().digest()
                    || role.length != k.identity().canonical_length()
                {
                    return Err(Error::Role);
                }
            } else {
                let reference = length(role.reference)?;
                if reference >= pool_count {
                    return Err(Error::Pool);
                }
                let [start, end] = pool[reference];
                if usize::try_from(role.length).ok() != Some(end - start) {
                    return Err(Error::Role);
                }
                budget.charge_work(index.checked_mul(81).ok_or(Error::Limit)?)?;
                if let Some(prior) = roles[..index]
                    .iter()
                    .find(|r| r.reference == role.reference)
                {
                    if prior.digest != role.digest || prior.length != role.length {
                        return Err(Error::Role);
                    }
                } else {
                    if reference != next_pool {
                        return Err(Error::Pool);
                    }
                    next_pool += 1;
                }
            }
        }
        if next_pool != pool_count {
            return Err(Error::Pool);
        }
        for index in 0..pool_count {
            let [start, end] = pool[index];
            let bytes = &wire[start..end];
            if same_bytes(bytes, k.canonical_bytes(), budget)? {
                return Err(Error::Pool);
            }
            for prior in &pool[..index] {
                if same_bytes(bytes, &wire[prior[0]..prior[1]], budget)? {
                    return Err(Error::Pool);
                }
            }
        }
        check_load_bytes(&wire[fields[3][0]..fields[3][1]], budget)?;
        let tail = &wire[fields[7][0]..fields[7][1]];
        budget.charge_work(TAIL_HEADER + 10)?;
        if tail.len() < TAIL_HEADER
            || length(word(tail, 0)?)? != tail.len()
            || word(tail, 4)? != 40
            || &tail[44..84] != POLICY8_COMMUTATIVE_PASS_NAME_V1.as_bytes()
        {
            return Err(Error::Field("P8 tail header"));
        }
        let mut counts = [0; 9];
        for (index, count) in counts.iter_mut().enumerate() {
            *count = word(tail, 8 + index * 4)?;
        }
        let tail =
            read_canonical_kir_occurrence_row_bytes_v1(&tail[TAIL_HEADER..], counts, budget)?;
        let row_storage = tail.storage().retained_storage();
        budget.reserve_storage(row_storage)?;
        let storage = CanonicalPolicy8HistoryStorageV1(add(wrapper, row_storage)?);
        Ok(InertPolicy8HistoryRefV1 {
            wire,
            output,
            roles,
            pool,
            pool_count,
            fields,
            tail,
            storage,
        })
    })
}

struct Plan<'a> {
    roles: [InertPolicy8HistoryGraphLocatorV1; ROLES],
    pool: [Option<&'a Owner>; ROLES - 1],
    count: usize,
}
fn plan<'a>(owners: [&'a Owner; ROLES], budget: &mut Budget<'_>) -> Result<Plan<'a>> {
    budget.charge_work(ROLES * 41)?;
    let blank = InertPolicy8HistoryGraphLocatorV1 {
        digest: [0; 32],
        length: 0,
        reference: EXTERNAL,
    };
    let mut plan = Plan {
        roles: [blank; ROLES],
        pool: [None; ROLES - 1],
        count: 0,
    };
    for (index, owner) in owners.iter().enumerate() {
        let mut reference = EXTERNAL;
        if !same_bytes(
            owner.canonical().canonical_bytes(),
            owners[6].canonical().canonical_bytes(),
            budget,
        )? {
            let mut found = None;
            for (ordinal, prior) in plan.pool[..plan.count].iter().enumerate() {
                if same_bytes(
                    owner.canonical().canonical_bytes(),
                    prior.ok_or(Error::Pool)?.canonical().canonical_bytes(),
                    budget,
                )? {
                    found = Some(ordinal);
                    break;
                }
            }
            reference = if let Some(ordinal) = found {
                ordinal as u32
            } else {
                if plan.count >= plan.pool.len() {
                    return Err(Error::Pool);
                }
                let ordinal = plan.count;
                plan.pool[ordinal] = Some(*owner);
                plan.count += 1;
                ordinal as u32
            };
        }
        plan.roles[index] = InertPolicy8HistoryGraphLocatorV1 {
            digest: *owner.canonical().identity().digest(),
            length: owner.canonical().identity().canonical_length(),
            reference,
        };
    }
    Ok(plan)
}
struct Writer {
    bytes: Vec<u8>,
    length: usize,
}
impl Writer {
    fn put(&mut self, bytes: &[u8]) -> Result<()> {
        if add(self.bytes.len(), bytes.len())? > self.length {
            return Err(Error::Limit);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn word(&mut self, value: usize) -> Result<()> {
        self.put(
            &u32::try_from(value)
                .map_err(|_| Error::Limit)?
                .to_le_bytes(),
        )
    }
}

/// Serializes actual owner locators with inert prefix/row claims. No semantics,
/// execution authentication, graph decoding or source/native admission occurs.
/// Exact observed output capacity plus the fixed planning scratch is prepaid;
/// all caller-owned inputs, including row bytes, must remain reserved or external.
/// Returns new owned payload unreserved on the same cumulative intact ledger.
pub fn encode_inert_policy8_history_v1(
    inputs: CanonicalPolicy8HistoryEncodingInputsV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<(
    InertCanonicalPolicy8HistoryV1,
    CanonicalPolicy8HistoryStorageV1,
)> {
    scoped(budget, |budget| {
        budget.charge_work(1 + FIELDS + ROLES)?;
        budget.reserve_storage(size_of::<Plan<'_>>())?;
        let plan = plan(inputs.roles, budget)?;
        let mut pool_bytes = 0;
        for owner in plan.pool[..plan.count].iter() {
            pool_bytes = add(
                pool_bytes,
                add(
                    4,
                    owner
                        .ok_or(Error::Pool)?
                        .canonical()
                        .canonical_bytes()
                        .len(),
                )?,
            )?;
        }
        let load_bytes = inputs
            .load_rows
            .len()
            .checked_mul(24)
            .and_then(|n| n.checked_add(4))
            .ok_or(Error::Limit)?;
        let tail_bytes = add(TAIL_HEADER, inputs.tail_rows.canonical_row_bytes().len())?;
        let lengths = [
            pool_bytes,
            inputs.policy4_wire.len(),
            inputs.policy5_record.len(),
            load_bytes,
            inputs.integer_record.len(),
            inputs.transition_wire.len(),
            inputs.policy7_record.len(),
            tail_bytes,
        ];
        prefix_lengths(lengths)?;
        let length = lengths.into_iter().try_fold(START, add)?;
        if length > MAX_CANONICAL_POLICY8_HISTORY_BYTES_V1 {
            return Err(Error::Limit);
        }
        budget.charge_work(add(length, inputs.load_rows.len())?)?;
        let mut previous = None;
        for row in inputs.load_rows {
            if previous.is_some_and(|prior| prior >= row.load) {
                return Err(Error::Field("P5 row order"));
            }
            previous = Some(row.load);
        }
        let header = size_of::<InertCanonicalPolicy8HistoryV1>();
        budget.reserve_storage(add(header, length)?)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| Resource::Allocation)?;
        let capacity = bytes.capacity();
        reconcile_capacity(length, capacity, budget)?;
        let mut writer = Writer { bytes, length };
        writer.put(&CANONICAL_POLICY8_HISTORY_MAGIC_V1)?;
        writer.put(&1u16.to_le_bytes())?;
        writer.put(&8u16.to_le_bytes())?;
        writer.word(length)?;
        writer.word(ROLES)?;
        writer.word(plan.count)?;
        for length in lengths {
            writer.word(length)?;
        }
        for role in plan.roles {
            writer.put(&role.digest)?;
            writer.put(&role.length.to_le_bytes())?;
            writer.put(&role.reference.to_le_bytes())?;
        }
        for owner in plan.pool[..plan.count].iter() {
            let bytes = owner.ok_or(Error::Pool)?.canonical().canonical_bytes();
            writer.word(bytes.len())?;
            writer.put(bytes)?;
        }
        writer.put(inputs.policy4_wire)?;
        writer.put(inputs.policy5_record)?;
        writer.word(inputs.load_rows.len())?;
        for row in inputs.load_rows {
            for site in [row.first, row.load] {
                writer.put(&site.block.function.0.to_le_bytes())?;
                writer.put(&site.block.block.to_le_bytes())?;
                writer.put(&site.operation.to_le_bytes())?;
            }
        }
        writer.put(inputs.integer_record)?;
        writer.put(inputs.transition_wire)?;
        writer.put(inputs.policy7_record)?;
        writer.word(tail_bytes)?;
        writer.word(POLICY8_COMMUTATIVE_PASS_NAME_V1.len())?;
        for count in inputs.tail_rows.counts() {
            writer.put(&count.to_le_bytes())?;
        }
        writer.put(POLICY8_COMMUTATIVE_PASS_NAME_V1.as_bytes())?;
        writer.put(inputs.tail_rows.canonical_row_bytes())?;
        if writer.bytes.len() != length {
            return Err(Error::Limit);
        }
        let storage = CanonicalPolicy8HistoryStorageV1(add(header, capacity)?);
        Ok((
            InertCanonicalPolicy8HistoryV1 {
                bytes: writer.bytes,
                storage,
            },
            storage,
        ))
    })
}

#[cfg(test)]
mod scope_tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use std::{cell::Cell, rc::Rc};

    struct BackingProbe(Rc<Cell<usize>>);
    impl Drop for BackingProbe {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    struct PanicOnDrop;
    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("private transport destructor probe");
        }
    }

    #[test]
    fn actual_spare_capacity_is_reserved_before_fill_and_short_denial_keeps_floor() {
        let backing = Vec::<u8>::with_capacity(33);
        let observed = backing.capacity();
        assert!(observed >= 33);
        for shortage in [0, 1] {
            let mut work = Work::new(100);
            let mut budget = Budget::new(&mut work, 17 + observed - shortage);
            budget.reserve_storage(17).unwrap();
            let result = scoped(&mut budget, |budget| {
                budget.reserve_storage(1)?;
                reconcile_capacity(1, observed, budget)
            });
            assert_eq!(result.is_ok(), shortage == 0);
            assert_eq!(budget.storage(), 17);
            if shortage == 1 {
                assert_eq!(budget.failed_storage(), Some(17 + observed));
            }
        }
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 100);
        assert!(matches!(
            reconcile_capacity(2, 1, &mut budget),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn private_partial_backing_drops_before_scope_cleanup_and_payload_destruction() {
        for mode in 0..5 {
            let mut work = Work::new(100);
            let mut budget = Budget::new(&mut work, 100);
            budget.reserve_storage(17).unwrap();
            let drops = Rc::new(Cell::new(0));
            let result = catch_unwind(AssertUnwindSafe(|| {
                scoped::<()>(&mut budget, |budget| {
                    budget.charge_work(3)?;
                    budget.reserve_storage(8)?;
                    let _backing = Vec::from([0u8; 8]);
                    let _probe = BackingProbe(drops.clone());
                    match mode {
                        0 => Ok(()),
                        1 => Err(Resource::Allocation.into()),
                        2 => {
                            budget.reserve_storage(100)?;
                            Ok(())
                        }
                        3 => panic!("private transport panic"),
                        _ => std::panic::panic_any(PanicOnDrop),
                    }
                })
            }));
            assert_eq!((budget.storage(), budget.work(), drops.get()), (17, 3, 1));
            if mode == 4 {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap().is_ok(), mode == 0);
            }
            if mode == 2 {
                assert_eq!(budget.failed_storage(), Some(125));
            }
        }
    }

    #[test]
    fn private_foreign_ledger_ok_error_panic_and_undercut_never_refund() {
        for mode in 0..3 {
            let mut own_work = Work::new(100);
            let mut other_work = Work::new(100);
            let mut budget = Budget::new(&mut own_work, 100);
            let mut other = Budget::new(&mut other_work, 100);
            budget.reserve_storage(17).unwrap();
            other.reserve_storage(41).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = scoped::<()>(&mut budget, |budget| {
                budget.reserve_storage(3)?;
                std::mem::swap(budget, &mut other);
                match mode {
                    0 => Ok(()),
                    1 => Err(Resource::Allocation.into()),
                    _ => panic!("foreign transport"),
                }
            });
            assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
            assert_eq!((budget.storage(), other.storage()), (41, 20));
            assert!(other.work_ledger_identity_v1() == ledger);
        }
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 100);
        budget.reserve_storage(17).unwrap();
        let result = scoped::<()>(&mut budget, |budget| {
            budget.release_storage(1)?;
            Ok(())
        });
        assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
        assert_eq!(budget.storage(), 16);
    }

    #[test]
    fn rejected_success_drop_panic_is_deferred_without_foreign_refund() {
        let mut own_work = Work::new(100);
        let mut other_work = Work::new(100);
        let mut budget = Budget::new(&mut own_work, 100);
        let mut other = Budget::new(&mut other_work, 100);
        budget.reserve_storage(17).unwrap();
        other.reserve_storage(41).unwrap();
        let result = scoped(&mut budget, |budget| {
            budget.reserve_storage(3)?;
            std::mem::swap(budget, &mut other);
            Ok(PanicOnDrop)
        });
        assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
        assert_eq!((budget.storage(), other.storage()), (41, 20));
    }
}
