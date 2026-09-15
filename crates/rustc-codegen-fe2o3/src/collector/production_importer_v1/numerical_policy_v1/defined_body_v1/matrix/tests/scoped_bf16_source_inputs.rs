//! Real source provider, full source census and original constructor/Bind join.
//! The memory proof remains pending; a lane roster cannot close its four reads.
use super::*;
use fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1 as E;
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

pub(super) fn check(
    imported: &ConstructedProductionSemanticMirV1,
    typed: Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
) {
    let typed = crate::compiler_descriptor::order_typed_descriptor_roots_by_semantic_v1(
        typed,
        &imported.semantic_mir,
    )
    .unwrap();
    crate::compiler_descriptor::validate_production_v1_semantic_ownership_evidence(
        &typed,
        &imported.semantic_mir,
    )
    .unwrap();
    assert_eq!(typed.len(), 1);
    let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        imported.semantic_mir.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(decoded, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    owner.verify_replay().unwrap();
    let root = imported.semantic_mir.roots()[0];
    let view = owner.execution_view_for_root(root).unwrap();
    let launch = typed[0].source_launch().unwrap();
    let contexts = &imported.kernel_contexts;
    let entry = contexts
        .checked_ranked_entry(&owner, root, launch, 1_048_576)
        .unwrap();
    let session = contexts
        .checked_ranked_bf16_source(
            &owner,
            root,
            launch,
            entry.as_ref().map(|(e, _)| e),
            1_048_576,
        )
        .unwrap()
        .expect("actual full Matrix+BF16 session");
    let mut inputs = contexts
        .capture_ranked_bf16_inputs(session)
        .expect("actual complete Global constructor source capture, not matrix-only custody");
    assert_eq!(
        inputs.len(),
        4,
        "two independent source loads for each of the actual A/B constructors"
    );
    let roster = contexts.global_bf16_constructors.as_ref().unwrap();
    let mut roles = BTreeSet::new();
    let mut role_order = Vec::new();
    let mut source_blocks = BTreeSet::new();
    let mut constructions = BTreeMap::new();
    let mut original_bound = None;
    let mut original_execution = None;
    for index in 0..inputs.len() {
        let row = inputs.row(index).unwrap();
        assert!(std::ptr::eq(row.owner(), &owner));
        assert!(std::ptr::eq(row.view(), view));
        assert_ne!(row.wrapper_function(), row.checked_function());
        assert_eq!(row.receiver().site().statement().is_some(), true);
        let [payload, error, result] = inputs.result_types(index).unwrap();
        assert_eq!(payload, row.contract().types().matrix);
        assert_ne!(payload, result, "the checked Result is not its payload");
        assert_ne!(payload, error);
        let role = match row.contract().operand().role {
            SemanticMfmaOperandRoleV1::A => 0,
            SemanticMfmaOperandRoleV1::B => 1,
        };
        roles.insert(role);
        role_order.push(role);
        assert!(source_blocks.insert(row.load_block()), "each original load occurs once");
        let construction = (
            row.wrapper_instance(),
            row.checked_instance(),
            row.matrix_value(),
            row.global_value(),
        );
        if let Some(previous) = constructions.insert(role, construction) {
            assert_eq!(previous, construction, "repeated loads retain the same real construction");
        }

        // Private native rows are tested against this actual captured source,
        // including exact indexed work boundaries. These are not receipts.
        let mut spent = 0;
        let provider = roster
            .join(row, &mut |n| {
                spent += n;
                Ok(())
            })
            .unwrap();
        assert!(spent > 0);
        let mut remaining = spent;
        assert!(
            roster
                .join(row, &mut |n| {
                    remaining = remaining.checked_sub(n).ok_or(E::CorrespondenceMismatch)?;
                    Ok(())
                })
                .is_ok()
        );
        assert_eq!(remaining, 0);
        let mut remaining = spent - 1;
        assert!(matches!(
            roster.join(row, &mut |n| {
                remaining = remaining.checked_sub(n).ok_or(E::CorrespondenceMismatch)?;
                Ok(())
            }),
            Err(E::CorrespondenceMismatch)
        ));

        let mut wrong_types = provider.bound_types();
        wrong_types.bound = row.contract().types().matrix;
        let contract = row.contract();
        assert!(matches!(
            inputs.test_inert_requirement(index, provider.identity(), wrong_types),
            Err(E::Unsupported {
                detail: "BF16 constructor receiver lost its original shared pair",
                ..
            })
        ));
        let lane = inputs
            .checked_constructor_lane(index)
            .expect("exact original receiver, Bind return, current lane and all live loans");
        assert_eq!(lane.contract(), contract);
        let execution = [lane.context_value(), lane.workgroup_value(), lane.subgroup_value()];
        if let Some(previous) = original_execution.replace(execution) {
            assert_eq!(previous, execution, "A/B and repeated loads retain the same checked source issuers");
        }
        for (position, issuer) in execution.iter().enumerate() {
            assert!(matches!(issuer, fe2o3_mir_model::SsaValueV1::Definition(_)));
            for previous in &execution[..position] {
                assert_ne!(issuer, previous, "Context, Workgroup and Subgroup are distinct source producers");
            }
        }
        if let Some(previous) = original_bound.replace(lane.bound_value()) {
            assert_eq!(
                previous,
                lane.bound_value(),
                "one actual source Bind constructs A and B"
            );
        }
        assert!(matches!(
            inputs.checked_constructor_lane(index),
            Err(E::Unsupported {
                detail: "BF16 lane query is duplicate or unreachable",
                ..
            })
        ));
    }
    assert_eq!(roles, BTreeSet::from([0, 1]));
    assert_eq!(role_order, [0, 1, 0, 1], "all four unused source fragments remain present");
    assert_eq!(source_blocks.len(), 4);
    assert_eq!(constructions.len(), 2);
    assert_ne!(constructions[&0].3, constructions[&1].3, "A/B retain their distinct Global issuers");
    assert!(inputs.row(inputs.len()).is_none());
    let mut events = inputs.prepare_guarded_volatile_events()
        .expect("actual source-bound guarded volatile BF16 subevent producers");
    assert_eq!(events.source_event_count(), 16);
    events.verify_source_events()
        .expect("every original load retains four ordered source-relative read producers");
    events.test_reject_source_substitutions();
    events.test_live_final_read_ports();
    assert!(
        matches!(
            events.test_finish_without_reads(),
            Err(E::Unsupported {
                detail: "BF16 source replay did not consume the exact checked lane roster",
                ..
            })
        ),
        "captured inputs and lanes must not erase the mandatory read consumer"
    );
    crate::production_ranked_projection_v1::test_guarded_bf16_root_owner_handoff(
        &owner, root, contexts, launch,
    );
}
