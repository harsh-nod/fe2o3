use super::*;
use fe2o3_kernel_descriptor::mixed_conditional_v26::*;
use fe2o3_kernel_descriptor::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent, Module, Signature,
    Terminator, WorkgroupSize,
};
use fe2o3_verifier::with_mixed_target_selection_v53;

const LIMIT: usize = 256 << 20;
const FLOOR: usize = 19;
const SEMANTIC: &[u8] = b"inert host target-readmission content, not source proof";

fn free(_: usize) -> std::result::Result<(), ()> {
    Ok(())
}

fn returned_resource(error: &AdmissionError) -> Option<Resource> {
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(error) = current {
        if let Some(resource) = error.downcast_ref::<Resource>() {
            return Some(*resource);
        }
        current = error.source();
    }
    None
}

fn owner(count: usize, name: &str) -> (Owner, usize) {
    let mut module = Module::new(name);
    for i in 0..count {
        let entry = format!("body{i}");
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::kernel_entry(
            entry.as_str(),
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
        let mut kernel = Kernel::new(
            format!("kernel{i}").as_str(),
            entry.as_str(),
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v18(&module, LAYOUT_LIMITS, &mut budget)
            .unwrap();
    (owner, storage.retained_storage())
}

// Only nominal content labels and V26 subjects are constructed here. There are
// no execution signatures, protected receipts or fabricated semantic proofs.
fn descriptor(owner: &Owner, profile: ProductionAmdTargetProfileV1, mode: usize) -> Vec<u8> {
    let compiler = CompilerIdentityV1::new(
        Text::new("rustc").unwrap(),
        Text::new("nightly").unwrap(),
        [7; 20],
    );
    let producer = ProducerIdentityV1::new(
        Text::new("fe2o3").unwrap(),
        Text::new("inert-readmission-test").unwrap(),
    );
    let launch = LaunchConstraintsV1::new(
        1,
        BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
        DimensionsV1::new(2, 1, 1).unwrap(),
        256,
        0,
        0,
    )
    .unwrap();
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([11; 32]),
        EvidenceDigest::from_sha256_bytes([12; 32]),
    );
    let count = owner.module().kernels.len();
    let names = (0..count).map(|i| format!("kernel{i}")).collect::<Vec<_>>();
    let symbols = (0..count)
        .map(|i| format!("kernel{i}.kd"))
        .collect::<Vec<_>>();
    let kernels = (0..count)
        .map(|i| KernelDescriptorInputV3 {
            kernel_id: KernelId::from_bytes([13 + i as u8; 32]),
            logical_name: &names[i],
            entry_name: &names[i],
            descriptor_symbol: &symbols[i],
            source_evidence: evidence,
            executable_ir_evidence: evidence,
            capabilities: &[CapabilityV1::AmdWave],
            abi_layout: KernelAbiLayoutV1::new(0, 0, 8).unwrap(),
            launch: &launch,
            arguments: &[],
        })
        .collect::<Vec<_>>();
    let requirements = kernels
        .iter()
        .map(|k| {
            KernelTargetRequirementsV2::new(
                k.kernel_id,
                LdsRequirementsV2::new(0, 0).unwrap(),
                RequiredWavefrontWidthV2::Wave64,
                false,
                SynchronizationRequirementsV2::empty(),
                AtomicRequirementsV2::empty(),
            )
        })
        .collect::<Vec<_>>();
    let input = DeviceDescriptorTableInputV3 {
        canonical_code_object_digest: CanonicalCodeObjectDigest::from_bytes([0; 32]),
        code_object_version: fe2o3_kernel_descriptor::CodeObjectVersion::V6,
        compiler: &compiler,
        producer: &producer,
        device_target: DeviceTargetV1::parse(profile.device_target()).unwrap(),
        type_records: &[],
        layout_records: &[],
        kernels: &kernels,
        requirements: &requirements,
    };
    let mut nominal = vec![0; encoded_device_descriptor_table_v3_len(&input, &mut free).unwrap()];
    encode_device_descriptor_table_v3(&input, &mut nominal, &mut free).unwrap();
    let table = decode_device_descriptor_table_v3(&nominal, &mut free).unwrap();
    let digest = mixed_descriptor_subject_v26(&table, &mut free).unwrap();
    let contracts = (0..count)
        .map(|i| {
            let mut subjects = MixedContractSubjectsV26 {
                kernel_id: *kernels[i].kernel_id.as_bytes(),
                source_semantic_identity: Sha256::digest(SEMANTIC).into(),
                original_graph_identity: *owner.identity().digest(),
                output_graph_identity: *owner.identity().digest(),
                descriptor_identity: digest,
                original_root: i as u32,
                output_function: i as u32,
                source_rank: 1,
                index_width: 64,
                exact_grid: [128, 1, 1],
                source_argument_count: 0,
                generated_field_count: 0,
                explicit_argument_bytes: 0,
                kernarg_alignment: 8,
            };
            match mode {
                1 => subjects.output_graph_identity = [71; 32],
                2 => subjects.source_semantic_identity = [72; 32],
                3 => subjects.output_function = count as u32,
                4 => subjects.exact_grid = [64, 1, 1],
                _ => {}
            }
            let contract = MixedContractInputV26 {
                subjects,
                arguments: &[],
                occurrences: &[],
            };
            let mut wire = vec![0; encoded_mixed_contract_v26_len(&contract, &mut free).unwrap()];
            encode_mixed_contract_v26(&contract, &mut wire, &mut free).unwrap();
            wire
        })
        .collect::<Vec<_>>();
    let views = contracts
        .iter()
        .map(|bytes| decode_mixed_contract_v26(bytes, &mut free).unwrap())
        .collect::<Vec<_>>();
    let mut wire = vec![0; encoded_mixed_descriptor_v53_len(&nominal, &views, &mut free).unwrap()];
    encode_mixed_descriptor_v53(&nominal, &views, &mut wire, &mut free).unwrap();
    wire
}

struct Fixture {
    forwarded: Vec<u8>,
    semantic: InertCanonicalSemanticMirReceiptV3,
    invocation: TargetLineageIdentityV3,
    descriptor: Vec<u8>,
    profile: ProductionAmdTargetProfileV1,
    received: Vec<u8>,
}

impl Fixture {
    fn new(profile: ProductionAmdTargetProfileV1, count: usize) -> Self {
        let (owner, retained) = owner(count, "host-readmission");
        let descriptor = descriptor(&owner, profile, 0);
        let semantic =
            InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(SEMANTIC).unwrap();
        let invocation = TargetLineageIdentityV3::new([1; 32], 11).unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(retained + descriptor.len()).unwrap();
        let received = with_mixed_target_selection_v53(
            &MixedTargetSelectionSubjectV53 {
                owner: &owner,
                invocation,
                semantic_mir: &semantic,
                descriptor: &descriptor,
                profile,
            },
            &mut budget,
            |wire, _| Ok(wire.to_vec()),
        )
        .unwrap();
        Self {
            forwarded: owner.canonical_bytes().to_vec(),
            semantic,
            invocation,
            descriptor,
            profile,
            received,
        }
    }

    fn readmit(&self, budget: &mut Budget<'_>) -> Result<()> {
        readmit_target_selection_v53(
            &self.forwarded,
            &self.semantic,
            self.invocation,
            &self.descriptor,
            self.profile,
            &self.received,
            budget,
        )
    }
}

#[test]
fn mixed_v53_target_readmission_both_profiles_and_multiple_roots() {
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        for count in [1, 2] {
            let fixture = Fixture::new(profile, count);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR).unwrap();
            fixture.readmit(&mut budget).unwrap();
            assert_eq!(budget.storage(), FLOOR);
            assert!(budget.work() > fixture.forwarded.len());
            assert!(budget.peak_storage() > FLOOR + fixture.forwarded.len());
        }
    }
}

