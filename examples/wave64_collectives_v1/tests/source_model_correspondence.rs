use std::path::Path;
use std::process::Command;

use fe2o3_wave64_collectives_v1::{
    CollectiveOutputV1, REVIEWED_SOURCE_CPU_CORRESPONDENCE_BOUNDARY_V2,
    SourceCpuCorrespondenceErrorV2, SourceStructureErrorV2, WAVE64_LANES_V1,
    bind_source_cpu_content_to_outer_commit_v2, collect_reviewed_source_algorithm_v2,
    exact_source_cpu_content_identities_v2, interpret_reviewed_source_algorithm_v2,
    verify_reviewed_source_to_cpu_correspondence_v2, wave64_collectives_oracle_v1,
};
use sha2::{Digest as _, Sha256};

const SOURCE: &str = include_str!("../src/kernel.rs");
const SOURCE_CPU_PROOF: &str =
    include_str!("../verus/wave64_attributed_source_cpu_correspondence_v2.rs");
const PUBLIC_BASE: &str = "b8daeb2bc953924a424542820bed566e52d57290";

fn parse_commit(hex: &str) -> [u8; 20] {
    assert_eq!(hex.len(), 40);
    core::array::from_fn(|index| {
        u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).expect("Git commit hex")
    })
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn verus_digest(name: &str) -> [u64; 4] {
    let declaration = format!("pub open spec fn {name}");
    let start = SOURCE_CPU_PROOF
        .find(&declaration)
        .unwrap_or_else(|| panic!("missing Verus digest {name}"));
    let body = &SOURCE_CPU_PROOF[start..];
    core::array::from_fn(|word| {
        let marker = format!("word{word}: 0x");
        let value = body
            .find(&marker)
            .map(|offset| &body[offset + marker.len()..])
            .unwrap_or_else(|| panic!("missing {marker} in {name}"));
        u64::from_str_radix(&value[..16], 16).expect("Verus SHA-256 word")
    })
}

fn digest_words(bytes: [u8; 32]) -> [u64; 4] {
    core::array::from_fn(|word| {
        u64::from_be_bytes(bytes[word * 8..(word + 1) * 8].try_into().unwrap())
    })
}

fn corpus() -> [f32; WAVE64_LANES_V1] {
    core::array::from_fn(|lane| ((lane * 37 + 11) % 127) as f32 - 63.0)
}

fn prefix_mask(end: usize) -> u64 {
    match end {
        0 => 0,
        WAVE64_LANES_V1.. => u64::MAX,
        _ => (1_u64 << end) - 1,
    }
}

fn replace_once(source: &str, from: &str, to: &str) -> String {
    assert_eq!(source.matches(from).count(), 1, "mutation anchor {from:?}");
    source.replacen(from, to, 1)
}

#[test]
fn exact_syntax_collects_all_reviewed_algorithm_fields() {
    let algorithm = collect_reviewed_source_algorithm_v2(SOURCE).unwrap();
    assert_eq!(algorithm.lanes(), 64);
    assert_eq!(
        algorithm.ordered_collectives(),
        [
            CollectiveOutputV1::Reduction,
            CollectiveOutputV1::Inclusive,
            CollectiveOutputV1::Exclusive,
        ]
    );
    assert!(algorithm.selects_mask_bit_at_physical_lane());
    assert!(algorithm.uses_increasing_physical_lane_order());
    assert!(algorithm.uses_inactive_positive_zero());
    assert!(algorithm.uses_same_lane_output_ownership());

    let comment_only = SOURCE.replace(
        "//! Ordinary attributed Rust source",
        "// non-doc structural review input\n//! Ordinary attributed Rust source",
    );
    assert_ne!(
        Sha256::digest(comment_only.as_bytes()),
        Sha256::digest(SOURCE)
    );
    assert!(collect_reviewed_source_algorithm_v2(&comment_only).is_ok());
}

