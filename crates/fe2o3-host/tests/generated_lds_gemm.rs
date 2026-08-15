// This path import is temporary until the primary integrator wires the module
// through fe2o3-host's generated SPI. It compiles the production source and
// exercises its pure validators without creating raw production capabilities.
pub use fe2o3_host::ObservedContext;

#[allow(dead_code)]
#[path = "../src/generated_lds_gemm.rs"]
mod generated_lds_gemm;

use fe2o3_core::{DeviceBufferView, DeviceBufferViewMut};
use fe2o3_hsaco_finalize::{
    ExactLdsGemmBufferRoleV1, InspectedExactLdsGemmCompilerImportIdentityV1,
    InspectedExactLdsGemmCompilerImportV1,
};
use generated_lds_gemm::test_support::{
    TestContractMutationV1, TestRegionFactsV1, explicit_kernarg_layout_v1, prepare_regions_v1,
    validate_contract_mutation_v1, validate_observed_target_v1,
};
use generated_lds_gemm::{
    GeneratedLdsGemmSlice1HostAdapterErrorV1, GeneratedLdsGemmSlice1HostAdapterV1,
};

fn u16_region(allocation_address: usize, region_address: usize) -> TestRegionFactsV1 {
    TestRegionFactsV1 {
        allocation_address,
        allocation_elements: 256,
        region_address,
        region_elements: 256,
        region_byte_start: region_address - allocation_address,
        region_byte_end: region_address - allocation_address + 512,
        element_bytes: 2,
        element_alignment: 2,
    }
}

fn f32_region(allocation_address: usize, region_address: usize) -> TestRegionFactsV1 {
    TestRegionFactsV1 {
        allocation_address,
        allocation_elements: 256,
        region_address,
        region_elements: 256,
        region_byte_start: region_address - allocation_address,
        region_byte_end: region_address - allocation_address + 1_024,
        element_bytes: 4,
        element_alignment: 4,
    }
}

fn canonical_regions() -> (TestRegionFactsV1, TestRegionFactsV1, TestRegionFactsV1) {
    (
        u16_region(0x1_0000, 0x1_0000),
        u16_region(0x2_0000, 0x2_0000),
        f32_region(0x3_0000, 0x3_0000),
    )
}

fn decode_word(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

#[test]
fn canonical_regions_produce_exact_pointer_length_abi() {
    let (a, b, c) = canonical_regions();
    let bytes = prepare_regions_v1(a, b, c).unwrap();

    assert_eq!(bytes.len(), 48);
    assert_eq!(explicit_kernarg_layout_v1(), (48, 8));
    assert_eq!(decode_word(&bytes, 0), 0x1_0000);
    assert_eq!(decode_word(&bytes, 8), 256);
    assert_eq!(decode_word(&bytes, 16), 0x2_0000);
    assert_eq!(decode_word(&bytes, 24), 256);
    assert_eq!(decode_word(&bytes, 32), 0x3_0000);
    assert_eq!(decode_word(&bytes, 40), 256);
}

#[test]
fn every_contract_or_profile_mutation_fails_closed() {
    assert_eq!(
        validate_contract_mutation_v1(TestContractMutationV1::None),
        Ok(())
    );
    for mutation in [
        TestContractMutationV1::Profile,
        TestContractMutationV1::Target,
        TestContractMutationV1::CodeObjectVersion,
        TestContractMutationV1::Grid,
        TestContractMutationV1::Workgroup,
        TestContractMutationV1::Wavefront,
        TestContractMutationV1::ExplicitKernarg,
        TestContractMutationV1::CompleteKernarg,
        TestContractMutationV1::KernargAlignment,
        TestContractMutationV1::StaticLds,
        TestContractMutationV1::LdsAllocations,
        TestContractMutationV1::LdsBytesPerAllocation,
        TestContractMutationV1::LdsAlignment,
        TestContractMutationV1::BufferRole,
        TestContractMutationV1::BufferElement,
        TestContractMutationV1::BufferLength,
        TestContractMutationV1::BufferBytes,
        TestContractMutationV1::BufferLengthIdentity,
        TestContractMutationV1::BufferOwnership,
        TestContractMutationV1::BufferAccess,
        TestContractMutationV1::BufferAlias,
    ] {
        assert!(
            validate_contract_mutation_v1(mutation).is_err(),
            "accepted {mutation:?}"
        );
    }
}

#[test]
fn public_prepare_copies_identities_and_releases_the_import_borrow() {
    fn consume_compiler_import(
        compiler_import: InspectedExactLdsGemmCompilerImportV1,
    ) -> InspectedExactLdsGemmCompilerImportIdentityV1 {
        compiler_import.identity()
    }

    fn prepare_then_consume_import<'a, 'b, 'c>(
        observed: &ObservedContext,
        compiler_import: InspectedExactLdsGemmCompilerImportV1,
        a: DeviceBufferView<'a, u16>,
        b: DeviceBufferView<'b, u16>,
        c: DeviceBufferViewMut<'c, f32>,
    ) -> Result<
        (
            GeneratedLdsGemmSlice1HostAdapterV1<'a, 'b, 'c>,
            InspectedExactLdsGemmCompilerImportIdentityV1,
        ),
        GeneratedLdsGemmSlice1HostAdapterErrorV1,
    > {
        let adapter =
            GeneratedLdsGemmSlice1HostAdapterV1::prepare(observed, &compiler_import, a, b, c)?;
        let consumed_identity = consume_compiler_import(compiler_import);
        assert_eq!(adapter.compiler_import_identity(), consumed_identity);
        Ok((adapter, consumed_identity))
    }

    fn assert_copied_identity_binding(
        adapter: &GeneratedLdsGemmSlice1HostAdapterV1<'_, '_, '_>,
        compiler_import: &InspectedExactLdsGemmCompilerImportV1,
    ) {
        let contract = compiler_import.contract();
        assert_eq!(
            adapter.compiler_import_identity(),
            compiler_import.identity()
        );
        assert_eq!(
            adapter.compiler_import_identity_v1(),
            compiler_import.identity()
        );
        assert_eq!(adapter.profile_identity(), contract.identity());
        assert_eq!(adapter.contract_v1(), contract);

        let descriptor_source_identity = compiler_import.descriptor_source().identity();
        let copied_descriptor_source_identity = adapter.descriptor_source_identity_v1();
        assert_eq!(
            copied_descriptor_source_identity.sha256_v1(),
            descriptor_source_identity.sha256()
        );
        assert_eq!(
            copied_descriptor_source_identity.byte_len_v1(),
            descriptor_source_identity.byte_len()
        );
        assert_eq!(
            adapter.length_identities_v1(),
            contract.buffers().map(|buffer| buffer.length_identity())
        );
    }

    let _ = prepare_then_consume_import;
    let _ = assert_copied_identity_binding;

    #[derive(Debug)]
    struct NonCloneIdentitySource(u64);
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct PreparedIdentity(u64);

    fn prepare_identity(source: &NonCloneIdentitySource) -> PreparedIdentity {
        PreparedIdentity(source.0)
    }
    fn consume_identity_source(source: NonCloneIdentitySource) -> u64 {
        source.0
    }

    let source = NonCloneIdentitySource(0x9799_0100);
    let prepared = prepare_identity(&source);
    let consumed_identity = consume_identity_source(source);
    assert_eq!(prepared, PreparedIdentity(consumed_identity));
    assert_eq!(prepared.0, 0x9799_0100);
}

