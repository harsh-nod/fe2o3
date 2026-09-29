//! Output-state-independent admission for the exact scale-qualification vecadd.
//!
//! The immutable source, object and ABI admission are shared with the original
//! sentinel-only profile. This distinct policy permits arbitrary output bytes
//! from the first invocation, but requires the same two input digests. Full
//! output overwrite is trusted for the exact embedded artifact, not proved from
//! its metadata or ISA. Ordinary ownership and completion checks still apply.
//! This is neither Worker authority nor general Rust/device-language support.

use sha2::{Digest, Sha256};

use crate::qualification_gfx942_vecadd_v1::{
    AdmittedGfx942VecaddQualificationV1, GFX942_VECADD_QUALIFICATION_KERNARG_BYTES_V1,
    Gfx942VecaddQualificationAdmissionErrorV1, Gfx942VecaddQualificationArgumentV1,
    Gfx942VecaddQualificationArgumentsV1, Gfx942VecaddQualificationFixtureErrorV1,
    Gfx942VecaddQualificationHostBuffersV1, admit_gfx942_vecadd_qualification_v1,
};
use crate::{
    KfdRuntimeAuthorityRequestV1, RuntimeAllocationIdV1, RuntimeArgumentsV1, RuntimeBindingV1,
    RuntimeLaunchGeometryV1,
};

/// Separate opt-in identity; the original sentinel-only profile is unchanged.
pub const GFX942_VECADD_REPEAT_QUALIFICATION_PROFILE_ID_V1: &str =
    "fe2o3.runtime.gfx942-vecadd-repeat-qualification.v1";
/// SHA-256 of the immutable output-state-independent admission policy.
pub const GFX942_VECADD_REPEAT_QUALIFICATION_POLICY_SHA256_V1: [u8; 32] = [
    0x21, 0xcc, 0x01, 0x4d, 0x1f, 0x34, 0x70, 0x08, 0x7c, 0xb5, 0x16, 0x31, 0xe4, 0xce, 0x3c, 0xe6,
    0xa9, 0xad, 0x92, 0x29, 0x20, 0x6b, 0xe6, 0xb6, 0x87, 0x26, 0x52, 0x9d, 0x3d, 0x35, 0xd3, 0xe0,
];
/// Typed signature distinct from the original sentinel-only profile.
pub const GFX942_VECADD_REPEAT_QUALIFICATION_SIGNATURE_V1: [u8; 32] =
    GFX942_VECADD_REPEAT_QUALIFICATION_POLICY_SHA256_V1;

const POLICY_BYTES_V1: &[u8] =
    include_bytes!("../fixtures/trusted-gfx942-vecadd-v1/policy-repeat-v1.txt");

/// Returns the exact policy bytes whose digest is the typed signature.
pub const fn gfx942_vecadd_repeat_qualification_policy_v1() -> &'static [u8] {
    POLICY_BYTES_V1
}

/// Non-cloneable admission of the exact artifact and independent repeat policy.
///
/// The policy bounds the invocation shape, not the number of repeated launches.
/// Its allocation IDs and input digests are supplied by trusted backend custody;
/// the gate itself does not authenticate physical mappings or current GPU bytes.
#[derive(Debug)]
pub struct AdmittedGfx942VecaddRepeatQualificationV1 {
    fixture: AdmittedGfx942VecaddQualificationV1,
}

impl AdmittedGfx942VecaddRepeatQualificationV1 {
    pub const fn hsaco(&self) -> &'static [u8] {
        self.fixture.hsaco()
    }

    pub const fn hsaco_sha256(&self) -> [u8; 32] {
        self.fixture.hsaco_sha256()
    }

    pub const fn kernel_name(&self) -> &'static str {
        self.fixture.kernel_name()
    }

    pub const fn signature(&self) -> [u8; 32] {
        GFX942_VECADD_REPEAT_QUALIFICATION_SIGNATURE_V1
    }

    pub const fn geometry(&self) -> RuntimeLaunchGeometryV1 {
        self.fixture.geometry()
    }

    pub const fn arguments(&self) -> &'static [Gfx942VecaddQualificationArgumentV1; 3] {
        self.fixture.arguments()
    }

    pub fn explicit_kernarg(&self) -> [u8; GFX942_VECADD_QUALIFICATION_KERNARG_BYTES_V1] {
        self.fixture.explicit_kernarg()
    }

    pub fn host_buffers(
        &self,
    ) -> Result<Gfx942VecaddQualificationHostBuffersV1, Gfx942VecaddQualificationFixtureErrorV1>
    {
        self.fixture.host_buffers()
    }

    pub(crate) fn authorizes_kfd_request_v1(
        &self,
        request: KfdRuntimeAuthorityRequestV1<'_>,
    ) -> bool {
        self.fixture.authorizes_repeat_kfd_request_v1(request)
    }
}

