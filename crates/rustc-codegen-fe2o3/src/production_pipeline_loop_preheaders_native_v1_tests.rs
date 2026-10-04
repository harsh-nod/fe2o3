//! Genuine constructed semantic-source owners, not authenticated rustc capture.
use super::*;
use crate::production_pipeline::checked_output_policy7_v1::CheckedOutputPolicy7StageErrorV1 as P7Error;
use crate::production_ranked_projection_v1::{
    with_backend_loop_preheaders_direct_prefix_v1, with_backend_policy8_erased_prefix_v1,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn with_prefix(
    erased: bool,
    profile: Profile,
    mutation: bool,
    next: impl FnOnce(Prefix6, &mut Budget<'_>),
) {
    if erased {
        assert!(
            !mutation,
            "UnitLocal mutation is not claimed by this fixture"
        );
        with_backend_policy8_erased_prefix_v1(profile, false, |owner, _, budget| {
            next(Prefix6::Erased(owner), budget)
        });
    } else {
        with_backend_loop_preheaders_direct_prefix_v1(profile, mutation, |owner, _, budget| {
            next(Prefix6::Direct(owner), budget)
        });
    }
}
fn assert_shape(owner: &Preheaders, mutation: bool) {
    let (rows, origins, incoming, parameters, kernels) = match owner {
        Preheaders::Direct(v) => (
            v.continuation().preheaders(),
            v.origins(),
            v.incoming_origins(),
            v.parameter_origins(),
            v.kernels(),
        ),
        Preheaders::Erased(v) => (
            v.continuation().preheaders(),
            v.origins(),
            v.incoming_origins(),
            v.parameter_origins(),
            v.kernels(),
        ),
    };
    assert_eq!(rows.len(), usize::from(mutation));
    assert_eq!(origins.len(), usize::from(mutation));
    assert_eq!(kernels.len(), 2);
    if mutation {
        let origin = &origins[0];
        assert_eq!(origin.incoming().len(), 2);
        assert!(
            !origin.parameters().is_empty(),
            "genuine source SSA must need typed forwarding"
        );
        assert_eq!(incoming.len(), 2);
        assert_ne!(incoming[0].input(), incoming[1].input());
        assert_eq!(parameters.len(), origin.parameters().len());
        assert_eq!(rows[0].header, origin.input_header());
        assert_eq!(rows[0].preheader, origin.output_preheader());
        for edge in incoming {
            assert_eq!(edge.input(), edge.output());
        }
        let header = origin.input_header();
        let preheader = origin.output_preheader();
        let old = &owner.promoted_input().module().functions[header.function.0 as usize]
            .body
            .as_ref()
            .unwrap()
            .blocks[header.block as usize];
        let new = &owner.output().module().functions[preheader.function.0 as usize]
            .body
            .as_ref()
            .unwrap()
            .blocks[preheader.block as usize];
        assert_eq!(old.parameters.len(), new.parameters.len());
        assert_eq!(parameters.len(), new.parameters.len());
        for (index, (old, new)) in old.parameters.iter().zip(&new.parameters).enumerate() {
            assert_eq!(old.ty, new.ty);
            assert_ne!(old.id, new.id);
            assert_eq!(
                parameters[index].input_header_parameter(),
                fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::BlockArgument {
                    block: header,
                    argument: index as u32
                }
            );
            assert_eq!(
                parameters[index].output_preheader_parameter(),
                fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::BlockArgument {
                    block: preheader,
                    argument: index as u32
                }
            );
        }
        match new.terminator.as_ref().unwrap() {
            fe2o3_kernel_ir::Terminator::Branch { target, arguments } => {
                assert_eq!(*target, old.id);
                assert_eq!(
                    *arguments,
                    new.parameters
                        .iter()
                        .map(|value| value.id)
                        .collect::<Vec<_>>()
                );
            }
            _ => panic!("the synthetic preheader must only forward ordered parameters"),
        }
        assert_ne!(
            owner.promoted_input().canonical().canonical_bytes(),
            owner.output().canonical().canonical_bytes()
        );
    } else {
        assert!(incoming.is_empty() && parameters.is_empty());
        assert_eq!(
            owner.promoted_input().canonical().canonical_bytes(),
            owner.output().canonical().canonical_bytes()
        );
        assert!(!std::ptr::eq(owner.promoted_input(), owner.output()));
    }
    let before = owner.promoted_input().module();
    let after = owner.output().module();
    assert_eq!(before.kernels, after.kernels);
    assert_eq!(before.functions.len(), after.functions.len());
    let mut appended = 0;
    for (old, new) in before.functions.iter().zip(&after.functions) {
        let (old, new) = match (&old.body, &new.body) {
            (Some(old), Some(new)) => (old, new),
            (None, None) => continue,
            _ => panic!("function body presence changed"),
        };
        assert!(new.blocks.len() >= old.blocks.len());
        for (old, new) in old.blocks.iter().zip(&new.blocks) {
            assert_eq!(old.operations, new.operations);
        }
        for block in &new.blocks[old.blocks.len()..] {
            assert!(block.operations.is_empty());
        }
        appended += new.blocks.len() - old.blocks.len();
    }
    assert_eq!(appended, usize::from(mutation));
}

#[test]
fn loop_preheaders_native_direct_mutation_emits_only_actual_final_graph_both_profiles() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_prefix(false, profile, true, |prefix, budget| {
            let original = match &prefix {
                Prefix6::Direct(v) => v
                    .source_semantic_kir()
                    .pre_ranked_executable()
                    .unwrap()
                    .canonical()
                    .canonical_bytes()
                    .as_ptr(),
                Prefix6::Erased(_) => panic!("direct fixture"),
            };
            let sibling = vec![0xa5_u8; 53];
            budget.reserve_storage(sibling.capacity()).unwrap();
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            let (value, storage) = prepare(prefix, profile, budget).unwrap();
            assert_eq!(budget.storage(), floor);
            budget.reserve_storage(storage.retained_storage()).unwrap();
            assert_eq!(value.retained_storage_floor_v1(), budget.storage());
            assert_eq!(
                value
                    .original()
                    .unwrap()
                    .canonical()
                    .canonical_bytes()
                    .as_ptr(),
                original
            );
            assert_shape(&value.owner, true);
            assert_eq!(value.prefix_execution.policy_version(), 7);
            assert!(!value.grants_artifact_or_launch_authority());
            assert!(!value.llvm_ir().contains(".fe2o3.kd.v1"));
            value.verify_equivalence(budget).unwrap();
            let (expected, bytes) = lower_native(value.output(), profile, budget).unwrap();
            budget.reserve_storage(bytes).unwrap();
            assert_eq!(expected, value.llvm_ir());
            drop(expected);
            budget.release_storage(bytes).unwrap();
            for prior in [value.promoted_input(), value.historical_p8_output()] {
                let (historical, bytes) = lower_native(prior, profile, budget).unwrap();
                budget.reserve_storage(bytes).unwrap();
                assert_ne!(historical, value.llvm_ir());
                assert!(matches!(
                    check_native_text(value.output(), profile, &historical, budget),
                    Err(ProductionPipelineError::LoopPreheadersNativeStage(
                        LoopPreheadersNativeStageErrorV1::Mismatch(
                            "exact loop-preheaders native LLVM"
                        )
                    ))
                ));
                drop(historical);
                budget.release_storage(bytes).unwrap();
            }
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert!(sibling.iter().all(|byte| *byte == 0xa5));
            drop(value);
            budget.release_storage(storage.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
            budget.release_storage(sibling.capacity()).unwrap();
        });
    }
}

#[test]
fn loop_preheaders_native_direct_and_unit_local_noop_are_exact_both_profiles() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_prefix(erased, profile, false, |prefix, budget| {
                let floor = budget.storage();
                let (value, storage) = prepare(prefix, profile, budget).unwrap();
                assert_eq!(budget.storage(), floor);
                budget.reserve_storage(storage.retained_storage()).unwrap();
                assert_shape(&value.owner, false);
                assert_eq!(matches!(value.owner, Preheaders::Erased(_)), erased);
                assert!(!value.grants_artifact_or_launch_authority());
                assert!(!value.llvm_ir().contains(".fe2o3.kd.v1"));
                let (prior, bytes) = lower_native(value.promoted_input(), profile, budget).unwrap();
                budget.reserve_storage(bytes).unwrap();
                assert_eq!(prior, value.llvm_ir());
                value.verify_equivalence(budget).unwrap();
                drop(prior);
                budget.release_storage(bytes).unwrap();
                drop(value);
                budget.release_storage(storage.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}

#[test]
fn loop_preheaders_native_preparation_is_deterministic_for_every_component_mode() {
    for (erased, mutation) in [(false, true), (false, false), (true, false)] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            let mut previous = None;
            for _ in 0..2 {
                with_prefix(erased, profile, mutation, |prefix, budget| {
                    let (value, receipt) = prepare(prefix, profile, budget).unwrap();
                    let observation = (
                        value.output().canonical().canonical_bytes().to_vec(),
                        value.llvm_ir().to_owned(),
                        value.prefix_execution.canonical_bytes().to_vec(),
                        receipt.retained_storage(),
                        budget.work(),
                        budget.peak_storage(),
                    );
                    if let Some(previous) = &previous {
                        assert_eq!(previous, &observation);
                    }
                    previous = Some(observation);
                });
            }
        }
    }
}

#[test]
fn loop_preheaders_native_replay_rejects_equal_length_text_wrong_profile_and_missing_floor() {
    for (erased, mutation) in [(false, true), (false, false), (true, false)] {
        with_prefix(erased, Profile::Gfx942, mutation, |prefix, budget| {
            let (mut value, storage) = prepare(prefix, Profile::Gfx942, budget).unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let floor = budget.storage();
            let capacity = value.llvm.capacity();
            let original = value.llvm.remove(0);
            value
                .llvm
                .insert(0, if original == ';' { ' ' } else { ';' });
            assert_eq!(value.llvm.capacity(), capacity);
            assert!(matches!(
                value.verify_equivalence(budget),
                Err(ProductionPipelineError::LoopPreheadersNativeStage(
                    LoopPreheadersNativeStageErrorV1::Mismatch("exact loop-preheaders native LLVM")
                ))
            ));
            value.llvm.remove(0);
            value.llvm.insert(0, original);
            value.verify_equivalence(budget).unwrap();
            assert_eq!(budget.storage(), floor);
            match check_native_text(value.output(), Profile::Gfx950, value.llvm_ir(), budget) {
                Err(ProductionPipelineError::TargetLowering(error)) => assert!(
                    error.contains(dialect_amdgcn::LoweringDiagnosticCode::UnsupportedCapability)
                ),
                _ => panic!("wrong exact target must fail native target preflight"),
            }
            assert_eq!(budget.storage(), floor);
            budget.release_storage(1).unwrap();
            let work = budget.work();
            assert!(matches!(
                value.verify_equivalence(budget),
                Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                    P7Error::Resource(Resource::Accounting)
                ))
            ));
            assert_eq!((budget.storage(), budget.work()), (floor - 1, work));
            budget.reserve_storage(1).unwrap();
            drop(value);
            budget.release_storage(storage.retained_storage()).unwrap();
        });
    }
}