#[test]
fn hostile_semantic_mutations_retain_names_but_fail_exact_ast_admission() {
    let call_order = "    let reduction = wave.reduce_sum(&context, contribution);\n    let inclusive = wave.inclusive_scan_sum(&context, contribution);";
    let swapped_order = "    let inclusive = wave.inclusive_scan_sum(&context, contribution);\n    let reduction = wave.reduce_sum(&context, contribution);";
    let mutations = [
        replace_once(SOURCE, "!= 0;", "== 0;"),
        replace_once(SOURCE, "1_u64 << lane", "1_u64 << (lane ^ 1)"),
        replace_once(SOURCE, "input[lane]", "input[63 - lane]"),
        replace_once(SOURCE, "else { 0.0_f32 }", "else { -0.0_f32 }"),
        replace_once(SOURCE, call_order, swapped_order),
        replace_once(
            SOURCE,
            "wave.exclusive_scan_sum(&context, contribution)",
            "wave.exclusive_scan_sum(&context, -contribution)",
        ),
        replace_once(
            SOURCE,
            "if active { exclusive } else { 0.0 }",
            "if active { inclusive } else { 0.0 }",
        ),
        replace_once(
            SOURCE,
            "if active { reduction } else { 0.0 }",
            "if active { reduction } else { -0.0 }",
        ),
        replace_once(
            SOURCE,
            "reduction_output.get_mut(lane_index)",
            "reduction_output.get_mut(thread::index_1d())",
        ),
        replace_once(SOURCE, "input.len() != 64", "input.len() < 64"),
        replace_once(
            SOURCE,
            "let reduction = wave.reduce_sum(&context, contribution);",
            "let reduction = if active { wave.reduce_sum(&context, contribution) } else { 0.0 };",
        ),
        replace_once(
            SOURCE,
            "2863304ebf7f501a7f177c5b8f5a456261ee34760472727ba3f0205ccf5ce9cc",
            "3863304ebf7f501a7f177c5b8f5a456261ee34760472727ba3f0205ccf5ce9cc",
        ),
    ];

    for (index, mutation) in mutations.iter().enumerate() {
        for retained in [
            "active_mask",
            "reduce_sum",
            "inclusive_scan_sum",
            "exclusive_scan_sum",
            "reduction_output",
            "inclusive_output",
            "exclusive_output",
            "get_mut",
        ] {
            assert!(
                mutation.contains(retained),
                "mutation {index} lost {retained}"
            );
        }
        assert_ne!(Sha256::digest(mutation.as_bytes()), Sha256::digest(SOURCE));
        assert_eq!(
            collect_reviewed_source_algorithm_v2(mutation),
            Err(SourceStructureErrorV2::NonCanonicalSyntaxTree),
            "hostile mutation {index} passed structural admission"
        );
    }
}

#[test]
fn malformed_or_injected_source_fails_closed() {
    assert_eq!(
        collect_reviewed_source_algorithm_v2("pub fn wave64_collectives_v1("),
        Err(SourceStructureErrorV2::InvalidRustSyntax)
    );
    let injected = format!("{SOURCE}\nfn shadow_collective() {{ loop {{}} }}\n");
    assert_eq!(
        collect_reviewed_source_algorithm_v2(&injected),
        Err(SourceStructureErrorV2::NonCanonicalSyntaxTree)
    );
}

