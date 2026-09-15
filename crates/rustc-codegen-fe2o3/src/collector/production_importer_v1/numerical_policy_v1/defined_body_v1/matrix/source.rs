use super::*;
use rustc_middle::ty::{FnSig, InstanceKind, TypeVisitableExt, TypingEnv};

pub(super) mod global_views;
pub(super) mod global_bf16_constructors;

const MATRIX_PROJECTION: &str = "fe2o3_device::matrix::PolicyMatrixCapability::matrix";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Kind {
    Bind,
    Narrow,
}

pub(super) fn kind<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Option<Kind> {
    match trusted_device_items::classify(tcx, instance.def_id()) {
        Some(TrustedDeviceItem::PolicyMatrixBind) => {
            let arguments = instance.args.types().collect::<Vec<_>>();
            // KernelContext::matrix pairs the root brand directly. It remains
            // an ordinary validated Bind, never a subgroup/epoch annotation.
            if matches!(arguments.as_slice(), [matrix, global, _]
                if matrix == global && rust_kernel_brand_v1(tcx, *global).is_some())
                && validate_policy_bind_source_v1(tcx, instance).is_ok()
            {
                None
            } else {
                Some(Kind::Bind)
            }
        }
        Some(TrustedDeviceItem::PolicyGfx950MatrixIssue) => Some(Kind::Narrow),
        _ => None,
    }
}

pub(super) struct Identity<'tcx> {
    pub root: RustKernelBrandV1<'tcx>,
    pub execution_brand: Ty<'tcx>,
    pub policy: Ty<'tcx>,
    pub matrix_brand: Ty<'tcx>,
    pub epoch: Ty<'tcx>,
}

pub(super) struct Bind<'tcx> {
    pub identity: Identity<'tcx>,
    pub types: [Ty<'tcx>; 5],
}

pub(super) struct Narrow<'tcx> {
    pub identity: Identity<'tcx>,
    pub types: [Ty<'tcx>; 7],
    pub projection: Instance<'tcx>,
}

pub(super) fn signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<FnSig<'tcx>, ProductionSemanticImportErrorV1> {
    if !matches!(instance.def, InstanceKind::Item(_))
        || !tcx.is_mir_available(instance.def_id())
        || instance.args.len() != tcx.generics_of(instance.def_id()).count()
        || instance.args.consts().next().is_some()
    {
        return Err(rejected(
            "defined matrix requires its original concrete Item body",
        ));
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    let signature = tcx
        .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
        .map_err(|_| rejected("defined matrix signature normalization"))?;
    if signature.safety != rustc_hir::Safety::Safe
        || signature.abi != rustc_abi::ExternAbi::Rust
        || signature.c_variadic
        || signature.has_non_region_param()
        || signature.has_infer()
        || signature.has_aliases()
        || signature.has_escaping_bound_vars()
    {
        return Err(rejected("defined matrix exact safe monomorphic Rust ABI"));
    }
    Ok(signature)
}

pub(super) fn fields<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Result<Vec<Ty<'tcx>>, ProductionSemanticImportErrorV1> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return Err(rejected("defined matrix result ADT"));
    };
    if !definition.is_struct() || definition.non_enum_variant().fields.len() > 4 {
        return Err(rejected("defined matrix closed struct field roster"));
    }
    definition
        .non_enum_variant()
        .fields
        .iter()
        .map(|field| {
            tcx.try_normalize_erasing_regions(
                TypingEnv::fully_monomorphized(),
                field.ty(tcx, arguments),
            )
            .map_err(|_| rejected("defined matrix field normalization"))
        })
        .collect()
}

fn phantom(tcx: TyCtxt<'_>, ty: Ty<'_>) -> bool {
    matches!(ty.kind(), TyKind::Adt(definition, _) if Some(definition.did()) == tcx.lang_items().phantom_data())
}

