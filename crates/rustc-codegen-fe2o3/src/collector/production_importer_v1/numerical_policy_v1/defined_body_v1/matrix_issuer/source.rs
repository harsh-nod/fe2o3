use super::*;
use rustc_middle::mir::{Body, Operand, START_BLOCK, TerminatorKind};
use rustc_middle::ty::{EarlyBinder, FnSig, InstanceKind, TypeVisitableExt, TypingEnv};

const GETTER: &str = "fe2o3_device::context::KernelContext::matrix";
const BRIDGE: &str = "fe2o3_device::matrix::MatrixCapability::current_branded";

pub(super) fn is_getter<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> bool {
    trusted_device_items::is_exact_reviewed_provider_definition_v1(tcx, instance.def_id(), GETTER)
}

pub(super) struct Getter<'tcx> {
    pub bridge: Instance<'tcx>,
    pub current: Instance<'tcx>,
    pub brand: RustKernelBrandV1<'tcx>,
    pub types: [Ty<'tcx>; 6],
}

fn signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<FnSig<'tcx>, ProductionSemanticImportErrorV1> {
    if !matches!(instance.def, InstanceKind::Item(_))
        || instance.args.len() != tcx.generics_of(instance.def_id()).count()
        || instance.args.consts().next().is_some()
    {
        return Err(rejected("defined Matrix requires an exact Item instance"));
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    let signature = tcx
        .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
        .map_err(|_| rejected("defined Matrix signature normalization"))?;
    if signature.safety != rustc_hir::Safety::Safe
        || signature.abi != rustc_abi::ExternAbi::Rust
        || signature.c_variadic
        || signature.has_non_region_param()
        || signature.has_infer()
        || signature.has_aliases()
        || signature.has_escaping_bound_vars()
    {
        return Err(rejected(
            "defined Matrix requires a safe monomorphic Rust signature",
        ));
    }
    Ok(signature)
}

