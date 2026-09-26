// Source-only component controls. No production admission or B2b owner.
mod fixed_guard_preparation_controls {
    use super::*;
    use crate::production_ranked_projection_v1::assertion_analyzer_resources_v1::{
        fixed_guard_raw_probe_for_test_v1, fixed_guard_session_for_test_v1,
    };
    use crate::production_ranked_projection_v1::bf16_nominal_source_preparation_v1::with_rich_tables_for_test_v1;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use fe2o3_mir_model::semantic_mir_v1::SemanticConstantBytesV1;

    fn original_authenticate_fixed_array_guard_v1(
        proof: &mut SemanticAssertProofsV1<'_>,
        guard: usize,
        success: usize,
        condition: &SemanticOperandV1,
        index: &SemanticOperandV1,
        bound: &SemanticOperandV1,
    ) -> Result<(SemanticLocalIdV1, u64), ProductionRankedProjectionErrorV1> {
        let refuse = || {
            ProductionRankedProjectionErrorV1::Incomplete(
                "a fixed-array bounds check lacks stable exact unsigned index < literal extent evidence",
            )
        };
        proof.charge(1)?;
        let index_local = simple_operand_local(index).ok_or_else(refuse)?;
        let index_slot = index_local.index() as usize;
        let condition_local = simple_operand_local(condition).ok_or_else(refuse)?;
        let condition_slot = condition_local.index() as usize;
        let bits = unsigned_index_bits_v1(proof.types, index.ty()).ok_or_else(refuse)?;
        let declaration = proof.function.locals().get(index_slot).ok_or_else(refuse)?;
        if declaration.ty() != index.ty()
            || bound.ty() != index.ty()
            || proof.address_escaped.get(index_slot) != Some(&false)
            || proof.address_escaped.get(condition_slot) != Some(&false)
            || proof.definition_counts.get(condition_slot) != Some(&1)
            || !matches!(
                proof
                    .types
                    .get(condition.ty().index() as usize)
                    .map(SemanticTypeDeclV1::shape),
                Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
            )
        {
            return Err(refuse());
        }
        let SemanticOperandV1::Constant(constant) = bound else {
            return Err(refuse());
        };
        let SemanticConstantValueV1::Scalar(value) = constant.value() else {
            return Err(refuse());
        };
        if u16::from(value.size_bytes()) * 8 != bits
            || value.bits() == 0
            || (bits < 64 && value.bits() >= (1_u128 << bits))
        {
            return Err(refuse());
        }
        let extent = u64::try_from(value.bits()).map_err(|_| refuse())?;
        let comparison = proof
            .assignments
            .get(condition_slot)
            .copied()
            .flatten()
            .ok_or_else(refuse)?;
        if comparison.block != guard || !proof.block_dominates(guard, success)? {
            return Err(refuse());
        }
        let SemanticStatementKindV1::Assign(assignment) =
            proof.function.blocks()[guard].statements()[comparison.statement].kind()
        else {
            return Err(refuse());
        };
        if !matches!(assignment.value().kind(), SemanticRvalueKindV1::Binary {
        operation: SemanticBinaryOpV1::LessThan, left, right,
    } if left == index && right == bound)
        {
            return Err(refuse());
        }
        if proof.definition_counts.get(index_slot) == Some(&0)
            && declaration.role().is_entry_argument()
        {
            return Ok((index_local, extent));
        }
        if proof.definition_counts.get(index_slot) != Some(&1) {
            return Err(refuse());
        }
        let definition = proof
            .assignments
            .get(index_slot)
            .copied()
            .flatten()
            .ok_or_else(refuse)?;
        if !proof.assignment_dominates_use(definition, guard, comparison.statement)?
            || !proof.assignment_dominates_use(definition, success, 0)?
        {
            return Err(refuse());
        }
        Ok((index_local, extent))
    }

