use super::*;
use crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT;

#[test]
fn original_mir_stable_reference_currentness_requires_version_frame_and_exact_borrow_site() {
    let text = SOURCE_BYTES_V36;
    for required in [
        "source.logical.versions[reference.origin] == reference.version",
        "source.machine.frames.active[i] == reference.frame",
        "source.machine.values[local] == source.machine.values[reference.origin]",
        "source.logical.references[reference].origin_generation != generation",
        "source.logical.references[reference].borrow_instance != instance",
        "source.logical.references[reference].borrow_block != block",
        "source.logical.references[reference].borrow_statement != statement",
        "logical.versions.update(local, logical.versions[local] + 1)",
        "witnesses: logical.witnesses.remove(local)",
        "references: logical.references.remove(local)",
        "logical: invocation_source_logical_write_v38(source.logical, local)",
    ] {
        assert!(
            text.contains(required),
            "missing source obligation: {required}"
        );
    }
    let initial = text
        .split("open spec fn invocation_source_logical_initial_v38")
        .nth(1)
        .unwrap()
        .split("open spec fn invocation_source_logical_well_formed_v38")
        .next()
        .unwrap();
    assert!(initial.contains("witnesses: Map::empty(), references: Map::empty()"));
}

#[test]
fn original_mir_witness_issue_and_conversion_do_not_create_pointer_provenance() {
    let text = include_str!("original_semantic_mir_source_logical_locals_v38.vrs");
    assert!(text.contains("byte_execution_index_v37(execution, 0, 0)"));
    assert!(text.contains("execution.rank != 1"));
    assert!(text.contains("memory_value_modulus_v30(bits / 8)"));
    assert!(text.contains("invocation_source_witness_current_v38(source, input, input_type)"));
    assert!(!text.contains("MemoryValueV30::Pointer"));
    assert!(!text.contains("MemoryAllocationV30::Private"));
    assert!(!text.contains("assume("));
}

const LIMIT: usize = 100_000_000;

#[test]
fn original_mir_witness_transfers_preserve_full_nominal_loan_and_consume_before_write() {
    let text = SOURCE_BYTES_V36;
    let transfer = text
        .split_once("open spec fn invocation_source_transfer_witness_v40")
        .unwrap()
        .1
        .split_once("proof fn invocation_source_same_value_write_invalidates_reference_v38")
        .unwrap()
        .0;
    assert!(
        transfer.contains("invocation_source_reference_current_v38(source, input, source_type)")
    );
    assert!(transfer.contains("destination == source.logical.references[input].origin"));
    assert!(transfer.contains("let loan = source.logical.references[input]"));
    assert!(transfer.contains("references: changed.logical.references.insert(destination, loan)"));
    let snapshot = transfer.find("let loan =").unwrap();
    let consume = transfer.find("let consumed =").unwrap();
    let write = transfer.find("let changed =").unwrap();
    assert!(snapshot < consume && consume < write);
    assert!(transfer.contains("source_type, source_type, true"));
    assert!(transfer.contains("else if !moved"));
    assert!(!transfer.contains("InvocationSourceReferenceV38 {"));
    assert!(!transfer.contains("MemoryValueV30::Pointer"));
    assert!(!transfer.contains("MemoryAllocationV30"));
    assert!(!transfer.contains("input == destination"));
}

#[test]
fn original_mir_witness_transfer_event_has_explicit_pure_effect_and_law_obligations() {
    let effects = include_str!("original_semantic_mir_observed_effects_v39.vrs");
    assert!(effects.contains("Some(InvocationSourceByteEventV36::WitnessTransfer { .. })"));
    assert!(
        SOURCE_BYTES_V36.contains("reference, moved } => invocation_source_transfer_witness_v40(")
    );
    let laws = include_str!("original_semantic_mir_witness_transfer_laws_v40.vrs");
    for law in [
        "cannot_copy_authority",
        "rejects_stale_input",
        "rejects_stale_loan",
        "keeps_exact_loan",
        "move_consumes_input",
        "move_invalidates_old_borrow",
        "self_move_advances_version_twice",
    ] {
        assert!(laws.contains(law));
    }
    assert!(!laws.contains("assume("));
}

