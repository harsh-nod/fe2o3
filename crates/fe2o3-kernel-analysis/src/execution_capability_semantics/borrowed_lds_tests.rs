use super::*;
mod fixture {
    use fe2o3_kernel_ir::*;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-kernel-ir/src/execution_capability_v1/borrowed_lds/fixture.rs"
    ));
}

#[test]
fn borrowed_lds_lifecycle_is_uninitialized_then_dormant_without_publication() {
    let module = fixture::module(true, true);
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    let mut analyzer = Analyzer {
        module: &module,
        identity: *canonical.identity(),
        final_epoch: 7,
        checked_functions: BTreeSet::new(),
        checked_operations: 0,
        checked_barrier_sites: 0,
        checked_collective_sites: 0,
        checked_epoch_transitions: 0,
        checked_atomic_operations: 0,
        checked_memory_effects: 0,
        call_edges: 0,
        flow_states: 0,
        sequence_states: 0,
        conflicting_effects_composed_with_retained_pliron: false,
        findings: Vec::new(),
    };
    let mut state = FlowState::default();
    for index in 2..=3 {
        let contract = fixture::contract_at(&module, index);
        let location = ExecutionCapabilitySemanticLocationV1 {
            root: module.kernels[0].id.clone(),
            function: module.functions[0].id.clone(),
            block: BlockId(0),
            operation: index,
            source: Some(contract.source),
        };
        analyzer.apply_lifecycle(
            contract,
            &fixture::operations(&module)[index],
            &location,
            &mut state,
        );
        if index == 2 {
            assert_eq!(
                state.lds,
                BTreeMap::from([(ValueId(90), BTreeSet::from([LdsPhase::Uninitialized]))])
            );
        }
    }
    assert!(analyzer.findings.is_empty(), "{:?}", analyzer.findings);
    assert_eq!(
        state.lds,
        BTreeMap::from([
            (ValueId(90), BTreeSet::from([LdsPhase::Consumed])),
            (ValueId(91), BTreeSet::from([LdsPhase::ReusableDormant])),
        ])
    );
    assert!(state.epochs.is_empty());
    assert!(state.views.is_empty());
    assert!(state.publication_ready.is_empty());
    assert!(state.pending_global_writes.is_empty());
    assert!(state.relaxed_acquire.is_none());
}

#[test]
fn borrowed_lds_final_graph_adds_no_barrier_or_epoch_transition() {
    let module = fixture::module(true, true);
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    let report = analyze_module(&module, *canonical.identity(), 7, false);
    assert_eq!(report.checked_operations(), 4);
    // This counter records accesses, not allocation declarations.
    assert_eq!(report.checked_memory_effects(), 0);
    assert_eq!(report.checked_barrier_sites(), 0);
    assert_eq!(report.checked_collective_sites(), 0);
    assert_eq!(report.checked_epoch_transitions(), 0);
    // This report does not supply retained alias/lifetime or machine authority.
    assert!(!report.conflicting_effects_composed_with_retained_pliron());
}