#[test]
fn mixed_v53_target_readmission_refuses_graph_receipt_descriptor_profile_and_context_changes() {
    for mode in 0..16 {
        let mut fixture = Fixture::new(ProductionAmdTargetProfileV1::Gfx942, 2);
        match mode {
            0 => fixture.forwarded[0] ^= 1,
            1 => {
                fixture.forwarded.pop();
            }
            2 => {
                fixture.forwarded.push(0);
            }
            3 => fixture.forwarded = owner(2, "foreign-owner").0.canonical_bytes().to_vec(),
            4 => fixture.received[0] ^= 1,
            5 => {
                fixture.received.pop();
            }
            6 => {
                fixture.received.push(0);
            }
            7 => fixture.profile = ProductionAmdTargetProfileV1::Gfx950,
            8 => fixture.invocation = TargetLineageIdentityV3::new([2; 32], 11).unwrap(),
            9 => fixture.invocation = TargetLineageIdentityV3::new([1; 32], 12).unwrap(),
            10 => {
                fixture.semantic = InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(
                    b"foreign inert semantic content".as_slice(),
                )
                .unwrap()
            }
            11 => fixture.descriptor[0] ^= 1,
            12 => fixture.descriptor = descriptor(&owner(2, "foreign-owner").0, fixture.profile, 0),
            13 => {
                fixture.descriptor = descriptor(
                    &owner(2, "host-readmission").0,
                    ProductionAmdTargetProfileV1::Gfx950,
                    0,
                )
            }
            14 => fixture.forwarded = owner(1, "host-readmission").0.canonical_bytes().to_vec(),
            15 => fixture.forwarded = owner(3, "host-readmission").0.canonical_bytes().to_vec(),
            _ => unreachable!(),
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(
            fixture.readmit(&mut budget).is_err(),
            "accepted mutation {mode}"
        );
        assert_eq!(budget.storage(), FLOOR, "mutation {mode}");
        assert!(
            budget.check_prior_denials_v1().is_ok(),
            "unexpected resource denial {mode}"
        );
    }
}

#[test]
fn mixed_v53_target_readmission_rechecks_well_formed_v26_subjects_against_actual_owner() {
    for mode in 1..=4 {
        let mut fixture = Fixture::new(ProductionAmdTargetProfileV1::Gfx942, 2);
        fixture.descriptor = descriptor(&owner(2, "host-readmission").0, fixture.profile, mode);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(
            fixture.readmit(&mut budget).is_err(),
            "accepted V26 mutation {mode}"
        );
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn mixed_v53_target_readmission_exact_and_one_short_limits_preserve_floor_and_first_denial() {
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        let fixture = Fixture::new(profile, 2);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        fixture.readmit(&mut budget).unwrap();
        let used = budget.work();
        let peak = budget.peak_storage();
        assert_eq!(budget.storage(), FLOOR);
        for (work_limit, storage_limit) in [(used, peak), (used - 1, peak), (used, peak - 1)] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(FLOOR).unwrap();
            let result = fixture.readmit(&mut budget);
            assert_eq!(budget.storage(), FLOOR);
            if (work_limit, storage_limit) == (used, peak) {
                result.unwrap();
                assert_eq!((budget.work(), budget.peak_storage()), (used, peak));
            } else {
                assert!(result.is_err());
                let denial = budget.check_prior_denials_v1().unwrap_err();
                assert_eq!(returned_resource(&result.unwrap_err()), Some(denial));
                assert!(
                    matches!(denial, Resource::Work(_)) && work_limit < used
                        || matches!(denial, Resource::Storage(_)) && storage_limit < peak
                );
                let before = (budget.work(), budget.storage(), budget.peak_storage());
                let repeated = fixture.readmit(&mut budget);
                assert!(repeated.is_err());
                assert_eq!(returned_resource(&repeated.unwrap_err()), Some(denial));
                assert_eq!(budget.check_prior_denials_v1().unwrap_err(), denial);
                assert_eq!(
                    (budget.work(), budget.storage(), budget.peak_storage()),
                    before
                );
            }
        }
    }
}

#[test]
fn mixed_v53_target_readmission_does_not_borrow_a_foreign_owners_ledger_credit() {
    let fixture = Fixture::new(ProductionAmdTargetProfileV1::Gfx942, 1);
    let (foreign, retained) = owner(1, "foreign-owner");
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut foreign_budget = Budget::new(&mut foreign_work, LIMIT);
    foreign_budget.reserve_storage(retained).unwrap();
    let before = (
        foreign_budget.work(),
        foreign_budget.storage(),
        foreign_budget.peak_storage(),
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, FLOOR);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(fixture.readmit(&mut budget).is_err());
    assert!(matches!(
        budget.check_prior_denials_v1(),
        Err(Resource::Storage(_))
    ));
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(
        (
            foreign_budget.work(),
            foreign_budget.storage(),
            foreign_budget.peak_storage()
        ),
        before
    );
    drop(foreign);
    foreign_budget.release_storage(retained).unwrap();
}
