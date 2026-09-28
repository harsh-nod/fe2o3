//! Actual packing tail, inert component inputs only: no Prefix or compiler custody.
#![cfg(test)]
use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerFfiEnvelopeV1, CompilerModuleKindV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1, DeviceTargetV1,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_rustc_invocation::{CompileEnvironmentV2, RustcInvocationDescriptorV2, RustcUnitV2};
use std::ffi::OsString;

const LIMIT: usize = MAX_NATIVE_CONDITIONAL_STORAGE_V1;
const WORK: usize = 128 * 1024 * 1024;
const INHERITED: usize = 73;
const CATALOG: &[u8] = b"inert catalog leaf";
const DESCRIPTOR: &[u8] = b"inert V5 descriptor leaf, not admitted";
const SOURCE: &[u8] = b"inert V2 source packet leaf";
const INVENTORY: &[u8] = b"inert rustc inventory preimage";
const PREFLIGHT: &[u8] = b"different inert rustc preflight preimage";

struct Fixture {
    carrier: Vec<u8>,
    policy_roster: Vec<u8>,
    history: Vec<u8>,
    invocation: Vec<u8>,
    layout: Box<[u8]>,
    lowering: InertNativeLoweringAssociationV1,
    commitment: Commitment,
    module: Module,
}
impl Fixture {
    fn new(profile: Profile, history_len: usize) -> Self {
        // Same no-device-FFI fixture construction as the existing V4 pack tests.
        let environment = CompileEnvironmentV2::from_child_environment(
            [
                ("CARGO_CFG_TARGET_ARCH", "amdgcn"),
                ("FE2O3_HSACO_DIR", "/workspace/output"),
                ("FE2O3_TARGET", profile.device_target()),
                ("FE2O3_VERIFY_KERNEL_IR", "1"),
            ]
            .map(|(k, v)| (OsString::from(k), OsString::from(v))),
        )
        .unwrap();
        let rustc = RustcUnitV2::new(
            "/workspace",
            vec![
                "/opt/rustc".into(),
                "--crate-name=native_v5_pack_test".into(),
                "input.rs".into(),
                "--crate-type=lib".into(),
                "-Zcodegen-backend=/opt/backend.so".into(),
            ],
        )
        .unwrap();
        let invocation = RustcInvocationDescriptorV3::new(
            RustcInvocationDescriptorV2::new([4; 32], [6; 32], rustc, environment).unwrap(),
            CompilerClosureV2::new([1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]).unwrap(),
        )
        .unwrap();
        let target = DeviceTargetV1::parse(profile.device_target()).unwrap();
        let module = Module::new(
            CompilerModuleKindV1::LlvmTextIr,
            target,
            CodeObjectVersion::V6,
            CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
                .unwrap(),
            CompilerModuleSymbolManifestV1::new([
                (CompilerModuleSymbolRoleV1::KernelEntry, "kernel"),
                (CompilerModuleSymbolRoleV1::KernelDescriptor, "kernel.kd"),
            ])
            .unwrap(),
            b"define amdgpu_kernel void @kernel() { ret void }\n",
        )
        .unwrap();
        let history = (0..history_len)
            .map(|n| (n % 251) as u8)
            .collect::<Vec<_>>();
        let output = NativeConditionalOutputLayoutV1::new::<()>(
            history.len(),
            CATALOG.len(),
            DESCRIPTOR.len(),
        )
        .unwrap();
        let carrier_layout =
            NativeConditionalCarrierLayoutV1::new::<()>(output.encoded_len(), SOURCE.len())
                .unwrap();
        let mut carrier = vec![0; carrier_layout.encoded_len()];
        let bytes = &mut carrier[carrier_layout.output_range()];
        bytes[output.history_range()].copy_from_slice(&history);
        bytes[output.catalog_range()].copy_from_slice(CATALOG);
        bytes[output.descriptor_range()].copy_from_slice(DESCRIPTOR);
        seal_native_conditional_output_v1(output, bytes, LIMIT, |_| Ok::<_, ()>(())).unwrap();
        carrier[carrier_layout.source_range()].copy_from_slice(SOURCE);
        let carrier_id =
            seal_native_conditional_carrier_v1(carrier_layout, &mut carrier, LIMIT, |_| {
                Ok::<_, ()>(())
            })
            .unwrap();
        let axis = |digest, len| TargetLineageIdentityV3::new(digest, len).unwrap();
        let lowering = InertNativeLoweringAssociationV1::new(NativeLoweringAssociationInputsV1 {
            final_native: InertNativeNeutralSubjectV1::new([1; 32], 7, [2; 32], 11).unwrap(),
            carrier: axis(*carrier_id.sha256(), carrier_id.byte_len()),
            descriptor: axis(Sha256::digest(DESCRIPTOR).into(), DESCRIPTOR.len() as u64),
            pre_descriptor_llvm: axis([3; 32], 1),
            final_llvm: axis(
                *module.module_identity().sha256(),
                module.module_identity().byte_len(),
            ),
            module_handoff: axis(*module.identity().sha256(), module.identity().byte_len()),
            profile,
        });
        Self {
            carrier,
            policy_roster: encode_native_conditional_policy_roster_v1(
                NativeConditionalPolicyRosterInputV1 {
                    source_packet: SOURCE,
                    roots: &[
                        NativeConditionalPolicyRootInputV1 {
                            semantic_root: 9,
                            kernel_binding: [7; 32],
                            effect_signers: &[[1; 32], [2; 32]],
                            effect_toolchain: [[3; 32]; 5],
                            formula_verifying_key: [4; 32],
                            formula_toolchain: [[5; 32]; 5],
                            formula_boundary: 3,
                        },
                        NativeConditionalPolicyRootInputV1 {
                            semantic_root: 2,
                            kernel_binding: [8; 32],
                            effect_signers: &[[6; 32]],
                            effect_toolchain: [[7; 32]; 5],
                            formula_verifying_key: [8; 32],
                            formula_toolchain: [[9; 32]; 5],
                            formula_boundary: 2,
                        },
                    ],
                },
                LIMIT,
                |_| Ok::<_, ()>(()),
            )
            .unwrap(),
            history,
            invocation: encode_descriptor_v3(&invocation).unwrap(),
            layout: canonical_semantic_target_layout_transcript_v1(
                profile.rustc_target(),
                "inert fixture layout",
                64,
                profile.cpu(),
                profile.rustc_features(),
            )
            .unwrap(),
            lowering,
            commitment: Commitment::from_handoff(&module).unwrap(),
            module,
        }
    }

