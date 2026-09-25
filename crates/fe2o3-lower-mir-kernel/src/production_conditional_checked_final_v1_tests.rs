//! Semantic components, not a manufactured live source request or proof receipt.
//! Public request consumption still requires the backend's genuine protected run.
use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
#[path = "production_conditional_checked_final_fixture_v1_tests.rs"]
mod fixture;
use fixture::{Complete, FLOOR, STORAGE, WORK, with_complete};
#[path = "production_conditional_checked_final_resources_v1_tests.rs"]
mod resources;

fn inspect(
    p: &Complete,
    inputs: Inputs<'_>,
    limits: Limits,
    premises: &[Premise],
    target: &mut Budget<'_>,
    source: &mut Budget<'_>,
) -> Result<()> {
    scoped(source, |source| {
        scoped(target, |target| {
            require_subjects(&p.b, &p.checked, inputs, limits, target)?;
            check_final(
                &p.checked,
                inputs,
                &KernelId::new("entry"),
                premises,
                target,
                source,
            )
        })
    })
}

fn original_premises(p: &Complete, source: &mut Budget<'_>) -> Vec<Premise> {
    scoped(source, |source| {
        let before = facts(p.checked.owner(), &KernelId::new("entry"), source)?;
        let reads = occurrences::reads(&before, source)?;
        let count = 4 + reads.len() * 3;
        source.charge_work(count * size_of::<Premise>())?;
        source.reserve_storage(count * size_of::<Premise>() + size_of::<Vec<Premise>>())?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| Resource::Allocation)?;
        source.reserve_storage((result.capacity() - count) * size_of::<Premise>())?;
        let parameter = before.output_parameter_index();
        result.extend([
            Premise::D1Launch,
            Premise::OutputWithinGlobalX { parameter },
            Premise::WritableOutput { parameter },
            Premise::RepresentableAddress {
                parameter,
                domain: before.address_domain(),
                element_bytes: before.element_bytes(),
                alignment: before.alignment(),
            },
        ]);
        for read in reads {
            result.extend([
                Premise::ReadableInput {
                    parameter: read.parameter(),
                    domain: read.access_domain(),
                },
                Premise::SeparateInputOutput {
                    input: read.parameter(),
                    output: parameter,
                },
                Premise::RepresentableAddress {
                    parameter: read.parameter(),
                    domain: read.address_domain(),
                    element_bytes: read.element_bytes(),
                    alignment: read.alignment(),
                },
            ]);
        }
        Ok(result)
    })
    .unwrap()
}

