//! Pure roster tests use inert records and opaque rustc keys, not issuer authority.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use rustc_hir::def_id::{DefId, DefIndex};
use rustc_middle::ty::GenericArgs;

fn instance(index: u32) -> Instance<'static> {
    Instance::new_raw(
        DefId::local(DefIndex::from_u32(index + 1)),
        GenericArgs::empty(),
    )
}

fn function(index: u8, value: u32) -> SemanticFunctionDeclV1 {
    let ty = SemanticTypeIdV1::from_index(0);
    let source = SemanticSourceProvenanceV1::unavailable();
    let mode = SemanticAbiPassModeV1::Direct(
        SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
            SemanticAbiExtensionV1::None,
            0,
            None,
        )
        .unwrap(),
    );
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([20; 32]),
        SemanticLayoutIdentityV1::from_sha256([21; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![],
        SemanticAbiValueV1::new(ty, mode),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([index + 1; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([index + 30; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([index + 40; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([index + 50; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([index + 60; 32]),
        source,
        abi,
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([index + 70; 32]),
            ty,
            SemanticLocalRoleV1::Return,
            source,
        )],
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([index + 80; 32]),
                source,
                vec![SemanticStatementV1::new(
                    source,
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], ty).unwrap(),
                        SemanticRvalueV1::new(
                            ty,
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                                SemanticConstantV1::new(
                                    ty,
                                    SemanticConstantValueV1::Scalar(
                                        SemanticScalarValueV1::new(u128::from(value), 4).unwrap(),
                                    ),
                                ),
                            )),
                        ),
                    )),
                )],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn expected(
    index: u32,
    function: &SemanticFunctionDeclV1,
) -> ExpectedFunctionCommitmentV29<'static> {
    ExpectedFunctionCommitmentV29::new(
        instance(index),
        SemanticFunctionIdV1::from_index(index),
        ProductionSemanticFunctionIdentitiesV1::new(
            function.identity(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
        ),
        function.role(),
        function.source(),
    )
}

fn owner(count: u32) -> ProductionSemanticBodyRequestOwnerV1<'static> {
    let callables = (0..count)
        .map(|index| {
            ProductionSemanticCallableOwnerEntryV1::defined(
                instance(index),
                SemanticCallableIdV1::from_index(index),
            )
        })
        .collect::<Vec<_>>();
    ProductionSemanticBodyRequestOwnerV1::new(SemanticMirLimitsV1::default(), 1, &callables)
        .unwrap()
}

fn admitted(values: &[u32], v29: bool) -> AdmittedInertSemanticMirV1 {
    let layout = SemanticTypeLayoutV1::new_with_backend_repr(
        Some(4),
        4,
        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 32, 4),
            SemanticScalarValidityRangeV1::new(0, u128::from(u32::MAX)),
        )),
        false,
    )
    .unwrap();
    let ty = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([90; 32]),
        SemanticLayoutIdentityV1::from_sha256([91; 32]),
        layout,
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    );
    let functions = values
        .iter()
        .enumerate()
        .map(|(index, value)| function(index as u8, *value))
        .collect();
    let roots = (0..values.len())
        .map(|index| SemanticFunctionIdV1::from_index(index as u32))
        .collect();
    let request = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([92; 32])),
        vec![ty],
        vec![],
        vec![],
        vec![],
        functions,
        roots,
    )
    .unwrap();
    if v29 {
        request
            .admit_exact_v29(SemanticMirLimitsV1::default())
            .unwrap()
    } else {
        request
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap()
    }
}

fn enabled(count: u32) -> ProductionSemanticBodyRequestOwnerV1<'static> {
    let mut owner = owner(count);
    owner
        .enable_function_commitments_v29(
            count as usize,
            (0..count).map(|index| expected(index, &function(index as u8, 7))),
        )
        .unwrap();
    owner
}

fn capture(owner: &mut ProductionSemanticBodyRequestOwnerV1<'_>, index: u32) {
    let commitment = owner
        .capture_function_commitment_v29(
            SemanticFunctionIdV1::from_index(index),
            &function(index as u8, 7),
        )
        .unwrap()
        .unwrap();
    owner
        .function_commitments
        .as_mut()
        .unwrap()
        .prepare(commitment)
        .unwrap()
        .publish();
}

fn verify(
    mut owner: ProductionSemanticBodyRequestOwnerV1<'_>,
    semantic: &AdmittedInertSemanticMirV1,
) -> Result<(), ProductionSemanticBodyErrorV1> {
    owner
        .function_commitments
        .take()
        .unwrap()
        .verify(semantic, &mut owner.totals, owner.limits)
}

