//! Artifact-component tests, not fabricated native source/F owners. Upstream
//! native semantic recovery currently admits descriptor V1 only.
use super::*;
use crate::native_worker_replay::{NativeWorkerReplayErrorV1, reconstruct_raw};
use fe2o3_compiler_ffi::{
    CompilerFfiEnvelopeV1, CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1 as Role,
};
use fe2o3_kernel_descriptor::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[allow(dead_code)]
#[path = "../../fe2o3-kernel-descriptor/tests/support/conditional_v4.rs"]
mod descriptor_fixture;

#[allow(dead_code)]
mod elf_fixture {
    use crate as fe2o3_hsaco_finalize;
    include!("../tests/fixtures/worker_v3_hsaco_test_support.rs");

    pub(super) fn raw(wire: &[u8], version: u8) -> Vec<u8> {
        let mut bytes = slice_fixture_with_descriptor_table_and_workgroup(wire, 256).bytes;
        let name = bytes
            .windows(13)
            .position(|w| w == b".fe2o3.kd.v1\0")
            .unwrap();
        bytes[name + 11] = b'0' + version;
        bytes
    }
}

const TARGET: &str = "gfx942:xnack-";
const SCRATCH: usize = NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V4;

fn free(_: usize) -> std::result::Result<(), &'static str> {
    Ok(())
}

fn wires(
    release: &str,
    customize: impl FnMut(&mut descriptor_fixture::Fixture),
) -> (Vec<u8>, Vec<u8>) {
    descriptor_fixture::with_custom_contracts(TARGET, 1, 0, customize, |input| {
        let compiler = CompilerIdentityV1::new(
            Text::new("rustc").unwrap(),
            Text::new(release).unwrap(),
            [7; 20],
        );
        let launch = LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(256, 1, 1).unwrap()),
            DimensionsV1::new(1024, 1, 1).unwrap(),
            256,
            0,
            0,
        )
        .unwrap();
        let kernel = &input.nominal.kernels[0];
        let kernels = [KernelDescriptorInputV3 {
            kernel_id: kernel.kernel_id,
            logical_name: "vecadd",
            entry_name: "vecadd",
            descriptor_symbol: "vecadd.kd",
            source_evidence: kernel.source_evidence,
            executable_ir_evidence: kernel.executable_ir_evidence,
            capabilities: kernel.capabilities,
            abi_layout: KernelAbiLayoutV1::new(16, 272, 8).unwrap(),
            launch: &launch,
            arguments: kernel.arguments,
        }];
        let input = DeviceDescriptorTableInputV4 {
            nominal: DeviceDescriptorTableInputV3 {
                compiler: &compiler,
                kernels: &kernels,
                ..input.nominal
            },
            contracts: input.contracts,
        };
        let mut v4 = vec![0; encoded_device_descriptor_table_v4_len(&input, &mut free).unwrap()];
        encode_device_descriptor_table_v4(&input, &mut v4, &mut free).unwrap();
        let mut v3 =
            vec![0; encoded_device_descriptor_table_v3_len(&input.nominal, &mut free).unwrap()];
        encode_device_descriptor_table_v3(&input.nominal, &mut v3, &mut free).unwrap();
        (v4, v3)
    })
}

fn inspect(raw: &[u8], launch: WorkerV3LaunchContractV1) -> SharedWorkerV3HsacoInspectionV1 {
    let target = DeviceTargetV1::parse(TARGET).unwrap();
    let manifest = CompilerModuleSymbolManifestV1::new([
        (Role::KernelEntry, "vecadd"),
        (Role::KernelDescriptor, "vecadd.kd"),
    ])
    .unwrap();
    inspect_worker_v3_hsaco_preimage_v1(
        target,
        CodeObjectVersion::V6,
        manifest,
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .unwrap()
            .identity(),
        ContentIdentityV1::calculate(raw),
        raw,
        launch,
    )
    .unwrap()
}

fn finish(
    schema: DescriptorSchema,
    raw: &[u8],
    abi: &[u8],
    budget: &mut Budget<'_>,
) -> Result<(Vec<u8>, Vec<u8>, CanonicalCodeObjectDigest)> {
    let launch = derive_launch(schema, abi, raw, budget)?;
    let inspected = inspect(raw, launch);
    finalize_artifact(
        schema,
        raw,
        ContentIdentityV1::calculate(raw),
        &inspected.policy,
        abi,
        budget,
    )
}

