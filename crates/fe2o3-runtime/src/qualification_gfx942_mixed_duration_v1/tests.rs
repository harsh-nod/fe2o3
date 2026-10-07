use super::*;
use crate::{KfdRuntimeAuthorityAllocationV1, KfdRuntimeAuthorityGlobalBufferV1};

const VARIANTS: [Gfx942MixedDurationQualificationVariantV1; 2] = [
    Gfx942MixedDurationQualificationVariantV1::Short,
    Gfx942MixedDurationQualificationVariantV1::Long,
];

struct Invocation {
    initial: [u8; 384],
    kernarg: [u8; 16],
    bindings: [BackendBindingV1; 1],
    abi: [KfdRuntimeAuthorityGlobalBufferV1<'static>; 1],
}

impl Invocation {
    fn new() -> Self {
        Self {
            initial: gfx942_mixed_duration_qualification_initial_v1(),
            kernarg: gfx942_mixed_duration_qualification_explicit_kernarg_v1(),
            bindings: gfx942_mixed_duration_qualification_bindings_v1(10),
            abi: [KfdRuntimeAuthorityGlobalBufferV1 {
                explicit_argument_index: 0,
                name: "data",
                kernarg_byte_offset: 0,
                pointee_alignment: 1,
                access: ArgumentAccess::ReadWrite,
            }],
        }
    }
    fn allocation(&self) -> KfdRuntimeAuthorityAllocationV1<'_> {
        KfdRuntimeAuthorityAllocationV1 {
            allocation: 10,
            kind: RuntimeMemoryKindV1::HostVisible,
            alignment: 4,
            byte_offset: 0,
            bytes: &self.initial,
            content_sha256: Some(Sha256::digest(self.initial).into()),
        }
    }
    fn request<'a>(
        &'a self,
        variant: Gfx942MixedDurationQualificationVariantV1,
        allocations: &'a [KfdRuntimeAuthorityAllocationV1<'a>],
    ) -> KfdRuntimeAuthorityRequestV1<'a> {
        KfdRuntimeAuthorityRequestV1 {
            module_image: variant.hsaco(),
            module_sha256: variant.hsaco_sha256(),
            kernel_name: variant.kernel_name(),
            signature: GFX942_MIXED_DURATION_QUALIFICATION_SIGNATURE_V1,
            explicit_kernarg: &self.kernarg,
            complete_kernarg_template: &self.kernarg,
            bindings: &self.bindings,
            dispatch_abi: &self.abi,
            allocations,
            geometry: GFX942_MIXED_DURATION_QUALIFICATION_GEOMETRY_V1,
            semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
        }
    }
}

#[test]
fn exact_pair_admits_both_objects_and_no_cross_profile_tuple() {
    assert!(std::mem::size_of::<AdmittedGfx942MixedDurationQualificationV1>() <= 64);
    let admitted = admit_gfx942_mixed_duration_qualification_v1().unwrap();
    let invocation = Invocation::new();
    let allocations = [invocation.allocation()];
    for variant in VARIANTS {
        let request = invocation.request(variant, &allocations);
        assert!(admitted.authorizes_kfd_request_v1(request));
        let other = if variant == VARIANTS[0] {
            VARIANTS[1]
        } else {
            VARIANTS[0]
        };
        for changed in [
            KfdRuntimeAuthorityRequestV1 {
                kernel_name: other.kernel_name(),
                ..request
            },
            KfdRuntimeAuthorityRequestV1 {
                module_image: other.hsaco(),
                ..request
            },
            KfdRuntimeAuthorityRequestV1 {
                module_sha256: other.hsaco_sha256(),
                ..request
            },
            KfdRuntimeAuthorityRequestV1 {
                signature: [0; 32],
                ..request
            },
            KfdRuntimeAuthorityRequestV1 {
                kernel_name: "unknown",
                ..request
            },
        ] {
            assert!(!admitted.authorizes_kfd_request_v1(changed));
        }
        let mut changed = variant.hsaco().to_vec();
        changed[1024] ^= 1;
        assert!(
            !admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                module_image: &changed,
                ..request
            })
        );
    }
}

