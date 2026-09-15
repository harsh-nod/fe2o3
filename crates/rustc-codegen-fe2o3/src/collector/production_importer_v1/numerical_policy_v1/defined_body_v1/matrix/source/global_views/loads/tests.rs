use super::*;
use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1 as Expansion;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBasicBlockV1, SemanticTerminatorKindV1, SemanticTerminatorV1,
};
use rustc_middle::mir::{TerminatorKind, UnwindAction};

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
        .filter(|(_, producer)| kind(tcx, producer.instance.def_id()).is_some())
        .collect::<Vec<_>>();
    assert_eq!(candidates.len(), 4, "all four original Global load bodies");
    let validate = |functions: &[SemanticFunctionDeclV1], callables: &[SemanticCallableDeclV1]| {
        validate_canonical(
            tcx,
            plan,
            mir.types(),
            functions,
            callables,
            &imported.kernel_contexts,
        )
    };
    for (index, producer) in candidates {
        let function = SemanticFunctionIdV1::from_index(index as u32);
        let instance = producer.instance;
        let facts = validate_source(tcx, instance).unwrap().unwrap();
        assert_eq!(
            mir.callables()[index],
            SemanticCallableDeclV1::defined(function)
        );
        assert!(
            mir.functions()[index]
                .defined_capability_contract()
                .is_none()
        );
        assert_eq!(mir.functions()[index].abi().source_input_types().len(), 4);
        let (format, role) = kind(tcx, instance.def_id()).unwrap();
        for (axis, argument) in instance.args.iter().enumerate() {
            if argument.as_type().is_none() {
                continue;
            }
            let mut arguments = instance.args.to_vec();
            arguments[axis] = tcx.types.u16.into();
            assert!(
                validate_source(
                    tcx,
                    Instance {
                        args: tcx.mk_args(&arguments),
                        ..instance
                    }
                )
                .is_err(),
                "load source generic axis {axis}"
            );
        }
        let original = tcx.instance_mir(instance.def);
        let check_body = |candidate| {
            body::wrapper(
                tcx,
                instance,
                candidate,
                facts.inputs,
                facts.output,
                facts.registers,
                format,
                role,
            )
        };
        assert!(check_body(original).is_some());
        let packing_block = original.basic_blocks.iter_enumerated().find_map(|(id, block)| {
            matches!(&block.terminator().kind, TerminatorKind::Call { args, .. } if args.len() == 4).then_some(id)
        }).unwrap();
        let mut swapped = original.clone();
        let TerminatorKind::Call { args, .. } = &mut swapped.basic_blocks.as_mut()[packing_block]
            .terminator_mut()
            .kind
        else {
            panic!("packer")
        };
        args.swap(2, 3);
        assert!(
            check_body(&swapped).is_none(),
            "two same-typed bases cannot be exchanged"
        );
        let mut skipped = original.clone();
        let TerminatorKind::Call { target, .. } = &mut skipped.basic_blocks.as_mut()[packing_block]
            .terminator_mut()
            .kind
        else {
            panic!("packer")
        };
        *target = Some(packing_block);
        assert!(
            check_body(&skipped).is_none(),
            "original normal-edge protocol"
        );
        let mut unwind = original.clone();
        let TerminatorKind::Call { unwind: action, .. } = &mut unwind.basic_blocks.as_mut()
            [packing_block]
            .terminator_mut()
            .kind
        else {
            panic!("packer")
        };
        *action = UnwindAction::Continue;
        assert!(check_body(&unwind).is_none());
        let TyKind::Adt(_, args) = *facts.output.kind() else {
            panic!("fragment")
        };
        let args = args.types().collect::<Vec<_>>();
        assert!(layout::fragment(tcx, facts.output, args[0], args[1], args[2]).is_some());
        assert!(layout::fragment(tcx, facts.output, args[1], args[0], args[2]).is_none());
        assert!(layout::fragment(tcx, facts.output, args[0], args[1], facts.root.ty).is_none());
        let mut callables = mir.callables().to_vec();
        let helper = plan
            .function_producers()
            .iter()
            .position(|producer| producer.instance == facts.helpers[1])
            .unwrap();
        callables[index] =
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(helper as u32));
        assert!(
            validate(mir.functions(), &callables).is_err(),
            "load source identity cannot name the packer"
        );
        let selected =
            replay::descendants(plan, mir.functions().len(), BTreeSet::from([function])).unwrap();
        let reads = plan
            .terminal_expansion_producers()
            .iter()
            .filter(|call| {
                selected.contains(&call.caller) && call.expansion == Expansion::CapabilityGlobalLoad
            })
            .collect::<Vec<_>>();
        assert_eq!(
            reads.len(),
            1,
            "retained checked byte-read edge, never a raw or BF16 load"
        );
        // Erase the actual source read's call while retaining canonical identity,
        // ABI, block count and source labels. Full body replay must reject it.
        let read_owner = reads[0].caller.index() as usize;
        let read_block = plan.body_producers()[read_owner]
            .blocks
            .iter()
            .position(|block| block.rustc_block == reads[0].block)
            .unwrap();
        let mut changed = mir.functions().to_vec();
        changed[read_owner] = erase_terminator(&changed[read_owner], read_block);
        assert!(
            validate(&changed, mir.callables()).is_err(),
            "checked memory read erased"
        );
        let zero_owner = selected
            .iter()
            .find_map(|id| {
                let function = &mir.functions()[id.index() as usize];
                function
                    .blocks()
                    .iter()
                    .position(|block| {
                        matches!(
                            block.terminator().kind(),
                            SemanticTerminatorKindV1::SwitchInt { .. }
                        )
                    })
                    .map(|block| (id.index() as usize, block))
            })
            .expect("original checked/zero-fill branch remains defined");
        let mut changed = mir.functions().to_vec();
        changed[zero_owner.0] = erase_terminator(&changed[zero_owner.0], zero_owner.1);
        assert!(
            validate(&changed, mir.callables()).is_err(),
            "zero-fill or loop guard erased"
        );
        let packing = &mir.functions()[helper];
        let (block, statement) = packing
            .blocks()
            .iter()
            .enumerate()
            .find_map(|(i, block)| (!block.statements().is_empty()).then_some((i, 0)))
            .expect("retained integer packing assignments");
        let mut blocks = packing.blocks().to_vec();
        let original = &blocks[block];
        let mut statements = original.statements().to_vec();
        statements.remove(statement);
        blocks[block] = SemanticBasicBlockV1::new(
            original.identity(),
            original.source(),
            statements,
            original.terminator().clone(),
        )
        .unwrap();
        let mut changed = mir.functions().to_vec();
        changed[helper] = function_with_blocks(packing, blocks);
        assert!(
            validate(&changed, mir.callables()).is_err(),
            "packing arithmetic or initialization erased"
        );
    }
    validate(mir.functions(), mir.callables()).unwrap();
}

fn erase_terminator(function: &SemanticFunctionDeclV1, index: usize) -> SemanticFunctionDeclV1 {
    let mut blocks = function.blocks().to_vec();
    let original = &blocks[index];
    blocks[index] = SemanticBasicBlockV1::new(
        original.identity(),
        original.source(),
        original.statements().to_vec(),
        SemanticTerminatorV1::new(
            original.terminator().source(),
            SemanticTerminatorKindV1::Unreachable,
        ),
    )
    .unwrap();
    function_with_blocks(function, blocks)
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
