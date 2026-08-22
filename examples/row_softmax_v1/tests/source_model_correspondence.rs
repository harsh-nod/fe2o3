use std::path::Path;
use std::process::{Command, Stdio};

use fe2o3_row_softmax_v1::{
    REVIEWED_ROW_SOFTMAX_SOURCE_BOUNDARY_V1, RowSoftmaxPhaseV1,
    RowSoftmaxSourceCorrespondenceErrorV1, RowSoftmaxSourceStructureErrorV1,
    bind_row_softmax_source_content_to_outer_commit_v1, collect_reviewed_row_softmax_algorithm_v1,
    exact_row_softmax_source_content_identities_v1, interpret_reviewed_row_softmax_source_v1,
    reviewed_row_softmax_abstract_model_v1, verify_reviewed_row_softmax_source_correspondence_v1,
};
use sha2::{Digest as _, Sha256};

const PUBLIC_BASE: &str = "e874da2083c2a1eb192048ea5f88a053c28d0ee2";
const LINEAGE_SOURCE_PATH: &str =
    "crates/rustc-codegen-fe2o3/tests/fixtures/collected-row-softmax-v1/src/lib.rs";
const SOURCE: &str = include_str!("../src/kernel.rs");
const MODEL: &[u8] = include_bytes!("../src/source_model_correspondence.rs");
const VERUS_MODEL: &[u8] = include_bytes!("../verus/row_softmax_v1.rs");
const MEMORY_PRECONDITIONS: &[u8] = b"input-f32-elements=64;output-disjoint-f32-elements=64";
const FIXTURE_FACADE: &str = include_str!(
    "../../../crates/rustc-codegen-fe2o3/tests/fixtures/collected-row-softmax-v1/src/lib.rs"
);

fn parse_commit(hex: &str) -> [u8; 20] {
    assert_eq!(hex.len(), 40);
    let mut bytes = [0_u8; 20];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
    }
    bytes
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hostile(source: &str, from: &str, to: &str) -> String {
    let mutation = source.replacen(from, to, 1);
    assert_ne!(
        mutation, source,
        "hostile mutation anchor was absent: {from}"
    );
    mutation
}

#[test]
fn example_owns_the_only_ordinary_kernel_and_fixture_is_only_a_facade() {
    assert!(SOURCE.contains("#[kernel("));
    assert!(SOURCE.contains("pub fn row_softmax_v1"));
    assert_eq!(SOURCE.len(), 1_289);
    assert_eq!(
        sha256(SOURCE.as_bytes()),
        [
            0xc4, 0xe2, 0xd6, 0xbb, 0x6e, 0xeb, 0xe0, 0x1e, 0xb6, 0xae, 0x7c, 0x0d, 0xa1, 0xa5,
            0x24, 0x11, 0x38, 0x19, 0xa3, 0x7b, 0x4e, 0xc2, 0xd0, 0xa5, 0x16, 0x7f, 0x32, 0xcc,
            0x31, 0x34, 0xe6, 0xf4,
        ]
    );
    assert_eq!(
        FIXTURE_FACADE,
        "//! Compiler-fixture facade over the example-owned canonical kernel source.\n\npub use fe2o3_row_softmax_v1::kernel::row_softmax_v1_gpu;\n"
    );
    assert!(!FIXTURE_FACADE.contains("#[kernel("));
    assert!(!FIXTURE_FACADE.contains("macro_rules!"));
    assert!(!SOURCE.contains("include!"));
    assert!(!SOURCE.contains("include_str!"));
    assert!(!SOURCE.contains("macro_rules!"));
}

#[test]
fn exact_source_admits_the_reviewed_lane_zero_three_loop_schedule() {
    let algorithm = collect_reviewed_row_softmax_algorithm_v1(SOURCE).unwrap();
    assert_eq!(algorithm.row_elements(), 64);
    assert_eq!(algorithm.participating_lane(), 0);
    assert_eq!(
        algorithm.phases(),
        [
            RowSoftmaxPhaseV1::Maximum,
            RowSoftmaxPhaseV1::Denominator,
            RowSoftmaxPhaseV1::Output,
        ]
    );
    assert_eq!(algorithm.barrier_count(), 0);
}

#[test]
fn comments_and_whitespace_do_not_change_the_ast_but_doc_attributes_do() {
    let with_comment = SOURCE.replacen(
        "let lane = thread::index_1d().get();",
        "/* inert comment */ let lane = thread::index_1d().get();",
        1,
    );
    assert!(collect_reviewed_row_softmax_algorithm_v1(&with_comment).is_ok());

    let with_doc = SOURCE.replacen(
        "pub fn row_softmax_v1",
        "/// Changed documentation attribute.\npub fn row_softmax_v1",
        1,
    );
    assert_eq!(
        collect_reviewed_row_softmax_algorithm_v1(&with_doc),
        Err(RowSoftmaxSourceStructureErrorV1::NonCanonicalSyntaxTree)
    );
}

