//! Original source/N/E to borrowed descriptor-V3 ABI agreement, not authority.
#![allow(
    clippy::result_large_err,
    reason = "Keep typed child errors inline without unmetered error allocation."
)]

use fe2o3_artifacts::{PointerWidth, RustNominalScalarEvidenceV3, RustNominalScalarKindV3};
use fe2o3_kernel_descriptor::{
    AccessMode as Access, AliasSemantics as Alias, BlockSizeV1, DESCRIPTOR_QUERY_STORAGE_V3,
    DESCRIPTOR_TABLE_VIEW_STORAGE_V3, DescriptorWireErrorV3, DeviceDescriptorTableV3 as Table,
    DeviceLayoutDescriptorV1 as Layout, KernelDescriptorRefV3 as Kernel, KernelId, MAX_KERNELS,
    OwnershipSemantics as Ownership, PhysicalAbiComponentKind as Component, ScalarTypeV1 as Scalar,
    SourceTypeDescriptorV3 as SourceType, SourceTypeRecordV3, device_layout_record_v3,
};
use fe2o3_kernel_ir::{
    AccessMode as KirAccess, AddressSpace, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Function, FunctionRole,
    ScalarType as KirScalar, Type, VerifiedCanonicalKernelIrModuleV12 as Graph,
};
use fe2o3_lower_mir_kernel::{
    CanonicalOutputFormalSourceAnchorV1 as Anchor, ProductionSemanticKirErrorV1,
    ProductionSourceLaunchRosterV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1 as Semantic, SemanticAbiExtensionV1 as Extension,
    SemanticAbiPassModeV1 as Mode, SemanticBackendPrimitiveV1 as Primitive,
    SemanticBackendReprV1 as Repr, SemanticBackendScalarV1 as BackendScalar,
    SemanticFunctionDeclV1 as SourceFunction, SemanticRustTypeKindV1 as Kind,
    SemanticScalarTypeV1 as SourceScalar, SemanticSourceArgumentOwnershipV1 as SourceOwnership,
    SemanticTargetArchitectureV1, SemanticTypeDeclV1 as SourceDecl, SemanticTypeShapeV1 as Shape,
};
use std::mem::size_of;

/// Typed refusal; source replay and descriptor callback errors remain intact.
#[derive(Debug)]
pub enum NominalSourceAbiErrorV3 {
    Resource(Resource),
    Source(ProductionSemanticKirErrorV1),
    Descriptor(DescriptorWireErrorV3<Resource>),
    MissingConnectedSource,
    TargetLayout,
    KernelRoster,
    Root { ordinal: usize },
    Launch { ordinal: usize },
    Signature { root: usize },
    Argument { root: usize, argument: usize },
    NominalKind { root: usize, argument: usize },
    PhysicalLayout { root: usize, argument: usize },
    KernargLayout { root: usize },
    Panicked,
}
type E = NominalSourceAbiErrorV3;
type R<T> = Result<T, E>;
impl From<Resource> for E {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for E {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "nominal source ABI V3: {self:?}")
    }
}
impl std::error::Error for E {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Source(error) => Some(error),
            Self::Descriptor(error) => Some(error),
            _ => None,
        }
    }
}