#[test]
fn abstract_interpreter_matches_cpu_oracle_across_masks_and_exact_inputs() {
    let mut masks = vec![
        0,
        u64::MAX,
        0xaaaa_aaaa_aaaa_aaaa,
        0x5555_5555_5555_5555,
        0x8000_0000_0000_0001,
    ];
    for lane in 0..WAVE64_LANES_V1 {
        masks.push(1_u64 << lane);
        masks.push(!(1_u64 << lane));
    }
    for end in 0..=WAVE64_LANES_V1 {
        masks.push(prefix_mask(end));
        masks.push(!prefix_mask(end));
    }
    let mut state = 0x243f_6a88_85a3_08d3_u64;
    for _ in 0..4096 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        masks.push(state);
    }

    let algorithm = collect_reviewed_source_algorithm_v2(SOURCE).unwrap();
    for input in [
        corpus(),
        [0.0; WAVE64_LANES_V1],
        [-0.0; WAVE64_LANES_V1],
        core::array::from_fn(|lane| if lane % 2 == 0 { -1024.0 } else { 1024.0 }),
    ] {
        for mask in masks.iter().copied() {
            let interpreted = interpret_reviewed_source_algorithm_v2(algorithm, &input, mask)
                .unwrap_or_else(|error| panic!("mask {mask:#018x} rejected: {error}"));
            let mut reduction = [f32::NAN; WAVE64_LANES_V1];
            let mut inclusive = [f32::NAN; WAVE64_LANES_V1];
            let mut exclusive = [f32::NAN; WAVE64_LANES_V1];
            wave64_collectives_oracle_v1(
                &input,
                mask,
                &mut reduction,
                &mut inclusive,
                &mut exclusive,
            )
            .unwrap();
            for lane in 0..WAVE64_LANES_V1 {
                if mask & (1_u64 << lane) == 0 {
                    assert_eq!(interpreted.reduction[lane].to_bits(), 0);
                    assert_eq!(interpreted.inclusive[lane].to_bits(), 0);
                    assert_eq!(interpreted.exclusive[lane].to_bits(), 0);
                    assert_eq!(reduction[lane].to_bits(), 0);
                    assert_eq!(inclusive[lane].to_bits(), 0);
                    assert_eq!(exclusive[lane].to_bits(), 0);
                } else {
                    assert_eq!(interpreted.reduction[lane], reduction[lane]);
                    assert_eq!(interpreted.inclusive[lane], inclusive[lane]);
                    assert_eq!(interpreted.exclusive[lane], exclusive[lane]);
                }
            }
        }
    }

    let receipt = verify_reviewed_source_to_cpu_correspondence_v2(
        &corpus(),
        0x8000_0042_8000_0021,
        bind_source_cpu_content_to_outer_commit_v2(parse_commit(PUBLIC_BASE)),
    )
    .unwrap();
    assert_eq!(receipt.checked_outputs(), 3 * 64);
}

#[test]
fn interpreter_recurrences_select_mask_lane_order_zero_and_ownership() {
    let algorithm = collect_reviewed_source_algorithm_v2(SOURCE).unwrap();
    let input: [f32; WAVE64_LANES_V1] = core::array::from_fn(|lane| (lane + 1) as f32);
    let mask = (1_u64 << 0) | (1_u64 << 2) | (1_u64 << 5) | (1_u64 << 63);
    let output = interpret_reviewed_source_algorithm_v2(algorithm, &input, mask).unwrap();
    assert_eq!(output.reduction[0], 74.0);
    assert_eq!(output.reduction[2], 74.0);
    assert_eq!(output.reduction[5], 74.0);
    assert_eq!(output.reduction[63], 74.0);
    assert_eq!(output.inclusive[0], 1.0);
    assert_eq!(output.inclusive[2], 4.0);
    assert_eq!(output.inclusive[5], 10.0);
    assert_eq!(output.inclusive[63], 74.0);
    assert_eq!(output.exclusive[0].to_bits(), 0.0_f32.to_bits());
    assert_eq!(output.exclusive[2], 1.0);
    assert_eq!(output.exclusive[5], 4.0);
    assert_eq!(output.exclusive[63], 10.0);
    for lane in 0..WAVE64_LANES_V1 {
        if mask & (1_u64 << lane) == 0 {
            assert_eq!(output.reduction[lane].to_bits(), 0.0_f32.to_bits());
            assert_eq!(output.inclusive[lane].to_bits(), 0.0_f32.to_bits());
            assert_eq!(output.exclusive[lane].to_bits(), 0.0_f32.to_bits());
        }
    }
}

