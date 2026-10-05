use sha2::{Digest as _, Sha256};

fn function<'a>(model: &'a str, name: &str) -> (&'a str, &'a str) {
    let prefix = format!("spec fn {name}(");
    assert_eq!(model.matches(&prefix).count(), 1);
    let start = model.find(&prefix).unwrap();
    let (header, rest) = model[start..].split_once(" {\n").unwrap();
    let (body, _) = rest.split_once("\n}\n").unwrap();
    (header, body)
}

// Frozen 2c9254c entry declarations, before factoring. Recomposition must
// preserve every guard, allocation, frame update and ordered argument install.
#[test]
fn source_entry_factor_recomposes_all_original_guards_and_bodies() {
    for (disjoint, copied, expected) in [
        (
            false,
            false,
            "8cdfdf2f46f98b356dae16637a269f32596dae3866f72d91733f849105bf9ed9",
        ),
        (
            false,
            true,
            "8cdfdf2f46f98b356dae16637a269f32596dae3866f72d91733f849105bf9ed9",
        ),
        (
            true,
            false,
            "669048bb37cca42dde2a5c7d6d207bbcbe0f2d3d564342a10e73d0a709acd4de",
        ),
    ] {
        super::run_write_model(super::LIMIT, super::LIMIT, disjoint, copied, |model| {
            let mut original = String::new();
            for root in 0..2 {
                for instance in 0..3 {
                    let entry = format!("invocation_source_enter_{root}_{instance}_v36");
                    let refuses = format!("invocation_source_entry_refuses_{root}_{instance}_v167");
                    let entered = format!("invocation_source_entry_body_{root}_{instance}_v167");
                    let (header, selection) = function(model, &entry);
                    let (_, guard) = function(model, &refuses);
                    let (_, body) = function(model, &entered);
                    assert!(guard.starts_with(" !source.machine.valid || "));
                    assert_eq!(selection, format!(" invocation_source_entry_select_v167(source, {refuses}(source, arguments, little_endian), {entered}(source, arguments, little_endian))"));
                    writeln!(original, "{header} {{\n if{} {{ invocation_source_byte_refused_v36(source) }} else {{\n{body}\n }}\n}}", guard).unwrap();
                }
            }
            let digest = |text: &str| format!("{:x}", Sha256::digest(text.as_bytes()));
            assert_eq!(digest(&original), expected);
            let bad_guard = original.replacen("!source.machine.valid", "source.machine.valid", 1);
            let bad_body = original.replacen("invocation_source_byte_put_local_v36(entered,", "invocation_source_byte_put_local_v36(source,", 1);
            assert_ne!(bad_guard, original);
            assert_ne!(bad_body, original);
            assert_ne!(digest(&bad_guard), expected);
            assert_ne!(digest(&bad_body), expected);
        }).0.unwrap();
    }
}

#[test]
fn source_entry_selection_law_has_no_admission_premises() {
    let text = include_str!("original_semantic_mir_source_entry_select_v167.vrs");
    assert_eq!(
        text,
        concat!(
            "spec fn invocation_source_entry_select_v167(\n",
            "    source: InvocationSourceByteStateV36, refused: bool, body: InvocationSourceByteStateV36,\n",
            ") -> InvocationSourceByteStateV36 {\n",
            "    if !source.machine.valid || refused {\n",
            "        invocation_source_byte_refused_v36(source)\n",
            "    } else {\n",
            "        body\n",
            "    }\n",
            "}\n\n",
            "proof fn invocation_source_entry_select_success_v167(\n",
            "    source: InvocationSourceByteStateV36, refused: bool, body: InvocationSourceByteStateV36,\n",
            ")\n",
            "    ensures invocation_source_entry_select_v167(source, refused, body).machine.valid ==>\n",
            "        source.machine.valid && !refused && invocation_source_entry_select_v167(source, refused, body) == body,\n",
            "{\n",
            "}\n",
        )
    );
    for forbidden in ["requires", "assume(", "admit(", "external_body"] {
        assert!(!text.contains(forbidden));
    }
}

use std::fmt::Write as _;