fn reviewed_helper<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    path: &str,
) -> Result<(), ProductionSemanticImportErrorV1> {
    if !trusted_device_items::is_exact_reviewed_provider_definition_v1(tcx, instance.def_id(), path)
        || !trusted_device_items::authenticate_reviewed_safe_external_helper_v1(
            tcx,
            instance.def_id(),
        )
        .map_err(|_| rejected("defined Matrix external helper authentication"))?
        || !matches!(instance.def, InstanceKind::Item(_))
        || !tcx.is_mir_available(instance.def_id())
    {
        return Err(rejected(
            "defined Matrix requires its exact reviewed external helper body",
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
        .ok_or_else(|| rejected("defined Matrix missing original entry"))?;
    let Some(terminator) = &entry.terminator else {
        return Err(rejected("defined Matrix missing original call"));
    };
    let TerminatorKind::Call {
        func: Operand::Constant(callee),
        ..
    } = &terminator.kind
    else {
        return Err(rejected(
            "defined Matrix requires an original constant callee",
        ));
    };
    let ty = instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(callee.const_.ty()),
        )
        .map_err(|_| rejected("defined Matrix callee normalization"))?;
    let TyKind::FnDef(definition, arguments) = *ty.kind() else {
        return Err(rejected("defined Matrix callee is not an exact FnDef"));
    };
    Instance::try_resolve(
        tcx,
        TypingEnv::fully_monomorphized(),
        definition,
        tcx.erase_and_anonymize_regions(arguments),
    )
    .ok()
    .flatten()
    .ok_or_else(|| rejected("defined Matrix callee instance resolution"))
}

fn root_brand<'tcx>(
    tcx: TyCtxt<'tcx>,
    brand: Ty<'tcx>,
) -> Result<RustKernelBrandV1<'tcx>, ProductionSemanticImportErrorV1> {
    let brand = rust_kernel_brand_v1(tcx, brand)
        .ok_or_else(|| rejected("defined Matrix exact kernel brand"))?;
    for (axis, path) in [
        (brand.target, "fe2o3_device::context::CurrentTarget"),
        (brand.launch, "fe2o3_device::context::RegisteredLaunch"),
    ] {
        if !rust_exact_reviewed_adt_arguments_v1(tcx, axis, path)
            .is_some_and(|args| args.is_empty())
        {
            return Err(rejected(
                "defined Matrix substituted target or launch brand",
            ));
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
        return Err(rejected("defined Matrix getter receiver arity"));
    };
    let context = rust_shared_reference_v1(*reference)
        .ok_or_else(|| rejected("defined Matrix getter requires shared context"))?;
    let (kernel, target, launch) = rust_kernel_context_axes_v1(tcx, context)
        .ok_or_else(|| rejected("defined Matrix getter exact KernelContext receiver"))?;
    let matrix = signature.output();
    let arguments =
        rust_exact_reviewed_adt_arguments_v1(tcx, matrix, "fe2o3_device::matrix::MatrixCapability")
            .ok_or_else(|| rejected("defined Matrix getter exact Matrix output"))?;
    let [brand] = arguments.as_slice() else {
        return Err(rejected("defined Matrix getter Matrix type arity"));
    };
    let brand = root_brand(
        tcx,
        brand
            .as_type()
            .ok_or_else(|| rejected("defined Matrix getter non-type brand argument"))?,
    )?;
    if (brand.kernel, brand.target, brand.launch) != (kernel, target, launch)
        || instance.args.types().collect::<Vec<_>>() != [kernel, target, launch]
    {
        return Err(rejected(
            "defined Matrix getter receiver/result brand mismatch",
        ));
    }
    let original = tcx.instance_mir(instance.def);
    let bridge = forwarding_callee(tcx, instance, original)?;
    reviewed_helper(tcx, bridge, BRIDGE)?;
    let bridge_signature = self::signature(tcx, bridge)?;
    if !bridge_signature.inputs().is_empty()
        || bridge_signature.output() != matrix
        || bridge.args.types().collect::<Vec<_>>() != [brand.ty]
        || !body::kernel_math_getter(tcx, instance, original, bridge, *reference, matrix)
    {
        return Err(rejected(
            "defined Matrix original getter/bridge body or signature",
        ));
    }
    let bridge_body = tcx.instance_mir(bridge.def);
    let current = forwarding_callee(tcx, bridge, bridge_body)?;
    if trusted_device_items::classify(tcx, current.def_id())
        != Some(TrustedDeviceItem::DeviceMatrixCurrent)
        || !current.args.is_empty()
    {
        return Err(rejected("defined Matrix bridge exact Current terminal"));
    }
    let current_signature = self::signature(tcx, current)?;
    let unbranded = current_signature.output();
    if !current_signature.inputs().is_empty()
        || !rust_is_trusted_adt_v1(tcx, unbranded, TrustedDeviceItem::DeviceMatrix)
        || !body::branded_math_bridge(tcx, bridge, bridge_body, current, matrix, unbranded)
    {
        return Err(rejected(
            "defined Matrix original bridge/Current body or type",
        ));
    }
    let [brand_marker, thread_marker] = wrapper_fields(tcx, matrix, unbranded, brand.ty)
        .ok_or_else(|| rejected("defined Matrix exact issuer fields and invariant brand"))?;
    Ok(Getter {
        bridge,
        current,
        brand,
        types: [
            *reference,
            context,
            matrix,
            unbranded,
            brand_marker,
            thread_marker,
        ],
    })
}

fn phantom<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    (Some(definition.did()) == tcx.lang_items().phantom_data() && arguments.len() == 1)
        .then(|| arguments[0].as_type())
        .flatten()
}

fn wrapper_fields<'tcx>(
    tcx: TyCtxt<'tcx>,
    matrix: Ty<'tcx>,
    unbranded: Ty<'tcx>,
    brand: Ty<'tcx>,
) -> Option<[Ty<'tcx>; 2]> {
    let TyKind::Adt(definition, arguments) = *matrix.kind() else {
        return None;
    };
    if !definition.is_struct() || definition.non_enum_variant().fields.len() != 3 {
        return None;
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
        })
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if fields[0] != unbranded {
        return None;
    }
    let invariant = phantom(tcx, fields[1])?;
    if !matches!(invariant.kind(), TyKind::FnPtr(signature, header)
        if signature.bound_vars().is_empty()
        && header.safety == rustc_hir::Safety::Safe
        && header.abi == rustc_abi::ExternAbi::Rust && !header.c_variadic
        && signature.skip_binder().inputs_and_output.as_slice() == [brand, brand])
        || !phantom(tcx, fields[2]).is_some_and(|ty| {
            matches!(ty.kind(), TyKind::RawPtr(unit, rustc_hir::Mutability::Mut)
                if *unit == tcx.types.unit)
        })
    {
        return None;
    }
    for ty in [matrix, unbranded, fields[1], fields[2]] {
        let layout = tcx
            .layout_of(TypingEnv::fully_monomorphized().as_query_input(ty))
            .ok()?;
        if layout.size.bytes() != 0
            || layout.align.abi.bytes() != 1
            || layout.uninhabited
            || layout.backend_repr != (rustc_abi::BackendRepr::Memory { sized: true })
        {
            return None;
        }
    }
    Some([fields[1], fields[2]])
}