    fn input(&self) -> NativeConditionalMetadataInputV1<'_> {
        NativeConditionalMetadataInputV1 {
            invocation: &self.invocation,
            rustc_inventory: INVENTORY,
            rustc_preflight: PREFLIGHT,
            semantic_target_layout: &self.layout,
            native_lowering: self.lowering.canonical_bytes(),
            final_module_commitment: self.commitment.canonical_bytes(),
        }
    }

    fn layouts(
        &self,
    ) -> (
        NativeConditionalMetadataLayoutV1,
        NativeConditionalMetadataLayoutV2,
        InertProductionSemanticCapsuleLayoutV5,
        Outer,
    ) {
        let metadata = NativeConditionalMetadataLayoutV1::new::<()>(self.input()).unwrap();
        let wrapper = NativeConditionalMetadataLayoutV2::new::<()>(
            metadata.encoded_len(),
            self.policy_roster.len(),
        )
        .unwrap();
        let capsule = InertProductionSemanticCapsuleLayoutV5::new::<()>(
            wrapper.encoded_len(),
            self.carrier.len(),
        )
        .unwrap();
        let outer = Outer::new(capsule.encoded_len(), self.module.canonical_bytes().len()).unwrap();
        (metadata, wrapper, capsule, outer)
    }

    fn buffer(&self, spare: bool) -> Vec<u8> {
        let capacity = if spare {
            self.layouts().3.encoded_len() + 257
        } else {
            self.carrier.len()
        };
        let mut bytes = Vec::with_capacity(capacity);
        bytes.extend_from_slice(&self.carrier);
        bytes
    }
}