#[test]
fn native_v4_artifact_component_roundtrip_retains_exact_contract_and_only_patches_digest() {
    let (wire, _) = wires("native-v4", |_| {});
    let raw = elf_fixture::raw(&wire, 4);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 11 + SCRATCH);
    budget.reserve_storage(11).unwrap();
    budget.charge_work(7).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let schema = NativeDescriptorMode::V4.from_abi(&wire).unwrap();
    let (bytes, descriptor, digest) = finish(schema, &raw, &wire, &mut budget).unwrap();
    assert_eq!(budget.storage(), 11);
    assert!(budget.work() > 7);
    assert!(budget.work_ledger_identity_v1() == ledger);
    let inspected = crate::inspect_finalized_nominal_hsaco_v4(&bytes, SCRATCH, &mut free).unwrap();
    assert_eq!(inspected.digest(), digest);
    let location = inspected.location();
    let start = location.digest_offset();
    assert_eq!(&raw[start..start + 32], &[0; 32]);
    assert_eq!(&bytes[..start], &raw[..start]);
    assert_eq!(&bytes[start + 32..], &raw[start + 32..]);
    assert_eq!(
        descriptor,
        bytes[location.offset()..location.offset() + location.size()]
    );
    let original = decode_device_descriptor_table_v4(&wire, &mut free).unwrap();
    let original_kernel = original.kernel(0, &mut free).unwrap();
    let actual_kernel = inspected.descriptor_table().kernel(0, &mut free).unwrap();
    assert_eq!(
        original_kernel
            .conditional_contract(&mut free)
            .unwrap()
            .canonical_bytes(),
        actual_kernel
            .conditional_contract(&mut free)
            .unwrap()
            .canonical_bytes(),
    );
    let before = budget.work();
    assert_eq!(
        reconstruct_raw(NativeDescriptorMode::V4, &wire, &bytes, &mut budget).unwrap(),
        raw
    );
    assert!(budget.work() > before);
    assert_eq!(budget.storage(), 11);
    assert!(!inspected.grants_launch_authority());
}

#[test]
fn native_schema_entrypoints_are_disjoint_and_nominal_v3_regresses() {
    let (wire, v3) = wires("schemas", |_| {});
    for version in [0u16, 1, 2, 3, 4, 5, 0x104, u16::MAX] {
        let mut abi = wire.clone();
        abi[8..10].copy_from_slice(&version.to_le_bytes());
        assert_eq!(
            NativeDescriptorMode::V4.from_abi(&abi).is_ok(),
            version == 4
        );
        assert_eq!(
            NativeDescriptorMode::Legacy.from_abi(&abi).is_ok(),
            matches!(version, 1 | 3)
        );
    }
    for end in 0..10 {
        assert!(NativeDescriptorMode::V4.from_abi(&wire[..end]).is_err());
        assert!(NativeDescriptorMode::Legacy.from_abi(&wire[..end]).is_err());
    }
    let mut bad_magic = wire.clone();
    bad_magic[0] ^= 1;
    assert!(NativeDescriptorMode::V4.from_abi(&bad_magic).is_err());
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let raw = elf_fixture::raw(&v3, 3);
    let schema = NativeDescriptorMode::Legacy.from_abi(&v3).unwrap();
    let (bytes, _, _) = finish(schema, &raw, &v3, &mut budget).unwrap();
    assert_eq!(
        reconstruct_raw(NativeDescriptorMode::Legacy, &v3, &bytes, &mut budget).unwrap(),
        raw
    );
    assert!(reconstruct_raw(NativeDescriptorMode::V4, &v3, &bytes, &mut budget).is_err());
    assert!(reconstruct_raw(NativeDescriptorMode::V4, &wire, &bytes, &mut budget).is_err());
    let raw = elf_fixture::raw(&wire, 4);
    let (bytes, _, _) = finish(DescriptorSchema::NominalV4, &raw, &wire, &mut budget).unwrap();
    assert!(reconstruct_raw(NativeDescriptorMode::Legacy, &wire, &bytes, &mut budget).is_err());
    assert!(reconstruct_raw(NativeDescriptorMode::Legacy, &v3, &bytes, &mut budget).is_err());
}

