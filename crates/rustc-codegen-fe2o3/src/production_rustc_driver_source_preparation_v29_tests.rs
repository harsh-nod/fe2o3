//! Actual preparation and original-owner consumption, not default activation.
use super::*;
use crate::production_pipeline::source_owned_v29::{
    PreparationObservationV29, start_preparation_observation_v29, take_preparation_observation_v29,
};
use fe2o3_lower_mir_kernel::{
    ProductionClosedScalarCheckErrorV18 as Check, ProductionClosedScalarHandoffErrorV18 as Handoff,
};

const PREPARATION_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::preparation_tests::source_owned_preparation_child";

#[derive(Default)]
struct PreparationCallbacks {
    result: Option<Result<PreparationObservationV29, String>>,
}

impl Callbacks for PreparationCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            start_preparation_observation_v29();
            let consumed = std::cell::Cell::new(false);
            let result = transaction.with_source_owned_scalar_handoff_v29::<(), _>(|_, _, _| {
                consumed.set(true);
                Ok(())
            });
            let observation = take_preparation_observation_v29()
                .ok_or_else(|| format!("actual preparation did not complete: {result:?}"))?;
            assert!(
                !consumed.get(),
                "general preparation cannot bypass the final closed subset"
            );
            assert!(
                observation.materialized,
                "the genuine prepared owner must reach its source consumer"
            );
            let expected = if observation.contexts {
                "non-root functions"
            } else {
                "memory or non-scalar type"
            };
            match result {
                Err(Error::Handoff(Handoff::Check(Check::Unsupported(actual))))
                    if actual == expected =>
                {
                    Ok(observation)
                }
                other => Err(format!(
                    "expected exact final refusal {expected:?}, got {other:?}"
                )),
            }
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn source_owned_preparation_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = PreparationCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks.result.expect("preparation callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("preparation result"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "source preparation: {result:?}");
}

fn source(body: &str) -> String {
    format!(
        r#"use fe2o3_device::{{kernel, DisjointSlice, DeviceGlobalMutPtr, KernelContext}};
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Payload {{ pub first: u32, pub second: u32 }}
{body}
"#
    )
}

fn check_preparation(body: &str, contexts: bool, kinds: &[&str]) {
    run_actual_sources::<PreparationObservationV29>(
        &[("preparation", body)],
        &[(0, 0)],
        PREPARATION_CHILD,
        "SOURCE_OWNED_PREPARATION_V29",
        source,
        |_, _, _, result, _| {
            assert_eq!(result.contexts, contexts);
            assert!(result.materialized);
            assert_eq!(result.kinds, kinds);
        },
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_full_typed_abi_reaches_original_source_owner_without_scalar_admission() {
    check_preparation(
        r#"#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn preparation_memory(_input: &[u32], _output: DisjointSlice<u32>, _pointer: DeviceGlobalMutPtr<u32>, _payload: Payload) {}"#,
        false,
        &[
            "SharedSlice(U32)",
            "DisjointSlice(U32)",
            "GlobalMutPointer(U32)",
            "ByValue",
        ],
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_retained_context_projection_reaches_original_source_owner_without_scalar_admission() {
    check_preparation(
        r#"#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn preparation_context(mut ctx: KernelContext<'_>, _output: DisjointSlice<u32>) {
    let _value = ctx.with_workgroup(|_wg| 7_u32);
}"#,
        true,
        &["DisjointSlice(U32)"],
    );
}