#[test]
fn observed_target_is_exact_and_xnack_sensitive() {
    assert_eq!(validate_observed_target_v1("gfx942:xnack-"), Ok(()));
    for target in [
        "gfx942",
        "gfx942:xnack+",
        "gfx942:sramecc+:xnack-",
        "gfx950:xnack-",
    ] {
        assert_eq!(
            validate_observed_target_v1(target),
            Err(GeneratedLdsGemmSlice1HostAdapterErrorV1::ObservedTargetMismatch),
            "accepted {target}"
        );
    }
}

#[test]
fn exact_lengths_are_a_closed_property() {
    let (a, b, c) = canonical_regions();
    for role in 0..3 {
        for length in 0..=512 {
            let mut regions = [a, b, c];
            regions[role].region_elements = length;
            let result = prepare_regions_v1(regions[0], regions[1], regions[2]);
            assert_eq!(result.is_ok(), length == 256, "role={role} length={length}");
        }
    }
}

#[test]
fn a_and_b_may_alias_but_c_must_be_disjoint() {
    let (a, _, c) = canonical_regions();
    assert!(prepare_regions_v1(a, a, c).is_ok());

    let mut c_over_a = c;
    c_over_a.allocation_address = a.allocation_address;
    c_over_a.region_address = a.region_address;
    assert_eq!(
        prepare_regions_v1(a, a, c_over_a),
        Err(GeneratedLdsGemmSlice1HostAdapterErrorV1::OutputOverlap {
            input: ExactLdsGemmBufferRoleV1::A,
        })
    );

    let (_, b, _) = canonical_regions();
    let c_after_b = f32_region(b.region_address + 512, b.region_address + 512);
    assert!(prepare_regions_v1(a, b, c_after_b).is_ok());
}

#[test]
fn c_overlap_is_rejected_at_every_input_byte_boundary() {
    let (a, b, c) = canonical_regions();
    for input in [a, b] {
        for delta in 0..512 {
            let address = input.region_address + delta;
            let overlapping_c = TestRegionFactsV1 {
                allocation_address: address,
                allocation_elements: 256,
                region_address: address,
                region_elements: 256,
                region_byte_start: 0,
                region_byte_end: 1_024,
                element_bytes: 4,
                element_alignment: 4,
            };
            let result = prepare_regions_v1(a, b, overlapping_c);
            if address.is_multiple_of(4) {
                assert!(
                    matches!(
                        result,
                        Err(GeneratedLdsGemmSlice1HostAdapterErrorV1::OutputOverlap { .. })
                    ),
                    "accepted overlap at {address:#x}"
                );
            } else {
                assert!(
                    matches!(
                        result,
                        Err(GeneratedLdsGemmSlice1HostAdapterErrorV1::Alignment {
                            role: ExactLdsGemmBufferRoleV1::C,
                            ..
                        })
                    ),
                    "misclassified unaligned overlap at {address:#x}"
                );
            }
        }
    }
    assert!(prepare_regions_v1(a, b, c).is_ok());
}

