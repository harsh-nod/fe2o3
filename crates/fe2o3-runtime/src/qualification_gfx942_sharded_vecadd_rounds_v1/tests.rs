use super::*;
use crate::qualification_gfx942_vecadd_v1::GFX942_VECADD_QUALIFICATION_ARGUMENTS_V1;
use crate::{
    BackendBindingV1, BackendMemoryRegionV1, KfdRuntimeAuthorityAllocationV1,
    KfdRuntimeAuthorityGlobalBufferV1, KfdRuntimeSemanticLaunchV1, RuntimeAccessV1,
    RuntimeMemoryKindV1,
};
use fe2o3_hsaco::ArgumentAccess;

fn with_request(
    admitted: &AdmittedGfx942ShardedVecaddRoundsQualificationV1,
    round: usize,
    ids: [u64; 3],
    run: impl FnOnce(KfdRuntimeAuthorityRequestV1<'_>),
) {
    let recipe = admitted.recipe(round).unwrap();
    let buffers = admitted.host_buffers(round).unwrap();
    let kernarg = recipe.explicit_kernarg();
    let policies = GFX942_VECADD_QUALIFICATION_ARGUMENTS_V1;
    let bindings: [_; 3] = core::array::from_fn(|index| BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation: ids[index],
            access: policies[index].access,
            byte_offset: 0,
            byte_len: recipe.padded_bytes() as u64,
        },
        kernarg_byte_offset: policies[index].pointer_offset,
    });
    let abi = policies.map(|policy| KfdRuntimeAuthorityGlobalBufferV1 {
        explicit_argument_index: policy.explicit_argument_index,
        name: policy.name,
        kernarg_byte_offset: u64::from(policy.pointer_offset),
        pointee_alignment: policy.reconciled_pointee_alignment,
        access: if policy.access == RuntimeAccessV1::Read {
            ArgumentAccess::ReadOnly
        } else {
            ArgumentAccess::WriteOnly
        },
    });
    let bytes = [buffers.a(), buffers.b(), buffers.c_initial()];
    let allocations: [_; 3] = core::array::from_fn(|index| KfdRuntimeAuthorityAllocationV1 {
        allocation: ids[index],
        kind: RuntimeMemoryKindV1::DeviceLocal,
        alignment: 4096,
        byte_offset: 0,
        bytes: bytes[index],
        content_sha256: Some(Sha256::digest(bytes[index]).into()),
    });
    run(KfdRuntimeAuthorityRequestV1 {
        module_image: admitted.hsaco(),
        module_sha256: admitted.hsaco_sha256(),
        kernel_name: admitted.kernel_name(),
        signature: admitted.signature(),
        explicit_kernarg: &kernarg,
        complete_kernarg_template: &kernarg,
        bindings: &bindings,
        dispatch_abi: &abi,
        allocations: &allocations,
        geometry: recipe.geometry(),
        semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
    });
}

#[track_caller]
fn denied(
    admitted: &AdmittedGfx942ShardedVecaddRoundsQualificationV1,
    request: KfdRuntimeAuthorityRequestV1<'_>,
    phase: usize,
) {
    let observation = admitted.observation_v1();
    let calls = observation.authorization_calls_v1();
    assert!(!admitted.authorizes_kfd_request_v1(request));
    assert_eq!(observation.authorization_calls_v1(), calls + 1);
    assert_eq!(observation.accepted_rounds_v1(), Some(phase));
}

fn accept(admitted: &AdmittedGfx942ShardedVecaddRoundsQualificationV1, round: usize) {
    with_request(admitted, round, [11, 22, 33], |request| {
        assert!(admitted.authorizes_kfd_request_v1(request));
    });
    assert_eq!(
        admitted.observation_v1().accepted_rounds_v1(),
        Some(round + 1)
    );
}

