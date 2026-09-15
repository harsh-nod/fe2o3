//! Exact source-to-semantic expansion of compiler-issued strict numerical policy.
//!
//! Issuance retains an obligation on subsequent policy-bound operations. It does
//! not certify their numerical refinement or their target implementation.

use super::*;
mod bind;
pub(super) mod defined_body_v1;
pub(super) fn defined_source_roster_v1<'a, 'tcx>(
    tcx: TyCtxt<'tcx>, plan: &'a ProductionSemanticPreflightPlanV1<'tcx>,
    types: &'a [SemanticTypeDeclV1], functions: &'a [SemanticFunctionDeclV1], callables: &'a [SemanticCallableDeclV1],
) -> Result<defined_body_v1::DefinedSourceRosterV1<'a, 'tcx>, ProductionSemanticImportErrorV1> {
    defined_body_v1::DefinedSourceRosterV1::new(tcx, plan, types, functions, callables)
}

// Reuse the same exact Matrix phase-to-allocation-root relation for transpose.
pub(super) fn transpose_global_root_v1<'tcx>(tcx: TyCtxt<'tcx>, brand: Ty<'tcx>) -> Option<Ty<'tcx>> {
    bind::matrix_global_brand(tcx, brand, 0)
}

mod math;
pub(super) use bind::validate_policy_bind_source_v1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticFunctionDeclV1, SemanticMutabilityV1, SemanticPointerKindV1, SemanticPointerMetadataV1,
};
pub(super) use math::policy_math_source_contract_v1;

#[allow(clippy::too_many_arguments)]
pub(super) fn policy_math_terminal_operation_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    function: fe2o3_kernel_ir::F32MathFunction,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    root: Option<&AuthenticatedProductionKernelContextRootV1>,
    source_identity: SemanticFunctionIdentityV1,
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticF32MathImplementationV1 as Implementation,
        SemanticNumericalModeV1 as Mode, SemanticNumericalPolicyMathContractV1,
        SemanticNumericalPolicyMathTypesV1,
    };
    let source = policy_math_source_contract_v1(
        tcx, instance, function, abi, types, root, source_identity, contexts,
    )?;
    let (mode, implementation) = source.numerical_requirements();
    let mode = match mode {
        fe2o3_kernel_ir::NumericalModeV1::StrictIeee => Mode::StrictIeee,
        _ => return Err(rejected("policy math mode has no exact semantic representation")),
    };
    let implementation = match implementation {
        fe2o3_kernel_ir::F32MathImplementation::ConstrainedLlvm => Implementation::ConstrainedLlvm,
        fe2o3_kernel_ir::F32MathImplementation::OcmlAbiV1 => Implementation::OcmlAbiV1,
        fe2o3_kernel_ir::F32MathImplementation::IeeeSqrtRoundTiesEvenIgnoreExceptionsV1 =>
            Implementation::IeeeSqrtRoundTiesEvenIgnoreExceptionsV1,
        fe2o3_kernel_ir::F32MathImplementation::IeeeFabsV1 =>
            return Err(rejected("fabs is not a policy FP32 terminal")),
    };
    let contract = SemanticNumericalPolicyMathContractV1::new(
        SemanticNumericalPolicyMathTypesV1::new(source.types().all()),
        source.policy(), source.kernel_brand(), semantic_f32_math_function_v1(source.function()),
        mode, implementation, source.provenance(), source.source_identity(),
    ).map_err(|_| rejected("policy math canonical consumer contract"))?;
    Ok(SemanticCompilerIntrinsicOperationV1::PolicyMathF32 { contract })
}

/// Observations made from the authenticated rustc instance, never source names.
struct NumericalPolicySourceV1 {
    terminal: Option<TrustedDeviceItem>,
    policy_marker: Option<TrustedDeviceItem>,
    source_identity: SemanticFunctionIdentityV1,
    context_reference: SemanticTypeIdentityV1,
    context: SemanticTypeIdentityV1,
    capability: SemanticTypeIdentityV1,
    policy: SemanticTypeIdentityV1,
    context_axes: [SemanticTypeIdentityV1; 3],
    capability_axes: [SemanticTypeIdentityV1; 3],
    instance_arguments: [SemanticTypeIdentityV1; 4],
}