#[test]
fn conditional_native_pack_roundtrips_relocation_and_all_metadata_leaves() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for (history_len, overlapping) in [(1, false), (16 * 1024, true)] {
            let fixture = Fixture::new(profile, history_len);
            let (_, wrapper, capsule, outer) = fixture.layouts();
            let destination = outer.capsule_range().start + capsule.carrier_range().start;
            assert_eq!(destination < fixture.carrier.len(), overlapping);
            let mut first = None;
            for spare in [false, true] {
                let bytes = fixture.buffer(spare);
                let pointer = bytes.as_ptr();
                let capacity = bytes.capacity();
                assert_eq!(capacity >= outer.encoded_len(), spare);
                let mut work = Work::new(WORK);
                let mut budget = Budget::new(&mut work, LIMIT);
                // Component-only ledger: fixtures are borrowed test data, not original custody.
                let floor = INHERITED + SCRATCH + capacity;
                budget.reserve_storage(floor).unwrap();
                let account = budget.work_ledger_identity_v1();
                let bytes = finish_carrier(
                    bytes,
                    fixture.input(),
                    &fixture.policy_roster,
                    &fixture.module,
                    &mut budget,
                )
                .unwrap();
                assert!(budget.work_ledger_identity_v1() == account);
                assert_eq!(budget.storage(), INHERITED + SCRATCH + bytes.capacity());
                assert_eq!(bytes.len(), outer.encoded_len());
                if spare {
                    assert_eq!(bytes.capacity(), capacity);
                    assert_eq!(bytes.as_ptr(), pointer);
                }
                assert_eq!(
                    &bytes[destination..destination + fixture.carrier.len()],
                    fixture.carrier
                );
                if let Some(expected) = &first {
                    assert_eq!(&bytes, expected);
                } else {
                    first = Some(bytes.clone());
                }
                budget.reserve_storage(DECODE_STORAGE).unwrap();
                budget
                    .charge_work(
                        inert_semantic_compiler_module_handoff_decode_work_v5(bytes.len()).unwrap(),
                    )
                    .unwrap();
                let pointer = bytes.as_ptr();
                let capacity = bytes.capacity();
                let decoded = Handoff::decode_owned(bytes).unwrap();
                assert_eq!(decoded.canonical_bytes().as_ptr(), pointer);
                assert_eq!(decoded.backing_capacity(), capacity);
                assert_eq!(decoded.module_handoff().backing_capacity(), capacity);
                let cap = decoded.capsule();
                assert_eq!(cap.target(), fixture.module.target());
                assert_eq!(cap.carrier_bytes(), fixture.carrier);
                assert_eq!(
                    cap.carrier_bytes().as_ptr(),
                    decoded.canonical_bytes()[destination..].as_ptr()
                );
                assert_eq!(cap.history_bytes(), fixture.history);
                assert_eq!(cap.catalog_bytes(), CATALOG);
                assert_eq!(cap.descriptor_bytes(), DESCRIPTOR);
                assert_eq!(cap.source_packet_bytes(), SOURCE);
                let original_carrier = read_native_conditional_carrier_v1(
                    &fixture.carrier,
                    LIMIT,
                    |_| Ok::<_, ()>(()),
                )
                .unwrap();
                assert_eq!(cap.carrier_identity(), original_carrier.identity());
                let frame =
                    read_inert_production_semantic_capsule_v5(cap.canonical_bytes(), LIMIT, |_| {
                        Ok::<_, ()>(())
                    })
                    .unwrap();
                let metadata = frame.metadata();
                let wrapper_base = outer.capsule_range().start + capsule.metadata_range().start;
                assert_eq!(cap.policy_roster_bytes(), fixture.policy_roster,);
                assert_eq!(
                    cap.policy_roster_bytes().as_ptr(),
                    decoded.canonical_bytes()[wrapper_base + wrapper.policy_roster_range().start..]
                        .as_ptr(),
                );
                assert_eq!(
                    frame.policy_roster().source_packet_sha256(),
                    &<[u8; 32]>::from(Sha256::digest(SOURCE)),
                );
                assert_eq!(
                    frame.policy_roster().source_packet_len(),
                    SOURCE.len() as u64
                );
                assert_eq!(
                    frame
                        .policy_roster()
                        .roots()
                        .map(|root| root.semantic_root())
                        .collect::<Vec<_>>(),
                    [9, 2],
                );
                for (actual, expected) in [
                    (metadata.invocation(), fixture.input().invocation),
                    (metadata.rustc_inventory(), INVENTORY),
                    (metadata.rustc_preflight(), PREFLIGHT),
                    (
                        metadata.semantic_target_layout_bytes(),
                        fixture.input().semantic_target_layout,
                    ),
                    (metadata.native_lowering(), fixture.input().native_lowering),
                    (
                        metadata.final_module_commitment(),
                        fixture.input().final_module_commitment,
                    ),
                ] {
                    assert_eq!(actual, expected);
                }
                assert_eq!(
                    encode_descriptor_v3(cap.invocation()).unwrap(),
                    fixture.invocation
                );
                assert_eq!(
                    cap.rustc_identity_inventory().canonical_preimage(),
                    INVENTORY
                );
                assert_eq!(cap.rustc_preflight_plan().canonical_preimage(), PREFLIGHT);
                assert_eq!(cap.semantic_target_layout_bytes(), &*fixture.layout);
                assert_eq!(
                    cap.native_lowering().canonical_bytes(),
                    fixture.lowering.canonical_bytes()
                );
                assert_eq!(
                    cap.final_module_commitment_bytes(),
                    fixture.commitment.canonical_bytes()
                );
                assert_eq!(
                    decoded.module_handoff().canonical_bytes(),
                    fixture.module.canonical_bytes()
                );
                assert_eq!(
                    decoded.module_handoff().identity(),
                    fixture.module.identity()
                );
                assert!(!decoded.grants_authority());
                assert!(!cap.grants_authority());
                drop(decoded);
                // Only the decode owner/backing is dropped; no opaque terminal refund.
                budget.release_storage(DECODE_STORAGE + capacity).unwrap();
                assert_eq!(budget.storage(), INHERITED + SCRATCH);
                assert!(budget.work_ledger_identity_v1() == account);
            }
        }
    }
}

