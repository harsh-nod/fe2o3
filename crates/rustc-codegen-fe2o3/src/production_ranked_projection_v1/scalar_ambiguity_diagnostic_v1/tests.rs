use super::*;
use fe2o3_mir_model::SsaBlockIdV1;
use fe2o3_pliron::{ProductionSemanticSsaLimitsV1, plan_semantic_function_ssa_with_module_v1};

const SCALAR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);

fn direct() -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        SCALAR,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}

fn place(local: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], SCALAR).unwrap()
}

fn assign(local: u32, operand: SemanticOperandV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local),
            SemanticRvalueV1::new(SCALAR, SemanticRvalueKindV1::Use(operand)),
        )),
    )
}

pub(super) fn fixture(definitions: usize) -> (Vec<SemanticTypeDeclV1>, SemanticFunctionDeclV1) {
    let types = vec![SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarValidityRangeV1::new(0, u32::MAX as u128),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    )];
    let locals = (0..3)
        .map(|local| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([local as u8 + 3; 32]),
                SCALAR,
                match local {
                    0 => SemanticLocalRoleV1::Return,
                    1 => SemanticLocalRoleV1::Argument(0),
                    _ => SemanticLocalRoleV1::Temporary,
                },
                SemanticSourceProvenanceV1::unavailable(),
            )
        })
        .collect();
    let mut statements = (0..definitions)
        .map(|_| assign(2, SemanticOperandV1::Copy(place(1))))
        .collect::<Vec<_>>();
    statements.push(assign(0, SemanticOperandV1::Copy(place(2))));
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([10; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([11; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([12; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([13; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([14; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256([15; 32]),
            SemanticLayoutIdentityV1::from_sha256([16; 32]),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![direct()],
            direct(),
        )
        .unwrap(),
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([17; 32]),
                SemanticSourceProvenanceV1::unavailable(),
                statements,
                SemanticTerminatorV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticTerminatorKindV1::Return,
                ),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    (types, function)
}

pub(super) fn final_operand(function: &SemanticFunctionDeclV1) -> &SemanticOperandV1 {
    let SemanticStatementKindV1::Assign(assignment) =
        function.blocks()[0].statements().last().unwrap().kind()
    else {
        unreachable!()
    };
    let SemanticRvalueKindV1::Use(operand) = assignment.value().kind() else {
        unreachable!()
    };
    operand
}

fn ambiguity(function: &SemanticFunctionDeclV1) -> AmbiguousUse<'_> {
    let SemanticOperandV1::Copy(place) = final_operand(function) else {
        unreachable!()
    };
    AmbiguousUse { local: 2, place }
}

#[test]
fn scalar_diagnostic_records_real_assignments_and_ssa_events_without_statement_guess() {
    let (types, function) = fixture(2);
    let plan = plan_semantic_function_ssa_with_module_v1(
        SemanticFunctionIdV1::from_index(0),
        &function,
        &types,
        &[],
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let context = Context {
        function: &function,
        plan: plan.plan(),
        view: None,
    };
    let report = context.describe(12, 0, Some((0, Some(2))), ambiguity(&function));
    for expected in [
        "ranked=bb12:op0",
        "historical-def=bb0:s0",
        "historical-def=bb0:s1",
        "exact-use=bb0:s2",
        "historical-defs-seen=2",
        "exact-use-occurrences-seen=1",
        "ssa-events-seen=3",
        "scan=complete",
        "ssa-statement-mapping=unavailable",
    ] {
        assert!(report.contains(expected), "missing {expected}: {report}");
    }
    let events = plan.plan().resolved_events(SsaBlockIdV1::new(0)).unwrap();
    let definitions = events
        .iter()
        .filter_map(|(_, event)| match event {
            SsaResolvedEventV1::Define { variable, value } if variable.get() == 2 => Some(*value),
            _ => None,
        })
        .collect::<Vec<_>>();
    let used = events
        .iter()
        .find_map(|(_, event)| match event {
            SsaResolvedEventV1::Use { variable, value } if variable.get() == 2 => Some(*value),
            _ => None,
        })
        .unwrap();
    assert_eq!(definitions.len(), 2);
    assert_ne!(definitions[0], used);
    assert_eq!(definitions[1], used);
}

#[test]
fn scalar_diagnostic_occurrences_use_pointer_identity_not_equal_places() {
    let (_, function) = fixture(2);
    let clone = function.clone();
    assert_eq!(final_operand(&function), final_operand(&clone));
    assert!(exact_operand(
        final_operand(&function),
        ambiguity(&function).place
    ));
    assert!(!exact_operand(
        final_operand(&clone),
        ambiguity(&function).place
    ));
}

#[test]
fn scalar_diagnostic_rejects_wrong_function_owner_even_with_equal_body() {
    let (_, function) = fixture(2);
    assert!(same_function(&function, &function, function.identity()));
    assert!(!same_function(
        &function,
        &function.clone(),
        function.identity()
    ));
    assert!(!same_function(
        &function,
        &function,
        SemanticFunctionIdentityV1::from_sha256([99; 32])
    ));
}

#[test]
fn scalar_diagnostic_shared_scan_exhaustion_is_explicit_lower_bound() {
    let (types, function) = fixture(2);
    let plan = plan_semantic_function_ssa_with_module_v1(
        SemanticFunctionIdV1::from_index(0),
        &function,
        &types,
        &[],
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let context = Context {
        function: &function,
        plan: plan.plan(),
        view: None,
    };
    for limit in [0, 1, 5] {
        let report = context.describe_with_limit(12, 0, None, ambiguity(&function), limit);
        assert!(report.contains("scan=lower-bound"), "{report}");
        assert!(report.contains(&format!("visited={limit}")), "{report}");
        assert!(report.len() <= MAX_BYTES);
    }
}

#[test]
fn scalar_diagnostic_samples_are_capped_without_changing_complete_scan_label() {
    let (types, function) = fixture(40);
    let plan = plan_semantic_function_ssa_with_module_v1(
        SemanticFunctionIdV1::from_index(0),
        &function,
        &types,
        &[],
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let context = Context {
        function: &function,
        plan: plan.plan(),
        view: None,
    };
    let report = context.describe(12, 0, None, ambiguity(&function));
    assert_eq!(report.matches("historical-def=").count(), MAX_ROWS);
    assert_eq!(report.matches("ssa-event=").count(), MAX_ROWS);
    assert!(report.contains("historical-defs-seen=40"));
    assert!(report.contains("scan=complete"));
    assert!(report.contains("samples-capped=true"));
    assert!(report.len() <= MAX_BYTES);
}

#[test]
fn scalar_diagnostic_output_buffer_never_exceeds_limit() {
    let mut output = Text::default();
    for _ in 0..MAX_BYTES {
        let _ = output.write_str("abcd");
    }
    let output = output.finish();
    assert!(output.len() <= MAX_BYTES);
    assert!(output.ends_with("diagnostic-text-truncated=true"));
}
