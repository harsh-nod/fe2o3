use super::*;
use crate::{
    BackendBindingV1, BackendMemoryRegionV1, KfdRuntimeAuthorityAllocationV1,
    KfdRuntimeAuthorityGlobalBufferV1,
};

fn with_request(
    count: usize,
    index: usize,
    round: usize,
    run: impl FnOnce(&AdmittedGfx942ShardedVecaddQualificationV1, KfdRuntimeAuthorityRequestV1<'_>),
) {
    let admitted = admit_gfx942_sharded_vecadd_qualification_v1(count, index, round).unwrap();
    let buffers = admitted.host_buffers().unwrap();
    let kernarg = admitted.explicit_kernarg();
    let bindings: [_; 3] = core::array::from_fn(|index| BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation: (index + 1) as u64,
            access: ACCESS[index],
            byte_offset: 0,
            byte_len: admitted.recipe().padded_bytes() as u64,
        },
        kernarg_byte_offset: (index * 16) as u32,
    });
    let abi: [_; 3] = core::array::from_fn(|index| KfdRuntimeAuthorityGlobalBufferV1 {
        explicit_argument_index: index * 2,
        name: NAMES[index],
        kernarg_byte_offset: (index * 16) as u64,
        pointee_alignment: 1,
        access: ABI_ACCESS[index],
    });
    let allocations = [buffers.a(), buffers.b(), buffers.c_initial()];
    let allocations: [_; 3] = core::array::from_fn(|index| KfdRuntimeAuthorityAllocationV1 {
        allocation: (index + 1) as u64,
        kind: RuntimeMemoryKindV1::DeviceLocal,
        alignment: 4096,
        byte_offset: 0,
        bytes: allocations[index],
        content_sha256: Some(Sha256::digest(allocations[index]).into()),
    });
    run(
        &admitted,
        KfdRuntimeAuthorityRequestV1 {
            module_image: admitted.hsaco(),
            module_sha256: admitted.hsaco_sha256(),
            kernel_name: admitted.kernel_name(),
            signature: admitted.signature(),
            explicit_kernarg: &kernarg,
            complete_kernarg_template: &kernarg,
            bindings: &bindings,
            dispatch_abi: &abi,
            allocations: &allocations,
            geometry: admitted.geometry(),
            semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
        },
    );
}

fn denied(
    admitted: &AdmittedGfx942ShardedVecaddQualificationV1,
    request: KfdRuntimeAuthorityRequestV1<'_>,
) {
    let observation = admitted.observation_v1();
    let before = observation.authorization_calls_v1();
    assert!(!admitted.authorizes_kfd_request_v1(request));
    assert_eq!(observation.authorization_calls_v1(), before + 1);
    assert!(!observation.accepted_v1());
}

fn bits(bytes: &[u8], index: usize) -> u32 {
    u32::from_le_bytes(bytes[index * 4..index * 4 + 4].try_into().unwrap())
}

