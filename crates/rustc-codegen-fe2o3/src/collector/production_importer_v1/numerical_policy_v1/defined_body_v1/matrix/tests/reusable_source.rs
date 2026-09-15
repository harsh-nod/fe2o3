//! Live sealed ancestry and canonical dual-identity carriage checks.
use super::*;
use crate::collector::production_importer_v1::ConstructedProductionSemanticMirV1;
use rustc_middle::ty::{Instance, Ty, TyKind};

pub(super) fn check<'tcx>(tcx: TyCtxt<'tcx>, plan: &ProductionSemanticPreflightPlanV1<'tcx>) {
    let mut count = 0;
    for producer in plan.function_producers() {
        if source::kind(tcx, producer.instance) != Some(source::Kind::Bind) {
            continue;
        }
        count += 1;
        let facts = source::bind(tcx, producer.instance).unwrap();
        let matrix = rust_matrix_capability_v1(tcx, facts.types[1]).unwrap();
        let root = facts.identity.root;
        assert_ne!(matrix.kernel_brand.ty, root.ty);
        assert!(!rust_same_kernel_brand_v1(matrix.kernel_brand, root));
        assert_eq!(matrix.subgroup_brand, facts.identity.matrix_brand);
        assert_eq!(matrix.epoch, facts.identity.epoch);
        assert_eq!(
            super::super::super::super::bind::matrix_global_brand(tcx, matrix.subgroup_brand, 0),
            Some(root.ty),
        );
        assert_eq!(
            rust_trusted_adt_type_arguments_v1(
                tcx,
                matrix.kernel_brand.ty,
                TrustedDeviceItem::ReusableWorkgroupBrand
            )
            .unwrap(),
            [root.ty]
        );
        assert!(
            rust_trusted_adt_type_arguments_v1(
                tcx,
                matrix.epoch,
                TrustedDeviceItem::DynamicPhaseEpoch
            )
            .unwrap()
            .is_empty()
        );
        assert_eq!(
            rust_shared_reference_v1(facts.types[0]),
            Some(facts.types[1])
        );
        assert_eq!(
            rust_shared_reference_v1(facts.types[2]),
            Some(facts.types[3])
        );
        let TyKind::Adt(root_definition, root_arguments) = *root.ty.kind() else {
            panic!("root")
        };
        let mut other_arguments = root_arguments.to_vec();
        let kernel_index = other_arguments
            .iter()
            .position(|arg| arg.as_type().is_some())
            .unwrap();
        other_arguments[kernel_index] = tcx.types.u16.into();
        let other_root = Ty::new_adt(tcx, root_definition, tcx.mk_args(&other_arguments));
        assert!(rust_kernel_brand_v1(tcx, other_root).is_some());
        for substitute in [other_root, matrix.kernel_brand.ty, tcx.types.u16] {
            let changed = replace_type(tcx, producer.instance, 1, substitute);
            assert!(validate_policy_bind_source_v1(tcx, changed).is_err());
            assert!(source::bind(tcx, changed).is_err());
        }
        let narrower = plan
            .function_producers()
            .iter()
            .find(|function| source::kind(tcx, function.instance) == Some(source::Kind::Narrow))
            .unwrap();
        let narrow = source::narrow(tcx, narrower.instance).unwrap();
        assert_eq!(narrow.identity.root.ty, root.ty);
        assert_eq!(narrow.identity.matrix_brand, matrix.subgroup_brand);
        assert_eq!(narrow.identity.epoch, matrix.epoch);
        assert_eq!(&narrow.types[..5], &facts.types);
        // A same-shape subgroup from another kernel cannot bind this root policy.
        let TyKind::Adt(phase, phase_arguments) = *matrix.kernel_brand.ty.kind() else {
            panic!("phase")
        };
        let mut phase_arguments = phase_arguments.to_vec();
        let parent_index = phase_arguments
            .iter()
            .position(|argument| argument.as_type().is_some())
            .unwrap();
        phase_arguments[parent_index] = other_root.into();
        let foreign_phase = Ty::new_adt(tcx, phase, tcx.mk_args(&phase_arguments));
        let TyKind::Adt(subgroup, arguments) = *matrix.subgroup_brand.kind() else {
            panic!("subgroup")
        };
        let mut arguments = arguments.to_vec();
        let parent_index = arguments
            .iter()
            .enumerate()
            .filter(|(_, arg)| arg.as_type().is_some())
            .nth(1)
            .unwrap()
            .0;
        arguments[parent_index] = foreign_phase.into();
        let foreign_subgroup = Ty::new_adt(tcx, subgroup, tcx.mk_args(&arguments));
        let changed = replace_type(tcx, producer.instance, 0, foreign_subgroup);
        assert!(validate_policy_bind_source_v1(tcx, changed).is_err());
        assert!(source::bind(tcx, changed).is_err());
    }
    assert_eq!(count, 1, "one actual reusable-phase Bind source");
}

