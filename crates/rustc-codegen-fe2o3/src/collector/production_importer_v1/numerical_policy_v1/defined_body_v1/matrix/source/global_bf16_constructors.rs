//! Private source-authenticated constructor coordinates. These retain the
//! checked Result body; they do not prove its successful path or any memory read.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{InertSemanticMirSha256V1, SemanticMfmaOperandRoleV1};

const A: &str = "fe2o3_device::matrix::PolicyMatrixCapability::bf16_a_global_row_major";
const B: &str = "fe2o3_device::matrix::PolicyMatrixCapability::bf16_b_global_row_major";
const CHECKED: &str = "fe2o3_device::tensor::GlobalBf16MfmaMatrix::checked";

fn kind(
    tcx: TyCtxt<'_>,
    definition: rustc_hir::def_id::DefId,
) -> Option<SemanticMfmaOperandRoleV1> {
    if tcx.def_kind(definition) != rustc_hir::def::DefKind::AssocFn {
        return None;
    }
    let implementation = tcx.impl_of_assoc(definition)?;
    let owner = tcx.type_of(implementation).instantiate_identity();
    let TyKind::Adt(owner, _) = owner.kind() else {
        return None;
    };
    if trusted_device_items::classify(tcx, owner.did())
        != Some(TrustedDeviceItem::PolicyMatrixCapability)
    {
        return None;
    }
    // Nominal-owner filtering is only a census optimization. Both the precise
    // provider definition and its reviewed source closure remain mandatory.
    [
        (A, SemanticMfmaOperandRoleV1::A),
        (B, SemanticMfmaOperandRoleV1::B),
    ]
    .into_iter()
    .find_map(|(path, role)| {
        trusted_device_items::is_exact_reviewed_provider_definition_v1(tcx, definition, path)
            .then_some(role)
    })
}

struct SourceConstructor<'tcx> {
    pair: Bind<'tcx>,
    inputs: [Ty<'tcx>; 6],
    global: Ty<'tcx>,
    matrix: Ty<'tcx>,
    error: Ty<'tcx>,
    result: Ty<'tcx>,
    checked: Instance<'tcx>,
    role: SemanticMfmaOperandRoleV1,
}