#[test]
fn malformed_and_hostile_source_mutations_fail_closed() {
    assert_eq!(
        collect_reviewed_row_softmax_algorithm_v1("pub fn"),
        Err(RowSoftmaxSourceStructureErrorV1::InvalidRustSyntax)
    );

    let mutations = [
        hostile(SOURCE, "if lane == 0", "if lane != 0"),
        hostile(SOURCE, "f32::NEG_INFINITY", "0.0_f32"),
        hostile(
            SOURCE,
            "while index < ROW_ELEMENTS",
            "while index <= ROW_ELEMENTS",
        ),
        hostile(SOURCE, "if value > maximum", "if value >= maximum"),
        hostile(SOURCE, "let value = input[index]", "let value = input[0]"),
        hostile(SOURCE, "maximum = value", "maximum = input[0]"),
        hostile(
            SOURCE,
            "DeviceMath::current()",
            "DeviceMath::current_unchecked()",
        ),
        hostile(
            SOURCE,
            "let mut denominator = 0.0_f32",
            "let mut denominator = 1.0_f32",
        ),
        hostile(
            SOURCE,
            "denominator += math.exp_f32(input[index] - maximum)",
            "denominator = math.exp_f32(input[index] - maximum)",
        ),
        hostile(
            SOURCE,
            "math.exp_f32(input[index] - maximum)",
            "math.exp_f32(input[index])",
        ),
        hostile(
            SOURCE,
            "math.exp_f32(input[index] - maximum) / denominator",
            "math.exp_f32(input[0] - maximum) / denominator",
        ),
        hostile(
            SOURCE,
            "output.get_mut_at(index)",
            "output.get_mut_at(index + 1)",
        ),
        hostile(SOURCE, "*slot = probability", "*slot += probability"),
        hostile(
            SOURCE,
            "control_flow(loop_bounds(64, 64, 64))",
            "control_flow(loop_bounds(64, 64, 63))",
        ),
        hostile(
            SOURCE,
            "launch(required = [64, 1, 1]",
            "launch(required = [32, 1, 1]",
        ),
    ];
    for mutation in mutations {
        assert_eq!(
            collect_reviewed_row_softmax_algorithm_v1(&mutation),
            Err(RowSoftmaxSourceStructureErrorV1::NonCanonicalSyntaxTree)
        );
    }
}

#[test]
fn source_interpreter_matches_independent_model_for_all_physical_lanes() {
    let algorithm = collect_reviewed_row_softmax_algorithm_v1(SOURCE).unwrap();
    for lane in 0..64 {
        assert_eq!(
            interpret_reviewed_row_softmax_source_v1(algorithm, lane),
            reviewed_row_softmax_abstract_model_v1(lane),
            "lane {lane}"
        );
    }
}

#[test]
fn lane_zero_trace_binds_reads_calls_writes_ownership_and_no_barriers() {
    let algorithm = collect_reviewed_row_softmax_algorithm_v1(SOURCE).unwrap();
    let lane_zero = interpret_reviewed_row_softmax_source_v1(algorithm, 0);
    assert_eq!(lane_zero.lane(), 0);
    assert_eq!(lane_zero.input_reads(), 3 * 64);
    assert_eq!(lane_zero.abstract_exp_calls(), 2 * 64);
    assert_eq!(lane_zero.output_writes(), (0..64).collect::<Vec<_>>());
    assert_eq!(lane_zero.barrier_count(), 0);
    assert_eq!(lane_zero.operations().len(), 707);

    for lane in 1..64 {
        let trace = interpret_reviewed_row_softmax_source_v1(algorithm, lane);
        assert_eq!(trace.operations().len(), 2);
        assert_eq!(trace.input_reads(), 0);
        assert_eq!(trace.abstract_exp_calls(), 0);
        assert!(trace.output_writes().is_empty());
        assert_eq!(trace.barrier_count(), 0);
    }
}

