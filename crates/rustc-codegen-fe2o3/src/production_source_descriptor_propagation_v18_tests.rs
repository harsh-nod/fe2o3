use super::*;

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_descriptors_cross_original_helpers_cfg_loops_tuples_and_returns() {
    run_actual_sources::<OwnedObservation>(
        &[
            ("helper", "let selected = identity(input);"),
            (
                "tuple",
                "let pair = (input, other); let selected = identity(pair.1);",
            ),
            (
                "branch",
                "let selected = if seed & 1 == 0 { input } else { other }; let selected = identity(selected);",
            ),
            (
                "loop",
                "let mut selected = input; let mut n = seed; while n != 0 { selected = identity(other); n -= 1; }",
            ),
            (
                "repeated",
                "let left = identity(input); let right = identity(other); let selected = if seed == 0 { left } else { right };",
            ),
            ("helper", "let selected = identity(input);"),
        ],
        &[(0, 0), (3, 2)],
        SOURCE_OWNED_CHILD,
        "SOURCE_DESCRIPTOR_PROPAGATION_V18",
        |body| {
            format!(
                r#"use fe2o3_device::{{kernel, thread, DisjointSlice}};
#[inline(never)]
fn identity(input: &[u32]) -> &[u32] {{ input }}
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn descriptor_probe(input: &[u32], other: &[u32], mut output: DisjointSlice<u32>, seed: u32) {{
    {body}
    let value = selected.len() as u32;
    if let Some(slot) = output.get_mut(thread::index_1d()) {{ *slot = value; }}
}}
"#
            )
        },
        |_, _, label, observation, previous| {
            assert_eq!(observation.kernels, 1);
            assert_eq!(observation.shared_slice_arguments, 2);
            assert_eq!(observation.generic_slice_parameters, 0);
            assert!(observation.global_slice_parameters >= 2);
            assert!(observation.source_as0_slice_types > 0);
            assert!(
                observation.helper_global_slice_parameters > 0,
                "the original outlined helper must receive its C1-selected whole descriptor"
            );
            assert_eq!(observation.helper_generic_slice_parameters, 0);
            assert_eq!(observation.descriptor_widenings, 0);
            assert_eq!(observation.stores, 1);
            if let Some(old) = previous.get(label) {
                assert_eq!(old, &observation);
            } else {
                previous.insert(label.to_owned(), observation);
            }
        },
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_mixed_descriptor_flow_widens_only_the_concrete_incoming_value() {
    run_actual_sources::<OwnedObservation>(
        &[(
            "mixed",
            "let known = identity(input); let unknown = identity(foreign.0); let selected = if seed == 0 { known } else { unknown };",
        )],
        &[(0, 0), (3, 2)],
        SOURCE_OWNED_CHILD,
        "SOURCE_DESCRIPTOR_MIXED_V18",
        |body| {
            format!(
                r#"use fe2o3_device::{{kernel, thread, DisjointSlice}};
#[inline(never)]
fn identity(input: &[u32]) -> &[u32] {{ input }}
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn descriptor_mixed(input: &[u32], foreign: (&[u32], u32), mut output: DisjointSlice<u32>, seed: u32) {{
    {body}
    let value = selected.len() as u32;
    if let Some(slot) = output.get_mut(thread::index_1d()) {{ *slot = value; }}
}}
"#
            )
        },
        |_, _, _, observation, _| {
            assert_eq!(observation.kernels, 1);
            assert_eq!(observation.shared_slice_arguments, 1);
            assert!(observation.compiler_laid_out_arguments > 0);
            assert!(observation.source_as0_slice_types > 0);
            assert!(observation.helper_global_slice_parameters > 0);
            assert!(
                observation.helper_generic_slice_parameters > 0,
                "the same Rust helper's separate unknown original instance must stay Generic"
            );
            assert!(
                observation.descriptor_widenings > 0,
                "mixed joins require an explicit whole-descriptor widening"
            );
            assert_eq!(observation.stores, 1);
        },
    );
}
