//! Inert exact Worker transactions, not authenticated typed proof execution.
use super::*;
use fe2o3_hsaco_finalize::{
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V53 as MIXED_SCRATCH, NominalFinalizationErrorV53,
    NominalWorkerFinalizationErrorV53, derive_unfinalized_nominal_hsaco_v53,
    finalize_protected_worker_nominal_hsaco_v53, inspect_finalized_nominal_hsaco_v53,
    persist_prepared_mixed_worker_publication_v53,
    prepare_mixed_worker_compact_finalizer_replay_v53, prepare_mixed_worker_publication_v53,
    recover_mixed_worker_publication_v53, recover_nominal_worker_publication_v5,
};
use fe2o3_kernel_descriptor::mixed_conditional_v26::*;
use fe2o3_kernel_descriptor::{
    decode_device_descriptor_table_v3, decode_mixed_descriptor_v53, encode_mixed_descriptor_v53,
    encoded_mixed_descriptor_v53_len,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkBudgetV1,
};

#[test]
fn mixed_v53_real_budget_finalizes_both_targets_and_preserves_exact_denials() {
    use fe2o3_hsaco_finalize::{
        MIXED_WORKER_FINALIZATION_STORAGE_LIMIT_V53, MIXED_WORKER_FINALIZATION_WORK_LIMIT_V53,
        MixedWorkerFinalizationBudgetErrorV53,
        finalize_protected_worker_nominal_hsaco_on_budget_v53,
    };
    fn resource(mut error: &(dyn std::error::Error + 'static)) -> Resource {
        loop {
            if let Some(error) = error.downcast_ref::<Resource>() {
                return *error;
            }
            error = error.source().expect("lossless nested resource cause");
        }
    }
    for target in [TARGET, "gfx950:xnack-"] {
        let wire = mixed_wire(target, 2);
        let bytes = mixed_artifact(&wire, target);
        let make = || {
            let directory = TestDirectory::new();
            let (_, evidence) = source(
                &directory,
                bytes.clone(),
                &wire,
                EvidenceConfig::BASE,
                target,
            );
            (directory, evidence)
        };
        let mut measured_work =
            CanonicalKernelIrWorkBudgetV1::new(MIXED_WORKER_FINALIZATION_WORK_LIMIT_V53);
        let mut budget = Budget::new(
            &mut measured_work,
            MIXED_WORKER_FINALIZATION_STORAGE_LIMIT_V53,
        );
        budget.reserve_storage(19).unwrap();
        let (_directory, evidence) = make();
        let owner =
            finalize_protected_worker_nominal_hsaco_on_budget_v53(evidence, &mut budget).unwrap();
        let measured = (budget.work(), budget.peak_storage());
        assert_eq!(budget.storage(), 19);
        assert!(measured.0 > 1 && measured.1 > MIXED_SCRATCH + 19);
        assert_eq!(owner.raw().source_evidence().output_bytes(), bytes);
        assert_ne!(owner.finalized().as_bytes(), bytes);
        assert_eq!(
            derive_unfinalized_nominal_hsaco_v53(
                owner.finalized().as_bytes(),
                MIXED_SCRATCH,
                &mut free
            )
            .unwrap(),
            bytes
        );
        assert!(!owner.grants_proof_authority() && !owner.grants_load_authority());
        drop(owner);
        for mode in 0..3 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(measured.0 - usize::from(mode == 1));
            let mut budget = Budget::new(&mut work, measured.1 - usize::from(mode == 2));
            budget.reserve_storage(19).unwrap();
            let (_directory, evidence) = make();
            let result =
                finalize_protected_worker_nominal_hsaco_on_budget_v53(evidence, &mut budget);
            assert_eq!(budget.storage(), 19);
            if mode == 0 {
                let owner = result.unwrap();
                assert_eq!(owner.raw().source_evidence().output_bytes(), bytes);
                assert_eq!((budget.work(), budget.peak_storage()), measured);
            } else {
                let first = resource(&result.unwrap_err());
                match (mode, first) {
                    (1, Resource::Work(error)) => {
                        assert_eq!(
                            (error.actual(), error.limit()),
                            (measured.0, measured.0 - 1)
                        );
                        assert_eq!(budget.failed_work(), Some(measured.0));
                    }
                    (2, Resource::Storage(error)) => {
                        assert_eq!(
                            (error.actual(), error.limit()),
                            (measured.1, measured.1 - 1)
                        );
                        assert_eq!(budget.failed_storage(), Some(measured.1));
                    }
                    _ => panic!("wrong resource denial: {first:?}"),
                }
                let before = (budget.work(), budget.storage(), budget.peak_storage());
                let (_directory, evidence) = make();
                let retry =
                    finalize_protected_worker_nominal_hsaco_on_budget_v53(evidence, &mut budget)
                        .unwrap_err();
                assert!(matches!(
                    retry,
                    MixedWorkerFinalizationBudgetErrorV53::Resource(_)
                ));
                assert_eq!(resource(&retry), first);
                assert_eq!(
                    (budget.work(), budget.storage(), budget.peak_storage()),
                    before
                );
            }
        }
    }
}

