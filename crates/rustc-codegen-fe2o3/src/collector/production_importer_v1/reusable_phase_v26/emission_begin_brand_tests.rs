//! Runs only on the real two-phase callback's retained source and emitted KIR.
//! No independently constructed source commitment or replacement producer.
use fe2o3_kernel_ir::{
    ExecutionCapabilityRoleV1, Module, OperationKind, PhaseOperationSourceV1,
    ReusablePhaseOperationV1 as PhaseOp, Type,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticDefinedCapabilityContractV1 as Defined, SemanticDefinedReusablePhaseRecipeV1 as Recipe,
    SemanticTypeIdV1,
};
use fe2o3_pliron::ProductionSemanticSsaOwnerV1;

pub(super) struct ExpectedBegin {
    instance: u32,
    caller_instance: u32,
    original_block: u32,
    expanded_block: u32,
    original_normal_target: u32,
    expanded_normal_target: u32,
    root_source: [u8; 32],
    expansion: [u8; 32],
    expanded_root: [u8; 32],
    source_function: [u8; 32],
    source_operation: [u8; 32],
    source_binding: [u8; 32],
    owner: [u8; 32],
    execution_brand: [u8; 32],
    phantom_marker: [u8; 32],
    phase_brand: [u8; 32],
    initial_epoch: [u8; 32],
}

