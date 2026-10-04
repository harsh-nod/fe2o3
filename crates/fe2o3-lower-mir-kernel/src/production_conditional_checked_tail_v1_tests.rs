//! Actual canonical/policy6/J components; no manufactured source request or proof.
use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    Function as KirFunction, MemoryAccess, Module, Operation, OperationKind as Kind, ScalarType,
    Signature, Terminator, Type, ValueDef, ValueId,
};
use fe2o3_kernel_opt::prepare_owned_redundant_store_continuation_v1;

// Reuse the existing immutable component fixture without editing its owner's files.
#[path = "production_conditional_checked_output_fixture_v1_tests.rs"]
#[allow(
    dead_code,
    reason = "reuse the immutable shared component fixture without its with_prefix wrapper"
)]
mod fixture;
use fixture::{FLOOR, STORAGE, WORK};

fn private_function() -> KirFunction {
    let mut block = BasicBlock::new(BlockId(83));
    let store = || {
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(10),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        )
    };
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(
                ValueId(10),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            ),
            Kind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        store(),
        store(),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    KirFunction::internal_helper(
        "private_tail",
        Signature::new(vec![Type::Scalar(ScalarType::U32)], vec![]),
        vec![ValueId(0)],
        vec![block],
    )
}

fn with_tail(
    profile: Profile,
    nonzero: bool,
    run: impl FnOnce(&Graph, &Prefix, &Tail, &mut Budget<'_>),
) {
    let mut work = Work::new(WORK);
    let mut target = Budget::new(&mut work, STORAGE);
    target.reserve_storage(FLOOR).unwrap();
    let mut module = fixture::module(false);
    if nonzero {
        module.functions.push(private_function());
    }
    let n = fixture::graph(&module, &mut target);
    let binding = dialect_amdgcn::bind_production_target_v1(n.module(), profile).unwrap();
    let b = fixture::graph(binding.module(), &mut target);
    drop(binding);
    drop(module);
    let prefix = fixture::prefix(&b, &mut target);
    let tail = prepare_owned_redundant_store_continuation_v1(prefix.owner(), &mut target).unwrap();
    target.reserve_storage(tail.retained_storage()).unwrap();
    assert_eq!(!tail.rows().is_empty(), nonzero);
    let floor = target.storage();
    let account = target.work_ledger_identity_v1();
    run(&b, &prefix, &tail, &mut target);
    assert_eq!(target.storage(), floor);
    assert!(target.work_ledger_identity_v1() == account);
}

fn projected(after: &Facts<'_>, reads: &[Read]) -> Vec<Premise> {
    let parameter = after.output_parameter_index();
    let mut result = vec![
        Premise::D1Launch,
        Premise::OutputWithinGlobalX { parameter },
        Premise::WritableOutput { parameter },
        Premise::RepresentableAddress {
            parameter,
            domain: after.address_domain(),
            element_bytes: after.element_bytes(),
            alignment: after.alignment(),
        },
    ];
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
    result
}

fn premises(prefix: &Prefix, budget: &mut Budget<'_>) -> Vec<Premise> {
    scoped(budget, |budget| {
        let input = facts(prefix.owner(), &KernelId::new("entry"), budget)?;
        let reads = occurrences::reads(&input, budget)?;
        Ok(projected(&input, &reads))
    })
    .unwrap()
}

fn inspect(
    b: &Graph,
    prefix: &Prefix,
    tail: &Tail,
    premises: &[Premise],
    target: &mut Budget<'_>,
    source: &mut Budget<'_>,
) -> Result<()> {
    scoped(source, |source| {
        scoped(target, |target| {
            require_target_floor(b, prefix, tail, target)?;
            check_tail(
                prefix,
                tail,
                &KernelId::new("entry"),
                premises,
                target,
                source,
            )
        })
    })
}

#[test]
fn both_targets_replay_actual_noop_and_nonzero_j_without_replacing_i() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for nonzero in [false, true] {
            with_tail(profile, nonzero, |b, prefix, tail, target| {
                let expected = premises(prefix, target);
                let before_i = prefix.owner().canonical().canonical_bytes().as_ptr();
                let before_j = tail.output().canonical().canonical_bytes().as_ptr();
                let before_rows = tail.retained_operations().as_ptr();
                let mut work = Work::new(WORK);
                let mut source = Budget::new(&mut work, STORAGE);
                source.reserve_storage(FLOOR).unwrap();
                let account = source.work_ledger_identity_v1();
                inspect(b, prefix, tail, &expected, target, &mut source).unwrap();
                assert_eq!(source.storage(), FLOOR);
                assert!(source.work_ledger_identity_v1() == account);
                assert_ne!(before_i, before_j);
                assert_eq!(
                    prefix.owner().canonical().canonical_bytes().as_ptr(),
                    before_i
                );
                assert_eq!(
                    tail.output().canonical().canonical_bytes().as_ptr(),
                    before_j
                );
                assert_eq!(tail.retained_operations().as_ptr(), before_rows);
                assert_eq!(
                    prefix.owner().canonical().canonical_bytes()
                        != tail.output().canonical().canonical_bytes(),
                    nonzero
                );
                assert!(!tail.grants_authority());
            });
        }
    }
}