#[test]
fn conditional_native_pack_exact_work_and_one_short_are_terminal() {
    let fixture = Fixture::new(Profile::Gfx942, 16 * 1024);
    let run = |limit| {
        let bytes = fixture.buffer(true);
        let floor = INHERITED + SCRATCH + bytes.capacity();
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, floor);
        budget.reserve_storage(floor).unwrap();
        let account = budget.work_ledger_identity_v1();
        let result = finish_carrier(
            bytes,
            fixture.input(),
            &fixture.policy_roster,
            &fixture.module,
            &mut budget,
        );
        assert!(budget.work_ledger_identity_v1() == account);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
        (result, budget.work(), budget.failed_work())
    };
    let (result, exact, failure) = run(WORK);
    let expected = result.unwrap();
    assert_eq!(failure, None);
    let (result, work, failure) = run(exact);
    assert_eq!(result.unwrap(), expected);
    assert_eq!((work, failure), (exact, None));
    let (result, work, failure) = run(exact - 1);
    assert!(matches!(
        result,
        Err(Error::Seal(HandoffError::Charge(Resource::Work(_))))
    ));
    assert!(work < exact);
    assert_eq!(failure, Some(exact));
}

#[test]
fn conditional_native_pack_decode_quote_covers_v2_and_refuses_one_short_before_decode() {
    let fixture = Fixture::new(Profile::Gfx942, 16 * 1024);
    let (_, _, capsule, outer) = fixture.layouts();
    let decode_work =
        inert_semantic_compiler_module_handoff_decode_work_v5(outer.encoded_len()).unwrap();
    let run = |work_limit| {
        let bytes = fixture.buffer(true);
        let floor = INHERITED + SCRATCH + bytes.capacity() + DECODE_STORAGE;
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, floor);
        budget.reserve_storage(floor - DECODE_STORAGE).unwrap();
        let account = budget.work_ledger_identity_v1();
        let bytes = finish_carrier(
            bytes,
            fixture.input(),
            &fixture.policy_roster,
            &fixture.module,
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(DECODE_STORAGE).unwrap();
        let result = budget.charge_work(decode_work);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
        assert!(budget.work_ledger_identity_v1() == account);
        (result, bytes, budget.work(), budget.failed_work())
    };
    let (result, bytes, exact, failed) = run(WORK);
    result.unwrap();
    assert_eq!(failed, None);
    let capsule_bytes = &bytes[outer.capsule_range()];
    let mut recursive_work = 0;
    let frame = read_inert_production_semantic_capsule_v5(capsule_bytes, LIMIT, |w| {
        recursive_work += w;
        Ok::<_, ()>(())
    })
    .unwrap();
    assert!(recursive_work < decode_work);
    let mut metadata_v2_work = 0;
    read_native_conditional_metadata_v2(&capsule_bytes[capsule.metadata_range()], LIMIT, |w| {
        metadata_v2_work += w;
        Ok::<_, ()>(())
    })
    .unwrap();
    let mut metadata_v1_work = 0;
    read_native_conditional_metadata_v1(frame.metadata().canonical_bytes(), LIMIT, |w| {
        metadata_v1_work += w;
        Ok::<_, ()>(())
    })
    .unwrap();
    let added = metadata_v2_work - metadata_v1_work + SOURCE.len() + 160;
    assert!(added <= 6 * bytes.len() + 4096 + 2 * 128 * 127);
    Handoff::decode_owned(bytes).unwrap();

    let (result, bytes, used, failed) = run(exact);
    result.unwrap();
    assert_eq!((used, failed), (exact, None));
    Handoff::decode_owned(bytes).unwrap();
    let (result, _undecoded, used, failed) = run(exact - 1);
    assert!(matches!(result, Err(Resource::Work(_))));
    assert_eq!(used, exact - decode_work);
    assert_eq!(failed, Some(exact));
}