#[test]
fn all_seventy_recipes_partition_one_global_result_with_exact_padding_and_abi() {
    let mut recipes = 0;
    for count in 2..=8 {
        for round in 0..=1 {
            let mut offset = 0;
            let mut padded_total = 0;
            for index in 0..count {
                recipes += 1;
                let recipe =
                    Gfx942ShardedVecaddQualificationRecipeV1::new(count, index, round).unwrap();
                assert_eq!(
                    (recipe.count(), recipe.index(), recipe.round()),
                    (count, index, round)
                );
                let elements = 65_537 / count + usize::from(index < 65_537 % count);
                assert_eq!(recipe.elements(), elements);
                assert_eq!(recipe.global_offset(), offset);
                assert!(elements > 0);
                assert_eq!(recipe.padded_bytes(), (elements * 4).div_ceil(4096) * 4096);
                assert_eq!(
                    recipe.geometry().grid,
                    [(elements.div_ceil(256) * 256) as u32, 1, 1]
                );
                assert_eq!(recipe.geometry().workgroup, [256, 1, 1]);
                assert_eq!(recipe.geometry().dynamic_shared_bytes, 0);
                let kernarg = recipe.explicit_kernarg();
                for pointer in [0, 16, 32] {
                    assert_eq!(&kernarg[pointer..pointer + 8], &[0; 8]);
                    assert_eq!(
                        u64::from_le_bytes(kernarg[pointer + 8..pointer + 16].try_into().unwrap()),
                        elements as u64
                    );
                }
                let buffers = recipe.host_buffers().unwrap();
                for bytes in [
                    buffers.a(),
                    buffers.b(),
                    buffers.c_initial(),
                    buffers.expected_c(),
                ] {
                    assert_eq!(bytes.len(), recipe.padded_bytes());
                }
                for local in 0..recipe.padded_bytes() / 4 {
                    if local < elements {
                        let a = (offset + local + 131_072 * round) as f32;
                        let b = (3 + round) as f32;
                        assert_eq!(bits(buffers.a(), local), a.to_bits());
                        assert_eq!(bits(buffers.b(), local), b.to_bits());
                        assert_eq!(bits(buffers.expected_c(), local), (a + b).to_bits());
                        assert_eq!(
                            bits(buffers.expected_c(), local),
                            ((offset + local + 3 + 131_073 * round) as f32).to_bits()
                        );
                    } else {
                        assert_eq!(bits(buffers.a(), local), (-7.0f32).to_bits());
                        assert_eq!(bits(buffers.b(), local), (-11.0f32).to_bits());
                        assert_eq!(bits(buffers.expected_c(), local), (-1.0f32).to_bits());
                    }
                    assert_eq!(bits(buffers.c_initial(), local), (-1.0f32).to_bits());
                }
                offset += elements;
                padded_total += recipe.padded_bytes();
            }
            assert_eq!(offset, 65_537);
            assert!(padded_total <= 286_720);
        }
    }
    assert_eq!(recipes, 70);
    assert_eq!(
        Gfx942ShardedVecaddQualificationRecipeV1::new(2, 0, 0)
            .unwrap()
            .padded_bytes(),
        135_168
    );
    assert_eq!(
        Gfx942ShardedVecaddQualificationRecipeV1::new(2, 1, 0)
            .unwrap()
            .padded_bytes(),
        131_072
    );
}

#[test]
fn all_seventy_recipes_admit_once_and_observation_never_resets_authority() {
    let mut accepted = 0;
    for count in 2..=8 {
        for index in 0..count {
            for round in 0..=1 {
                with_request(count, index, round, |admitted, request| {
                    let observation = admitted.observation_v1();
                    assert_eq!(observation.authorization_calls_v1(), 0);
                    assert!(!observation.accepted_v1());
                    assert!(admitted.authorizes_kfd_request_v1(request));
                    assert!(observation.accepted_v1());
                    assert!(!admitted.authorizes_kfd_request_v1(request));
                    assert_eq!(observation.authorization_calls_v1(), 2);
                    accepted += 1;
                });
            }
        }
    }
    assert_eq!(accepted, 70);
}

#[test]
fn out_of_domain_recipes_and_modified_artifacts_are_rejected() {
    for count in [0, 1, 9, usize::MAX] {
        assert_eq!(
            Gfx942ShardedVecaddQualificationRecipeV1::new(count, 0, 0),
            Err(Gfx942ShardedVecaddQualificationAdmissionErrorV1::Recipe)
        );
    }
    for count in 2..=8 {
        for (index, round) in [(count, 0), (usize::MAX, 0), (0, 2), (0, usize::MAX)] {
            assert!(admit_gfx942_sharded_vecadd_qualification_v1(count, index, round).is_err());
        }
    }
    validate_artifacts_v1(SOURCE, POLICY, OBJECT).unwrap();
    for artifact in 0..3 {
        let mut changed = [SOURCE.to_vec(), POLICY.to_vec(), OBJECT.to_vec()];
        changed[artifact][0] ^= 1;
        assert_eq!(
            validate_artifacts_v1(&changed[0], &changed[1], &changed[2]),
            Err(Gfx942ShardedVecaddQualificationAdmissionErrorV1::Identity)
        );
        changed[artifact].clear();
        assert_eq!(
            validate_artifacts_v1(&changed[0], &changed[1], &changed[2]),
            Err(Gfx942ShardedVecaddQualificationAdmissionErrorV1::Identity)
        );
    }
    assert!(std::str::from_utf8(POLICY).unwrap().contains(&format!(
        "profile={GFX942_SHARDED_VECADD_QUALIFICATION_PROFILE_ID_V1}\n"
    )));
    assert_eq!(
        Gfx942ShardedVecaddQualificationArgumentsV1::SIGNATURE_V1,
        <[u8; 32]>::from(Sha256::digest(POLICY))
    );
}