#[test]
fn alignment_is_checked_for_each_typed_role() {
    let (a, b, c) = canonical_regions();
    for (role, required) in [(0, 2), (1, 2), (2, 4)] {
        for offset in 1..required {
            let mut regions = [a, b, c];
            regions[role].allocation_address += offset;
            regions[role].region_address += offset;
            assert!(matches!(
                prepare_regions_v1(regions[0], regions[1], regions[2]),
                Err(GeneratedLdsGemmSlice1HostAdapterErrorV1::Alignment { .. })
            ));
        }
    }
}

#[test]
fn checked_arithmetic_and_range_substitution_fail_closed() {
    let (a, b, c) = canonical_regions();

    let mut allocation_size_overflow = a;
    allocation_size_overflow.allocation_elements = usize::MAX;
    assert!(matches!(
        prepare_regions_v1(allocation_size_overflow, b, c),
        Err(
            GeneratedLdsGemmSlice1HostAdapterErrorV1::ByteLengthOverflow {
                role: ExactLdsGemmBufferRoleV1::A
            }
        )
    ));

    let mut allocation_address_overflow = a;
    allocation_address_overflow.allocation_address = usize::MAX - 1;
    allocation_address_overflow.region_address = usize::MAX - 1;
    assert!(matches!(
        prepare_regions_v1(allocation_address_overflow, b, c),
        Err(
            GeneratedLdsGemmSlice1HostAdapterErrorV1::AllocationAddressOverflow {
                role: ExactLdsGemmBufferRoleV1::A
            }
        )
    ));

    let mut region_address_overflow = a;
    region_address_overflow.allocation_address = 0x4_0000;
    region_address_overflow.allocation_elements = 512;
    region_address_overflow.region_address = usize::MAX - 255;
    region_address_overflow.region_byte_start = 768;
    region_address_overflow.region_byte_end = 1_280;
    assert!(matches!(
        prepare_regions_v1(region_address_overflow, b, c),
        Err(
            GeneratedLdsGemmSlice1HostAdapterErrorV1::RegionAddressOverflow {
                role: ExactLdsGemmBufferRoleV1::A
            }
        )
    ));

    assert!(matches!(
        prepare_regions_v1(
            TestRegionFactsV1 {
                allocation_address: 0,
                region_address: 0x1_0000,
                ..a
            },
            b,
            c
        ),
        Err(GeneratedLdsGemmSlice1HostAdapterErrorV1::NullAddress {
            role: ExactLdsGemmBufferRoleV1::A
        })
    ));

    for mutation in [
        TestRegionFactsV1 {
            region_byte_end: 511,
            ..a
        },
        TestRegionFactsV1 {
            region_byte_start: 1,
            ..a
        },
        TestRegionFactsV1 {
            region_address: a.region_address + 2,
            ..a
        },
        TestRegionFactsV1 {
            allocation_elements: 255,
            ..a
        },
    ] {
        assert!(matches!(
            prepare_regions_v1(mutation, b, c),
            Err(
                GeneratedLdsGemmSlice1HostAdapterErrorV1::InvalidRegionRange {
                    role: ExactLdsGemmBufferRoleV1::A
                }
            )
        ));
    }
}

#[test]
fn null_regions_and_element_layout_substitution_are_rejected() {
    let (a, b, c) = canonical_regions();
    assert!(matches!(
        prepare_regions_v1(
            TestRegionFactsV1 {
                allocation_address: 0,
                region_address: 0,
                ..a
            },
            b,
            c
        ),
        Err(GeneratedLdsGemmSlice1HostAdapterErrorV1::NullAddress {
            role: ExactLdsGemmBufferRoleV1::A
        })
    ));

    for mutation in [
        TestRegionFactsV1 {
            element_bytes: 4,
            ..a
        },
        TestRegionFactsV1 {
            element_alignment: 4,
            ..a
        },
    ] {
        assert!(matches!(
            prepare_regions_v1(mutation, b, c),
            Err(GeneratedLdsGemmSlice1HostAdapterErrorV1::ElementLayout {
                role: ExactLdsGemmBufferRoleV1::A
            })
        ));
    }
}

// Compile-fail follow-up for the primary integrator, which owns lib.rs and the
// trybuild fixture tree outside this agent's exact write scope:
// - adapter cannot Clone or Copy;
// - A/B/C buffers cannot be dropped while the adapter is live;
// - C cannot be mutably or immutably reborrowed while the adapter is live;
// - private fields, explicit_kernarg_bytes_v1, and contract_v1 are inaccessible
//   downstream;
// - no launch/load/into_inner/as_raw/raw-pointer method exists;
// Compile-pass follow-up: the compiler import can move into #97 while the
// three-lifetime adapter remains live (covered above without a GPU fixture).
// Runtime follow-up: issue #100 must compare the copied import, profile,
// contract, descriptor, and length identities with #97's finalized receipt
// before it creates protected launch authority.