#[test]
fn mixed_v53_raw_reconstruction_is_paid_immutable_and_cumulative() {
    use fe2o3_hsaco_finalize::{
        MIXED_WORKER_FINALIZATION_STORAGE_LIMIT_V53, MIXED_WORKER_FINALIZATION_WORK_LIMIT_V53,
        MixedWorkerFinalizationBudgetErrorV53 as Error,
        derive_unfinalized_nominal_hsaco_on_budget_v53,
        finalize_protected_worker_nominal_hsaco_on_budget_v53,
    };
    for target in [TARGET, "gfx950:xnack-"] {
        let wire = mixed_wire(target, 2);
        let raw = mixed_artifact(&wire, target);
        let make = || {
            let directory = TestDirectory::new();
            let (_, source) = source(&directory, raw.clone(), &wire, EvidenceConfig::BASE, target);
            (directory, source)
        };
        let (_directory, evidence) = make();
        let owner = finalize_protected_worker_nominal_hsaco_v53(evidence, MIXED_SCRATCH, &mut free)
            .unwrap();
        let finalized = owner.finalized().as_bytes();
        let before = ContentIdentityV1::calculate(finalized);
        let run = |limit, storage| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = Budget::new(&mut work, storage);
            budget.reserve_storage(19).unwrap();
            budget.charge_work(7).unwrap();
            let result = derive_unfinalized_nominal_hsaco_on_budget_v53(finalized, &mut budget);
            assert_eq!(budget.storage(), 19);
            assert!(before.matches(finalized));
            (
                result,
                budget.work(),
                budget.peak_storage(),
                budget.failed_work(),
                budget.failed_storage(),
            )
        };
        let measured = run(
            MIXED_WORKER_FINALIZATION_WORK_LIMIT_V53,
            MIXED_WORKER_FINALIZATION_STORAGE_LIMIT_V53,
        );
        assert_eq!(measured.0.unwrap(), raw);
        let exact = run(measured.1, measured.2);
        assert_eq!(exact.0.unwrap(), raw);
        assert_eq!((exact.1, exact.2), (measured.1, measured.2));
        let short_work = run(measured.1 - 1, measured.2);
        assert!(short_work.0.is_err());
        assert_eq!(short_work.3, Some(measured.1));
        let short_storage = run(measured.1, measured.2 - 1);
        assert!(
            matches!(short_storage.0, Err(Error::Resource(Resource::Storage(e)))
            if e.actual() == measured.2 && e.limit() == measured.2 - 1)
        );
        assert_eq!(short_storage.4, Some(measured.2));

        let mut work = CanonicalKernelIrWorkBudgetV1::new(MIXED_WORKER_FINALIZATION_WORK_LIMIT_V53);
        let mut budget = Budget::new(&mut work, MIXED_WORKER_FINALIZATION_STORAGE_LIMIT_V53);
        budget.reserve_storage(19).unwrap();
        let restored =
            derive_unfinalized_nominal_hsaco_on_budget_v53(finalized, &mut budget).unwrap();
        let after_raw = budget.work();
        assert_eq!(restored, raw);
        drop(restored);
        let (_directory, evidence) = make();
        let recovered =
            finalize_protected_worker_nominal_hsaco_on_budget_v53(evidence, &mut budget).unwrap();
        assert!(budget.work() > after_raw);
        assert_eq!(budget.storage(), 19);
        assert_eq!(recovered.finalized().as_bytes(), finalized);
    }
}

