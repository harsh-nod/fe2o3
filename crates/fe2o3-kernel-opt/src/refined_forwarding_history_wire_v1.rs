//! Full twelve-role inert history framing, never legacy nine-slice P6 association.
use crate::{
    CanonicalPolicy7SemanticErrorV1, CanonicalRefinedForwardingHistoryInputsV1 as Inputs,
    CanonicalRefinedForwardingHistoryLimitsV1 as Limits, POLICY7_EXECUTION_HEADER_BYTES_V1,
    POLICY7_EXECUTION_ROW_BYTES_V1, POLICY8_COMMUTATIVE_PASS_NAME_V1 as PASS,
    decode_canonical_policy7_rows_v1, private_cell_promotion_resources_v1 as resources,
    refined_forwarding_history_rows_v1 as rows,
};
use fe2o3_kernel_analysis::{
    CanonicalKirCrossBlockForwardingOriginV1 as Forwarding,
    CanonicalKirInductionRefinementOriginV1 as Refinement, CanonicalKirLicmOriginV1 as Licm,
    CanonicalKirLoadForwardingRowV1 as Load, CanonicalKirLoopPreheaderV1 as Preheader,
    CanonicalKirPrivateCellOriginV1 as Promotion,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayAdmissionErrorV12 as AdmissionError,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirOperationCoordinateV1 as Site, CanonicalKirTransitionReceiptErrorV1 as TailError,
    encode_canonical_kir_occurrence_row_bytes_v1, read_canonical_kir_occurrence_row_bytes_v1,
};
use std::{fmt, mem::size_of};

pub const REFINED_FORWARDING_HISTORY_MAGIC_V1: [u8; 8] = *b"F2RFH1\0\0";
pub const MAX_REFINED_FORWARDING_HISTORY_BYTES_V1: usize = 16 * 1024 * 1024;
pub const MAX_REFINED_FORWARDING_HISTORY_GRAPH_BYTES_V1: usize = 12 * 1024 * 1024;
pub const MAX_REFINED_FORWARDING_HISTORY_ROW_BYTES_V1: usize = 4 * 1024 * 1024;
pub const MAX_REFINED_FORWARDING_HISTORY_ROWS_V1: usize = 262_144;
pub const MAX_REFINED_FORWARDING_HISTORY_STORAGE_V1: usize = 256 * 1024 * 1024;
pub(super) const HEADER: usize = 248;
pub(super) const TAIL_HEADER: usize = 4 + 40 + 80 + 36 + 8;

/// Every role is a complete, independently admitted V12 graph, even for equal bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum RefinedForwardingHistoryRoleV1 {
    B,
    C,
    S,
    O,
    I,
    J,
    K,
    P,
    H,
    L,
    R,
    F,
}

/// No diagnostics are boxed by this codec; original typed refusals are preserved.
#[derive(Debug)]
pub enum RefinedForwardingHistoryWireErrorV1 {
    Resource(Resource),
    Length,
    Header,
    Reserved,
    Limit,
    Field(usize),
    Rows,
    Tag,
    Limits,
    TailIdentity,
    Policy7Rows,
    Policy7(CanonicalPolicy7SemanticErrorV1),
    Tail(TailError),
    Admission {
        role: RefinedForwardingHistoryRoleV1,
        error: AdmissionError,
    },
    Panicked,
}
pub(super) type Error = RefinedForwardingHistoryWireErrorV1;
pub(super) type Meter<'a, 'w> = resources::Meter<'a, 'w, Error>;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "inert full history: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Complete new logical retained extent, returned unreserved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RefinedForwardingHistoryWireStorageV1(pub(super) usize);
impl RefinedForwardingHistoryWireStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Framing/closed row syntax only. Graphs and historical relations remain unchecked.
/// Borrowed wire backing must remain prepaid once, independently of this header.
pub struct InertRefinedForwardingHistoryRefV1<'w> {
    pub(super) wire: &'w [u8],
    pub(super) fields: [&'w [u8]; 27],
    pub(super) limits: Limits,
    pub(super) tail_counts: [u32; 9],
    pub(super) row_count: usize,
}
impl<'w> InertRefinedForwardingHistoryRefV1<'w> {
    pub const fn canonical_bytes(&self) -> &'w [u8] {
        self.wire
    }
    pub fn graph_bytes(&self, role: RefinedForwardingHistoryRoleV1) -> &'w [u8] {
        self.fields[role as usize]
    }
    pub const fn limits(&self) -> Limits {
        self.limits
    }
    pub const fn row_count(&self) -> usize {
        self.row_count
    }
    pub const fn storage(&self) -> RefinedForwardingHistoryWireStorageV1 {
        RefinedForwardingHistoryWireStorageV1(size_of::<Self>())
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    pub(super) fn tail_body(&self) -> &'w [u8] {
        &self.fields[19][TAIL_HEADER..]
    }
}

