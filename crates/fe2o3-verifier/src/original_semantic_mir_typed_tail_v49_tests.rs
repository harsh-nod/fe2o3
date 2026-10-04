use super::super::invocations::InvocationPlan;
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV18 as Inventory, check_canonical_kir_transition_v18,
};
use fe2o3_kernel_ir::{EndiannessV2, ExplicitLaunchExtent, FormalIndexWidth};
use fe2o3_mir_model::semantic_mir_v1::*;

const LIMIT: usize = 512 * 1024 * 1024;

include!("original_semantic_mir_original_model_reuse_v59_tests.rs");

fn fixture_error(error: impl std::error::Error + 'static) -> Error {
    let mut chain: &(dyn std::error::Error + 'static) = &error;
    loop {
        if let Some(resource) = chain.downcast_ref::<Resource>() {
            return Error::Resource(*resource);
        }
        chain = chain
            .source()
            .unwrap_or_else(|| panic!("fixture preparation failed: {error}"));
    }
}

fn run(
    work: usize,
    storage: usize,
    hostile: bool,
    examine: impl FnOnce(&str),
) -> (Result<()>, usize, usize, usize) {
    run_mode(work, storage, hostile, false, examine)
}

fn run_mode(
    work: usize,
    storage: usize,
    hostile: bool,
    consensus: bool,
    examine: impl FnOnce(&str),
) -> (Result<()>, usize, usize, usize) {
    run_transform(work, storage, hostile, consensus, None, examine)
}

fn run_transform(
    work: usize,
    storage: usize,
    hostile: bool,
    consensus: bool,
    transform: Option<fn(&mut Vec<SemanticTypeDeclV1>, &mut Vec<SemanticFunctionDeclV1>)>,
    examine: impl FnOnce(&str),
) -> (Result<()>, usize, usize, usize) {
    let consume = |plan: &mut InvocationPlan<'_, '_>, out: &mut Writer<'_, '_>| {
        source_function::tests::with_slots(plan, out, |slots, out| {
            let relation = slots.correspondence(out)?;
            let original = relation.inventory(out.budget)?;
            let layouts = fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default()
                .storage_layout_limits();
            let optimized = fe2o3_pliron::optimize_neutral_kernel_ir_mixed_fixedpoint_v18(
                original.owner(),
                layouts,
                out.budget,
            )
            .map_err(fixture_error)?;
            out.budget
                .reserve_storage(optimized.storage().retained_storage())?;
            assert_eq!(optimized.execution().policy_version(), 11);
            let (middle, receipt) = Inventory::derive_v18(optimized.owner(), out.budget)?;
            out.budget.reserve_storage(receipt.retained_storage())?;
            let (prefix, receipt) = check_canonical_kir_transition_v18(
                original,
                &middle,
                optimized.occurrences().candidate(),
                out.budget,
            )?;
            out.budget.reserve_storage(receipt.retained_storage())?;
            let prepared =
                fe2o3_kernel_opt::prepare_owned_licm_v18(optimized.owner(), layouts, out.budget)
                    .map_err(fixture_error)?;
            out.budget.reserve_storage(prepared.retained_storage())?;
            let (licm, receipt) = prepared
                .replay_against(optimized.owner(), out.budget)
                .map_err(fixture_error)?;
            out.budget.reserve_storage(receipt.retained_storage())?;
            let (output, receipt) = Inventory::derive_v18(licm.output(), out.budget)?;
            out.budget.reserve_storage(receipt.retained_storage())?;
            let prepared_forwarding = fe2o3_kernel_opt::prepare_owned_cross_block_forwarding_v18(
                licm.output(),
                Default::default(),
                layouts,
                out.budget,
            )
            .map_err(fixture_error)?;
            out.budget
                .reserve_storage(prepared_forwarding.retained_storage())?;
            let (forwarding, receipt) = prepared_forwarding
                .replay_against(licm.output(), out.budget)
                .map_err(fixture_error)?;
            out.budget.reserve_storage(receipt.retained_storage())?;
            if consensus {
                let replaced = forwarding
                    .origins()
                    .iter()
                    .filter(|row| row.store.is_some())
                    .count();
                assert!(
                    replaced >= 4,
                    "each genuine root/helper instance must forward its joined load: replaced {replaced}; input {:#?}",
                    forwarding.input().module()
                );
                assert_ne!(
                    forwarding.input().identity(),
                    forwarding.output().identity()
                );
            }
            let (final_inventory, receipt) =
                Inventory::derive_v18(forwarding.output(), out.budget)?;
            out.budget.reserve_storage(receipt.retained_storage())?;
            if hostile {
                assert!(matches!(
                    typed_tail::Tail::derive(relation, &prefix, &licm, &middle, out),
                    Err(Error::Statement(_))
                ));
                assert!(out.text.is_empty());
            }
            let launches = [ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [64, 1, 1],
            }; 2];
            let census = generate_refinement_typed_v49(
                relation,
                &prefix,
                &licm,
                &output,
                &forwarding,
                &final_inventory,
                &launches,
                FormalIndexWidth::Bits64,
                EndiannessV2::Little,
                out,
            )?;
            assert_eq!(census[0], 2);
            examine(&out.text);
            Ok(())
        })
    };
    if let Some(transform) = transform {
        super::super::invocations::tests::run_source_transform(work, storage, transform, consume)
    } else if consensus {
        super::super::invocations::tests::run_source_transform(
            work,
            storage,
            consensus_transform,
            consume,
        )
    } else {
        super::super::invocations::tests::run_variant(work, storage, true, consume)
    }
}