fn with_source<T>(p: &Complete, action: impl FnOnce(&[Premise], &mut Budget<'_>) -> T) -> T {
    let mut work = Work::new(WORK);
    let mut source = Budget::new(&mut work, STORAGE);
    source.reserve_storage(FLOOR).unwrap();
    source.charge_work(11).unwrap();
    let premises = original_premises(p, &mut source);
    let retained = premises.capacity() * size_of::<Premise>() + size_of::<Vec<Premise>>();
    source.reserve_storage(retained).unwrap();
    let floor = source.storage();
    let account = source.work_ledger_identity_v1();
    let result = action(&premises, &mut source);
    assert_eq!(source.storage(), floor);
    assert!(source.work_ledger_identity_v1() == account);
    drop(premises);
    source.release_storage(retained).unwrap();
    result
}

fn copy_rows<T: Copy>(rows: &[T], budget: &mut Budget<'_>) -> Vec<T> {
    budget.charge_work(size_of_val(rows)).unwrap();
    budget
        .reserve_storage(size_of_val(rows) + size_of::<Vec<T>>())
        .unwrap();
    let mut result = Vec::with_capacity(rows.len());
    budget
        .reserve_storage((result.capacity() - rows.len()) * size_of::<T>())
        .unwrap();
    result.extend_from_slice(rows);
    result
}

#[test]
fn conditional_final_both_targets_replay_real_value_sharing_and_r_f_rewrites() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for changed in [false, true] {
            with_complete(profile, changed, |p, target| {
                with_source(p, |premises, source| {
                    let inputs = p.inputs();
                    let i = p.checked.owner().canonical().canonical_bytes().as_ptr();
                    let f = p.f.output().canonical().canonical_bytes().as_ptr();
                    let rows = p.f.origins().as_ptr();
                    inspect(p, inputs, inputs.limits, premises, target, source).unwrap();
                    assert_eq!(i, p.checked.owner().canonical().canonical_bytes().as_ptr());
                    assert_eq!(f, p.f.output().canonical().canonical_bytes().as_ptr());
                    assert_eq!(rows, p.f.origins().as_ptr());
                    assert_eq!(p.k.proved_pairs(), usize::from(changed));
                    assert_eq!(!p.j.rows().is_empty(), changed);
                    assert_eq!(!p.p.selected_allocations().is_empty(), changed);
                    assert_eq!(
                        p.r.origins()
                            .iter()
                            .filter(|r| matches!(r, Refinement::CheckedAddSplit { .. }))
                            .count(),
                        usize::from(changed)
                    );
                    assert_eq!(
                        p.f.origins().iter().filter(|r| r.store.is_some()).count(),
                        usize::from(changed)
                    );
                    if changed {
                        assert_ne!(
                            p.j.output().canonical().canonical_bytes(),
                            p.k.output().canonical().canonical_bytes()
                        );
                        assert_ne!(
                            p.l.output().canonical().canonical_bytes(),
                            p.r.output().canonical().canonical_bytes()
                        );
                        assert_ne!(
                            p.r.output().canonical().canonical_bytes(),
                            p.f.output().canonical().canonical_bytes()
                        );
                    }
                    assert_eq!(premises.len(), 10);
                    assert!(!p.f.grants_authority());
                })
            });
        }
    }
}

#[test]
fn conditional_final_rejects_equal_byte_foreign_prefix_owners_and_wrong_limits() {
    with_complete(Profile::Gfx942, true, |p, target| {
        with_source(p, |premises, source| {
            for stage in 0..5 {
                scoped(target, |target| {
                    let mut inputs = p.inputs();
                    let p6 = &mut inputs.prefix.prefix.prefix;
                    let actual = match stage {
                        0 => p6.prefix.input,
                        1 => p6.prefix.intermediate,
                        2 => p6.prefix.stored,
                        3 => p6.prefix.output,
                        _ => p6.output,
                    };
                    let foreign = fixture::graph(actual.module(), target);
                    assert_eq!(
                        actual.canonical().canonical_bytes(),
                        foreign.canonical().canonical_bytes()
                    );
                    match stage {
                        0 => p6.prefix.input = &foreign,
                        1 => p6.prefix.intermediate = &foreign,
                        2 => p6.prefix.stored = &foreign,
                        3 => p6.prefix.output = &foreign,
                        _ => p6.output = &foreign,
                    }
                    assert!(matches!(
                        inspect(p, inputs, p.inputs().limits, premises, target, source),
                        Err(Error::Mismatch(_))
                    ));
                    Ok(())
                })
                .unwrap();
            }
            for forwarding in [false, true] {
                let mut inputs = p.inputs();
                if forwarding {
                    inputs.limits.forwarding.memory.effects += 1;
                } else {
                    inputs.limits.refinement.operations += 1;
                }
                assert!(matches!(
                    inspect(p, inputs, p.inputs().limits, premises, target, source),
                    Err(Error::LimitsMismatch)
                ));
            }
            scoped(target, |target| {
                let mut inputs = p.inputs();
                let record = copy_rows(inputs.prefix.prefix.prefix.prefix.policy5_record, target);
                inputs.prefix.prefix.prefix.prefix.policy5_record = &record;
                assert!(matches!(
                    inspect(p, inputs, inputs.limits, premises, target, source),
                    Err(Error::Mismatch(_))
                ));
                Ok(())
            })
            .unwrap();
        })
    });
}