#[test]
fn foreign_policy_signatures_are_denied_in_both_directions_without_consuming_gate() {
    use crate::qualification_gfx942_r57_n3_v1 as r57;
    use crate::qualification_gfx942_vecadd_v1 as original;
    with_request(2, 0, 0, |admitted, request| {
        for signature in [
            original::GFX942_VECADD_QUALIFICATION_SIGNATURE_V1,
            r57::GFX942_R57_N3_QUALIFICATION_SIGNATURE_V1,
            r57::GFX942_R57_N3_QUALIFICATION_SIGNATURE_V2,
        ] {
            assert_ne!(signature, admitted.signature());
            denied(
                admitted,
                KfdRuntimeAuthorityRequestV1 {
                    signature,
                    ..request
                },
            );
        }
        assert!(
            !original::admit_gfx942_vecadd_qualification_v1()
                .unwrap()
                .authorizes_kfd_request_v1(request)
        );
        assert!(
            !r57::admit_gfx942_r57_n3_qualification_v1()
                .unwrap()
                .authorizes_kfd_request_v1(request)
        );
        assert!(
            !r57::admit_gfx942_r57_n3_qualification_v2()
                .unwrap()
                .authorizes_kfd_request_v1(request)
        );
        #[cfg(feature = "scale-qualification")]
        {
            use crate::qualification_gfx942_vecadd_repeat_v1 as repeat;
            assert_ne!(
                repeat::GFX942_VECADD_REPEAT_QUALIFICATION_SIGNATURE_V1,
                admitted.signature()
            );
            denied(
                admitted,
                KfdRuntimeAuthorityRequestV1 {
                    signature: repeat::GFX942_VECADD_REPEAT_QUALIFICATION_SIGNATURE_V1,
                    ..request
                },
            );
            assert!(
                !repeat::admit_gfx942_vecadd_repeat_qualification_v1()
                    .unwrap()
                    .authorizes_kfd_request_v1(request)
            );
        }
        assert!(admitted.authorizes_kfd_request_v1(request));
    });
}