#[path = "original_semantic_mir_float_values_v52_tests.rs"]
mod float_values;

fn consensus_transform(
    types: &mut Vec<SemanticTypeDeclV1>,
    functions: &mut Vec<SemanticFunctionDeclV1>,
) {
    let helper = functions.last_mut().unwrap();
    assert_eq!(helper.locals().len(), 4);
    let source = helper.source();
    let word = helper.locals()[1].ty();
    let pointer = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([243; 32]),
        SemanticLayoutIdentityV1::from_sha256([244; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                word,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    let mut locals = helper.locals().to_vec();
    for (ordinal, ty) in [word, pointer].into_iter().enumerate() {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([245 + ordinal as u8; 32]),
            ty,
            SemanticLocalRoleV1::Temporary,
            source,
        ));
    }
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let assign = |destination: SemanticPlaceV1, rhs| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                destination.clone(),
                SemanticRvalueV1::new(destination.ty(), rhs),
            )),
        )
    };
    let live = |local| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(local)),
        )
    };
    let dead = |local| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(local)),
        )
    };
    let edge =
        |role, block| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block));
    let goto = || SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3));
    let block = |identity, statements, kind| {
        SemanticBasicBlockV1::new(
            identity,
            source,
            statements,
            SemanticTerminatorV1::new(source, kind),
        )
        .unwrap()
    };
    let store = || {
        assign(
            place(4, word),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, word))),
        )
    };
    let loaded = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(5),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, word).unwrap()],
        word,
    )
    .unwrap();
    let blocks = vec![
        block(
            helper.blocks()[0].identity(),
            vec![
                live(4),
                live(5),
                assign(
                    place(5, pointer),
                    SemanticRvalueKindV1::AddressOf {
                        place: place(4, word),
                        mutability: SemanticMutabilityV1::Mutable,
                    },
                ),
            ],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(place(2, word)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, 1),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            },
        ),
        block(
            SemanticBlockIdentityV1::from_sha256([247; 32]),
            vec![store()],
            goto(),
        ),
        block(
            SemanticBlockIdentityV1::from_sha256([248; 32]),
            vec![store()],
            goto(),
        ),
        block(
            SemanticBlockIdentityV1::from_sha256([249; 32]),
            vec![
                assign(
                    place(0, word),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(loaded)),
                ),
                dead(5),
                dead(4),
            ],
            SemanticTerminatorKindV1::Return,
        ),
    ];
    *helper = SemanticFunctionDeclV1::new(
        helper.identity(),
        helper.role(),
        helper.item_definition_identity(),
        helper.monomorphization_identity(),
        helper.generic_type_arguments_identity(),
        helper.const_generic_arguments_identity(),
        source,
        helper.abi().clone(),
        locals,
        helper.entry(),
        blocks,
    )
    .unwrap();
}

