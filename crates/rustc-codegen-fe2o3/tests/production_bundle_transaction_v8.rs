const PIPELINE: &str = include_str!("../src/production_pipeline.rs");
const TRANSACTION: &str = include_str!("../src/production_bundle_transaction_v8.rs");

fn protected_publication_body() -> &'static str {
    PIPELINE
        .split("    fn publish_worker_handoff(")
        .nth(1)
        .expect("protected publication method")
        .split("\nfn require_complete_simulation_debug_source_capture_v2(")
        .next()
        .expect("bounded protected publication method")
}

#[test]
fn protected_v13_publication_binds_bundle_after_w4_and_before_execution_acquisition() {
    let body = protected_publication_body();
    let w4 = body
        .find("execute_w4_capability_witness")
        .expect("live W4 execution");
    let capability = body
        .find("bind_capability_handoff")
        .expect("Bundle V8 to V5/W4 binding");
    let publish = body
        .find("publish_compiler_capability_handoff_v5")
        .expect("native V5 publication");
    let inert_bundle = body
        .find(".inert_simulation_bundle()")
        .expect("revalidated inert Bundle V8 custody");
    let transaction = body
        .find("bind_publication(&receipt)")
        .expect("durable V5 transaction binding");
    let subject = body
        .find("from_capability_publication_v5")
        .expect("compiler-execution subject construction");
    let subject_binding = body
        .find("bind_compiler_execution_subject")
        .expect("Bundle V8 compiler-subject binding");
    let acquire = body
        .find(".acquire(subject.clone())")
        .expect("protected compiler execution acquisition");
    let transport = body
        .find("publish_compiler_execution_receipt_transport_for_capability_v5")
        .expect("native V5 receipt transport");

    assert!(
        w4 < capability
            && capability < inert_bundle
            && inert_bundle < publish
            && publish < transaction
            && transaction < subject
            && subject < subject_binding
            && subject_binding < acquire
            && acquire < transport
    );
}

#[test]
fn protected_bundle_path_has_no_legacy_fallback_or_side_file_publication() {
    let body = protected_publication_body();
    for forbidden in [
        "publish_compiler_module_handoff_v3",
        "InertCompilerExecutionSubjectV1::from_publication(",
        "publish_compiler_execution_receipt_transport_v1(",
        "into_simulation_bundle_v8()",
        "std::fs",
        "File::create",
        "OpenOptions",
    ] {
        assert!(
            !body.contains(forbidden),
            "protected Bundle V8 path contains forbidden operation {forbidden}"
        );
    }
    assert!(body.contains("ProductionTargetVerificationCustody::Legacy"));
    assert!(body.contains("UnexpectedLegacyBundle"));
}

#[test]
fn extraction_and_protected_publication_share_one_v8_builder() {
    assert_eq!(
        PIPELINE.matches("fn prepare_simulation_bundle_v8(").count(),
        1
    );
    assert_eq!(
        PIPELINE.matches(".prepare_simulation_bundle_v8()").count(),
        2,
        "extraction and protected publication must consume the same builder"
    );

    let extraction = PIPELINE
        .split("    fn into_simulation_bundle_v8(")
        .nth(1)
        .expect("V8 extraction boundary")
        .split("    pub(crate) fn into_inert_worker_handoff_for_extraction(")
        .next()
        .expect("bounded V8 extraction boundary");
    assert!(extraction.contains("compiler_custody\n            .is_extraction_only()"));
    assert!(extraction.contains("self.prepare_simulation_bundle_v8()"));

    let builder = PIPELINE
        .split("    fn prepare_simulation_bundle_v8(")
        .nth(1)
        .expect("shared V8 builder")
        .split("    fn into_simulation_bundle_v8(")
        .next()
        .expect("bounded shared V8 builder");
    assert!(!builder.contains("is_extraction_only"));
    assert!(builder.contains("require_v13_mut"));
    assert!(builder.contains("prepare_simulation_bundle_v8_graph"));
}

#[test]
fn typed_join_checks_every_cross_transaction_axis() {
    for required in [
        "bundle.revalidate()?",
        "rustc_identity_inventory_receipt_sha256",
        "rustc_preflight_plan_receipt_sha256",
        "self.bundle.semantic_mir()",
        "self.bundle.target()",
        "self.bundle.canonical_kir_v13()",
        "report.final_graph()",
        "report.final_epoch()",
        "report.w4_witness().identity()",
        "InertSimulationBundleV8::from_verified_canonical_bytes",
        "receipt.attempt()",
        "receipt.handoff_identity()",
        "receipt.transaction_identity()",
        "subject.transaction_identity()",
        ".matches_canonical_bytes(subject.canonical_bytes())",
    ] {
        assert!(
            TRANSACTION.contains(required),
            "production Bundle V8 join omits {required}"
        );
    }
    assert!(!TRANSACTION.contains("#[derive(Clone)]\npub(super) struct"));
}
