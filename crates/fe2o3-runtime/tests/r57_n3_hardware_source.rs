//! Source-shape gate for the hardware-only R57 N3 qualification lane.

const EXAMPLE: &str = include_str!("../examples/gfx942-runtime-r57-n3-qualification.rs");
const AUTHORITY: &str = include_str!("../src/qualification_gfx942_r57_n3_v1.rs");
const BACKEND: &str = include_str!("../src/kfd_backend.rs");
const POLICY: &str = include_str!("../fixtures/trusted-gfx942-r57-n3-v1/policy-v1.txt");

#[test]
fn live_lane_has_one_reject_two_launches_four_readbacks_and_explicit_cleanup() {
    let a_upload = EXAMPLE
        .find("upload_full_h2d(&mut context, stream, upload, allocations[0], &a)")
        .expect("A authenticated H2D initialization");
    let b_upload = EXAMPLE
        .find("upload_full_h2d(&mut context, stream, upload, allocations[1], &b)")
        .expect("B authenticated H2D initialization");
    let first_c_upload = EXAMPLE
        .find("self.allocations[2],\n                &self.initial[2]")
        .expect("C authenticated H2D initialization");
    let first_d_upload = EXAMPLE
        .find("self.allocations[3],\n                &self.initial[3]")
        .expect("D authenticated H2D initialization");
    let rejected_launch = EXAMPLE
        .find("uninitialized C launch unexpectedly succeeded")
        .expect("prepublication rejection branch");
    assert!(a_upload < b_upload);
    assert!(b_upload < rejected_launch);
    assert!(rejected_launch < first_c_upload);
    assert!(first_c_upload < first_d_upload);
    assert_eq!(
        EXAMPLE.matches("RuntimeMemoryKindV1::DeviceLocal").count(),
        1
    );
    assert_eq!(
        EXAMPLE.matches("RuntimeMemoryKindV1::HostVisible").count(),
        1
    );
    assert!(EXAMPLE.contains("for _ in 0..4"));
    let upload_helper = EXAMPLE
        .split("fn upload_full_h2d(")
        .nth(1)
        .expect("authenticated H2D helper")
        .split("struct QualifiedRunV1")
        .next()
        .expect("bounded authenticated H2D helper");
    assert!(upload_helper.contains("write_allocation(upload"));
    assert!(upload_helper.contains(".copy_async("));
    assert!(upload_helper.contains("full_region(upload, RuntimeAccessV1::Read)"));
    assert!(upload_helper.contains("full_region(destination, RuntimeAccessV1::Write)"));
    assert!(upload_helper.contains("flush_stream(stream)"));
    assert!(upload_helper.contains(".wait(&mut submission"));
    assert!(upload_helper.contains("release_submission(submission)"));
    assert_eq!(EXAMPLE.matches("upload_full_h2d(").count(), 5);
    assert!(EXAMPLE.contains("authorization_calls_v1() != 0"));
    assert!(EXAMPLE.contains("authorization_calls_v1() != 2"));
    assert_eq!(EXAMPLE.matches(".launch(").count(), 3);
    assert!(
        EXAMPLE
            .contains("for (allocation, bytes) in self.allocations.into_iter().zip(&mut observed)")
    );
    assert!(EXAMPLE.contains("exact_bytes(\"A\""));
    assert!(EXAMPLE.contains("exact_bytes(\"B\""));
    assert!(EXAMPLE.contains("exact_bytes(\"C\""));
    assert!(EXAMPLE.contains("exact_bytes(\"D\""));
    assert!(EXAMPLE.contains("KfdRuntimeLaunchDataPathV1::PersistentDeviceReused"));
    assert!(EXAMPLE.contains("performance.user_data_materializations() != 0"));
    assert!(EXAMPLE.contains("performance.persistent_control_reused()"));
    assert!(EXAMPLE.contains("release_submission(first)"));
    assert!(EXAMPLE.contains("release_submission(second)"));
    assert!(EXAMPLE.contains("release_allocation(allocation)"));
    assert!(EXAMPLE.contains("release_allocation(self.upload)"));
    assert!(EXAMPLE.contains("unload_module(self.module)"));
    assert!(EXAMPLE.contains("destroy_stream(self.stream)"));
    assert!(EXAMPLE.contains("self.context.shutdown()"));
    assert!(EXAMPLE.contains("backend.shutdown_native_v1()"));
    assert_eq!(EXAMPLE.matches("\n        println!(").count(), 1);
    assert!(EXAMPLE.contains("PASS schema=fe2o3.runtime.gfx942-r57-n3-qualification.v1"));
    assert!(EXAMPLE.contains("cleanup=complete"));
}

#[test]
fn gate_is_independent_bounded_and_does_not_modify_the_old_vecadd_policy() {
    assert!(AUTHORITY.contains("GFX942_R57_N3_QUALIFICATION_POLICY_SHA256_V1"));
    assert!(AUTHORITY.contains("QualificationPhaseV1::First"));
    assert!(AUTHORITY.contains("QualificationPhaseV1::Second"));
    assert!(AUTHORITY.contains("QualificationPhaseV1::Complete"));
    assert!(AUTHORITY.contains("request.allocations.len() == 3"));
    assert!(AUTHORITY.contains("RuntimeMemoryKindV1::DeviceLocal"));
    assert!(AUTHORITY.contains("ids[0] != c"));
    assert!(AUTHORITY.contains("ids[1] != b"));
    assert!(BACKEND.contains("ExactGfx942R57N3"));
    assert!(BACKEND.contains("ExactGfx942Vecadd"));
    assert!(POLICY.contains("expected-rejection-authority-calls=0"));
    assert!(POLICY.contains("phase-1=launch(a,b,c),expected-c=a+b"));
    assert!(POLICY.contains("phase-2=launch(c,b,d),expected-d=c+b"));
}