#[test]
fn all_thirty_five_authorities_accept_exactly_two_ordered_recipes() {
    let mut accepted = 0;
    for count in 2..=8 {
        for index in 0..count {
            let admitted =
                admit_gfx942_sharded_vecadd_rounds_qualification_v1(count, index).unwrap();
            let observation = admitted.observation_v1();
            assert_eq!(observation.accepted_rounds_v1(), Some(0));
            for round in 0..2 {
                let recipe = admitted.recipe(round).unwrap();
                assert_eq!(
                    (recipe.count(), recipe.index(), recipe.round()),
                    (count, index, round)
                );
                assert_eq!(recipe.explicit_kernarg(), admitted.explicit_kernarg());
                assert_eq!(recipe.geometry(), admitted.geometry());
            }
            with_request(&admitted, 1, [11, 22, 33], |request| {
                denied(&admitted, request, 0)
            });
            accept(&admitted, 0);
            with_request(&admitted, 0, [11, 22, 33], |request| {
                denied(&admitted, request, 1)
            });
            accept(&admitted, 1);
            for round in 0..2 {
                with_request(&admitted, round, [11, 22, 33], |request| {
                    denied(&admitted, request, 2)
                });
            }
            assert_eq!(observation.accepted_rounds_v1(), Some(2));
            assert_eq!(observation.authorization_calls_v1(), 6);
            accepted += 2;
        }
    }
    assert_eq!(accepted, 70);
}

#[test]
fn invalid_domain_or_policy_and_foreign_signatures_do_not_admit() {
    for (count, index) in [
        (0, 0),
        (1, 0),
        (9, 0),
        (usize::MAX, 0),
        (2, 2),
        (8, usize::MAX),
    ] {
        assert!(admit_gfx942_sharded_vecadd_rounds_qualification_v1(count, index).is_err());
    }
    let admitted = admit_gfx942_sharded_vecadd_rounds_qualification_v1(2, 0).unwrap();
    assert!(admitted.recipe(2).is_err());
    assert!(admitted.host_buffers(usize::MAX).is_err());
    let inner_policy = one_shot::gfx942_sharded_vecadd_qualification_policy_v1();
    validate_policy_v1(POLICY, inner_policy).unwrap();
    for index in 0..2 {
        let mut changed = [POLICY.to_vec(), inner_policy.to_vec()];
        changed[index][0] ^= 1;
        assert_eq!(
            validate_policy_v1(&changed[0], &changed[1]),
            Err(Gfx942ShardedVecaddRoundsQualificationAdmissionErrorV1::Identity)
        );
    }
    assert_eq!(
        Gfx942ShardedVecaddRoundsQualificationArgumentsV1::SIGNATURE_V1,
        <[u8; 32]>::from(Sha256::digest(POLICY))
    );
    assert!(std::str::from_utf8(POLICY).unwrap().contains(&format!(
        "profile={GFX942_SHARDED_VECADD_ROUNDS_QUALIFICATION_PROFILE_ID_V1}\n"
    )));
    with_request(&admitted, 0, [11, 22, 33], |request| {
        let original = one_shot::admit_gfx942_sharded_vecadd_qualification_v1(2, 0, 0).unwrap();
        assert!(!original.authorizes_kfd_request_v1(request));
        assert!(!original.observation_v1().accepted_v1());
        for signature in [
            original.signature(),
            crate::qualification_gfx942_vecadd_v1::GFX942_VECADD_QUALIFICATION_SIGNATURE_V1,
            crate::qualification_gfx942_r57_n3_v1::GFX942_R57_N3_QUALIFICATION_SIGNATURE_V1,
            crate::qualification_gfx942_r57_n3_v1::GFX942_R57_N3_QUALIFICATION_SIGNATURE_V2,
            [0; 32],
        ] {
            assert_ne!(signature, request.signature);
            denied(
                &admitted,
                KfdRuntimeAuthorityRequestV1 {
                    signature,
                    ..request
                },
                0,
            );
        }
        assert!(
            original.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                signature: original.signature(),
                ..request
            })
        );
        assert!(admitted.authorizes_kfd_request_v1(request));
    });
}