fn validate_source<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<Option<SourceConstructor<'tcx>>, ProductionSemanticImportErrorV1> {
    let Some(role) = kind(tcx, instance.def_id()) else {
        return Ok(None);
    };
    if !trusted_device_items::authenticate_reviewed_safe_external_helper_v1(tcx, instance.def_id())
        .map_err(|_| rejected("BF16 constructor original provider authentication"))?
    {
        return Err(rejected(
            "BF16 constructor requires the reviewed original provider",
        ));
    }
    let signature = super::signature(tcx, instance)?;
    let inputs: [Ty<'tcx>; 6] = signature
        .inputs()
        .try_into()
        .map_err(|_| rejected("BF16 constructor six original inputs"))?;
    let bound = rust_shared_reference_v1(inputs[0])
        .ok_or_else(|| rejected("BF16 constructor shared policy pair receiver"))?;
    rust_trusted_adt_type_arguments_v1(tcx, bound, TrustedDeviceItem::PolicyMatrixCapability)
        .ok_or_else(|| rejected("BF16 constructor exact nominal policy pair"))?;
    // Root DeviceMatrix wrappers remain on their existing route. This roster
    // contains only the scoped MatrixAccess/Bind source contract, never Narrow.
    let fields = super::fields(tcx, bound)?;
    if !fields
        .first()
        .and_then(|ty| rust_shared_reference_v1(*ty))
        .is_some_and(|ty| rust_matrix_capability_v1(tcx, ty).is_some())
    {
        return Ok(None);
    }
    let pair = super::pair(tcx, bound)?;
    if inputs[2..] != [tcx.types.usize; 4]
        || instance.args.types().collect::<Vec<_>>()
            != [
                pair.identity.matrix_brand,
                pair.identity.root.ty,
                pair.identity.policy,
            ]
    {
        return Err(rejected("BF16 constructor exact index and instance axes"));
    }
    let global = rust_shared_reference_v1(inputs[1])
        .ok_or_else(|| rejected("BF16 constructor shared Global reference"))?;
    let memory = rust_capability_memory_view_v1(tcx, global)
        .ok_or_else(|| rejected("BF16 constructor exact Global source"))?;
    if memory.element != tcx.types.u16
        || !matches!(memory.role, RustCapabilityMemoryRoleV1::ReadOnly)
        || memory.brand.ty != pair.identity.root.ty
    {
        return Err(rejected(
            "BF16 constructor ReadOnly u16 and exact Global root",
        ));
    }
    let result = signature.output();
    let (matrix, error) = rust_result_payloads_v1(tcx, result)
        .ok_or_else(|| rejected("BF16 constructor retains Result and both payloads"))?;
    let arguments = rust_trusted_adt_type_arguments_v1(
        tcx,
        matrix,
        TrustedDeviceItem::Bf16MfmaGlobalMatrixView,
    )
    .ok_or_else(|| rejected("BF16 constructor exact Global matrix result"))?;
    let [role_ty, matrix_brand, global_brand] = arguments.as_slice() else {
        return Err(rejected("BF16 constructor exact matrix type arity"));
    };
    let role_marker = match role {
        SemanticMfmaOperandRoleV1::A => TrustedDeviceItem::MfmaOperandA,
        SemanticMfmaOperandRoleV1::B => TrustedDeviceItem::MfmaOperandB,
    };
    if !rust_is_exact_trusted_marker_v1(tcx, *role_ty, role_marker)
        || *matrix_brand != pair.identity.matrix_brand
        || *global_brand != pair.identity.root.ty
        || !rust_is_exact_trusted_marker_v1(tcx, error, TrustedDeviceItem::Bf16MfmaMatrixViewError)
    {
        return Err(rejected(
            "BF16 constructor role, brands or error payload changed",
        ));
    }
    let original = tcx.instance_mir(instance.def);
    let checked = super::super::super::source::forwarding_callee(tcx, instance, original)?;
    if !trusted_device_items::is_exact_reviewed_provider_definition_v1(
        tcx,
        checked.def_id(),
        CHECKED,
    ) || !trusted_device_items::authenticate_reviewed_safe_external_helper_v1(
        tcx,
        checked.def_id(),
    )
    .map_err(|_| rejected("BF16 checked provider authentication"))?
    {
        return Err(rejected("BF16 constructor original checked provider body"));
    }
    let checked_signature = super::signature(tcx, checked)?;
    if checked_signature.inputs() != &inputs[1..]
        || checked_signature.output() != result
        || checked.args.types().collect::<Vec<_>>() != arguments
        || !global_views::body::constructor(tcx, instance, original, inputs, result, checked)
    {
        return Err(rejected(
            "BF16 constructor exact forwarding body and checked ABI",
        ));
    }
    Ok(Some(SourceConstructor {
        pair,
        inputs,
        global,
        matrix,
        error,
        result,
        checked,
        role,
    }))
}

// No public constructor or serialized representation. The complete canonical
// subject below is retained separately from nominal function identities.
pub(in crate::collector::production_importer_v1)
struct ConstructorRowV1
{
    root: SemanticFunctionIdV1,
    wrapper: SemanticFunctionIdV1,
    checked: SemanticFunctionIdV1,
    identity: SemanticDefinedMatrixIdentityV1,
    role: SemanticMfmaOperandRoleV1,
    inputs: [SemanticTypeIdV1; 6],
    bound_types: [SemanticTypeIdV1; 5],
    // Global, matrix payload, error payload, exact Result.
    output_types: [SemanticTypeIdV1; 4],
}

pub(in crate::collector::production_importer_v1)
struct ConstructorRosterV1
{
    subject: InertSemanticMirSha256V1,
    rows: Vec<ConstructorRowV1>,
}

impl ConstructorRosterV1 {
    pub(in crate::collector::production_importer_v1::numerical_policy_v1::defined_body_v1::matrix) fn find(
        &self,
        subject: InertSemanticMirSha256V1,
        root: SemanticFunctionIdV1,
        wrapper: SemanticFunctionIdV1,
        checked: SemanticFunctionIdV1,
    ) -> Option<&ConstructorRowV1> {
        if subject != self.subject {
            return None;
        }
        let index = self
            .rows
            .binary_search_by_key(&wrapper.index(), |row| row.wrapper.index())
            .ok()?;
        let row = &self.rows[index];
        (row.root == root && row.checked == checked).then_some(row)
    }
}