    type Error = ProductionRankedProjectionErrorV1;
    const LIMIT: usize = 256 * 1024 * 1024;
    const FLOOR: usize = 37;
    fn good() -> SemanticFunctionDeclV1 {
        fixed_guard_function(FixedGuardOptions {
            extent: 4,
            ..Default::default()
        })
    }
    fn scalar(value: u128, bytes: u8) -> SemanticOperandV1 {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            U64_TYPE,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, bytes).unwrap()),
        ))
    }
    fn rebuilt(
        f: &SemanticFunctionDeclV1,
        locals: Vec<SemanticLocalDeclV1>,
        blocks: Vec<SemanticBasicBlockV1>,
    ) -> SemanticFunctionDeclV1 {
        SemanticFunctionDeclV1::new(
            f.identity(),
            f.role(),
            f.item_definition_identity(),
            f.monomorphization_identity(),
            f.generic_type_arguments_identity(),
            f.const_generic_arguments_identity(),
            f.source(),
            f.abi().clone(),
            locals,
            f.entry(),
            blocks,
        )
        .unwrap()
    }
    fn comparison(
        f: &SemanticFunctionDeclV1,
        left: SemanticOperandV1,
        right: SemanticOperandV1,
    ) -> SemanticFunctionDeclV1 {
        let mut blocks = f.blocks().to_vec();
        let mut statements = blocks[0].statements().to_vec();
        statements[1] = typed_assignment(
            3,
            BOOL_TYPE,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::LessThan,
                left,
                right,
            },
        );
        blocks[0] = block(190, statements, blocks[0].terminator().kind().clone());
        rebuilt(f, f.locals().to_vec(), blocks)
    }
    fn message(
        f: &SemanticFunctionDeclV1,
        index: SemanticOperandV1,
        length: SemanticOperandV1,
        target: u32,
    ) -> SemanticFunctionDeclV1 {
        let mut blocks = f.blocks().to_vec();
        blocks[0] = block(
            190,
            blocks[0].statements().to_vec(),
            SemanticTerminatorKindV1::Assert {
                condition: typed_operand(3, BOOL_TYPE),
                expected: true,
                message: SemanticAssertMessageV1::BoundsCheck { length, index },
                target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, target),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        );
        rebuilt(f, f.locals().to_vec(), blocks)
    }
    fn original_selected(
        proof: &mut SemanticAssertProofsV1<'_>,
        guard: usize,
    ) -> Result<(SemanticLocalIdV1, u64), Error> {
        let f = proof.function;
        let block = f.blocks().get(guard).ok_or(Error::Unsupported(
            "prepared fixed guard coordinate is outside its original function",
        ))?;
        let SemanticTerminatorKindV1::Assert {
            condition,
            expected,
            message: SemanticAssertMessageV1::BoundsCheck { length, index },
            target,
            unwind,
        } = block.terminator().kind()
        else {
            return Err(Error::Unsupported(
                "prepared fixed guard query requires a source BoundsCheck Assert",
            ));
        };
        if !*expected || !matches!(unwind, SemanticUnwindActionV1::Unreachable) {
            return Err(Error::Incomplete(
                "a Rust bounds check without the canonical success/unreachable shape",
            ));
        }
        original_authenticate_fixed_array_guard_v1(
            proof,
            guard,
            target.target().index() as usize,
            condition,
            index,
            length,
        )
    }
    fn legacy_pair(types: &[SemanticTypeDeclV1], f: &SemanticFunctionDeclV1) -> (String, usize) {
        let mut original = SemanticAssertProofsV1::new(types, f).unwrap();
        let mut current = SemanticAssertProofsV1::new(types, f).unwrap();
        let a = original_selected(&mut original, 0);
        let SemanticTerminatorKindV1::Assert {
            condition,
            expected,
            message: SemanticAssertMessageV1::BoundsCheck { length, index },
            target,
            unwind,
        } = f.blocks()[0].terminator().kind()
        else {
            panic!("fixture source guard")
        };
        let b = if !*expected || !matches!(unwind, SemanticUnwindActionV1::Unreachable) {
            Err(Error::Incomplete(
                "a Rust bounds check without the canonical success/unreachable shape",
            ))
        } else {
            authenticate_fixed_array_guard_v1(
                &mut current,
                0,
                target.target().index() as usize,
                condition,
                index,
                length,
            )
        };
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
        assert_eq!(original.work, current.work);
        (format!("{a:?}"), original.work)
    }
    #[derive(Debug, PartialEq)]
    struct Observation {
        data: Option<(SemanticLocalIdV1, u64)>,
        logical: Option<usize>,
        error: Option<String>,
        work: usize,
        peak: usize,
        owned: usize,
        failed_work: bool,
        failed_storage: bool,
    }
    fn raw_with(
        types: &[SemanticTypeDeclV1],
        f: &SemanticFunctionDeclV1,
        graph: &ProjectedLoopCfgV1,
        inventory: &AssertionDefinitionInventoryV1,
        guard: usize,
        w: usize,
        p: usize,
    ) -> Observation {
        let mut work = Work::new(w);
        let mut budget = Budget::new(&mut work, p);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let identity = budget.work_ledger_identity_v1();
        let slot = &budget as *const Budget<'_> as usize;
        let result = {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            fixed_guard_raw_probe_for_test_v1(types, f, graph, inventory, guard, &mut resources)
        };
        let observed = Observation {
            data: result.as_ref().ok().map(|(data, _)| *data),
            logical: result.as_ref().ok().map(|(_, logical)| *logical),
            error: result.as_ref().err().map(|e| format!("{e:?}")),
            work: budget.work(),
            peak: budget.peak_storage(),
            owned,
            failed_work: budget.failed_work().is_some(),
            failed_storage: budget.failed_storage().is_some(),
        };
        drop(result);
        assert_eq!(&budget as *const Budget<'_> as usize, slot);
        assert!(budget.work_ledger_identity_v1() == identity);
        assert_eq!(budget.storage(), FLOOR + owned);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        observed
    }
    fn raw(
        types: &[SemanticTypeDeclV1],
        f: &SemanticFunctionDeclV1,
        guard: usize,
        w: usize,
        p: usize,
    ) -> Observation {
        raw_with(
            types,
            f,
            &projected_loop_cfg_graph_v1(f).unwrap(),
            &assertion_definition_inventory(f).unwrap(),
            guard,
            w,
            p,
        )
    }
    #[test]
    fn fixed_guard_legacy_keeps_frozen_helper_decisions_and_logical_work() {
        let types = assertion_proof_types();
        let base = FixedGuardOptions {
            extent: 4,
            ..Default::default()
        };
        for options in [
            base,
            FixedGuardOptions {
                wrong_comparison: true,
                ..base
            },
            FixedGuardOptions {
                wrong_message: true,
                ..base
            },
            FixedGuardOptions {
                late_index: true,
                ..base
            },
            FixedGuardOptions {
                bypass: true,
                ..base
            },
            FixedGuardOptions {
                expected_false: true,
                ..base
            },
            FixedGuardOptions { extent: 0, ..base },
            FixedGuardOptions {
                mutated_array: true,
                ..base
            },
        ] {
            let f = fixed_guard_function(options);
            legacy_pair(&types, &f);
        }
    }
    #[test]
    fn fixed_guard_strict_selected_source_matches_independent_original() {
        let types = assertion_proof_types();
        let base = FixedGuardOptions {
            extent: 4,
            ..Default::default()
        };
        for options in [
            base,
            FixedGuardOptions {
                wrong_comparison: true,
                ..base
            },
            FixedGuardOptions {
                wrong_message: true,
                ..base
            },
            FixedGuardOptions {
                late_index: true,
                ..base
            },
            FixedGuardOptions {
                bypass: true,
                ..base
            },
            FixedGuardOptions {
                expected_false: true,
                ..base
            },
            FixedGuardOptions { extent: 0, ..base },
        ] {
            let f = fixed_guard_function(options);
            let mut original = SemanticAssertProofsV1::new(&types, &f).unwrap();
            let old = original_selected(&mut original, 0);
            let actual = raw(&types, &f, 0, LIMIT, LIMIT);
            match old {
                Ok(data) => {
                    assert_eq!(actual.data, Some(data));
                    assert_eq!(actual.logical, Some(original.work));
                }
                Err(error) => assert_eq!(actual.error, Some(format!("{error:?}"))),
            }
            assert!(!actual.failed_work && !actual.failed_storage);
        }
    }
    #[test]
    fn fixed_guard_source_coordinate_and_assert_shape_fail_closed() {
        let types = assertion_proof_types();
        let f = good();
        for (guard, message) in [
            (1, "requires a source BoundsCheck Assert"),
            (99, "coordinate is outside"),
        ] {
            let actual = raw(&types, &f, guard, LIMIT, LIMIT);
            assert!(actual.error.unwrap().contains(message));
            assert!(actual.data.is_none());
        }
        let wrong = message(&f, typed_operand(4, U64_TYPE), scalar(4, 8), 1);
        assert!(
            raw(&types, &wrong, 0, LIMIT, LIMIT)
                .error
                .unwrap()
                .contains("lacks stable exact")
        );
        let swapped = comparison(&f, scalar(4, 8), typed_operand(2, U64_TYPE));
        assert!(
            raw(&types, &swapped, 0, LIMIT, LIMIT)
                .error
                .unwrap()
                .contains("lacks stable exact")
        );
    }
    #[test]
    fn fixed_guard_single_assignment_before_comparison_is_accepted_but_late_or_duplicate_is_not() {
        let types = assertion_proof_types();
        let f = good();
        let mut blocks = f.blocks().to_vec();
        let mut statements = blocks[0].statements().to_vec();
        statements.insert(
            1,
            typed_assignment(
                2,
                U64_TYPE,
                SemanticRvalueKindV1::Use(typed_operand(4, U64_TYPE)),
            ),
        );
        blocks[0] = block(
            190,
            statements.clone(),
            blocks[0].terminator().kind().clone(),
        );
        let mut locals = f.locals().to_vec();
        locals[2] = local(195, U64_TYPE, SemanticLocalRoleV1::Temporary);
        let before = rebuilt(&f, locals.clone(), blocks.clone());
        let original = legacy_pair(&types, &before);
        let actual = raw(&types, &before, 0, LIMIT, LIMIT);
        assert_eq!(actual.data, Some((SemanticLocalIdV1::from_index(2), 4)));
        assert_eq!(actual.logical, Some(original.1));
        statements.insert(1, statements[1].clone());
        blocks[0] = block(190, statements, blocks[0].terminator().kind().clone());
        let duplicate = rebuilt(&f, locals, blocks);
        legacy_pair(&types, &duplicate);
        assert!(raw(&types, &duplicate, 0, LIMIT, LIMIT).data.is_none());
        let late = fixed_guard_function(FixedGuardOptions {
            extent: 4,
            late_index: true,
            ..Default::default()
        });
        assert!(raw(&types, &late, 0, LIMIT, LIMIT).data.is_none());
    }
    #[test]
    fn fixed_guard_escaped_and_duplicate_condition_or_index_rows_refuse() {
        let types = assertion_proof_types();
        let f = good();
        let graph = projected_loop_cfg_graph_v1(&f).unwrap();
        for case in 0..4 {
            let mut inventory = assertion_definition_inventory(&f).unwrap();
            match case {
                0 => inventory.address_escaped[2] = true,
                1 => inventory.address_escaped[3] = true,
                2 => inventory.counts[3] = 2,
                _ => inventory.counts[2] = 2,
            }
            let actual = raw_with(&types, &f, &graph, &inventory, 0, LIMIT, LIMIT);
            assert!(actual.error.unwrap().contains("lacks stable exact"));
            assert!(actual.data.is_none());
        }
    }
    #[test]
    fn fixed_guard_unsigned_width_nonzero_literal_and_message_identity_are_preserved() {
        let f = good();
        let ordinary = assertion_proof_types();
        for (bits, signed) in [(32, false), (64, true), (128, false)] {
            let mut types = ordinary.clone();
            types[U64_TYPE.index() as usize] = SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(230)),
                SemanticLayoutIdentityV1::from_sha256(bytes(230)),
                SemanticTypeLayoutV1::new(Some(u64::from(bits / 8)), u64::from(bits / 8)).unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { bits, signed }),
            );
            legacy_pair(&types, &f);
            assert!(raw(&types, &f, 0, LIMIT, LIMIT).data.is_none());
        }
        for bound in [
            scalar(0, 8),
            scalar(4, 4),
            scalar(1_u128 << 64, 16),
            scalar(u64::MAX as u128, 8),
        ] {
            let changed = message(&f, typed_operand(2, U64_TYPE), bound, 1);
            legacy_pair(&ordinary, &changed);
            assert!(raw(&ordinary, &changed, 0, LIMIT, LIMIT).data.is_none());
        }
        // Existing scalar constructor disallows overflow in an 8-byte literal.
        assert!(SemanticScalarValueV1::new(1_u128 << 64, 8).is_err());
    }
    #[test]
    fn fixed_guard_success_uniqueness_and_entry_remain_post_authentication_checks() {
        let types = assertion_proof_types();
        let bypass = fixed_guard_function(FixedGuardOptions {
            extent: 4,
            bypass: true,
            ..Default::default()
        });
        assert!(raw(&types, &bypass, 0, LIMIT, LIMIT).data.is_some());
        assert!(
            format!("{:?}", collect_fixed_guards(&bypass).err().unwrap())
                .contains("not uniquely controlled")
        );
        let entry = message(&good(), typed_operand(2, U64_TYPE), scalar(4, 8), 0);
        assert!(raw(&types, &entry, 0, LIMIT, LIMIT).data.is_some());
        assert!(
            format!("{:?}", collect_fixed_guards(&entry).err().unwrap())
                .contains("not uniquely controlled")
        );
    }
    #[test]
    fn fixed_guard_exact_work_storage_and_one_short_are_measured_not_guessed() {
        let types = assertion_proof_types();
        let f = good();
        let exact = raw(&types, &f, 0, LIMIT, LIMIT);
        assert!(exact.data.is_some() && exact.owned > 0);
        assert_eq!(raw(&types, &f, 0, exact.work, exact.peak), exact);
        let work = raw(&types, &f, 0, exact.work - 1, exact.peak);
        assert!(work.data.is_none() && work.failed_work && !work.failed_storage);
        let storage = raw(&types, &f, 0, exact.work, exact.peak - 1);
        assert!(storage.data.is_none() && !storage.failed_work && storage.failed_storage);
    }
    #[test]
    fn fixed_guard_partial_frame_and_table_refusals_keep_all_accepted_credits() {
        let types = assertion_proof_types();
        let f = good();
        let exact = raw(&types, &f, 0, LIMIT, LIMIT);
        for w in [0, 8, 16, 64, exact.work / 2, exact.work - 1] {
            let result = raw(&types, &f, 0, w, LIMIT);
            assert!(result.failed_work && result.data.is_none());
        }
        for p in [FLOOR, FLOOR + 1, exact.peak / 2, exact.peak - 1] {
            let result = raw(&types, &f, 0, LIMIT, p);
            assert!(result.failed_storage && result.data.is_none());
        }
        assert_eq!(raw(&types, &f, 0, exact.work, exact.peak), exact);
    }
    fn deep_place(count: usize) -> SemanticOperandV1 {
        SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(2),
                (0..count)
                    .map(|_| {
                        SemanticProjectionV1::new(SemanticProjectionKindV1::OpaqueCast, U64_TYPE)
                            .unwrap()
                    })
                    .collect(),
                U64_TYPE,
            )
            .unwrap(),
        )
    }
    #[test]
    fn fixed_guard_projection_scan_is_prepaid_before_simple_operand_rejection() {
        let types = assertion_proof_types();
        let f = good();
        let short = message(&f, deep_place(1), scalar(4, 8), 1);
        let long = message(&f, deep_place(2048), scalar(4, 8), 1);
        let a = raw(&types, &short, 0, LIMIT, LIMIT);
        let b = raw(&types, &long, 0, LIMIT, LIMIT);
        assert!(a.data.is_none() && b.data.is_none());
        assert_eq!(b.work - a.work, 2047);
        let denied = raw(&types, &long, 0, b.work - 1, LIMIT);
        assert!(denied.failed_work && denied.data.is_none());
        assert_eq!(denied.work, b.work - 2048);
        legacy_pair(&types, &short);
        legacy_pair(&types, &long);
    }
    #[test]
    fn fixed_guard_left_comparison_payload_is_paid_before_exact_equality() {
        let types = assertion_proof_types();
        let f = good();
        let short = comparison(&f, deep_place(1), scalar(4, 8));
        let long = comparison(&f, deep_place(2048), scalar(4, 8));
        let a = raw(&types, &short, 0, LIMIT, LIMIT);
        let b = raw(&types, &long, 0, LIMIT, LIMIT);
        assert!(a.error.as_ref().unwrap().contains("lacks stable exact"));
        assert!(b.error.as_ref().unwrap().contains("lacks stable exact"));
        assert_eq!(
            b.work - a.work,
            2047 * std::mem::size_of::<SemanticProjectionV1>()
        );
        let denied = raw(&types, &long, 0, b.work - 1, LIMIT);
        assert!(denied.failed_work && denied.data.is_none());
        let last = 2048 * std::mem::size_of::<SemanticProjectionV1>()
            + 2 * std::mem::size_of::<SemanticOperandV1>()
            + 16;
        assert_eq!(denied.work, b.work - last);
    }
    #[test]
    fn fixed_guard_right_comparison_payload_and_left_short_circuit_keep_order() {
        let types = assertion_proof_types();
        let f = good();
        let bytes = |n| {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                U64_TYPE,
                SemanticConstantValueV1::Bytes(SemanticConstantBytesV1::new(vec![7; n]).unwrap()),
            ))
        };
        let a = raw(
            &types,
            &comparison(&f, typed_operand(2, U64_TYPE), bytes(1)),
            0,
            LIMIT,
            LIMIT,
        );
        let b = raw(
            &types,
            &comparison(&f, typed_operand(2, U64_TYPE), bytes(2048)),
            0,
            LIMIT,
            LIMIT,
        );
        assert!(a.data.is_none() && b.data.is_none());
        assert_eq!(b.work - a.work, 2047);
        let long = comparison(&f, typed_operand(2, U64_TYPE), bytes(2048));
        let denied = raw(&types, &long, 0, b.work - 1, LIMIT);
        assert!(denied.failed_work);
        let last = 2048 + 2 * std::mem::size_of::<SemanticOperandV1>() + 16;
        assert_eq!(denied.work, b.work - last);
        let c = raw(
            &types,
            &comparison(&f, typed_operand(4, U64_TYPE), bytes(1)),
            0,
            LIMIT,
            LIMIT,
        );
        let d = raw(
            &types,
            &comparison(&f, typed_operand(4, U64_TYPE), bytes(2048)),
            0,
            LIMIT,
            LIMIT,
        );
        assert_eq!(c.work, d.work);
        assert_eq!(c.error, d.error);
    }
    #[test]
    fn fixed_guard_raw_unmetered_and_wrong_table_or_graph_shape_refuse() {
        let types = assertion_proof_types();
        let f = good();
        let graph = projected_loop_cfg_graph_v1(&f).unwrap();
        let inventory = assertion_definition_inventory(&f).unwrap();
        assert!(
            fixed_guard_raw_probe_for_test_v1(
                &types,
                &f,
                &graph,
                &inventory,
                0,
                &mut PreparationResourcesV1::unmetered()
            )
            .is_err()
        );
        let mut inventory = assertion_definition_inventory(&f).unwrap();
        inventory.counts.pop();
        assert!(
            raw_with(&types, &f, &graph, &inventory, 0, LIMIT, LIMIT)
                .error
                .unwrap()
                .contains("source tables differ")
        );
        let mut graph = projected_loop_cfg_graph_v1(&f).unwrap();
        graph.entry = 1;
        assert!(
            raw_with(
                &types,
                &f,
                &graph,
                &assertion_definition_inventory(&f).unwrap(),
                0,
                LIMIT,
                LIMIT
            )
            .error
            .unwrap()
            .contains("source tables differ")
        );
    }

    macro_rules! in_rich {
        ($types:ident,$f:ident,$rich:ident,$budget:ident,$owned:ident,$body:block)=>{{
            let mut work=Work::new(LIMIT);let mut original=Budget::new(&mut work,LIMIT);
            original.reserve_storage(FLOOR).unwrap();
            let result=with_rich_tables_for_test_v1(&[],&$types,&$f,&mut original,|$rich,$budget|{
                let floor=$budget.storage();let mut $owned=0;
                $body
                assert_eq!($budget.storage(),floor+$owned);
                $budget.release_storage($owned).unwrap();assert_eq!($budget.storage(),floor);
                Ok(())
            });
            assert_eq!(original.storage(),FLOOR);result
        }};
    }
    #[test]
    fn fixed_guard_eager_rich_session_reuses_existing_proof_and_cache() {
        let types = assertion_proof_types();
        let f = good();
        let graph = projected_loop_cfg_graph_v1(&f).unwrap();
        in_rich!(types, f, rich, budget, owned, {
            let mut resources = PreparationResourcesV1::new(budget, &mut owned);
            let mut session =
                fixed_guard_session_for_test_v1(&types, &f, &graph, rich, &mut resources).unwrap();
            let mut old = SemanticAssertProofsV1::new(&types, &f).unwrap();
            for _ in 0..3 {
                let expected = original_selected(&mut old, 0).unwrap();
                assert_eq!(
                    session
                        .query_for_test_v1(&types, &f, &graph, rich, 0)
                        .unwrap(),
                    expected
                );
                let state = session.state_for_test_v1();
                assert_eq!(state.0, old.work);
                assert!(!state.1);
                assert_eq!(state.2, 1);
            }
            drop(session);
            drop(resources);
        })
        .unwrap();
    }
    #[test]
    fn fixed_guard_foreign_equal_content_function_types_and_graph_query_refuse() {
        let types = assertion_proof_types();
        let f = good();
        let graph = projected_loop_cfg_graph_v1(&f).unwrap();
        let foreign = f.clone();
        let foreign_types = types.clone();
        let foreign_graph = projected_loop_cfg_graph_v1(&f).unwrap();
        for case in 0..3 {
            in_rich!(types, f, rich, budget, owned, {
                let mut resources = PreparationResourcesV1::new(budget, &mut owned);
                let mut session =
                    fixed_guard_session_for_test_v1(&types, &f, &graph, rich, &mut resources)
                        .unwrap();
                let result = match case {
                    0 => session.query_for_test_v1(&types, &foreign, &graph, rich, 0),
                    1 => session.query_for_test_v1(&foreign_types, &f, &graph, rich, 0),
                    _ => session.query_for_test_v1(&types, &f, &foreign_graph, rich, 0),
                };
                assert!(format!("{:?}", result.err().unwrap()).contains("original source loan"));
                assert!(session.state_for_test_v1().1);
                assert!(
                    session
                        .query_for_test_v1(&types, &f, &graph, rich, 0)
                        .is_err()
                );
                drop(session);
                drop(resources);
            })
            .unwrap();
        }
    }
    #[test]
    fn fixed_guard_rich_foreign_source_and_original_ledger_constructor_refuse() {
        let types = assertion_proof_types();
        let f = good();
        let foreign = f.clone();
        let graph = projected_loop_cfg_graph_v1(&f).unwrap();
        in_rich!(types, f, rich, budget, owned, {
            {
                let mut resources = PreparationResourcesV1::new(budget, &mut owned);
                let result =
                    fixed_guard_session_for_test_v1(&types, &foreign, &graph, rich, &mut resources);
                assert!(result.is_err());
                drop(result);
            }
            let mut other_work = Work::new(LIMIT);
            let mut other = Budget::new(&mut other_work, LIMIT);
            let mut other_owned = 0;
            {
                let mut resources = PreparationResourcesV1::new(&mut other, &mut other_owned);
                let result =
                    fixed_guard_session_for_test_v1(&types, &f, &graph, rich, &mut resources);
                assert!(result.is_err());
                drop(result);
            }
            assert!(other_owned > 0);
            assert_eq!(other.storage(), other_owned);
            other.release_storage(other_owned).unwrap();
            assert_eq!(other.storage(), 0);
        })
        .unwrap();
    }
    #[test]
    fn fixed_guard_rich_foreign_row_loan_and_ledger_query_refuse() {
        let types = assertion_proof_types();
        let f = good();
        let graph = projected_loop_cfg_graph_v1(&f).unwrap();
        in_rich!(types, f, rich, budget, owned, {
            let mut resources = PreparationResourcesV1::new(budget, &mut owned);
            let mut session =
                fixed_guard_session_for_test_v1(&types, &f, &graph, rich, &mut resources).unwrap();
            let mut other_work = Work::new(LIMIT);
            let mut other = Budget::new(&mut other_work, LIMIT);
            with_rich_tables_for_test_v1(&[], &types, &f, &mut other, |foreign_rich, _| {
                assert!(
                    session
                        .query_for_test_v1(&types, &f, &graph, foreign_rich, 0)
                        .is_err()
                );
                Ok(())
            })
            .unwrap();
            assert_eq!(other.storage(), 0);
            assert!(session.state_for_test_v1().1);
            drop(session);
            drop(resources);
        })
        .unwrap();
    }
    #[test]
    fn fixed_guard_stale_source_coordinate_poison_prevents_retry() {
        let types = assertion_proof_types();
        let f = good();
        let graph = projected_loop_cfg_graph_v1(&f).unwrap();
        in_rich!(types, f, rich, budget, owned, {
            let mut resources = PreparationResourcesV1::new(budget, &mut owned);
            let mut session =
                fixed_guard_session_for_test_v1(&types, &f, &graph, rich, &mut resources).unwrap();
            assert!(
                session
                    .query_for_test_v1(&types, &f, &graph, rich, 99)
                    .is_err()
            );
            assert!(session.state_for_test_v1().1);
            assert!(
                session
                    .query_for_test_v1(&types, &f, &graph, rich, 0)
                    .is_err()
            );
            drop(session);
            drop(resources);
        })
        .unwrap();
    }
    #[test]
    fn fixed_guard_caller_unwind_drops_proof_before_owned_refund() {
        let types = assertion_proof_types();
        let f = good();
        let graph = projected_loop_cfg_graph_v1(&f).unwrap();
        in_rich!(types, f, rich, budget, owned, {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut resources = PreparationResourcesV1::new(budget, &mut owned);
                let mut session =
                    fixed_guard_session_for_test_v1(&types, &f, &graph, rich, &mut resources)
                        .unwrap();
                assert!(
                    session
                        .query_for_test_v1(&types, &f, &graph, rich, 0)
                        .is_ok()
                );
                panic!("intentional fixed-guard component caller unwind");
            }));
            assert!(outcome.is_err());
            drop(outcome);
            assert!(owned > 0);
        })
        .unwrap();
    }

    #[test]
    fn fixed_guard_same_original_ledger_but_distinct_rich_row_loan_refuses() {
        let types = assertion_proof_types();
        let f = good();
        let graph = projected_loop_cfg_graph_v1(&f).unwrap();
        in_rich!(types, f, rich, budget, owned, {
            with_rich_tables_for_test_v1(&[], &types, &f, budget, |other_rich, original_budget| {
                let mut resources = PreparationResourcesV1::new(original_budget, &mut owned);
                let mut session =
                    fixed_guard_session_for_test_v1(&types, &f, &graph, rich, &mut resources)
                        .unwrap();
                assert!(
                    format!(
                        "{:?}",
                        session
                            .query_for_test_v1(&types, &f, &graph, other_rich, 0)
                            .err()
                            .unwrap()
                    )
                    .contains("original source loan")
                );
                assert!(session.state_for_test_v1().1);
                drop(session);
                drop(resources);
                // The nested rich owner can refund only its own credits. This
                // component's accepted credits must leave before that callback.
                original_budget.release_storage(owned).unwrap();
                owned = 0;
                Ok(())
            })
            .unwrap();
        })
        .unwrap();
    }
    #[test]
    fn fixed_guard_preexisting_denial_is_sticky_and_does_not_accept_new_credits() {
        let types = assertion_proof_types();
        let f = good();
        let graph = projected_loop_cfg_graph_v1(&f).unwrap();
        let inventory = assertion_definition_inventory(&f).unwrap();
        let mut work = Work::new(1);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(budget.charge_work(2).is_err());
        let before = budget.work();
        let mut owned = 0;
        {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            assert!(
                fixed_guard_raw_probe_for_test_v1(
                    &types,
                    &f,
                    &graph,
                    &inventory,
                    0,
                    &mut resources
                )
                .is_err()
            );
        }
        assert_eq!(budget.work(), before);
        assert_eq!(owned, 0);
        assert_eq!(budget.storage(), FLOOR);
        assert!(budget.failed_work().is_some());
    }
}