/// Owned exact bytes, not an optimizer execution or source/native owner.
pub struct InertRefinedForwardingHistoryBytesV1 {
    bytes: Vec<u8>,
    storage: RefinedForwardingHistoryWireStorageV1,
}
impl InertRefinedForwardingHistoryBytesV1 {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub const fn storage(&self) -> RefinedForwardingHistoryWireStorageV1 {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

pub(super) fn add(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_add(b).ok_or_else(|| Resource::Arithmetic.into())
}
pub(super) fn configured(budget: &Budget<'_>) -> Result<(), Error> {
    if budget.storage_limit() > MAX_REFINED_FORWARDING_HISTORY_STORAGE_V1 {
        Err(Error::Limit)
    } else {
        Ok(())
    }
}
fn lengths(fields: &[&[u8]; 27]) -> Result<usize, Error> {
    let total = fields.iter().try_fold(HEADER, |n, b| add(n, b.len()))?;
    if total > MAX_REFINED_FORWARDING_HISTORY_BYTES_V1 {
        return Err(Error::Limit);
    }
    if fields[..12].iter().try_fold(0, |n, b| add(n, b.len()))?
        > MAX_REFINED_FORWARDING_HISTORY_GRAPH_BYTES_V1
    {
        return Err(Error::Limit);
    }
    let row_bytes = [14, 18, 19, 20, 21, 22, 23, 24, 25]
        .into_iter()
        .try_fold(0, |n, i| add(n, fields[i].len()))?;
    if row_bytes > MAX_REFINED_FORWARDING_HISTORY_ROW_BYTES_V1 {
        return Err(Error::Limit);
    }
    for (i, field) in fields.iter().enumerate() {
        if field.is_empty() {
            return Err(Error::Field(i));
        }
    }
    if fields[13].len() != 200
        || fields[15].len() != 256
        || fields[16].len() != 416
        || fields[26].len() != 136
    {
        return Err(Error::Length);
    }
    if fields[12].len() > crate::MAX_CANONICAL_POLICY4_EXECUTION_RECEIPT_BYTES_V1
        || fields[17].len() > fe2o3_kernel_ir::MAX_CANONICAL_KIR_TRANSITION_RECEIPT_BYTES_V1
    {
        return Err(Error::Limit);
    }
    Ok(total)
}
fn parse_fields(wire: &[u8]) -> Result<[&[u8]; 27], Error> {
    if wire.len() < HEADER {
        return Err(Error::Length);
    }
    let mut c = rows::Reader {
        bytes: wire,
        pos: 0,
    };
    if c.take(8)? != REFINED_FORWARDING_HISTORY_MAGIC_V1
        || c.take(4)? != [1, 0, 1, 0]
        || c.word()? != HEADER as u32
        || c.size()? != wire.len()
        || c.take(2)? != [27, 0]
    {
        return Err(Error::Header);
    }
    if c.take(6)? != [0; 6] {
        return Err(Error::Reserved);
    }
    let mut result = [&[][..]; 27];
    let mut at = HEADER;
    for field in &mut result {
        let end = add(at, c.size()?)?;
        *field = wire.get(at..end).ok_or(Error::Length)?;
        at = end;
    }
    if at != wire.len() {
        return Err(Error::Length);
    }
    lengths(&result)?;
    Ok(result)
}

fn check_fields<'w>(
    wire: &'w [u8],
    fields: [&'w [u8]; 27],
    meter: &mut Meter<'_, '_>,
) -> Result<InertRefinedForwardingHistoryRefV1<'w>, Error> {
    // Prepay both fixed joins and all new row/tag scans before payload access.
    let scan = [14, 18, 19, 20, 21, 22, 23, 24, 25, 26]
        .into_iter()
        .try_fold(256, |n, i| add(n, fields[i].len()))?;
    meter.work(scan)?;
    if fields[18].len() < POLICY7_EXECUTION_HEADER_BYTES_V1
        || fields[18].get(16..272) != Some(fields[15])
    {
        return Err(Error::Field(18));
    }
    let p7_body = fields[18].len() - POLICY7_EXECUTION_HEADER_BYTES_V1;
    if p7_body % POLICY7_EXECUTION_ROW_BYTES_V1 != 0 {
        return Err(Error::Rows);
    }
    let mut count = p7_body / POLICY7_EXECUTION_ROW_BYTES_V1;
    for n in [
        rows::scan::<Load>(fields[14])?,
        rows::scan::<Site>(fields[20])?,
        rows::scan::<Promotion>(fields[21])?,
        rows::scan::<Preheader>(fields[22])?,
        rows::scan::<Licm>(fields[23])?,
        rows::scan::<Refinement>(fields[24])?,
        rows::scan::<Forwarding>(fields[25])?,
    ] {
        count = add(count, n)?;
    }
    let mut tail = rows::Reader {
        bytes: fields[19],
        pos: 0,
    };
    if tail.word()? as usize != PASS.len() || tail.take(PASS.len())? != PASS.as_bytes() {
        return Err(Error::Field(19));
    }
    tail.take(80)?;
    let mut counts = [0; 9];
    for n in &mut counts {
        *n = tail.word()?;
        count = add(count, *n as usize)?;
    }
    if tail.size()?
        != fields[19]
            .len()
            .checked_sub(TAIL_HEADER)
            .ok_or(Error::Length)?
    {
        return Err(Error::Field(19));
    }
    if count > MAX_REFINED_FORWARDING_HISTORY_ROWS_V1 {
        return Err(Error::Limit);
    }
    let storage = {
        let view = meter.derive(|b| {
            read_canonical_kir_occurrence_row_bytes_v1(&fields[19][TAIL_HEADER..], counts, b)
                .map_err(Error::Tail)
        })?;
        let storage = view.storage().retained_storage();
        meter.reserve(storage)?;
        storage
    };
    let limits = rows::read_limits(fields[26])?;
    meter.release(storage)?;
    Ok(InertRefinedForwardingHistoryRefV1 {
        wire,
        fields,
        limits,
        tail_counts: counts,
        row_count: count,
    })
}

/// Validates the complete fixed framing and typed row grammar without graph
/// admission or semantic evidence. Whole input backing is caller-owned/prepaid
/// once; the returned fixed view header is UNRESERVED. New scratch is paid on
/// the original cumulative Budget and dropped before restoring the full floor.
/// A configured storage limit above 256 MiB is rejected, never clamped/reset.
pub fn read_refined_forwarding_history_v1<'w>(
    wire: &'w [u8],
    budget: &mut Budget<'_>,
) -> Result<InertRefinedForwardingHistoryRefV1<'w>, Error> {
    configured(budget)?;
    if wire.len() > MAX_REFINED_FORWARDING_HISTORY_BYTES_V1 {
        return Err(Error::Limit);
    }
    if budget.storage() < wire.len() {
        return Err(Resource::Accounting.into());
    }
    resources::scoped(budget, |meter| {
        meter.reserve(
            size_of::<InertRefinedForwardingHistoryRefV1<'_>>() + size_of::<rows::Reader<'_>>(),
        )?;
        meter.work(HEADER)?;
        let fields = parse_fields(wire)?;
        check_fields(wire, fields, meter)
    })
}