#[test]
fn every_artifact_kernarg_geometry_and_abi_field_is_checked_before_acceptance() {
    with_request(3, 1, 1, |admitted, request| {
        let mut object = request.module_image.to_vec();
        object[0] ^= 1;
        denied(
            admitted,
            KfdRuntimeAuthorityRequestV1 {
                module_image: &object,
                ..request
            },
        );
        denied(
            admitted,
            KfdRuntimeAuthorityRequestV1 {
                module_sha256: [0; 32],
                ..request
            },
        );
        denied(
            admitted,
            KfdRuntimeAuthorityRequestV1 {
                kernel_name: "foreign",
                ..request
            },
        );
        denied(
            admitted,
            KfdRuntimeAuthorityRequestV1 {
                signature: [0; 32],
                ..request
            },
        );
        denied(
            admitted,
            KfdRuntimeAuthorityRequestV1 {
                explicit_kernarg: &request.explicit_kernarg[..47],
                ..request
            },
        );
        for byte in 0..48 {
            let mut kernarg = request.explicit_kernarg.to_vec();
            kernarg[byte] ^= 1;
            denied(
                admitted,
                KfdRuntimeAuthorityRequestV1 {
                    explicit_kernarg: &kernarg,
                    ..request
                },
            );
            denied(
                admitted,
                KfdRuntimeAuthorityRequestV1 {
                    complete_kernarg_template: &kernarg,
                    ..request
                },
            );
        }
        for field in 0..7 {
            let mut geometry = request.geometry;
            match field {
                0..=2 => geometry.grid[field] += 1,
                3..=5 => geometry.workgroup[field - 3] += 1,
                6 => geometry.dynamic_shared_bytes = 4,
                _ => unreachable!(),
            }
            denied(
                admitted,
                KfdRuntimeAuthorityRequestV1 {
                    geometry,
                    ..request
                },
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
            admitted,
            KfdRuntimeAuthorityRequestV1 {
                semantic_launch,
                ..request
            },
        );
        denied(
            admitted,
            KfdRuntimeAuthorityRequestV1 {
                dispatch_abi: &request.dispatch_abi[..2],
                ..request
            },
        );
        for index in 0..3 {
            for field in 0..5 {
                let mut abi = request.dispatch_abi.to_vec();
                match field {
                    0 => abi[index].explicit_argument_index += 1,
                    1 => abi[index].name = "foreign",
                    2 => abi[index].kernarg_byte_offset += 4,
                    3 => abi[index].pointee_alignment = 4,
                    4 => abi[index].access = ArgumentAccess::ReadWrite,
                    _ => unreachable!(),
                }
                denied(
                    admitted,
                    KfdRuntimeAuthorityRequestV1 {
                        dispatch_abi: &abi,
                        ..request
                    },
                );
            }
        }
        assert!(admitted.authorizes_kfd_request_v1(request));
    });
}

#[test]
fn binding_and_allocation_identity_shape_alignment_and_current_contents_are_exact() {
    with_request(2, 0, 0, |admitted, request| {
        denied(
            admitted,
            KfdRuntimeAuthorityRequestV1 {
                bindings: &request.bindings[..2],
                ..request
            },
        );
        denied(
            admitted,
            KfdRuntimeAuthorityRequestV1 {
                allocations: &request.allocations[..2],
                ..request
            },
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
                    2 => bindings[index].region.allocation = 99,
                    3 => bindings[index].region.access = RuntimeAccessV1::ReadWrite,
                    4 => bindings[index].region.byte_offset = 4,
                    5 => bindings[index].region.byte_len -= 4,
                    6 => bindings[index].kernarg_byte_offset += 4,
                    _ => unreachable!(),
                }
                denied(
                    admitted,
                    KfdRuntimeAuthorityRequestV1 {
                        bindings: &bindings,
                        ..request
                    },
                );
            }
            for field in 0..9 {
                let mut allocations = request.allocations.to_vec();
                match field {
                    0 => allocations[index].allocation = 99,
                    1 => allocations[index].allocation = allocations[(index + 1) % 3].allocation,
                    2 => allocations[index].kind = RuntimeMemoryKindV1::HostVisible,
                    3 => allocations[index].alignment = 3,
                    4 => allocations[index].alignment = 2,
                    5 => allocations[index].byte_offset = 4,
                    6 => {
                        allocations[index].bytes =
                            &allocations[index].bytes[..allocations[index].bytes.len() - 4]
                    }
                    7 => allocations[index].content_sha256 = None,
                    8 => allocations[index].content_sha256 = Some([0; 32]),
                    _ => unreachable!(),
                }
                denied(
                    admitted,
                    KfdRuntimeAuthorityRequestV1 {
                        allocations: &allocations,
                        ..request
                    },
                );
            }
            for offset in [
                0,
                admitted.recipe().elements() * 4,
                admitted.recipe().padded_bytes() - 1,
            ] {
                let mut bytes = request.allocations[index].bytes.to_vec();
                bytes[offset] ^= 1;
                let mut allocations = request.allocations.to_vec();
                allocations[index].bytes = &bytes;
                denied(
                    admitted,
                    KfdRuntimeAuthorityRequestV1 {
                        allocations: &allocations,
                        ..request
                    },
                );
                allocations[index].content_sha256 = Some(Sha256::digest(&bytes).into());
                denied(
                    admitted,
                    KfdRuntimeAuthorityRequestV1 {
                        allocations: &allocations,
                        ..request
                    },
                );
            }
        }
        // Allocation metadata order is irrelevant; identities bind it to ABI order.
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

#[test]
fn another_shard_or_round_cannot_supply_current_initial_content() {
    with_request(3, 0, 0, |admitted, request| {
        for recipe in [(3, 1, 0), (3, 0, 1), (2, 0, 0), (8, 0, 0)] {
            let wrong = Gfx942ShardedVecaddQualificationRecipeV1::new(recipe.0, recipe.1, recipe.2)
                .unwrap();
            let buffers = wrong.host_buffers().unwrap();
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
                admitted,
                KfdRuntimeAuthorityRequestV1 {
                    allocations: &allocations,
                    ..request
                },
            );
        }
        assert!(admitted.authorizes_kfd_request_v1(request));
    });
}

#[test]
fn simultaneous_valid_requests_cannot_accept_more_than_once() {
    with_request(8, 7, 1, |admitted, request| {
        let accepted = std::thread::scope(|scope| {
            let handles = (0..8)
                .map(|_| scope.spawn(|| admitted.authorizes_kfd_request_v1(request)))
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .map(|handle| usize::from(handle.join().unwrap()))
                .sum::<usize>()
        });
        assert_eq!(accepted, 1);
        assert!(admitted.observation_v1().accepted_v1());
        assert_eq!(admitted.observation_v1().authorization_calls_v1(), 8);
    });
    with_request(2, 0, 0, |admitted, request| {
        admitted.state.calls.store(u64::MAX, Ordering::Release);
        assert!(!admitted.authorizes_kfd_request_v1(request));
        assert!(!admitted.observation_v1().accepted_v1());
    });
}
