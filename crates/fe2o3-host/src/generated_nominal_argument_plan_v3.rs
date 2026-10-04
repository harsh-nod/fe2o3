//! Borrowed, inert V3 packing. Zero pointer placeholders are not runtime kernargs.
use super::{GeneratedDeviceScalarV1, descriptor_scalar_to_rust_layout};
use crate::{GeneratedKfdReadSlice, GeneratedKfdReadWriteSlice, GeneratedKfdWriteSlice};
use fe2o3_artifacts::{
    PointerWidth, RUST_NOMINAL_LAYOUT_DOMAIN_V3, RUST_NOMINAL_TYPE_DOMAIN_V3,
    RustNominalLayoutErrorV3, RustNominalScalarEvidenceV3, RustNominalScalarKindV3,
    RustScalarElementTypeV1, TypeIdentity,
};
use fe2o3_kernel_descriptor::{
    AccessMode, AliasSemantics, DESCRIPTOR_QUERY_STORAGE_V3, DESCRIPTOR_TABLE_VIEW_STORAGE_V3,
    DescriptorWireErrorV3, DeviceDescriptorTableV3, DeviceLayoutDescriptorV1, DeviceLayoutIdentity,
    KernelId, MAX_ARGUMENTS_PER_KERNEL, MAX_KERNARG_SEGMENT_BYTES,
    MAX_PHYSICAL_COMPONENTS_PER_KERNEL, OwnershipSemantics, PhysicalAbiComponentKind,
    PhysicalComponentV3, RustTypeIdentity, ScalarTypeV1, SourceTypeDescriptorV3,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[derive(Debug)]
pub enum GeneratedNominalPackErrorV3 {
    Resource(Resource),
    Descriptor(DescriptorWireErrorV3<Resource>),
    Nominal(RustNominalLayoutErrorV3),
    HostWidth { actual: usize },
    Argument { index: usize, reason: &'static str },
    Count { expected: usize, actual: usize },
    OutputLength { expected: usize, actual: usize },
    Panicked,
}
impl fmt::Display for GeneratedNominalPackErrorV3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for GeneratedNominalPackErrorV3 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Descriptor(e) => Some(e),
            Self::Nominal(e) => Some(e),
            _ => None,
        }
    }
}
impl From<Resource> for GeneratedNominalPackErrorV3 {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<DescriptorWireErrorV3<Resource>> for GeneratedNominalPackErrorV3 {
    fn from(e: DescriptorWireErrorV3<Resource>) -> Self {
        Self::Descriptor(e)
    }
}
type Result<T> = std::result::Result<T, GeneratedNominalPackErrorV3>;
fn argument(index: usize, reason: &'static str) -> GeneratedNominalPackErrorV3 {
    GeneratedNominalPackErrorV3::Argument { index, reason }
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or(Resource::Arithmetic.into())
}
fn host_width(width: usize) -> Result<()> {
    if width != 8 {
        return Err(GeneratedNominalPackErrorV3::HostWidth { actual: width });
    }
    Ok(())
}

// Each scope returns only an unreserved addition. No inherited credit is released.
fn scoped<'work, T>(
    minimum: usize,
    budget: &mut Budget<'work>,
    run: impl FnOnce(&mut Budget<'work>) -> Result<T>,
) -> Result<T> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    if floor < minimum {
        return Err(Resource::Accounting.into());
    }
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(value) => value,
        Err(payload) => {
            payloads[0] = Some(payload);
            Err(GeneratedNominalPackErrorV3::Panicked)
        }
    };
    let cleanup = if budget.work_ledger_identity_v1() != ledger || budget.storage() < floor {
        Err(Resource::Accounting)
    } else {
        budget.release_storage(budget.storage() - floor)
    };
    if let Err(error) = cleanup {
        let rejected = std::mem::replace(&mut result, Err(error.into()));
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(rejected))) {
            payloads[1] = Some(payload);
        }
    }
    // A caught payload may itself panic on drop; restore valid credit first.
    drop(payloads);
    result
}

/// Unreserved logical header addition, not authentication or a memory allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedNominalStorageV3(usize);
impl GeneratedNominalStorageV3 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

