//! Authenticated source facts for policy-bound scalar FP consumers.
//!
//! These facts are not a bare MathF32 operation or a refinement certificate.
//! The parent must encode them in a policy-bearing semantic operation before
//! admitting the terminal; see PARENT_HOOKS.md.

use super::*;
use rustc_middle::ty::{InstanceKind, TypeVisitableExt, TypingEnv};

mod contract;
#[cfg(test)]
mod import_tests;
pub(in super::super) use contract::PolicyMathSourceContractV1;
use contract::PolicyMathTypesV1;

#[derive(Clone, Copy, Debug)]
pub(super) struct RustPolicyMathConsumerV1<'tcx> {
    pub function: fe2o3_kernel_ir::F32MathFunction,
    pub kernel_brand: Ty<'tcx>,
    pub policy: Ty<'tcx>,
    // Order agrees with PolicyMathTypesV1::new.
    pub types: [Ty<'tcx>; 7],
}

pub(super) fn validate_policy_math_source_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    expected: fe2o3_kernel_ir::F32MathFunction,
) -> Result<RustPolicyMathConsumerV1<'tcx>, ProductionSemanticImportErrorV1> {
    use dialect_amdgcn::DeviceMathDiagnosticItem;
    if expected == fe2o3_kernel_ir::F32MathFunction::Abs
        || trusted_device_items::classify(tcx, instance.def_id())
            != Some(TrustedDeviceItem::DeviceMath(
                DeviceMathDiagnosticItem::F32(expected),
            ))
        || !matches!(instance.def, InstanceKind::Item(_))
        || instance.args.len() != tcx.generics_of(instance.def_id()).count()
        || instance.args.consts().next().is_some()
    {
        return Err(rejected(
            "policy math requires its exact authenticated FP32 terminal",
        ));
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    let signature = tcx
        .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
        .map_err(|_| rejected("policy math signature normalization"))?;
    if signature.safety != rustc_hir::Safety::Safe
        || signature.abi != rustc_abi::ExternAbi::Rust
        || signature.c_variadic
        || signature.has_non_region_param()
        || signature.has_infer()
        || signature.has_aliases()
        || signature.has_escaping_bound_vars()
        || signature.inputs().len() != expected.arity() + 1
        || signature.inputs()[1..]
            .iter()
            .any(|ty| *ty != tcx.types.f32)
        || signature.output() != tcx.types.f32
    {
        return Err(rejected(
            "policy math requires an exact safe FP32 Rust signature",
        ));
    }
    let reference = signature.inputs()[0];
    let bound = rust_shared_reference_v1(reference)
        .ok_or_else(|| rejected("policy math requires a shared wrapper borrow"))?;
    let arguments =
        rust_trusted_adt_type_arguments_v1(tcx, bound, TrustedDeviceItem::PolicyMathCapability)
            .ok_or_else(|| rejected("policy math requires the authenticated policy wrapper"))?;
    let [brand, policy] = arguments.as_slice() else {
        return Err(rejected("policy math wrapper type arity"));
    };
    if instance.args.types().collect::<Vec<_>>() != [*brand, *policy] {
        return Err(rejected("policy math substituted instance or kernel brand"));
    }
    let root_brand = rust_kernel_brand_v1(tcx, *brand)
        .ok_or_else(|| rejected("policy math requires an authenticated kernel brand"))?;
    for (axis, path) in [
        (root_brand.target, "fe2o3_device::context::CurrentTarget"),
        (root_brand.launch, "fe2o3_device::context::RegisteredLaunch"),
    ] {
        if !rust_exact_reviewed_adt_arguments_v1(tcx, axis, path)
            .is_some_and(|arguments| arguments.is_empty())
        {
            return Err(rejected("policy math substituted target or launch brand"));
        }
    }
    let TyKind::Adt(policy_definition, policy_arguments) = *policy.kind() else {
        return Err(rejected(
            "policy math requires the authenticated strict policy",
        ));
    };
    if trusted_device_items::classify(tcx, policy_definition.did())
        != Some(TrustedDeviceItem::StrictIeeeNumericalPolicy)
        || !policy_definition.is_enum()
        || !policy_definition.variants().is_empty()
        || !policy_arguments.is_empty()
    {
        return Err(rejected(
            "policy math does not admit an unknown numerical policy",
        ));
    }
    let TyKind::Adt(definition, arguments) = *bound.kind() else {
        return Err(rejected("policy math wrapper must be an aggregate"));
    };
    if !definition.is_struct() || definition.non_enum_variant().fields.len() != 3 {
        return Err(rejected("policy math wrapper field arity"));
    }
    let fields = definition
        .non_enum_variant()
        .fields
        .iter()
        .map(|field| {
            tcx.try_normalize_erasing_regions(
                TypingEnv::fully_monomorphized(),
                field.ty(tcx, arguments),
            )
            .map_err(|_| rejected("policy math field normalization"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let math = rust_shared_reference_v1(fields[0])
        .ok_or_else(|| rejected("policy math must retain a shared math reference"))?;
    let capability = rust_shared_reference_v1(fields[1])
        .ok_or_else(|| rejected("policy math must retain a shared policy reference"))?;
    if rust_trusted_adt_type_arguments_v1(
        tcx,
        math,
        TrustedDeviceItem::DeviceMath(DeviceMathDiagnosticItem::Context),
    ) != Some(vec![*brand])
        || rust_trusted_adt_type_arguments_v1(
            tcx,
            capability,
            TrustedDeviceItem::NumericalPolicyCapability,
        ) != Some(vec![*brand, *policy])
        || !matches!(fields[2].kind(), TyKind::Adt(definition, _)
            if Some(definition.did()) == tcx.lang_items().phantom_data())
    {
        return Err(rejected(
            "policy math substituted retained capability, brand, or policy",
        ));
    }
    Ok(RustPolicyMathConsumerV1 {
        function: expected,
        kernel_brand: *brand,
        policy: *policy,
        types: [
            reference,
            bound,
            fields[0],
            math,
            fields[1],
            capability,
            tcx.types.f32,
        ],
    })
}

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn policy_math_source_contract_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    function: fe2o3_kernel_ir::F32MathFunction,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    root: Option<&AuthenticatedProductionKernelContextRootV1>,
    source_identity: SemanticFunctionIdentityV1,
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<PolicyMathSourceContractV1, ProductionSemanticImportErrorV1> {
    let source = validate_policy_math_source_v1(tcx, instance, function)?;
    let root = root.ok_or_else(|| rejected("policy math lacks authenticated root custody"))?;
    let brand = rust_kernel_brand_v1(tcx, source.kernel_brand)
        .ok_or_else(|| rejected("policy math kernel brand"))?;
    if !rust_kernel_brand_matches_root_v1(tcx, brand, root)
        || canonical_function_identities_v1(tcx, instance).function() != source_identity
    {
        return Err(rejected("policy math substituted root or source identity"));
    }
    let mut inputs = vec![source.types[0]];
    inputs.extend(std::iter::repeat_n(tcx.types.f32, function.arity()));
    let mut ownership = vec![SemanticSourceArgumentOwnershipV1::SharedBorrow];
    ownership.extend(std::iter::repeat_n(
        SemanticSourceArgumentOwnershipV1::ByValue,
        function.arity(),
    ));
    require_capability_memory_terminal_abi_v1(tcx, abi, types, &inputs, tcx.types.f32, &ownership)?;
    if abi
        .arguments()
        .iter()
        .any(|argument| !matches!(argument.value().mode(), SemanticAbiPassModeV1::Direct(_)))
        || !matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Direct(_))
    {
        return Err(rejected(
            "policy math requires direct reference and FP32 ABI values",
        ));
    }
    let mut ids = [SemanticTypeIdV1::from_index(0); 7];
    for (id, ty) in ids.iter_mut().zip(source.types) {
        *id = semantic_type_for_rust_v1(tcx, types, ty)?;
    }
    PolicyMathSourceContractV1::new(
        PolicyMathTypesV1::new(ids),
        rustc_type_identity_v1(tcx, source.policy),
        rustc_type_identity_v1(tcx, source.kernel_brand),
        source.function,
        capability_memory_provenance_v1(root, contexts)?,
        source_identity,
        types,
    )
    .map_err(rejected)
}
