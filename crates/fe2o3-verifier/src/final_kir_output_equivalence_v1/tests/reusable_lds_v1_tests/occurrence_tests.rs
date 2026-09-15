//! Inert transfer-model fixtures; no retained Rust-source or protected proof claim.
use super::*;
use fe2o3_kernel_ir::{
    DiagnosticCode, VerifiedCanonicalKernelIrErrorV13, decode_execution_capability_contract_v1,
    encode_execution_capability_contract_v1,
};

fn contract(module: &Module, index: usize) -> &ExecutionCapabilityOpV1 {
    let OperationKind::ExecutionCapability(contract) = &operations(module)[index].kind else {
        panic!("execution contract")
    };
    contract
}

fn exact_malformed(module: &Module) {
    match Transfers::collect(&module.functions[0], &mut 0) {
        Err(error) => assert_eq!(error, FinalKirOutputEquivalenceErrorV1::MalformedValueFlow),
        Ok(_) => panic!("the exact final-transfer source guard must reject"),
    }
}

fn changed_identity_rejects(axis: usize) {
    assert!(matches!(axis, 1 | 2));
    for borrowed in [false, true] {
        for changed_operation in [2, 3] {
            let mut module = logical_module(borrowed, None);
            VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
            Transfers::collect(&module.functions[0], &mut 0).unwrap();
            let original = contract(&module, changed_operation).clone();
            let before = original.source.occurrence.unwrap();
            let mut identities = [
                before.root_source_identity(),
                before.expansion_identity(),
                before.expanded_root_identity(),
            ];
            identities[axis][0] ^= 1;
            let after = ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
                identities[0],
                identities[1],
                identities[2],
                before.caller_instance(),
                before.expanded_block(),
            )
            .unwrap();
            assert_eq!(after.root_source_identity(), before.root_source_identity());
            assert_eq!(after.caller_instance(), before.caller_instance());
            assert_eq!(after.expanded_block(), before.expanded_block());
            if axis == 1 {
                assert_ne!(after.expansion_identity(), before.expansion_identity());
                assert_eq!(
                    after.expanded_root_identity(),
                    before.expanded_root_identity()
                );
            } else {
                assert_eq!(after.expansion_identity(), before.expansion_identity());
                assert_ne!(
                    after.expanded_root_identity(),
                    before.expanded_root_identity()
                );
            }
            contract_mut(&mut module, changed_operation)
                .source
                .occurrence = Some(after);
            assert!(contract(&module, changed_operation).is_complete());
            let mut restored = contract(&module, changed_operation).clone();
            restored.source.occurrence = Some(before);
            assert_eq!(restored, original, "no second contract field changed");
            // Exercise the final adapter guard directly, not an earlier canonical
            // source-mode rejection that could hide a missing final comparison.
            exact_malformed(&module);
        }
    }
}

#[test]
fn reusable_lds_whole_expansion_identity_mismatch_rejects_independently() {
    changed_identity_rejects(1);
}

#[test]
fn reusable_lds_expanded_root_identity_mismatch_rejects_independently() {
    changed_identity_rejects(2);
}

#[test]
fn reusable_lds_legacy_op2_none_allocation_occurrence_transfers_without_new_memory() {
    let mut module = logical_module(false, None);
    VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    let original_allocation = contract(&module, 2).clone();
    let original_conversion = contract(&module, 3).clone();
    contract_mut(&mut module, 2).source.occurrence = None;
    let allocation = contract(&module, 2);
    assert!(matches!(
        allocation.operation,
        ExecutionCapabilityOperationV1::LdsAllocate { .. }
    ));
    assert!(allocation.source.occurrence.is_none());
    assert_eq!(contract(&module, 3), &original_conversion);
    assert!(original_conversion.source.occurrence.is_some());
    let mut restored = allocation.clone();
    restored.source.occurrence = original_allocation.source.occurrence;
    assert_eq!(restored, original_allocation);
    let payload = encode_execution_capability_contract_v1(allocation).unwrap();
    assert_eq!(&payload[..2], &[1, 2], "unchanged legacy revision/opcode");
    assert_eq!(
        decode_execution_capability_contract_v1(&payload, allocation.operands.clone()),
        Some(allocation.clone())
    );

    let mut nodes = 0;
    let transfers = Transfers::collect(&module.functions[0], &mut nodes).unwrap();
    let mut state = initial_state();
    allocate(&module, &mut state);
    let Some(SymbolicValueV1::WorkgroupView(before)) = state.values.get(&ValueId(102)).cloned()
    else {
        panic!("real op2 allocation, not a fabricated reusable value");
    };
    let memories = format!("{:?}", state.memories);
    let events = format!("{:?}", state.events);
    let written = state.written_roots.clone();
    let path = format!("{:?}", state.path);
    let visits = state.visits.clone();
    let counters = (
        state.next_private_allocation,
        state.next_workgroup_allocation,
    );
    transfers
        .execute(&operations(&module)[3], &mut state, &mut nodes)
        .unwrap();
    let Some(SymbolicValueV1::ReusableLds(after)) = state.values.get(&ValueId(103)) else {
        panic!("dormant reusable handle");
    };
    assert_eq!(after.view.memory_root, before.memory_root);
    assert_eq!(after.view.layout, before.layout);
    assert_eq!(after.view.elements, before.elements);
    assert!(!after.view.initialized && !after.view.published);
    assert!(value_matches_type(
        &SymbolicValueV1::ReusableLds(after.clone()),
        &operations(&module)[3].results[0].ty
    ));
    assert!(!state.values.contains_key(&ValueId(102)));
    assert_eq!(format!("{:?}", state.memories), memories);
    assert_eq!(format!("{:?}", state.events), events);
    assert_eq!(state.written_roots, written);
    assert_eq!(format!("{:?}", state.path), path);
    assert_eq!(state.visits, visits);
    assert_eq!(
        (
            state.next_private_allocation,
            state.next_workgroup_allocation
        ),
        counters
    );
    let saved = format!("{state:?}");
    assert_eq!(
        transfers.execute(&operations(&module)[3], &mut state, &mut nodes),
        Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow)
    );
    assert_eq!(format!("{state:?}"), saved);
    drop(transfers);

    // This is the legacy final-transfer API positive, not a mixed-mode whole-KIR
    // positive. The independent canonical source-custody gate stays unchanged.
    let Err(VerifiedCanonicalKernelIrErrorV13::Verification(errors)) =
        VerifiedCanonicalKernelIrV13::from_module(module)
    else {
        panic!("the mixed original/expanded canonical source gate must remain");
    };
    assert!(
        errors.diagnostics().iter().any(|diagnostic| diagnostic.code
            == DiagnosticCode::InvalidExecutionCapability
            && diagnostic.message
                == "execution source occurrences disagree on their root, expansion, or mode"),
        "{errors:?}"
    );
}

#[test]
fn reusable_lds_legacy_op2_still_requires_conversion_occurrence() {
    for allocation_occurrence in [false, true] {
        let mut module = logical_module(false, None);
        VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
        if !allocation_occurrence {
            contract_mut(&mut module, 2).source.occurrence = None;
        }
        let allocation = contract(&module, 2).clone();
        let conversion = contract(&module, 3).clone();
        assert!(conversion.source.occurrence.is_some());
        Transfers::collect(&module.functions[0], &mut 0).unwrap();
        contract_mut(&mut module, 3).source.occurrence = None;
        assert_eq!(contract(&module, 2), &allocation);
        let mut restored = contract(&module, 3).clone();
        restored.source.occurrence = conversion.source.occurrence;
        assert_eq!(restored, conversion);
        exact_malformed(&module);
    }
}