#[test]
fn roster_comes_from_complete_preflight_not_successfully_captured_bodies() {
    for count in [0, 1, 3] {
        let mut owner = owner(2);
        let before = owner.totals.validation_work;
        assert!(
            owner
                .enable_function_commitments_v29(
                    2,
                    (0..count).map(|index| expected(index, &function(index as u8, 7))),
                )
                .is_err()
        );
        assert!(owner.function_commitments.is_none());
        assert!(owner.totals.validation_work > before);
    }
    let mut owner = enabled(2);
    assert!(
        owner
            .enable_function_commitments_v29(2, std::iter::empty())
            .is_err()
    );
    assert_eq!(owner.function_commitments.as_ref().unwrap().rows.len(), 2);
    assert_eq!(owner.function_commitments.as_ref().unwrap().completed, 0);
    capture(&mut owner, 0);
    assert!(verify(owner, &admitted(&[7, 7], true)).is_err());
}

#[test]
fn preflight_rejects_order_and_instance_rebinding() {
    let mut shortened = owner(2);
    assert!(
        shortened
            .enable_function_commitments_v29(1, std::iter::once(expected(0, &function(0, 7))),)
            .is_err()
    );
    assert!(shortened.function_commitments.is_none());
    for change in 0..3 {
        let mut owner = owner(2);
        let mut rows = vec![expected(0, &function(0, 7)), expected(1, &function(1, 7))];
        match change {
            0 => rows.swap(0, 1),
            1 => rows[1].instance = instance(0),
            2 => rows[1].instance = instance(99),
            _ => unreachable!(),
        }
        assert!(
            owner
                .enable_function_commitments_v29(2, rows.into_iter())
                .is_err()
        );
        assert!(owner.function_commitments.is_none());
    }
}

#[test]
fn source_order_and_full_output_identity_are_checked_before_capture() {
    let mut owner = enabled(2);
    assert!(
        owner
            .capture_function_commitment_v29(SemanticFunctionIdV1::from_index(1), &function(1, 7),)
            .is_err()
    );
    assert!(
        owner
            .capture_function_commitment_v29(SemanticFunctionIdV1::from_index(0), &function(1, 7),)
            .is_err()
    );
    assert_eq!(owner.function_commitments.as_ref().unwrap().completed, 0);
    capture(&mut owner, 0);
    assert!(
        owner
            .capture_function_commitment_v29(SemanticFunctionIdV1::from_index(0), &function(0, 7),)
            .is_err()
    );
    capture(&mut owner, 1);
    assert!(verify(owner, &admitted(&[7, 7], true)).is_ok());
}

#[test]
fn dropped_capture_or_preparation_does_not_publish_or_refund_work() {
    let mut owner = enabled(1);
    let before = owner.totals.validation_work;
    let commitment = owner
        .capture_function_commitment_v29(SemanticFunctionIdV1::from_index(0), &function(0, 7))
        .unwrap()
        .unwrap();
    let after_capture = owner.totals.validation_work;
    assert!(after_capture > before);
    {
        let _prepared = owner
            .function_commitments
            .as_mut()
            .unwrap()
            .prepare(commitment)
            .unwrap();
    }
    let pending = owner.function_commitments.as_ref().unwrap();
    assert_eq!(pending.completed, 0);
    assert!(pending.rows[0].captured.is_none());
    assert_eq!(owner.totals.validation_work, after_capture);
    capture(&mut owner, 0);
    assert!(owner.totals.validation_work > after_capture);
    assert!(verify(owner, &admitted(&[7], true)).is_ok());
}

#[test]
fn sealing_detects_constant_substitution_missing_body_and_wrong_version() {
    for (values, v29) in [(vec![11], true), (vec![7, 7], true), (vec![7], false)] {
        let mut owner = enabled(1);
        capture(&mut owner, 0);
        assert!(verify(owner, &admitted(&values, v29)).is_err());
    }
}

#[test]
fn complete_roster_is_not_context_or_launch_authority() {
    let mut owner = enabled(1);
    capture(&mut owner, 0);
    assert!(matches!(
        owner.seal_context_entries(&admitted(&[7], true)),
        Err(ProductionSemanticBodyErrorV1::IdentityTableMismatch {
            table: "function commitment context transaction",
        }),
    ));
}

