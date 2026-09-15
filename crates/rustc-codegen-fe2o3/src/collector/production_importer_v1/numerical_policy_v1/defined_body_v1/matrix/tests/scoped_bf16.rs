//! Actual Bind-only source lane checks. These intentionally do not claim a
//! Global constructor, memory event, numerical, KIR, or final contract proof.
use super::*;

#[path = "scoped_bf16_use_probe.rs"]
mod use_probe;

#[path = "scoped_query_map.rs"]
mod query_map;

pub(super) fn check(
    imported: &crate::collector::production_importer_v1::ConstructedProductionSemanticMirV1,
    typed: Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
    repeated: Option<bool>,
) {
    use fe2o3_pliron::{
        ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
        ProductionSemanticSsaOwnerV1,
    };
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
    let make_owner = || {
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            imported.semantic_mir.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        ProductionSemanticSsaOwnerV1::try_new(
            ProductionSemanticMirOwnerV1::try_new(
                decoded,
                ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap()
    };
    let owner = make_owner();
    owner.verify_replay().unwrap();
    let root = imported.semantic_mir.roots()[0];
    let view = owner.execution_view_for_root(root).unwrap();
    let body = view.body();
    query_map::inspect_recorded(
        &owner,
        view,
        &[
            ("27afc514c15b1f830c60922ab37359f2538a4109aa77898d972b7c4a75ab99ff", 16, 45),
            ("711a885fb5a87d692a1de1698f40aa2580e29e65719b3a7c02d0e0e23a05eb56", 16, 45),
        ],
        &mut std::io::stderr().lock(),
    ).unwrap();
    let launch = typed[0].source_launch().unwrap();
    let contexts = &imported.kernel_contexts;
    let entry = contexts
        .checked_ranked_entry(&owner, root, launch, 1_048_576)
        .unwrap();
    let checked_entry = entry.as_ref().map(|(e, _)| e);
    // The matrix-only API must not pretend this independent BF16 batch was
    // consumed. Its output type carries no BF16 relation or memory authority.
    use fe2o3_lower_mir_kernel::{ProductionKernelContextLoweringInputV1, ProductionScopedMatrixUseRelationV1};
    assert!(!ProductionScopedMatrixUseRelationV1::has_potential_source_consumers(
        body, owner.source_semantic().callables(),
    ));
    let source = contexts.roots.iter().find(|r| r.selected_root == root).unwrap();
    let mut matrix_input = ProductionKernelContextLoweringInputV1::new(
        root, contexts.frontend_unit_identity, source.kernel_marker_identity,
        contexts.target_brand_identity, source.launch_brand_identity, source.issuance_identity,
    );
    if let Some(transfer) = source.entry_transfer {
        matrix_input = matrix_input.with_entry_transfer(transfer);
    }
    let matrix_only = ProductionScopedMatrixUseRelationV1::checked_source_uses(
        &owner, &matrix_input, checked_entry, 1_048_576,
    ).unwrap();
    matrix_only.check_consumed(body, &[]).unwrap();
    assert!(
        contexts
            .checked_ranked_matrix(&owner, root, launch, checked_entry, 1_048_576)
            .unwrap()
            .is_none(),
        "a real Bind-only source cannot become the FP4/8 narrowed route"
    );
    let bindings = owner
        .execution_expansion()
        .defined_capability_bindings(owner.source_semantic())
        .unwrap();
    assert!(
        !bindings.iter().any(|b| matches!(
            b.contract(),
            SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(_)
        )),
        "source deliberately has no narrowing"
    );
    let bound = bindings
        .iter()
        .filter(|b| {
            b.root() == root
                && matches!(
                    b.contract(),
                    SemanticDefinedCapabilityContractV1::PolicyMatrixBind(_)
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(bound.len(), if repeated.is_some() { 2 } else { 1 });
    if repeated.is_some() {
        assert_eq!(
            bound[0].contract(),
            bound[1].contract(),
            "the same real helper, contract and canonical types"
        );
        assert_ne!(bound[0].callee_instance(), bound[1].callee_instance());
        assert_ne!(
            bound[0].destination(),
            bound[1].destination(),
            "expansion keeps both return locals distinct"
        );
    }
    let new_session = || {
        contexts
            .checked_ranked_bf16_source(&owner, root, launch, checked_entry, 1_048_576)
            .unwrap()
            .expect("source-authenticated BF16 session")
    };
    assert!(
        new_session().finish(&[]).is_err(),
        "omitted actual BF16 lanes cannot publish a source batch"
    );
    let calls =
        body.blocks()
            .iter()
            .enumerate()
            .filter_map(|(block, data)| {
                let SemanticTerminatorKindV1::Call(call) = data.terminator().kind() else {
                    return None;
                };
                matches!(owner.source_semantic().callables().get(call.callee().index() as usize),
            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::GlobalBf16MatrixLoad { .. }, ..
            })).then_some((block as u32, call))
            })
            .collect::<Vec<_>>();
    assert_eq!(
        calls.len(),
        if repeated.is_some() { 8 } else { 4 },
        "both A/B source roles and repeated legal lane uses survive"
    );
    if repeated == Some(true) {
        // Source normalization must pass before any cross-Bind negative.
        // The negative checks below start with a separate unconsumed session.
        let mut positive_session = new_session();
        let mut consumed = Vec::new();
        let mut lanes = BTreeSet::new();
        let mut binds = BTreeSet::new();
        for (block, call) in &calls {
            let caller = view.block_origins()[*block as usize].instance();
            let matching = bound
                .iter()
                .filter(|b| b.caller_instance() == caller)
                .collect::<Vec<_>>();
            let [current] = matching.as_slice() else {
                panic!("load and Bind in the same actual caller instance")
            };
            let bind_call = current.expanded_call_block().index();
            let row = positive_session
                .checked_bf16_lane_at(body, *block, call, bind_call)
                .unwrap_or_else(|error| {
                    panic!(
                        "shared-Subgroup positive lane at block {block} with Bind {bind_call}: {error:?}"
                    )
                });
            assert_eq!(row.bind_call_block(), bind_call);
            assert_eq!(row.bind_instance(), current.callee_instance());
            lanes.insert(row.lane_identity());
            binds.insert(row.bind_identity());
            consumed.push(*block);
        }
        assert_eq!(
            (consumed.len(), lanes.len(), binds.len()),
            (8, 1, 2),
            "all eight shared-Subgroup loads retain one lane and two Bind identities"
        );
        let (narrow, lanes) = positive_session
            .finish(&consumed)
            .expect("all eight shared-Subgroup positive lanes must finish");
        narrow.check_consumed(body, &[]).unwrap();
        lanes.check_consumed(body, &consumed).unwrap();
    }
    let foreign = make_owner();
    let foreign_body = foreign.execution_view_for_root(root).unwrap().body();
    let mut session = new_session();
    let mut consumed = Vec::new();
    let mut lanes = BTreeSet::new();
    let mut binds = BTreeSet::new();
    for (block, call) in &calls {
        let caller = view.block_origins()[*block as usize].instance();
        let matching = bound
            .iter()
            .filter(|b| b.caller_instance() == caller)
            .collect::<Vec<_>>();
        let [current] = matching.as_slice() else {
            panic!("load and Bind in the same actual caller instance")
        };
        let bind_call = current.expanded_call_block().index();
        if let Some(shared_subgroup) = repeated {
            let other = bound
                .iter()
                .find(|b| b.callee_instance() != current.callee_instance())
                .unwrap();
            let error = session
                .checked_bf16_lane_at(body, *block, call, other.expanded_call_block().index())
                .unwrap_err();
            assert!(
                matches!(error, fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::Unsupported { detail, .. }
                if if shared_subgroup { detail.contains("loan") } else {
                    detail == "BF16 Bind and lane name different live MatrixAccess owners"
                }),
                "a different real instance must fail its exact owner/loan check: {error:?}"
            );
        }
        assert!(
            session
                .checked_bf16_lane_at(&body.clone(), *block, call, bind_call)
                .is_err()
        );
        assert!(
            session
                .checked_bf16_lane_at(foreign_body, *block, call, bind_call)
                .is_err()
        );
        assert!(
            session
                .checked_bf16_lane_at(body, *block, call, u32::MAX)
                .is_err(),
            "a coordinate is never a made-up occurrence"
        );
        let mut arguments = call.arguments().to_vec();
        arguments.swap(0, 1);
        let changed = SemanticDirectCallV1::new_callable(
            call.callee(),
            arguments,
            call.destination().cloned(),
            call.unwind(),
        )
        .unwrap();
        assert!(
            session
                .checked_bf16_lane_at(body, *block, &changed, bind_call)
                .is_err()
        );
        use_probe::inspect(&owner, view, current, *block, call);
        let row = session
            .checked_bf16_lane_at(body, *block, call, bind_call)
            .unwrap();
        assert_eq!(row.bind_call_block(), bind_call);
        assert_eq!(row.bind_instance(), current.callee_instance());
        lanes.insert(row.lane_identity());
        binds.insert(row.bind_identity());
        assert!(
            session
                .checked_bf16_lane_at(body, *block, call, bind_call)
                .is_err(),
            "one lane query per source read, not one allowance per component"
        );
        consumed.push(*block);
    }
    assert_eq!(
        (lanes.len(), binds.len()),
        match repeated {
            None => (1, 1),
            Some(false) => (2, 2),
            Some(true) => (1, 2),
        }
    );
    // This fixture consumes only the lane relation. It must not install a
    // ranked Global operand or claim the missing constructor/read relation.
    let (narrow, lanes) = session.finish(&consumed).unwrap();
    narrow.check_consumed(body, &[]).unwrap();
    lanes.check_consumed(body, &consumed).unwrap();
    assert!(lanes.check_consumed(body, &consumed[1..]).is_err());
    let mut changed = consumed.clone();
    changed[1] = changed[0];
    assert!(lanes.check_consumed(body, &changed).is_err());
    changed.reverse();
    assert!(lanes.check_consumed(body, &changed).is_err());
    assert!(lanes.check_consumed(foreign_body, &consumed).is_err());
    for (block, call) in calls {
        assert!(lanes.use_at(body, block, call).unwrap().is_some());
        assert!(lanes.use_at(foreign_body, block, call).is_err());
    }
    assert!(
        contexts
            .checked_ranked_bf16_source(&owner, root, launch, checked_entry, 1)
            .is_err()
    );
    if repeated == Some(false) {
        super::scoped_owner_negatives::check_workgroup_carrier_lifetimes(imported);
    }
}

#[test]
#[ignore = "requires existing authenticated AMD metadata; invokes rustc only, never Cargo"]
fn scoped_bf16_bind_only_lane_gfx942() {
    run(
        "gfx942",
        "scoped_bf16::scoped_bf16_bind_only_lane_gfx942",
        Fixture::ScopedBf16,
    );
}

#[test]
#[ignore = "requires existing authenticated AMD metadata; invokes rustc only, never Cargo"]
fn scoped_bf16_bind_only_lane_gfx950() {
    run(
        "gfx950",
        "scoped_bf16::scoped_bf16_bind_only_lane_gfx950",
        Fixture::ScopedBf16,
    );
}

pub(super) fn repeated_source(shared_subgroup: bool) -> String {
    let source = include_str!("scoped_bf16_source.rs");
    let start = "        let subgroup = workgroup.subgroup::<SubgroupWidth64>();\n";
    assert_eq!(source.matches(start).count(), 1);
    let prefix = if shared_subgroup {
        format!("{start}        let issue = || {{\n")
    } else {
        format!("        let issue = || {{\n{start}")
    };
    let source = source.replacen(start, &prefix, 1);
    let end = "        })\n    })\n}";
    assert_eq!(source.matches(end).count(), 1);
    source.replacen(
        end,
        "        })\n        };\n        issue()?;\n        issue()\n    })\n}",
        1,
    )
}

#[test]
#[ignore = "requires existing authenticated AMD metadata; invokes rustc only, never Cargo"]
fn scoped_workgroup_carrier_lifetime_actual_amdgpu_gfx942() {
    run("gfx942", "scoped_bf16::scoped_workgroup_carrier_lifetime_actual_amdgpu_gfx942", Fixture::ScopedBf16Repeated(false));
}

#[test]
#[ignore = "requires existing authenticated AMD metadata; invokes rustc only, never Cargo"]
fn scoped_workgroup_carrier_lifetime_actual_amdgpu_gfx950() {
    run("gfx950", "scoped_bf16::scoped_workgroup_carrier_lifetime_actual_amdgpu_gfx950", Fixture::ScopedBf16Repeated(false));
}