#[test]
fn conditional_final_complete_history_refuses_every_foreign_tail_endpoint() {
    with_complete(Profile::Gfx950, true, |p, target| {
        with_source(p, |premises, source| {
            for stage in 0..7 {
                let mut inputs = p.inputs();
                // N has distinct target metadata and was never any actual tail.
                match stage {
                    0 => inputs.prefix.prefix.output = &p.n,
                    1 => inputs.prefix.output = &p.n,
                    2 => inputs.promoted = &p.n,
                    3 => inputs.preheaders = &p.n,
                    4 => inputs.licm = &p.n,
                    5 => inputs.refined = &p.n,
                    _ => inputs.output = &p.n,
                }
                assert!(matches!(
                    inspect(p, inputs, p.inputs().limits, premises, target, source),
                    Err(Error::History(_))
                ));
            }
        })
    });
}

#[test]
fn conditional_final_complete_history_refuses_omitted_and_reordered_rows() {
    with_complete(Profile::Gfx942, true, |p, target| {
        with_source(p, |premises, source| {
            scoped(target, |target| {
                let original = p.inputs();
                let mut j = copy_rows(
                    original.prefix.prefix.continuation.retained_operations,
                    target,
                );
                let mut k = copy_rows(original.prefix.continuation.occurrences.operations, target);
                let mut promotion = copy_rows(original.promotion_origins, target);
                let mut licm = copy_rows(original.licm_origins, target);
                let mut refinement = copy_rows(original.refinement_origins, target);
                let mut forwarding = copy_rows(original.forwarding_origins, target);
                j.swap(0, 1);
                k.swap(0, 1);
                promotion.swap(0, 1);
                licm.swap(0, 1);
                refinement.swap(0, 1);
                forwarding.swap(0, 1);
                for omit in [false, true] {
                    for stage in 0..6 {
                        let mut inputs = original;
                        let start = usize::from(omit);
                        match stage {
                            0 => {
                                inputs.prefix.prefix.continuation.retained_operations = &j[start..]
                            }
                            1 => inputs.prefix.continuation.occurrences.operations = &k[start..],
                            2 => inputs.promotion_origins = &promotion[start..],
                            3 => inputs.licm_origins = &licm[start..],
                            4 => inputs.refinement_origins = &refinement[start..],
                            _ => inputs.forwarding_origins = &forwarding[start..],
                        }
                        assert!(matches!(
                            inspect(p, inputs, original.limits, premises, target, source),
                            Err(Error::History(_))
                        ));
                    }
                }
                let mut inputs = original;
                inputs.selected_allocations = &[];
                assert!(matches!(
                    inspect(p, inputs, original.limits, premises, target, source),
                    Err(Error::History(_))
                ));
                let mut inputs = original;
                inputs.prefix.prefix.continuation.deletion_rows = &[];
                assert!(matches!(
                    inspect(p, inputs, original.limits, premises, target, source),
                    Err(Error::History(_))
                ));
                let mut inputs = original;
                let extra = [fe2o3_kernel_analysis::CanonicalKirLoopPreheaderV1 {
                    header: original.licm_origins[0].input.block,
                    preheader: original.licm_origins[0].input.block,
                }];
                inputs.preheader_rows = &extra;
                assert!(matches!(
                    inspect(p, inputs, original.limits, premises, target, source),
                    Err(Error::History(_))
                ));
                Ok(())
            })
            .unwrap();
        })
    });
}

#[test]
fn conditional_final_preserves_original_premises_despite_shared_computation() {
    with_complete(Profile::Gfx950, true, |p, target| {
        with_source(p, |premises, source| {
            assert_eq!(p.k.proved_pairs(), 1);
            for mutation in 0..4 {
                scoped(source, |source| {
                    let mut bad = copy_rows(premises, source);
                    match mutation {
                        0 => {
                            bad.truncate(7);
                        }
                        1 => bad.swap(4, 7),
                        2 => {
                            bad[5] = Premise::SeparateInputOutput {
                                input: 1,
                                output: 1,
                            }
                        }
                        _ => bad[4] = Premise::ReadableInput {
                            parameter: 1,
                            domain:
                                fe2o3_kernel_ir::ConditionalTotalViewAddressDomainV1::GlobalLaunch,
                        },
                    }
                    assert!(matches!(
                        inspect(p, p.inputs(), p.inputs().limits, &bad, target, source),
                        Err(Error::Prefix(_))
                    ));
                    Ok(())
                })
                .unwrap();
            }
            inspect(p, p.inputs(), p.inputs().limits, premises, target, source).unwrap();
        })
    });
}

