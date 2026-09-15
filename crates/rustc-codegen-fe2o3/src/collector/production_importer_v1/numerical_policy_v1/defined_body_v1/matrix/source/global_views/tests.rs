use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBasicBlockV1, SemanticDirectCallV1, SemanticTerminatorKindV1, SemanticTerminatorV1,
};

pub(super) fn check_source<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) {
    let facts = validate_source(tcx, instance).unwrap().unwrap();
    body_tests::check(tcx, instance, facts.inputs, facts.output, facts.checked);
    let TyKind::Adt(_, arguments) = facts.view.kind() else {
        panic!("view")
    };
    let arguments = arguments.types().collect::<Vec<_>>();
    let [format, role, matrix, root] = arguments.as_slice() else {
        panic!("view arguments")
    };
    assert!(layout::view(
        tcx,
        facts.view,
        facts.inputs[1],
        *format,
        *role,
        *matrix
    ));
    assert!(!layout::view(
        tcx,
        facts.view,
        facts.inputs[0],
        *format,
        *role,
        *matrix
    ));
    assert!(!layout::view(
        tcx,
        facts.view,
        facts.inputs[1],
        *role,
        *format,
        *matrix
    ));
    assert!(!layout::view(
        tcx,
        facts.view,
        facts.inputs[1],
        *format,
        *role,
        *root
    ));
    assert!(layout::result(tcx, facts.output));
    assert!(!layout::result(tcx, facts.view));
    assert!(!layout::result(tcx, tcx.types.bool));
    for (index, argument) in instance.args.iter().enumerate() {
        if argument.as_type().is_none() {
            continue;
        }
        let mut arguments = instance.args.to_vec();
        arguments[index] = tcx.types.u16.into();
        let changed = Instance {
            args: tcx.mk_args(&arguments),
            ..instance
        };
        assert!(
            validate_source(tcx, changed).is_err(),
            "constructor generic axis {index}"
        );
    }
}

pub(super) fn check_import<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    imported: &ConstructedProductionSemanticMirV1,
) {
    let mir = &imported.semantic_mir;
    let candidates = plan
        .function_producers()
        .iter()
        .enumerate()
        .filter(|(_, function)| kind(tcx, function.instance.def_id()).is_some())
        .collect::<Vec<_>>();
    assert_eq!(candidates.len(), 4, "all four original Global constructors");
    let mut checked_ids = BTreeSet::new();
    for (index, producer) in candidates {
        check_source(tcx, producer.instance);
        let facts = validate_source(tcx, producer.instance).unwrap().unwrap();
        let checked = plan
            .function_producers()
            .iter()
            .position(|function| function.instance == facts.checked)
            .unwrap();
        checked_ids.insert(checked);
        let constructor = &mir.functions()[index];
        assert_eq!(constructor.abi().source_input_types().len(), 6);
        assert_eq!(mir.functions()[checked].abi().source_input_types().len(), 5);
        assert!(constructor.defined_capability_contract().is_none());
        assert!(
            mir.functions()[checked]
                .defined_capability_contract()
                .is_none()
        );
        assert_eq!(
            mir.callables()[index],
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
        );
        assert_eq!(
            mir.callables()[checked],
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(checked as u32))
        );
        // Change an actual canonical operand without changing its source labels.
        let mut changed = mir.functions().to_vec();
        let mut blocks = constructor.blocks().to_vec();
        let entry = constructor.entry().index() as usize;
        let original = &blocks[entry];
        let SemanticTerminatorKindV1::Call(call) = original.terminator().kind() else {
            panic!("original constructor call")
        };
        let mut arguments = call.arguments().to_vec();
        arguments.swap(1, 2);
        let changed_call = SemanticDirectCallV1::new_callable_with_variadic_argument_abis(
            call.callee(),
            arguments,
            call.variadic_argument_abis().to_vec(),
            call.destination().cloned(),
            call.unwind(),
        )
        .unwrap();
        blocks[entry] = SemanticBasicBlockV1::new(
            original.identity(),
            original.source(),
            original.statements().to_vec(),
            SemanticTerminatorV1::new(
                original.terminator().source(),
                SemanticTerminatorKindV1::Call(changed_call),
            ),
        )
        .unwrap();
        changed[index] = function_with_blocks(constructor, blocks);
        assert!(
            validate_canonical(
                tcx,
                plan,
                mir.types(),
                &changed,
                mir.callables(),
                &imported.kernel_contexts
            )
            .is_err(),
            "canonical forwarding argument substitution"
        );
        // Retain block identities/counts but remove one checked-helper error edge.
        let mut changed = mir.functions().to_vec();
        let helper = &mir.functions()[checked];
        let mut blocks = helper.blocks().to_vec();
        let branch = blocks
            .iter()
            .position(|block| {
                matches!(
                    block.terminator().kind(),
                    SemanticTerminatorKindV1::SwitchInt { .. }
                )
            })
            .expect("original checked Result error/success branch");
        let original = &blocks[branch];
        blocks[branch] = SemanticBasicBlockV1::new(
            original.identity(),
            original.source(),
            original.statements().to_vec(),
            SemanticTerminatorV1::new(
                original.terminator().source(),
                SemanticTerminatorKindV1::Unreachable,
            ),
        )
        .unwrap();
        changed[checked] = function_with_blocks(helper, blocks);
        assert!(
            validate_canonical(
                tcx,
                plan,
                mir.types(),
                &changed,
                mir.callables(),
                &imported.kernel_contexts
            )
            .is_err(),
            "canonical checked helper error path erased"
        );
    }
    assert_eq!(checked_ids.len(), 4);
    validate_canonical(
        tcx,
        plan,
        mir.types(),
        mir.functions(),
        mir.callables(),
        &imported.kernel_contexts,
    )
    .unwrap();
}

fn function_with_blocks(
    function: &SemanticFunctionDeclV1,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
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
        blocks,
    )
    .unwrap()
}