#[test]
fn original_source_typed_tail_forwards_genuine_same_value_branch_stores() {
    let mut complete = false;
    let result = run_mode(LIMIT, LIMIT, false, true, |text| {
        assert!(text.contains("tagged_select_value"));
        assert!(text.contains("forwarding_store_fact_v46"));
        assert!(text.contains("proof fn typed_final_native_source_trace_"));
        assert!(text.contains("typed_source_observation_transport_"));
        assert_forwarding_reuses_exact_parent_byte_functions_v55(text, true);
        assert!(!text.contains("assume("));
        complete = true;
    });
    assert!(
        complete,
        "genuine source forwarding route refused: {:?}",
        result.0
    );
    result.0.unwrap();
}

#[test]
fn original_source_typed_tail_preserves_exact_cut_entry_and_shared_middle_state() {
    let mut completed = false;
    let result = run(LIMIT, LIMIT, false, |text| {
        assert_eq!(text.matches("mod typed_prefix_v49 {").count(), 1);
        assert_eq!(text.matches("struct TypedSourceBoundaryV49 {").count(), 1);
        assert_eq!(text.matches("mod forwarding_v46 {").count(), 1);
        assert_forwarding_reuses_exact_parent_byte_functions_v55(text, false);
        for root in 0..2 {
            assert!(text.contains(&format!("proof fn typed_source_original_block_{root}_v49")));
            assert!(text.contains(&format!(
                "super::byte_block_step_{root}_v30(s, invocation_runtime_little_endian_v36())"
            )));
            assert!(text.contains(&format!("invocation_byte_cut_{root}_v36(cursor.segment)")));
            assert!(text.contains(&format!(
                "typed_source_follow_{root}_v49(original, middle, actual,"
            )));
            assert!(text.contains(&format!("invocation_paired_step_{root}_v36(source, original); typed_source_boundary_{root}_v49(original, middle, actual);")));
            assert!(text.contains(&format!(
                "proof fn invocation_source_observation_extensionality_{root}_v49"
            )));
            assert!(text.contains(&format!("proof fn typed_final_source_step_{root}_v49")));
            assert!(text.contains(&format!(
                "typed_source_forwarded_boundary_{root}_v49(actual).observations"
            )));
            let initial = text
                .split(&format!("proof fn typed_source_initial_{root}_v49("))
                .nth(1)
                .unwrap();
            assert!(initial.contains(&format!(
                "invocation_paired_native_inputs_{root}_v38(arguments, external, execution)"
            )));
            assert!(initial.contains(&format!(
                "invocation_paired_raw_initial_{root}_v36(arguments, external, execution)"
            )));
        }
        assert!(text.contains("observations: head.observations + tail.observations"));
        assert!(text.contains("typed_allocation_environment_0_v48(before) == typed_allocation_environment_1_v48(after)"));
        assert!(text.contains("a.after == (MemoryStateV30 { pc: -2, ..a.before })"));
        assert!(!text.contains("assume("));
        assert!(!text.contains("external_body"));
        completed = true;
    });
    assert!(
        completed,
        "source-owned composed generation did not complete: {:?}",
        result.0
    );
    result.0.unwrap();
}