#[test]
fn native_v4_rejects_valid_source_contract_and_schema_substitution() {
    let (wire, v3) = wires("exact", |_| {});
    let (foreign, _) = wires("foreign", |_| {});
    let (contract, _) = wires("exact", |contract| {
        contract.arguments[0].adjusted_argument = 8
    });
    let raw = elf_fixture::raw(&wire, 4);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    for abi in [&foreign, &contract] {
        decode_device_descriptor_table_v4(abi, &mut free).unwrap();
        let error = finish(DescriptorSchema::NominalV4, &raw, abi, &mut budget).unwrap_err();
        assert!(matches!(
            error,
            NativeWorkerFinalizationErrorV1::Artifact {
                phase: "conditional finalization",
                ..
            }
        ));
        // An independently finalized, valid replacement still cannot match the
        // original ABI when replay reaches the finalization join.
        let other = elf_fixture::raw(abi, 4);
        let (bytes, _, _) = finish(DescriptorSchema::NominalV4, &other, abi, &mut budget).unwrap();
        let reconstructed =
            reconstruct_raw(NativeDescriptorMode::V4, &wire, &bytes, &mut budget).unwrap();
        assert!(
            finish(
                DescriptorSchema::NominalV4,
                &reconstructed,
                &wire,
                &mut budget
            )
            .is_err()
        );
    }
    for (raw, abi) in [
        (elf_fixture::raw(&wire, 3), &wire),
        (elf_fixture::raw(&wire, 5), &wire),
        (elf_fixture::raw(&v3, 4), &wire),
        (elf_fixture::raw(&wire, 4), &v3),
    ] {
        assert!(finish(DescriptorSchema::NominalV4, &raw, abi, &mut budget).is_err());
    }
}

#[test]
fn native_v4_shared_inspection_keeps_exact_symbols_target_export_and_artifact_integrity() {
    let (wire, _) = wires("strict", |_| {});
    let raw = elf_fixture::raw(&wire, 4);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let launch = derive_launch(DescriptorSchema::NominalV4, &wire, &raw, &mut budget).unwrap();
    let target = DeviceTargetV1::parse(TARGET).unwrap();
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .unwrap();
    for (entry, symbol, expected_target) in [
        ("foreign", "vecadd.kd", target),
        ("vecadd", "foreign.kd", target),
        (
            "vecadd",
            "vecadd.kd",
            DeviceTargetV1::parse("gfx950:xnack-").unwrap(),
        ),
    ] {
        let manifest = CompilerModuleSymbolManifestV1::new([
            (Role::KernelEntry, entry),
            (Role::KernelDescriptor, symbol),
        ])
        .unwrap();
        let error = inspect_worker_v3_hsaco_preimage_v1(
            expected_target,
            CodeObjectVersion::V6,
            manifest,
            envelope.identity(),
            ContentIdentityV1::calculate(&raw),
            &raw,
            launch,
        )
        .err()
        .unwrap();
        use crate::WorkerV3HsacoInspectionError as Error;
        assert!(matches!(
            (entry, symbol, error),
            ("foreign", _, Error::KernelEntryRoleMismatch)
                | (_, "foreign.kd", Error::KernelDescriptorRoleMismatch)
                | ("vecadd", "vecadd.kd", Error::TargetMismatch { .. })
        ));
    }
    assert!(matches!(
        check_export_manifest(b"source", b"module", &mut budget),
        Err(NativeWorkerFinalizationErrorV1::Artifact {
            phase: "export manifest",
            ..
        })
    ));
    let (mut bytes, _, _) = finish(DescriptorSchema::NominalV4, &raw, &wire, &mut budget).unwrap();
    use object::{Object as _, ObjectSection as _};
    let text_offset = object::File::parse(bytes.as_slice())
        .unwrap()
        .section_by_name(".text")
        .unwrap()
        .file_range()
        .unwrap()
        .0 as usize;
    bytes[text_offset] ^= 1;
    assert!(reconstruct_raw(NativeDescriptorMode::V4, &wire, &bytes, &mut budget).is_err());
}

