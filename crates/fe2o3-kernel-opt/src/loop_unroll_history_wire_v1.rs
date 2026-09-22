//! Inert complete F history followed by a separately named U graph and six rosters.
use crate::{
    InertRefinedForwardingHistoryRefV1 as Prefix,
    RefinedForwardingHistoryWireErrorV1 as PrefixError, loop_unroll_history_rows_v1 as rows,
    private_cell_promotion_resources_v1 as resources, read_refined_forwarding_history_v1,
};
use fe2o3_kernel_analysis::{
    CanonicalKirLoopUnrollLimitsV1 as Limits, CanonicalKirLoopUnrollOriginsV1 as Origins,
    CanonicalKirLoopUnrollSelectionV1 as Selection,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayAdmissionErrorV12 as Admission,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirEdgeArgumentCoordinateV1 as Argument, CanonicalKirEdgeCoordinateV1 as Edge,
    CanonicalKirOperationCoordinateV1 as Site, VerifiedCanonicalKernelIrModuleV12 as Graph,
};
use std::{fmt, mem::size_of};

pub const LOOP_UNROLL_HISTORY_MAGIC_V1: [u8; 8] = *b"F2LUH1\0\0";
pub const MAX_LOOP_UNROLL_HISTORY_BYTES_V1: usize = 16 * 1024 * 1024;
pub const MAX_LOOP_UNROLL_HISTORY_GRAPH_BYTES_V1: usize = 12 * 1024 * 1024;
pub const MAX_LOOP_UNROLL_HISTORY_ROW_BYTES_V1: usize = 4 * 1024 * 1024;
pub const MAX_LOOP_UNROLL_HISTORY_ROWS_V1: usize = 262_144;
pub const MAX_LOOP_UNROLL_HISTORY_STORAGE_V1: usize = 256 * 1024 * 1024;
pub(super) const HEADER: usize = 104;
pub(super) const SCRATCH: usize = size_of::<[&[u8]; 9]>()
    + size_of::<[usize; 9]>()
    + size_of::<[usize; 6]>()
    + size_of::<Origins<'_>>()
    + size_of::<Limits>()
    + size_of::<Option<Selection>>()
    + rows::SCRATCH;

#[derive(Debug)]
pub enum LoopUnrollHistoryWireErrorV1 {
    Resource(Resource),
    Length,
    Header,
    Reserved,
    Limit,
    Field(usize),
    Rows { family: u8 },
    Tag { family: u8 },
    Settings,
    Prefix(PrefixError),
    Admission(Admission),
    Panicked,
}
pub(super) type Error = LoopUnrollHistoryWireErrorV1;
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
        write!(f, "inert loop-unroll history: {self:?}")
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Prefix(error) => Some(error),
            Self::Admission(error) => Some(error),
            Self::Length
            | Self::Header
            | Self::Reserved
            | Self::Limit
            | Self::Field(_)
            | Self::Rows { .. }
            | Self::Tag { .. }
            | Self::Settings
            | Self::Panicked => None,
        }
    }
}

