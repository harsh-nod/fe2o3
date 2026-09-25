//! Component positives use actual target binding and sealed policy6, not proof receipts.
use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, KernelId};
use fe2o3_pliron::ProductionConditionalRuntimePremiseV1 as Premise;

#[path = "production_conditional_checked_output_fixture_v1_tests.rs"]
mod fixture;
use fixture::{FLOOR, STORAGE, WORK, with_prefix};

fn inspect(n: &Graph, p6: &Prefix, budget: &mut Budget<'_>) -> Result<()> {
    scoped(budget, |budget| {
        budget.reserve_storage(SCRATCH)?;
        let before = facts(n, &KernelId::new("entry"), budget)?;
        let after = facts(p6.owner(), &KernelId::new("entry"), budget)?;
        let original = occurrences::reads(&before, budget)?;
        let final_reads = occurrences::reads(&after, budget)?;
        occurrences::coverage(&before, &after, p6, &original, &final_reads, budget)
    })
}

#[test]
fn both_targets_check_actual_prefix_and_renumbered_memory_occurrences() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_prefix(profile, |n, b, p6, target| {
            scoped(target, |target| {
                let (_, storage) =
                    dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                        n, b, profile, target,
                    )
                    .unwrap();
                target.reserve_storage(storage.retained_storage())?;
                p6.replay(b, target).map_err(Error::Prefix)
            })
            .unwrap();
            let mut work = Work::new(WORK);
            let mut source = Budget::new(&mut work, STORAGE);
            source.reserve_storage(FLOOR).unwrap();
            assert!(source.work_ledger_identity_v1() != target.work_ledger_identity_v1());
            inspect(n, p6, &mut source).unwrap();
            assert_eq!(source.storage(), FLOOR);
            assert_ne!(
                n.canonical().canonical_bytes(),
                p6.owner().canonical().canonical_bytes()
            );
            assert!(!p6.grants_authority());
        });
    }
}

#[test]
fn substituted_source_target_and_prefix_are_refused_by_existing_relations() {
    with_prefix(Profile::Gfx942, |n, b, p6, target| {
        scoped(target, |target| {
            let foreign = fixture::graph(&fixture::module(true), target);
            assert!(
                dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                    &foreign,
                    b,
                    Profile::Gfx942,
                    target
                )
                .is_err()
            );
            assert!(
                dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                    n,
                    b,
                    Profile::Gfx950,
                    target
                )
                .is_err()
            );
            assert!(p6.replay(&foreign, target).is_err());
            let binding =
                dialect_amdgcn::bind_production_target_v1(foreign.module(), Profile::Gfx942)
                    .unwrap();
            let foreign_b = fixture::graph(binding.module(), target);
            let foreign_p6 = fixture::prefix(&foreign_b, target);
            assert!(foreign_p6.replay(b, target).is_err());
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn independently_valid_foreign_i_and_reordered_reads_cannot_replace_actual_occurrences() {
    with_prefix(Profile::Gfx942, |n, _, p6, target| {
        scoped(target, |target| {
            let copy = fixture::graph(p6.owner().module(), target);
            let mut work = Work::new(WORK);
            let mut source = Budget::new(&mut work, STORAGE);
            source.reserve_storage(FLOOR).unwrap();
            scoped(&mut source, |source| {
                let before = facts(n, &KernelId::new("entry"), source)?;
                let after = facts(p6.owner(), &KernelId::new("entry"), source)?;
                let foreign = facts(&copy, &KernelId::new("entry"), source)?;
                let original = occurrences::reads(&before, source)?;
                let mut final_reads = occurrences::reads(&after, source)?;
                assert_eq!(original.len(), 2);
                assert!(
                    occurrences::coverage(&before, &foreign, p6, &original, &final_reads, source)
                        .is_err()
                );
                final_reads.swap(0, 1);
                assert!(
                    occurrences::coverage(&before, &after, p6, &original, &final_reads, source)
                        .is_err()
                );
                final_reads.pop();
                assert!(
                    occurrences::coverage(&before, &after, p6, &original, &final_reads, source)
                        .is_err()
                );
                Ok(())
            })
            .unwrap();
            assert_eq!(source.storage(), FLOOR);
            Ok(())
        })
        .unwrap();
    });
}

fn projected(
    after: &Facts<'_>,
    reads: &[fe2o3_kernel_ir::ConditionalTotalViewReadV1],
) -> Vec<Premise> {
    let parameter = after.output_parameter_index();
    let mut premises = vec![
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
        premises.extend([
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
    premises
}

#[test]
fn projected_premises_cannot_weaken_read_domains_or_substitute_roots_and_layouts() {
    with_prefix(Profile::Gfx950, |_, _, p6, _| {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        scoped(&mut budget, |budget| {
            let after = facts(p6.owner(), &KernelId::new("entry"), budget)?;
            let reads = occurrences::reads(&after, budget)?;
            let expected = projected(&after, &reads);
            occurrences::premises(&after, &reads, &expected, budget).unwrap();
            for axis in 0..expected.len() {
                let mut changed = expected.clone();
                changed[axis] = match changed[axis] {
                    Premise::D1Launch => Premise::WritableOutput { parameter: 77 },
                    Premise::OutputWithinGlobalX { .. } => {
                        Premise::OutputWithinGlobalX { parameter: 77 }
                    }
                    Premise::WritableOutput { .. } => Premise::WritableOutput { parameter: 77 },
                    Premise::ReadableInput { parameter, .. } => Premise::ReadableInput {
                        parameter,
                        domain: fe2o3_kernel_ir::ConditionalTotalViewAddressDomainV1::GlobalLaunch,
                    },
                    Premise::SeparateInputOutput { input, .. } => {
                        Premise::SeparateInputOutput { input, output: 77 }
                    }
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
                };
                assert!(
                    occurrences::premises(&after, &reads, &changed, budget).is_err(),
                    "axis {axis}"
                );
            }
            assert!(
                occurrences::premises(&after, &reads, &expected[..expected.len() - 1], budget)
                    .is_err()
            );
            Ok(())
        })
        .unwrap();
    });
}

#[path = "production_conditional_checked_output_resources_v1_tests.rs"]
mod resources;