/// Rechecks the repeat policy and the original immutable source/object/ABI.
pub fn admit_gfx942_vecadd_repeat_qualification_v1()
-> Result<AdmittedGfx942VecaddRepeatQualificationV1, Gfx942VecaddQualificationAdmissionErrorV1> {
    if <[u8; 32]>::from(Sha256::digest(POLICY_BYTES_V1))
        != GFX942_VECADD_REPEAT_QUALIFICATION_POLICY_SHA256_V1
    {
        return Err(Gfx942VecaddQualificationAdmissionErrorV1::Identity);
    }
    Ok(AdmittedGfx942VecaddRepeatQualificationV1 {
        fixture: admit_gfx942_vecadd_qualification_v1()?,
    })
}

/// Context-branded arguments carrying only the repeat profile's signature.
#[derive(Debug)]
pub struct Gfx942VecaddRepeatQualificationArgumentsV1 {
    arguments: Gfx942VecaddQualificationArgumentsV1,
}

impl Gfx942VecaddRepeatQualificationArgumentsV1 {
    pub fn new(
        left: RuntimeAllocationIdV1,
        right: RuntimeAllocationIdV1,
        output: RuntimeAllocationIdV1,
    ) -> Result<Self, Gfx942VecaddQualificationFixtureErrorV1> {
        Ok(Self {
            arguments: Gfx942VecaddQualificationArgumentsV1::new(left, right, output)?,
        })
    }

    pub const fn allocations(&self) -> [RuntimeAllocationIdV1; 3] {
        self.arguments.allocations()
    }
}

impl RuntimeArgumentsV1 for Gfx942VecaddRepeatQualificationArgumentsV1 {
    const SIGNATURE_V1: [u8; 32] = GFX942_VECADD_REPEAT_QUALIFICATION_SIGNATURE_V1;

    fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
        self.arguments.encode_explicit_kernarg_v1()
    }

    fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
        self.arguments.bindings_v1()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qualification_gfx942_vecadd_v1::*;
    use crate::{
        KfdRuntimeAuthorityAllocationV1, KfdRuntimeAuthorityGlobalBufferV1,
        KfdRuntimeSemanticLaunchV1, RuntimeAccessV1, RuntimeMemoryKindV1,
    };
    use fe2o3_hsaco::ArgumentAccess;

    fn with_request(operation: impl FnOnce(KfdRuntimeAuthorityRequestV1<'_>)) {
        let admitted = admit_gfx942_vecadd_repeat_qualification_v1().unwrap();
        let buffers = admitted.host_buffers().unwrap();
        let bindings = gfx942_vecadd_qualification_bindings_v1([10, 20, 30]).unwrap();
        let allocations = core::array::from_fn::<_, 3, _>(|index| {
            let bytes = [buffers.left(), buffers.right(), buffers.output()][index];
            KfdRuntimeAuthorityAllocationV1 {
                allocation: bindings[index].region.allocation,
                kind: RuntimeMemoryKindV1::HostVisible,
                alignment: GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1,
                byte_offset: 0,
                bytes,
                content_sha256: Some(Sha256::digest(bytes).into()),
            }
        });
        let abi = GFX942_VECADD_QUALIFICATION_ARGUMENTS_V1.map(|policy| {
            KfdRuntimeAuthorityGlobalBufferV1 {
                explicit_argument_index: policy.explicit_argument_index,
                name: policy.name,
                kernarg_byte_offset: u64::from(policy.pointer_offset),
                pointee_alignment: policy.reconciled_pointee_alignment,
                access: match policy.access {
                    RuntimeAccessV1::Read => ArgumentAccess::ReadOnly,
                    RuntimeAccessV1::Write => ArgumentAccess::WriteOnly,
                    RuntimeAccessV1::ReadWrite => unreachable!(),
                },
            }
        });
        let kernarg = admitted.explicit_kernarg();
        operation(KfdRuntimeAuthorityRequestV1 {
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
        });
    }

    #[test]
    fn policy_identity_and_typed_signature_are_distinct() {
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(POLICY_BYTES_V1)),
            GFX942_VECADD_REPEAT_QUALIFICATION_POLICY_SHA256_V1
        );
        assert_ne!(
            Gfx942VecaddRepeatQualificationArgumentsV1::SIGNATURE_V1,
            Gfx942VecaddQualificationArgumentsV1::SIGNATURE_V1
        );
        let admitted = admit_gfx942_vecadd_repeat_qualification_v1().unwrap();
        assert_eq!(admitted.signature(), Gfx942VecaddRepeatQualificationArgumentsV1::SIGNATURE_V1);
        assert_eq!(admitted.hsaco(), gfx942_vecadd_qualification_hsaco_v1());
        assert!(std::str::from_utf8(POLICY_BYTES_V1).unwrap().contains(
            &format!("profile={GFX942_VECADD_REPEAT_QUALIFICATION_PROFILE_ID_V1}\n")
        ));
    }

    #[test]
    fn signatures_are_isolated_in_both_directions() {
        let repeat = admit_gfx942_vecadd_repeat_qualification_v1().unwrap();
        let original = admit_gfx942_vecadd_qualification_v1().unwrap();
        with_request(|request| {
            assert!(repeat.authorizes_kfd_request_v1(request));
            assert!(!original.authorizes_kfd_request_v1(request));
            let old = KfdRuntimeAuthorityRequestV1 {
                signature: GFX942_VECADD_QUALIFICATION_SIGNATURE_V1,
                ..request
            };
            assert!(original.authorizes_kfd_request_v1(old));
            assert!(!repeat.authorizes_kfd_request_v1(old));
        });
    }

    #[test]
    fn arbitrary_output_is_admitted_from_the_first_invocation_but_not_by_v1() {
        let repeat = admit_gfx942_vecadd_repeat_qualification_v1().unwrap();
        let original = admit_gfx942_vecadd_qualification_v1().unwrap();
        with_request(|request| {
            let mut output = vec![0xa5; GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1];
            for digest in [None, Some([0; 32]), Some(Sha256::digest(&output).into())] {
                let mut allocations = request.allocations.to_vec();
                allocations[2].bytes = &output;
                allocations[2].content_sha256 = digest;
                let changed = KfdRuntimeAuthorityRequestV1 { allocations: &allocations, ..request };
                assert!(repeat.authorizes_kfd_request_v1(changed));
                assert!(!original.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                    signature: GFX942_VECADD_QUALIFICATION_SIGNATURE_V1, ..changed
                }));
            }
            output.copy_from_slice(repeat.host_buffers().unwrap().expected_output());
            let mut allocations = request.allocations.to_vec();
            allocations[2].bytes = &output;
            allocations[2].content_sha256 = None;
            assert!(repeat.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                allocations: &allocations, ..request
            }));
        });
    }

    #[test]
    fn repeat_requires_both_exact_input_digests() {
        let admitted = admit_gfx942_vecadd_repeat_qualification_v1().unwrap();
        with_request(|request| {
            for index in 0..2 {
                for digest in [None, Some([0; 32]), request.allocations[1 - index].content_sha256] {
                    let mut allocations = request.allocations.to_vec();
                    allocations[index].content_sha256 = digest;
                    assert!(!admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                        allocations: &allocations, ..request
                    }), "input {index} digest");
                }
            }
        });
    }

    #[test]
    fn repeat_rejects_artifact_kernarg_geometry_and_semantic_drift() {
        let admitted = admit_gfx942_vecadd_repeat_qualification_v1().unwrap();
        with_request(|request| {
            macro_rules! rejects {
                ($field:ident, $value:expr) => {
                    assert!(!admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                        $field: $value, ..request
                    }), stringify!($field));
                };
            }
            let mut object = request.module_image.to_vec();
            object[1024] ^= 1;
            rejects!(module_image, &object);
            rejects!(module_sha256, [0; 32]);
            rejects!(kernel_name, "other");
            rejects!(signature, [0; 32]);
            for index in 0..GFX942_VECADD_QUALIFICATION_KERNARG_BYTES_V1 {
                let mut kernarg = request.explicit_kernarg.to_vec();
                kernarg[index] ^= 1;
                rejects!(explicit_kernarg, &kernarg);
                rejects!(complete_kernarg_template, &kernarg);
            }
            rejects!(explicit_kernarg, &request.explicit_kernarg[..47]);
            rejects!(complete_kernarg_template, &request.complete_kernarg_template[..47]);
            for axis in 0..3 {
                let mut geometry = request.geometry;
                geometry.grid[axis] += 1;
                rejects!(geometry, geometry);
                geometry = request.geometry;
                geometry.workgroup[axis] += 1;
                rejects!(geometry, geometry);
            }
            let mut geometry = request.geometry;
            geometry.dynamic_shared_bytes = 4;
            rejects!(geometry, geometry);
            rejects!(semantic_launch, KfdRuntimeSemanticLaunchV1::Atomic(crate::RuntimeAtomicLaunchContractV1 {
                operation: crate::RuntimeAtomicOperationV1::Add,
                scope: crate::RuntimeMemoryScopeV1::Workgroup,
                order: crate::RuntimeMemoryOrderV1::Relaxed,
                failure_order: None,
                weak: false,
                geometry: request.geometry,
            }));
            rejects!(semantic_launch, KfdRuntimeSemanticLaunchV1::Collective(crate::RuntimeCollectiveLaunchContractV1 {
                operation: crate::RuntimeCollectiveOperationV1::ReduceSum,
                scope: crate::RuntimeMemoryScopeV1::Workgroup,
                order: crate::RuntimeMemoryOrderV1::Relaxed,
                participants: 256,
                geometry: request.geometry,
            }));
        });
    }

    #[test]
    fn repeat_rejects_every_abi_binding_and_allocation_shape_dimension() {
        let admitted = admit_gfx942_vecadd_repeat_qualification_v1().unwrap();
        with_request(|request| {
            for index in 0..3 {
                macro_rules! binding_rejects {
                    ($($field:ident).+, $value:expr) => {{
                        let mut bindings = request.bindings.to_vec();
                        bindings[index].$($field).+ = $value;
                        assert!(!admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                            bindings: &bindings, ..request
                        }), "binding {} {}", index, stringify!($($field).+));
                    }};
                }
                binding_rejects!(kernarg_byte_offset, 99);
                binding_rejects!(region.access, RuntimeAccessV1::ReadWrite);
                binding_rejects!(region.access, if index == 2 { RuntimeAccessV1::Read } else { RuntimeAccessV1::Write });
                binding_rejects!(region.byte_offset, 4);
                binding_rejects!(region.byte_len, 4);
                binding_rejects!(region.allocation, 999);
                binding_rejects!(region.allocation, request.bindings[(index + 1) % 3].region.allocation);
                macro_rules! abi_rejects {
                    ($field:ident, $value:expr) => {{
                        let mut abi = request.dispatch_abi.to_vec();
                        abi[index].$field = $value;
                        assert!(!admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                            dispatch_abi: &abi, ..request
                        }), "ABI {} {}", index, stringify!($field));
                    }};
                }
                abi_rejects!(explicit_argument_index, 99);
                abi_rejects!(name, "other");
                abi_rejects!(kernarg_byte_offset, 99);
                abi_rejects!(pointee_alignment, 8);
                abi_rejects!(access, ArgumentAccess::ReadWrite);
                macro_rules! allocation_rejects {
                    ($field:ident, $value:expr) => {{
                        let mut allocations = request.allocations.to_vec();
                        allocations[index].$field = $value;
                        assert!(!admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                            allocations: &allocations, ..request
                        }), "allocation {} {}", index, stringify!($field));
                    }};
                }
                allocation_rejects!(allocation, 999);
                allocation_rejects!(allocation, request.allocations[(index + 1) % 3].allocation);
                allocation_rejects!(kind, RuntimeMemoryKindV1::DeviceLocal);
                allocation_rejects!(alignment, 0);
                allocation_rejects!(alignment, 2);
                allocation_rejects!(alignment, 6);
                allocation_rejects!(byte_offset, 4);
                allocation_rejects!(bytes, &request.allocations[index].bytes[..4]);
            }
            for count in [0, 1, 2, 4] {
                let mut bindings = request.bindings.to_vec();
                bindings.resize(count, bindings[0]);
                assert!(!admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                    bindings: &bindings, ..request
                }));
                let mut abi = request.dispatch_abi.to_vec();
                abi.resize(count, abi[0]);
                assert!(!admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                    dispatch_abi: &abi, ..request
                }));
                let mut allocations = request.allocations.to_vec();
                allocations.resize(count, allocations[0]);
                assert!(!admitted.authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                    allocations: &allocations, ..request
                }));
            }
        });
    }
}