/// Added borrowing-agreement header, excluding retained source/table backing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NominalAbiStorageV3(usize);
impl NominalAbiStorageV3 {
    /// Reserve before further controlled work; release only after agreement drop.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Complete immutable source/N/E and nominal/physical ABI agreement.
/// This is not source-to-F lineage, CPU/features authentication, signed execution,
/// typed-rustc ABI authority, a native artifact, or permission to launch.
/// Descriptor logical names, build evidence and producer/compiler labels remain
/// inert metadata: the source anchor does not retain/authenticate those fields.
///
/// ```compile_fail
/// use fe2o3_verifier::NominalSourceAbiAgreementV3;
/// fn duplicate(value: NominalSourceAbiAgreementV3<'_, '_>) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_verifier::{NominalSourceAbiAgreementV3, check_nominal_source_abi_v3};
/// use fe2o3_lower_mir_kernel::CanonicalOutputFormalSourceAnchorV1;
/// use fe2o3_kernel_descriptor::DeviceDescriptorTableV3;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn escape<'s>(source: CanonicalOutputFormalSourceAnchorV1<'s>,
///     table: DeviceDescriptorTableV3<'static>, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>)
///     -> NominalSourceAbiAgreementV3<'s, 'static> {
///     check_nominal_source_abi_v3(source, &table, budget).unwrap().0
/// }
/// ```
/// ```compile_fail
/// use fe2o3_verifier::NominalSourceAbiAgreementV3;
/// fn authority(value: NominalSourceAbiAgreementV3<'_, '_>) -> fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1 {
///     value.into()
/// }
/// ```
pub struct NominalSourceAbiAgreementV3<'a, 'wire> {
    source: Anchor<'a>,
    table: &'a Table<'wire>,
    inherited: usize,
}
impl NominalSourceAbiAgreementV3<'_, '_> {
    /// Repeats the complete check with the exact original borrowed owners.
    pub fn verify_equivalence(&self, budget: &mut Budget<'_>) -> R<()> {
        let required = self
            .inherited
            .checked_add(HEADER)
            .ok_or(Resource::Arithmetic)?;
        if budget.storage() < required {
            return Err(Resource::Accounting.into());
        }
        scoped(budget, |budget| {
            budget.charge_work(4)?;
            budget.reserve_storage(SCRATCH)?;
            let parts = source_parts(self.source, budget)?;
            check_table(&parts, self.table, budget)?;
            budget.charge_work(1)?;
            Ok(())
        })
    }
    pub const fn retained_storage(&self) -> usize {
        HEADER
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

const HEADER: usize = size_of::<NominalSourceAbiAgreementV3<'static, 'static>>();
// A query overlaps retained borrowed rows/cursors, two pairs of actual/expected
// records and the portable declaration. QUERY pays the callee's temporary state.
const SCRATCH: usize = DESCRIPTOR_QUERY_STORAGE_V3
    + size_of::<Parts<'static>>()
    + size_of::<Kernel<'static, 'static>>()
    + size_of::<fe2o3_kernel_descriptor::ArgumentCursorV3<'static, 'static>>()
    + size_of::<fe2o3_kernel_descriptor::LogicalArgumentRefV3<'static, 'static>>()
    + 2 * size_of::<SourceTypeRecordV3>()
    + 2 * size_of::<fe2o3_kernel_descriptor::DeviceLayoutRecordV1>()
    + size_of::<Layout>()
    + size_of::<fe2o3_kernel_descriptor::PhysicalComponentV3>()
    + size_of::<RustNominalScalarEvidenceV3>();
struct Parts<'a> {
    semantic: &'a Semantic,
    original: &'a Graph,
    erased: Option<&'a Graph>,
    launch: &'a ProductionSourceLaunchRosterV1,
}

fn scoped<'w, T>(budget: &mut Budget<'w>, run: impl FnOnce(&mut Budget<'w>) -> R<T>) -> R<T> {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'w> as usize;
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            payloads[0] = Some(payload);
            Err(E::Panicked)
        }
    };
    let cleanup = if budget.work_ledger_identity_v1() != ledger
        || slot != budget as *const Budget<'w> as usize
        || budget.storage() < floor
    {
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
    drop(payloads);
    result
}

fn source_floor(source: Anchor<'_>) -> R<usize> {
    match source {
        Anchor::Direct(source) => source
            .pre_ranked_retained_analysis_storage_v1()
            .ok_or(E::MissingConnectedSource),
        Anchor::Erased(source) => Ok(source.retained_storage_floor_v1()),
    }
}

fn source_parts<'a>(source: Anchor<'a>, budget: &mut Budget<'_>) -> R<Parts<'a>> {
    if budget.storage() < source_floor(source)? {
        return Err(Resource::Accounting.into());
    }
    match source {
        Anchor::Direct(source) => {
            let original = source
                .pre_ranked_executable()
                .ok_or(E::MissingConnectedSource)?;
            let launch = source
                .source_launch_roster()
                .ok_or(E::MissingConnectedSource)?;
            source
                .verify_equivalence_with_budget_v1(budget)
                .map_err(E::Source)?;
            Ok(Parts {
                semantic: source.semantic().semantic(),
                original,
                erased: None,
                launch,
            })
        }
        Anchor::Erased(source) => {
            source.verify_equivalence(budget).map_err(E::Source)?;
            let original = source.original_source();
            Ok(Parts {
                semantic: original.semantic_ssa().source_semantic(),
                original: original.executable(),
                erased: Some(source.erased()),
                launch: original.source_launch(),
            })
        }
    }
}

fn scalar(value: Scalar) -> (KirScalar, SourceScalar) {
    use SourceScalar::{Float, Integer};
    match value {
        Scalar::I8 => (
            KirScalar::I8,
            Integer {
                signed: true,
                bits: 8,
            },
        ),
        Scalar::U8 => (
            KirScalar::U8,
            Integer {
                signed: false,
                bits: 8,
            },
        ),
        Scalar::I16 => (
            KirScalar::I16,
            Integer {
                signed: true,
                bits: 16,
            },
        ),
        Scalar::U16 => (
            KirScalar::U16,
            Integer {
                signed: false,
                bits: 16,
            },
        ),
        Scalar::I32 => (
            KirScalar::I32,
            Integer {
                signed: true,
                bits: 32,
            },
        ),
        Scalar::U32 => (
            KirScalar::U32,
            Integer {
                signed: false,
                bits: 32,
            },
        ),
        Scalar::I64 => (
            KirScalar::I64,
            Integer {
                signed: true,
                bits: 64,
            },
        ),
        Scalar::U64 => (
            KirScalar::U64,
            Integer {
                signed: false,
                bits: 64,
            },
        ),
        Scalar::F16 => (KirScalar::F16, Float { bits: 16 }),
        Scalar::F32 => (KirScalar::F32, Float { bits: 32 }),
        Scalar::F64 => (KirScalar::F64, Float { bits: 64 }),
    }
}

fn nominal(ty: &SourceDecl, declared: SourceType, root: usize, argument: usize) -> R<()> {
    let kind = match (ty.rust_type_kind(), declared) {
        (Kind::Usize, SourceType::Usize) => RustNominalScalarKindV3::Usize,
        (Kind::Isize, SourceType::Isize) => RustNominalScalarKindV3::Isize,
        (Kind::Ordinary, SourceType::Scalar(value)) => {
            if ty.shape() == &Shape::Scalar(scalar(value).1) {
                return Ok(());
            }
            return Err(E::NominalKind { root, argument });
        }
        (
            Kind::Ordinary,
            SourceType::SharedSlice(_)
            | SourceType::DisjointSlice(_)
            | SourceType::GlobalMutPointer(_),
        ) => return Ok(()),
        _ => return Err(E::NominalKind { root, argument }),
    };
    let evidence = RustNominalScalarEvidenceV3::new(kind, PointerWidth::Bits64)
        .map_err(|_| E::TargetLayout)?;
    let signed = kind == RustNominalScalarKindV3::Isize;
    // Artifact [version, nominal tag] and descriptor [nominal tag] payloads
    // deliberately inhabit different domains. Never equate their digest bytes.
    if evidence.canonical_type_payload() != [3, 0, if signed { 6 } else { 5 }, 0]
        || declared.canonical_payload() != [if signed { 6 } else { 5 }, 0, 0, 0]
        || evidence.physical_scalar()
            != if signed {
                fe2o3_artifacts::RustScalarElementTypeV1::I64
            } else {
                fe2o3_artifacts::RustScalarElementTypeV1::U64
            }
        || evidence.abi_class() != fe2o3_artifacts::RustcAbiClassV1::Scalar
        || ty.shape() != &Shape::Scalar(SourceScalar::Integer { signed, bits: 64 })
        || ty.layout().size_bytes() != Some(evidence.size())
        || ty.layout().alignment_bytes() != u64::from(evidence.abi_alignment())
        || !matches!(ty.layout().backend_repr(),
            Repr::Scalar(BackendScalar::Initialized { primitive, .. })
            if *primitive == Primitive::integer(signed, 64, 8))
    {
        return Err(E::NominalKind { root, argument });
    }
    Ok(())
}

fn argument_shape(source: SourceType, ty: &Type, access: Access) -> bool {
    let element = scalar(source.physical_scalar()).0;
    match (source, ty) {
        (SourceType::Scalar(_) | SourceType::Usize | SourceType::Isize, Type::Scalar(actual)) => {
            *actual == element && access == Access::ByValue
        }
        (SourceType::SharedSlice(_), Type::Slice(actual)) => {
            actual.address_space == AddressSpace::Global
                && actual.element.as_scalar() == Some(element)
                && access == Access::ReadOnly
                && actual.access == KirAccess::ReadOnly
        }
        (SourceType::DisjointSlice(_), Type::Slice(actual)) => {
            actual.address_space == AddressSpace::Global
                && actual.element.as_scalar() == Some(element)
                && matches!(
                    (access, actual.access),
                    (Access::WriteOnly, KirAccess::WriteOnly)
                        | (Access::ReadWrite, KirAccess::ReadWrite)
                )
        }
        (SourceType::GlobalMutPointer(_), Type::Pointer(actual)) => {
            actual.address_space == AddressSpace::Global
                && actual.pointee.as_scalar() == Some(element)
                && access == Access::ReadWrite
                && actual.access == KirAccess::ReadWrite
        }
        _ => false,
    }
}

fn arguments(
    table: &Table<'_>,
    row: &Kernel<'_, '_>,
    parts: &Parts<'_>,
    function: &SourceFunction,
    original: &Function,
    root: usize,
    budget: &mut Budget<'_>,
) -> R<()> {
    let abi = function.abi();
    let count = row.argument_count();
    if !original.signature.results.is_empty()
        || count != original.signature.parameters.len()
        || count != abi.source_input_types().len()
        || count != abi.adjusted_arguments().len()
        || count != abi.source_argument_ownership().len()
    {
        return Err(E::Signature { root });
    }
    let mut cursor = row.arguments();
    let mut offset = 0u32;
    let mut alignment = 1u32;
    for index in 0..count {
        budget.charge_work(192)?;
        let arg = cursor
            .next(&mut |work| budget.charge_work(work))
            .map_err(E::Descriptor)?
            .ok_or(E::Argument {
                root,
                argument: index,
            })?;
        let source = table
            .source_type(arg.source_type(), &mut |work| budget.charge_work(work))
            .map_err(E::Descriptor)?;
        let layout = table
            .device_layout(arg.device_layout(), &mut |work| budget.charge_work(work))
            .map_err(E::Descriptor)?;
        let kind = source.descriptor();
        let physical = kind.physical_scalar();
        let expected_layout = match kind {
            SourceType::Scalar(_) | SourceType::Usize | SourceType::Isize => {
                Layout::scalar(physical)
            }
            SourceType::SharedSlice(_) => Layout::shared_slice(physical),
            SourceType::DisjointSlice(_) => Layout::disjoint_slice(physical),
            SourceType::GlobalMutPointer(_) => Layout::global_mut_pointer(physical),
        };
        let expected_source =
            SourceTypeRecordV3::new(kind, &mut |w| budget.charge_work(w)).map_err(E::Descriptor)?;
        let expected_physical =
            device_layout_record_v3(expected_layout.clone(), &mut |w| budget.charge_work(w))
                .map_err(E::Descriptor)?;
        let ty = parts
            .semantic
            .types()
            .get(abi.source_input_types()[index].index() as usize)
            .ok_or(E::Argument {
                root,
                argument: index,
            })?;
        nominal(ty, kind, root, index)?;
        let slice = matches!(
            kind,
            SourceType::SharedSlice(_) | SourceType::DisjointSlice(_)
        );
        let by_value = matches!(
            kind,
            SourceType::Scalar(_) | SourceType::Usize | SourceType::Isize
        );
        let (ownership, alias, source_ownership) = if by_value {
            (Ownership::ByValue, Alias::Value, SourceOwnership::ByValue)
        } else if matches!(kind, SourceType::SharedSlice(_)) {
            (
                Ownership::SharedBorrow,
                Alias::SharedReadOnly,
                SourceOwnership::SharedBorrow,
            )
        } else {
            (
                Ownership::UniqueBorrow,
                Alias::Exclusive,
                SourceOwnership::ExclusiveOwner,
            )
        };
        let adjusted = &abi.adjusted_arguments()[index];
        if arg.source_index() as usize != index
            || arg.ownership() != ownership
            || arg.alias() != alias
            || abi.source_argument_ownership()[index] != source_ownership
            || !argument_shape(kind, &original.signature.parameters[index], arg.access())
            || adjusted.ty() != abi.source_input_types()[index]
            || !matches!(
                (slice, adjusted.mode()),
                (true, Mode::Pair { .. }) | (false, Mode::Direct(_))
            )
            || (matches!(kind, SourceType::Usize | SourceType::Isize)
                && !matches!(adjusted.mode(), Mode::Direct(attrs) if attrs.extension() == Extension::None))
        {
            return Err(E::Argument {
                root,
                argument: index,
            });
        }
        if source.identity() != expected_source.identity()
            || layout != expected_physical
            || ty.layout().size_bytes() != Some(u64::from(expected_layout.size_bytes()))
            || ty.layout().alignment_bytes() != u64::from(expected_layout.alignment_bytes())
        {
            return Err(E::PhysicalLayout {
                root,
                argument: index,
            });
        }
        let align = u32::from(expected_layout.alignment_bytes());
        alignment = alignment.max(align);
        offset = offset.checked_add(align - 1).ok_or(Resource::Arithmetic)? / align * align;
        if arg.component_count() != if slice { 2 } else { 1 } {
            return Err(E::PhysicalLayout {
                root,
                argument: index,
            });
        }
        for component in 0..arg.component_count() {
            let actual = arg
                .component(component, &mut |work| budget.charge_work(work))
                .map_err(E::Descriptor)?;
            let (kind, at, size, align) = if component == 1 {
                (
                    Component::SliceLengthU64,
                    offset.checked_add(8).ok_or(Resource::Arithmetic)?,
                    8,
                    8,
                )
            } else if by_value {
                (
                    Component::ScalarByValue(physical),
                    offset,
                    expected_layout.size_bytes(),
                    expected_layout.alignment_bytes(),
                )
            } else {
                (Component::GlobalPointer, offset, 8, 8)
            };
            let (access, alias) = if component == 1 {
                (Access::ByValue, Alias::Value)
            } else {
                (arg.access(), arg.alias())
            };
            if (
                actual.kind,
                actual.offset,
                actual.size,
                actual.alignment,
                actual.access,
                actual.alias,
            ) != (kind, at, size, align, access, alias)
            {
                return Err(E::PhysicalLayout {
                    root,
                    argument: index,
                });
            }
        }
        offset = offset
            .checked_add(u32::from(expected_layout.size_bytes()))
            .ok_or(Resource::Arithmetic)?;
    }
    if cursor
        .next(&mut |w| budget.charge_work(w))
        .map_err(E::Descriptor)?
        .is_some()
    {
        return Err(E::Signature { root });
    }
    offset = offset
        .checked_add(alignment - 1)
        .ok_or(Resource::Arithmetic)?
        / alignment
        * alignment;
    let layout = row.abi_layout();
    if layout.explicit_argument_size() != offset
        || layout.kernarg_segment_size() != offset.checked_add(256).ok_or(Resource::Arithmetic)?
        || layout.kernarg_segment_alignment() != alignment
    {
        return Err(E::KernargLayout { root });
    }
    Ok(())
}

fn kernel<'a>(
    graph: &'a Graph,
    name: &[u8],
) -> Option<(&'a fe2o3_kernel_ir::Kernel, &'a Function)> {
    let mut rows = graph
        .module()
        .kernels
        .iter()
        .filter(|k| k.id.as_str().as_bytes() == name);
    let row = rows.next()?;
    if rows.next().is_some() {
        return None;
    }
    let function = graph
        .module()
        .functions
        .iter()
        .find(|f| f.id == row.entry)?;
    (function.role == FunctionRole::KernelEntry).then_some((row, function))
}

