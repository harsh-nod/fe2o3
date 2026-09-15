mod abi_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::*;

    #[test]
    fn source_call_abi_checks_exact_argument_and_destination_types() {
        let unit = SemanticTypeIdV1::from_index(0);
        let argument = SemanticTypeIdV1::from_index(1);
        let foreign = SemanticTypeIdV1::from_index(2);
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([1; 32]),
            SemanticLayoutIdentityV1::from_sha256([2; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                argument,
                SemanticAbiPassModeV1::Ignore,
            ))],
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let call = |input: Option<SemanticTypeIdV1>, output: Option<SemanticTypeIdV1>| {
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                input
                    .into_iter()
                    .map(|ty| {
                        SemanticOperandV1::Copy(
                            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], ty)
                                .unwrap(),
                        )
                    })
                    .collect(),
                output.map(|ty| {
                    SemanticCallDestinationV1::new(
                        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], ty).unwrap(),
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::CallReturn,
                            SemanticBlockIdV1::from_index(1),
                        ),
                    )
                }),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap()
        };
        assert!(require_abi(&call(Some(argument), Some(unit)), &abi).is_ok());
        for (input, output) in [
            (Some(foreign), Some(unit)),
            (Some(argument), Some(foreign)),
            (None, Some(unit)),
            (Some(argument), None),
        ] {
            assert!(require_abi(&call(input, output), &abi).is_err());
        }
    }
}