#[test]
fn context_free_capture_does_not_hash_or_charge() {
    let mut owner = owner(1);
    let before = owner.totals;
    assert!(
        owner
            .capture_function_commitment_v29(SemanticFunctionIdV1::from_index(0), &function(0, 7),)
            .unwrap()
            .is_none()
    );
    assert_eq!(owner.totals, before);
    assert!(owner.seal_context_entries(&admitted(&[7], true)).is_ok());
}

#[test]
fn capture_exact_and_one_short_work_share_the_original_counter() {
    let mut baseline = enabled(1);
    capture(&mut baseline, 0);
    let exact = baseline.totals.validation_work;
    for maximum in [exact, exact - 1] {
        let mut owner = enabled(1);
        owner.limits = owner
            .limits
            .with_limit(SemanticMirResourceV1::ValidationWork, maximum)
            .unwrap();
        let result = owner
            .capture_function_commitment_v29(SemanticFunctionIdV1::from_index(0), &function(0, 7));
        assert_eq!(result.is_ok(), maximum == exact);
        assert_eq!(owner.totals.validation_work, exact);
        assert_eq!(owner.function_commitments.as_ref().unwrap().completed, 0);
        if maximum < exact {
            assert!(
                matches!(result, Err(ProductionSemanticBodyErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::ValidationWork, actual, maximum: max,
            }) if actual == exact && max == maximum)
            );
        }
    }
}

#[test]
fn seal_exact_and_one_short_work_preserve_capture_debits() {
    let semantic = admitted(&[7], true);
    let mut baseline = enabled(1);
    capture(&mut baseline, 0);
    baseline
        .function_commitments
        .take()
        .unwrap()
        .verify(&semantic, &mut baseline.totals, baseline.limits)
        .unwrap();
    let exact = baseline.totals.validation_work;
    for maximum in [exact, exact - 1] {
        let mut owner = enabled(1);
        capture(&mut owner, 0);
        let limits = owner
            .limits
            .with_limit(SemanticMirResourceV1::ValidationWork, maximum)
            .unwrap();
        let result =
            owner
                .function_commitments
                .take()
                .unwrap()
                .verify(&semantic, &mut owner.totals, limits);
        assert_eq!(result.is_ok(), maximum == exact);
        assert_eq!(owner.totals.validation_work, exact);
    }
}

#[test]
fn initialization_exact_and_one_short_work_are_charged_before_publication() {
    for maximum in [3, 2] {
        let mut owner = owner(1);
        owner.limits = owner
            .limits
            .with_limit(SemanticMirResourceV1::ValidationWork, maximum)
            .unwrap();
        let result =
            owner.enable_function_commitments_v29(1, std::iter::once(expected(0, &function(0, 7))));
        assert_eq!(result.is_ok(), maximum == 3);
        assert_eq!(owner.totals.validation_work, 3);
        assert_eq!(owner.function_commitments.is_some(), maximum == 3);
    }
}

#[test]
fn terminal_rows_cannot_supply_defined_function_roster() {
    let entry = ProductionSemanticCallableOwnerEntryV1::terminal(
        instance(0),
        ProductionTerminalExpansionV1::ContextIssue,
        SemanticCallableIdV1::from_index(0),
    );
    let mut owner =
        ProductionSemanticBodyRequestOwnerV1::new(SemanticMirLimitsV1::default(), 1, &[entry])
            .unwrap();
    assert_eq!(owner.defined_functions, 0);
    assert!(
        owner
            .enable_function_commitments_v29(1, std::iter::once(expected(0, &function(0, 7))),)
            .is_err()
    );
}

#[test]
fn capture_byte_limit_preserves_error_details_and_unpublished_state() {
    let f = function(0, 7);
    let expected = canonical_function_commitment_v1(
        &f,
        SemanticMirWireVersionV1::V29,
        SemanticMirLimitsV1::default(),
        &mut |_| Ok(()),
    )
    .unwrap()
    .canonical_bytes();
    let mut owner = enabled(1);
    owner.limits = owner
        .limits
        .with_limit(SemanticMirResourceV1::CanonicalBytes, expected - 1)
        .unwrap();
    let before = owner.totals.validation_work;
    assert!(matches!(owner.capture_function_commitment_v29(
        SemanticFunctionIdV1::from_index(0), &f,
    ), Err(ProductionSemanticBodyErrorV1::LimitExceeded {
        resource: SemanticMirResourceV1::CanonicalBytes, actual, maximum,
    }) if actual == expected && maximum == expected - 1));
    assert!(owner.totals.validation_work > before);
    let pending = owner.function_commitments.as_ref().unwrap();
    assert_eq!(pending.completed, 0);
    assert!(pending.rows[0].captured.is_none());
}