#[test]
fn exact_identities_and_transcript_fail_closed_under_mutation() {
    let exact = exact_source_cpu_content_identities_v2();
    assert_eq!(
        encode_hex(&exact.attributed_source_sha256),
        "01ac1365b0fdfe91cdc8f7cf6a14ae5acbea41528103ec3de5fe6d895261625e"
    );
    assert_eq!(
        encode_hex(&exact.cpu_oracle_sha256),
        "837aae894e5c04da4b598e45f344f2e5df0aa8bc6155acf0bf05809ecd86d407"
    );
    assert_eq!(
        encode_hex(&exact.correspondence_sha256),
        "7b910de7f37d5fbdf8e72103f353dc743bd1292af39c6efcee405f3fcf5a9514"
    );
    assert_eq!(
        verus_digest("attributed_source_identity_v2"),
        digest_words(exact.attributed_source_sha256)
    );
    assert_eq!(
        verus_digest("cpu_oracle_identity_v2"),
        digest_words(exact.cpu_oracle_sha256)
    );
    assert_eq!(
        verus_digest("reviewed_correspondence_identity_v2"),
        digest_words(exact.correspondence_sha256)
    );

    let binding = bind_source_cpu_content_to_outer_commit_v2(parse_commit(PUBLIC_BASE));
    for mutate in 0..5 {
        let mut hostile = binding;
        match mutate {
            0 => hostile.content.attributed_source_sha256[0] ^= 1,
            1 => hostile.content.cpu_oracle_sha256[9] ^= 1,
            2 => hostile.content.correspondence_sha256[31] ^= 1,
            3 => hostile.outer_commit[0] ^= 1,
            4 => hostile.transcript_sha256[17] ^= 1,
            _ => unreachable!(),
        }
        assert_eq!(
            verify_reviewed_source_to_cpu_correspondence_v2(&corpus(), u64::MAX, hostile),
            Err(SourceCpuCorrespondenceErrorV2::IdentityBinding)
        );
    }
}

#[test]
fn receipt_names_every_non_authority_boundary() {
    let receipt = verify_reviewed_source_to_cpu_correspondence_v2(
        &corpus(),
        u64::MAX,
        bind_source_cpu_content_to_outer_commit_v2(parse_commit(PUBLIC_BASE)),
    )
    .unwrap();
    assert!(receipt.is_reviewed_structural_correspondence());
    assert!(!receipt.proves_source_to_model_refinement());
    assert!(!receipt.proves_outer_commit_contains_content());
    assert!(!receipt.proves_compiler_causality());
    assert!(!receipt.proves_machine_refinement_or_execution());
    assert!(!receipt.proves_generalized_safety());
    assert!(!receipt.grants_parity_promotion());
    for boundary in [
        "reviewed exact-syntax structural correspondence",
        "active zero sign is abstracted",
        "Git-tree membership is not proven",
        "proves_source_to_model_refinement=false",
        "no MIR/compiler causality",
        "no KIR/LLVM/ISA or GPU evidence",
        "no generalized memory safety or race freedom",
        "no parity authority",
    ] {
        assert!(
            REVIEWED_SOURCE_CPU_CORRESPONDENCE_BOUNDARY_V2.contains(boundary),
            "missing boundary {boundary}"
        );
    }
}

#[test]
fn current_outer_commit_process_check_contains_the_three_exact_files() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&repo)
        .output()
        .expect("run git rev-parse");
    assert!(head.status.success());
    let head = std::str::from_utf8(&head.stdout).unwrap().trim();
    assert_eq!(head.len(), 40);

    let base = Command::new("git")
        .args(["merge-base", "--is-ancestor", PUBLIC_BASE, head])
        .current_dir(&repo)
        .status()
        .expect("run git merge-base");
    assert!(
        base.success(),
        "outer commit does not descend from public base"
    );

    for (path, exact) in [
        (
            "examples/wave64_collectives_v1/src/kernel.rs",
            include_bytes!("../src/kernel.rs").as_slice(),
        ),
        (
            "examples/wave64_collectives_v1/src/oracle.rs",
            include_bytes!("../src/oracle.rs").as_slice(),
        ),
        (
            "examples/wave64_collectives_v1/src/source_model_correspondence.rs",
            include_bytes!("../src/source_model_correspondence.rs").as_slice(),
        ),
    ] {
        let object = format!("{head}:{path}");
        let output = Command::new("git")
            .args(["show", &object])
            .current_dir(&repo)
            .output()
            .expect("run git show");
        assert!(output.status.success(), "outer commit lacks {path}");
        assert_eq!(output.stdout, exact, "outer commit bytes differ for {path}");
    }

    let receipt = verify_reviewed_source_to_cpu_correspondence_v2(
        &corpus(),
        0x8000_0000_0000_0021,
        bind_source_cpu_content_to_outer_commit_v2(parse_commit(head)),
    )
    .unwrap();
    assert_eq!(receipt.binding().outer_commit, parse_commit(head));
}