#[test]
fn native_v4_descriptor_and_reconstruction_budgets_are_cumulative() {
    let (wire, _) = wires("resources", |_| {});
    let raw = elf_fixture::raw(&wire, 4);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let (bytes, _, _) = finish(DescriptorSchema::NominalV4, &raw, &wire, &mut budget).unwrap();
    let final_work = budget.work();
    reconstruct_raw(NativeDescriptorMode::V4, &wire, &bytes, &mut budget).unwrap();
    let replay_work = budget.work() - final_work;
    for replay in [false, true] {
        let needed = if replay { replay_work } else { final_work };
        for (work_limit, storage_limit, success) in [
            (7 + needed, 11 + SCRATCH, true),
            (7 + needed - 1, 11 + SCRATCH, false),
            (7 + needed, 11 + SCRATCH - 1, false),
        ] {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(11).unwrap();
            budget.charge_work(7).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = if replay {
                reconstruct_raw(NativeDescriptorMode::V4, &wire, &bytes, &mut budget).map(|_| ())
            } else {
                finish(DescriptorSchema::NominalV4, &raw, &wire, &mut budget)
                    .map(|_| ())
                    .map_err(NativeWorkerReplayErrorV1::from)
            };
            assert_eq!(result.is_ok(), success, "{result:?}");
            assert_eq!(budget.storage(), 11);
            assert!(budget.work_ledger_identity_v1() == ledger);
            if success {
                assert_eq!(budget.work(), work_limit);
            } else if storage_limit == 11 + SCRATCH - 1 {
                assert!(matches!(
                    result,
                    Err(NativeWorkerReplayErrorV1::Resource(Resource::Storage(_)))
                ));
                assert_eq!(budget.failed_storage(), Some(11 + SCRATCH));
            } else {
                assert!(matches!(
                    result,
                    Err(NativeWorkerReplayErrorV1::Resource(Resource::Work(_)))
                ));
            }
        }
    }
}

#[test]
fn native_v4_nested_resource_failures_keep_exact_variants() {
    let mut work = Work::new(7);
    let mut budget = Budget::new(&mut work, 11);
    budget.charge_work(3).unwrap();
    budget.reserve_storage(5).unwrap();
    for resource in [
        budget.charge_work(5).unwrap_err(),
        budget.reserve_storage(7).unwrap_err(),
        Resource::Allocation,
        Resource::Accounting,
        Resource::Arithmetic,
    ] {
        for error in [
            NominalFinalizationErrorV4::Work(resource),
            NominalFinalizationErrorV4::Wire(DescriptorWireErrorV4::Nominal(
                DescriptorWireErrorV3::Work(resource),
            )),
            NominalFinalizationErrorV4::Wire(DescriptorWireErrorV4::Contract(
                ConditionalInvocationWireErrorV1::Work(resource),
            )),
        ] {
            let error = conditional_failure("conditional descriptor", error);
            assert!(
                matches!(error, NativeWorkerFinalizationErrorV1::Resource(actual) if actual == resource)
            );
            assert!(
                matches!(NativeWorkerReplayErrorV1::from(error), NativeWorkerReplayErrorV1::Resource(actual) if actual == resource)
            );
        }
    }
    assert_eq!(budget.storage(), 5);
    assert_eq!(budget.work(), 3);
}

