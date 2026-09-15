use super::*;
use rustc_middle::mir::{Body, Operand, START_BLOCK, TerminatorKind};
use rustc_middle::ty::{EarlyBinder, FnSig, InstanceKind, TypeVisitableExt, TypingEnv};

const GETTER: &str = "fe2o3_device::context::KernelContext::math";
const BRIDGE: &str = "fe2o3_device::math::DeviceMath::current_branded";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Kind {
    Getter,
    Bind,
}

pub(super) fn kind(tcx: TyCtxt<'_>, instance: Instance<'_>) -> Option<Kind> {
    if trusted_device_items::classify(tcx, instance.def_id())
        == Some(TrustedDeviceItem::PolicyMathBind)
    {
        Some(Kind::Bind)
    } else if trusted_device_items::is_exact_reviewed_provider_definition_v1(
        tcx,
        instance.def_id(),
        GETTER,
    ) {
        Some(Kind::Getter)
    } else {
        None
    }
}

pub(super) struct Getter<'tcx> {
    pub bridge: Instance<'tcx>,
    pub current: Instance<'tcx>,
    pub brand: RustKernelBrandV1<'tcx>,
    pub types: [Ty<'tcx>; 4],
}

pub(super) struct Bind<'tcx> {
    pub brand: RustKernelBrandV1<'tcx>,
    pub policy: Ty<'tcx>,
    pub types: [Ty<'tcx>; 5],
}

fn signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<FnSig<'tcx>, ProductionSemanticImportErrorV1> {
    if !matches!(instance.def, InstanceKind::Item(_))
        || instance.args.len() != tcx.generics_of(instance.def_id()).count()
        || instance.args.consts().next().is_some()
    {
        return Err(rejected("defined Math requires an exact Item instance"));
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    let signature = tcx
        .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
        .map_err(|_| rejected("defined Math signature normalization"))?;
    if signature.safety != rustc_hir::Safety::Safe
        || signature.abi != rustc_abi::ExternAbi::Rust
        || signature.c_variadic
        || signature.has_non_region_param()
        || signature.has_infer()
        || signature.has_aliases()
        || signature.has_escaping_bound_vars()
    {
        return Err(rejected(
            "defined Math requires a safe monomorphic Rust signature",
        ));
    }
    Ok(signature)
}

fn reviewed_helper(
    tcx: TyCtxt<'_>,
    instance: Instance<'_>,
    path: &str,
) -> Result<(), ProductionSemanticImportErrorV1> {
    if !trusted_device_items::is_exact_reviewed_provider_definition_v1(tcx, instance.def_id(), path)
        || !trusted_device_items::authenticate_reviewed_safe_external_helper_v1(
            tcx,
            instance.def_id(),
        )
        .map_err(|_| rejected("defined Math external helper authentication"))?
        || !matches!(instance.def, InstanceKind::Item(_))
        || !tcx.is_mir_available(instance.def_id())
    {
        return Err(rejected(
            "defined Math requires its exact reviewed external helper body",
        ));
    }
    Ok(())
}

pub(super) fn forwarding_callee<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
) -> Result<Instance<'tcx>, ProductionSemanticImportErrorV1> {
    let entry = body
        .basic_blocks
        .get(START_BLOCK)
        .ok_or_else(|| rejected("defined Math missing original entry"))?;
    let Some(terminator) = &entry.terminator else {
        return Err(rejected("defined Math missing original call"));
    };
    let TerminatorKind::Call {
        func: Operand::Constant(callee),
        ..
    } = &terminator.kind
    else {
        return Err(rejected(
            "defined Math requires an original constant callee",
        ));
    };
    let ty = instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(callee.const_.ty()),
        )
        .map_err(|_| rejected("defined Math callee normalization"))?;
    let TyKind::FnDef(definition, arguments) = *ty.kind() else {
        return Err(rejected("defined Math callee is not an exact FnDef"));
    };
    Instance::try_resolve(
        tcx,
        TypingEnv::fully_monomorphized(),
        definition,
        tcx.erase_and_anonymize_regions(arguments),
    )
    .ok()
    .flatten()
    .ok_or_else(|| rejected("defined Math callee instance resolution"))
}

fn root_brand<'tcx>(
    tcx: TyCtxt<'tcx>,
    brand: Ty<'tcx>,
) -> Result<RustKernelBrandV1<'tcx>, ProductionSemanticImportErrorV1> {
    let brand = rust_kernel_brand_v1(tcx, brand)
        .ok_or_else(|| rejected("defined Math exact kernel brand"))?;
    for (axis, path) in [
        (brand.target, "fe2o3_device::context::CurrentTarget"),
        (brand.launch, "fe2o3_device::context::RegisteredLaunch"),
    ] {
        if !rust_exact_reviewed_adt_arguments_v1(tcx, axis, path)
            .is_some_and(|args| args.is_empty())
        {
            return Err(rejected("defined Math substituted target or launch brand"));
        }
    }
    Ok(brand)
}