pub(super) fn collect(owner: &ProductionSemanticSsaOwnerV1) -> Vec<ExpectedBegin> {
    owner.verify_replay().unwrap();
    let source = owner.source_semantic();
    assert_eq!(source.roots().len(), 1);
    let root = source.roots()[0];
    let identity =
        |ty: SemanticTypeIdV1| *source.types()[ty.index() as usize].identity().as_bytes();
    let bindings = owner
        .execution_expansion()
        .defined_capability_bindings(source)
        .unwrap();
    let conversions = bindings
        .iter()
        .filter_map(|binding| {
            if binding.root() != root {
                return None;
            }
            let Defined::ReusablePhase(record) = binding.contract() else {
                return None;
            };
            let Recipe::OwnerConvert {
                owner,
                root_brand,
                outer_workgroup_brand,
                ..
            } = record.recipe()
            else {
                return None;
            };
            Some((
                identity(owner),
                identity(root_brand),
                identity(outer_workgroup_brand),
            ))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        conversions.len(),
        1,
        "actual fixture has one original reusable owner"
    );
    let mut result = Vec::new();
    for binding in &bindings {
        if binding.root() != root {
            continue;
        }
        let Defined::ReusablePhase(record) = binding.contract() else {
            continue;
        };
        let Recipe::Issue { owner: owner_type, brands, .. } = record.recipe() else {
            continue;
        };
        let execution_brand = identity(brands.root_brand);
        let phantom_marker = identity(brands.outer_workgroup_brand);
        assert_ne!(
            execution_brand, phantom_marker,
            "B and WorkgroupBrand<B> are different source types"
        );
        assert_eq!(
            conversions[0],
            (identity(owner_type), execution_brand, phantom_marker),
            "Issue retains the exact original owner parameter and its distinct layout marker"
        );
        let caller = &source.functions()[binding.caller_function().index() as usize];
        let fe2o3_mir_model::semantic_mir_v1::SemanticTerminatorKindV1::Call(original) =
            caller.blocks()[binding.call_block().index() as usize].terminator().kind()
        else { panic!("original Issue call"); };
        let normal = original.destination().unwrap().edge().target();
        let view = owner.execution_view_for_root(root).unwrap();
        let normal_blocks = view.block_origins().iter().enumerate()
            .filter(|(_, origin)| origin.instance() == binding.caller_instance()
                && origin.block() == normal && origin.function() == binding.caller_function())
            .map(|(index, _)| index as u32).collect::<Vec<_>>();
        assert_eq!(normal_blocks.len(), 1);
        result.push(ExpectedBegin {
            instance: binding.callee_instance().index(),
            caller_instance: binding.caller_instance().index(),
            original_block: binding.call_block().index(),
            expanded_block: binding.expanded_call_block().index(),
            original_normal_target: normal.index(),
            expanded_normal_target: normal_blocks[0],
            root_source: *source.functions()[root.index() as usize]
                .identity()
                .as_bytes(),
            expansion: *binding.expansion_identity(),
            expanded_root: *binding.root_identity(),
            source_function: *source.functions()[binding.caller_function().index() as usize]
                .identity()
                .as_bytes(),
            source_operation: *record.source_identity().as_bytes(),
            source_binding: *record.source_binding(),
            owner: identity(owner_type),
            execution_brand,
            phantom_marker,
            phase_brand: identity(brands.phase_brand),
            initial_epoch: identity(brands.dynamic_epoch),
        });
    }
    assert_eq!(
        result.len(),
        2,
        "both real Issue call occurrences must reach Begin"
    );
    assert_ne!(result[0].instance, result[1].instance);
    result
}

pub(super) fn check(module: &Module, expected: &[ExpectedBegin]) {
    let mut seen = vec![false; expected.len()];
    let mut original_owner_inputs = 0;
    let mut restored_owner_inputs = 0;
    let mut scoped_epochs = Vec::new();
    for function in &module.functions {
        let Some(body) = &function.body else {
            continue;
        };
        let operations = body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .collect::<Vec<_>>();
        for operation in &operations {
            let OperationKind::ReusablePhase(contract) = &operation.kind else {
                continue;
            };
            let PhaseOp::Begin {
                owner,
                outer_brand,
                phase_brand,
                dynamic_epoch,
                ..
            } = &contract.operation
            else {
                continue;
            };
            let PhaseOperationSourceV1::Defined(defined) = &contract.source else {
                panic!("Begin source");
            };
            let (slot, source) = expected
                .iter()
                .enumerate()
                .find(|(_, source)| source.instance == defined.call.callee_instance)
                .expect("Begin must consume an original checked Issue occurrence");
            assert!(!std::mem::replace(&mut seen[slot], true));
            assert_eq!(defined.call.source.function, source.source_function);
            assert_eq!(defined.call.source.operation, source.source_operation);
            assert_eq!(defined.call.source.block, source.original_block);
            assert_eq!(defined.source_binding, source.source_binding);
            let occurrence = defined.call.source.occurrence.unwrap();
            assert_eq!(occurrence.root_source_identity(), source.root_source);
            assert_eq!(occurrence.expansion_identity(), source.expansion);
            assert_eq!(occurrence.expanded_root_identity(), source.expanded_root);
            assert_eq!(occurrence.caller_instance(), source.caller_instance);
            assert_eq!(occurrence.expanded_block(), source.expanded_block);
            assert_eq!(owner.bytes(), source.owner);
            assert_eq!(*outer_brand, source.execution_brand);
            assert_ne!(*outer_brand, source.phantom_marker);
            assert_eq!(*phase_brand, source.phase_brand);
            assert_eq!(defined.call.original_normal_target, source.original_normal_target);
            assert_eq!(defined.call.expanded_normal_target, source.expanded_normal_target);
            let key = fe2o3_kernel_ir::PhaseKeyV1::for_begin(defined.call).unwrap();
            use sha2::{Digest as _, Sha256};
            let mut digest = Sha256::new();
            digest.update(b"fe2o3.kir.reusable-phase.source-epoch.v1\0");
            digest.update(key.bytes());
            digest.update(source.initial_epoch);
            let expected_epoch: [u8; 32] = digest.finalize().into();
            assert_eq!(*dynamic_epoch, expected_epoch);
            assert_ne!(*dynamic_epoch, source.initial_epoch);
            scoped_epochs.push(*dynamic_epoch);

            assert_eq!(contract.operands.len(), 1);
            let input = contract.operands[0];
            let producers = operations
                .iter()
                .filter_map(|producer| {
                    producer
                        .results
                        .iter()
                        .find(|result| result.id == input)
                        .map(|result| (*producer, result))
                })
                .collect::<Vec<_>>();
            assert_eq!(producers.len(), 1, "use the actual unique KIR producer");
            let (producer, result) = producers[0];
            let Type::ExecutionCapability(cap) = &result.ty else {
                panic!("actual owner type");
            };
            assert_eq!(cap.source_type.bytes(), source.owner);
            assert_eq!(cap.role, ExecutionCapabilityRoleV1::ReusableWorkgroup);
            assert_eq!(cap.provenance, contract.provenance);
            assert_eq!(cap.workgroup_brand, Some(source.execution_brand));
            assert!(cap.epoch.is_some());
            match &producer.kind {
                OperationKind::ReusablePhase(p)
                    if matches!(p.operation, PhaseOp::OwnerConvert { .. }) =>
                {
                    assert_eq!(producer.results[0].id, input);
                    original_owner_inputs += 1;
                }
                OperationKind::ReusablePhase(p) if matches!(p.operation, PhaseOp::End { .. }) => {
                    assert_eq!(producer.results[0].id, input);
                    restored_owner_inputs += 1;
                }
                other => {
                    panic!("Begin substituted its original/restored owner producer: {other:?}")
                }
            }
            let inputs = [(input, &result.ty)];
            let first = operation.results[0].id;
            assert!(input.0 < first.0);
            let (replayed, _) = contract.clone().checked_operation(&inputs, first).unwrap();
            assert_eq!(replayed, **operation);

            // The old wrong namespace must remain rejected by the shared gate.
            let mut wrong_descriptor = contract.clone();
            let PhaseOp::Begin { outer_brand, .. } = &mut wrong_descriptor.operation else {
                unreachable!()
            };
            *outer_brand = source.phantom_marker;
            assert!(wrong_descriptor.checked_operation(&inputs, first).is_none());
            let mut wrong_cap = cap.clone();
            wrong_cap.workgroup_brand = Some(source.phantom_marker);
            let wrong_type = Type::ExecutionCapability(wrong_cap);
            assert!(
                contract
                    .clone()
                    .checked_operation(&[(input, &wrong_type)], first)
                    .is_none()
            );
            assert!(
                contract
                    .clone()
                    .checked_operation(&[(first, &result.ty)], first)
                    .is_none()
            );
            assert!(contract.clone().checked_operation(&inputs, input).is_none());
            let mut wrong_epoch = cap.clone();
            wrong_epoch.epoch = None;
            assert!(
                contract
                    .clone()
                    .checked_operation(&[(input, &Type::ExecutionCapability(wrong_epoch))], first)
                    .is_none()
            );
            assert_eq!(cap.workgroup_brand, Some(source.execution_brand));
        }
    }
    assert!(seen.into_iter().all(|value| value));
    assert_eq!((original_owner_inputs, restored_owner_inputs), (1, 1));
    assert_eq!(scoped_epochs.len(), 2);
    assert_ne!(scoped_epochs[0], scoped_epochs[1]);
}
