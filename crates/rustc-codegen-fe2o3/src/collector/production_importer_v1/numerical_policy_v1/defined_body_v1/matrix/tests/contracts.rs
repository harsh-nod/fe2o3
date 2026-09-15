use super::*;
use crate::collector::production_importer_v1::ConstructedProductionSemanticMirV1;
use crate::rustc_semantic_adapter_v1::canonical_function_identities_v1;
use fe2o3_mir_model::{SemanticCallExpansionLimitsV1, SemanticCallExpansionV1};

pub(super) fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    imported: &ConstructedProductionSemanticMirV1,
) {
    let mir = &imported.semantic_mir;
    let binds = mir
        .functions()
        .iter()
        .filter_map(|f| match f.defined_capability_contract().copied() {
            Some(SemanticDefinedCapabilityContractV1::PolicyMatrixBind(record)) => Some(record),
            _ => None,
        })
        .collect::<Vec<_>>();
    let narrows = mir
        .functions()
        .iter()
        .filter_map(|f| match f.defined_capability_contract().copied() {
            Some(SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(record)) => Some(record),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [bind] = binds.as_slice() else {
        panic!("one original Matrix Bind")
    };
    let [narrow] = narrows.as_slice() else {
        panic!("one original gfx950 narrowing")
    };
    assert_eq!(bind.identity(), narrow.identity());
    assert_eq!(bind.types(), narrow.types().bind);
    assert_eq!(bind.reference_arguments(), [0, 1]);
    assert_eq!(bind.reference_fields(), [0, 1]);
    assert_eq!(narrow.receiver_argument(), 0);
    assert_eq!(narrow.reference_field(), 0);
    let [root] = imported.kernel_contexts.roots.as_ref() else {
        panic!()
    };
    assert_eq!(
        bind.provenance(),
        capability_memory_provenance_v1(root, &imported.kernel_contexts).unwrap()
    );
    let bind_source = plan.function_producers()[bind.function().index() as usize].instance;
    let narrow_source = plan.function_producers()[narrow.function().index() as usize].instance;
    let getter_source =
        plan.function_producers()[narrow.projection().function().index() as usize].instance;
    let bind_facts = source::bind(tcx, bind_source).unwrap();
    let narrow_facts = source::narrow(tcx, narrow_source).unwrap();
    assert_eq!(narrow_facts.projection, getter_source);
    assert_eq!(
        bind.identity().kernel_brand(),
        rustc_type_identity_v1(tcx, bind_facts.identity.root.ty)
    );
    assert_eq!(
        bind.identity().execution_brand(),
        rustc_type_identity_v1(tcx, bind_facts.identity.execution_brand)
    );
    for (function, instance, source, abi) in [
        (
            bind.function(),
            bind_source,
            bind.source_identity(),
            bind.abi_identity(),
        ),
        (
            narrow.function(),
            narrow_source,
            narrow.source_identity(),
            narrow.abi_identity(),
        ),
        (
            narrow.projection().function(),
            getter_source,
            narrow.projection().source_identity(),
            narrow.projection().abi_identity(),
        ),
    ] {
        assert!(std::ptr::eq(
            plan.function_mir(function).unwrap(),
            tcx.instance_mir(instance.def)
        ));
        assert_eq!(
            source,
            canonical_function_identities_v1(tcx, instance).function()
        );
        assert_eq!(
            abi,
            mir.functions()[function.index() as usize].abi().identity()
        );
        assert_eq!(
            mir.callables()[function.index() as usize],
            SemanticCallableDeclV1::defined(function)
        );
    }
    assert!(
        mir.functions()[narrow.projection().function().index() as usize]
            .defined_capability_contract()
            .is_none(),
        "the field getter does not independently issue matrix authority"
    );
    for (types, identities) in [
        (bind.types().all().to_vec(), bind_facts.types.to_vec()),
        (narrow.types().all().to_vec(), narrow_facts.types.to_vec()),
    ] {
        for (ty, actual) in types.into_iter().zip(identities) {
            assert_eq!(
                mir.types()[ty.index() as usize].identity(),
                rustc_type_identity_v1(tcx, actual)
            );
        }
    }
    for (reference, pointee) in [
        (bind.types().matrix_reference, bind.types().matrix),
        (bind.types().policy_reference, bind.types().capability),
        (narrow.types().bound_reference, bind.types().bound),
    ] {
        let SemanticTypeShapeV1::Pointer(pointer) = mir.types()[reference.index() as usize].shape()
        else {
            panic!()
        };
        assert_eq!(pointer.kind(), SemanticPointerKindV1::Reference);
        assert_eq!(pointer.mutability(), SemanticMutabilityV1::Immutable);
        assert_eq!(pointer.pointee(), pointee);
    }
    super::super::body_tests::check(
        tcx,
        bind_source,
        bind_facts.types,
        narrow_source,
        narrow_facts.types,
        getter_source,
    );
    assert!(source::bind(tcx, narrow_source).is_err());
    assert!(source::narrow(tcx, bind_source).is_err());
    assert!(source::narrow(tcx, getter_source).is_err());
    for (instance, kind) in [
        (bind_source, source::Kind::Bind),
        (narrow_source, source::Kind::Narrow),
    ] {
        for (index, argument) in instance.args.iter().enumerate() {
            if argument.as_type().is_none() {
                continue;
            }
            let mut args = instance.args.to_vec();
            args[index] = tcx.types.u16.into();
            let changed = Instance {
                args: tcx.mk_args(&args),
                ..instance
            };
            assert!(
                match kind {
                    source::Kind::Bind => source::bind(tcx, changed).is_err(),
                    source::Kind::Narrow => source::narrow(tcx, changed).is_err(),
                },
                "source generic substitution {kind:?} axis {index}"
            );
        }
    }
    let mut copied = mir.functions().to_vec();
    attach(
        tcx,
        plan,
        mir.types(),
        &mut copied,
        mir.callables(),
        &imported.kernel_contexts,
    )
    .unwrap();
    assert_eq!(copied, mir.functions());
    let expansion =
        SemanticCallExpansionV1::try_new(mir, SemanticCallExpansionLimitsV1::default()).unwrap();
    let occurrences = expansion.defined_capability_bindings(mir).unwrap();
    let mut found = [0; 2];
    for occurrence in occurrences {
        let (index, arity) = match occurrence.contract() {
            SemanticDefinedCapabilityContractV1::PolicyMatrixBind(record) => {
                assert_eq!(record, *bind);
                (0, 2)
            }
            SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(record) => {
                assert_eq!(record, *narrow);
                (1, 1)
            }
            _ => continue,
        };
        found[index] += 1;
        assert_eq!(occurrence.root(), bind.provenance().root());
        assert_eq!(occurrence.arguments().len(), arity);
        assert_eq!(occurrence.callee_arguments().len(), arity);
    }
    assert_eq!(found, [1, 1]);
    for axis in 0..if bind.identity().has_distinct_execution_brand() {
        5
    } else {
        4
    } {
        let mut markers = [
            bind.identity().policy(),
            bind.identity().kernel_brand(),
            bind.identity().matrix_brand(),
            bind.identity().epoch(),
        ];
        if axis < 4 {
            markers[axis] = SemanticTypeIdentityV1::from_sha256([180 + axis as u8; 32]);
        }
        let mut identity = SemanticDefinedMatrixIdentityV1::new(
            bind.provenance(),
            markers[0],
            markers[1],
            markers[2],
            markers[3],
        )
        .unwrap();
        if bind.identity().has_distinct_execution_brand() {
            identity = identity
                .with_execution_brand(if axis == 4 {
                    SemanticTypeIdentityV1::from_sha256([184; 32])
                } else {
                    bind.identity().execution_brand()
                })
                .unwrap();
        }
        let changed_bind = SemanticPolicyMatrixBindV1::for_defined_function(
            bind.function(),
            mir.functions(),
            mir.callables(),
            mir.types(),
            bind.types(),
            identity,
        )
        .unwrap();
        let changed_narrow = SemanticPolicyGfx950NarrowV1::for_defined_function(
            narrow.function(),
            mir.functions(),
            mir.callables(),
            mir.types(),
            narrow.types(),
            identity,
        )
        .unwrap();
        let mut functions = mir.functions().to_vec();
        functions[bind.function().index() as usize] =
            unannotated(&functions[bind.function().index() as usize])
                .with_defined_capability_contract(
                    SemanticDefinedCapabilityContractV1::PolicyMatrixBind(changed_bind),
                )
                .unwrap();
        functions[narrow.function().index() as usize] =
            unannotated(&functions[narrow.function().index() as usize])
                .with_defined_capability_contract(
                    SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(changed_narrow),
                )
                .unwrap();
        let before = functions.clone();
        assert!(
            attach(
                tcx,
                plan,
                mir.types(),
                &mut functions,
                mir.callables(),
                &imported.kernel_contexts
            )
            .is_err(),
            "coherently substituted identity axis {axis} is not original source"
        );
        assert_eq!(functions, before, "failed attachment is atomic");
    }
    for mutation in 0..3 {
        let mut types = mir.types().to_vec();
        let mut callables = mir.callables().to_vec();
        let mut functions = mir.functions().to_vec();
        match mutation {
            0 => {
                callables.pop();
            }
            1 => {
                let index = bind.types().matrix_reference.index() as usize;
                let ty = &types[index];
                types[index] = SemanticTypeDeclV1::new(
                    ty.identity(),
                    ty.layout_identity(),
                    ty.layout().clone(),
                    SemanticTypeShapeV1::Unit,
                );
            }
            2 => {
                let index = bind.provenance().root().index() as usize;
                functions[index] = functions[index]
                    .clone()
                    .with_role(SemanticFunctionRoleV1::InternalHelper);
            }
            _ => unreachable!(),
        }
        let before = functions.clone();
        assert!(
            attach(
                tcx,
                plan,
                &types,
                &mut functions,
                &callables,
                &imported.kernel_contexts
            )
            .is_err()
        );
        assert_eq!(functions, before);
    }
}

pub(super) fn unannotated(function: &SemanticFunctionDeclV1) -> SemanticFunctionDeclV1 {
    assert_eq!(function.role(), SemanticFunctionRoleV1::InternalHelper);
    assert!(function.export().is_none());
    SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        function.abi().clone(),
        function.locals().to_vec(),
        function.entry(),
        function.blocks().to_vec(),
    )
    .unwrap()
}