const EMPTY: PhysicalComponentV3 = PhysicalComponentV3 {
    kind: PhysicalAbiComponentKind::GlobalPointer,
    offset: 0,
    size: 8,
    alignment: 8,
    access: AccessMode::ReadOnly,
    alias: AliasSemantics::SharedReadOnly,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Argument {
    source: RustTypeIdentity,
    layout: DeviceLayoutIdentity,
    kind: SourceTypeDescriptorV3,
    ownership: OwnershipSemantics,
    access: AccessMode,
    alias: AliasSemantics,
    count: usize,
    components: [PhysicalComponentV3; 2],
}

// QUERY covers callee temporaries, not the caller's simultaneously retained rows.
const PLAN_SCRATCH: usize = DESCRIPTOR_QUERY_STORAGE_V3
    + size_of::<fe2o3_kernel_descriptor::KernelDescriptorRefV3<'static, 'static>>()
    + size_of::<fe2o3_kernel_descriptor::ArgumentCursorV3<'static, 'static>>()
    + size_of::<fe2o3_kernel_descriptor::LogicalArgumentRefV3<'static, 'static>>()
    + size_of::<fe2o3_kernel_descriptor::SourceTypeRecordV3>()
    + size_of::<fe2o3_kernel_descriptor::DeviceLayoutRecordV1>()
    + size_of::<DeviceLayoutDescriptorV1>()
    + size_of::<[PhysicalComponentV3; 2]>()
    + size_of::<PhysicalComponentV3>()
    + size_of::<Argument>();

/// A plan borrows one actual checked table and caches only fixed-size argument rows.
/// It does not authenticate that table or enable a runtime/worker launch.
///
/// The caller prepays the actual wire backing and VIEW before construction, then
/// reserves each returned addition before further controlled use. Keep that credit
/// until the owner is dropped. All routines use the caller's cumulative Budget;
/// ledger identities are checked within a call, not used as permanent seals.
/// Each derived owner retains the historical cumulative storage baseline,
/// including existing sibling owners. Keep those siblings, backing and credits
/// live while using it. Retiring a sibling does not rebase later owners; rebuild
/// the affected stages afterward. Input/output backing is separately prepaid.
/// Fixed scratch/header extents are logical payload, not a stack/RSS guarantee.
/// Plan scratch separately covers the retained kernel/argument views and cursor,
/// source/layout records, expected layout, component array/value and cached row,
/// in addition to the descriptor query's own temporary extent.
/// No V1 owned table, legacy plan, buffer registration or address fixup is used.
///
/// ```compile_fail
/// use fe2o3_host::GeneratedNominalArgumentPlanV3;
/// fn copy(plan: GeneratedNominalArgumentPlanV3<'_, '_>) {
///     let first = plan;
///     let second = plan;
///     drop((first, second));
/// }
/// ```
///
/// The actual checked table remains borrowed while the plan is used.
///
/// ```compile_fail,E0505
/// use fe2o3_host::GeneratedNominalArgumentPlanV3;
/// use fe2o3_kernel_descriptor::{DeviceDescriptorTableV3, KernelId};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn release_table(table: DeviceDescriptorTableV3<'_>, kernel: KernelId, budget: &mut Budget<'_>) {
///     let (plan, _) = GeneratedNominalArgumentPlanV3::new(&table, kernel, budget).unwrap();
///     drop(table);
///     std::hint::black_box(plan.kernel_id());
/// }
/// ```
pub struct GeneratedNominalArgumentPlanV3<'view, 'wire> {
    table: &'view DeviceDescriptorTableV3<'wire>,
    kernel: KernelId,
    count: usize,
    output_len: usize,
    alignment: usize,
    arguments: [Option<Argument>; MAX_ARGUMENTS_PER_KERNEL],
    floor: usize,
}

impl<'view, 'wire> GeneratedNominalArgumentPlanV3<'view, 'wire> {
    pub fn new(
        table: &'view DeviceDescriptorTableV3<'wire>,
        kernel: KernelId,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, GeneratedNominalStorageV3)> {
        let minimum = add(
            DESCRIPTOR_TABLE_VIEW_STORAGE_V3,
            table.canonical_bytes().len(),
        )?;
        let floor = budget.storage();
        scoped(minimum, budget, |budget| {
            budget.charge_work(1)?;
            host_width(size_of::<usize>())?;
            host_width(size_of::<isize>())?;
            budget.reserve_storage(NOMINAL_ARGUMENT_PLAN_STORAGE_V3)?;
            budget.reserve_storage(PLAN_SCRATCH)?;
            let row = table.find_kernel(kernel, &mut |n| budget.charge_work(n))?;
            let count = row.argument_count();
            let output_len = row.abi_layout().kernarg_segment_size() as usize;
            let explicit = row.abi_layout().explicit_argument_size() as usize;
            let alignment = row.abi_layout().kernarg_segment_alignment() as usize;
            if count > MAX_ARGUMENTS_PER_KERNEL || output_len > MAX_KERNARG_SEGMENT_BYTES as usize {
                return Err(argument(count, "descriptor limits"));
            }
            budget.charge_work(MAX_ARGUMENTS_PER_KERNEL)?;
            let mut arguments = [None; MAX_ARGUMENTS_PER_KERNEL];
            let mut cursor = row.arguments();
            let mut previous_end = 0;
            for (index, slot) in arguments[..count].iter_mut().enumerate() {
                let row = cursor
                    .next(&mut |n| budget.charge_work(n))?
                    .ok_or_else(|| argument(index, "missing descriptor argument"))?;
                budget.charge_work(4)?;
                if usize::from(row.source_index()) != index {
                    return Err(argument(index, "source ordinal"));
                }
                let source =
                    table.source_type(row.source_type(), &mut |n| budget.charge_work(n))?;
                let layout =
                    table.device_layout(row.device_layout(), &mut |n| budget.charge_work(n))?;
                let kind = source.descriptor();
                let expected = match kind {
                    SourceTypeDescriptorV3::Scalar(s) => DeviceLayoutDescriptorV1::scalar(s),
                    SourceTypeDescriptorV3::Usize => {
                        DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::U64)
                    }
                    SourceTypeDescriptorV3::Isize => {
                        DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::I64)
                    }
                    SourceTypeDescriptorV3::SharedSlice(s) => {
                        DeviceLayoutDescriptorV1::shared_slice(s)
                    }
                    SourceTypeDescriptorV3::DisjointSlice(s) => {
                        DeviceLayoutDescriptorV1::disjoint_slice(s)
                    }
                    SourceTypeDescriptorV3::GlobalMutPointer(_) => {
                        return Err(argument(index, "raw pointer not supported"));
                    }
                };
                budget.charge_work(16)?;
                if layout.descriptor() != &expected {
                    return Err(argument(index, "source/layout identity join"));
                }
                let n = row.component_count();
                if n == 0 || n > 2 {
                    return Err(argument(index, "component count"));
                }
                let mut components = [EMPTY; 2];
                for (component, output) in components[..n].iter_mut().enumerate() {
                    let p = row.component(component, &mut |n| budget.charge_work(n))?;
                    budget.charge_work(16)?;
                    let start = p.offset as usize;
                    let end = add(start, usize::from(p.size))?;
                    if p.size == 0
                        || p.size > 8
                        || p.alignment == 0
                        || !p.alignment.is_power_of_two()
                        || !start.is_multiple_of(usize::from(p.alignment))
                        || usize::from(p.alignment) > alignment
                        || start < previous_end
                        || end > explicit
                    {
                        return Err(argument(index, "component range/alignment"));
                    }
                    previous_end = end;
                    *output = p;
                }
                *slot = Some(Argument {
                    source: row.source_type(),
                    layout: row.device_layout(),
                    kind,
                    ownership: row.ownership(),
                    access: row.access(),
                    alias: row.alias(),
                    count: n,
                    components,
                });
            }
            if cursor.next(&mut |n| budget.charge_work(n))?.is_some() {
                return Err(argument(count, "extra descriptor argument"));
            }
            budget.charge_work(1)?;
            Ok((
                Self {
                    table,
                    kernel,
                    count,
                    output_len,
                    alignment,
                    arguments,
                    floor: add(floor, NOMINAL_ARGUMENT_PLAN_STORAGE_V3)?,
                },
                GeneratedNominalStorageV3(NOMINAL_ARGUMENT_PLAN_STORAGE_V3),
            ))
        })
    }
    pub const fn kernel_id(&self) -> KernelId {
        self.kernel
    }
    pub const fn argument_count(&self) -> usize {
        self.count
    }
    pub const fn output_len(&self) -> usize {
        self.output_len
    }
    pub const fn required_alignment(&self) -> usize {
        self.alignment
    }
    /// Historical cumulative minimum, not a sum of current dependencies.
    pub const fn retained_storage_floor(&self) -> usize {
        self.floor
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    fn row(&self, index: usize) -> Result<Argument> {
        self.arguments
            .get(index)
            .and_then(|row| *row)
            .ok_or_else(|| argument(index, "argument index"))
    }
    fn bind<'plan, B>(
        &'plan self,
        index: usize,
        custody: B,
        request: Request,
        encode: impl FnOnce(&B) -> Result<[Value; 2]>,
        budget: &mut Budget<'_>,
    ) -> Result<(
        GeneratedNominalBindingV3<'plan, 'view, 'wire, B>,
        GeneratedNominalStorageV3,
    )> {
        let floor = budget.storage();
        let header = size_of::<GeneratedNominalBindingV3<'plan, 'view, 'wire, B>>();
        scoped(self.floor, budget, move |budget| {
            budget.charge_work(1)?;
            budget.reserve_storage(header)?;
            budget.reserve_storage(NOMINAL_BIND_SCRATCH_STORAGE_V3)?;
            budget.charge_work(32)?;
            let row = self.row(index)?;
            if !request.matches(row) {
                return Err(argument(index, "exact source kind/ownership/access"));
            }
            let values = encode(&custody)?;
            for (p, value) in row.components[..row.count].iter().zip(values) {
                if usize::from(value.len) != usize::from(p.size) {
                    return Err(argument(index, "encoded width"));
                }
            }
            let nominal = nominal_identity(row.kind, budget)?;
            budget.charge_work(1)?;
            Ok((
                GeneratedNominalBindingV3 {
                    core: BindingCore {
                        plan: self,
                        index,
                        row,
                        values,
                        nominal,
                        floor: add(floor, header)?,
                    },
                    _custody: custody,
                },
                GeneratedNominalStorageV3(header),
            ))
        })
    }

    pub fn bind_usize<'plan>(
        &'plan self,
        index: usize,
        value: usize,
        budget: &mut Budget<'_>,
    ) -> Result<(
        GeneratedNominalBindingV3<'plan, 'view, 'wire, usize>,
        GeneratedNominalStorageV3,
    )> {
        self.bind(
            index,
            value,
            Request::Usize,
            |v| {
                host_width(size_of::<usize>())?;
                let value = u64::try_from(*v).map_err(|_| argument(index, "usize range"))?;
                Ok([
                    Value {
                        bytes: value.to_le_bytes(),
                        len: 8,
                    },
                    Value::ZERO,
                ])
            },
            budget,
        )
    }
    pub fn bind_isize<'plan>(
        &'plan self,
        index: usize,
        value: isize,
        budget: &mut Budget<'_>,
    ) -> Result<(
        GeneratedNominalBindingV3<'plan, 'view, 'wire, isize>,
        GeneratedNominalStorageV3,
    )> {
        self.bind(
            index,
            value,
            Request::Isize,
            |v| {
                host_width(size_of::<isize>())?;
                let value = i64::try_from(*v).map_err(|_| argument(index, "isize range"))?;
                Ok([
                    Value {
                        bytes: value.to_le_bytes(),
                        len: 8,
                    },
                    Value::ZERO,
                ])
            },
            budget,
        )
    }
    pub fn bind_scalar<'plan, T: GeneratedDeviceScalarV1>(
        &'plan self,
        index: usize,
        value: T,
        budget: &mut Budget<'_>,
    ) -> Result<(
        GeneratedNominalBindingV3<'plan, 'view, 'wire, T>,
        GeneratedNominalStorageV3,
    )> {
        self.bind(
            index,
            value,
            Request::Scalar(T::RUST_SCALAR_TYPE),
            |v| {
                let (bytes, len) = v.encode_le_bytes_v1();
                Ok([Value { bytes, len }, Value::ZERO])
            },
            budget,
        )
    }
    pub fn bind_read_slice<'plan, 'allocation, T: GeneratedDeviceScalarV1>(
        &'plan self,
        index: usize,
        value: GeneratedKfdReadSlice<'allocation, T>,
        budget: &mut Budget<'_>,
    ) -> Result<(
        GeneratedNominalBindingV3<'plan, 'view, 'wire, GeneratedKfdReadSlice<'allocation, T>>,
        GeneratedNominalStorageV3,
    )> {
        self.bind(
            index,
            value,
            Request::Read(T::RUST_SCALAR_TYPE),
            |v| slice_values(index, v.len()),
            budget,
        )
    }
    pub fn bind_write_slice<'plan, 'allocation, T: GeneratedDeviceScalarV1>(
        &'plan self,
        index: usize,
        value: GeneratedKfdWriteSlice<'allocation, T>,
        budget: &mut Budget<'_>,
    ) -> Result<(
        GeneratedNominalBindingV3<'plan, 'view, 'wire, GeneratedKfdWriteSlice<'allocation, T>>,
        GeneratedNominalStorageV3,
    )> {
        self.bind(
            index,
            value,
            Request::Write(T::RUST_SCALAR_TYPE),
            |v| slice_values(index, v.len()),
            budget,
        )
    }
    pub fn bind_read_write_slice<'plan, 'allocation, T: GeneratedDeviceScalarV1>(
        &'plan self,
        index: usize,
        value: GeneratedKfdReadWriteSlice<'allocation, T>,
        budget: &mut Budget<'_>,
    ) -> Result<(
        GeneratedNominalBindingV3<'plan, 'view, 'wire, GeneratedKfdReadWriteSlice<'allocation, T>>,
        GeneratedNominalStorageV3,
    )> {
        self.bind(
            index,
            value,
            Request::ReadWrite(T::RUST_SCALAR_TYPE),
            |v| slice_values(index, v.len()),
            budget,
        )
    }

    /// Caller retains paid input-reference headers and actual output backing.
    /// Padding is zeroed only after every query, join, reservation and work charge.
    /// Pointer components remain zero; byte-buffer alignment is not runtime alignment.
    pub fn pack<'plan, 'bindings, 'output>(
        &'plan self,
        inputs: &'bindings [GeneratedNominalArgumentRefV3<'bindings, 'plan, 'view, 'wire>],
        output: &'output mut [u8],
        budget: &mut Budget<'_>,
    ) -> Result<(
        GeneratedNominalPackedArgumentsV3<'output, 'bindings, 'plan, 'view, 'wire>,
        GeneratedNominalStorageV3,
    )> {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let header =
            size_of::<GeneratedNominalPackedArgumentsV3<'output, 'bindings, 'plan, 'view, 'wire>>();
        scoped(self.floor, budget, |budget| {
            budget.charge_work(1)?;
            budget.reserve_storage(header)?;
            budget.reserve_storage(NOMINAL_PACK_SCRATCH_STORAGE_V3)?;
            if inputs.len() != self.count {
                return Err(GeneratedNominalPackErrorV3::Count {
                    expected: self.count,
                    actual: inputs.len(),
                });
            }
            if output.len() != self.output_len {
                return Err(GeneratedNominalPackErrorV3::OutputLength {
                    expected: self.output_len,
                    actual: output.len(),
                });
            }
            budget.charge_work(MAX_ARGUMENTS_PER_KERNEL + MAX_PHYSICAL_COMPONENTS_PER_KERNEL)?;
            let mut scratch = PackScratch {
                order: [usize::MAX; MAX_ARGUMENTS_PER_KERNEL],
                writes: [Write::EMPTY; MAX_PHYSICAL_COMPONENTS_PER_KERNEL],
            };
            for (position, input) in inputs.iter().enumerate() {
                budget.charge_work(4)?;
                let core = input.core;
                if !std::ptr::eq(core.plan, self) || !std::ptr::eq(core.plan.table, self.table) {
                    return Err(argument(core.index, "foreign actual plan"));
                }
                if core.index >= self.count {
                    return Err(argument(core.index, "argument index"));
                }
                if floor < core.floor {
                    return Err(Resource::Accounting.into());
                }
                if scratch.order[core.index] != usize::MAX {
                    return Err(argument(core.index, "duplicate argument"));
                }
                scratch.order[core.index] = position;
            }
            let (mut count, mut byte_work, mut end) = (0, 0usize, 0usize);
            for (index, position) in scratch.order[..self.count].iter().copied().enumerate() {
                budget.charge_work(size_of::<Argument>() + 1)?;
                if position == usize::MAX {
                    return Err(argument(index, "missing argument"));
                }
                let core = inputs[position].core;
                let row = self.row(index)?;
                if core.row != row {
                    return Err(argument(index, "source/layout/component identity"));
                }
                if core.nominal != nominal_identity(row.kind, budget)? {
                    return Err(argument(index, "nominal declaration identity"));
                }
                for (p, value) in row.components[..row.count].iter().zip(core.values) {
                    budget.charge_work(16)?;
                    let start = p.offset as usize;
                    let next = add(start, usize::from(value.len))?;
                    if value.len != p.size as u8
                        || value.len == 0
                        || value.len > 8
                        || start < end
                        || next > output.len()
                        || !start.is_multiple_of(usize::from(p.alignment))
                    {
                        return Err(argument(index, "output component range"));
                    }
                    if count >= scratch.writes.len() {
                        return Err(argument(index, "component limit"));
                    }
                    scratch.writes[count] = Write { start, value };
                    count += 1;
                    end = next;
                    byte_work = add(byte_work, usize::from(value.len))?;
                }
            }
            let retained_floor = add(floor, header)?;
            let expected_live = add(retained_floor, NOMINAL_PACK_SCRATCH_STORAGE_V3)?;
            if budget.storage() != expected_live || budget.work_ledger_identity_v1() != ledger {
                return Err(Resource::Accounting.into());
            }
            budget.charge_work(add(add(output.len(), byte_work)?, 1)?)?;
            // No queries, callbacks, allocation, or fallible checks after this point.
            output.fill(0);
            for write in &scratch.writes[..count] {
                let end = write.start + usize::from(write.value.len);
                output[write.start..end]
                    .copy_from_slice(&write.value.bytes[..usize::from(write.value.len)]);
            }
            Ok((
                GeneratedNominalPackedArgumentsV3 {
                    plan: self,
                    _inputs: inputs,
                    bytes: output,
                    floor: retained_floor,
                },
                GeneratedNominalStorageV3(header),
            ))
        })
    }
}

