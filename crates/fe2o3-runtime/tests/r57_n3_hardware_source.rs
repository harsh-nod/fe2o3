//! Source-shape gate for the hardware-only R57 N3 qualification lane.

const EXAMPLE: &str = include_str!("../examples/gfx942-runtime-r57-n3-qualification.rs");
const AUTHORITY: &str = include_str!("../src/qualification_gfx942_r57_n3_v1.rs");
const BACKEND: &str = include_str!("../src/kfd_backend.rs");
const POLICY: &str = include_str!("../fixtures/trusted-gfx942-r57-n3-v2/policy-v2.txt");
const HISTORICAL_POLICY: &str = include_str!("../fixtures/trusted-gfx942-r57-n3-v1/policy-v1.txt");

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
        .find("mixed-memory roster unexpectedly succeeded")
        .expect("prepublication rejection branch");
    let negative = EXAMPLE
        .split("let invalid_arguments =")
        .nth(1)
        .expect("V2 negative roster")
        .split("upload_full_h2d(")
        .next()
        .unwrap();
    assert!(negative.contains("Gfx942R57N3QualificationArgumentsV2::new(\n                self.allocations[0],\n                self.allocations[1],\n                self.upload,"));
    assert!(negative.contains("&invalid_arguments,"));
    assert!(negative.contains("Err(RuntimeErrorV1::BackendRejected(error))"));
    assert!(negative.contains("error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch"));
    let exact_reason = POLICY
        .lines()
        .find_map(|line| line.strip_prefix("expected-rejection-detail="))
        .unwrap();
    let normalized_negative = negative
        .split_ascii_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(normalized_negative.contains(&format!("error.detail() == \"{exact_reason}\"")));
    assert!(negative.contains("mixed-memory roster produced the wrong rejection"));
    assert!(negative.contains("Ok(_) => return Err("));
    assert!(negative.contains("authorization_calls_v1() != 0"));
    assert!(negative.contains("mixed-memory roster reached final launch authority"));
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
    assert_eq!(EXAMPLE.matches("flush_stream(self.stream)").count(), 2);
    let first_launch = EXAMPLE
        .find("let mut first = self")
        .expect("first admitted launch");
    assert!(first_d_upload < first_launch);
    assert!(EXAMPLE.contains("let first_arguments = Gfx942R57N3QualificationArgumentsV2::new(\n                self.allocations[0],\n                self.allocations[1],\n                self.allocations[2],"));
    assert!(EXAMPLE.contains("let second_arguments = Gfx942R57N3QualificationArgumentsV2::new(\n                self.allocations[2],\n                self.allocations[1],\n                self.allocations[3],"));
    assert!(EXAMPLE[first_launch..].contains("&first_arguments,"));
    let first_flush = EXAMPLE[first_launch..]
        .find("flush_stream(self.stream)")
        .map(|offset| first_launch + offset)
        .expect("first admitted flush");
    let first_wait = EXAMPLE[first_flush..]
        .find(".wait(&mut first")
        .map(|offset| first_flush + offset)
        .expect("first admitted wait");
    let second_launch = EXAMPLE
        .find("let mut second = self")
        .expect("second admitted launch");
    let second_flush = EXAMPLE[second_launch..]
        .find("flush_stream(self.stream)")
        .map(|offset| second_launch + offset)
        .expect("second admitted flush");
    assert!(EXAMPLE[second_launch..].contains("&second_arguments,"));
    let second_wait = EXAMPLE[second_flush..]
        .find(".wait(&mut second")
        .map(|offset| second_flush + offset)
        .expect("second admitted wait");
    assert!(first_launch < first_flush && first_flush < first_wait);
    assert!(first_wait < second_launch);
    assert!(second_launch < second_flush && second_flush < second_wait);
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
    assert!(EXAMPLE.contains("let profile = GFX942_R57_N3_QUALIFICATION_PROFILE_ID_V2;"));
    assert!(EXAMPLE.contains("let policy = hex(GFX942_R57_N3_QUALIFICATION_POLICY_SHA256_V2);"));
    assert!(EXAMPLE.contains("PASS schema={profile} policy_sha256={policy}"));
    assert!(EXAMPLE.contains("open_gfx942_r57_n3_qualification_v2(device_unique_id)"));
    assert!(!EXAMPLE.contains("admit_gfx942_r57_n3_qualification_v1"));
    assert!(!EXAMPLE.contains("Gfx942R57N3QualificationArgumentsV1"));
    assert!(EXAMPLE.contains("cleanup=complete"));
}

#[test]
fn gate_is_independent_bounded_and_does_not_modify_the_old_vecadd_policy() {
    assert!(AUTHORITY.contains("GFX942_R57_N3_QUALIFICATION_POLICY_SHA256_V1"));
    assert!(AUTHORITY.contains("GFX942_R57_N3_QUALIFICATION_POLICY_SHA256_V2"));
    assert!(AUTHORITY.contains("QualificationPhaseV1::First"));
    assert!(AUTHORITY.contains("QualificationPhaseV1::Second"));
    assert!(AUTHORITY.contains("QualificationPhaseV1::Complete"));
    assert!(AUTHORITY.contains("request.allocations.len() == 3"));
    assert!(AUTHORITY.contains("RuntimeMemoryKindV1::DeviceLocal"));
    assert!(AUTHORITY.contains("ids[0] != c"));
    assert!(AUTHORITY.contains("ids[1] != b"));
    assert!(
        AUTHORITY.contains("initial_content_matches_v1(&request, ids, &self.initial_sha256[..3])")
    );
    assert!(
        AUTHORITY
            .contains("allocation_content_matches_v1(&request, ids[1], self.initial_sha256[1])")
    );
    assert!(
        AUTHORITY
            .contains("allocation_content_matches_v1(&request, ids[2], self.initial_sha256[3])")
    );
    assert!(AUTHORITY.contains("allocation.content_sha256 == Some(digest)"));
    assert!(AUTHORITY.contains("<[u8; 32]>::from(Sha256::digest(allocation.bytes)) == digest"));
    assert!(AUTHORITY.contains("request.signature == profile.signature()"));
    assert!(BACKEND.contains("ExactGfx942R57N3"));
    assert!(BACKEND.contains("ExactGfx942R57N3V2"));
    assert!(BACKEND.contains("ExactGfx942Vecadd"));
    assert!(POLICY.contains("expected-rejection-authority-calls=0"));
    assert!(HISTORICAL_POLICY.contains("expected-rejection=launch(a,b,c)-before-initial-c-h2d"));
    assert!(POLICY.contains("profile=fe2o3.runtime.gfx942-r57-n3-qualification.v2"));
    assert!(
        POLICY.contains(
            "expected-rejection=launch(a,b,host-visible-upload)-before-initial-c-and-d-h2d"
        )
    );
    assert!(POLICY.contains("phase-1=launch(a,b,c),expected-c=a+b"));
    assert!(POLICY.contains("phase-2=launch(c,b,d),expected-d=c+b"));
}