#[test]
fn exact_content_identities_and_outer_commit_binding_fail_closed() {
    let exact = exact_row_softmax_source_content_identities_v1();
    assert_eq!(exact.attributed_source_sha256, sha256(SOURCE.as_bytes()));
    assert_eq!(exact.abstract_model_sha256, sha256(MODEL));
    assert_eq!(exact.verus_model_sha256, sha256(VERUS_MODEL));
    assert_eq!(
        exact.memory_preconditions_sha256,
        sha256(MEMORY_PRECONDITIONS)
    );

    let binding = bind_row_softmax_source_content_to_outer_commit_v1(parse_commit(PUBLIC_BASE));
    for mutation in 0..5 {
        let mut hostile = binding;
        match mutation {
            0 => hostile.content.attributed_source_sha256[0] ^= 1,
            1 => hostile.content.abstract_model_sha256[11] ^= 1,
            2 => hostile.content.verus_model_sha256[31] ^= 1,
            3 => hostile.content.memory_preconditions_sha256[7] ^= 1,
            4 => hostile.transcript_sha256[19] ^= 1,
            _ => unreachable!(),
        }
        assert_eq!(
            verify_reviewed_row_softmax_source_correspondence_v1(hostile),
            Err(RowSoftmaxSourceCorrespondenceErrorV1::IdentityBinding)
        );
    }
}

#[test]
fn receipt_is_bounded_and_names_every_non_authority_boundary() {
    let receipt = verify_reviewed_row_softmax_source_correspondence_v1(
        bind_row_softmax_source_content_to_outer_commit_v1(parse_commit(PUBLIC_BASE)),
    )
    .unwrap();
    assert_eq!(receipt.checked_physical_lanes(), 64);
    assert_eq!(receipt.checked_abstract_operations(), 833);
    assert_eq!(receipt.required_input_elements(), 64);
    assert_eq!(receipt.required_output_elements(), 64);
    assert!(receipt.authenticates_exact_memory_preconditions());
    assert!(!receipt.proves_runtime_memory_preconditions());
    assert!(receipt.has_single_canonical_ordinary_source());
    assert!(!receipt.proves_source_to_model_refinement());
    assert!(!receipt.proves_exp_ieee_or_ocml_semantics());
    assert!(!receipt.proves_outer_commit_contains_content());
    assert!(!receipt.proves_compiler_or_gpu_causality());
    assert!(!receipt.proves_generalized_memory_or_race_safety());
    assert!(!receipt.grants_parity_promotion());

    for marker in [
        "ordinary example-owned #[kernel] Rust source",
        "lane0-only three-loop zero-barrier",
        "conditional on authenticated exact 64-element input and output preconditions",
        "runtime precondition satisfaction unproved",
        "proves_source_to_model_refinement=false",
        "exp_f32/IEEE/OCML semantics unproved",
        "Rust operational semantics unproved",
        "no MIR/compiler/KIR/LLVM/ISA/GPU causality",
        "no generalized memory safety or race freedom",
        "no parity authority",
    ] {
        assert!(
            REVIEWED_ROW_SOFTMAX_SOURCE_BOUNDARY_V1.contains(marker),
            "missing boundary {marker}"
        );
    }
}

#[test]
fn current_outer_commit_contains_the_exact_source_model_and_verus_files() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let lineage_object = format!("{PUBLIC_BASE}:{LINEAGE_SOURCE_PATH}");
    let lineage = Command::new("git")
        .args(["show", &lineage_object])
        .current_dir(&repo)
        .output()
        .expect("read lineage source");
    assert!(lineage.status.success());
    assert_eq!(lineage.stdout, SOURCE.as_bytes());

    let absent_example_object = format!("{PUBLIC_BASE}:examples/row_softmax_v1/src/kernel.rs");
    assert!(
        !Command::new("git")
            .args(["cat-file", "-e", &absent_example_object])
            .current_dir(&repo)
            .stderr(Stdio::null())
            .status()
            .expect("check predecessor example path")
            .success()
    );

    let head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&repo)
        .output()
        .expect("run git rev-parse");
    assert!(head.status.success());
    let head = std::str::from_utf8(&head.stdout).unwrap().trim();
    assert_eq!(head.len(), 40);

    assert!(
        Command::new("git")
            .args(["merge-base", "--is-ancestor", PUBLIC_BASE, head])
            .current_dir(&repo)
            .status()
            .expect("run git merge-base")
            .success()
    );

    for (path, exact) in [
        ("examples/row_softmax_v1/src/kernel.rs", SOURCE.as_bytes()),
        (
            "examples/row_softmax_v1/src/source_model_correspondence.rs",
            MODEL,
        ),
        (
            "examples/row_softmax_v1/verus/row_softmax_v1.rs",
            VERUS_MODEL,
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

    let receipt = verify_reviewed_row_softmax_source_correspondence_v1(
        bind_row_softmax_source_content_to_outer_commit_v1(parse_commit(head)),
    )
    .unwrap();
    assert_eq!(receipt.binding().outer_commit, parse_commit(head));
}