pub(super) fn getter<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<Getter<'tcx>, ProductionSemanticImportErrorV1> {
    reviewed_helper(tcx, instance, GETTER)?;
    let signature = signature(tcx, instance)?;
    let [reference] = signature.inputs() else {
        return Err(rejected("defined Math getter receiver arity"));
    };
    let context = rust_shared_reference_v1(*reference)
        .ok_or_else(|| rejected("defined Math getter requires shared context"))?;
    let (kernel, target, launch) = rust_kernel_context_axes_v1(tcx, context)
        .ok_or_else(|| rejected("defined Math getter exact KernelContext receiver"))?;
    let math = signature.output();
    let arguments = rust_trusted_adt_type_arguments_v1(
        tcx,
        math,
        TrustedDeviceItem::DeviceMath(dialect_amdgcn::DeviceMathDiagnosticItem::Context),
    )
    .ok_or_else(|| rejected("defined Math getter exact Math output"))?;
    let [brand] = arguments.as_slice() else {
        return Err(rejected("defined Math getter Math type arity"));
    };
    let brand = root_brand(tcx, *brand)?;
    if (brand.kernel, brand.target, brand.launch) != (kernel, target, launch)
        || instance.args.types().collect::<Vec<_>>() != [kernel, target, launch]
    {
        return Err(rejected(
            "defined Math getter receiver/result brand mismatch",
        ));
    }
    let original = tcx.instance_mir(instance.def);
    let bridge = forwarding_callee(tcx, instance, original)?;
    reviewed_helper(tcx, bridge, BRIDGE)?;
    let bridge_signature = self::signature(tcx, bridge)?;
    if !bridge_signature.inputs().is_empty()
        || bridge_signature.output() != math
        || bridge.args.types().collect::<Vec<_>>() != [brand.ty]
        || !body::kernel_math_getter(tcx, instance, original, bridge, *reference, math)
    {
        return Err(rejected(
            "defined Math original getter/bridge body or signature",
        ));
    }
    let bridge_body = tcx.instance_mir(bridge.def);
    let current = forwarding_callee(tcx, bridge, bridge_body)?;
    if trusted_device_items::classify(tcx, current.def_id())
        != Some(TrustedDeviceItem::DeviceMath(
            dialect_amdgcn::DeviceMathDiagnosticItem::ContextFromCompiler,
        ))
        || !current.args.is_empty()
    {
        return Err(rejected("defined Math bridge exact Current terminal"));
    }
    let current_signature = self::signature(tcx, current)?;
    let unbranded = current_signature.output();
    let unbranded_arguments = rust_trusted_adt_type_arguments_v1(
        tcx,
        unbranded,
        TrustedDeviceItem::DeviceMath(dialect_amdgcn::DeviceMathDiagnosticItem::Context),
    )
    .ok_or_else(|| rejected("defined Math Current exact Math type"))?;
    let [marker] = unbranded_arguments.as_slice() else {
        return Err(rejected("defined Math Current unbranded arity"));
    };
    if !current_signature.inputs().is_empty()
        || !rust_exact_reviewed_adt_arguments_v1(
            tcx,
            *marker,
            "fe2o3_device::context::UnbrandedCapability",
        )
        .is_some_and(|args| args.is_empty())
        || !body::branded_math_bridge(tcx, bridge, bridge_body, current, math, unbranded)
    {
        return Err(rejected(
            "defined Math original bridge/Current body or type",
        ));
    }
    Ok(Getter {
        bridge,
        current,
        brand,
        types: [*reference, context, math, unbranded],
    })
}

pub(super) fn bind<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<Bind<'tcx>, ProductionSemanticImportErrorV1> {
    if trusted_device_items::classify(tcx, instance.def_id())
        != Some(TrustedDeviceItem::PolicyMathBind)
    {
        return Err(rejected("defined Math Bind exact reviewed marker"));
    }
    validate_policy_bind_source_v1(tcx, instance)?;
    let signature = signature(tcx, instance)?;
    let [math_reference, policy_reference] = signature.inputs() else {
        return Err(rejected("defined Math Bind receiver arity"));
    };
    let math = rust_shared_reference_v1(*math_reference)
        .ok_or_else(|| rejected("defined Math Bind shared Math"))?;
    let capability = rust_shared_reference_v1(*policy_reference)
        .ok_or_else(|| rejected("defined Math Bind shared policy"))?;
    let arguments = rust_trusted_adt_type_arguments_v1(
        tcx,
        capability,
        TrustedDeviceItem::NumericalPolicyCapability,
    )
    .ok_or_else(|| rejected("defined Math Bind policy type"))?;
    let [brand, policy] = arguments.as_slice() else {
        return Err(rejected("defined Math Bind policy arity"));
    };
    let brand = root_brand(tcx, *brand)?;
    if !body::policy_math_bind(
        tcx,
        instance,
        tcx.instance_mir(instance.def),
        *math_reference,
        *policy_reference,
        signature.output(),
    ) {
        return Err(rejected(
            "defined Math Bind original ordered reference aggregate",
        ));
    }
    Ok(Bind {
        brand,
        policy: *policy,
        types: [
            *math_reference,
            math,
            *policy_reference,
            capability,
            signature.output(),
        ],
    })
}