#[test]
fn conditional_native_pack_expanded_metadata_still_obeys_shared_capsule_cap() {
    assert_eq!(
        MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V5,
        MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3
    );
    let overhead = INERT_PRODUCTION_SEMANTIC_CAPSULE_HEADER_BYTES_V5 + 32;
    let metadata = MAX_NATIVE_CONDITIONAL_METADATA_BYTES_V2;
    let carrier = MAX_NATIVE_CONDITIONAL_CARRIER_BYTES_V1;
    let exact = InertProductionSemanticCapsuleLayoutV5::new::<()>(metadata, carrier).unwrap();
    assert_eq!(exact.encoded_len(), overhead + metadata + carrier);
    // Current member maxima are tighter than the unchanged aggregate ceiling.
    assert!(exact.encoded_len() <= MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V5);
    assert!(matches!(
        InertProductionSemanticCapsuleLayoutV5::new::<()>(metadata, carrier + 1),
        Err(InertProductionSemanticCapsuleErrorV5::CarrierLength),
    ));
    assert!(matches!(
        InertProductionSemanticCapsuleLayoutV5::new::<()>(metadata + 1, carrier),
        Err(InertProductionSemanticCapsuleErrorV5::MetadataLength),
    ));
}

#[test]
fn conditional_native_pack_one_short_growth_storage_keeps_inherited_floor() {
    let fixture = Fixture::new(Profile::Gfx950, 1);
    let bytes = fixture.buffer(false);
    let capacity = bytes.capacity();
    let floor = INHERITED + SCRATCH + capacity;
    let required = INHERITED + SCRATCH + fixture.layouts().3.encoded_len();
    assert!(required > floor);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, required - 1);
    budget.reserve_storage(floor).unwrap();
    let account = budget.work_ledger_identity_v1();
    assert!(matches!(
        finish_carrier(
            bytes,
            fixture.input(),
            &fixture.policy_roster,
            &fixture.module,
            &mut budget
        ),
        Err(Error::Resource(Resource::Storage(_)))
    ));
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.failed_storage(), Some(required));
    assert!(budget.work_ledger_identity_v1() == account);
}