#[test]
fn second_round_requires_same_three_identities_even_with_otherwise_exact_inputs() {
    let admitted = admit_gfx942_sharded_vecadd_rounds_qualification_v1(3, 1).unwrap();
    accept(&admitted, 0);
    for binding in 0..3 {
        let mut ids = [11, 22, 33];
        ids[binding] += 100;
        with_request(&admitted, 1, ids, |request| denied(&admitted, request, 1));
    }
    for ids in [[22, 11, 33], [11, 33, 22], [11, 11, 33], [0, 22, 33]] {
        with_request(&admitted, 1, ids, |request| denied(&admitted, request, 1));
    }
    assert!(!admitted.rounds[1].observation_v1().accepted_v1());
    accept(&admitted, 1);
}

#[test]
fn request_forwarding_preserves_every_artifact_abi_and_geometry_check_in_both_phases() {
    for round in 0..2 {
        let admitted = admit_gfx942_sharded_vecadd_rounds_qualification_v1(3, 1).unwrap();
        if round == 1 {
            accept(&admitted, 0);
        }
        with_request(&admitted, round, [11, 22, 33], |request| {
            let mut image = request.module_image.to_vec();
            image[0] ^= 1;
            denied(
                &admitted,
                KfdRuntimeAuthorityRequestV1 {
                    module_image: &image,
                    ..request
                },
                round,
            );
            denied(
                &admitted,
                KfdRuntimeAuthorityRequestV1 {
                    module_sha256: [0; 32],
                    ..request
                },
                round,
            );
            denied(
                &admitted,
                KfdRuntimeAuthorityRequestV1 {
                    kernel_name: "other",
                    ..request
                },
                round,
            );
            denied(
                &admitted,
                KfdRuntimeAuthorityRequestV1 {
                    explicit_kernarg: &request.explicit_kernarg[..47],
                    ..request
                },
                round,
            );
            denied(
                &admitted,
                KfdRuntimeAuthorityRequestV1 {
                    complete_kernarg_template: &request.complete_kernarg_template[..47],
                    ..request
                },
                round,
            );
            for index in 0..48 {
                let mut bytes = request.explicit_kernarg.to_vec();
                bytes[index] ^= 1;
                denied(
                    &admitted,
                    KfdRuntimeAuthorityRequestV1 {
                        explicit_kernarg: &bytes,
                        ..request
                    },
                    round,
                );
                denied(
                    &admitted,
                    KfdRuntimeAuthorityRequestV1 {
                        complete_kernarg_template: &bytes,
                        ..request
                    },
                    round,
                );
            }
            for index in 0..7 {
                let mut geometry = request.geometry;
                match index {
                    0..=2 => geometry.grid[index] += 1,
                    3..=5 => geometry.workgroup[index - 3] += 1,
                    _ => geometry.dynamic_shared_bytes = 4,
                }
                denied(
                    &admitted,
                    KfdRuntimeAuthorityRequestV1 {
                        geometry,
                        ..request
                    },
                    round,
                );
            }
            let semantic_launch =
                KfdRuntimeSemanticLaunchV1::Atomic(crate::RuntimeAtomicLaunchContractV1 {
                    operation: crate::RuntimeAtomicOperationV1::Add,
                    scope: crate::RuntimeMemoryScopeV1::Workgroup,
                    order: crate::RuntimeMemoryOrderV1::Relaxed,
                    failure_order: None,
                    weak: false,
                    geometry: request.geometry,
                });
            denied(
                &admitted,
                KfdRuntimeAuthorityRequestV1 {
                    semantic_launch,
                    ..request
                },
                round,
            );
            denied(
                &admitted,
                KfdRuntimeAuthorityRequestV1 {
                    dispatch_abi: &request.dispatch_abi[..2],
                    ..request
                },
                round,
            );
            for index in 0..3 {
                for field in 0..5 {
                    let mut abi = request.dispatch_abi.to_vec();
                    match field {
                        0 => abi[index].explicit_argument_index += 1,
                        1 => abi[index].name = "other",
                        2 => abi[index].kernarg_byte_offset += 4,
                        3 => abi[index].pointee_alignment = 4,
                        _ => abi[index].access = ArgumentAccess::ReadWrite,
                    }
                    denied(
                        &admitted,
                        KfdRuntimeAuthorityRequestV1 {
                            dispatch_abi: &abi,
                            ..request
                        },
                        round,
                    );
                }
            }
            assert!(admitted.authorizes_kfd_request_v1(request));
        });
    }
}

