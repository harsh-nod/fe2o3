use super::*;

// Reproduces the measured failure stage/coordinate, not the production identity.
// The separately authenticated AMD callback exercises real getter/body custody.
fn measured_shape() -> Fixture {
    let mut f = Fixture::new();
    let mut statements = f.blocks[7].statements().to_vec();
    statements[64] = assign(
        2,
        1,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty(1),
            SemanticConstantValueV1::ZeroSized,
        ))),
    );
    f.blocks = vec![
        block(
            0,
            statements,
            call(
                0,
                vec![SemanticOperandV1::Copy(place(7, 4))],
                place(8, 5),
                1,
            ),
        ),
        block(
            1,
            f.blocks[8].statements().to_vec(),
            call(
                0,
                vec![SemanticOperandV1::Copy(place(9, 4))],
                place(10, 5),
                2,
            ),
        ),
        block(
            2,
            f.blocks[9].statements().to_vec(),
            call(
                1,
                vec![SemanticOperandV1::Move(place(11, 2))],
                place(12, 3),
                3,
            ),
        ),
        block(
            3,
            f.blocks[10].statements().to_vec(),
            SemanticTerminatorKindV1::Return,
        ),
    ];
    f
}

#[test]
fn ordered_context_measured_bb0_stmt64_promotes_only_checked_component() {
    let f = measured_shape();
    let body = f.body();
    let original = body.clone();
    let sites = f.sites();
    assert_eq!(
        sites,
        BTreeSet::from([
            site(0, 65),
            site(0, 67),
            site(0, 69),
            site(1, 0),
            site(2, 0)
        ])
    );
    let (input, _, _) = semantic_function_ssa_input_v1(&body, Some(&f.types), &f.callables, &sites);
    let (unclassified, _, _) =
        semantic_function_ssa_input_v1(&body, Some(&f.types), &f.callables, &BTreeSet::new());
    assert!(input.promotable()[2]);
    assert!(!unclassified.promotable()[2]);
    assert_eq!(input.blocks(), unclassified.blocks());
    let plan = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
    assert_eq!(plan.resolved_events(SsaBlockIdV1::new(0)).unwrap().iter().filter(|(_, event)|
        matches!(event, SsaResolvedEventV1::Define { variable, .. } if variable.get() == 2)).count(), 1);
    let rejected = plan_ssa_with_limits_v1(&unclassified, SsaPlannerLimitsV1::default()).unwrap();
    assert!(!rejected.resolved_events(SsaBlockIdV1::new(0)).unwrap().iter().any(|(_, event)|
        matches!(event, SsaResolvedEventV1::Define { variable, .. } if variable.get() == 2)));
    assert_eq!(body, original);
}

#[test]
fn ordered_context_measured_late_shared_descendant_keeps_parameter_unpromoted() {
    let mut f = measured_shape();
    // The shared child created before exclusive resumption is used again after
    // it. Removing the owner StorageDead keeps this an ordering rejection.
    f.blocks[3] = block(
        3,
        vec![],
        call(
            0,
            vec![SemanticOperandV1::Copy(place(9, 4))],
            place(10, 5),
            4,
        ),
    );
    f.blocks
        .push(block(4, vec![], SemanticTerminatorKindV1::Return));
    let sites = f.sites();
    assert!(sites.is_empty());
    let (input, _, _) =
        semantic_function_ssa_input_v1(&f.body(), Some(&f.types), &f.callables, &sites);
    assert!(!input.promotable()[2]);
}