/// Encodes all complete typed inputs, without proving their relations. Inconsistent
/// separately supplied P7 rows or J/K claims are refused, never silently replaced.
/// All twelve graph byte fields remain physically present. P8 uses the existing
/// neutral row codec, not the historical P6 nine-slice association. The returned
/// Vec/header receipt is UNRESERVED; old/new backing coexists on the same ledger.
pub fn encode_refined_forwarding_history_v1(
    inputs: Inputs<'_>,
    budget: &mut Budget<'_>,
) -> Result<InertRefinedForwardingHistoryBytesV1, Error> {
    configured(budget)?;
    resources::scoped(budget, |meter| {
        // Fixed tables/headers and temporary byte-array backing are all new.
        meter.reserve(
            size_of::<InertRefinedForwardingHistoryBytesV1>()
                + size_of::<[&[u8]; 27]>()
                + size_of::<[usize; 27]>()
                + 136
                + size_of::<[&fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12; 12]>()
                + size_of::<[usize; 9]>()
                + 2 * size_of::<[usize; 7]>()
                + size_of::<rows::Writer<'_>>()
                + size_of::<InertRefinedForwardingHistoryRefV1<'_>>(),
        )?;
        meter.work(256)?;
        let p8 = inputs.prefix;
        let p7 = p8.prefix;
        let p6 = p7.prefix;
        let p5 = p6.prefix;
        let raw_graphs = [
            p5.input,
            p5.intermediate,
            p5.stored,
            p5.output,
            p6.output,
            p7.output,
            p8.output,
            inputs.promoted,
            inputs.preheaders,
            inputs.licm,
            inputs.refined,
            inputs.output,
        ];
        let graph_bytes = raw_graphs
            .into_iter()
            .try_fold(0, |n, g| add(n, g.canonical().canonical_bytes().len()))?;
        if graph_bytes > MAX_REFINED_FORWARDING_HISTORY_GRAPH_BYTES_V1 {
            return Err(Error::Limit);
        }
        let p7_len = p7.continuation.execution_record.len();
        if p7_len < POLICY7_EXECUTION_HEADER_BYTES_V1
            || p7_len > MAX_REFINED_FORWARDING_HISTORY_ROW_BYTES_V1
        {
            return Err(Error::Limit);
        }
        let c = p8.continuation.occurrences;
        let counts = [
            c.functions.len(),
            c.blocks.len(),
            c.segments.len(),
            c.operations.len(),
            c.definitions.len(),
            c.definition_outputs.len(),
            c.uses.len(),
            c.edges.len(),
            c.edge_arguments.len(),
        ];
        let typed_counts = [
            p5.load_rows.len(),
            inputs.selected_allocations.len(),
            inputs.promotion_origins.len(),
            inputs.preheader_rows.len(),
            inputs.licm_origins.len(),
            inputs.refinement_origins.len(),
            inputs.forwarding_origins.len(),
        ];
        let count = counts.into_iter().chain(typed_counts).try_fold(
            (p7_len - POLICY7_EXECUTION_HEADER_BYTES_V1) / POLICY7_EXECUTION_ROW_BYTES_V1,
            add,
        )?;
        if count > MAX_REFINED_FORWARDING_HISTORY_ROWS_V1 {
            return Err(Error::Limit);
        }
        // Bound exact output extents before any constituent decoder allocation.
        meter.work(typed_counts.into_iter().try_fold(0, add)?)?;
        let row_lens = [
            rows::extent(p5.load_rows)?,
            rows::extent(inputs.selected_allocations)?,
            rows::extent(inputs.promotion_origins)?,
            rows::extent(inputs.preheader_rows)?,
            rows::extent(inputs.licm_origins)?,
            rows::extent(inputs.refinement_origins)?,
            rows::extent(inputs.forwarding_origins)?,
        ];
        let tail_len = counts
            .into_iter()
            .zip([8usize, 16, 24, 36, 28, 24, 40, 24, 32])
            .try_fold(TAIL_HEADER, |n, (count, width)| {
                add(n, count.checked_mul(width).ok_or(Resource::Arithmetic)?)
            })?;
        let row_bytes = row_lens.into_iter().try_fold(add(p7_len, tail_len)?, add)?;
        if row_bytes > MAX_REFINED_FORWARDING_HISTORY_ROW_BYTES_V1 {
            return Err(Error::Limit);
        }
        let record_bytes = [
            p5.policy4_wire.len(),
            p5.policy5_record.len(),
            p6.continuation.composition_record.len(),
            p6.continuation.integer_record.len(),
            p6.continuation.transition_wire.len(),
            136,
        ]
        .into_iter()
        .try_fold(0, add)?;
        if p5.policy4_wire.len() > crate::MAX_CANONICAL_POLICY4_EXECUTION_RECEIPT_BYTES_V1
            || p6.continuation.transition_wire.len()
                > fe2o3_kernel_ir::MAX_CANONICAL_KIR_TRANSITION_RECEIPT_BYTES_V1
            || p5.policy5_record.len() != 200
            || p6.continuation.composition_record.len() != 256
            || p6.continuation.integer_record.len() != 416
        {
            return Err(Error::Length);
        }
        if add(add(add(HEADER, graph_bytes)?, row_bytes)?, record_bytes)?
            > MAX_REFINED_FORWARDING_HISTORY_BYTES_V1
        {
            return Err(Error::Limit);
        }
        if p8.continuation.pass_name != PASS
            || !p8
                .continuation
                .input
                .matches_verified(p7.output.canonical().identity())
            || !p8
                .continuation
                .output
                .matches_verified(p8.output.canonical().identity())
        {
            return Err(Error::TailIdentity);
        }
        let p7_rows = meter.derive(|b| {
            decode_canonical_policy7_rows_v1(p7.continuation.execution_record, b)
                .map_err(Error::Policy7)
        })?;
        let p7_storage = p7_rows.storage().retained_storage();
        meter.reserve(p7_storage)?;
        let claims = p7_rows.claims();
        meter.work(p7.continuation.execution_record.len())?;
        if claims.deletion_rows != p7.continuation.deletion_rows
            || claims.retained_operations != p7.continuation.retained_operations
        {
            return Err(Error::Policy7Rows);
        }
        let (tail, tail_storage) = meter.derive(|b| {
            encode_canonical_kir_occurrence_row_bytes_v1(p8.continuation.occurrences, b)
                .map_err(Error::Tail)
        })?;
        meter.reserve(tail_storage.retained_storage())?;
        let mut limit_bytes = [0; 136];
        rows::write_limits(inputs.limits, &mut limit_bytes)?;
        let fields: [&[u8]; 27] = [
            p5.input.canonical().canonical_bytes(),
            p5.intermediate.canonical().canonical_bytes(),
            p5.stored.canonical().canonical_bytes(),
            p5.output.canonical().canonical_bytes(),
            p6.output.canonical().canonical_bytes(),
            p7.output.canonical().canonical_bytes(),
            p8.output.canonical().canonical_bytes(),
            inputs.promoted.canonical().canonical_bytes(),
            inputs.preheaders.canonical().canonical_bytes(),
            inputs.licm.canonical().canonical_bytes(),
            inputs.refined.canonical().canonical_bytes(),
            inputs.output.canonical().canonical_bytes(),
            p5.policy4_wire,
            p5.policy5_record,
            &[],
            p6.continuation.composition_record,
            p6.continuation.integer_record,
            p6.continuation.transition_wire,
            p7.continuation.execution_record,
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &limit_bytes,
        ];
        let mut lens = fields.map(<[u8]>::len);
        lens[14] = row_lens[0];
        lens[19] = add(TAIL_HEADER, tail.canonical_row_bytes().len())?;
        lens[20..26].copy_from_slice(&row_lens[1..]);
        let total = lens.into_iter().try_fold(HEADER, add)?;
        if total > MAX_REFINED_FORWARDING_HISTORY_BYTES_V1 {
            return Err(Error::Limit);
        }
        let (mut bytes, capacity) = meter.table::<u8>(total)?;
        meter.work(total.checked_mul(2).ok_or(Resource::Arithmetic)?)?;
        bytes.resize(total, 0);
        let mut w = rows::Writer {
            bytes: &mut bytes[..HEADER],
            pos: 0,
        };
        w.put(&REFINED_FORWARDING_HISTORY_MAGIC_V1)?;
        w.put(&[1, 0, 1, 0])?;
        w.word(HEADER as u32)?;
        w.wide(total as u64)?;
        w.put(&[27, 0, 0, 0, 0, 0, 0, 0])?;
        for n in lens {
            w.wide(n as u64)?;
        }
        let mut at = HEADER;
        for (i, n) in lens.into_iter().enumerate() {
            let out = &mut bytes[at..at + n];
            match i {
                14 => rows::encode(p5.load_rows, out)?,
                19 => {
                    let mut w = rows::Writer { bytes: out, pos: 0 };
                    w.word(PASS.len() as u32)?;
                    w.put(PASS.as_bytes())?;
                    for id in [p8.continuation.input, p8.continuation.output] {
                        w.put(id.digest())?;
                        w.wide(id.canonical_length())?;
                    }
                    for n in tail.counts() {
                        w.word(n)?;
                    }
                    w.wide(tail.canonical_row_bytes().len() as u64)?;
                    w.put(tail.canonical_row_bytes())?;
                }
                20 => rows::encode(inputs.selected_allocations, out)?,
                21 => rows::encode(inputs.promotion_origins, out)?,
                22 => rows::encode(inputs.preheader_rows, out)?,
                23 => rows::encode(inputs.licm_origins, out)?,
                24 => rows::encode(inputs.refinement_origins, out)?,
                25 => rows::encode(inputs.forwarding_origins, out)?,
                _ => out.copy_from_slice(fields[i]),
            }
            at += n;
        }
        // Reuse exactly the reader's cap/tag/partition contract on the final bytes.
        {
            let _frame = check_fields(&bytes, parse_fields(&bytes)?, meter)?;
        }
        drop(tail);
        meter.release(tail_storage.retained_storage())?;
        drop(p7_rows);
        meter.release(p7_storage)?;
        Ok(InertRefinedForwardingHistoryBytesV1 {
            bytes,
            storage: RefinedForwardingHistoryWireStorageV1(add(
                size_of::<InertRefinedForwardingHistoryBytesV1>(),
                capacity,
            )?),
        })
    })
}

#[cfg(test)]
#[path = "refined_forwarding_history_wire_v1_tests.rs"]
pub(crate) mod tests;
