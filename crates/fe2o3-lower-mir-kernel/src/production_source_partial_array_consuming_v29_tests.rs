thread_local! {
    static PARTIAL_ARRAY_FAULT_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PARTIAL_ARRAY_CHANGED_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn mutate_original_partial_array_read_v29(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    inspect_original_literal_array_v29(source, instances, emitted, slots, budget)?;
    let fault = PARTIAL_ARRAY_FAULT_V29.get();
    let mut changed = 0;
    for slot in &slots.slots {
        let lowered = emitted[slot.instance.index()].as_mut().unwrap();
        let original = instances.instance(slot.instance).unwrap().declaration();
        let SemanticStatementKindV1::Assign(statement) =
            original.blocks()[1].statements()[0].kind()
        else {
            panic!("original partial array read");
        };
        let SemanticRvalueKindV1::Load(load) = statement.value().kind() else {
            panic!("original Load");
        };
        assert!(matches!(
            load.source().projections()[0].kind(),
            SemanticProjectionKindV1::ConstantIndex {
                offset: 0,
                minimum_length: 2,
                from_end: false
            }
        ));
        let kernel_block = lowered
            .blocks
            .iter()
            .find(|row| row.semantic_block.index() == 1)
            .unwrap()
            .kernel_ir_block;
        let block = lowered
            .function
            .body
            .as_mut()
            .unwrap()
            .blocks
            .iter_mut()
            .find(|block| block.id == kernel_block)
            .unwrap();
        let read = block
            .operations
            .iter()
            .position(|operation| matches!(operation.kind, OperationKind::Load { .. }))
            .unwrap();
        let OperationKind::Load { pointer, access } = &mut block.operations[read].kind else {
            unreachable!()
        };
        let pointer = *pointer;
        assert_eq!(access.alignment, 4);
        if fault == 1 || fault == 2 {
            access.alignment = if fault == 1 { 8 } else { 2 };
        } else {
            assert_eq!(fault, 0);
            let gep = block
                .operations
                .iter()
                .find(|operation| operation.results.iter().any(|result| result.id == pointer))
                .unwrap();
            let OperationKind::GetElementPointer { base, offset } = gep.kind else {
                panic!("literal element pointer");
            };
            assert_eq!(base, slot.origin.pointer);
            let offset = block
                .operations
                .iter_mut()
                .find(|operation| operation.results.iter().any(|result| result.id == offset))
                .unwrap();
            assert_eq!(offset.kind, OperationKind::Constant(Constant::Index(0)));
            offset.kind = OperationKind::Constant(Constant::Index(1));
        }
        changed += 1;
    }
    assert_eq!(
        changed, 2,
        "both repeated original array reads are actually changed"
    );
    PARTIAL_ARRAY_CHANGED_V29.set(PARTIAL_ARRAY_CHANGED_V29.get() + changed);
    // The same altered candidate must continue through assembly and the final
    // original-source/physical checks. No independent clean re-emission here.
    Ok(())
}

#[test]
fn original_partial_array_read_rejects_same_candidate_index_and_alignment_changes() {
    for fault in 0..2 {
        let (positive, _, _, completed) = run_original_repeated_source_v29(
            || initialized_literal_array_owner_v29(false),
            inspect_original_literal_array_v29,
            10_000_000,
            10_000_000,
        );
        assert!(positive.is_ok() && completed, "{positive:?}");
        assert_eq!(OBSERVED.get(), 3);
        PARTIAL_ARRAY_FAULT_V29.set(fault);
        PARTIAL_ARRAY_CHANGED_V29.set(0);
        let (refused, _, _, completed) = run_original_repeated_source_v29(
            || initialized_literal_array_owner_v29(false),
            mutate_original_partial_array_read_v29,
            10_000_000,
            10_000_000,
        );
        assert!(!completed, "fault {fault}: {refused:?}");
        assert_eq!(OBSERVED.get(), 1);
        assert_eq!(PARTIAL_ARRAY_CHANGED_V29.get(), 2);
        let expected = match fault {
            0 => "scoped slot read is not initialized in its fresh physical activation",
            1 => "scoped slot access exceeds its cell bounds or alignment",
            _ => unreachable!(),
        };
        assert!(
            matches!(refused,
            Err(ProductionSourceOwnedViewErrorV18::Source(ProductionPendingScopedSourceErrorV29::Source(
                ProductionSemanticKirErrorV1::Unsupported {
                    function: 3, detail, ..
                }))) if detail == expected),
            "fault {fault}: {refused:?}"
        );
    }
}

#[test]
fn original_partial_array_read_accepts_only_conservative_alignment_refinement() {
    let (positive, _, _, completed) = run_original_repeated_source_v29(
        || initialized_literal_array_owner_v29(false),
        inspect_original_literal_array_v29,
        10_000_000,
        10_000_000,
    );
    assert!(positive.is_ok() && completed, "{positive:?}");
    PARTIAL_ARRAY_FAULT_V29.set(2);
    PARTIAL_ARRAY_CHANGED_V29.set(0);
    let (refined, _, _, completed) = run_original_repeated_source_v29(
        || initialized_literal_array_owner_v29(false),
        mutate_original_partial_array_read_v29,
        10_000_000,
        10_000_000,
    );
    assert!(refined.is_ok() && completed, "{refined:?}");
    assert_eq!(OBSERVED.get(), 3);
    assert_eq!(PARTIAL_ARRAY_CHANGED_V29.get(), 6);
}