fn mixed_wire(target: &str, source_seed: u8) -> Vec<u8> {
    let (_, nominal) = wires(target, "v53-worker");
    let table = decode_device_descriptor_table_v3(&nominal, &mut free).unwrap();
    let kernel = table.kernel(0, &mut free).unwrap();
    let input = MixedContractInputV26 {
        subjects: MixedContractSubjectsV26 {
            kernel_id: *kernel.kernel_id().as_bytes(),
            source_semantic_identity: [source_seed; 32],
            original_graph_identity: [3; 32],
            output_graph_identity: [4; 32],
            descriptor_identity: mixed_descriptor_subject_v26(&table, &mut free).unwrap(),
            original_root: 0,
            output_function: 0,
            source_rank: kernel.launch().rank(),
            index_width: 64,
            exact_grid: [256, 1, 1],
            source_argument_count: kernel.argument_count() as u32,
            generated_field_count: kernel.component_count() as u32,
            explicit_argument_bytes: kernel.abi_layout().explicit_argument_size(),
            kernarg_alignment: kernel.abi_layout().kernarg_segment_alignment(),
        },
        arguments: &[],
        occurrences: &[],
    };
    let mut contract = vec![0; encoded_mixed_contract_v26_len(&input, &mut free).unwrap()];
    encode_mixed_contract_v26(&input, &mut contract, &mut free).unwrap();
    let contracts = [decode_mixed_contract_v26(&contract, &mut free).unwrap()];
    let mut output =
        vec![0; encoded_mixed_descriptor_v53_len(&nominal, &contracts, &mut free).unwrap()];
    encode_mixed_descriptor_v53(&nominal, &contracts, &mut output, &mut free).unwrap();
    output
}

fn mixed_artifact(wire: &[u8], target: &str) -> Vec<u8> {
    let mut bytes =
        hsaco_fixture::slice_fixture_with_descriptor_table_workgroup_target(wire, 256, target)
            .bytes;
    let sections = u64::from_le_bytes(bytes[40..48].try_into().unwrap()) as usize;
    let strings_index = u16::from_le_bytes(bytes[62..64].try_into().unwrap()) as usize;
    let header = sections + strings_index * 64;
    let start = u64::from_le_bytes(bytes[header + 24..header + 32].try_into().unwrap()) as usize;
    let size = u64::from_le_bytes(bytes[header + 32..header + 40].try_into().unwrap()) as usize;
    let mut strings = bytes[start..start + size].to_vec();
    let name = strings.len();
    strings.extend_from_slice(b".fe2o3.kd.v53\0");
    let offset = bytes.len() as u64;
    bytes.extend_from_slice(&strings);
    bytes[header + 24..header + 32].copy_from_slice(&offset.to_le_bytes());
    bytes[header + 32..header + 40].copy_from_slice(&(strings.len() as u64).to_le_bytes());
    let descriptor = sections + 6 * 64;
    bytes[descriptor..descriptor + 4].copy_from_slice(&(name as u32).to_le_bytes());
    bytes
}

#[test]
fn mixed_v53_worker_finalization_preserves_exact_contract_and_strict_custody() {
    for target in [TARGET, "gfx950:xnack-"] {
        let wire = mixed_wire(target, 2);
        let bytes = mixed_artifact(&wire, target);
        let directory = TestDirectory::new();
        let (attempt, evidence) = source(
            &directory,
            bytes.clone(),
            &wire,
            EvidenceConfig::BASE,
            target,
        );
        let before = evidence.identity();
        let owner = finalize_protected_worker_nominal_hsaco_v53(evidence, MIXED_SCRATCH, &mut free)
            .unwrap();
        assert_eq!(owner.attempt(), attempt);
        assert_eq!(owner.raw().source_evidence_identity(), before);
        assert_eq!(
            owner
                .raw()
                .outer_handoff()
                .capsule()
                .receipts()
                .abi()
                .canonical_preimage(),
            wire
        );
        assert!(
            owner
                .output_identity()
                .matches(owner.finalized().as_bytes())
        );
        assert!(
            owner
                .descriptor_identity()
                .matches(owner.finalized().descriptor_bytes())
        );
        let checked = inspect_finalized_nominal_hsaco_v53(
            owner.finalized().as_bytes(),
            MIXED_SCRATCH,
            &mut free,
        )
        .unwrap();
        let original = decode_mixed_descriptor_v53(&wire, &mut free).unwrap();
        assert_eq!(
            checked
                .descriptor_table()
                .contract(0, &mut free)
                .unwrap()
                .canonical_bytes(),
            original.contract(0, &mut free).unwrap().canonical_bytes()
        );
        assert_eq!(
            derive_unfinalized_nominal_hsaco_v53(
                owner.finalized().as_bytes(),
                MIXED_SCRATCH,
                &mut free
            )
            .unwrap(),
            bytes
        );
        assert!(!owner.grants_proof_authority() && !owner.grants_launch_authority());
        let mut changed = owner.finalized().as_bytes().to_vec();
        let last = changed.len() - 1;
        changed[last] ^= 1;
        assert!(inspect_finalized_nominal_hsaco_v53(&changed, MIXED_SCRATCH, &mut free).is_err());
    }
}

