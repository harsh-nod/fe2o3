mod global_wrapper_transport_root_tests_v1 {
    use super::super::*;
    use global_enum_transport_v1::tests::{build_owner, initial, joined};

    #[test]
    fn wrapper_transport_statement_hook_retains_exact_global_after_scalar_copy() {
        let owner = build_owner();
        let types = owner.source_semantic().types();
        let function = owner
            .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
            .unwrap()
            .body();
        let reads = private_scalar_capture_v1::PrivateScalarReads::for_root(
            &owner,
            SemanticFunctionIdV1::from_index(0),
        )
        .unwrap();
        let conditions = reads.conditions_for(types, function).unwrap();
        let mut state = joined(types, function, conditions);
        let dominance = SemanticEnumPayloadDominanceV1::analyze(function, types).unwrap();
        let mut uses = ProjectedGlobalSemanticUsesV1::default();
        transfer_capability_statements_with_transport_v1(
            types,
            function,
            4,
            &mut state,
            &dominance,
            Some(&mut uses),
            None,
            Some(&reads),
        )
        .unwrap();
        assert_eq!(state.get(&10), initial().get(&2));
        assert_eq!(state.get(&5), Some(&ProjectedCapabilityValueV1::Invalid));
        assert!(matches!(
            state.get(&8),
            Some(ProjectedCapabilityValueV1::CapturedGlobal(_))
        ));
    }

    #[test]
    fn wrapper_transport_statement_hook_cannot_consume_unbound_read_evidence() {
        let owner = build_owner();
        let types = owner.source_semantic().types();
        let function = owner
            .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
            .unwrap()
            .body();
        let reads = private_scalar_capture_v1::PrivateScalarReads::for_root(
            &owner,
            SemanticFunctionIdV1::from_index(0),
        )
        .unwrap();
        let initial = joined(
            types,
            function,
            reads.conditions_for(types, function).unwrap(),
        );
        let dominance = SemanticEnumPayloadDominanceV1::analyze(function, types).unwrap();
        let mut without = initial.clone();
        transfer_capability_statements_with_transport_v1(
            types,
            function,
            4,
            &mut without,
            &dominance,
            None,
            None,
            None,
        )
        .unwrap();
        assert!(!without.contains_key(&10));
        let foreign = function.clone();
        let mut state = initial;
        transfer_capability_statements_with_transport_v1(
            types,
            &foreign,
            4,
            &mut state,
            &dominance,
            None,
            None,
            Some(&reads),
        )
        .unwrap();
        assert!(!state.contains_key(&10));
    }
}