pub(super) fn check_import<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    imported: &ConstructedProductionSemanticMirV1,
) {
    let mir = &imported.semantic_mir;
    let bind = mir
        .functions()
        .iter()
        .find_map(
            |function| match function.defined_capability_contract().copied()? {
                SemanticDefinedCapabilityContractV1::PolicyMatrixBind(record) => Some(record),
                _ => None,
            },
        )
        .unwrap();
    let narrow = mir
        .functions()
        .iter()
        .find_map(
            |function| match function.defined_capability_contract().copied()? {
                SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(record) => Some(record),
                _ => None,
            },
        )
        .unwrap();
    let identity = bind.identity();
    assert!(identity.has_distinct_execution_brand());
    assert_ne!(identity.execution_brand(), identity.kernel_brand());
    assert_eq!(identity, narrow.identity());
    let instance = plan.function_producers()[bind.function().index() as usize].instance;
    let facts = source::bind(tcx, instance).unwrap();
    assert_eq!(
        identity.execution_brand(),
        rustc_type_identity_v1(tcx, facts.identity.execution_brand)
    );
    assert_eq!(
        identity.kernel_brand(),
        rustc_type_identity_v1(tcx, facts.identity.root.ty)
    );
    let mut issuers = 0;
    for callable in mir.callables() {
        let SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
            ..
        } = callable
        else {
            continue;
        };
        let SemanticExecutionCapabilityOperationV1::MatrixAccess {
            matrix,
            subgroup_brand,
            width,
            ..
        } = contract.operation()
        else {
            continue;
        };
        if matrix != bind.types().matrix {
            continue;
        }
        issuers += 1;
        assert_eq!(width, 64);
        assert_eq!(subgroup_brand, identity.matrix_brand());
        assert_eq!(contract.workgroup_brand(), Some(identity.execution_brand()));
        assert_eq!(contract.epoch_before(), Some(identity.epoch()));
        assert_eq!(contract.epoch_after(), None);
        assert_eq!(contract.provenance(), identity.provenance());
    }
    assert!(
        issuers > 0,
        "the original Matrix issuer retains E, subgroup and epoch"
    );
    let decoded = AdmittedInertSemanticMirV1::decode_exact_v23_canonical(
        mir.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.canonical_encoding(), mir.canonical_encoding());
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v22_canonical(
            mir.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .is_err()
    );

    // Keep both original helper bodies and every issuer intact. Only the pair changes.
    let direct = SemanticDefinedMatrixIdentityV1::new(
        identity.provenance(),
        identity.policy(),
        identity.kernel_brand(),
        identity.matrix_brand(),
        identity.epoch(),
    )
    .unwrap();
    for changed in [
        direct,
        direct
            .with_execution_brand(SemanticTypeIdentityV1::from_sha256([185; 32]))
            .unwrap(),
    ] {
        let changed_bind = SemanticPolicyMatrixBindV1::for_defined_function(
            bind.function(),
            mir.functions(),
            mir.callables(),
            mir.types(),
            bind.types(),
            changed,
        )
        .unwrap();
        let changed_narrow = SemanticPolicyGfx950NarrowV1::for_defined_function(
            narrow.function(),
            mir.functions(),
            mir.callables(),
            mir.types(),
            narrow.types(),
            changed,
        )
        .unwrap();
        let mut functions = mir.functions().to_vec();
        for (function, contract) in [
            (
                bind.function(),
                SemanticDefinedCapabilityContractV1::PolicyMatrixBind(changed_bind),
            ),
            (
                narrow.function(),
                SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(changed_narrow),
            ),
        ] {
            let index = function.index() as usize;
            functions[index] = contracts::unannotated(&functions[index])
                .with_defined_capability_contract(contract)
                .unwrap();
        }
        let request = InertSemanticMirRequestV1::new_with_callables(
            mir.target(),
            mir.types().to_vec(),
            mir.allocations().to_vec(),
            mir.statics().to_vec(),
            mir.vtables().to_vec(),
            functions,
            mir.callables().to_vec(),
            mir.roots().to_vec(),
        )
        .unwrap();
        assert!(
            matches!(
                request.admit_current_production(SemanticMirLimitsV1::default()),
                Err(SemanticMirErrorV1::InvalidFunctionAbi)
            ),
            "a flattened or substituted E cannot match the original issuer"
        );
    }
}

fn replace_type<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    ordinal: usize,
    ty: Ty<'tcx>,
) -> Instance<'tcx> {
    let mut arguments = instance.args.to_vec();
    let index = arguments
        .iter()
        .enumerate()
        .filter(|(_, arg)| arg.as_type().is_some())
        .nth(ordinal)
        .unwrap()
        .0;
    arguments[index] = ty.into();
    Instance {
        args: tcx.mk_args(&arguments),
        ..instance
    }
}