#[test]
fn native_v4_identity_is_separate_and_binds_every_existing_axis() {
    let (wire, _) = wires("identity", |_| {});
    let raw = elf_fixture::raw(&wire, 4);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let launch = derive_launch(DescriptorSchema::NominalV4, &wire, &raw, &mut budget).unwrap();
    let mut inspected = inspect(&raw, launch);
    let (bytes, descriptor, digest) =
        finish(DescriptorSchema::NominalV4, &raw, &wire, &mut budget).unwrap();
    let output = ContentIdentityV1::calculate(&bytes);
    let descriptor = ContentIdentityV1::calculate(&descriptor);
    let identity = |mode,
                    source,
                    binding,
                    inspection: &SharedWorkerV3HsacoInspectionV1,
                    output,
                    descriptor,
                    digest| {
        finalization_identity(
            mode, source, binding, inspection, output, descriptor, digest,
        )
    };
    let original = identity(
        NativeDescriptorMode::V4,
        &[1; 32],
        &[2; 32],
        &inspected,
        output,
        descriptor,
        digest,
    );
    let mut expected_legacy = Sha256::new();
    expected_legacy.update(b"FE2O3/NATIVE-WORKER-CANONICAL-FINALIZATION/V1\0");
    expected_legacy.update([1; 32]);
    expected_legacy.update([2; 32]);
    expected_legacy.update(inspected.policy.identity().as_bytes());
    expected_legacy.update(inspected.descriptor_identity);
    expected_legacy.update(inspected.abi_identity);
    expected_legacy.update(inspected.resource_identity);
    for content in [output, descriptor] {
        expected_legacy.update(content.sha256());
        expected_legacy.update(content.byte_len().to_le_bytes());
    }
    expected_legacy.update(digest.as_bytes());
    let expected_legacy: [u8; 32] = expected_legacy.finalize().into();
    assert_eq!(
        identity(
            NativeDescriptorMode::Legacy,
            &[1; 32],
            &[2; 32],
            &inspected,
            output,
            descriptor,
            digest
        ),
        expected_legacy
    );
    for other in [
        identity(
            NativeDescriptorMode::Legacy,
            &[1; 32],
            &[2; 32],
            &inspected,
            output,
            descriptor,
            digest,
        ),
        identity(
            NativeDescriptorMode::V4,
            &[3; 32],
            &[2; 32],
            &inspected,
            output,
            descriptor,
            digest,
        ),
        identity(
            NativeDescriptorMode::V4,
            &[1; 32],
            &[3; 32],
            &inspected,
            output,
            descriptor,
            digest,
        ),
        identity(
            NativeDescriptorMode::V4,
            &[1; 32],
            &[2; 32],
            &inspected,
            ContentIdentityV1::from_parts([3; 32], output.byte_len()),
            descriptor,
            digest,
        ),
        identity(
            NativeDescriptorMode::V4,
            &[1; 32],
            &[2; 32],
            &inspected,
            ContentIdentityV1::from_parts(*output.sha256(), output.byte_len() + 1),
            descriptor,
            digest,
        ),
        identity(
            NativeDescriptorMode::V4,
            &[1; 32],
            &[2; 32],
            &inspected,
            output,
            ContentIdentityV1::from_parts([3; 32], descriptor.byte_len()),
            digest,
        ),
        identity(
            NativeDescriptorMode::V4,
            &[1; 32],
            &[2; 32],
            &inspected,
            output,
            ContentIdentityV1::from_parts(*descriptor.sha256(), descriptor.byte_len() + 1),
            digest,
        ),
        identity(
            NativeDescriptorMode::V4,
            &[1; 32],
            &[2; 32],
            &inspected,
            output,
            descriptor,
            CanonicalCodeObjectDigest::from_bytes([3; 32]),
        ),
    ] {
        assert_ne!(original, other);
    }
    for field in 0..3 {
        let value = match field {
            0 => &mut inspected.descriptor_identity,
            1 => &mut inspected.abi_identity,
            _ => &mut inspected.resource_identity,
        };
        value[0] ^= 1;
        assert_ne!(
            original,
            identity(
                NativeDescriptorMode::V4,
                &[1; 32],
                &[2; 32],
                &inspected,
                output,
                descriptor,
                digest
            )
        );
        match field {
            0 => inspected.descriptor_identity[0] ^= 1,
            1 => inspected.abi_identity[0] ^= 1,
            _ => inspected.resource_identity[0] ^= 1,
        }
    }
    // Policy identity includes the exact compiler-envelope coordinate, separate
    // from physical descriptor/ABI/resource observations.
    let target = DeviceTargetV1::parse(TARGET).unwrap();
    let other_envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V5)
            .unwrap();
    let other = inspect_worker_v3_hsaco_preimage_v1(
        target,
        CodeObjectVersion::V6,
        inspected.policy.symbol_manifest().clone(),
        other_envelope.identity(),
        ContentIdentityV1::calculate(&raw),
        &raw,
        launch,
    )
    .unwrap();
    assert_ne!(inspected.policy.identity(), other.policy.identity());
    assert_ne!(
        original,
        identity(
            NativeDescriptorMode::V4,
            &[1; 32],
            &[2; 32],
            &other,
            output,
            descriptor,
            digest
        )
    );
}