#[derive(Clone, Copy)]
enum Request {
    Usize,
    Isize,
    Scalar(RustScalarElementTypeV1),
    Read(RustScalarElementTypeV1),
    Write(RustScalarElementTypeV1),
    ReadWrite(RustScalarElementTypeV1),
}
impl Request {
    fn matches(self, row: Argument) -> bool {
        let scalar = |s, expected| descriptor_scalar_to_rust_layout(s) == expected;
        match (self, row.kind) {
            (Self::Usize, SourceTypeDescriptorV3::Usize)
            | (Self::Isize, SourceTypeDescriptorV3::Isize) => row.count == 1,
            (Self::Scalar(expected), SourceTypeDescriptorV3::Scalar(s)) => {
                scalar(s, expected) && row.count == 1
            }
            (Self::Read(expected), SourceTypeDescriptorV3::SharedSlice(s)) => {
                scalar(s, expected)
                    && row.count == 2
                    && row.ownership == OwnershipSemantics::SharedBorrow
                    && row.access == AccessMode::ReadOnly
                    && row.alias == AliasSemantics::SharedReadOnly
            }
            (Self::Write(expected), SourceTypeDescriptorV3::DisjointSlice(s)) => {
                scalar(s, expected)
                    && row.count == 2
                    && row.ownership == OwnershipSemantics::UniqueBorrow
                    && row.access == AccessMode::WriteOnly
                    && row.alias == AliasSemantics::Exclusive
            }
            (Self::ReadWrite(expected), SourceTypeDescriptorV3::DisjointSlice(s)) => {
                scalar(s, expected)
                    && row.count == 2
                    && row.ownership == OwnershipSemantics::UniqueBorrow
                    && row.access == AccessMode::ReadWrite
                    && row.alias == AliasSemantics::Exclusive
            }
            _ => false,
        }
    }
}
#[derive(Clone, Copy)]
struct Value {
    bytes: [u8; 8],
    len: u8,
}
impl Value {
    const ZERO: Self = Self {
        bytes: [0; 8],
        len: 8,
    };
}
fn slice_values(index: usize, length: usize) -> Result<[Value; 2]> {
    let length = u64::try_from(length).map_err(|_| argument(index, "slice length range"))?;
    Ok([
        Value::ZERO,
        Value {
            bytes: length.to_le_bytes(),
            len: 8,
        },
    ])
}
fn nominal_identity(
    kind: SourceTypeDescriptorV3,
    budget: &mut Budget<'_>,
) -> Result<Option<TypeIdentity>> {
    let kind = match kind {
        SourceTypeDescriptorV3::Usize => RustNominalScalarKindV3::Usize,
        SourceTypeDescriptorV3::Isize => RustNominalScalarKindV3::Isize,
        _ => return Ok(None),
    };
    budget.charge_work(
        RUST_NOMINAL_TYPE_DOMAIN_V3.len() + 8 + 4 + RUST_NOMINAL_LAYOUT_DOMAIN_V3.len() + 56 + 32,
    )?;
    let declaration = RustNominalScalarEvidenceV3::new(kind, PointerWidth::Bits64)
        .map_err(GeneratedNominalPackErrorV3::Nominal)?;
    Ok(Some(declaration.type_identity()))
}
struct BindingCore<'plan, 'view, 'wire> {
    plan: &'plan GeneratedNominalArgumentPlanV3<'view, 'wire>,
    index: usize,
    row: Argument,
    values: [Value; 2],
    nominal: Option<TypeIdentity>,
    floor: usize,
}