#[test]
fn mixed_v53_worker_rejects_foreign_complete_contract_and_legacy_schema() {
    let wire = mixed_wire(TARGET, 2);
    let foreign = mixed_wire(TARGET, 9);
    decode_mixed_descriptor_v53(&foreign, &mut free).unwrap();
    let directory = TestDirectory::new();
    let (_, evidence) = source(
        &directory,
        mixed_artifact(&wire, TARGET),
        &foreign,
        EvidenceConfig::BASE,
        TARGET,
    );
    assert!(matches!(
        finalize_protected_worker_nominal_hsaco_v53(evidence, MIXED_SCRATCH, &mut free),
        Err(NominalWorkerFinalizationErrorV53::Finalization(
            NominalFinalizationErrorV53::DescriptorSourceMismatch
        ))
    ));
    let directory = TestDirectory::new();
    let (old, _) = wires(TARGET, "old");
    let (_, evidence) = source(
        &directory,
        artifact(&old, 5, TARGET),
        &old,
        EvidenceConfig::BASE,
        TARGET,
    );
    assert!(
        finalize_protected_worker_nominal_hsaco_v53(evidence, MIXED_SCRATCH, &mut free).is_err()
    );
}

#[test]
fn mixed_v53_finalizer_requires_exact_scratch_and_work() {
    let wire = mixed_wire(TARGET, 2);
    let make = || {
        let directory = TestDirectory::new();
        let (_, evidence) = source(
            &directory,
            mixed_artifact(&wire, TARGET),
            &wire,
            EvidenceConfig::BASE,
            TARGET,
        );
        (directory, evidence)
    };
    let (_directory, evidence) = make();
    let mut work = 0usize;
    finalize_protected_worker_nominal_hsaco_v53(evidence, MIXED_SCRATCH, &mut |n| {
        work += n;
        Ok::<_, &'static str>(())
    })
    .unwrap();
    let (_directory, evidence) = make();
    let mut left = work;
    finalize_protected_worker_nominal_hsaco_v53(evidence, MIXED_SCRATCH, &mut |n| {
        left = left.checked_sub(n).ok_or("work")?;
        Ok::<(), &'static str>(())
    })
    .unwrap();
    assert_eq!(left, 0);
    let (_directory, evidence) = make();
    let mut left = work - 1;
    assert!(
        finalize_protected_worker_nominal_hsaco_v53(evidence, MIXED_SCRATCH, &mut |n| {
            left = left.checked_sub(n).ok_or("work")?;
            Ok::<(), &'static str>(())
        })
        .is_err()
    );
    let (_directory, evidence) = make();
    assert!(
        finalize_protected_worker_nominal_hsaco_v53(evidence, MIXED_SCRATCH - 1, &mut free)
            .is_err()
    );
}