pub(super) fn pair<'tcx>(
    tcx: TyCtxt<'tcx>,
    bound: Ty<'tcx>,
) -> Result<Bind<'tcx>, ProductionSemanticImportErrorV1> {
    let arguments =
        rust_trusted_adt_type_arguments_v1(tcx, bound, TrustedDeviceItem::PolicyMatrixCapability)
            .ok_or_else(|| rejected("defined matrix exact PolicyMatrixCapability receiver"))?;
    let [matrix_brand, global_brand, policy] = arguments.as_slice() else {
        return Err(rejected("defined matrix bound type arity"));
    };
    let field_types = fields(tcx, bound)?;
    let [matrix_reference, policy_reference, brands, private] = field_types.as_slice() else {
        return Err(rejected(
            "defined matrix bind must retain two references and two markers",
        ));
    };
    if !phantom(tcx, *brands) || !phantom(tcx, *private) {
        return Err(rejected("defined matrix private marker fields"));
    }
    let matrix = rust_shared_reference_v1(*matrix_reference)
        .and_then(|ty| rust_matrix_capability_v1(tcx, ty))
        .ok_or_else(|| rejected("defined matrix requires the exact subgroup matrix capability"))?;
    let capability = rust_shared_reference_v1(*policy_reference)
        .ok_or_else(|| rejected("defined matrix retains a shared policy reference"))?;
    let policy_arguments = rust_trusted_adt_type_arguments_v1(
        tcx,
        capability,
        TrustedDeviceItem::NumericalPolicyCapability,
    )
    .ok_or_else(|| rejected("defined matrix numerical policy capability"))?;
    let global_root = rust_kernel_brand_v1(tcx, *global_brand)
        .ok_or_else(|| rejected("defined matrix exact policy kernel root"))?;
    let matrix_root = super::super::super::bind::matrix_global_brand(tcx, matrix.subgroup_brand, 0);
    if matrix.width != 64
        || matrix.subgroup_brand != *matrix_brand
        || matrix_root != Some(global_root.ty)
        || matrix.kernel_brand.kernel != global_root.kernel
        || matrix.kernel_brand.target != global_root.target
        || matrix.kernel_brand.launch != global_root.launch
        || policy_arguments != [*global_brand, *policy]
        || !rust_is_exact_trusted_marker_v1(
            tcx,
            *policy,
            TrustedDeviceItem::StrictIeeeNumericalPolicy,
        )
    {
        return Err(rejected(
            "defined matrix substituted width, subgroup/root brand, epoch or strict policy",
        ));
    }
    for (axis, path) in [
        (
            matrix.kernel_brand.target,
            "fe2o3_device::context::CurrentTarget",
        ),
        (
            matrix.kernel_brand.launch,
            "fe2o3_device::context::RegisteredLaunch",
        ),
    ] {
        if !rust_exact_reviewed_adt_arguments_v1(tcx, axis, path)
            .is_some_and(|args| args.is_empty())
        {
            return Err(rejected(
                "defined matrix substituted target or launch marker",
            ));
        }
    }
    Ok(Bind {
        identity: Identity {
            root: global_root,
            execution_brand: matrix.kernel_brand.ty,
            policy: *policy,
            matrix_brand: *matrix_brand,
            epoch: matrix.epoch,
        },
        types: [
            *matrix_reference,
            matrix.ty,
            *policy_reference,
            capability,
            bound,
        ],
    })
}

pub(super) fn bind<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<Bind<'tcx>, ProductionSemanticImportErrorV1> {
    if kind(tcx, instance) != Some(Kind::Bind) {
        return Err(rejected("defined matrix Bind reviewed marker"));
    }
    validate_policy_bind_source_v1(tcx, instance)?;
    let signature = signature(tcx, instance)?;
    let facts = pair(tcx, signature.output())?;
    if signature.inputs() != [facts.types[0], facts.types[2]]
        || instance.args.types().collect::<Vec<_>>()
            != [
                facts.identity.matrix_brand,
                facts.identity.root.ty,
                facts.identity.policy,
            ]
        || !body::bind(tcx, instance, tcx.instance_mir(instance.def), facts.types)
    {
        return Err(rejected(
            "defined matrix Bind original ordered-reference body",
        ));
    }
    Ok(facts)
}

pub(super) fn narrow<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<Narrow<'tcx>, ProductionSemanticImportErrorV1> {
    if kind(tcx, instance) != Some(Kind::Narrow) {
        return Err(rejected("defined matrix narrowing reviewed marker"));
    }
    let signature = signature(tcx, instance)?;
    let [reference] = signature.inputs() else {
        return Err(rejected("defined matrix narrowing receiver arity"));
    };
    let bound = rust_shared_reference_v1(*reference)
        .ok_or_else(|| rejected("defined matrix narrowing shared receiver"))?;
    let pair = pair(tcx, bound)?;
    let arguments = rust_trusted_adt_type_arguments_v1(
        tcx,
        signature.output(),
        TrustedDeviceItem::PolicyGfx950MatrixCapability,
    )
    .ok_or_else(|| rejected("defined matrix exact PolicyGfx950Matrix result"))?;
    let expected = [
        pair.identity.matrix_brand,
        pair.identity.root.ty,
        pair.identity.policy,
    ];
    if arguments != expected || instance.args.types().collect::<Vec<_>>() != expected {
        return Err(rejected(
            "defined matrix narrowing changed source/result instance arguments",
        ));
    }
    let field_types = fields(tcx, signature.output())?;
    if !matches!(field_types.as_slice(), [captured, marker] if *captured == *reference && phantom(tcx, *marker))
    {
        return Err(rejected(
            "defined matrix narrowing must capture the exact bound reference",
        ));
    }
    let [
        matrix_reference,
        matrix,
        policy_reference,
        capability,
        bound,
    ] = pair.types;
    let types = [
        matrix_reference,
        matrix,
        policy_reference,
        capability,
        bound,
        *reference,
        signature.output(),
    ];
    let original = tcx.instance_mir(instance.def);
    let projection = {
        let projection = super::super::source::forwarding_callee(tcx, instance, original)?;
        if !trusted_device_items::is_exact_reviewed_provider_definition_v1(
            tcx,
            projection.def_id(),
            MATRIX_PROJECTION,
        ) || !trusted_device_items::authenticate_reviewed_safe_external_helper_v1(
            tcx,
            projection.def_id(),
        )
        .map_err(|_| rejected("defined matrix projection provider authentication"))?
        {
            return Err(rejected(
                "defined matrix narrowing substituted the original field getter",
            ));
        }
        let signature = self::signature(tcx, projection)?;
        if signature.inputs() != [*reference]
            || signature.output() != matrix_reference
            || projection.args.types().collect::<Vec<_>>() != expected
            || !body::projection(tcx, projection, tcx.instance_mir(projection.def), types)
        {
            return Err(rejected(
                "defined matrix original getter must project field zero",
            ));
        }
        projection
    };
    if !body::narrow(tcx, instance, original, types, projection) {
        return Err(rejected("defined matrix original narrowing recipe"));
    }
    Ok(Narrow {
        identity: pair.identity,
        types,
        projection,
    })
}
