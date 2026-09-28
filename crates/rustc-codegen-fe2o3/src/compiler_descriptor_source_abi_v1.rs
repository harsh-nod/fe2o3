//! Captured Rust source ABI to exact checked physical entry, not a packer.
use super::checked_output_policy3_v1::nominal_policy4_v3::OwnerRef;
use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Function, FunctionRole, Type,
    ValueId,
};
use fe2o3_lower_mir_kernel::{
    ProductionSemanticKirErrorV1 as SourceError, ProductionSourceAbiPlanV1 as Plan,
};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticBackendReprV1, SemanticFunctionDeclV1,
    SemanticFunctionIdV1, SemanticLayoutIdentityV1, SemanticRustTypeKindV1,
    SemanticSourceArgumentOwnershipV1, SemanticTargetDataLayoutV1, SemanticTypeDeclV1,
    SemanticTypeShapeV1,
};
use rustc_middle::ty::Ty;
use std::mem::{align_of, size_of};
#[path = "compiler_descriptor_entry_packing_v1.rs"]
pub(crate) mod entry_packing;
#[cfg(test)]
#[path = "compiler_descriptor_source_abi_v1_tests.rs"]
pub(crate) mod tests;

#[derive(Debug)]
pub(crate) enum SourceAbiErrorV1 {
    Resource(Resource),
    Source(SourceError),
    Checked(fe2o3_lower_mir_kernel::ProductionCheckedOutputAdmissionErrorPolicy4V1),
    Target(dialect_amdgcn::ProductionTargetCoordinateErrorV1),
    Mismatch(&'static str),
}
impl fmt::Display for SourceAbiErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{self:?}")
    }
}
impl std::error::Error for SourceAbiErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Source(error) => Some(error),
            Self::Checked(error) => Some(error),
            Self::Target(error) => Some(error),
            Self::Mismatch(_) => None,
        }
    }
}
impl From<Resource> for SourceAbiErrorV1 {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<SourceError> for SourceAbiErrorV1 {
    fn from(e: SourceError) -> Self {
        Self::Source(e)
    }
}

// The outer descriptor already carries the complete Policy3 diagnostic. Do
// not duplicate that large payload through another nested checked-error enum.
// This private boundary retains every nonchecked payload without allocation;
// checked failures are normalized by CompilerDescriptorError::from_source_abi.
#[derive(Debug)]
pub(crate) enum SourceAbiDiagnosticV1 {
    Resource(Resource),
    Source(SourceError),
    Target(dialect_amdgcn::ProductionTargetCoordinateErrorV1),
    Mismatch(&'static str),
}
impl fmt::Display for SourceAbiDiagnosticV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{self:?}")
    }
}
impl std::error::Error for SourceAbiDiagnosticV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Source(error) => Some(error),
            Self::Target(error) => Some(error),
            Self::Mismatch(_) => None,
        }
    }
}
type E = SourceAbiErrorV1;
type R<T> = Result<T, E>;