fn assert_forwarding_reuses_exact_parent_byte_functions_v55(text: &str, has_operations: bool) {
    let (_, forwarding) = text.split_once("mod forwarding_v46 {").unwrap();
    let mut aliases = 0usize;
    for line in forwarding.lines() {
        let Some((source, target)) = line.trim().split_once(" as byte_operation_") else {
            continue;
        };
        if !source.starts_with("byte_operation_") {
            continue;
        }
        let target = format!("byte_operation_{}", target.strip_suffix(',').unwrap());
        assert_eq!(text.matches(&format!("open spec fn {source}(")).count(), 1);
        assert!(!forwarding.contains(&format!("open spec fn {target}(")));
        assert!(forwarding.contains(&format!(
            "ensures super::{source}(s, little_endian) == {target}(s, little_endian),"
        )));
        aliases += 1;
    }
    assert_eq!(aliases > 0, has_operations);
    assert_eq!(
        forwarding
            .matches("proof fn forwarding_shared_operation_")
            .count(),
        aliases
    );
    assert_eq!(
        forwarding.matches("proof fn forwarding_operation_").count(),
        aliases
    );
    assert_eq!(
        forwarding.matches("open spec fn byte_operation_").count(),
        aliases
    );
    let mut controls = 0usize;
    let mut blocks = 0usize;
    for line in forwarding.lines() {
        let Some((source, target)) = line.trim().split_once(" as ") else {
            continue;
        };
        if source.starts_with("byte_control_") {
            controls += 1;
        } else if source.starts_with("byte_block_") && !source.starts_with("byte_block_step_") {
            blocks += 1;
        } else {
            continue;
        }
        let target = target.strip_suffix(',').unwrap();
        assert_eq!(text.matches(&format!("open spec fn {source}(")).count(), 1);
        assert!(!forwarding.contains(&format!("open spec fn {target}(")));
    }
    assert!(controls > 0);
    assert_eq!(controls, blocks);
    assert_eq!(
        forwarding.matches("open spec fn byte_control_").count(),
        controls
    );
}

#[test]
fn original_source_typed_tail_rejects_the_prefix_owner_as_the_licm_output() {
    let mut completed = false;
    let result = run(LIMIT, LIMIT, true, |_| completed = true);
    assert!(
        completed,
        "genuine retry after owner refusal failed: {:?}",
        result.0
    );
    result.0.unwrap();
}

#[test]
fn original_source_typed_tail_full_generation_is_deterministic_and_exactly_budgeted() {
    use sha2::Digest as _;
    let observe = |work, storage, fingerprint: &mut (usize, [u8; 32])| {
        run(work, storage, false, |text| {
            *fingerprint = (text.len(), sha2::Sha256::digest(text.as_bytes()).into());
        })
    };
    let mut first = (0, [0; 32]);
    let measured = observe(LIMIT, LIMIT, &mut first);
    measured.0.unwrap();
    let mut second = (0, [0; 32]);
    let repeated = observe(measured.1, measured.3, &mut second);
    repeated.0.unwrap();
    assert_eq!(first, second);
    assert_eq!(
        (repeated.1, repeated.2, repeated.3),
        (measured.1, measured.2, measured.3)
    );
    for work_short in [false, true] {
        let work = measured.1 - usize::from(work_short);
        let storage = measured.3 - usize::from(!work_short);
        let mut rejected = (0, [0; 32]);
        let denied = observe(work, storage, &mut rejected);
        let error = denied.0.expect_err("one-short generation must refuse");
        let mut chain: &(dyn std::error::Error + 'static) = &error;
        let resource = loop {
            if let Some(resource) = chain.downcast_ref::<Resource>() {
                break *resource;
            }
            chain = chain
                .source()
                .unwrap_or_else(|| panic!("missing resource: {error:?}"));
        };
        match resource {
            Resource::Work(limit) if work_short => {
                assert_eq!(limit.limit(), work);
                assert_eq!(limit.actual(), measured.1);
            }
            Resource::Storage(limit) if !work_short => {
                assert_eq!(limit.limit(), storage);
                assert_eq!(limit.actual(), measured.3);
            }
            other => panic!("wrong resource: {other:?}"),
        }
    }
}