#[test]
fn foreign_i_and_foreign_owned_j_are_rejected_by_full_history_replay() {
    with_tail(Profile::Gfx942, true, |b, prefix, tail, target| {
        scoped(target, |target| {
            let binding =
                dialect_amdgcn::bind_production_target_v1(&fixture::module(true), Profile::Gfx942)
                    .unwrap();
            let foreign_b = fixture::graph(binding.module(), target);
            let foreign = fixture::prefix(&foreign_b, target);
            let foreign_j =
                prepare_owned_redundant_store_continuation_v1(foreign.owner(), target).unwrap();
            target.reserve_storage(foreign_j.retained_storage())?;
            let expected = premises(prefix, target);
            let mut work = Work::new(WORK);
            let mut source = Budget::new(&mut work, STORAGE);
            source.reserve_storage(FLOOR)?;
            assert!(matches!(
                inspect(b, prefix, &foreign_j, &expected, target, &mut source),
                Err(Error::Tail(_))
            ));
            assert!(matches!(
                inspect(&foreign_b, &foreign, tail, &expected, target, &mut source),
                Err(Error::Tail(_))
            ));
            assert_eq!(source.storage(), FLOOR);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn equal_bytes_foreign_views_and_read_substitution_cannot_replace_actual_i_j_facts() {
    with_tail(Profile::Gfx950, true, |_, prefix, tail, target| {
        scoped(target, |target| {
            let duplicate_i = fixture::graph(prefix.owner().module(), target);
            let duplicate_j = fixture::graph(tail.output().module(), target);
            let before = facts(prefix.owner(), &KernelId::new("entry"), target)?;
            let after = facts(tail.output(), &KernelId::new("entry"), target)?;
            let other_i = facts(&duplicate_i, &KernelId::new("entry"), target)?;
            let other_j = facts(&duplicate_j, &KernelId::new("entry"), target)?;
            let original = occurrences::reads(&before, target)?;
            let mut output = occurrences::reads(&after, target)?;
            coverage(prefix, tail, &before, &after, &original, &output, target).unwrap();
            assert!(coverage(prefix, tail, &other_i, &after, &original, &output, target).is_err());
            assert!(coverage(prefix, tail, &before, &other_j, &original, &output, target).is_err());
            output.swap(0, 1);
            assert!(coverage(prefix, tail, &before, &after, &original, &output, target).is_err());
            output[0] = output[1];
            assert!(coverage(prefix, tail, &before, &after, &original, &output, target).is_err());
            output.pop();
            assert!(coverage(prefix, tail, &before, &after, &original, &output, target).is_err());
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn complete_history_and_selected_memory_maps_reject_substitutions() {
    with_tail(Profile::Gfx942, true, |_, prefix, tail, target| {
        scoped(target, |target| {
            let before = facts(prefix.owner(), &KernelId::new("entry"), target)?;
            let after = facts(tail.output(), &KernelId::new("entry"), target)?;
            let input = site(occurrences::coordinate(
                &before,
                before.store_location(),
                target,
            )?)?;
            let output = site(occurrences::coordinate(
                &after,
                after.store_location(),
                target,
            )?)?;
            assert_ne!(input.block.block, before.store_location().block.0);
            require_mapping(input, output, tail.retained_operations(), target).unwrap();
            let row_index = tail
                .retained_operations()
                .iter()
                .position(|r| r.input == input)
                .unwrap();
            for axis in 0..4 {
                let mut rows = tail.retained_operations().to_vec();
                match axis {
                    0 => {
                        rows.remove(row_index);
                    }
                    1 => rows.push(rows[row_index]),
                    2 => rows[row_index].input.operation += 1,
                    _ => rows[row_index].output.block.block = before.store_location().block.0,
                }
                assert!(require_mapping(input, output, &rows, target).is_err());
                assert!(
                    fe2o3_kernel_analysis::check_canonical_kir_redundant_store_v1(
                        prefix.owner(),
                        tail.output(),
                        tail.rows(),
                        &rows,
                        target,
                    )
                    .is_err()
                );
            }
            let mut deletions = tail.rows().to_vec();
            deletions[0].removed = input;
            assert!(
                fe2o3_kernel_analysis::check_canonical_kir_redundant_store_v1(
                    prefix.owner(),
                    tail.output(),
                    &deletions,
                    tail.retained_operations(),
                    target,
                )
                .is_err()
            );
            // Whole-module replay still checks nonselected functions.
            let mut rows = tail.retained_operations().to_vec();
            let other = rows
                .iter_mut()
                .find(|r| r.input.block.function.0 != input.block.function.0)
                .unwrap();
            other.output.operation += 1;
            assert!(
                fe2o3_kernel_analysis::check_canonical_kir_redundant_store_v1(
                    prefix.owner(),
                    tail.output(),
                    tail.rows(),
                    &rows,
                    target,
                )
                .is_err()
            );
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn j_premises_must_equal_complete_component_projection() {
    with_tail(Profile::Gfx950, false, |b, prefix, tail, target| {
        let expected = premises(prefix, target);
        let mut work = Work::new(WORK);
        let mut source = Budget::new(&mut work, STORAGE);
        source.reserve_storage(FLOOR).unwrap();
        for axis in 0..expected.len() {
            let mut changed = expected.clone();
            changed[axis] = match changed[axis] {
                Premise::ReadableInput { parameter, .. } => Premise::ReadableInput {
                    parameter,
                    domain: fe2o3_kernel_ir::ConditionalTotalViewAddressDomainV1::GlobalLaunch,
                },
                Premise::RepresentableAddress {
                    parameter,
                    domain,
                    element_bytes,
                    ..
                } => Premise::RepresentableAddress {
                    parameter,
                    domain,
                    element_bytes,
                    alignment: 1,
                },
                _ => Premise::WritableOutput { parameter: 77 },
            };
            assert!(
                inspect(b, prefix, tail, &changed, target, &mut source).is_err(),
                "axis {axis}"
            );
            assert_eq!(source.storage(), FLOOR);
        }
        for length in [0, 3, expected.len() - 1] {
            assert!(inspect(b, prefix, tail, &expected[..length], target, &mut source).is_err());
        }
        let mut extra = expected.clone();
        extra.push(Premise::D1Launch);
        assert!(inspect(b, prefix, tail, &extra, target, &mut source).is_err());
    });
}

#[test]
fn additional_private_memory_in_selected_conditional_function_stays_unsupported() {
    let mut module: Module = fixture::module(false);
    let helper = private_function();
    let allocation = helper.body.unwrap().blocks.remove(0).operations.remove(0);
    // Select a fresh local id; the unsupported operation, not an invalid SSA
    // collision, is what the existing conditional analyzer must refuse.
    let mut allocation = allocation;
    allocation.results[0].id = ValueId(100);
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(allocation);
    let mut work = Work::new(WORK);
    let mut target = Budget::new(&mut work, STORAGE);
    let input = fixture::graph(&module, &mut target);
    assert!(facts(&input, &KernelId::new("entry"), &mut target).is_err());
}

#[path = "production_conditional_checked_tail_resources_v1_tests.rs"]
mod resources;