pub(super) fn capture_target(tcx: TyCtxt<'_>) -> R<SemanticTargetDataLayoutV1> {
    let target = crate::semantic_layout_bridge::rustc_semantic_layout_target_v1(tcx)
        .map_err(|_| E::Mismatch("fresh rustc source-layout target"))?;
    Ok(crate::rustc_semantic_adapter_v1::canonical_target_layout_v1(&target))
}
pub(super) fn capture_layout<'tcx>(
    tcx: TyCtxt<'tcx>,
    target: SemanticTargetDataLayoutV1,
    ty: Ty<'tcx>,
) -> R<SemanticLayoutIdentityV1> {
    let layout = tcx
        .layout_of(TypingEnv::fully_monomorphized().as_query_input(ty))
        .map_err(|_| E::Mismatch("fresh rustc source type layout"))?;
    Ok(crate::rustc_semantic_adapter_v1::rustc_semantic_layout_identity_v1(tcx, target, layout))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceAbiSummaryV1 {
    pub(crate) roots: usize,
    pub(crate) logical: usize,
    pub(crate) physical: usize,
}

type Subjects<'a> = (
    &'a AdmittedInertSemanticMirV1,
    [&'a Module; 4],
    &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
);
type CheckFrame<'a, 'work> = (
    OwnerRef<'a>,
    &'a [TypedDescriptorRootV1],
    ProductionAmdTargetProfileV1,
    &'a mut Budget<'work>,
    usize,
);
type RootFrame<'a, 'source, 'work> = (
    &'a mut Plan<'a, 'source, 'work>,
    &'a AdmittedInertSemanticMirV1,
    &'a SemanticFunctionDeclV1,
    &'a TypedDescriptorRootV1,
    usize,
    [&'a Module; 4],
);
// Value-sized upper bound also covers reference captures of these parameters.
type RootVisitor<'visit> = dyn for<'scope, 'source, 'work> FnMut(
        usize,
        &TypedDescriptorRootV1,
        &AdmittedInertSemanticMirV1,
        &mut Plan<'scope, 'source, 'work>,
    ) -> R<()>
    + 'visit;
type CheckCapture<'a, 'work> = (CheckFrame<'a, 'work>, Option<&'a mut RootVisitor<'a>>);

fn frame<T>() -> usize {
    size_of::<T>() + 3 * size_of::<R<T>>() + 2 * size_of::<Result<T, SourceError>>()
}
fn sum_frames(parts: &[usize]) -> R<usize> {
    parts.iter().try_fold(0usize, |sum, n| {
        sum.checked_add(*n).ok_or(Resource::Arithmetic.into())
    })
}

// Complete stack-only query/result envelopes. Dynamic source-shape scratch and
// immutable argument rows remain paid by the existing lowerer plan's scope.
fn headers() -> R<usize> {
    use self::frame as h;
    sum_frames(&[
        h::<CheckFrame<'_, '_>>(),
        h::<RootFrame<'_, '_, '_>>(),
        h::<CheckCapture<'_, '_>>(),
        h::<CheckFrame<'_, '_>>(),
        h::<Option<&mut RootVisitor<'_>>>(),
        h::<&mut Option<&mut RootVisitor<'_>>>(),
        h::<&mut RootVisitor<'_>>(),
        h::<(
            usize,
            &TypedDescriptorRootV1,
            &AdmittedInertSemanticMirV1,
            &mut Plan<'_, '_, '_>,
        )>(),
        h::<Subjects<'_>>(),
        h::<OwnerRef<'_>>(),
        h::<SourceAbiSummaryV1>(),
        h::<Option<E>>(),
        h::<()>(),
        h::<usize>(),
        h::<Option<usize>>(),
        h::<(usize, usize)>(),
        h::<&AdmittedInertSemanticMirV1>(),
        h::<&SemanticFunctionDeclV1>(),
        h::<Option<&SemanticFunctionDeclV1>>(),
        h::<&SemanticTypeDeclV1>(),
        h::<&TypedDescriptorRootV1>(),
        h::<&TypedDescriptorArgumentV1>(),
        h::<[&Module; 4]>(),
        h::<&Function>(),
        h::<Option<&Function>>(),
        h::<&fe2o3_kernel_ir::FunctionBody>(),
        h::<Option<&fe2o3_kernel_ir::FunctionBody>>(),
        2 * h::<&fe2o3_kernel_ir::Kernel>(),
        2 * h::<Option<&fe2o3_kernel_ir::Kernel>>(),
        h::<&fe2o3_kernel_ir::FunctionId>(),
        h::<&Type>(),
        h::<Option<&Type>>(),
        h::<ValueId>(),
        h::<Option<&ValueId>>(),
        h::<&mut Plan<'_, '_, '_>>(),
        h::<fe2o3_lower_mir_kernel::ProductionSourceAbiArgumentV1>(),
        2 * h::<fe2o3_lower_mir_kernel::ProductionSourceAbiComponentV1<'_>>(),
        h::<fe2o3_lower_mir_kernel::ProductionPhysicalArgumentV1<'_>>(),
        h::<&fe2o3_lower_mir_kernel::SemanticKirFunctionCorrespondenceV1>(),
        h::<fe2o3_mir_model::semantic_mir_v1::SemanticKernelBodySelectionV1>(),
        h::<Option<fe2o3_mir_model::semantic_mir_v1::SemanticKernelBodySelectionV1>>(),
        h::<&fe2o3_mir_model::semantic_mir_v1::SemanticKernelEntryV1>(),
        h::<Option<&fe2o3_mir_model::semantic_mir_v1::SemanticKernelEntryV1>>(),
        h::<&fe2o3_mir_model::semantic_mir_v1::SemanticFunctionAbiV1>(),
        h::<&fe2o3_mir_model::semantic_mir_v1::SemanticTypeLayoutV1>(),
        h::<&[fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1]>(),
        h::<&[SemanticSourceArgumentOwnershipV1]>(),
        h::<Option<&SemanticSourceArgumentOwnershipV1>>(),
        h::<&[SemanticFunctionDeclV1]>(),
        h::<&[SemanticFunctionIdV1]>(),
        h::<&[TypedDescriptorArgumentV1]>(),
        h::<&[&Module]>(),
        h::<&Module>(),
        h::<&[Type]>(),
        h::<&[ValueId]>(),
        h::<&[fe2o3_kernel_ir::Kernel]>(),
        h::<&[u8; 32]>(),
        h::<fe2o3_mir_model::semantic_mir_v1::SemanticKernelBindingIdentityV1>(),
        h::<&fe2o3_mir_model::semantic_mir_v1::SemanticKernelBindingIdentityV1>(),
        h::<&fe2o3_mir_model::semantic_mir_v1::SemanticLinkSymbolV1>(),
        h::<&fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1>(),
        h::<&fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1>(),
        h::<&fe2o3_pliron::ProductionSemanticMirOwnerV1>(),
        h::<&fe2o3_pliron::ProductionSemanticSsaOwnerV1>(),
        h::<&fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12>(),
        h::<Option<&fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12>>(),
        h::<Result<(), fe2o3_lower_mir_kernel::ProductionCheckedOutputAdmissionErrorPolicy4V1>>(),
        h::<Result<usize, fe2o3_lower_mir_kernel::ProductionCheckedOutputAdmissionErrorPolicy4V1>>(
        ),
        h::<Result<(), dialect_amdgcn::ProductionTargetCoordinateErrorV1>>(),
        h::<Result<(), Resource>>(),
        h::<Result<usize, Resource>>(),
        h::<fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdentityV1>(),
        h::<SemanticLayoutIdentityV1>(),
        h::<[u8; 32]>(),
        h::<SemanticRustTypeKindV1>(),
        h::<&SemanticTypeShapeV1>(),
        h::<SemanticBackendReprV1>(),
        h::<&SemanticBackendReprV1>(),
        h::<Option<u64>>(),
        h::<&[u8]>(),
        h::<&str>(),
        h::<bool>(),
        h::<SemanticFunctionIdV1>(),
        h::<RustcAbiClassV1>(),
        h::<std::ops::Range<usize>>(),
        h::<Option<usize>>(),
        h::<std::iter::Enumerate<std::slice::Iter<'_, TypedDescriptorRootV1>>>(),
        h::<Option<(usize, &TypedDescriptorRootV1)>>(),
        h::<std::iter::Enumerate<std::slice::Iter<'_, TypedDescriptorArgumentV1>>>(),
        h::<Option<(usize, &TypedDescriptorArgumentV1)>>(),
        h::<std::slice::Iter<'_, &Module>>(),
        h::<Option<&&Module>>(),
        size_of::<std::array::IntoIter<&Module, 4>>(),
        size_of::<std::slice::Iter<'_, Function>>(),
        16 * size_of::<usize>(),
        4 * size_of::<u64>(),
        size_of::<&mut Budget<'_>>(),
        size_of::<ProductionAmdTargetProfileV1>(),
    ])
}

fn bridge_headers<T, F>() -> R<usize> {
    sum_frames(&[
        frame::<(
            OwnerRef<'_>,
            SemanticFunctionIdV1,
            SemanticFunctionIdV1,
            &mut Budget<'_>,
        )>(),
        frame::<F>(),
        align_of::<F>(),
        frame::<(Result<T, SourceError>, Option<E>)>(),
        frame::<Option<E>>(),
        frame::<T>(),
        frame::<&mut Option<E>>(),
        // Conservative callback capture bound for (run, &mut selected),
        // independently checked against the actual closure before invocation.
        size_of::<F>(),
        size_of::<&mut Option<E>>(),
        align_of::<F>(),
        align_of::<&mut Option<E>>(),
        8 * frame::<usize>(),
        frame::<bool>(),
    ])
}

fn with_plan<'work, T, F>(
    owner: OwnerRef<'_>,
    root: SemanticFunctionIdV1,
    body: SemanticFunctionIdV1,
    budget: &mut Budget<'work>,
    run: F,
) -> R<T>
where
    F: for<'scope, 'source> FnOnce(&mut Plan<'scope, 'source, 'work>) -> R<T>,
{
    let floor = budget.storage();
    let retained = bridge_headers::<T, F>()?;
    budget.reserve_storage(retained)?;
    let mut selected = None;
    let callback = |plan: &mut Plan<'_, '_, 'work>| match run(plan) {
        Ok(value) => Ok(value),
        Err(error) => {
            selected = Some(error);
            Err(SourceError::CorrespondenceMismatch)
        }
    };
    let capture_size = size_of::<F>()
        + size_of::<&mut Option<E>>()
        + align_of::<F>()
        + align_of::<&mut Option<E>>();
    let capture_align = align_of::<F>().max(align_of::<&mut Option<E>>());
    let result = if std::mem::size_of_val(&callback) > capture_size
        || std::mem::align_of_val(&callback) > capture_align
    {
        drop(callback);
        Err(SourceError::ArgumentCorrespondenceResource(
            Resource::Accounting,
        ))
    } else {
        match owner {
            OwnerRef::Direct(value) => value
                .source_semantic_kir()
                .with_checked_source_abi_plan_v1(root, body, budget, callback),
            OwnerRef::Erased(value) => value
                .original_source()
                .with_checked_source_abi_plan_v1(root, body, budget, callback),
        }
    };
    let result = match (result, selected) {
        (Ok(value), None) => Ok(value),
        // The lowerer preserves its own first query resource failure. Other
        // callback errors keep their original type and do not run a final query.
        (Err(error @ SourceError::ArgumentCorrespondenceResource(_)), _) => Err(E::Source(error)),
        (Err(_), Some(error)) => Err(error),
        (Err(error), None) => Err(E::Source(error)),
        (Ok(_), Some(_)) => Err(E::Mismatch("source ABI callback completion")),
    };
    if budget
        .storage()
        .checked_sub(floor)
        .is_none_or(|live| live < retained)
    {
        return Err(Resource::Accounting.into());
    }
    budget.release_storage(retained)?;
    result
}

