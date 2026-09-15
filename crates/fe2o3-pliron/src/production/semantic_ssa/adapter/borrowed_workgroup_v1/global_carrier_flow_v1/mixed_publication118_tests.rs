//! Actual classifier construction/publication with inert capability metadata.
//! This is not authenticated kernel issuance or admitted source/SSA evidence.
use super::*;

fn subgroup() -> SemanticCallableDeclV1 {
    let SemanticCallableDeclV1::CompilerIntrinsic {
        operation: SemanticCompilerIntrinsicOperationV1::GlobalBf16MatrixLoad { contract: global },
        ..
    } = callable(0, 62)
    else {
        unreachable!()
    };
    let identity = SemanticFunctionIdentityV1::from_sha256([242; 32]);
    let contract = SemanticExecutionCapabilityContractV1::new(
        E::SubgroupDeriveBorrowed {
            workgroup_reference: ty(11),
            workgroup: ty(10),
            subgroup: ty(0),
            width: 64,
        },
        SemanticExecutionCapabilitySignatureV1::new(&[ty(11)], ty(0)).unwrap(),
        global.provenance(),
        SemanticTypeIdentityV1::from_sha256([243; 32]),
        SemanticTypeIdentityV1::from_sha256([244; 32]),
        None,
        identity,
    )
    .unwrap();
    let value = |id| SemanticAbiValueV1::new(ty(id), SemanticAbiPassModeV1::Ignore);
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([242; 32]),
        SemanticLayoutIdentityV1::from_sha256([242; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![value(11)],
        value(0),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow])
    .unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            identity,
            SemanticItemDefinitionIdentityV1::from_sha256([242; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([242; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([242; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([242; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([242; 32]),
    }
}

fn fixture(escape: Option<u32>) -> (Vec<SemanticTypeDeclV1>, SemanticFunctionDeclV1) {
    let (types, original) = mixed(escape);
    let mut locals = original.locals().to_vec();
    for (tag, t) in [(245, 11), (246, 0)] {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([tag; 32]),
            ty(t),
            SemanticLocalRoleV1::Temporary,
            original.source(),
        ));
    }
    let block = &original.blocks()[0];
    let mut statements = block.statements().to_vec();
    statements.push(SemanticStatementV1::new(
        original.source(),
        copy(16, 11, projected(7, 2, 11)),
    ));
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(1),
        vec![SemanticOperandV1::Copy(place(16, ty(11)))],
        Some(SemanticCallDestinationV1::new(
            place(17, ty(0)),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(1),
            ),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let blocks = vec![
        SemanticBasicBlockV1::new(
            block.identity(),
            original.source(),
            statements,
            SemanticTerminatorV1::new(original.source(), SemanticTerminatorKindV1::Call(call)),
        )
        .unwrap(),
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([247; 32]),
            original.source(),
            vec![],
            block.terminator().clone(),
        )
        .unwrap(),
    ];
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        original.entry(),
        blocks,
    )
    .unwrap();
    (types, function)
}

fn classify(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    limit: usize,
) -> Result<BTreeSet<SemanticTransparentBorrowSiteV1>, ProductionSemanticSsaErrorV1> {
    // Unlike typed_direct_sites, do not turn a resource failure into an empty set.
    super::super::super::super::sites(
        function,
        &[callable(0, 62), subgroup()],
        &[],
        limit,
        Some(types),
    )
}

#[test]
fn mixed_global_publication118_real_route_discovery_and_late_global_veto() {
    let (types, function) = fixture(None);
    let expected = [1, 2, 3, 9]
        .into_iter()
        .map(|statement| SemanticTransparentBorrowSiteV1 {
            block: 0,
            statement,
        })
        .collect();
    assert_eq!(
        classify(&types, &function, MAX_FLOW_WORK).unwrap(),
        expected
    );
    for local in [4, 5, 8, 9] {
        let (types, function) = fixture(Some(local));
        assert!(
            classify(&types, &function, MAX_FLOW_WORK)
                .unwrap()
                .is_empty(),
            "late Global escape {local} must withhold the primary Borrow too"
        );
    }
}

fn work_error(error: ProductionSemanticSsaErrorV1, limit: usize) {
    let ProductionSemanticSsaErrorV1::BorrowFlowWork {
        phase_work_units,
        remaining_work_units,
        requested_work_units,
        error,
        ..
    } = error
    else {
        panic!("expected work failure: {error:?}")
    };
    assert_eq!(
        phase_work_units.iter().sum::<usize>() + remaining_work_units,
        limit
    );
    assert!(requested_work_units > remaining_work_units);
    assert_eq!(
        *error,
        ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits,
            required: limit + 1,
            limit,
        }
    );
}

#[test]
fn mixed_global_publication118_one_budget_includes_defer_finish_resolve_and_publication() {
    for escape in [None, Some(4)] {
        let (types, function) = fixture(escape);
        let expected = classify(&types, &function, MAX_FLOW_WORK).unwrap();
        let (mut low, mut high) = (0, MAX_FLOW_WORK);
        // At most19 independent runs, each with a single production allowance.
        while low < high {
            let middle = low + (high - low) / 2;
            match classify(&types, &function, middle) {
                Ok(sites) => {
                    assert_eq!(sites, expected);
                    high = middle;
                }
                Err(error) => {
                    work_error(error, middle);
                    low = middle + 1;
                }
            }
        }
        assert!(low > 0);
        assert_eq!(classify(&types, &function, low).unwrap(), expected);
        work_error(classify(&types, &function, low - 1).unwrap_err(), low - 1);
        assert_eq!(classify(&types, &function, low).unwrap(), expected);
    }
}