#[test]
fn loop_preheaders_native_replay_requires_the_genuine_retained_p7_execution_witness() {
    with_prefix(false, Profile::Gfx942, true, |prefix, budget| {
        let (mut first, first_storage) = prepare(prefix, Profile::Gfx942, budget).unwrap();
        budget
            .reserve_storage(first_storage.retained_storage())
            .unwrap();
        let floor = budget.storage();
        with_prefix(false, Profile::Gfx950, true, |prefix, other| {
            let (mut second, second_storage) = prepare(prefix, Profile::Gfx950, other).unwrap();
            other
                .reserve_storage(second_storage.retained_storage())
                .unwrap();
            assert_eq!(
                first.prefix_execution.retained_storage(),
                second.prefix_execution.retained_storage()
            );
            assert_ne!(
                first.prefix_execution.canonical_bytes(),
                second.prefix_execution.canonical_bytes()
            );
            std::mem::swap(&mut first.prefix_execution, &mut second.prefix_execution);
            assert!(matches!(
                first.verify_equivalence(budget),
                Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                    P7Error::Execution("Policy7 complete execution transcript")
                ))
            ));
            assert_eq!(budget.storage(), floor);
            std::mem::swap(&mut first.prefix_execution, &mut second.prefix_execution);
            first.verify_equivalence(budget).unwrap();
            second.verify_equivalence(other).unwrap();
            drop(second);
            other
                .release_storage(second_storage.retained_storage())
                .unwrap();
        });
        drop(first);
        budget
            .release_storage(first_storage.retained_storage())
            .unwrap();
    });
}

#[test]
fn loop_preheaders_native_panic_drops_real_completed_owner_before_sibling_floor_restore() {
    use std::{cell::Cell, rc::Rc};
    struct AfterOwner(Rc<Cell<bool>>);
    impl Drop for AfterOwner {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    with_prefix(false, Profile::Gfx942, true, |prefix, budget| {
        let sibling = vec![0x53_u8; 43];
        budget.reserve_storage(sibling.capacity()).unwrap();
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let dropped = Rc::new(Cell::new(false));
        let result: Result<()> = scoped(floor, budget, |budget| {
            let (value, receipt) = prepare(prefix, Profile::Gfx942, budget)?;
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let _live = (value, AfterOwner(dropped.clone()));
            std::panic::panic_any(71_u32)
        });
        assert!(matches!(
            result,
            Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                P7Error::Panicked
            ))
        ));
        assert!(dropped.get());
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert!(sibling.iter().all(|byte| *byte == 0x53));
        budget.release_storage(sibling.capacity()).unwrap();
    });
}

#[path = "production_pipeline_loop_preheaders_native_resources_v1_tests.rs"]
mod resources;
