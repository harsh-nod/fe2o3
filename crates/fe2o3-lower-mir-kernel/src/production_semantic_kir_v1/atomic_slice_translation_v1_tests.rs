#[test]
fn atomic_slice_translation_preserves_address_ordering_and_system_scope() {
    // This exercises the private projection reconciler, not source admission or
    // a fabricated production owner. The source-extraction tests cover custody.
    let fixture = || {
        let mut fixture = unsupported_index_correlation_fixture();
        let scalar = Type::Scalar(ScalarType::U32);
        let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
        let function = &mut fixture.module.functions[0];
        function.signature = Signature::new(
            vec![Type::slice(
                scalar.clone(),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            )],
            vec![],
        );
        let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
        operations[2].results[0].ty = pointer.clone();
        operations[3].results[0].ty = pointer;
        operations[4].results[0].ty = scalar;
        operations[4].kind = OperationKind::Atomic(Atomic {
            kind: AtomicKind::Load,
            pointer: ValueId(4),
            value: None,
            compare: None,
            access: MemoryAccess::new(AddressSpace::Global, 4),
            scope: SynchronizationScope::System,
            ordering: MemoryOrdering::Acquire,
            failure_ordering: None,
        });
        verify_module(&fixture.module).unwrap();
        fixture
    };
    let ranked = |ordering, scope| {
        ranked_correlation_input_with_coherence_v1(
            vec![ProductionRankedOperationV1::AtomicAccess {
                kind: AccessKindAttr::AtomicRead,
                ordering,
                scope,
                view: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
                indices: vec![ProductionRankedValueV1::Local(
                    ProductionRankedValueIdV1::new(1),
                )],
            }],
            1,
            &[1],
        )
    };
    let exact = ranked(
        dialect_kernel::AtomicOrderingAttr::Acquire,
        dialect_kernel::AtomicScopeAttr::System,
    );
    let original = fixture();
    assert!(validate_translation_fixture(&original, &exact, &[original.source], 16).is_ok());
    for mismatch in [
        ranked(
            dialect_kernel::AtomicOrderingAttr::Relaxed,
            dialect_kernel::AtomicScopeAttr::System,
        ),
        ranked(
            dialect_kernel::AtomicOrderingAttr::Acquire,
            dialect_kernel::AtomicScopeAttr::Agent,
        ),
    ] {
        assert!(matches!(
            validate_translation_fixture(&original, &mismatch, &[original.source], 16),
            Err(ProductionMirPlironTranslationErrorV1::AtomicContractMismatch { .. })
        ));
    }
    for replacement in [ValueId(1), ValueId(0)] {
        let mut altered = fixture();
        let OperationKind::GetElementPointer { offset, .. } =
            &mut altered.module.functions[0].body.as_mut().unwrap().blocks[0].operations[3].kind
        else {
            unreachable!()
        };
        *offset = replacement;
        assert!(!unsupported_indices_match_ranked_sources(
            &altered.module,
            &altered.correspondence,
            &exact,
            &[altered.source],
            &altered.reasons,
            16
        ));
    }
    let mut altered = fixture();
    let OperationKind::Atomic(atomic) =
        &mut altered.module.functions[0].body.as_mut().unwrap().blocks[0].operations[4].kind
    else {
        unreachable!()
    };
    atomic.pointer = ValueId(3);
    verify_module(&altered.module).unwrap();
    assert!(!unsupported_indices_match_ranked_sources(
        &altered.module,
        &altered.correspondence,
        &exact,
        &[altered.source],
        &altered.reasons,
        16
    ));
}