#[test]
fn conditional_native_pack_every_leaf_mutation_breaks_outer_roundtrip() {
    let fixture = Fixture::new(Profile::Gfx942, 16 * 1024);
    let (metadata, wrapper, capsule, outer) = fixture.layouts();
    let bytes = fixture.buffer(true);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget
        .reserve_storage(INHERITED + SCRATCH + bytes.capacity())
        .unwrap();
    let bytes = finish_carrier(
        bytes,
        fixture.input(),
        &fixture.policy_roster,
        &fixture.module,
        &mut budget,
    )
    .unwrap();
    // Establish a positive content decode for this exact image before corrupting it.
    let original = Handoff::decode_owned(bytes.clone()).unwrap();
    assert!(!original.grants_authority());
    drop(original);
    let wrapper_base = outer.capsule_range().start + capsule.metadata_range().start;
    let metadata_base = wrapper_base + wrapper.metadata_range().start;
    let carrier_base = outer.capsule_range().start + capsule.carrier_range().start;
    let output = NativeConditionalOutputLayoutV1::new::<()>(
        fixture.history.len(),
        CATALOG.len(),
        DESCRIPTOR.len(),
    )
    .unwrap();
    let carrier =
        NativeConditionalCarrierLayoutV1::new::<()>(output.encoded_len(), SOURCE.len()).unwrap();
    let output_base = carrier_base + carrier.output_range().start;
    for offset in [
        metadata_base + metadata.invocation_range().start,
        metadata_base + metadata.rustc_inventory_range().start,
        metadata_base + metadata.rustc_preflight_range().start,
        metadata_base + metadata.semantic_target_layout_range().start,
        metadata_base + metadata.native_lowering_range().start,
        metadata_base + metadata.final_module_commitment_range().start,
        wrapper_base + wrapper.policy_roster_range().start,
        wrapper_base + wrapper.policy_roster_range().end - 1,
        output_base + output.history_range().start,
        output_base + output.catalog_range().start,
        output_base + output.descriptor_range().start,
        carrier_base + carrier.source_range().start,
        outer.module_handoff_range().start,
    ] {
        let mut changed = bytes.clone();
        changed[offset] ^= 1;
        assert_ne!(changed, bytes);
        assert!(Handoff::decode_owned(changed).is_err());
    }
}