fn check_table(parts: &Parts<'_>, table: &Table<'_>, budget: &mut Budget<'_>) -> R<()> {
    budget.charge_work(8)?;
    if !matches!(
        parts.semantic.target().architecture(),
        SemanticTargetArchitectureV1::AmdGpuGfx942
    ) {
        return Err(E::TargetLayout);
    }
    // This architecture's semantic ABI is 64-bit; neither it nor its opaque
    // layout hash authenticates the descriptor's exact CPU/features.
    let count = parts.semantic.roots().len();
    if !(1..=MAX_KERNELS).contains(&count)
        || table.kernel_count() != count
        || parts.launch.roots().len() != count
        || parts.original.module().kernels.len() != count
        || parts
            .erased
            .is_some_and(|e| e.module().kernels.len() != count)
        || parts.launch.semantic_sha256() != parts.semantic.semantic_sha256().as_bytes()
    {
        return Err(E::KernelRoster);
    }
    let scan = parts
        .semantic
        .canonical_encoding()
        .len()
        .checked_add(parts.original.canonical().canonical_bytes().len())
        .and_then(|n| {
            n.checked_add(
                parts
                    .erased
                    .map_or(0, |e| e.canonical().canonical_bytes().len()),
            )
        })
        .and_then(|n| n.checked_add(table.canonical_bytes().len()))
        .and_then(|n| n.checked_add(256))
        .ok_or(Resource::Arithmetic)?;
    for (ordinal, id) in parts.semantic.roots().iter().enumerate() {
        budget.charge_work(scan)?;
        let function = parts
            .semantic
            .functions()
            .get(id.index() as usize)
            .ok_or(E::Root { ordinal })?;
        let entry = function.kernel_entry().ok_or(E::Root { ordinal })?;
        let launch = parts.launch.roots()[ordinal];
        let name = entry.export_symbol().as_bytes();
        let (n, nf) = kernel(parts.original, name).ok_or(E::Root { ordinal })?;
        let key = KernelId::from_bytes(*entry.kernel_binding_identity().as_bytes());
        let row = table
            .find_kernel(key, &mut |w| budget.charge_work(w))
            .map_err(E::Descriptor)?;
        if parts.launch.roots()[..ordinal]
            .iter()
            .any(|prior| prior.kernel_binding() == launch.kernel_binding())
            || row.entry_name().as_bytes() != name
            || row
                .descriptor_symbol()
                .strip_suffix(".kd")
                .map(str::as_bytes)
                != Some(name)
            || launch.selected_root() != *id
            || launch.semantic_root_identity() != function.identity()
            || launch.kernel_binding() != *entry.kernel_binding_identity().as_bytes()
        {
            return Err(E::Root { ordinal });
        }
        let wg = launch
            .source_launch()
            .exact_workgroup()
            .ok_or(E::Launch { ordinal })?;
        let BlockSizeV1::Exact(block) = row.launch().block_size() else {
            return Err(E::Launch { ordinal });
        };
        let grid = row.launch().max_grid();
        let resources = entry
            .source_contract()
            .resources()
            .map(|r| {
                (
                    r.static_shared_memory_bytes(),
                    r.max_dynamic_shared_memory_bytes(),
                )
            })
            .unwrap_or_default();
        if row.launch().rank() != launch.source_rank()
            || [block.x(), block.y(), block.z()] != wg
            || [grid.x(), grid.y(), grid.z()] != launch.source_launch().max_grid()
            || row.launch().max_flat_workgroup_size()
                != wg
                    .into_iter()
                    .try_fold(1u32, u32::checked_mul)
                    .ok_or(Resource::Arithmetic)?
            || (
                row.launch().static_shared_memory_bytes(),
                row.launch().max_dynamic_shared_memory_bytes(),
            ) != resources
            || n.domain.rank() != launch.source_rank()
            || n.workgroup_size.map(|w| [w.x, w.y, w.z]) != Some(wg)
        {
            return Err(E::Launch { ordinal });
        }
        if let Some(erased) = parts.erased {
            let (e, ef) = kernel(erased, name).ok_or(E::Root { ordinal })?;
            if e.domain != n.domain
                || e.workgroup_size != n.workgroup_size
                || ef.signature != nf.signature
            {
                return Err(E::Signature { root: ordinal });
            }
        }
        // FnAbi and target-layout fingerprints occupy different domains. Check
        // observed argument layouts below, never equality across those domains.
        arguments(table, &row, parts, function, nf, ordinal, budget)?;
    }
    Ok(())
}