/// Added owned/view storage only, returned unreserved. Wire backing stays caller-paid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoopUnrollHistoryWireStorageV1(pub(super) usize);
impl LoopUnrollHistoryWireStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Framing and closed syntax are not semantic replay or execution authentication.
pub struct InertLoopUnrollHistoryRefV1<'w> {
    pub(super) wire: &'w [u8],
    pub(super) fields: [&'w [u8]; 9],
    pub(super) prefix: Prefix<'w>,
    pub(super) limits: Limits,
    pub(super) selection: Option<Selection>,
    pub(super) row_count: usize,
}
impl<'w> InertLoopUnrollHistoryRefV1<'w> {
    pub const fn canonical_bytes(&self) -> &'w [u8] {
        self.wire
    }
    pub const fn prefix(&self) -> &Prefix<'w> {
        &self.prefix
    }
    pub fn output_bytes(&self) -> &'w [u8] {
        self.fields[1]
    }
    pub const fn limits(&self) -> Limits {
        self.limits
    }
    pub const fn selection(&self) -> Option<Selection> {
        self.selection
    }
    pub const fn row_count(&self) -> usize {
        self.row_count
    }
    pub const fn storage(&self) -> LoopUnrollHistoryWireStorageV1 {
        LoopUnrollHistoryWireStorageV1(size_of::<Self>())
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}
pub struct InertLoopUnrollHistoryBytesV1 {
    bytes: Vec<u8>,
    storage: LoopUnrollHistoryWireStorageV1,
}
impl InertLoopUnrollHistoryBytesV1 {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub const fn storage(&self) -> LoopUnrollHistoryWireStorageV1 {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}
pub struct LoopUnrollHistoryInputsV1<'a, 'w> {
    pub prefix: &'a Prefix<'w>,
    pub output: &'a Graph,
    pub origins: Origins<'a>,
    pub limits: Limits,
}
pub(super) fn add(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_add(b).ok_or_else(|| Resource::Arithmetic.into())
}
pub(super) fn configured(budget: &Budget<'_>) -> Result<(), Error> {
    if budget.storage_limit() > MAX_LOOP_UNROLL_HISTORY_STORAGE_V1 {
        Err(Error::Limit)
    } else {
        Ok(())
    }
}
fn field_lengths(fields: &[&[u8]; 9]) -> Result<usize, Error> {
    fields.iter().enumerate().try_fold(HEADER, |n, (i, bytes)| {
        if bytes.is_empty() {
            return Err(Error::Field(i));
        }
        add(n, bytes.len())
    })
}
fn parse(wire: &[u8]) -> Result<[&[u8]; 9], Error> {
    if wire.len() < HEADER {
        return Err(Error::Length);
    }
    if wire[..8] != LOOP_UNROLL_HISTORY_MAGIC_V1
        || wire[8..12] != [1, 0, 1, 0]
        || wire[12..16] != (HEADER as u32).to_le_bytes()
        || wire[24..26] != [9, 0]
    {
        return Err(Error::Header);
    }
    if wire[26..32] != [0; 6] {
        return Err(Error::Reserved);
    }
    let wide = |at| -> Result<usize, Error> {
        usize::try_from(u64::from_le_bytes(
            wire[at..at + 8].try_into().map_err(|_| Error::Length)?,
        ))
        .map_err(|_| Resource::Arithmetic.into())
    };
    if wide(16)? != wire.len() {
        return Err(Error::Length);
    }
    let mut fields = [&[][..]; 9];
    let mut pos = HEADER;
    for (i, field) in fields.iter_mut().enumerate() {
        let end = add(pos, wide(32 + 8 * i)?)?;
        *field = wire.get(pos..end).ok_or(Error::Length)?;
        pos = end;
    }
    if pos != wire.len() {
        return Err(Error::Length);
    }
    Ok(fields)
}
fn aggregate(
    prefix: &Prefix<'_>,
    output_bytes: usize,
    row_bytes: usize,
    rows: usize,
) -> Result<(), Error> {
    let graph_bytes = prefix.fields[..12]
        .iter()
        .try_fold(output_bytes, |n, b| add(n, b.len()))?;
    let row_bytes = [14, 18, 19, 20, 21, 22, 23, 24, 25]
        .iter()
        .try_fold(row_bytes, |n, i| add(n, prefix.fields[*i].len()))?;
    if graph_bytes > MAX_LOOP_UNROLL_HISTORY_GRAPH_BYTES_V1
        || row_bytes > MAX_LOOP_UNROLL_HISTORY_ROW_BYTES_V1
        || add(prefix.row_count(), rows)? > MAX_LOOP_UNROLL_HISTORY_ROWS_V1
    {
        return Err(Error::Limit);
    }
    Ok(())
}
fn validate_rows(fields: &[&[u8]; 9]) -> Result<(), Error> {
    rows::validate::<Block>(fields[2], 0)?;
    rows::validate::<Definition>(fields[3], 1)?;
    rows::validate::<Site>(fields[4], 2)?;
    rows::validate::<Block>(fields[5], 3)?;
    rows::validate::<Edge>(fields[6], 4)?;
    rows::validate::<Argument>(fields[7], 5)
}

/// Preserves the exact old prefix reader and typed errors. Every new fixed
/// header/cursor and scan is paid before traversal. The returned frame is
/// unreserved; its embedded F frame is already included in its actual header.
pub fn read_loop_unroll_history_v1<'w>(
    wire: &'w [u8],
    budget: &mut Budget<'_>,
) -> Result<InertLoopUnrollHistoryRefV1<'w>, Error> {
    configured(budget)?;
    if wire.len() > MAX_LOOP_UNROLL_HISTORY_BYTES_V1 {
        return Err(Error::Limit);
    }
    if wire.len() < HEADER {
        return Err(Error::Length);
    }
    if budget.storage() < wire.len() {
        return Err(Resource::Accounting.into());
    }
    resources::scoped(budget, |meter| {
        meter.reserve(add(size_of::<InertLoopUnrollHistoryRefV1<'_>>(), SCRATCH)?)?;
        meter.work(HEADER)?;
        let fields = parse(wire)?;
        if field_lengths(&fields)? != wire.len() {
            return Err(Error::Length);
        }
        let prefix = meter
            .derive(|b| read_refined_forwarding_history_v1(fields[0], b).map_err(Error::Prefix))?;
        meter.work(152)?;
        let (limits, selection) = rows::settings(fields[8])?;
        let mut row_count = 0;
        let mut row_bytes = 0;
        for family in 0..6 {
            row_count = add(row_count, rows::header(fields[2 + family], family as u8)?)?;
            row_bytes = add(row_bytes, fields[2 + family].len())?;
        }
        aggregate(&prefix, fields[1].len(), row_bytes, row_count)?;
        meter.work(row_bytes)?;
        validate_rows(&fields)?;
        meter.work(1)?;
        let row_count = add(row_count, prefix.row_count())?;
        Ok(InertLoopUnrollHistoryRefV1 {
            wire,
            fields,
            prefix,
            limits,
            selection,
            row_count,
        })
    })
}

/// Serializes inert claims without selecting, repairing or checking a pass.
/// Existing prefix bytes/errors remain exact. Actual output capacity and all
/// fixed working extents coexist under the original ledger. Result is unreserved.
pub fn encode_loop_unroll_history_v1(
    inputs: LoopUnrollHistoryInputsV1<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<InertLoopUnrollHistoryBytesV1, Error> {
    configured(budget)?;
    if budget.storage()
        < add(
            inputs.prefix.canonical_bytes().len(),
            inputs.prefix.storage().retained_storage(),
        )?
    {
        return Err(Resource::Accounting.into());
    }
    resources::scoped(budget, |meter| {
        meter.reserve(add(size_of::<InertLoopUnrollHistoryBytesV1>(), SCRATCH)?)?;
        let origins = inputs.origins;
        let count = [
            origins.blocks.len(),
            origins.definitions.len(),
            origins.operations.len(),
            origins.terminators.len(),
            origins.edges.len(),
            origins.arguments.len(),
        ]
        .into_iter()
        .try_fold(0, add)?;
        meter.work(add(152, count)?)?;
        let lengths = [
            inputs.prefix.canonical_bytes().len(),
            inputs.output.canonical().canonical_bytes().len(),
            rows::extent(origins.blocks)?,
            rows::extent(origins.definitions)?,
            rows::extent(origins.operations)?,
            rows::extent(origins.terminators)?,
            rows::extent(origins.edges)?,
            rows::extent(origins.arguments)?,
            104,
        ];
        let total = lengths.iter().try_fold(HEADER, |n, v| add(n, *v))?;
        if total > MAX_LOOP_UNROLL_HISTORY_BYTES_V1 {
            return Err(Error::Limit);
        }
        aggregate(
            inputs.prefix,
            lengths[1],
            lengths[2..8].iter().try_fold(0, |n, v| add(n, *v))?,
            count,
        )?;
        let mut settings = [0; 104];
        rows::write_settings(inputs.limits, origins.selection, &mut settings)?;
        let (mut bytes, capacity) = meter.table::<u8>(total)?;
        meter.work(total)?;
        bytes.resize(total, 0);
        meter.work(total)?;
        bytes[..8].copy_from_slice(&LOOP_UNROLL_HISTORY_MAGIC_V1);
        bytes[8..12].copy_from_slice(&[1, 0, 1, 0]);
        bytes[12..16].copy_from_slice(&(HEADER as u32).to_le_bytes());
        bytes[16..24].copy_from_slice(
            &u64::try_from(total)
                .map_err(|_| Resource::Arithmetic)?
                .to_le_bytes(),
        );
        bytes[24..26].copy_from_slice(&[9, 0]);
        for (i, n) in lengths.iter().enumerate() {
            bytes[32 + i * 8..40 + i * 8].copy_from_slice(
                &u64::try_from(*n)
                    .map_err(|_| Resource::Arithmetic)?
                    .to_le_bytes(),
            );
        }
        let mut pos = HEADER;
        for (i, n) in lengths.iter().enumerate() {
            let end = add(pos, *n)?;
            let dest = &mut bytes[pos..end];
            match i {
                0 => dest.copy_from_slice(inputs.prefix.canonical_bytes()),
                1 => dest.copy_from_slice(inputs.output.canonical().canonical_bytes()),
                2 => rows::encode(origins.blocks, dest, 0)?,
                3 => rows::encode(origins.definitions, dest, 1)?,
                4 => rows::encode(origins.operations, dest, 2)?,
                5 => rows::encode(origins.terminators, dest, 3)?,
                6 => rows::encode(origins.edges, dest, 4)?,
                7 => rows::encode(origins.arguments, dest, 5)?,
                8 => dest.copy_from_slice(&settings),
                _ => return Err(Error::Header),
            }
            pos = end;
        }
        let vs = {
            let view = meter.derive(|b| read_loop_unroll_history_v1(&bytes, b))?;
            let vs = view.storage().retained_storage();
            meter.reserve(vs)?;
            meter.work(1)?;
            vs
        };
        meter.release(vs)?;
        Ok(InertLoopUnrollHistoryBytesV1 {
            bytes,
            storage: LoopUnrollHistoryWireStorageV1(add(
                size_of::<InertLoopUnrollHistoryBytesV1>(),
                capacity,
            )?),
        })
    })
}

#[cfg(test)]
#[path = "loop_unroll_history_wire_v1_tests.rs"]
mod tests;
