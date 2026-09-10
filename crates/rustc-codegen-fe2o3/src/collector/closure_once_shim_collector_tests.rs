use super::closure_once_shim_v1::tests::{entry, first_callee, run, shim};
use super::*;
use crate::closure_profile_v1::{
    ClosureOriginPolicyV1, ClosureOriginV1, analyze_production_closures_v1,
};

fn collect_entry<'tcx>(
    tcx: TyCtxt<'tcx>,
    name: &str,
) -> Result<CollectionResult<'tcx>, CollectError> {
    let mut collector = DeviceCollector::new(tcx, false, vec![], "gfx950".into());
    collector.add_root(KernelRoot {
        target: entry(tcx, name),
        logical_name: name.into(),
        export_name: name.into(),
        generated_host_contract_identity: None,
        kernel_binding: None,
        frontend_contract: None,
        kernel_context_contract: None,
        reference_effect_binding: None,
    })?;
    collector.collect()
}

#[test]
fn closure_once_shim_production_collector_retains_adapter_and_concrete_closure() {
    run(
        |tcx| {
            for name in ["entry", "entry_mut", "option_entry"] {
                let mut adapter = shim(tcx, name);
                if name == "option_entry" {
                    adapter = first_callee(tcx, adapter);
                }
                let closure = first_callee(tcx, adapter);
                let result = collect_entry(tcx, name).unwrap();
                for instance in [adapter, closure] {
                    let function = result
                        .functions
                        .iter()
                        .find(|function| function.instance == instance)
                        .expect("adapter and closure remain in the production graph");
                    assert_eq!(function.role, CollectedFunctionRole::InternalHelper);
                    assert!(function.dead_branches.is_some());
                }
                let function = result
                    .functions
                    .iter()
                    .find(|function| function.instance == adapter)
                    .unwrap();
                let profile = function
                    .closure_plan
                    .as_ref()
                    .expect("adapter retains closure profile");
                assert_eq!(profile.environments().len(), 1);
                assert_eq!(
                    profile.environments()[0].origin,
                    ClosureOriginV1::InvocationReceiver
                );
                assert_eq!(profile.calls().len(), 1);
                assert_eq!(
                    profile.calls()[0].target_definition_hash,
                    tcx.def_path_hash(closure.def_id()).0.to_le_bytes()
                );
            }
        },
        "abort",
    );
}

#[test]
fn closure_once_shim_receiver_origin_does_not_admit_an_unproven_host_argument() {
    run(
        |tcx| {
            let caller = first_callee(tcx, entry(tcx, "entry"));
            assert!(
                analyze_production_closures_v1(
                    tcx,
                    caller,
                    ClosureOriginPolicyV1::Either,
                    "gfx950",
                )
                .unwrap_err()
                .to_string()
                .contains("host closure references")
            );
            let adapter = first_callee(tcx, caller);
            let profile = analyze_production_closures_v1(
                tcx,
                adapter,
                ClosureOriginPolicyV1::Either,
                "gfx950",
            )
            .unwrap();
            assert_eq!(
                profile.environments()[0].origin,
                ClosureOriginV1::InvocationReceiver
            );
        },
        "abort",
    );
}

#[test]
fn closure_once_shim_production_collector_still_rejects_other_adapters_and_unsafe_callees() {
    run(
        |tcx| {
            let pointer = collect_entry(tcx, "entry_fn_pointer").unwrap_err();
            assert!(
                pointer.to_string().contains("FE2O3-FFI-CALL003"),
                "{pointer}"
            );
            let unsafe_closure = collect_entry(tcx, "entry_unsafe").unwrap_err();
            assert!(
                unsafe_closure.to_string().contains("FE2O3-CAP-SOURCE"),
                "{unsafe_closure}"
            );
            assert!(
                unsafe_closure.to_string().contains("unsafe_leaf"),
                "{unsafe_closure}"
            );
        },
        "abort",
    );
}