#[allow(clippy::too_many_arguments)]
pub(super) fn numerical_policy_terminal_operation_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    root: Option<&AuthenticatedProductionKernelContextRootV1>,
    source_identity: SemanticFunctionIdentityV1,
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    let root = root.ok_or(ProductionSemanticImportErrorV1::KernelContextBinding(
        "numerical-policy issuance lacks authenticated root custody",
    ))?;
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    if signature.safety != rustc_hir::Safety::Safe
        || signature.abi != rustc_abi::ExternAbi::Rust
        || signature.c_variadic
        || instance.args.consts().next().is_some()
    {
        return Err(rejected("numerical-policy source signature"));
    }
    let [receiver] = signature.inputs() else {
        return Err(rejected("numerical-policy source arity"));
    };
    require_capability_memory_terminal_abi_v1(
        tcx,
        abi,
        types,
        signature.inputs(),
        signature.output(),
        &[SemanticSourceArgumentOwnershipV1::SharedBorrow],
    )?;
    let context = rust_shared_reference_v1(*receiver)
        .ok_or_else(|| rejected("numerical-policy shared context borrow"))?;
    let (kernel, target, launch) = rust_kernel_context_axes_v1(tcx, context)
        .ok_or_else(|| rejected("numerical-policy trusted context type"))?;
    for (axis, path) in [
        (target, "fe2o3_device::context::CurrentTarget"),
        (launch, "fe2o3_device::context::RegisteredLaunch"),
    ] {
        if !rust_exact_reviewed_adt_arguments_v1(tcx, axis, path)
            .is_some_and(|arguments| arguments.is_empty())
        {
            return Err(rejected(
                "numerical-policy authenticated target or launch marker",
            ));
        }
    }
    let output_arguments = rust_trusted_adt_type_arguments_v1(
        tcx,
        signature.output(),
        TrustedDeviceItem::NumericalPolicyCapability,
    )
    .ok_or_else(|| rejected("numerical-policy trusted capability type"))?;
    let [brand, policy] = output_arguments.as_slice() else {
        return Err(rejected("numerical-policy capability type arity"));
    };
    let brand = rust_kernel_brand_v1(tcx, *brand)
        .ok_or_else(|| rejected("numerical-policy exact kernel brand"))?;
    let TyKind::Adt(policy_definition, policy_arguments) = *policy.kind() else {
        return Err(rejected("numerical-policy trusted policy marker"));
    };
    if !policy_arguments.is_empty()
        || !policy_definition.is_enum()
        || !policy_definition.variants().is_empty()
    {
        return Err(rejected("numerical-policy policy marker arity"));
    }
    let instance_arguments = instance.args.types().collect::<Vec<_>>();
    let [
        instance_kernel,
        instance_target,
        instance_launch,
        instance_policy,
    ] = instance_arguments.as_slice()
    else {
        return Err(rejected("numerical-policy instance type arity"));
    };
    let identity = |ty| rustc_type_identity_v1(tcx, ty);
    let observed = NumericalPolicySourceV1 {
        terminal: trusted_device_items::classify(tcx, instance.def_id()),
        policy_marker: trusted_device_items::classify(tcx, policy_definition.did()),
        source_identity: canonical_function_identities_v1(tcx, instance).function(),
        context_reference: identity(*receiver),
        context: identity(context),
        capability: identity(signature.output()),
        policy: identity(*policy),
        context_axes: [kernel, target, launch].map(identity),
        capability_axes: [brand.kernel, brand.target, brand.launch].map(identity),
        instance_arguments: [
            *instance_kernel,
            *instance_target,
            *instance_launch,
            *instance_policy,
        ]
        .map(identity),
    };
    expand_numerical_policy_issue_v1(
        abi,
        types,
        &observed,
        capability_memory_provenance_v1(root, contexts)?,
        source_identity,
    )
}

fn rejected(detail: &'static str) -> ProductionSemanticImportErrorV1 {
    ProductionSemanticImportErrorV1::KernelContextBinding(detail)
}