fn source_class(ty: &SemanticTypeDeclV1) -> R<RustcAbiClassV1> {
    if ty.layout().is_uninhabited() {
        return Err(E::Mismatch("inhabited source ABI"));
    }
    match ty.layout().backend_repr() {
        SemanticBackendReprV1::Scalar(_) => Ok(RustcAbiClassV1::Scalar),
        SemanticBackendReprV1::ScalarPair { .. } => Ok(RustcAbiClassV1::ScalarPair),
        SemanticBackendReprV1::Memory { sized: true } => Ok(RustcAbiClassV1::Aggregate),
        _ => Err(E::Mismatch("sized scalar or by-value source ABI")),
    }
}

fn check_root(
    plan: &mut Plan<'_, '_, '_>,
    semantic: &AdmittedInertSemanticMirV1,
    source: &SemanticFunctionDeclV1,
    root: &TypedDescriptorRootV1,
    ordinal: usize,
    endpoints: [&Module; 4],
) -> R<(usize, usize)> {
    plan.check_subject(semantic, endpoints[0])?;
    let (logical, physical) = plan.counts()?;
    if logical != root.arguments.len() || logical != source.abi().source_input_types().len() {
        return Err(E::Mismatch("complete logical source argument coverage"));
    }
    let first = endpoints[0]
        .kernels
        .get(ordinal)
        .ok_or(E::Mismatch("original kernel ordinal"))?;
    plan.charge_work(first.entry.as_str().len())?;
    if plan.association()?.kernel_ir_function() != &first.entry {
        return Err(E::Mismatch("owner-bound physical source entry"));
    }
    for module in endpoints {
        let kernel = module
            .kernels
            .get(ordinal)
            .ok_or(E::Mismatch("complete endpoint kernel roster"))?;
        plan.charge_work(
            root.export_name
                .len()
                .checked_add(first.entry.as_str().len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        if kernel.id.as_str() != root.export_name || kernel.entry != first.entry {
            return Err(E::Mismatch("exact endpoint kernel/entry identity"));
        }
        plan.charge_work(
            module
                .functions
                .len()
                .checked_mul(
                    first
                        .entry
                        .as_str()
                        .len()
                        .checked_add(1)
                        .ok_or(Resource::Arithmetic)?,
                )
                .ok_or(Resource::Arithmetic)?,
        )?;
        let function = module
            .function(&first.entry)
            .ok_or(E::Mismatch("endpoint entry definition"))?;
        let body = function
            .body
            .as_ref()
            .ok_or(E::Mismatch("defined endpoint entry"))?;
        if function.role != FunctionRole::KernelEntry
            || !function.signature.results.is_empty()
            || function.signature.parameters.len() != physical
            || body.parameters.len() != physical
        {
            return Err(E::Mismatch("complete physical entry signature coverage"));
        }
        for slot in 0..physical {
            plan.charge_work(32)?;
            let row = plan.component(slot)?.physical();
            if row.slot() != slot
                || function.signature.parameters.get(slot) != Some(row.ty())
                || body.parameters.get(slot) != Some(&row.value())
            {
                return Err(E::Mismatch(
                    "exact original/optimized entry parameter transport",
                ));
            }
        }
    }
    for (index, captured) in root.arguments.as_slice().iter().enumerate() {
        plan.charge_work(32)?;
        let argument = plan.argument(index)?;
        let source_ty = plan.argument_type(index)?;
        if captured.semantic_type_identity != source_ty.identity()
            || captured.semantic_layout_identity != source_ty.layout_identity()
            || source_ty.layout().size_bytes() != Some(captured.source_size)
            || source_ty.layout().alignment_bytes() != u64::from(captured.source_alignment)
            || source_class(source_ty)? != captured.rustc_abi_class
        {
            return Err(E::Mismatch("exact fresh rustc source type/layout capture"));
        }
        let nominal = source_ty.rust_type_kind();
        if captured.kind == DescriptorArgumentKindV1::CompilerLaidOutByValue {
            if captured.access != AccessMode::ByValue
                || captured.layout.is_some()
                || !matches!(
                    source_ty.shape(),
                    SemanticTypeShapeV1::Unit
                        | SemanticTypeShapeV1::Tuple(_)
                        | SemanticTypeShapeV1::Aggregate(_)
                        | SemanticTypeShapeV1::Array { .. }
                )
                || source.abi().source_argument_ownership().get(index)
                    != Some(&SemanticSourceArgumentOwnershipV1::ByValue)
            {
                return Err(E::Mismatch("captured aggregate by-value source ownership"));
            }
        } else {
            if argument.physical_slots().len() != 1 {
                return Err(E::Mismatch("whole scalar/descriptor physical coverage"));
            }
            let nominal_matches = match captured.kind {
                DescriptorArgumentKindV1::CompilerLaidOutUsize => {
                    nominal == SemanticRustTypeKindV1::Usize
                }
                DescriptorArgumentKindV1::CompilerLaidOutIsize => {
                    nominal == SemanticRustTypeKindV1::Isize
                }
                _ => !matches!(
                    nominal,
                    SemanticRustTypeKindV1::Usize | SemanticRustTypeKindV1::Isize
                ),
            };
            let row = plan.component(argument.physical_slots().start)?;
            if !nominal_matches
                || !super::checked_output_policy3_v1::nominal_policy4_v3::physical_matches(
                    captured.kind,
                    captured.access,
                    row.physical().ty(),
                )
            {
                return Err(E::Mismatch("captured scalar/descriptor source kind"));
            }
        }
    }
    Ok((logical, physical))
}

/// Replays exact owners and joins capture/source/entry facts only. The result is
/// inert counts, not an encoding, packing, native, artifact or launch permit.
pub(crate) fn check(
    owner: OwnerRef<'_>,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> R<SourceAbiSummaryV1> {
    visit_checked(owner, roots, profile, budget, None)
}

// Only a complete captured/source/entry join can invoke the private consumer.
// This is not a constructor from observer rows or detached argument maps.
fn visit_checked(
    owner: OwnerRef<'_>,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
    mut visitor: Option<&mut RootVisitor<'_>>,
) -> R<SourceAbiSummaryV1> {
    let floor = budget.storage();
    budget.reserve_storage(headers()?)?;
    let mut query = || {
        let required = match owner {
            OwnerRef::Direct(value) => value.retained_input_storage_floor_v1(),
            OwnerRef::Erased(value) => value.retained_input_storage_floor_v1(),
        }
        .map_err(E::Checked)?;
        if floor < required {
            return Err(Resource::Accounting.into());
        }
        match owner {
            OwnerRef::Direct(value) => value.verify_equivalence(budget),
            OwnerRef::Erased(value) => value.verify_equivalence(budget),
        }
        .map_err(E::Checked)?;
        let (semantic, endpoints, neutral, bound) = match owner {
            OwnerRef::Direct(value) => {
                let source = value.source_semantic_kir();
                let neutral = source
                    .pre_ranked_executable()
                    .ok_or(E::Mismatch("retained original entry"))?;
                (
                    source.semantic().semantic(),
                    [
                        source.module(),
                        neutral.module(),
                        value.bound().module(),
                        value.output().module(),
                    ],
                    neutral,
                    value.bound(),
                )
            }
            OwnerRef::Erased(value) => (
                value.original_source().semantic_ssa().source_semantic(),
                [
                    value.original_source().executable().module(),
                    value.erased().module(),
                    value.bound().module(),
                    value.output().module(),
                ],
                value.erased(),
                value.bound(),
            ),
        };
        dialect_amdgcn::check_production_target_coordinate_preservation_v1(
            neutral, bound, profile, budget,
        )
        .map_err(E::Target)?;
        if roots.is_empty() || roots.len() != semantic.roots().len() {
            return Err(E::Mismatch(
                "complete capture/source/endpoint root coverage",
            ));
        }
        for module in endpoints {
            if module.kernels.len() != roots.len() {
                return Err(E::Mismatch(
                    "complete capture/source/endpoint root coverage",
                ));
            }
        }
        let mut summary = SourceAbiSummaryV1 {
            roots: roots.len(),
            logical: 0,
            physical: 0,
        };
        for (ordinal, captured) in roots.iter().enumerate() {
            budget.charge_work(semantic.canonical_encoding().len())?;
            let root = semantic.roots()[ordinal];
            let selection = semantic
                .select_kernel_body_for_root_v1(root)
                .ok_or(E::Mismatch("exact source body selection"))?;
            let source_root = &semantic.functions()[root.index() as usize];
            let entry = source_root
                .kernel_entry()
                .ok_or(E::Mismatch("source kernel export"))?;
            budget.charge_work(captured.export_name.len())?;
            if captured.kernel_binding_bytes() != *entry.kernel_binding_identity().as_bytes()
                || captured.export_name.as_bytes() != entry.export_symbol().as_bytes()
            {
                return Err(E::Mismatch("captured original kernel binding/export"));
            }
            let source = &semantic.functions()[selection.body().index() as usize];
            let checked = with_plan(owner, root, selection.body(), budget, |plan| {
                let counts = check_root(plan, semantic, source, captured, ordinal, endpoints)?;
                if let Some(visitor) = visitor.as_mut() {
                    visitor(ordinal, captured, semantic, plan)?;
                }
                Ok(counts)
            })?;
            summary.logical = summary
                .logical
                .checked_add(checked.0)
                .ok_or(Resource::Arithmetic)?;
            summary.physical = summary
                .physical
                .checked_add(checked.1)
                .ok_or(Resource::Arithmetic)?;
        }
        Ok(summary)
    };
    let result = if std::mem::size_of_val(&query) > size_of::<CheckCapture<'_, '_>>()
        || std::mem::align_of_val(&query) > align_of::<CheckCapture<'_, '_>>()
    {
        drop(query);
        Err(Resource::Accounting.into())
    } else {
        let result = query();
        drop(query);
        result
    };
    budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?,
    )?;
    result
}

/// Structural selection only. Scalar-only legacy encoding keeps its existing
/// consumer; any aggregate uses the complete prerequisite with no retry.
pub(crate) fn check_if_aggregate(
    owner: OwnerRef<'_>,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> R<()> {
    let floor = budget.storage();
    let retained = selector_headers()?;
    budget.reserve_storage(retained)?;
    let mut query = || {
        budget.charge_work(roots.len())?;
        let mut selected = false;
        for root in roots {
            budget.charge_work(root.arguments.len())?;
            selected |=
                root.arguments.as_slice().iter().any(|argument| {
                    argument.kind == DescriptorArgumentKindV1::CompilerLaidOutByValue
                });
        }
        if selected {
            check(owner, roots, profile, budget)?;
        }
        Ok(())
    };
    let result = if std::mem::size_of_val(&query) > size_of::<CheckCapture<'_, '_>>()
        || std::mem::align_of_val(&query) > align_of::<CheckCapture<'_, '_>>()
    {
        drop(query);
        Err(Resource::Accounting.into())
    } else {
        let result = query();
        drop(query);
        result
    };
    if budget
        .storage()
        .checked_sub(floor)
        .is_none_or(|live| live < retained)
    {
        return Err(Resource::Accounting.into());
    }
    budget.release_storage(retained)?;
    result
}

fn selector_headers() -> R<usize> {
    sum_frames(&[
        frame::<CheckFrame<'_, '_>>(),
        frame::<CheckCapture<'_, '_>>(),
        frame::<std::slice::Iter<'_, TypedDescriptorRootV1>>(),
        frame::<Option<&TypedDescriptorRootV1>>(),
        frame::<&TypedDescriptorRootV1>(),
        frame::<std::slice::Iter<'_, TypedDescriptorArgumentV1>>(),
        frame::<Option<&TypedDescriptorArgumentV1>>(),
        frame::<&TypedDescriptorArgumentV1>(),
        frame::<&[TypedDescriptorArgumentV1]>(),
        frame::<SourceAbiSummaryV1>(),
        frame::<bool>(),
        frame::<()>(),
        8 * frame::<usize>(),
    ])
}