#[test]
fn exact_gate_rejects_geometry_kernarg_and_semantic_changes() {
    let admitted = admit_gfx942_mixed_duration_qualification_v1().unwrap();
    let invocation = Invocation::new();
    let allocations = [invocation.allocation()];
    for variant in VARIANTS {
        let request = invocation.request(variant, &allocations);
        for dimension in 0..3 {
            let mut changed = request;
            changed.geometry.grid[dimension] += 1;
            assert!(!admitted.authorizes_kfd_request_v1(changed));
            let mut changed = request;
            changed.geometry.workgroup[dimension] += 1;
            assert!(!admitted.authorizes_kfd_request_v1(changed));
        }
        let mut changed = request;
        changed.geometry.dynamic_shared_bytes = 4;
        assert!(!admitted.authorizes_kfd_request_v1(changed));
        for index in 0..16 {
            let mut bytes = invocation.kernarg;
            bytes[index] ^= 1;
            assert!(
                !admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                    explicit_kernarg: &bytes,
                    ..request
                })
            );
            assert!(
                !admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                    complete_kernarg_template: &bytes,
                    ..request
                })
            );
        }
        for bytes in [&invocation.kernarg[..15], &[0u8; 17][..]] {
            assert!(
                !admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                    explicit_kernarg: bytes,
                    ..request
                })
            );
            assert!(
                !admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                    complete_kernarg_template: bytes,
                    ..request
                })
            );
        }
        let semantic = KfdRuntimeSemanticLaunchV1::Atomic(crate::RuntimeAtomicLaunchContractV1 {
            operation: crate::RuntimeAtomicOperationV1::Add,
            scope: crate::RuntimeMemoryScopeV1::Workgroup,
            order: crate::RuntimeMemoryOrderV1::Relaxed,
            failure_order: None,
            weak: false,
            geometry: request.geometry,
        });
        assert!(
            !admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                semantic_launch: semantic,
                ..request
            })
        );
        let semantic =
            KfdRuntimeSemanticLaunchV1::Collective(crate::RuntimeCollectiveLaunchContractV1 {
                operation: crate::RuntimeCollectiveOperationV1::Barrier,
                scope: crate::RuntimeMemoryScopeV1::Workgroup,
                order: crate::RuntimeMemoryOrderV1::Relaxed,
                participants: 64,
                geometry: request.geometry,
            });
        assert!(
            !admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                semantic_launch: semantic,
                ..request
            })
        );
    }
}

#[test]
fn exact_gate_rejects_binding_and_abi_changes() {
    for variant in VARIANTS {
        check_binding_and_abi_changes(variant);
    }
}

fn check_binding_and_abi_changes(variant: Gfx942MixedDurationQualificationVariantV1) {
    let admitted = admit_gfx942_mixed_duration_qualification_v1().unwrap();
    let invocation = Invocation::new();
    let allocations = [invocation.allocation()];
    let request = invocation.request(variant, &allocations);
    let binding = invocation.bindings[0];
    for changed in [
        BackendBindingV1 {
            kernarg_byte_offset: 8,
            ..binding
        },
        BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                access: RuntimeAccessV1::Read,
                ..binding.region
            },
            ..binding
        },
        BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                byte_offset: 4,
                ..binding.region
            },
            ..binding
        },
        BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                byte_len: 380,
                ..binding.region
            },
            ..binding
        },
        BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: 11,
                ..binding.region
            },
            ..binding
        },
    ] {
        assert!(
            !admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                bindings: &[changed],
                ..request
            })
        );
    }
    for bindings in [&[][..], &[binding, binding][..]] {
        assert!(
            !admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                bindings,
                ..request
            })
        );
    }
    let abi = invocation.abi[0];
    for changed in [
        KfdRuntimeAuthorityGlobalBufferV1 {
            explicit_argument_index: 1,
            ..abi
        },
        KfdRuntimeAuthorityGlobalBufferV1 {
            name: "other",
            ..abi
        },
        KfdRuntimeAuthorityGlobalBufferV1 {
            kernarg_byte_offset: 8,
            ..abi
        },
        KfdRuntimeAuthorityGlobalBufferV1 {
            pointee_alignment: 4,
            ..abi
        },
        KfdRuntimeAuthorityGlobalBufferV1 {
            access: ArgumentAccess::ReadOnly,
            ..abi
        },
    ] {
        assert!(
            !admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                dispatch_abi: &[changed],
                ..request
            })
        );
    }
    for dispatch_abi in [&[][..], &[abi, abi][..]] {
        assert!(
            !admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                dispatch_abi,
                ..request
            })
        );
    }
}

#[test]
fn exact_gate_rejects_allocation_and_stale_content_changes() {
    for variant in VARIANTS {
        check_allocation_and_stale_content_changes(variant);
    }
}

