//! Authenticate policy pairing, then import the original Rust aggregate body.
//!
//! Neither borrowed input is replaced by a ZST or an intrinsic. Ordinary ABI,
//! borrow, aggregate, call-expansion and use checks continue to apply. In
//! particular a later FP consumer does not become a bare math operation here.

use super::*;
use rustc_middle::ty::{InstanceKind, TypeVisitableExt, TypingEnv};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BindKind {
    Math,
    Matrix,
}

impl BindKind {
    fn from_marker(marker: TrustedDeviceItem) -> Option<Self> {
        match marker {
            TrustedDeviceItem::PolicyMathBind => Some(Self::Math),
            TrustedDeviceItem::PolicyMatrixBind => Some(Self::Matrix),
            _ => None,
        }
    }

    fn output_marker(self) -> TrustedDeviceItem {
        match self {
            Self::Math => TrustedDeviceItem::PolicyMathCapability,
            Self::Matrix => TrustedDeviceItem::PolicyMatrixCapability,
        }
    }
}

pub(in super::super) fn validate_policy_bind_source_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<(), ProductionSemanticImportErrorV1> {
    let Some(kind) =
        trusted_device_items::classify(tcx, instance.def_id()).and_then(BindKind::from_marker)
    else {
        if trusted_device_items::rejected_provider(tcx, instance.def_id()).is_some() {
            return Err(rejected(
                "policy bind marker has an unauthenticated provider",
            ));
        }
        // An untrusted lookalike remains an ordinary function and obtains no
        // reviewed-helper admission from this validator.
        return Ok(());
    };
    if !matches!(instance.def, InstanceKind::Item(_))
        || !tcx.is_mir_available(instance.def_id())
        || instance.args.len() != tcx.generics_of(instance.def_id()).count()
        || instance.args.consts().next().is_some()
    {
        return Err(rejected(
            "policy bind requires its original concrete Rust body",
        ));
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    let signature = tcx
        .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
        .map_err(|_| rejected("policy bind signature normalization"))?;
    if signature.safety != rustc_hir::Safety::Safe
        || signature.abi != rustc_abi::ExternAbi::Rust
        || signature.c_variadic
        || signature.has_non_region_param()
        || signature.has_infer()
        || signature.has_aliases()
        || signature.has_escaping_bound_vars()
    {
        return Err(rejected(
            "policy bind requires an exact safe Rust signature",
        ));
    }
    let [receiver, policy_reference] = signature.inputs() else {
        return Err(rejected("policy bind requires exactly two borrowed inputs"));
    };
    let receiver_type = rust_shared_reference_v1(*receiver)
        .ok_or_else(|| rejected("policy bind receiver must be a shared reference"))?;
    let policy_type = rust_shared_reference_v1(*policy_reference)
        .ok_or_else(|| rejected("policy bind policy must be a shared reference"))?;
    let arguments = rust_trusted_adt_type_arguments_v1(
        tcx,
        policy_type,
        TrustedDeviceItem::NumericalPolicyCapability,
    )
    .ok_or_else(|| rejected("policy bind requires the reviewed numerical capability"))?;
    let [global_brand, policy] = arguments.as_slice() else {
        return Err(rejected("policy bind numerical capability type arity"));
    };
    let TyKind::Adt(definition, arguments) = *policy.kind() else {
        return Err(rejected("policy bind requires the reviewed strict policy"));
    };
    if trusted_device_items::classify(tcx, definition.did())
        != Some(TrustedDeviceItem::StrictIeeeNumericalPolicy)
        || !definition.is_enum()
        || !definition.variants().is_empty()
        || !arguments.is_empty()
        || rust_kernel_brand_v1(tcx, *global_brand).is_none()
    {
        return Err(rejected(
            "policy bind substituted policy or global kernel brand",
        ));
    }
    let receiver_arguments = match kind {
        BindKind::Math => rust_trusted_adt_type_arguments_v1(
            tcx,
            receiver_type,
            TrustedDeviceItem::DeviceMath(dialect_amdgcn::DeviceMathDiagnosticItem::Context),
        ),
        BindKind::Matrix => rust_exact_reviewed_adt_arguments_v1(
            tcx,
            receiver_type,
            "fe2o3_device::matrix::MatrixCapability",
        )
        .map(|arguments| arguments.types().collect()),
    }
    .ok_or_else(|| rejected("policy bind substituted operation capability"))?;
    let [receiver_brand] = receiver_arguments.as_slice() else {
        return Err(rejected("policy bind operation capability type arity"));
    };
    let root_brand = match kind {
        BindKind::Math => Some(*receiver_brand),
        BindKind::Matrix => matrix_global_brand(tcx, *receiver_brand, 0),
    };
    if root_brand != Some(*global_brand) {
        return Err(rejected(
            "policy bind operation and policy belong to different roots",
        ));
    }
    let output = signature.output();
    let output_arguments = rust_trusted_adt_type_arguments_v1(tcx, output, kind.output_marker())
        .ok_or_else(|| rejected("policy bind substituted result wrapper"))?;
    let expected_arguments = match kind {
        BindKind::Math => vec![*receiver_brand, *policy],
        BindKind::Matrix => vec![*receiver_brand, *global_brand, *policy],
    };
    if output_arguments != expected_arguments
        || instance.args.types().collect::<Vec<_>>() != expected_arguments
    {
        return Err(rejected(
            "policy bind changed exact instance or result type arguments",
        ));
    }
    let TyKind::Adt(definition, arguments) = *output.kind() else {
        return Err(rejected("policy bind result is not an aggregate"));
    };
    if !definition.is_struct() {
        return Err(rejected("policy bind result is not the reviewed struct"));
    }
    let fields = &definition.non_enum_variant().fields;
    let expected_fields = match kind {
        BindKind::Math => 3,
        BindKind::Matrix => 4,
    };
    if fields.len() != expected_fields {
        return Err(rejected("policy bind result field arity"));
    }
    let field_types = fields
        .iter()
        .map(|field| {
            tcx.try_normalize_erasing_regions(
                TypingEnv::fully_monomorphized(),
                field.ty(tcx, arguments),
            )
            .map_err(|_| rejected("policy bind field type normalization"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if field_types[..2] != [*receiver, *policy_reference]
        || field_types[2..].iter().any(|ty| {
            !matches!(ty.kind(), TyKind::Adt(definition, _)
                if Some(definition.did()) == tcx.lang_items().phantom_data())
        })
    {
        return Err(rejected(
            "policy bind must retain both exact ordered references",
        ));
    }
    Ok(())
}

// Mirror only the sealed GlobalAccess implementations. The complete nested
// brand remains in the receiver/result types; extracting its root for equality
// does not replace a subgroup width, epoch or reusable-workgroup identity.
pub(super) fn matrix_global_brand<'tcx>(
    tcx: TyCtxt<'tcx>,
    brand: Ty<'tcx>,
    depth: usize,
) -> Option<Ty<'tcx>> {
    if depth == 8 {
        return None;
    }
    if rust_kernel_brand_v1(tcx, brand).is_some() {
        return Some(brand);
    }
    if let Some(arguments) =
        rust_exact_reviewed_adt_arguments_v1(tcx, brand, "fe2o3_device::execution::SubgroupBrand")
    {
        let arguments = arguments.types().collect::<Vec<_>>();
        let [width, parent, _epoch] = arguments.as_slice() else {
            return None;
        };
        rust_subgroup_width_v1(tcx, *width)?;
        return matrix_global_brand(tcx, *parent, depth + 1);
    }
    let arguments = rust_exact_reviewed_adt_arguments_v1(
        tcx,
        brand,
        "fe2o3_device::execution::ReusableWorkgroupBrand",
    )?
    .types()
    .collect::<Vec<_>>();
    let [parent] = arguments.as_slice() else {
        return None;
    };
    matrix_global_brand(tcx, *parent, depth + 1)
}

#[cfg(test)]
mod tests;