#[test]
fn mixed_v53_compact_replay_binds_exact_transaction_and_full_contract() {
    let wire = mixed_wire(TARGET, 2);
    let foreign_wire = mixed_wire(TARGET, 9);
    let mut cases = Vec::new();
    for (index, wire) in [&wire, &wire, &foreign_wire].into_iter().enumerate() {
        let directory = TestDirectory::new();
        let (attempt, evidence) = source(
            &directory,
            mixed_artifact(wire, TARGET),
            wire,
            EvidenceConfig {
                attempt_seed: 0x71 + index as u8,
                ..EvidenceConfig::BASE
            },
            TARGET,
        );
        let finalized =
            finalize_protected_worker_nominal_hsaco_v53(evidence, MIXED_SCRATCH, &mut free)
                .unwrap();
        let identity = finalized.identity();
        let parts = prepare_mixed_worker_compact_finalizer_replay_v53(finalized)
            .unwrap()
            .into_parts();
        let replay = revalidate_protected_worker_v3_finalizer_derivation_v1(
            attempt,
            &parts.outer_handoff,
            parts.external_provider_payloads.iter().map(Vec::as_slice),
            &parts.transcript,
            &parts.finalized_hsaco,
        )
        .unwrap();
        assert_eq!(replay.descriptor_schema_version(), 53);
        assert_eq!(replay.finalization_identity(), identity);
        assert!(!replay.proves_llvm_to_machine_semantic_refinement());
        assert!(!replay.grants_compiler_authority() && !replay.grants_publication_authority());
        assert!(!replay.grants_load_authority() && !replay.grants_launch_authority());
        cases.push((directory, attempt, parts));
    }
    assert_eq!(cases[0].2.finalized_hsaco, cases[1].2.finalized_hsaco);
    assert_ne!(cases[0].2.finalized_hsaco, cases[2].2.finalized_hsaco);
    let (_, attempt, first) = &cases[0];
    for (_, other_attempt, other) in &cases[1..] {
        assert!(
            revalidate_protected_worker_v3_finalizer_derivation_v1(
                *other_attempt,
                &first.outer_handoff,
                first.external_provider_payloads.iter().map(Vec::as_slice),
                &first.transcript,
                &first.finalized_hsaco,
            )
            .is_err()
        );
        assert!(
            revalidate_protected_worker_v3_finalizer_derivation_v1(
                *attempt,
                &first.outer_handoff,
                first.external_provider_payloads.iter().map(Vec::as_slice),
                &other.transcript,
                &first.finalized_hsaco,
            )
            .is_err()
        );
    }
    assert!(
        revalidate_protected_worker_v3_finalizer_derivation_v1(
            *attempt,
            &first.outer_handoff,
            first.external_provider_payloads.iter().map(Vec::as_slice),
            &first.transcript,
            &cases[2].2.finalized_hsaco,
        )
        .is_err()
    );
}

#[test]
fn mixed_v53_durable_recovery_preserves_schema_and_remains_inert() {
    let directory = TestDirectory::new();
    let wire = mixed_wire(TARGET, 2);
    let (attempt, evidence) = source(
        &directory,
        mixed_artifact(&wire, TARGET),
        &wire,
        EvidenceConfig::BASE,
        TARGET,
    );
    let finalized =
        finalize_protected_worker_nominal_hsaco_v53(evidence, MIXED_SCRATCH, &mut free).unwrap();
    let identity = finalized.identity();
    let exact = finalized.finalized().as_bytes().to_vec();
    let prepared = prepare_mixed_worker_publication_v53(&producer(), finalized).unwrap();
    assert!(
        !prepared.grants_publication_authority()
            && !prepared.grants_load_authority()
            && !prepared.grants_launch_authority()
    );
    let intent = prepared.publication_intent();
    let persisted =
        persist_prepared_mixed_worker_publication_v53(&directory.0, &producer(), prepared).unwrap();
    assert_eq!(
        persisted.outcome(),
        WorkerV3PublicationIntentOutcomeV1::Persisted
    );
    let record = persisted.storage_record();
    drop(persisted);
    assert!(matches!(
        recover_nominal_worker_publication_v5(&directory.0, &producer(), attempt),
        Err(WorkerV3HsacoPublicationErrorV1::DescriptorSchemaMismatch)
    ));
    for _ in 0..2 {
        let recovered =
            recover_mixed_worker_publication_v53(&directory.0, &producer(), attempt).unwrap();
        assert_eq!(
            recovered.outcome(),
            WorkerV3PublicationIntentOutcomeV1::Recovered
        );
        assert_eq!(recovered.storage_record(), record);
        assert_eq!(recovered.publication_intent(), intent);
        assert_eq!(recovered.exact_finalized_hsaco(), exact);
        assert_eq!(recovered.finalized_evidence().identity(), identity);
        assert!(
            !recovered.grants_publication_authority()
                && !recovered.grants_load_authority()
                && !recovered.grants_launch_authority()
        );
        let descriptor = decode_mixed_descriptor_v53(
            recovered
                .finalized_evidence()
                .finalized()
                .descriptor_bytes(),
            &mut free,
        )
        .unwrap();
        assert_eq!(
            descriptor
                .contract(0, &mut free)
                .unwrap()
                .subjects()
                .source_semantic_identity,
            [2; 32]
        );
    }
}