#[test]
fn original_mir_witness_transfer_classifier_leaves_real_ordinary_scalar_uses_unchanged() {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAssignmentV1, SemanticLocalIdV1, SemanticRvalueV1,
    };
    run(LIMIT, LIMIT, false, 1, |body, out| {
        let context = body.context(out)?;
        let ty = context.function.locals()[1].ty();
        let place = Place::new(SemanticLocalIdV1::from_index(1), vec![], ty).unwrap();
        let assignment = SemanticAssignmentV1::new(
            place.clone(),
            SemanticRvalueV1::new(ty, Rvalue::Use(Operand::Copy(place))),
        );
        let before = out.text.len();
        assert!(witness_transfers::Transfer::derive(&context, &assignment, out)?.is_none());
        assert_eq!(out.text.len(), before);
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_statement_refusal_keeps_original_coordinates_kind_type_and_reason() {
    run(LIMIT, LIMIT, false, 1, |body, out| {
        let context = body.context(out)?;
        let statement = context.function.blocks()[0].statements()[0].kind();
        assert_eq!(statement_kind(statement), "Assign.Binary");
        let site = [body.root, body.instance, body.function, 0, 0];
        let error = statement_error(unsupported(), site, statement, context.types);
        assert!(matches!(&error, Error::SourceStatement {
            root, instance, function, block: 0, statement: 0,
            kind: "Assign.Binary", result_type: Some((_, "Scalar")),
            reason: "original MIR typed byte statement is not modeled",
        } if *root == body.root && *instance == 1 && *function == body.function));
        assert!(std::error::Error::source(&error).is_none());
        let text = format!("{error:?}");
        // Already contextualized errors must not be reassigned to a later site.
        let same = statement_error(error, [99; 5], &Statement::Nop, &[]);
        assert_eq!(format!("{same:?}"), text);
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_statement_refusal_does_not_reclassify_resource_or_owner_errors() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationStorageLimitV1 as StorageLimit,
        CanonicalKernelIrWorkBudgetV1 as WorkBudget,
    };
    let mut work = WorkBudget::new(7);
    let work = work.charge_work(8).unwrap_err();
    for resource in [
        Resource::Work(work),
        Resource::Storage(StorageLimit::new(13, 12)),
        Resource::Accounting,
        Resource::Arithmetic,
        Resource::Allocation,
    ] {
        let mapped = statement_error(Error::Resource(resource), [3; 5], &Statement::Nop, &[]);
        assert!(matches!(mapped, Error::Resource(actual) if actual == resource));
    }
    let owner = Error::Source(
        fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Binding(
            "exact original owner differs",
        ),
    );
    assert!(matches!(
        statement_error(owner, [3; 5], &Statement::Nop, &[]),
        Error::Source(
            fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Binding(
                "exact original owner differs"
            )
        )
    ));
}

fn run(
    work: usize,
    storage: usize,
    control: bool,
    instance: usize,
    examine: impl FnOnce(&SourceByteBody<'_, '_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::invocations::tests::run_control_variant(
        work,
        storage,
        false,
        control,
        |plan, out| {
            let source = plan.source(out)?;
            let owner = source.canonical(out.budget)?;
            let (inventory, receipt) =
                super::super::super::super::Inventory::derive_v18(owner, out.budget)?;
            out.budget.reserve_storage(receipt.retained_storage())?;
            let result = source.with_ranked_correspondence_v18(
                &inventory,
                out.budget,
                |relation, budget| {
                    let mut writer = Writer::new(budget)?;
                    let slots = SourceSlots::derive(plan, relation, &mut writer)?;
                    let body = SourceByteBody::derive(plan, &slots, 0, instance, &mut writer)?;
                    examine(&body, &mut writer)
                },
            );
            drop(inventory);
            if result.is_ok() {
                out.budget.release_storage(receipt.retained_storage())?;
            }
            result
        },
    )
}

#[test]
fn original_mir_typed_call_operands_keep_exact_archived_order_and_local_identity() {
    run(LIMIT, LIMIT, false, 0, |body, out| {
        let first = body.call_argument(0, 0, out)?;
        let second = body.call_argument(0, 1, out)?;
        let scalar = ScalarV30::Integer {
            signed: false,
            width: 32,
        };
        assert_eq!(first.ty(), TypeId::from_index(0));
        assert_eq!(first.scalar(), Some(scalar));
        assert_eq!(
            first.kind,
            OperandKind::Scalar {
                value: Value::Local {
                    local: body.locals.start + 1,
                    moved: false,
                },
                scalar,
            }
        );
        assert_eq!(
            second.kind,
            OperandKind::Scalar {
                value: Value::Local {
                    local: body.locals.start + 2,
                    moved: false,
                },
                scalar,
            }
        );
        first.emit(out)?;
        assert!(out.text.contains("InvocationSourceOperandV36::Scalar"));
        assert!(out.text.contains("bits: 32int"));
        assert!(!out.text.contains("assume("));
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_typed_operands_refuse_wrong_terminators_and_missing_arguments() {
    run(LIMIT, LIMIT, false, 0, |body, out| {
        // Root block 2 is an actual Return, not a Call with an empty ABI.
        assert!(body.call_argument(2, 0, out).is_err());
        assert!(body.call_argument(0, 2, out).is_err());
        assert!(body.call_argument(99, 0, out).is_err());
        assert!(body.switch_operand(0, out).is_err());
        assert!(body.switch_operand(99, out).is_err());
        assert!(out.text.is_empty());
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_typed_switch_operand_uses_original_control_discriminant() {
    run(LIMIT, LIMIT, true, 1, |body, out| {
        let first = body.switch_operand(0, out)?;
        let second = body.switch_operand(2, out)?;
        assert!(matches!(
            first.kind,
            OperandKind::Scalar {
                value: Value::Local { local, moved: false },
                scalar: ScalarV30::Integer { signed: false, width: 32 },
            } if local == body.locals.start + 1
        ));
        assert!(matches!(
            second.kind,
            OperandKind::Scalar {
                value: Value::Local { local, moved: false },
                scalar: ScalarV30::Integer { signed: false, width: 32 },
            } if local == body.locals.start + 2
        ));
        assert!(body.switch_operand(1, out).is_err());
        assert!(body.call_argument(0, 0, out).is_err());
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_typed_operand_emission_keeps_pointer_slice_and_unit_tags_distinct() {
    // Isolated emitter inputs are not authenticated extraction/admission tests.
    run(LIMIT, LIMIT, false, 0, |_, out| {
        for kind in [
            OperandKind::Pointer {
                local: 7,
                moved: true,
            },
            OperandKind::Slice {
                local: 8,
                moved: false,
                metadata_bits: 64,
            },
            OperandKind::Scalar {
                value: Value::Constant(0),
                scalar: ScalarV30::Unit,
            },
        ] {
            TypedOperand {
                ty: TypeId::from_index(0),
                kind,
            }
            .emit(out)?;
        }
        assert!(
            out.text
                .contains("InvocationSourceOperandV36::Pointer { local: 7int, moved: true }")
        );
        assert!(out.text.contains(
            "InvocationSourceOperandV36::Slice { local: 8int, moved: false, metadata_bits: 64int }"
        ));
        assert!(out.text.contains("bits: 0int"));
        let carrier = SOURCE_BYTES_V36
            .split("open spec fn invocation_source_carrier_evaluate_v36(")
            .nth(1)
            .unwrap()
            .split("open spec fn invocation_source_operand_evaluate_v36(")
            .next()
            .unwrap();
        let capture = carrier
            .find("let value = source.machine.values[local]")
            .unwrap();
        let clear = carrier
            .find("invocation_source_byte_put_local_v36")
            .unwrap();
        assert!(capture < clear);
        assert!(carrier.contains("MemoryValueV30::Pointer(_) => metadata_bits == 0"));
        assert!(
            carrier.contains("0 <= value.length < memory_value_modulus_v30(metadata_bits / 8)")
        );
        assert!(!carrier.contains("byte_load_v30"));
        assert!(!carrier.contains("byte_store_v30"));
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_slice_operand_metadata_uses_unsigned_archived_scalar_pair() {
    use fe2o3_mir_model::semantic_mir_v1::*;
    // Isolated layout parsing tests do not admit this synthetic declaration.
    run(LIMIT, LIMIT, false, 0, |_, out| {
        let first = BackendScalar::initialized(
            BackendPrimitive::pointer(0, 8, 8),
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        );
        let declaration = |second| {
            Type::new(
                SemanticTypeIdentityV1::from_sha256([245; 32]),
                SemanticLayoutIdentityV1::from_sha256([246; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(16),
                    8,
                    BackendRepr::ScalarPair { first, second },
                    false,
                )
                .unwrap(),
                Shape::Opaque,
            )
        };
        for bits in [8u16, 16, 32, 64] {
            let ty = declaration(BackendScalar::initialized(
                BackendPrimitive::integer(false, bits, u64::from(bits / 8)),
                SemanticScalarValidityRangeV1::new(0, (1u128 << bits) - 1),
            ));
            let before = (out.budget.work(), out.budget.storage());
            assert_eq!(slice_metadata_bits_v36(&ty, out)?, u32::from(bits));
            assert_eq!(out.budget.work() - before.0, 2);
            assert_eq!(out.budget.storage(), before.1);
            if bits == 64 {
                use fe2o3_kernel_ir::{
                    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
                    CanonicalKernelIrWorkBudgetV1 as Work,
                };
                for limit in [2, 1] {
                    let mut work = Work::new(limit);
                    let storage = SOURCE_LIMIT + 17;
                    let mut budget = Budget::new(&mut work, storage);
                    budget.reserve_storage(storage).unwrap();
                    let mut writer = Writer::new(&mut budget).unwrap();
                    let result = slice_metadata_bits_v36(&ty, &mut writer);
                    if limit == 2 {
                        assert_eq!(result.unwrap(), 64);
                        assert_eq!(writer.budget.work(), 2);
                    } else {
                        assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                    }
                    assert_eq!(writer.budget.storage(), storage);
                    assert!(writer.text.is_empty());
                }
            }
        }
        for scalar in [
            BackendScalar::initialized(
                BackendPrimitive::integer(true, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            ),
            BackendScalar::union(BackendPrimitive::integer(false, 64, 8)),
            BackendScalar::initialized(
                BackendPrimitive::integer(false, 64, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            ),
        ] {
            assert!(slice_metadata_bits_v36(&declaration(scalar), out).is_err());
        }
        assert!(out.text.is_empty());
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_byte_wrapper_retains_dynamic_slot_invariants_without_target_state() {
    let predicate = SOURCE_BYTES_V36
        .split("open spec fn invocation_source_byte_state_well_formed_v36(")
        .nth(1)
        .unwrap()
        .split("enum InvocationSourceByteBaseV36")
        .next()
        .unwrap();
    assert!(predicate.contains("source.slots.dom().finite()"));
    assert!(predicate.contains("private_generation_counters_valid_v30"));
    assert!(predicate.contains("source.machine.frames.active[i].invocation == invocation"));
    assert!(
        predicate.contains(
            "source.machine.memory.live.contains_key(source.slots[descriptor].allocation)"
        )
    );
    assert!(predicate.contains("source.slots[left].allocation != source.slots[right].allocation"));
    assert!(!predicate.contains("target"));
    let activate = SOURCE_BYTES_V36
        .split("open spec fn invocation_source_byte_activate_v36(")
        .nth(1)
        .unwrap()
        .split("open spec fn invocation_source_byte_end_v36(")
        .next()
        .unwrap();
    assert!(activate.contains("source.slots.contains_key(descriptor)"));
    assert!(
        activate.contains("generations: source.machine.generations.insert(site, generation + 1)")
    );
    assert!(activate.contains("slots: source.slots.insert(descriptor"));
    let end = SOURCE_BYTES_V36
        .split("open spec fn invocation_source_byte_end_v36(")
        .nth(1)
        .unwrap()
        .split("open spec fn invocation_source_byte_step_v36(")
        .next()
        .unwrap();
    assert!(end.contains("slots: cleared.slots.remove(descriptor)"));
    assert!(end.contains("generations: cleared.machine.generations"));
    assert!(!end.contains("byte_pop_frame_v30"));
}

#[test]
fn original_mir_typed_operand_extraction_has_exact_and_one_short_resources() {
    let inspect = |body: &SourceByteBody<'_, '_, '_>, out: &mut Writer<'_, '_>| {
        body.call_argument(0, 0, out)?.emit(out)?;
        body.call_argument(0, 1, out)?.emit(out)
    };
    let measured = run(LIMIT, LIMIT, false, 0, inspect);
    measured.0.unwrap();
    run(measured.1, measured.3, false, 0, inspect).0.unwrap();
    assert!(
        run(measured.1 - 1, measured.3, false, 0, inspect)
            .0
            .is_err()
    );
    assert!(
        run(measured.1, measured.3 - 1, false, 0, inspect)
            .0
            .is_err()
    );
}

#[test]
fn original_mir_storage_dead_checks_all_initialized_pointer_fragments_before_deallocation() {
    let end = SOURCE_BYTES_V36
        .split_once("open spec fn invocation_source_byte_end_v36(")
        .unwrap()
        .1
        .split("open spec fn ")
        .next()
        .unwrap();
    let census = end
        .find("!invocation_memory_names_allocation_v37(cleared.machine.memory, pointer.allocation)")
        .unwrap();
    let remove = end
        .find("byte_end_lifetime_v30(cleared.machine.memory, pointer.allocation)")
        .unwrap();
    assert!(census < remove);
    assert!(end.contains("forall|i: int| 0 <= i < cleared.machine.values.len()"));
    assert!(end.contains(
        "!invocation_value_names_allocation_v36(cleared.machine.values[i], pointer.allocation)"
    ));
    assert!(end.contains("else { invocation_source_byte_refused_v36(cleared) }"));
    let shared = include_str!("original_semantic_mir_invocation_bytes_v36.rs");
    let census = shared
        .split_once("open spec fn invocation_memory_names_allocation_v37(")
        .unwrap()
        .1
        .split("struct InvocationByteLifetimeV36")
        .next()
        .unwrap();
    for required in [
        "object != allocation",
        "memory.live.contains_key(object)",
        "memory.live[object].relocations[at].pointer.allocation == allocation",
        "memory.live[object].initialized[at]",
        "MemoryByteV37::PointerFragment { pointer, .. } => pointer.allocation == allocation",
    ] {
        assert!(census.contains(required), "{required}");
    }
    assert!(!census.contains("generation =="));
    assert!(!census.contains("write_epochs"));
}