#[test]
fn request_forwarding_preserves_full_binding_content_and_padding_checks_in_both_phases() {
    for round in 0..2 {
        let admitted = admit_gfx942_sharded_vecadd_rounds_qualification_v1(2, 0).unwrap();
        if round == 1 {
            accept(&admitted, 0);
        }
        with_request(&admitted, round, [11, 22, 33], |request| {
            denied(
                &admitted,
                KfdRuntimeAuthorityRequestV1 {
                    bindings: &request.bindings[..2],
                    ..request
                },
                round,
            );
            denied(
                &admitted,
                KfdRuntimeAuthorityRequestV1 {
                    allocations: &request.allocations[..2],
                    ..request
                },
                round,
            );
            for index in 0..3 {
                for field in 0..7 {
                    let mut bindings = request.bindings.to_vec();
                    match field {
                        0 => bindings[index].region.allocation = 0,
                        1 => {
                            bindings[index].region.allocation =
                                bindings[(index + 1) % 3].region.allocation
                        }
                        2 => bindings[index].region.allocation += 100,
                        3 => bindings[index].region.access = RuntimeAccessV1::ReadWrite,
                        4 => bindings[index].region.byte_offset = 4,
                        5 => bindings[index].region.byte_len -= 4,
                        _ => bindings[index].kernarg_byte_offset += 4,
                    }
                    denied(
                        &admitted,
                        KfdRuntimeAuthorityRequestV1 {
                            bindings: &bindings,
                            ..request
                        },
                        round,
                    );
                }
                for field in 0..9 {
                    let mut allocations = request.allocations.to_vec();
                    match field {
                        0 => allocations[index].allocation += 100,
                        1 => {
                            allocations[index].allocation = allocations[(index + 1) % 3].allocation
                        }
                        2 => allocations[index].kind = RuntimeMemoryKindV1::HostVisible,
                        3 => allocations[index].alignment = 3,
                        4 => allocations[index].alignment = 2,
                        5 => allocations[index].byte_offset = 4,
                        6 => {
                            allocations[index].bytes =
                                &allocations[index].bytes[..allocations[index].bytes.len() - 4]
                        }
                        7 => allocations[index].content_sha256 = None,
                        _ => allocations[index].content_sha256 = Some([0; 32]),
                    }
                    denied(
                        &admitted,
                        KfdRuntimeAuthorityRequestV1 {
                            allocations: &allocations,
                            ..request
                        },
                        round,
                    );
                }
                for offset in [
                    0,
                    admitted.recipe(round).unwrap().elements() * 4,
                    request.allocations[index].bytes.len() - 1,
                ] {
                    let mut bytes = request.allocations[index].bytes.to_vec();
                    bytes[offset] ^= 1;
                    let mut allocations = request.allocations.to_vec();
                    allocations[index].bytes = &bytes;
                    denied(
                        &admitted,
                        KfdRuntimeAuthorityRequestV1 {
                            allocations: &allocations,
                            ..request
                        },
                        round,
                    );
                    allocations[index].content_sha256 = Some(Sha256::digest(&bytes).into());
                    denied(
                        &admitted,
                        KfdRuntimeAuthorityRequestV1 {
                            allocations: &allocations,
                            ..request
                        },
                        round,
                    );
                }
            }
            let mut allocations = request.allocations.to_vec();
            allocations.reverse();
            assert!(
                admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                    allocations: &allocations,
                    ..request
                })
            );
        });
    }
}

