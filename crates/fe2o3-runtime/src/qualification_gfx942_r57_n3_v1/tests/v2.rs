use super::*;

#[test]
fn versioned_policy_identities_and_request_signatures_do_not_alias() {
    assert_eq!(
        Sha256::digest(POLICY_BYTES_V2).as_slice(),
        GFX942_R57_N3_QUALIFICATION_POLICY_SHA256_V2
    );
    assert_ne!(
        GFX942_R57_N3_QUALIFICATION_SIGNATURE_V1,
        GFX942_R57_N3_QUALIFICATION_SIGNATURE_V2
    );
    assert_eq!(
        Gfx942R57N3QualificationArgumentsV2::SIGNATURE_V1,
        GFX942_R57_N3_QUALIFICATION_SIGNATURE_V2
    );
    assert!(
        gfx942_r57_n3_qualification_policy_v2().starts_with(
            format!("profile={GFX942_R57_N3_QUALIFICATION_PROFILE_ID_V2}\n").as_bytes()
        )
    );
    let v1 = admit_gfx942_r57_n3_qualification_v1().unwrap();
    let v2 = admit_gfx942_r57_n3_qualification_v2().unwrap();
    let buffers = v2.host_buffers().unwrap();
    let kernarg = gfx942_r57_n3_qualification_explicit_kernarg_v1();
    let abi = authority_abi_v1();
    let bindings = bindings_v1([10, 20, 30]);
    let allocations = initial_allocations(&buffers);
    let request = request_v1(&kernarg, &abi, &bindings, &allocations);
    assert!(!v2.authorizes_kfd_request_v1(request));
    let request_v2 = KfdRuntimeAuthorityRequestV1 {
        signature: GFX942_R57_N3_QUALIFICATION_SIGNATURE_V2,
        ..request
    };
    assert!(!v1.authorizes_kfd_request_v1(request_v2));
    assert_eq!(v1.observation_v1().authorization_calls_v1(), 1);
    assert_eq!(v2.observation_v1().authorization_calls_v1(), 1);
    assert!(v1.authorizes_kfd_request_v1(request));
    assert!(v2.authorizes_kfd_request_v1(request_v2));
    assert_eq!(v2.hsaco(), v1.hsaco());
    assert_eq!(v2.kernel_name(), v1.kernel_name());
}

#[test]
fn both_profiles_reject_malformed_first_without_advancing_phase() {
    let buffers = gfx942_r57_n3_qualification_host_buffers_v1().unwrap();
    let kernarg = gfx942_r57_n3_qualification_explicit_kernarg_v1();
    let abi = authority_abi_v1();
    let bindings = bindings_v1([10, 20, 30]);
    for profile in [QualificationProfile::V1, QualificationProfile::V2] {
        for case in 0..11 {
            let admitted = admit_qualification_profile(profile).unwrap();
            let mut allocations = initial_allocations(&buffers);
            let mut bad_kernarg = kernarg;
            bad_kernarg[8] ^= 1;
            let mut bad_abi = abi;
            bad_abi[2].access = ArgumentAccess::ReadWrite;
            match case {
                0..=2 => allocations[case].content_sha256 = None,
                3..=5 => allocations[case - 3].bytes = buffers.d_initial(),
                6 => allocations[2].kind = RuntimeMemoryKindV1::HostVisible,
                _ => {}
            }
            let mut request = request_v1(&kernarg, &abi, &bindings, &allocations);
            request.signature = profile.signature();
            match case {
                7 => request.geometry.grid[0] -= 1,
                8 => request.dispatch_abi = &bad_abi,
                9 => request.explicit_kernarg = &bad_kernarg,
                10 => request.complete_kernarg_template = &bad_kernarg,
                _ => {}
            }
            assert!(
                !admitted.authorizes_kfd_request_v1(request),
                "{profile:?} case {case}"
            );
            assert!(matches!(
                *admitted.state.phase.lock().unwrap(),
                QualificationPhaseV1::First
            ));
            assert_eq!(admitted.state.calls.load(Ordering::Acquire), 1);
            let good_allocations = initial_allocations(&buffers);
            let good = KfdRuntimeAuthorityRequestV1 {
                signature: profile.signature(),
                ..request_v1(&kernarg, &abi, &bindings, &good_allocations)
            };
            assert!(admitted.authorizes_kfd_request_v1(good));
            assert_eq!(admitted.state.calls.load(Ordering::Acquire), 2);
        }
    }
}

#[test]
fn both_profiles_retain_second_phase_content_and_one_shot_checks() {
    let buffers = gfx942_r57_n3_qualification_host_buffers_v1().unwrap();
    let kernarg = gfx942_r57_n3_qualification_explicit_kernarg_v1();
    let abi = authority_abi_v1();
    let first_bindings = bindings_v1([10, 20, 30]);
    let first_allocations = initial_allocations(&buffers);
    let second_bindings = bindings_v1([30, 20, 40]);
    let second_allocations = [
        authority_allocation_v1(30, buffers.c_initial(), None),
        authority_allocation_v1(20, buffers.b(), Some(Sha256::digest(buffers.b()).into())),
        authority_allocation_v1(
            40,
            buffers.d_initial(),
            Some(Sha256::digest(buffers.d_initial()).into()),
        ),
    ];
    for profile in [QualificationProfile::V1, QualificationProfile::V2] {
        let admitted = admit_qualification_profile(profile).unwrap();
        let first = KfdRuntimeAuthorityRequestV1 {
            signature: profile.signature(),
            ..request_v1(&kernarg, &abi, &first_bindings, &first_allocations)
        };
        let second = KfdRuntimeAuthorityRequestV1 {
            signature: profile.signature(),
            ..request_v1(&kernarg, &abi, &second_bindings, &second_allocations)
        };
        assert!(!admitted.authorizes_kfd_request_v1(second));
        assert!(admitted.authorizes_kfd_request_v1(first));
        for case in 0..8 {
            let mut allocations = second_allocations;
            let mut bindings = second_bindings;
            match case {
                0 => {
                    allocations[0].content_sha256 = Some(Sha256::digest(buffers.c_initial()).into())
                }
                1 => allocations[1].content_sha256 = None,
                2 => allocations[2].content_sha256 = None,
                3 => allocations[1].bytes = buffers.a(),
                4 => allocations[2].bytes = buffers.c_initial(),
                5..=7 => {
                    let index = case - 5;
                    bindings[index].region.allocation = 10;
                    allocations[index].allocation = 10;
                }
                _ => unreachable!(),
            }
            assert!(
                !admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                    bindings: &bindings,
                    allocations: &allocations,
                    ..second
                }),
                "{profile:?} case {case}"
            );
            assert!(matches!(
                *admitted.state.phase.lock().unwrap(),
                QualificationPhaseV1::Second { .. }
            ));
        }
        assert!(admitted.authorizes_kfd_request_v1(second));
        assert!(!admitted.authorizes_kfd_request_v1(second));
        assert_eq!(admitted.state.calls.load(Ordering::Acquire), 12);
    }
}

fn initial_allocations(
    buffers: &Gfx942R57N3QualificationHostBuffersV1,
) -> [KfdRuntimeAuthorityAllocationV1<'_>; 3] {
    let images = [buffers.a(), buffers.b(), buffers.c_initial()];
    core::array::from_fn(|index| {
        authority_allocation_v1(
            [10, 20, 30][index],
            images[index],
            Some(Sha256::digest(images[index]).into()),
        )
    })
}