/// Check every actual source argument and complete root/launch roster. Source
/// replay is mandatory; a legacy disconnected Direct owner cannot enter.
/// Borrowed wire/table, source and siblings must already be reserved. This adds
/// only fixed header/query storage, uses the original ledger, and restores
/// its floor on every return/unwind. The returned header receipt is unreserved.
/// Existing source replay retains its established accounting domain. No bytes
/// or metadata are upgraded to signed execution, native or launch authority.
pub fn check_nominal_source_abi_v3<'a, 'wire>(
    source: Anchor<'a>,
    table: &'a Table<'wire>,
    budget: &mut Budget<'_>,
) -> R<(NominalSourceAbiAgreementV3<'a, 'wire>, NominalAbiStorageV3)> {
    let inherited = budget.storage();
    scoped(budget, |budget| {
        budget.charge_work(4)?;
        let minimum = source_floor(source)?
            .checked_add(table.canonical_bytes().len())
            .and_then(|n| n.checked_add(DESCRIPTOR_TABLE_VIEW_STORAGE_V3))
            .ok_or(Resource::Arithmetic)?;
        if inherited < minimum {
            return Err(Resource::Accounting.into());
        }
        budget.reserve_storage(HEADER)?;
        budget.reserve_storage(SCRATCH)?;
        let parts = source_parts(source, budget)?;
        check_table(&parts, table, budget)?;
        budget.charge_work(1)?;
        Ok((
            NominalSourceAbiAgreementV3 {
                source,
                table,
                inherited,
            },
            NominalAbiStorageV3(HEADER),
        ))
    })
}

#[cfg(test)]
#[path = "compiler_nominal_abi_v3_tests.rs"]
mod tests;
