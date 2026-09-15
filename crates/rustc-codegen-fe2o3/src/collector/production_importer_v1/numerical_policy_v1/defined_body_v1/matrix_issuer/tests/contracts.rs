use super::*;
use crate::collector::production_importer_v1::ConstructedProductionSemanticMirV1;

fn unannotated(function: &SemanticFunctionDeclV1) -> SemanticFunctionDeclV1 {
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

pub(super) fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    imported: &ConstructedProductionSemanticMirV1,
) {
    let mir = &imported.semantic_mir;
    let records = mir
        .functions()
        .iter()
        .filter_map(|function| match function.defined_capability_contract() {
            Some(SemanticDefinedCapabilityContractV1::KernelMatrixDerive(record)) => Some(*record),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [record] = records.as_slice() else {
        panic!("one original Matrix getter")
    };
    let instance = plan.function_producers()[record.function().index() as usize].instance;
    let facts = source::getter(tcx, instance).unwrap();
    assert!(source::is_getter(tcx, instance));
    assert!(!source::is_getter(tcx, facts.bridge));
    assert!(!source::is_getter(tcx, facts.current));
    assert!(source::getter(tcx, facts.bridge).is_err());
    assert!(source::getter(tcx, facts.current).is_err());
    let lookalike = tcx
        .iter_local_def_id()
        .find(|id| {
            tcx.def_kind(*id) == rustc_hir::def::DefKind::Fn
                && tcx.item_name(id.to_def_id()).as_str() == "matrix"
        })
        .unwrap();
    let lookalike = Instance::mono(tcx, lookalike.to_def_id());
    assert!(!source::is_getter(tcx, lookalike));
    assert!(source::getter(tcx, lookalike).is_err());
    assert_eq!(
        plan.function_producers()[record.bridge().function().index() as usize].instance,
        facts.bridge
    );
    assert_eq!(
        record.current_source_identity(),
        canonical_function_identities_v1(tcx, facts.current).function()
    );
    assert_eq!(
        record.kernel_brand(),
        rustc_type_identity_v1(tcx, facts.brand.ty)
    );
    assert!(
        mir.functions()[record.bridge().function().index() as usize]
            .defined_capability_contract()
            .is_none()
    );
    for (id, ty) in record.types().all().into_iter().zip(facts.types) {
        assert_eq!(
            mir.types()[id.index() as usize].identity(),
            rustc_type_identity_v1(tcx, ty)
        );
    }
    for (function, instance) in [
        (record.function(), instance),
        (record.bridge().function(), facts.bridge),
    ] {
        assert!(std::ptr::eq(
            plan.function_mir(function).unwrap(),
            tcx.instance_mir(instance.def)
        ));
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
    let getter_index = record.function().index() as usize;
    copied[getter_index] = unannotated(&copied[getter_index]);
    let erased = InertSemanticMirRequestV1::new_with_callables(
        mir.target(),
        mir.types().to_vec(),
        mir.allocations().to_vec(),
        mir.statics().to_vec(),
        mir.vtables().to_vec(),
        copied.clone(),
        mir.callables().to_vec(),
        mir.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v23(SemanticMirLimitsV1::default())
    .unwrap();
    assert!(
        validate_carriage(tcx, plan, &imported.kernel_contexts, &erased).is_err(),
        "an erased source attachment cannot pass live replay"
    );
    attach(
        tcx,
        plan,
        mir.types(),
        &mut copied,
        mir.callables(),
        &imported.kernel_contexts,
    )
    .unwrap();
    assert_eq!(
        copied,
        mir.functions(),
        "exact source reconstructs the omitted attachment"
    );

    // Change a complete nominal identity, not just its label in an expected result.
    let foreign = SemanticKernelMatrixDeriveV1::for_defined_function(
        record.function(),
        mir.functions(),
        mir.callables(),
        mir.types(),
        record.types(),
        record.provenance(),
        SemanticTypeIdentityV1::from_sha256([183; 32]),
    )
    .unwrap();
    copied[getter_index] = unannotated(&copied[getter_index])
        .with_defined_capability_contract(SemanticDefinedCapabilityContractV1::KernelMatrixDerive(
            foreign,
        ))
        .unwrap();
    let before = copied.clone();
    assert!(
        attach(
            tcx,
            plan,
            mir.types(),
            &mut copied,
            mir.callables(),
            &imported.kernel_contexts
        )
        .is_err()
    );
    assert_eq!(copied, before);

    for mutation in 0..4 {
        let mut functions = mir.functions().to_vec();
        let mut callables = mir.callables().to_vec();
        let mut types = mir.types().to_vec();
        match mutation {
            0 => {
                callables.pop();
            }
            1 => {
                let id = record.current_callable().index() as usize;
                let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } =
                    &mut callables[id]
                else {
                    panic!("exact Current")
                };
                *operation = SemanticCompilerIntrinsicOperationV1::MathContextCurrent {
                    context: record.types().unbranded_matrix,
                };
            }
            2 => {
                let id = record.types().context_reference.index() as usize;
                let ty = &types[id];
                types[id] = SemanticTypeDeclV1::new(
                    ty.identity(),
                    ty.layout_identity(),
                    ty.layout().clone(),
                    SemanticTypeShapeV1::Unit,
                );
            }
            3 => {
                let id = record.provenance().root().index() as usize;
                functions[id] = functions[id]
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

    let original = tcx.instance_mir(instance.def);
    for mutation in 0..3 {
        let mut changed = original.clone();
        match mutation {
            0 => {
                let rustc_middle::mir::TerminatorKind::Call { target, .. } =
                    &mut changed.basic_blocks.as_mut()[rustc_middle::mir::START_BLOCK]
                        .terminator_mut()
                        .kind
                else {
                    panic!("getter call")
                };
                *target = Some(rustc_middle::mir::START_BLOCK);
            }
            1 => changed.local_decls[rustc_middle::mir::RETURN_PLACE].ty = tcx.types.u32,
            2 => {
                let rustc_middle::mir::TerminatorKind::Call { destination, .. } =
                    &mut changed.basic_blocks.as_mut()[rustc_middle::mir::START_BLOCK]
                        .terminator_mut()
                        .kind
                else {
                    panic!("getter call")
                };
                destination.local = rustc_middle::mir::Local::from_usize(1);
            }
            _ => unreachable!(),
        }
        assert!(
            !body::kernel_math_getter(
                tcx,
                instance,
                &changed,
                facts.bridge,
                facts.types[0],
                facts.types[2]
            ),
            "changed original getter body {mutation}"
        );
    }
    let mut erased = original.clone();
    erased.basic_blocks.as_mut()[rustc_middle::mir::START_BLOCK]
        .terminator_mut()
        .kind = rustc_middle::mir::TerminatorKind::Return;
    assert!(!body::kernel_math_getter(
        tcx,
        instance,
        &erased,
        facts.bridge,
        facts.types[0],
        facts.types[2]
    ));
    assert!(!body::branded_math_bridge(
        tcx,
        facts.bridge,
        tcx.instance_mir(facts.bridge.def),
        facts.bridge,
        facts.types[2],
        facts.types[3]
    ));
    for (axis, (index, _)) in instance
        .args
        .iter()
        .enumerate()
        .filter(|(_, arg)| arg.as_type().is_some())
        .enumerate()
    {
        let mut arguments = instance.args.to_vec();
        arguments[index] = tcx.types.u16.into();
        let changed = Instance {
            args: tcx.mk_args(&arguments),
            ..instance
        };
        if axis == 0 {
            assert_ne!(
                source::getter(tcx, changed).unwrap().brand.ty,
                facts.brand.ty
            );
        } else {
            assert!(source::getter(tcx, changed).is_err());
        }
    }
}