/// Move-only binding retains its actual scalar or borrowed KFD slice wrapper.
/// No runtime buffer is allocated and no existing `bind_argument` is called.
/// The private core cannot be constructed from an address or caller token.
///
/// ```compile_fail
/// use fe2o3_host::{GeneratedNominalArgumentPlanV3, GeneratedKfdWriteSlice};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn held(plan: &GeneratedNominalArgumentPlanV3<'_, '_>, budget: &mut Budget<'_>) {
///     let mut values = [0u32; 1];
///     let (binding, _) = plan.bind_write_slice(0, GeneratedKfdWriteSlice::new(&mut values), budget).unwrap();
///     let inputs = [binding.as_input()];
///     let mut bytes = [0u8; 16];
///     let (packed, _) = plan.pack(&inputs, &mut bytes, budget).unwrap();
///     values[0] = 7;
///     std::hint::black_box(packed.as_bytes());
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_host::GeneratedNominalBindingV3;
/// fn release(binding: GeneratedNominalBindingV3<'_, '_, '_, usize>) {
///     let input = binding.as_input();
///     drop(binding);
///     std::hint::black_box(input);
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_host::GeneratedNominalArgumentPlanV3;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn release(plan: GeneratedNominalArgumentPlanV3<'_, '_>, budget: &mut Budget<'_>) {
///     let (binding, _) = plan.bind_usize(0, 1, budget).unwrap();
///     drop(plan);
///     let _input = binding.as_input();
/// }
/// ```
pub struct GeneratedNominalBindingV3<'plan, 'view, 'wire, B> {
    core: BindingCore<'plan, 'view, 'wire>,
    _custody: B,
}
impl<'plan, 'view, 'wire, B> GeneratedNominalBindingV3<'plan, 'view, 'wire, B> {
    /// Caller prepays NOMINAL_ARGUMENT_REF_STORAGE_V3 for each live returned ref.
    pub fn as_input(&self) -> GeneratedNominalArgumentRefV3<'_, 'plan, 'view, 'wire> {
        GeneratedNominalArgumentRefV3 { core: &self.core }
    }
    pub const fn nominal_type_identity(&self) -> Option<TypeIdentity> {
        self.core.nominal
    }
    pub const fn argument_index(&self) -> usize {
        self.core.index
    }
}
/// Borrows the private core of a still-live typed binding, not a synthetic token.
pub struct GeneratedNominalArgumentRefV3<'binding, 'plan, 'view, 'wire> {
    core: &'binding BindingCore<'plan, 'view, 'wire>,
}