fn expand_numerical_policy_issue_v1(
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    source: &NumericalPolicySourceV1,
    provenance: SemanticKernelCapabilityProvenanceV1,
    source_identity: SemanticFunctionIdentityV1,
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    if source.terminal != Some(TrustedDeviceItem::NumericalPolicyIssue)
        || source.policy_marker != Some(TrustedDeviceItem::StrictIeeeNumericalPolicy)
        || source.source_identity != source_identity
        || source_identity.as_bytes() == &[0; 32]
        || source.policy.as_bytes() == &[0; 32]
        || [source.context_reference, source.context, source.capability].contains(&source.policy)
    {
        return Err(rejected(
            "numerical-policy source or strict policy identity",
        ));
    }
    if source.context_axes != source.capability_axes
        || source.instance_arguments
            != [
                source.context_axes[0],
                source.context_axes[1],
                source.context_axes[2],
                source.policy,
            ]
        || source.context_axes[0] != provenance.kernel_marker()
        || source
            .context_axes
            .iter()
            .any(|axis| axis.as_bytes() == &[0; 32])
    {
        return Err(rejected(
            "numerical-policy kernel, target, launch, or policy substitution",
        ));
    }
    let [context_reference] = abi.source_input_types() else {
        return Err(rejected("numerical-policy semantic source arity"));
    };
    let capability = abi.source_output_type();
    if abi.canon_abi() != SemanticCanonAbiV1::Rust
        || abi.extern_abi() != SemanticExternAbiV1::Rust
        || abi.can_unwind()
        || abi.c_variadic()
        || abi.fixed_count() != 1
        || abi.source_argument_ownership() != [SemanticSourceArgumentOwnershipV1::SharedBorrow]
        || !abi.hidden_arguments().is_empty()
        || abi.arguments().len() != 1
        || abi.adjusted_arguments().len() != 1
        || abi.return_value().ty() != capability
        || !matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Ignore)
        || abi.return_value().adjusted().is_some()
        || abi.return_value().pointee_override().is_some()
    {
        return Err(rejected("numerical-policy semantic FnAbi"));
    }
    let argument = &abi.arguments()[0];
    if argument.role() != SemanticAbiArgumentRoleV1::Source
        || argument.ty() != *context_reference
        || !matches!(argument.value().mode(), SemanticAbiPassModeV1::Direct(_))
        || argument.value().adjusted().is_some()
        || argument.value().pointee_override().is_some()
    {
        return Err(rejected("numerical-policy context physical ABI"));
    }
    let exact_type = |ty: SemanticTypeIdV1, identity| {
        identity != SemanticTypeIdentityV1::from_sha256([0; 32])
            && types
                .get(ty.index() as usize)
                .is_some_and(|decl| decl.identity() == identity)
            && types
                .iter()
                .filter(|decl| decl.identity() == identity)
                .count()
                == 1
    };
    let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
        .get(context_reference.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Err(rejected("numerical-policy context reference type"));
    };
    if pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.mutability() != SemanticMutabilityV1::Immutable
        || pointer.metadata() != SemanticPointerMetadataV1::None
    {
        return Err(rejected("numerical-policy context shared reference shape"));
    }
    let context = pointer.pointee();
    if !exact_type(*context_reference, source.context_reference)
        || !exact_type(context, source.context)
        || !exact_type(capability, source.capability)
        || !semantic_exact_inhabited_aggregate_zst_v1(types, context)
        || !semantic_exact_inhabited_aggregate_zst_v1(types, capability)
        || types[capability.index() as usize]
            .layout()
            .alignment_bytes()
            != 1
    {
        return Err(rejected(
            "numerical-policy exact context or capability type/layout",
        ));
    }
    let contract = SemanticExecutionCapabilityContractV1::new_kernel_scoped(
        SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue {
            context: *context_reference,
            capability,
            policy: source.policy,
        },
        SemanticExecutionCapabilitySignatureV1::new(&[*context_reference], capability)
            .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?,
        provenance,
        source_identity,
    )
    .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?;
    Ok(SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract })
}

#[cfg(test)]
mod tests;