#[test]
fn other_shards_and_prior_computed_output_cannot_replace_fresh_round_inputs() {
    for round in 0..2 {
        let admitted = admit_gfx942_sharded_vecadd_rounds_qualification_v1(3, 0).unwrap();
        if round == 1 {
            accept(&admitted, 0);
        }
        with_request(&admitted, round, [11, 22, 33], |request| {
            for (count, index, wrong_round) in [(3, 1, round), (3, 0, 1 - round), (2, 0, round)] {
                let buffers =
                    Gfx942ShardedVecaddRoundsQualificationRecipeV1::new(count, index, wrong_round)
                        .unwrap()
                        .host_buffers()
                        .unwrap();
                let mut allocations = request.allocations.to_vec();
                for (allocation, bytes) in
                    allocations
                        .iter_mut()
                        .zip([buffers.a(), buffers.b(), buffers.c_initial()])
                {
                    allocation.bytes = bytes;
                    allocation.content_sha256 = Some(Sha256::digest(bytes).into());
                }
                denied(
                    &admitted,
                    KfdRuntimeAuthorityRequestV1 {
                        allocations: &allocations,
                        ..request
                    },
                    round,
                );
            }
            let prior = admitted.host_buffers(0).unwrap();
            let mut allocations = request.allocations.to_vec();
            allocations[2].bytes = prior.expected_c();
            allocations[2].content_sha256 = Some(Sha256::digest(prior.expected_c()).into());
            denied(
                &admitted,
                KfdRuntimeAuthorityRequestV1 {
                    allocations: &allocations,
                    ..request
                },
                round,
            );
            assert!(admitted.authorizes_kfd_request_v1(request));
        });
    }
}

#[test]
fn competing_requests_accept_once_per_phase_without_consuming_sibling_authorities() {
    let admitted = admit_gfx942_sharded_vecadd_rounds_qualification_v1(8, 7).unwrap();
    let sibling = admit_gfx942_sharded_vecadd_rounds_qualification_v1(8, 6).unwrap();
    for round in 0..2 {
        with_request(&admitted, round, [11, 22, 33], |request| {
            let barrier = std::sync::Barrier::new(8);
            let accepted = std::thread::scope(|scope| {
                let handles: Vec<_> = (0..8)
                    .map(|_| {
                        scope.spawn(|| {
                            barrier.wait();
                            admitted.authorizes_kfd_request_v1(request)
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .map(|handle| usize::from(handle.join().unwrap()))
                    .sum::<usize>()
            });
            assert_eq!(accepted, 1);
        });
        assert_eq!(
            admitted.observation_v1().accepted_rounds_v1(),
            Some(round + 1)
        );
        assert_eq!(sibling.observation_v1().accepted_rounds_v1(), Some(0));
        assert_eq!(sibling.observation_v1().authorization_calls_v1(), 0);
    }
    assert_eq!(admitted.observation_v1().authorization_calls_v1(), 16);
}

#[test]
fn poisoned_or_overflowed_authority_never_reopens_an_accepted_phase() {
    for accepted in 0..=1 {
        let admitted = admit_gfx942_sharded_vecadd_rounds_qualification_v1(2, 0).unwrap();
        if accepted == 1 {
            accept(&admitted, 0);
        }
        let outcome = std::panic::catch_unwind(|| {
            let _guard = admitted.state.phase.lock().unwrap();
            panic!("ambiguous authorization boundary");
        });
        assert!(outcome.is_err());
        assert_eq!(admitted.observation_v1().accepted_rounds_v1(), None);
        for round in 0..2 {
            with_request(&admitted, round, [11, 22, 33], |request| {
                assert!(!admitted.authorizes_kfd_request_v1(request));
            });
        }
        assert_eq!(
            admitted.rounds[0].observation_v1().accepted_v1(),
            accepted == 1
        );
        assert!(!admitted.rounds[1].observation_v1().accepted_v1());
        let admitted = admit_gfx942_sharded_vecadd_rounds_qualification_v1(2, 0).unwrap();
        if accepted == 1 {
            accept(&admitted, 0);
        }
        admitted.state.calls.store(u64::MAX, Ordering::Release);
        with_request(&admitted, accepted, [11, 22, 33], |request| {
            assert!(!admitted.authorizes_kfd_request_v1(request))
        });
        assert_eq!(
            admitted.observation_v1().accepted_rounds_v1(),
            Some(accepted)
        );
        assert_eq!(admitted.observation_v1().authorization_calls_v1(), u64::MAX);
    }
}