fn check_allocation_and_stale_content_changes(variant: Gfx942MixedDurationQualificationVariantV1) {
    let admitted = admit_gfx942_mixed_duration_qualification_v1().unwrap();
    let invocation = Invocation::new();
    let allocations = [invocation.allocation()];
    let request = invocation.request(variant, &allocations);
    let allocation = allocations[0];
    for alignment in [4, 8, 64, 4096] {
        assert!(
            admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                allocations: &[KfdRuntimeAuthorityAllocationV1 {
                    alignment,
                    ..allocation
                }],
                ..request
            })
        );
    }
    for changed in [
        KfdRuntimeAuthorityAllocationV1 {
            allocation: 11,
            ..allocation
        },
        KfdRuntimeAuthorityAllocationV1 {
            kind: RuntimeMemoryKindV1::DeviceLocal,
            ..allocation
        },
        KfdRuntimeAuthorityAllocationV1 {
            alignment: 2,
            ..allocation
        },
        KfdRuntimeAuthorityAllocationV1 {
            alignment: 6,
            ..allocation
        },
        KfdRuntimeAuthorityAllocationV1 {
            byte_offset: 4,
            ..allocation
        },
        KfdRuntimeAuthorityAllocationV1 {
            bytes: &invocation.initial[..380],
            ..allocation
        },
        KfdRuntimeAuthorityAllocationV1 {
            content_sha256: None,
            ..allocation
        },
        KfdRuntimeAuthorityAllocationV1 {
            content_sha256: Some([0; 32]),
            ..allocation
        },
    ] {
        assert!(
            !admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                allocations: &[changed],
                ..request
            })
        );
    }
    for allocations in [&[][..], &[allocation, allocation][..]] {
        assert!(
            !admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                allocations,
                ..request
            })
        );
    }
    for index in 0..384 {
        let mut bytes = invocation.initial;
        bytes[index] ^= 1;
        // A stale retained digest must not hide changed bytes, including guards.
        let changed = KfdRuntimeAuthorityAllocationV1 {
            bytes: &bytes,
            ..allocation
        };
        assert!(
            !admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                allocations: &[changed],
                ..request
            })
        );
    }
}

#[test]
fn logarithmic_oracle_matches_direct_short_recurrences() {
    for steps in [0, 1, 2, 31, 32, 255, 256, 257, 1025] {
        let (scale, shift) = affine_power(steps);
        for initial in [0, 1, u32::MAX, 0xa5a5_5a5a, 0x0123_4567] {
            let mut direct = initial;
            for _ in 0..steps {
                direct = direct.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            }
            assert_eq!(initial.wrapping_mul(scale).wrapping_add(shift), direct);
        }
    }
}

#[test]
fn output_validation_checks_every_byte_and_both_guards() {
    let initial = gfx942_mixed_duration_qualification_initial_v1();
    assert_ne!(VARIANTS[0].expected_output(), VARIANTS[1].expected_output());
    for variant in VARIANTS {
        let expected = variant.expected_output();
        assert!(variant.validate_output(&expected));
        assert!(!variant.validate_output(&initial));
        let other = if variant == VARIANTS[0] {
            VARIANTS[1]
        } else {
            VARIANTS[0]
        };
        assert!(!variant.validate_output(&other.expected_output()));
        assert_eq!(&expected[..64], &initial[..64]);
        assert_eq!(&expected[320..], &initial[320..]);
        for index in 0..384 {
            let mut changed = expected;
            changed[index] ^= 1;
            assert!(!variant.validate_output(&changed), "byte {index}");
        }
        assert!(!variant.validate_output(&expected[..383]));
        let mut extended = expected.to_vec();
        extended.push(0);
        assert!(!variant.validate_output(&extended));
    }
}

#[test]
fn complete_images_match_independent_modular_geometric_sum_oracle() {
    assert_eq!(
        &gfx942_mixed_duration_qualification_initial_v1(),
        include_bytes!("../../fixtures/trusted-gfx942-mixed-duration-v1/initial.bin")
    );
    assert_eq!(
        &VARIANTS[0].expected_output(),
        include_bytes!("../../fixtures/trusted-gfx942-mixed-duration-v1/short.expected.bin")
    );
    assert_eq!(
        &VARIANTS[1].expected_output(),
        include_bytes!("../../fixtures/trusted-gfx942-mixed-duration-v1/long.expected.bin")
    );
}