#[test]
fn conditional_final_memory_mapping_is_complete_without_assuming_read_order_or_ssa_ids() {
    with_complete(Profile::Gfx942, true, |p, target| {
        with_source(p, |_, source| {
            scoped(source, |source| {
                scoped(target, |target| {
                    let history = check_canonical_refined_forwarding_history_v1(p.inputs(), target)
                        .map_err(Error::History)?;
                    target.reserve_storage(history.storage().retained_storage())?;
                    let before = facts(p.checked.owner(), &KernelId::new("entry"), target)?;
                    let after = facts(history.output(), &KernelId::new("entry"), target)?;
                    let input_store = coordinate(&before, before.store_location(), source)?;
                    let output_store = coordinate(&after, after.store_location(), source)?;
                    assert_ne!(input_store, output_store);
                    assert_eq!(follow(&history, input_store, source)?, output_store);
                    for mutation in 0..4 {
                        scoped(source, |source| {
                            let mut original = occurrences::reads(&before, source)?;
                            let mut output = occurrences::reads(&after, source)?;
                            let second_input = coordinate(&before, original[1].location(), source)?;
                            let second_output = coordinate(&after, output[1].location(), source)?;
                            assert_ne!(second_input, second_output);
                            assert_eq!(follow(&history, second_input, source)?, second_output);
                            match mutation {
                                0 => output.swap(0, 1),
                                1 => output[0] = output[1],
                                2 => {
                                    output.pop();
                                }
                                _ => original[0] = original[1],
                            }
                            let result = coverage(
                                &p.checked,
                                &history,
                                &before,
                                &after,
                                &mut original,
                                &output,
                                source,
                            );
                            assert_eq!(result.is_ok(), mutation == 0);
                            Ok(())
                        })?;
                    }
                    let foreign = fixture::graph(history.output().module(), target);
                    let foreign_facts = facts(&foreign, &KernelId::new("entry"), target)?;
                    let mut original = occurrences::reads(&before, source)?;
                    let output = occurrences::reads(&after, source)?;
                    assert!(matches!(
                        coverage(
                            &p.checked,
                            &history,
                            &before,
                            &foreign_facts,
                            &mut original,
                            &output,
                            source
                        ),
                        Err(Error::Mismatch(_))
                    ));
                    Ok(())
                })
            })
            .unwrap();
        })
    });
}

#[test]
fn conditional_final_private_load_entry_remains_outside_conditional_coverage_domain() {
    use fe2o3_kernel_ir::{
        AccessMode, AddressSpace, MemoryAccess, Operation, OperationKind as Kind, ScalarType, Type,
        ValueDef, ValueId,
    };
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    scoped(&mut budget, |budget| {
        let mut module = fixture::module(false);
        let block = &mut module.functions[0].body.as_mut().unwrap().blocks[1];
        let ty = Type::Scalar(ScalarType::U32);
        let insert = block.operations.len() - 1;
        block.operations.splice(
            insert..insert,
            [
                Operation::effect_free(
                    ValueDef::new(
                        ValueId(60),
                        Type::pointer(ty.clone(), AddressSpace::Private, AccessMode::ReadWrite),
                    ),
                    Kind::Alloca {
                        element: ty.clone(),
                        count: None,
                        address_space: AddressSpace::Private,
                        alignment: 4,
                    },
                ),
                Operation::new(
                    vec![],
                    Kind::Store {
                        pointer: ValueId(60),
                        value: ValueId(51),
                        access: MemoryAccess::new(AddressSpace::Private, 4),
                    },
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(61), ty),
                    Kind::Load {
                        pointer: ValueId(60),
                        access: MemoryAccess::new(AddressSpace::Private, 4),
                    },
                ),
            ],
        );
        let graph = fixture::graph(&module, budget);
        assert!(matches!(
            facts(&graph, &KernelId::new("entry"), budget),
            Err(super::super::Error::Mismatch(_))
        ));
        Ok(())
    })
    .unwrap();
}