#[test]
fn conditional_native_pack_requires_intact_roster_and_keeps_terminal_storage() {
    let fixture = Fixture::new(Profile::Gfx942, 1);
    let mut changed = fixture.policy_roster.clone();
    let last = changed.len() - 1;
    changed[last] ^= 1;
    let original =
        read_native_conditional_policy_roster_v1(
            &fixture.policy_roster,
            LIMIT,
            |_| Ok::<_, ()>(()),
        )
        .unwrap();
    let rows: Vec<_> = original
        .roots()
        .map(|row| NativeConditionalPolicyRootInputV1 {
            semantic_root: row.semantic_root(),
            kernel_binding: *row.kernel_binding(),
            effect_signers: row.effect_signers(),
            effect_toolchain: *row.effect_toolchain(),
            formula_verifying_key: *row.formula_verifying_key(),
            formula_toolchain: *row.formula_toolchain(),
            formula_boundary: row.formula_boundary(),
        })
        .collect();
    let mut different_source = SOURCE.to_vec();
    different_source[0] ^= 1;
    let mismatched = encode_native_conditional_policy_roster_v1(
        NativeConditionalPolicyRosterInputV1 {
            source_packet: &different_source,
            roots: &rows,
        },
        LIMIT,
        |_| Ok::<_, ()>(()),
    )
    .unwrap();
    for roster in [&[][..], changed.as_slice(), mismatched.as_slice()] {
        let bytes = fixture.buffer(true);
        let floor = INHERITED + SCRATCH + bytes.capacity();
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, floor + DECODE_STORAGE);
        budget.reserve_storage(floor).unwrap();
        let account = budget.work_ledger_identity_v1();
        let result = finish_carrier(bytes, fixture.input(), roster, &fixture.module, &mut budget);
        if roster.is_empty() {
            assert!(matches!(result, Err(Error::MetadataV2(_))));
            assert_eq!(budget.work(), 0);
            assert_eq!(budget.storage(), floor);
        } else {
            // Framing seals do not decode members. The existing consuming
            // continuation prepays and rejects the corrupted nested roster.
            let bytes = result.unwrap();
            budget.reserve_storage(DECODE_STORAGE).unwrap();
            budget
                .charge_work(
                    inert_semantic_compiler_module_handoff_decode_work_v5(bytes.len()).unwrap(),
                )
                .unwrap();
            assert!(Handoff::decode_owned(bytes).is_err());
            assert_eq!(budget.storage(), floor + DECODE_STORAGE);
        }
        assert_eq!(budget.storage(), budget.peak_storage());
        assert!(budget.work_ledger_identity_v1() == account);
    }
}

#[test]
fn conditional_native_pack_rejects_legacy_metadata_without_v2_roster() {
    let fixture = Fixture::new(Profile::Gfx950, 1);
    let (metadata, wrapper, capsule, outer) = fixture.layouts();
    let bytes = fixture.buffer(true);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget
        .reserve_storage(INHERITED + SCRATCH + bytes.capacity())
        .unwrap();
    let bytes = finish_carrier(
        bytes,
        fixture.input(),
        &fixture.policy_roster,
        &fixture.module,
        &mut budget,
    )
    .unwrap();
    let legacy = InertProductionSemanticCapsuleLayoutV5::new::<()>(
        metadata.encoded_len(),
        fixture.carrier.len(),
    )
    .unwrap();
    let mut legacy_bytes = vec![0; legacy.encoded_len()];
    legacy_bytes[legacy.metadata_range()].copy_from_slice(
        &bytes[outer.capsule_range()][capsule.metadata_range()][wrapper.metadata_range()],
    );
    legacy_bytes[legacy.carrier_range()].copy_from_slice(&fixture.carrier);
    let legacy_id =
        seal_inert_production_semantic_capsule_v5(legacy, &mut legacy_bytes, LIMIT, |_| {
            Ok::<_, ()>(())
        })
        .unwrap();
    let legacy_outer =
        Outer::new(legacy.encoded_len(), fixture.module.canonical_bytes().len()).unwrap();
    let mut bytes = vec![0; legacy_outer.encoded_len()];
    bytes[legacy_outer.capsule_range()].copy_from_slice(&legacy_bytes);
    bytes[legacy_outer.module_handoff_range()].copy_from_slice(fixture.module.canonical_bytes());
    seal_inert_semantic_compiler_module_handoff_v5(
        legacy_outer,
        &mut bytes,
        legacy_id,
        fixture.module.identity(),
        |_| Ok::<_, ()>(()),
    )
    .unwrap();
    // All outer seals are valid, but V5 requires the nested V2 roster.
    assert!(Handoff::decode_owned(bytes).is_err());
}
