#[test]
fn discovery_inventory_borrowed_and_proof_owned_emit_identical_bound() {
    let types = assertion_proof_types();
    let function = widened_u64_induction_function_with_latch(16, WidenedLatchKind::Checked);
    let definitions = local_definition_counts(&function);
    let constants = vec![None; function.locals().len()];
    let mut outputs = Vec::new();
    for proof_owned in [false, true] {
        let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
        let assignments = proof.assignments.clone();
        let escapes = proof.address_escaped.clone();
        let expected_assignments = if proof_owned {
            &proof.assignments
        } else {
            &assignments
        };
        let expected_escapes = if proof_owned {
            &proof.address_escaped
        } else {
            &escapes
        };
        let inventory_identity = (expected_assignments.as_ptr(), expected_escapes.as_ptr());
        let owner_identity = (proof.assignments.as_ptr(), proof.address_escaped.as_ptr());
        let mut arguments = vec![None; function.locals().len()];
        let mut next_argument = 1;
        let mut operations = Vec::new();
        let mut next_value = 0;
        let mut projector = TotalUnsignedIndexProjectorV1::with_inventory(
            &types,
            &function,
            &constants,
            &definitions,
            if proof_owned {
                TotalUnsignedIndexInventoryV1::ProofOwned
            } else {
                TotalUnsignedIndexInventoryV1::Borrowed {
                    address_escaped: &escapes,
                    definitions: &assignments,
                }
            },
            &mut proof,
            &mut arguments,
            &mut next_argument,
            &mut operations,
            &mut next_value,
        )
        .unwrap();
        assert_eq!(
            (
                projector.definitions().as_ptr(),
                projector.address_escaped().as_ptr()
            ),
            inventory_identity
        );
        let state_capacity = projector.states.capacity();
        let result = projector
            .resolve_operand(&discovery_bound_operand_v1(), 1, 0)
            .unwrap()
            .unwrap();
        assert_eq!(result.maximum, u64::from(u32::MAX));
        assert!(!result.invocation_dependent);
        assert_eq!(projector.states.capacity(), state_capacity);
        let node_work = projector.node_work;
        drop(projector);
        assert_eq!(
            (proof.assignments.as_ptr(), proof.address_escaped.as_ptr()),
            owner_identity
        );
        outputs.push((
            result.ranked,
            result.maximum,
            arguments,
            next_argument,
            operations,
            next_value,
            proof.work,
            node_work,
        ));
    }
    assert_eq!(outputs[0], outputs[1]);
}

#[test]
fn discovery_inventory_existing_constructor_keeps_conservative_supplied_tables() {
    let types = assertion_proof_types();
    let function = widened_u64_induction_function_with_latch(16, WidenedLatchKind::Checked);
    let definitions = local_definition_counts(&function);
    let constants = vec![None; function.locals().len()];
    for mutation in 0..3 {
        let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
        let mut assignments = proof.assignments.clone();
        let mut escapes = proof.address_escaped.clone();
        match mutation {
            1 => escapes[4] = true,
            2 => assignments[3] = None,
            _ => {}
        }
        let mut arguments = vec![None; function.locals().len()];
        let mut next_argument = 1;
        let mut operations = Vec::new();
        let mut next_value = 0;
        let mut projector = TotalUnsignedIndexProjectorV1::new(
            &types,
            &function,
            &constants,
            &definitions,
            &escapes,
            &assignments,
            &mut proof,
            &mut arguments,
            &mut next_argument,
            &mut operations,
            &mut next_value,
        )
        .unwrap();
        assert_eq!(
            projector
                .resolve_operand(&discovery_bound_operand_v1(), 1, 0)
                .unwrap()
                .is_some(),
            mutation == 0
        );
    }
}

#[test]
fn discovery_inventory_source_identity_and_table_lengths_remain_exact() {
    let types = assertion_proof_types();
    let function = widened_u64_induction_function_with_latch(16, WidenedLatchKind::Checked);
    let other = function.clone();
    for proof_owned in [false, true] {
        for mutation in 0..4 {
            let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
            let mut constants = vec![None; function.locals().len()];
            let mut definitions = local_definition_counts(&function);
            let assignments = proof.assignments.clone();
            let escapes = proof.address_escaped.clone();
            let mut arguments = vec![None; function.locals().len()];
            match mutation {
                0 => {
                    constants.pop();
                }
                1 => {
                    definitions.pop();
                }
                2 => {
                    arguments.pop();
                }
                _ => {}
            }
            let mut next_argument = 1;
            let mut operations = Vec::new();
            let mut next_value = 0;
            let result = TotalUnsignedIndexProjectorV1::with_inventory(
                &types,
                if mutation == 3 { &other } else { &function },
                &constants,
                &definitions,
                if proof_owned {
                    TotalUnsignedIndexInventoryV1::ProofOwned
                } else {
                    TotalUnsignedIndexInventoryV1::Borrowed {
                        address_escaped: &escapes,
                        definitions: &assignments,
                    }
                },
                &mut proof,
                &mut arguments,
                &mut next_argument,
                &mut operations,
                &mut next_value,
            );
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "pure uniform index tables do not match the semantic local table"
                ))
            ));
        }
    }
}

#[test]
fn discovery_inventory_proof_owned_node_limit_still_fails_closed() {
    let types = assertion_proof_types();
    let function = widened_u64_induction_function_with_latch(16, WidenedLatchKind::Checked);
    let definitions = local_definition_counts(&function);
    let constants = vec![None; function.locals().len()];
    for proof_owned in [false, true] {
        let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
        let assignments = proof.assignments.clone();
        let escapes = proof.address_escaped.clone();
        let mut arguments = vec![None; function.locals().len()];
        let mut next_argument = 1;
        let mut operations = Vec::new();
        let mut next_value = 0;
        let mut projector = TotalUnsignedIndexProjectorV1::with_inventory(
            &types,
            &function,
            &constants,
            &definitions,
            if proof_owned {
                TotalUnsignedIndexInventoryV1::ProofOwned
            } else {
                TotalUnsignedIndexInventoryV1::Borrowed {
                    address_escaped: &escapes,
                    definitions: &assignments,
                }
            },
            &mut proof,
            &mut arguments,
            &mut next_argument,
            &mut operations,
            &mut next_value,
        )
        .unwrap();
        let capacity = projector.states.capacity();
        for _ in 0..MAX_PURE_UNIFORM_INDEX_NODES_V1 {
            projector.charge_node().unwrap();
        }
        assert_eq!(projector.node_work, MAX_PURE_UNIFORM_INDEX_NODES_V1);
        assert_loop_unsupported(
            projector.resolve_operand(&discovery_bound_operand_v1(), 1, 0),
            "pure uniform index expression exceeds its node limit",
        );
        assert_eq!(projector.states.capacity(), capacity);
    }
}
fn discovery_bound_operand_v1() -> SemanticOperandV1 {
    SemanticOperandV1::Copy(
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(3), vec![], U64_TYPE).unwrap(),
    )
}
