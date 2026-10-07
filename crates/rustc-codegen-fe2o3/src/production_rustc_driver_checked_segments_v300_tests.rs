//! Ordinary Rust reaches the private coupled-step emitter; this is not a proof run.
use super::*;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::expanded_source_tests::expanded_model_tests::checked_segments::checked_segment_model_child";

#[test]
#[ignore = "process helper; exact ordinary Rust supplied by the parent"]
fn checked_segment_model_child() {
    // This separate matrix must not overwrite the finite V282 export namespace.
    child(false, false);
}

fn source(case: &str) -> String {
    let body = match case {
        "distinct" => "let right = seed ^ 0x8000_0000; seed + right",
        "repeated" => "seed + seed",
        "subtract" => "let right = seed ^ 0x8000_0000; seed - right",
        "wrapping" => "seed.wrapping_add(seed)",
        _ => panic!("unknown checked-segment fixture"),
    };
    let mut source = format!(
        "use fe2o3_device::{{kernel, KernelContext}};\n\
         #[inline(never)]\n\
         fn shared_scalar(seed: u32) -> u32 {{ {body} }}\n"
    );
    for name in ["first", "second"] {
        source.push_str(&format!(
            "#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [3, 1, 1]))]\n\
             pub fn {name}(_ctx: KernelContext<'_>, seed: u32) {{\n\
                 let _ = shared_scalar(seed);\n\
             }}\n"
        ));
    }
    source
}

#[test]
#[ignore = "requires pinned nightly rust-src and authentic ordinary AMD source compilation"]
fn actual_rustc_checked_segments_follow_retained_u32_adds_only() {
    run_actual_sources::<Option<ModelObservation>>(
        &[
            ("distinct", "distinct"),
            ("distinct", "distinct"),
            ("repeated", "repeated"),
            ("subtract", "subtract"),
            ("wrapping", "wrapping"),
        ],
        &[(0, 0), (3, 0)],
        CHILD,
        "CHECKED_SEGMENTS_V300",
        source,
        |_, _, label, outcome, observations| {
            let observed = outcome.as_ref().expect("actual complete expanded model");
            assert!(observed.model_consumer_called);
            assert_eq!(observed.counts[0], 2);
            assert_ne!(observed.model, [0; 32]);
            assert_ne!(observed.census, [0; 32]);
            let count = observed.checked_segments[0];
            assert_eq!(observed.checked_segments, [count; 5]);
            assert_eq!(observed.checked_segment_roots.iter().sum::<usize>(), count);
            if matches!(label, "distinct" | "repeated") {
                assert!(
                    observed.checked_segment_roots.into_iter().all(|n| n > 0),
                    "both source roots must retain a coupled step"
                );
            } else {
                assert_eq!(count, 0, "other operators must not acquire Add proofs");
            }
            if let Some(previous) = observations.get(label) {
                assert_eq!(&outcome, previous);
            } else {
                observations.insert(label.to_owned(), outcome);
            }
        },
    );
}
