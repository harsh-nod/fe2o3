//! Inert owner/account controls, not conditional Request or proof success.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_kernel_opt::{
    CanonicalRefinedForwardingHistoryLimitsV1 as Limits, RefinedForwardingHistoryRoleV1 as Role,
};

#[test]
fn conditional_bridge_actual_chain_history_and_text_both_targets() {
    use crate::production_ranked_projection_v1::with_backend_checked_output_policy6_owned_v1;
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_backend_checked_output_policy6_owned_v1(profile, |owner, budget| {
            let floor = budget.storage();
            let account = budget.work_ledger_identity_v1();
            let limits = Limits {
                refinement: fe2o3_kernel_analysis::CanonicalKirLoopLimitsV1::default(),
                forwarding:
                    fe2o3_kernel_analysis::CanonicalKirCrossBlockForwardingLimitsV1::default(),
            };
            let chain =
                FinalChain::prepare(owner.bound(), owner.checked_output(), limits, budget).unwrap();
            chain.check_owned(budget).unwrap();
            let wire = encode_refined_forwarding_history_v1(
                chain.inputs(owner.bound(), owner.checked_output()),
                budget,
            )
            .unwrap();
            budget
                .reserve_storage(wire.storage().retained_storage())
                .unwrap();
            let frame = read_refined_forwarding_history_v1(wire.canonical_bytes(), budget).unwrap();
            budget
                .reserve_storage(frame.storage().retained_storage())
                .unwrap();
            assert_eq!(frame.limits(), limits);
            assert_eq!(
                frame.graph_bytes(Role::B),
                owner.bound().canonical().canonical_bytes()
            );
            assert_eq!(
                frame.graph_bytes(Role::I),
                owner.output().canonical().canonical_bytes()
            );
            assert_eq!(
                frame.graph_bytes(Role::F),
                chain.output().canonical().canonical_bytes()
            );
            let decoded = materialize_refined_forwarding_history_v1(&frame, budget).unwrap();
            budget
                .reserve_storage(decoded.storage().retained_storage())
                .unwrap();
            let checked = decoded.check_semantics(budget).unwrap();
            budget
                .reserve_storage(checked.storage().retained_storage())
                .unwrap();
            assert_eq!(
                checked.output().canonical().identity(),
                chain.output().canonical().identity()
            );
            let text_floor = budget.storage();
            let (text, text_storage) = emit_prefix(chain.output(), profile, budget).unwrap();
            assert_eq!(budget.storage(), text_floor + text_storage);
            assert!(text.contains(profile.device_target().split(':').next().unwrap()));
            assert!(!text.contains(".fe2o3.kd."));
            assert!(budget.work_ledger_identity_v1() == account);
            // All owners/views die before releasing only this component delta.
            drop(text);
            budget.release_storage(text_storage).unwrap();
            drop(checked);
            drop(decoded);
            drop(frame);
            drop(wire);
            drop(chain);
            budget.release_storage(budget.storage() - floor).unwrap();
        });
    }
}

#[test]
fn conditional_bridge_descriptor_adoption_prepays_original_capacity_before_decode() {
    // Invalid inert bytes deliberately never mint a descriptor or proof owner.
    let extra = COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V5 - size_of::<Vec<u8>>()
        + COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V5;
    for short in [false, true] {
        let mut bytes = Vec::with_capacity(79);
        bytes.extend_from_slice(b"not a V5 source");
        let backing = size_of::<Vec<u8>>() + bytes.capacity();
        let required = 19 + backing + extra;
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, required - usize::from(short));
        budget.reserve_storage(19 + backing).unwrap();
        let account = budget.work_ledger_identity_v1();
        let result = adopt_descriptor(bytes, &mut budget);
        if short {
            assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
            assert_eq!(budget.failed_storage(), Some(required));
            assert_eq!(budget.work(), 0);
            assert_eq!(budget.storage(), 19 + backing);
        } else {
            assert!(matches!(
                result,
                Err(Error::Source(CompilerDescriptorSourceErrorV5::Wire(_)))
            ));
            assert_eq!(budget.storage(), required);
        }
        assert!(budget.work_ledger_identity_v1() == account);
    }
}

#[test]
fn conditional_bridge_missing_contract_roster_rejects_before_allocation() {
    let mut work = Work::new(10);
    let mut budget = Budget::new(&mut work, 19);
    budget.reserve_storage(19).unwrap();
    assert!(matches!(
        contracts(&[], &[0], &mut budget),
        Err(Error::Mismatch("complete canonical contract roster"))
    ));
    assert_eq!(budget.storage(), 19);
    assert_eq!(budget.work(), 1);
}

#[test]
fn conditional_bridge_opaque_refusal_is_terminal_at_outer_f_entry() {
    use crate::production_pipeline::ProductionPipelineError;
    use std::error::Error as _;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    for unwind in [false, true] {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 100);
        budget.reserve_storage(19).unwrap();
        let account = budget.work_ledger_identity_v1();
        let error = Error::Resource(Resource::Accounting);
        assert!(error.source().is_none());
        let result = catch_unwind(AssertUnwindSafe(|| {
            super::super::conditional_refusal(&mut budget, |budget| {
                budget.reserve_storage(23).unwrap();
                budget.charge_work(7).unwrap();
                if unwind {
                    panic!("inert final bridge unwind");
                }
                Err(ProductionPipelineError::conditional_final_bridge_v1(error))
            })
        }));
        assert_eq!(result.is_err(), unwind);
        assert_eq!(budget.storage(), 42);
        assert_eq!(budget.work(), 7);
        assert!(budget.work_ledger_identity_v1() == account);
    }
}