pub(in crate::collector::production_importer_v1) fn capture<
    'tcx,
>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    contexts: &AuthenticatedProductionKernelContextsV1,
    mir: &AdmittedInertSemanticMirV1,
) -> Result<ConstructorRosterV1, ProductionSemanticImportErrorV1> {
    let rows = validate_canonical(
        tcx,
        plan,
        mir.types(),
        mir.functions(),
        mir.callables(),
        contexts,
    )?;
    Ok(ConstructorRosterV1 {
        subject: mir.semantic_sha256(),
        rows,
    })
}

fn validate_canonical<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    types: &[SemanticTypeDeclV1],
    functions: &[SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<Vec<ConstructorRowV1>, ProductionSemanticImportErrorV1> {
    let candidates = plan
        .function_producers()
        .iter()
        .enumerate()
        .filter_map(|(index, producer)| {
            kind(tcx, producer.instance.def_id())
                .map(|_| SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    // Existing complete native type/ABI/identity reconstruction, not digest-only
    // matching. Replay below shares one existing body-request budget owner.
    let roster = roster::Roster::new(tcx, plan, types, functions, callables)?;
    let edges = plan
        .direct_call_producers()
        .iter()
        .map(|call| (call.caller, call.callee))
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut rows = Vec::new();
    rows.try_reserve_exact(candidates.len())
        .map_err(|_| rejected("BF16 constructor roster storage reservation"))?;
    for function in candidates {
        let instance = roster.defined(function)?;
        let Some(facts) = validate_source(tcx, instance)? else {
            continue;
        };
        let checked = roster.defined_instance(facts.checked)?;
        let root = authenticate_capability_memory_root_v1(
            contexts,
            &BTreeSet::from([function]),
            &edges,
            false,
        )?;
        roster.root(root, contexts)?;
        let identity = super::super::identity(tcx, &facts.pair.identity, root, contexts)?;
        let inputs = roster.types(facts.inputs)?;
        let bound_types = roster.types(facts.pair.types)?;
        let output_types = roster.types([facts.global, facts.matrix, facts.error, facts.result])?;
        let calls = plan
            .direct_call_producers()
            .iter()
            .filter(|call| call.caller == function)
            .collect::<Vec<_>>();
        if !matches!(calls.as_slice(), [call] if call.block == 0 && call.callee == checked)
            || plan
                .terminal_expansion_producers()
                .iter()
                .any(|call| call.caller == function)
            || plan
                .normalized_intrinsic_producers()
                .iter()
                .any(|call| call.caller == function)
            || functions[function.index() as usize]
                .defined_capability_contract()
                .is_some()
            || functions[checked.index() as usize]
                .defined_capability_contract()
                .is_some()
        {
            return Err(rejected(
                "BF16 constructor original call and defined-body roster",
            ));
        }
        let abi = functions[function.index() as usize].abi();
        let checked_abi = functions[checked.index() as usize].abi();
        if abi.source_input_types() != inputs
            || abi.source_output_type() != output_types[3]
            || abi.source_argument_ownership()
                != [
                    SemanticSourceArgumentOwnershipV1::SharedBorrow,
                    SemanticSourceArgumentOwnershipV1::SharedBorrow,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                ]
            || checked_abi.source_input_types() != &inputs[1..]
            || checked_abi.source_output_type() != output_types[3]
            || checked_abi.source_argument_ownership()
                != [
                    SemanticSourceArgumentOwnershipV1::SharedBorrow,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                ]
        {
            return Err(rejected(
                "BF16 constructor canonical ABI or ownership changed",
            ));
        }
        selected.extend([function, checked]);
        rows.push(ConstructorRowV1 {
            root: root.selected_root,
            wrapper: function,
            checked,
            identity,
            role: facts.role,
            inputs,
            bound_types,
            output_types,
        });
    }
    if !selected.is_empty() {
        global_views::replay::validate(tcx, plan, types, functions, selected)?;
    }
    Ok(rows)
}

#[cfg(test)]
mod tests;

mod source_join;

#[cfg(test)]
pub(in crate::collector::production_importer_v1::numerical_policy_v1::defined_body_v1::matrix) use tests::check_import;
