// All exports in this included leaf are cfg(test)-only. They return diagnostics,
// never a constructed owner, source Fact, graph certificate or public facade.
mod qualification_oracle_v1 {
    include!("production_canonical_scalar_assertion_qualification_oracle_v1_tests.rs");
    include!("production_canonical_scalar_assertion_qualification_costs_v1_tests.rs");
    include!("production_canonical_scalar_assertion_qualification_source_costs_v1_tests.rs");
}

pub(super) fn qualification_cost_components_v1(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    dynamic: bool,
) {
    qualification_oracle_v1::qualify_graph_components(owner, dynamic);
    qualification_oracle_v1::qualify_source_components(owner.original_source(), dynamic);
    qualification_oracle_v1::qualify_call_index(owner.original_source(), dynamic);
}

#[derive(Clone, Copy, Debug)]
pub(super) enum QualificationJoinFaultV1 {
    SelectorOperand,
    SelectorDefinition,
    SuccessPolarity,
    FinalAliasOrder,
    FinalSyntheticAlias,
    ForeignFinalOwner,
}

pub(super) fn qualification_final_reader_v1(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    donor: &ProductionCanonicalScalarFixedPointOwnerV1,
    fault: QualificationJoinFaultV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<()> {
    // Genuine original source, every actual transition, fresh final inventory
    // and full C all precede the intentionally unauthenticated reader mutation.
    csa_scope_v1(budget, |budget| {
        owner.original.with_checked_canonical_ranked_source_v1(budget, |view, budget| {
            Ok(csa_with_source_v1(view, budget, |source, coverage, budget| {
                owner.history.replay_against(owner.original.executable(), budget)?;
                let mut assertions = CsaTransportV1::original(source, coverage, budget)?;
                let lineage = cs_lineage_with_observer_v1(owner, source.inventory, &mut assertions, budget)?;
                cs_with_final_view_v1(owner, source, &lineage, budget, |output, checked, budget| {
                    fe2o3_pliron::with_canonical_trap_policy_checks_v1(checked, budget, |policies, budget| {
                        Ok(csa_scope_v1(budget, |budget| {
                            csa_final_join_v1(source, coverage, output, &lineage, &assertions, policies, budget)?;
                            match fault {
                                QualificationJoinFaultV1::SelectorOperand
                                | QualificationJoinFaultV1::SelectorDefinition
                                | QualificationJoinFaultV1::SuccessPolarity => {
                                    let row = &assertions.rows[0];
                                    let mut condition = row.state.condition.expect("retained actual selector");
                                    let CsaEdgePlaceV1::Retained(mut success) = row.state.success else {
                                        panic!("retained actual success edge")
                                    };
                                    csa_condition_v1(output, condition, success, row.binding.expected(), budget)?;
                                    match fault {
                                        QualificationJoinFaultV1::SelectorOperand => {
                                            let CsUseV1::TerminatorOperand { block, operand: 0 } = condition.used else {
                                                panic!("baseline uses selector operand zero")
                                            };
                                            condition.used = CsUseV1::TerminatorOperand { block, operand: 1 };
                                        }
                                        QualificationJoinFaultV1::SelectorDefinition => {
                                            let other = assertions.rows[1].state.condition.expect("second retained selector");
                                            assert_ne!(condition.definition, other.definition);
                                            let ordinal = cs_definition_v1(output, other.definition, budget)?;
                                            assert_eq!(*output.definitions()[ordinal].ty, Type::BOOL);
                                            condition.definition = other.definition;
                                        }
                                        QualificationJoinFaultV1::SuccessPolarity => {
                                            std::mem::swap(&mut success.successor, &mut condition.failure.successor);
                                        }
                                        _ => unreachable!(),
                                    }
                                    csa_condition_v1(output, condition, success, row.binding.expected(), budget)
                                }
                                QualificationJoinFaultV1::FinalAliasOrder => {
                                    assert_eq!(assertions.rows.len(), 2);
                                    assert_ne!(assertions.rows[0].span, assertions.rows[1].span);
                                    assertions.rows.swap(0, 1);
                                    csa_final_join_v1(source, coverage, output, &lineage, &assertions, policies, budget)
                                }
                                QualificationJoinFaultV1::FinalSyntheticAlias => {
                                    assert!(policies.pair_count(budget)? > 0);
                                    budget.reserve_storage(std::mem::size_of::<Coverage<'_>>())?;
                                    let mut copies = rows(coverage.rows.len(), budget)?;
                                    for row in &coverage.rows {
                                        copies.push(row.as_ref().map(|row| ProductionCanonicalAssertionV1 {
                                            span: row.span, binding: row.binding, proof: row.proof,
                                        }));
                                    }
                                    let mut synthetic = rows(coverage.synthetic.len(), budget)?;
                                    synthetic.extend_from_slice(&coverage.synthetic);
                                    let at = synthetic.iter().position(|row| *row).expect("actual synthetic trap alias");
                                    synthetic[at] = false;
                                    let unauthenticated = Coverage { inventory: coverage.inventory, rows: copies, synthetic };
                                    csa_final_join_v1(source, &unauthenticated, output, &lineage, &assertions, policies, budget)
                                }
                                QualificationJoinFaultV1::ForeignFinalOwner => {
                                    assert_eq!(output.owner().canonical().canonical_bytes(), donor.output().canonical().canonical_bytes());
                                    assert!(!std::ptr::eq(output.owner(), donor.output()));
                                    let (foreign, storage) = CanonicalKirInventoryV1::derive(donor.output(), budget)?;
                                    budget.reserve_storage(storage.retained_storage())?;
                                    csa_final_join_v1(source, coverage, &foreign, &lineage, &assertions, policies, budget)
                                }
                            }
                        }))
                    }).map_err(ProductionCanonicalScalarSourceErrorV1::FinalPolicy)?
                })
            }))
        })?
    })
}

pub(super) fn qualification_foreign_original_v1(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    donor: &ProductionCanonicalScalarFixedPointOwnerV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<()> {
    csa_scope_v1(budget, |budget| {
        owner
            .original
            .with_checked_canonical_ranked_source_v1(budget, |view, budget| {
                Ok(csa_with_source_v1(
                    view,
                    budget,
                    |source, coverage, budget| {
                        assert_eq!(
                            source.inventory.owner().canonical().canonical_bytes(),
                            donor.original.executable().canonical().canonical_bytes()
                        );
                        assert!(!std::ptr::eq(
                            source.inventory.owner(),
                            donor.original.executable()
                        ));
                        let mut baseline = CsaTransportV1::original(source, coverage, budget)?;
                        let valid = cs_lineage_with_observer_v1(
                            owner,
                            source.inventory,
                            &mut baseline,
                            budget,
                        )?;
                        drop(valid);
                        drop(baseline);
                        let mut assertions = CsaTransportV1::original(source, coverage, budget)?;
                        let (foreign, receipt) =
                            CanonicalKirInventoryV1::derive(donor.original.executable(), budget)?;
                        budget.reserve_storage(receipt.retained_storage())?;
                        let result =
                            cs_lineage_with_observer_v1(owner, &foreign, &mut assertions, budget);
                        result.map(drop)
                    },
                ))
            })?
    })
}

struct QualificationPayloadObserverV1<'a> {
    actual: CsaTransportV1,
    reached: &'a std::cell::Cell<bool>,
}
impl CsPairObserverV1 for QualificationPayloadObserverV1<'_> {
    type Next = CsaTransportV1;
    #[allow(clippy::too_many_arguments)]
    fn pair(
        &self,
        input: &CanonicalKirInventoryV1<'_>,
        output: &CanonicalKirInventoryV1<'_>,
        control: &fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
        candidate: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
        previous: &CsLineageV1,
        next: &CsLineageV1,
        round: u16,
        integer: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<Self::Next> {
        for row in &self.actual.rows {
            let CsaEdgePlaceV1::Retained(edge) = row.state.success else {
                continue;
            };
            let placement = control.edge(edge, budget)?.placement;
            for argument in control.input_edge_arguments(edge, budget)? {
                let Some(mapped) = control.edge_argument(argument.coordinate, budget)? else {
                    continue;
                };
                csa_payloads_v1(output, control, candidate, edge, placement, budget)?;
                let ordinal = cs_argument_v1(output, mapped, budget)?;
                let mut changed = cs_vec_v1(candidate.edge_arguments.len(), budget)?;
                changed.extend_from_slice(candidate.edge_arguments);
                assert_eq!(changed[ordinal].input, argument.coordinate);
                // Only an inert candidate occurrence changes. The checked
                // control index and both actual owners stay immutable.
                changed[ordinal].input.argument =
                    changed[ordinal].input.argument.checked_add(1).unwrap();
                self.reached.set(true);
                let hostile = fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1 {
                    edge_arguments: &changed,
                    ..candidate
                };
                return csa_payloads_v1(output, control, hostile, edge, placement, budget)
                    .map(|()| panic!("payload occurrence mutation was accepted"));
            }
        }
        self.actual.pair(
            input, output, control, candidate, previous, next, round, integer, budget,
        )
    }
    fn retained(next: &Self::Next) -> usize {
        CsaTransportV1::retained(next)
    }
    fn replace(&mut self, next: Self::Next) -> usize {
        self.actual.replace(next)
    }
}

pub(super) fn qualification_payload_reader_v1(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    reached: &std::cell::Cell<bool>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<()> {
    csa_scope_v1(budget, |budget| {
        owner
            .original
            .with_checked_canonical_ranked_source_v1(budget, |view, budget| {
                Ok(csa_with_source_v1(
                    view,
                    budget,
                    |source, coverage, budget| {
                        owner
                            .history
                            .replay_against(owner.original.executable(), budget)?;
                        let mut observer = QualificationPayloadObserverV1 {
                            actual: CsaTransportV1::original(source, coverage, budget)?,
                            reached,
                        };
                        cs_lineage_with_observer_v1(owner, source.inventory, &mut observer, budget)
                            .map(drop)
                    },
                ))
            })?
    })
}
