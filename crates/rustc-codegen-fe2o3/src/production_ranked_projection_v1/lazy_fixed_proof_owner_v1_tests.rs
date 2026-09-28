// Private component fixtures only; no actual prepared-view or all-producer loan.
mod lazy_fixed_proof_controls {
    use super::*;
    use crate::production_ranked_projection_v1::assertion_analyzer_resources_v1::lazy_fixed_proof_owner_v1::{
        LazyFixedProofOwnerV1 as Owner, LazyFixedEventV1 as Event,
    };
    use crate::production_ranked_projection_v1::bf16_nominal_source_preparation_v1::with_rich_tables_for_test_v1;
    use fe2o3_kernel_ir::{CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work};
    use std::{cell::Cell, rc::Rc};
    use fe2o3_mir_model::semantic_mir_v1::{SemanticSwitchTargetV1,SemanticSwitchTargetsV1};
    const LIMIT: usize = 512 * 1024 * 1024;
    const FLOOR: usize = 37;
    fn good() -> SemanticFunctionDeclV1 {
        fixed_guard_function(FixedGuardOptions {
            extent: 4,
            ..Default::default()
        })
    }
    fn scalar(n: u128) -> SemanticOperandV1 {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            U64_TYPE,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(n, 8).unwrap()),
        ))
    }
    fn rebuild(
        f: &SemanticFunctionDeclV1,
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
            f.locals().to_vec(),
            f.entry(),
            blocks,
        )
        .unwrap()
    }
    fn literal() -> SemanticFunctionDeclV1 {
        let f = good();
        let mut blocks = f.blocks().to_vec();
        blocks[0] = block(
            190,
            blocks[0].statements().to_vec(),
            SemanticTerminatorKindV1::Assert {
                condition: typed_operand(3, BOOL_TYPE),
                expected: true,
                message: SemanticAssertMessageV1::BoundsCheck {
                    length: scalar(4),
                    index: scalar(1),
                },
                target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        );
        rebuild(&f, blocks)
    }
    fn prefixed() -> SemanticFunctionDeclV1 {
        let f = good();
        let guard = &f.blocks()[0];
        let SemanticTerminatorKindV1::Assert {
            condition,
            expected,
            message,
            unwind,
            ..
        } = guard.terminator().kind()
        else {
            panic!("fixture")
        };
        rebuild(
            &f,
            vec![
                block(
                    191,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: typed_operand(2, U64_TYPE),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                            )],
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                        )
                        .unwrap(),
                    },
                ),
                block(
                    190,
                    guard.statements().to_vec(),
                    SemanticTerminatorKindV1::Assert {
                        condition: condition.clone(),
                        expected: *expected,
                        message: message.clone(),
                        target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 2),
                        unwind: *unwind,
                    },
                ),
                f.blocks()[1].clone(),
            ],
        )
    }
    macro_rules! in_owner {
        ($types:ident,$f:ident,$owner:ident,$body:block) => {
            in_owner!(@expect_denial false, $types, $f, $owner, $body)
        };
        (@expect_denial $denied:expr,$types:ident,$f:ident,$owner:ident,$body:block) => {{
            let graph=projected_loop_cfg_graph_v1(&$f).unwrap();
            let mut work=Work::new(LIMIT);let mut budget=Budget::new(&mut work,LIMIT);
            budget.reserve_storage(FLOOR).unwrap();
            let result=with_rich_tables_for_test_v1(&[],&$types,&$f,&mut budget,|rich,budget|{
                let floor=budget.storage();let identity=budget.work_ledger_identity_v1();
                let mut owned=0;
                {
                    let mut resources=PreparationResourcesV1::new(budget,&mut owned);
                    let mut $owner=Owner::for_test(&$types,&$f,&graph,rich,&mut resources).unwrap();
                    $body
                    drop($owner);drop(resources);
                }
                assert!(budget.work_ledger_identity_v1()==identity);
                assert_eq!(budget.storage(),floor+owned);
                budget.release_storage(owned).unwrap();assert_eq!(budget.storage(),floor);
                Ok(())
            });
            assert_eq!(budget.storage(),FLOOR);
            if $denied {
                assert!(budget.failed_work().is_some());
                assert!(matches!(result, Err(fe2o3_lower_mir_kernel::Bf16NominalCallQueryErrorV1::Resource(
                    fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
                ))));
            } else {
                result.unwrap();
            }
        }};
    }

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

    fn original_event<'a>(
        types: &'a [SemanticTypeDeclV1],
        f: &'a SemanticFunctionDeclV1,
        constants: &[Option<u64>],
        block_index: usize,
        proof: &mut Option<SemanticAssertProofsV1<'a>>,
    ) -> Result<Event, ProductionRankedProjectionErrorV1> {
        let source = &f.blocks()[block_index];
        match source.terminator().kind() {
            SemanticTerminatorKindV1::Assert {
                condition,
                expected,
                message: SemanticAssertMessageV1::BoundsCheck { length, index },
                target,
                unwind,
            } => {
                if !*expected || !matches!(unwind, SemanticUnwindActionV1::Unreachable) {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "a Rust bounds check without the canonical success/unreachable shape",
                    ));
                }
                if constant_operand_value(length, constants).is_some()
                    && constant_operand_value(index, constants).is_some()
                {
                    return Ok(Event::LiteralSkip);
                }
                if matches!(length, SemanticOperandV1::Constant(_)) {
                    if proof.is_none() {
                        *proof = Some(SemanticAssertProofsV1::new(types, f)?);
                    }
                    let (index, extent) = original_authenticate_fixed_array_guard_v1(
                        proof.as_mut().unwrap(),
                        block_index,
                        target.target().index() as usize,
                        condition,
                        index,
                        length,
                    )?;
                    Ok(Event::Fixed { index, extent })
                } else {
                    Ok(Event::NonFixedBoundary)
                }
            }
            SemanticTerminatorKindV1::SwitchInt { .. } => Ok(Event::NonFixedBoundary),
            _ => Ok(Event::Other),
        }
    }
    #[test]
    fn lazy_original_event_order_and_first_fixed_match_frozen_guard_oracle() {
        let types = assertion_proof_types();
        for f in [
            good(),
            literal(),
            prefixed(),
            fixed_guard_function(FixedGuardOptions {
                extent: 4,
                wrong_comparison: true,
                ..Default::default()
            }),
        ] {
            let constants = constant_locals(&f).unwrap();
            let mut original = None;
            in_owner!(types, f, owner, {
                assert_eq!(owner.state_for_test().0, 0);
                for index in 0..f.blocks().len() {
                    let old = original_event(&types, &f, &constants, index, &mut original);
                    let actual = owner.visit(index);
                    assert_eq!(format!("{actual:?}"), format!("{old:?}"));
                    if actual.is_err() {
                        assert!(owner.state_for_test().2);
                        break;
                    }
                    assert_eq!(
                        owner.state_for_test().0,
                        if original.is_some() { 2 } else { 0 }
                    );
                }
            });
        }
    }
    #[test]
    fn lazy_literal_pair_and_return_never_build_proof() {
        let types = assertion_proof_types();
        let f = literal();
        in_owner!(types, f, owner, {
            assert_eq!(owner.visit(0).unwrap(), Event::LiteralSkip);
            assert_eq!(owner.visit(1).unwrap(), Event::Other);
            assert_eq!(owner.state_for_test().0, 0);
        });
    }
    #[test]
    fn lazy_nonfixed_boundary_does_not_authorize_slice_or_eagerly_build() {
        let types = assertion_proof_types();
        let f = prefixed();
        in_owner!(types, f, owner, {
            assert_eq!(owner.visit(0).unwrap(), Event::NonFixedBoundary);
            assert_eq!(owner.state_for_test().0, 0);
            assert!(matches!(
                owner.visit(1).unwrap(),
                Event::Fixed { extent: 4, .. }
            ));
            assert_eq!(owner.state_for_test().0, 2);
        });
    }
    #[test]
    fn lazy_canonical_refusal_precedes_literal_skip_and_build() {
        let types = assertion_proof_types();
        let f = fixed_guard_function(FixedGuardOptions {
            extent: 4,
            expected_false: true,
            ..Default::default()
        });
        in_owner!(types, f, owner, {
            assert!(format!("{:?}", owner.visit(0).err().unwrap()).contains("canonical success"));
            assert_eq!(owner.state_for_test().0, 0);
            assert!(owner.state_for_test().2);
            assert!(owner.visit(0).is_err());
        });
    }
    #[test]
    fn lazy_out_of_order_event_is_terminal_without_activation() {
        let types = assertion_proof_types();
        let f = good();
        in_owner!(types, f, owner, {
            assert!(owner.visit(1).is_err());
            assert_eq!(owner.state_for_test().0, 0);
            assert!(owner.visit(0).is_err());
            assert_eq!(owner.state_for_test().1, 0);
        });
    }
    #[test]
    fn lazy_duplicate_event_does_not_retry_ready_proof() {
        let types = assertion_proof_types();
        let f = good();
        in_owner!(types, f, owner, {
            owner.visit(0).unwrap();
            let work = owner.state_for_test().5;
            assert!(owner.visit(0).is_err());
            assert!(owner.visit(1).is_err());
            assert_eq!(owner.state_for_test().5, work);
        });
    }
    #[test]
    fn lazy_wrong_fixed_evidence_retains_ready_proof_and_is_terminal() {
        let types = assertion_proof_types();
        let f = fixed_guard_function(FixedGuardOptions {
            extent: 4,
            wrong_message: true,
            ..Default::default()
        });
        in_owner!(types, f, owner, {
            assert!(owner.visit(0).is_err());
            assert_eq!(owner.state_for_test().0, 2);
            assert!(owner.state_for_test().3 > 0);
            assert!(owner.state_for_test().2);
            assert!(owner.visit(1).is_err());
        });
    }
    #[test]
    fn lazy_refused_strict_constructor_has_no_payload_or_second_adapter() {
        let types = assertion_proof_types();
        let f = good();
        in_owner!(@expect_denial true, types, f, owner, {
            assert!(owner.denial_before_strict_for_test().is_err());
            assert_eq!(owner.state_for_test(), (3, 0, true, 0, 0, 0));
            assert!(owner.visit(0).is_err());
            let mut values: Vec<usize> = Vec::new();
            assert!(owner.reserve(&mut values, 1).is_err());
            assert_eq!(values.capacity(), 0);
        });
    }
    #[test]
    fn lazy_caught_initialization_unwind_retains_building_payload_until_owner_drop() {
        let types = assertion_proof_types();
        let f = good();
        in_owner!(types, f, owner, {
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                owner.panic_after_initialization_for_test()
            }));
            assert!(panic.is_err());
            drop(panic);
            let state = owner.state_for_test();
            assert_eq!(state.0, 1);
            assert!(state.2);
            assert_eq!(state.3, f.locals().len());
            assert!(state.4 >= state.3);
            assert_eq!(state.5, 2);
            assert!(owner.visit(0).is_err());
            assert_eq!(owner.state_for_test(), state);
        });
    }
    #[test]
    fn lazy_exact_preparation_reserve_and_push_continue_through_same_ready_owner() {
        let types = assertion_proof_types();
        let f = good();
        in_owner!(types, f, owner, {
            let mut values = Vec::new();
            let mut first = Some(7usize);
            owner.push_retained(&mut values, &mut first).unwrap();
            assert!(first.is_none());
            owner.visit(0).unwrap();
            let mut second = Some(11usize);
            owner.push_retained(&mut values, &mut second).unwrap();
            assert!(second.is_none());
            assert_eq!(values, vec![7, 11]);
            assert_eq!(values.capacity(), 2);
            drop(values);
        });
    }
    struct Tracked {
        id: usize,
        drops: Rc<Cell<usize>>,
        payload: Box<usize>,
    }
    impl Drop for Tracked {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }
    #[test]
    fn lazy_failed_push_keeps_candidate_identity_and_destructor_unrun() {
        let types = assertion_proof_types();
        let f = good();
        in_owner!(types, f, owner, {
            let drops = Rc::new(Cell::new(0));
            let mut candidate = Some(Tracked {
                id: 9,
                drops: drops.clone(),
                payload: Box::new(31),
            });
            let address = &*candidate.as_ref().unwrap().payload as *const usize;
            assert!(owner.visit(1).is_err());
            let mut values = Vec::new();
            assert!(owner.push_retained(&mut values, &mut candidate).is_err());
            assert!(values.is_empty());
            assert_eq!(drops.get(), 0);
            assert_eq!(candidate.as_ref().unwrap().id, 9);
            assert_eq!(
                &*candidate.as_ref().unwrap().payload as *const usize,
                address
            );
            drop(candidate);
            drop(values);
            assert_eq!(drops.get(), 1);
        });
    }
    #[test]
    fn lazy_successful_take_and_expected_caller_unwind_keep_one_physical_candidate() {
        let types = assertion_proof_types();
        let f = good();
        in_owner!(types, f, owner, {
            let drops = Rc::new(Cell::new(0));
            let mut candidate = Some(Tracked {
                id: 5,
                drops: drops.clone(),
                payload: Box::new(19),
            });
            let address = &*candidate.as_ref().unwrap().payload as *const usize;
            let mut values = Vec::new();
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                owner.push_retained(&mut values, &mut candidate).unwrap();
                panic!("intentional caller unwind with retained candidate");
            }));
            assert!(panic.is_err());
            drop(panic);
            assert!(candidate.is_none());
            assert_eq!(drops.get(), 0);
            assert_eq!(values.len(), 1);
            assert_eq!(&*values[0].payload as *const usize, address);
            drop(values);
            assert_eq!(drops.get(), 1);
        });
    }
    #[test]
    fn lazy_empty_candidate_refuses_before_allocation_and_stays_terminal() {
        let types = assertion_proof_types();
        let f = good();
        in_owner!(types, f, owner, {
            let mut values: Vec<usize> = Vec::new();
            let mut candidate = None;
            assert!(owner.push_retained(&mut values, &mut candidate).is_err());
            assert_eq!(values.capacity(), 0);
            assert!(owner.state_for_test().2);
            candidate = Some(1);
            assert!(owner.push_retained(&mut values, &mut candidate).is_err());
            assert_eq!(candidate, Some(1));
        });
    }

    #[test]
    fn lazy_foreign_function_or_original_ledger_refuses_without_proof_activation() {
        let types = assertion_proof_types();
        let f = good();
        let foreign = f.clone();
        let graph = projected_loop_cfg_graph_v1(&f).unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        with_rich_tables_for_test_v1(&[], &types, &f, &mut budget, |rich, budget| {
            let floor = budget.storage();
            let mut owned = 0;
            {
                let mut resources = PreparationResourcesV1::new(budget, &mut owned);
                let result = Owner::for_test(&types, &foreign, &graph, rich, &mut resources);
                assert!(result.is_err());
                drop(result);
            }
            assert_eq!(budget.storage(), floor + owned);
            budget.release_storage(owned).unwrap();
            let mut other_work = Work::new(LIMIT);
            let mut other = Budget::new(&mut other_work, LIMIT);
            let mut other_owned = 0;
            {
                let mut resources = PreparationResourcesV1::new(&mut other, &mut other_owned);
                let result = Owner::for_test(&types, &f, &graph, rich, &mut resources);
                assert!(result.is_err());
                drop(result);
            }
            assert_eq!(other.storage(), other_owned);
            other.release_storage(other_owned).unwrap();
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn lazy_late_cache_denial_retains_partial_checked_index_and_terminal_handle() {
        let types = assertion_proof_types();
        let f = good();
        in_owner!(@expect_denial true, types, f, owner, {
            assert!(owner.deny_after_index_for_test().is_err());
            let state = owner.state_for_test();
            assert_eq!(state.0, 1);
            assert!(state.2);
            assert_eq!(state.3, f.locals().len());
            assert!(state.4 >= state.3);
            assert_eq!(state.5, 0);
            assert!(owner.visit(0).is_err());
            assert_eq!(owner.state_for_test(), state);
        });
    }

    #[test]
    fn lazy_ready_denial_before_fitting_capacity_keeps_candidate_payload() {
        let types = assertion_proof_types();
        let f = good();
        in_owner!(@expect_denial true, types, f, owner, {
            owner.visit(0).unwrap();
            let mut values = Vec::new();
            owner.reserve::<Tracked>(&mut values, 1).unwrap();
            let cap = values.capacity();
            let drops = Rc::new(Cell::new(0));
            let mut candidate = Some(Tracked {
                id: 4,
                drops: drops.clone(),
                payload: Box::new(23),
            });
            let address = &*candidate.as_ref().unwrap().payload as *const usize;
            owner.deny_available_for_test();
            assert!(owner.push_retained(&mut values, &mut candidate).is_err());
            assert_eq!(values.capacity(), cap);
            assert!(values.is_empty());
            assert_eq!(drops.get(), 0);
            assert_eq!(
                &*candidate.as_ref().unwrap().payload as *const usize,
                address
            );
            drop(candidate);
            drop(values);
            assert_eq!(drops.get(), 1);
            assert!(owner.state_for_test().2);
        });
    }
    #[test]
    fn lazy_reserve_arithmetic_refusal_preserves_caller_vec_and_terminal_state() {
        let types = assertion_proof_types();
        let f = good();
        in_owner!(types, f, owner, {
            let mut values = Vec::new();
            let mut candidate = Some(9usize);
            owner.push_retained(&mut values, &mut candidate).unwrap();
            let pointer = values.as_ptr();
            let capacity = values.capacity();
            assert!(owner.reserve(&mut values, usize::MAX).is_err());
            assert_eq!(values, vec![9]);
            assert_eq!(values.as_ptr(), pointer);
            assert_eq!(values.capacity(), capacity);
            assert!(owner.state_for_test().2);
            assert!(owner.visit(0).is_err());
            drop(values);
        });
    }

    #[derive(Debug, PartialEq, Eq)]
    struct Measured {
        ok: bool,
        work: usize,
        peak: usize,
        denied_work: bool,
        denied_storage: bool,
    }
    fn measured(
        types: &[SemanticTypeDeclV1],
        f: &SemanticFunctionDeclV1,
        work_limit: usize,
        storage_limit: usize,
    ) -> Measured {
        let graph = projected_loop_cfg_graph_v1(f).unwrap();
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        let result = with_rich_tables_for_test_v1(&[], types, f, &mut budget, |rich, budget| {
            let floor = budget.storage();
            let mut owned = 0;
            let result = {
                let mut resources = PreparationResourcesV1::new(budget, &mut owned);
                let owner = Owner::for_test(types, f, &graph, rich, &mut resources);
                match owner {
                    Ok(mut owner) => owner.visit(0).map(|_| ()),
                    Err(error) => Err(error),
                }
            };
            assert_eq!(budget.storage(), floor + owned);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), floor);
            Ok(result.is_ok())
        });
        assert_eq!(budget.storage(), 0);
        Measured {
            ok: matches!(result, Ok(true)),
            work: budget.work(),
            peak: budget.peak_storage(),
            denied_work: budget.failed_work().is_some(),
            denied_storage: budget.failed_storage().is_some(),
        }
    }
    #[test]
    fn lazy_exact_component_work_storage_and_one_short_are_measured_not_residuals() {
        let types = assertion_proof_types();
        let f = good();
        let full = measured(&types, &f, LIMIT, LIMIT);
        assert!(full.ok);
        assert_eq!(measured(&types, &f, full.work, full.peak), full);
        let short_work = measured(&types, &f, full.work - 1, full.peak);
        assert!(!short_work.ok && short_work.denied_work);
        let short_storage = measured(&types, &f, full.work, full.peak - 1);
        assert!(!short_storage.ok && short_storage.denied_storage);
    }
}

include!("lazy_fixed_proof_retirement_v1_tests.rs");
