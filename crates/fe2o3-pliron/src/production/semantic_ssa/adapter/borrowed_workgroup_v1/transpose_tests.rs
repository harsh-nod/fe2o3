//! Component reference-observability tests, not issuance or initialization proof.
//! Full issuer/result custody is exercised by the separate actual AMD callbacks.
use super::*;

fn issue_callable() -> SemanticCallableDeclV1 {
    let mut callable = borrowed_callable(5, true);
    let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut callable else {
        unreachable!()
    };
    let SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract } = *operation else {
        unreachable!()
    };
    let transpose = SemanticGfx950TransposeContractV1::new(
        SemanticGfx950TransposeOperationV1::Issue {
            partition_reference: ty(5),
            partition: ty(4),
            tile: ty(7),
        },
        SemanticGfx950LdsTransposeFormatV1::Fp4E2M1,
        SemanticTypeIdentityV1::from_sha256([172; 32]),
        None,
    )
    .unwrap();
    *operation = SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
        contract: SemanticExecutionCapabilityContractV1::new(
            E::Gfx950Transpose(transpose),
            contract.signature(),
            contract.provenance(),
            contract.workgroup_brand().unwrap(),
            contract.epoch_before().unwrap(),
            None,
            contract.source_identity(),
        )
        .unwrap(),
    };
    callable
}

#[test]
fn transpose_issue_reuses_exact_shared_alias_traversal() {
    assert_eq!(
        direct_sites(
            &direct_function(direct_statements(), false),
            &[issue_callable()]
        ),
        BTreeSet::from([SemanticTransparentBorrowSiteV1 {
            block: 0,
            statement: 1
        }])
    );
    let mut statements = direct_statements();
    statements[2] = borrow(
        3,
        5,
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(2),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(4)).unwrap()],
            ty(4),
        )
        .unwrap(),
        SemanticBorrowKindV1::Shared,
    );
    assert_eq!(
        direct_sites(&direct_function(statements, false), &[issue_callable()]),
        BTreeSet::from([
            SemanticTransparentBorrowSiteV1 {
                block: 0,
                statement: 1
            },
            SemanticTransparentBorrowSiteV1 {
                block: 0,
                statement: 2
            }
        ])
    );
}

#[test]
fn transpose_issue_rejects_mutable_raw_escape_and_duplicate_reference_definitions() {
    for mutation in 0..4 {
        let mut statements = direct_statements();
        match mutation {
            0 => statements[1] = borrow(2, 5, place(1, 4), SemanticBorrowKindV1::Mutable),
            1 => {
                statements[1] = assign(
                    2,
                    5,
                    SemanticRvalueKindV1::AddressOf {
                        mutability: SemanticMutabilityV1::Immutable,
                        place: place(1, 4),
                    },
                )
            }
            2 => statements.push(statement(SemanticStatementKindV1::Assume(
                SemanticOperandV1::Copy(place(2, 5)),
            ))),
            3 => statements.push(alias(3, 2, 5)),
            _ => unreachable!(),
        }
        assert!(
            direct_sites(&direct_function(statements, false), &[issue_callable()]).is_empty(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn transpose_issue_rejects_wrong_callable_binding_arity_and_exhausted_work() {
    let mut callable = issue_callable();
    let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut callable else {
        unreachable!()
    };
    let SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract } = *operation else {
        unreachable!()
    };
    *operation = SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
        contract: SemanticExecutionCapabilityContractV1::new(
            contract.operation(),
            contract.signature(),
            contract.provenance(),
            contract.workgroup_brand().unwrap(),
            contract.epoch_before().unwrap(),
            None,
            SemanticFunctionIdentityV1::from_sha256([173; 32]),
        )
        .unwrap(),
    };
    assert!(direct_sites(&direct_function(direct_statements(), false), &[callable]).is_empty());
    assert!(
        direct_sites(
            &direct_function(direct_statements(), true),
            &[issue_callable()]
        )
        .is_empty()
    );
    assert!(matches!(
        sites(
            &direct_function(direct_statements(), false),
            &[issue_callable()],
            &[],
            0,
            None
        )
        .map_err(flow_work_profile_v1::original_error_for_test),
        Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits,
            ..
        })
    ));
}

#[test]
fn transpose_stage_and_read_borrow_only_their_real_second_argument() {
    let operations = [
        SemanticGfx950TransposeOperationV1::Stage {
            input_tile: ty(0),
            output_tile: ty(1),
            view_reference: ty(5),
            view: ty(4),
            index: ty(6),
            global_reference: ty(7),
            global: ty(8),
        },
        SemanticGfx950TransposeOperationV1::Read {
            tile: ty(0),
            lane_reference: ty(5),
            lane: ty(4),
            fragment: ty(1),
            registers: ty(7),
            word: ty(8),
        },
    ];
    for operation in operations {
        let operation = E::Gfx950Transpose(
            SemanticGfx950TransposeContractV1::new(
                operation,
                SemanticGfx950LdsTransposeFormatV1::Fp4E2M1,
                SemanticTypeIdentityV1::from_sha256([174; 32]),
                None,
            )
            .unwrap(),
        );
        assert_eq!(reference_pair(operation, 0), None);
        assert_eq!(reference_pair(operation, 1), Some((ty(5), ty(4))));
        assert_eq!(reference_pair(operation, 2), None);
        assert_eq!(reference_pair(operation, 3), None);
    }
}