/// Inert address-free bytes. Slice pointers are zero placeholders, lengths real.
/// This value borrows the output, inputs, source allocations and exact plan/table.
/// No conversion to existing runtime-packed or dispatch types is provided.
///
/// ```compile_fail
/// use fe2o3_host::{GeneratedNominalPackedArgumentsV3, GeneratedKfdPackedArguments};
/// fn runtime<'a>(value: GeneratedNominalPackedArgumentsV3<'a, 'a, 'a, 'a, 'a>)
///     -> GeneratedKfdPackedArguments<'a> { value.into() }
/// ```
///
/// ```compile_fail
/// use fe2o3_host::GeneratedDeviceScalarV1;
/// fn scalar<T: GeneratedDeviceScalarV1>() {}
/// scalar::<usize>();
/// ```
///
/// ```compile_fail
/// use fe2o3_host::GeneratedNominalArgumentPlanV3;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn overwrite(plan: &GeneratedNominalArgumentPlanV3<'_, '_>, budget: &mut Budget<'_>) {
///     let (binding, _) = plan.bind_usize(0, 1, budget).unwrap();
///     let inputs = [binding.as_input()];
///     let mut bytes = [0u8; 8];
///     let (packed, _) = plan.pack(&inputs, &mut bytes, budget).unwrap();
///     bytes[0] = 9;
///     std::hint::black_box(packed.as_bytes());
/// }
/// ```
///
/// The input-reference array remains borrowed while packed bytes are used.
///
/// ```compile_fail,E0505
/// use fe2o3_host::GeneratedNominalArgumentPlanV3;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn release_inputs(plan: &GeneratedNominalArgumentPlanV3<'_, '_>, budget: &mut Budget<'_>) {
///     let (binding, _) = plan.bind_usize(0, 1, budget).unwrap();
///     let inputs = [binding.as_input()];
///     let mut bytes = [0u8; 8];
///     let (packed, _) = plan.pack(&inputs, &mut bytes, budget).unwrap();
///     drop(inputs);
///     std::hint::black_box(packed.as_bytes());
/// }
/// ```
pub struct GeneratedNominalPackedArgumentsV3<'output, 'bindings, 'plan, 'view, 'wire> {
    plan: &'plan GeneratedNominalArgumentPlanV3<'view, 'wire>,
    _inputs: &'bindings [GeneratedNominalArgumentRefV3<'bindings, 'plan, 'view, 'wire>],
    bytes: &'output mut [u8],
    floor: usize,
}
impl GeneratedNominalPackedArgumentsV3<'_, '_, '_, '_, '_> {
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes
    }
    pub const fn kernel_id(&self) -> KernelId {
        self.plan.kernel
    }
    pub const fn required_alignment(&self) -> usize {
        self.plan.alignment
    }
    /// Historical cumulative minimum, including the retained input baselines.
    pub const fn retained_storage_floor(&self) -> usize {
        self.floor
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy)]
struct Write {
    start: usize,
    value: Value,
}
impl Write {
    const EMPTY: Self = Self {
        start: 0,
        value: Value::ZERO,
    };
}
struct PackScratch {
    order: [usize; MAX_ARGUMENTS_PER_KERNEL],
    writes: [Write; MAX_PHYSICAL_COMPONENTS_PER_KERNEL],
}
pub const NOMINAL_ARGUMENT_PLAN_STORAGE_V3: usize =
    size_of::<GeneratedNominalArgumentPlanV3<'static, 'static>>();
pub const NOMINAL_ARGUMENT_REF_STORAGE_V3: usize =
    size_of::<GeneratedNominalArgumentRefV3<'static, 'static, 'static, 'static>>();
/// One binding's fixed hash/row/encoding scratch; the typed binding header is separate.
pub const NOMINAL_BIND_SCRATCH_STORAGE_V3: usize = size_of::<Argument>()
    + size_of::<[Value; 2]>()
    + size_of::<RustNominalScalarEvidenceV3>()
    + size_of::<TypeIdentity>()
    + size_of::<sha2::Sha256>()
    + size_of::<[u8; 56]>()
    + size_of::<[u8; 32]>() * 2
    + size_of::<[u8; 8]>()
    + size_of::<[u8; 4]>();
/// Full fixed write/index tables and simultaneous nominal row/hash scratch.
pub const NOMINAL_PACK_SCRATCH_STORAGE_V3: usize =
    size_of::<PackScratch>() + NOMINAL_BIND_SCRATCH_STORAGE_V3;

#[cfg(test)]
#[path = "generated_nominal_argument_plan_v3_tests.rs"]
mod tests;
