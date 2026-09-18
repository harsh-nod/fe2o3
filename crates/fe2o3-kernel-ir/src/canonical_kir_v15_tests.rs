use super::*;
use crate::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1, DiagnosticCode,
    ExecutionOperationV15 as Execution, ExecutionRoleV15 as Role, Function, Kernel, LaunchDomain,
    LaunchExtent, Operation, OperationKind, ScalarType, Signature, Terminator, Type, ValueDef,
    ValueId,
};

const WORK_PREFIX: usize = 11;
const STORAGE_PREFIX: usize = 7;
const LIMIT: usize = 10_000_000;

struct Probe<T> {
    result: Result<T, AdmissionError>,
    work: usize,
    peak: usize,
    denied_work: Option<usize>,
    denied_storage: Option<usize>,
}

fn probe<T>(
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
    seed_denials: bool,
    run: impl FnOnce(&mut Budget<'_>) -> Result<T, AdmissionError>,
) -> Probe<T> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    work.charge_work(WORK_PREFIX).unwrap();
    let (result, peak, denied_storage) = {
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        if seed_denials {
            assert!(budget.charge_work(usize::MAX).is_err());
            assert!(budget.reserve_storage(usize::MAX).is_err());
        }
        let result = run(&mut budget);
        assert_eq!(budget.storage(), floor);
        (result, budget.peak_storage(), budget.failed_storage())
    };
    Probe {
        result,
        work: work.work(),
        peak,
        denied_work: work.failed_work(),
        denied_storage,
    }
}

fn admit_source(
    source: &Module,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
    seed_denials: bool,
) -> Probe<(
    VerifiedCanonicalKernelIrModuleV15,
    CanonicalKernelIrReplayStorageV15,
)> {
    probe(floor, work_limit, storage_limit, seed_denials, |budget| {
        VerifiedCanonicalKernelIrModuleV15::from_module_ref_with_verification_budget_v15(
            source, budget,
        )
    })
}

fn execution(result: Option<(u32, Role)>, operation: Execution) -> Operation {
    Operation::new(
        result
            .into_iter()
            .map(|(id, role)| ValueDef::new(ValueId(id), Type::Execution(role)))
            .collect(),
        OperationKind::Execution(operation),
    )
}

fn close(workgroup: u32, discarded: &[u32]) -> Operation {
    execution(
        None,
        Execution::ScopeEnd {
            workgroup: ValueId(workgroup),
            discarded: discarded.iter().copied().map(ValueId).collect(),
        },
    )
}

fn scope_module() -> Module {
    let mut module = Module::new("scoped-owner");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![BasicBlock {
            id: BlockId(7),
            parameters: vec![],
            operations: vec![
                execution(Some((10, Role::Context)), Execution::ContextIssue),
                execution(
                    Some((11, Role::Workgroup)),
                    Execution::WorkgroupDerive {
                        context: ValueId(10),
                    },
                ),
                close(11, &[]),
            ],
            terminator: Some(Terminator::Return { values: vec![] }),
        }],
    ));
    module.kernels.push(Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    module
}

fn operations(module: &mut Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}

fn tile_module(discard: bool) -> Module {
    let mut module = scope_module();
    module.functions[0].signature.parameters = vec![
        Type::slice(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
        Type::INDEX,
    ];
    module.functions[0].body.as_mut().unwrap().parameters = vec![ValueId(0), ValueId(1)];
    let ops = operations(&mut module);
    ops.pop();
    ops.push(execution(
        Some((
            12,
            Role::MaskedTileU32 {
                lanes: 64,
                elements: 1,
            },
        )),
        Execution::MaskedTileLoadU32 {
            workgroup: ValueId(11),
            input: ValueId(0),
            base: ValueId(1),
            lanes: 64,
            elements: 1,
        },
    ));
    if !discard {
        ops.push(execution(
            Some((
                13,
                Role::LaneFragmentU32 {
                    lanes: 64,
                    elements: 1,
                },
            )),
            Execution::TileIntoFragmentU32 {
                tile: ValueId(12),
                lanes: 64,
                elements: 1,
            },
        ));
        ops.push(Operation::new(
            vec![
                ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32)),
                ValueDef::new(ValueId(21), Type::BOOL),
            ],
            OperationKind::Execution(Execution::FragmentIntoPartsU32 {
                fragment: ValueId(13),
                lanes: 64,
                elements: 1,
            }),
        ));
    }
    ops.push(close(11, if discard { &[12] } else { &[] }));
    module
}

#[test]
fn exact_v15_owner_accepts_lifecycles_and_has_deterministic_domain_bound_bytes() {
    let mut context_only = scope_module();
    operations(&mut context_only).truncate(1);
    let mut repeated = scope_module();
    operations(&mut repeated).extend([
        execution(
            Some((14, Role::Workgroup)),
            Execution::WorkgroupDerive {
                context: ValueId(10),
            },
        ),
        close(14, &[]),
    ]);
    for source in [
        context_only,
        scope_module(),
        repeated,
        tile_module(false),
        tile_module(true),
    ] {
        let (owner, receipt) = admit_source(&source, STORAGE_PREFIX, LIMIT, LIMIT, false)
            .result
            .unwrap();
        let (again, second_receipt) = admit_source(&source, STORAGE_PREFIX, LIMIT, LIMIT, false)
            .result
            .unwrap();
        assert_eq!(owner.module(), &source);
        assert_eq!(
            owner.canonical_bytes(),
            crate::encode_module_v15(&source).unwrap()
        );
        assert_eq!(&owner.canonical_bytes()[8..10], &15_u16.to_le_bytes());
        assert_eq!(owner.identity(), again.identity());
        assert_eq!(owner.canonical_bytes(), again.canonical_bytes());
        assert_eq!(receipt, second_receipt);
        assert_eq!(
            owner.identity().canonical_length(),
            owner.canonical_bytes().len() as u64
        );
        let domain = b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V15\0";
        let mut hash = Sha256::new();
        hash.update((domain.len() as u32).to_le_bytes());
        hash.update(domain);
        hash.update((owner.canonical_bytes().len() as u64).to_le_bytes());
        hash.update(owner.canonical_bytes());
        let expected: [u8; 32] = hash.finalize().into();
        assert_eq!(owner.identity().digest(), &expected);
        assert_ne!(
            owner.identity().digest(),
            &<[u8; 32]>::from(Sha256::digest(owner.canonical_bytes()))
        );
        assert!(crate::decode_module_v12(owner.canonical_bytes()).is_err());
        assert_eq!(
            crate::decode_module_v15(owner.canonical_bytes()).unwrap(),
            source
        );
    }
}

#[test]
fn exact_v15_owner_rejects_invalid_closes_live_calls_and_context_reissuance() {
    let mut missing = scope_module();
    operations(&mut missing).pop();
    let mut duplicate = scope_module();
    operations(&mut duplicate).push(close(11, &[]));
    let mut foreign = scope_module();
    operations(&mut foreign).extend([
        execution(
            Some((14, Role::Workgroup)),
            Execution::WorkgroupDerive {
                context: ValueId(10),
            },
        ),
        close(11, &[]),
    ]);
    let mut call = scope_module();
    call.functions.push(Function::internal_helper(
        "helper",
        Signature::new(vec![], vec![]),
        vec![],
        vec![BasicBlock {
            id: BlockId(0),
            parameters: vec![],
            operations: vec![],
            terminator: Some(Terminator::Return { values: vec![] }),
        }],
    ));
    operations(&mut call).insert(
        2,
        Operation::new(
            vec![],
            OperationKind::Call {
                callee: "helper".into(),
                arguments: vec![],
            },
        ),
    );
    let mut closed_call = call.clone();
    let operation = operations(&mut closed_call).remove(2);
    operations(&mut closed_call).push(operation);
    admit_source(&closed_call, STORAGE_PREFIX, LIMIT, LIMIT, false)
        .result
        .unwrap();
    let mut cycle = scope_module();
    cycle.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(7),
        arguments: vec![],
    });
    let mut missing_discard = tile_module(true);
    *operations(&mut missing_discard).last_mut().unwrap() = close(11, &[]);
    for source in [missing, duplicate, foreign, call, cycle, missing_discard] {
        let expected = crate::verify_module(&source).unwrap_err();
        assert!(
            expected
                .diagnostics()
                .iter()
                .any(|error| error.code == DiagnosticCode::InvalidSemanticOperation)
        );
        let error = admit_source(&source, STORAGE_PREFIX, LIMIT, LIMIT, false)
            .result
            .unwrap_err();
        assert!(matches!(error, AdmissionError::Verification(actual) if actual == expected));
    }
}

#[test]
fn full_semantic_and_wire_failures_are_typed_and_restore_the_incoming_floor() {
    let mut semantic = scope_module();
    operations(&mut semantic).truncate(1);
    operations(&mut semantic)[0].results[0].ty = Type::INDEX;
    let expected = crate::verify_module(&semantic).unwrap_err();
    assert!(
        expected
            .diagnostics()
            .iter()
            .any(|error| error.code == DiagnosticCode::TypeMismatch)
    );
    assert!(matches!(
        admit_source(&semantic, STORAGE_PREFIX, LIMIT, LIMIT, false).result,
        Err(AdmissionError::Verification(actual)) if actual == expected
    ));

    let mut unencodable = scope_module();
    unencodable.functions[0].role = crate::FunctionRole::InternalHelper;
    assert!(matches!(
        admit_source(&unencodable, STORAGE_PREFIX, LIMIT, LIMIT, false).result,
        Err(AdmissionError::Encode(
            KernelIrEncodeError::NonCanonical { .. }
        ))
    ));
}

#[test]
fn exact_v15_decoder_rejects_older_and_unallocated_versions() {
    let canonical = crate::encode_module_v15(&Module::new("m")).unwrap();
    for version in [1_u16, 12, 13, 14, 16] {
        let mut bytes = canonical.clone();
        bytes[8..10].copy_from_slice(&version.to_le_bytes());
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(STORAGE_PREFIX).unwrap();
        assert!(matches!(
            crate::wire::decode_module_v15_with_allocation_budget_v1(&bytes, &mut budget),
            Err(KernelIrDecodeError::UnknownVersion(found)) if found == version
        ));
        assert_eq!(budget.storage(), STORAGE_PREFIX);
    }
}

#[test]
fn admission_and_candidate_copy_obey_exact_measured_and_one_short_budgets() {
    // The caller's input reservation coexists with every admission temporary.
    let input_floor = STORAGE_PREFIX + 64 * 1024;
    for source in [scope_module(), tile_module(true)] {
        let measured = admit_source(&source, input_floor, LIMIT, LIMIT, false);
        let work = measured.work;
        let peak = measured.peak;
        let (owner, retained) = measured.result.unwrap();
        assert!(peak >= input_floor + retained.retained_storage());
        for (work_limit, storage_limit, success) in [
            (work, peak, true),
            (work - 1, peak, false),
            (work, peak - 1, false),
        ] {
            let result = admit_source(&source, input_floor, work_limit, storage_limit, false);
            assert_eq!(result.result.is_ok(), success);
            if success {
                assert_eq!(result.work, work);
                assert_eq!(result.peak, peak);
                assert_eq!(result.result.unwrap().1, retained);
            } else if work_limit < work {
                assert!(matches!(
                    result.result,
                    Err(AdmissionError::Resource(ResourceError::Work(_)))
                ));
                assert!(
                    result
                        .denied_work
                        .is_some_and(|attempt| attempt > work_limit)
                );
                assert!(result.denied_storage.is_none());
            } else {
                assert!(matches!(
                    result.result,
                    Err(AdmissionError::Resource(ResourceError::Storage(_)))
                        | Err(AdmissionError::Decode(KernelIrDecodeError::Resource(
                            ResourceError::Storage(_)
                        )))
                ));
                assert_eq!(result.denied_storage, Some(peak));
                assert!(result.denied_work.is_none());
            }
        }
        let copy_floor = input_floor + retained.retained_storage();
        let measured = probe(copy_floor, LIMIT, LIMIT, false, |budget| {
            owner.copy_module_for_transformation_v15(budget)
        });
        let copy_work = measured.work;
        let copy_peak = measured.peak;
        let (candidate, copied) = measured.result.unwrap();
        assert_eq!(&candidate, owner.module());
        assert_eq!(
            retained.retained_storage(),
            copied.retained_storage() + std::mem::size_of::<VerifiedCanonicalKernelIrModuleV15>()
                - std::mem::size_of::<Module>()
                + owner.canonical_bytes().len()
        );
        drop(candidate);
        for (work_limit, storage_limit, success) in [
            (copy_work, copy_peak, true),
            (copy_work - 1, copy_peak, false),
            (copy_work, copy_peak - 1, false),
        ] {
            let result = probe(copy_floor, work_limit, storage_limit, false, |budget| {
                owner.copy_module_for_transformation_v15(budget)
            });
            assert_eq!(result.result.is_ok(), success);
            if success {
                assert_eq!(result.result.unwrap().1, copied);
            } else if work_limit < copy_work {
                assert!(matches!(
                    result.result,
                    Err(AdmissionError::Resource(ResourceError::Work(_)))
                ));
                assert!(result.denied_work.is_some());
            } else {
                assert!(matches!(
                    result.result,
                    Err(AdmissionError::Resource(ResourceError::Storage(_)))
                        | Err(AdmissionError::Decode(KernelIrDecodeError::Resource(
                            ResourceError::Storage(_)
                        )))
                ));
                assert_eq!(result.denied_storage, Some(copy_peak));
            }
        }
    }
}

#[test]
fn candidate_is_independent_and_mutation_requires_fresh_admission() {
    let mut source = tile_module(true);
    let snapshot = source.clone();
    let (owner, retained) = admit_source(&source, STORAGE_PREFIX, LIMIT, LIMIT, false)
        .result
        .unwrap();
    let identity = *owner.identity();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let floor = STORAGE_PREFIX + retained.retained_storage();
    budget.reserve_storage(floor).unwrap();
    let (mut candidate, copied) = owner
        .copy_module_for_transformation_v15(&mut budget)
        .unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(copied.retained_storage()).unwrap();
    assert_ne!(
        candidate.functions.as_ptr(),
        owner.module().functions.as_ptr()
    );
    assert_ne!(
        candidate.id.as_str().as_ptr(),
        owner.module().id.as_str().as_ptr()
    );
    let removed = operations(&mut candidate).pop().unwrap();
    assert!(matches!(
        VerifiedCanonicalKernelIrModuleV15::from_module_ref_with_verification_budget_v15(
            &candidate,
            &mut budget
        ),
        Err(AdmissionError::Verification(_))
    ));
    assert_eq!(budget.storage(), floor + copied.retained_storage());
    operations(&mut candidate).push(removed);
    // Change only existing scalar IDs; no unaccounted mutation allocation.
    operations(&mut candidate)[0].results[0].id = ValueId(100);
    operations(&mut candidate)[1].kind = OperationKind::Execution(Execution::WorkgroupDerive {
        context: ValueId(100),
    });
    let (readmitted, receipt) =
        VerifiedCanonicalKernelIrModuleV15::from_module_ref_with_verification_budget_v15(
            &candidate,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert_ne!(readmitted.identity(), &identity);
    assert_eq!(readmitted.module(), &candidate);
    source.functions.clear();
    drop(source);
    assert_eq!(owner.module(), &snapshot);
    assert_eq!(owner.identity(), &identity);
    drop(owner);
    budget.release_storage(retained.retained_storage()).unwrap();
    assert_ne!(candidate, snapshot);
    assert_eq!(readmitted.module(), &candidate);
    drop(candidate);
    budget.release_storage(copied.retained_storage()).unwrap();
    assert_ne!(readmitted.module(), &snapshot);
    drop(readmitted);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), STORAGE_PREFIX);
}

#[test]
fn empty_module_independent_counts_and_v12_regression() {
    let source = Module::new("m");
    // Count10 + encode51 + decode101 + verifier5 + equality37 + hash88.
    let retained = std::mem::size_of::<VerifiedCanonicalKernelIrModuleV15>() + 37 + 1;
    let result = admit_source(
        &source,
        STORAGE_PREFIX,
        WORK_PREFIX + 292,
        STORAGE_PREFIX + retained,
        false,
    );
    assert_eq!(result.work, WORK_PREFIX + 292);
    assert_eq!(result.peak, STORAGE_PREFIX + retained);
    let (owner, receipt) = result.result.unwrap();
    assert_eq!(receipt.retained_storage(), retained);
    let floor = STORAGE_PREFIX + retained;
    let copied = std::mem::size_of::<Module>() + 1;
    let result = probe(floor, WORK_PREFIX + 138, floor + copied, false, |budget| {
        owner.copy_module_for_transformation_v15(budget)
    });
    assert_eq!(result.work, WORK_PREFIX + 138);
    assert_eq!(result.result.unwrap().1.retained_storage(), copied);
    let result = probe(STORAGE_PREFIX, LIMIT, LIMIT, false, |budget| {
        Ok(crate::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &source, budget,
        )
        .unwrap())
    });
    assert_eq!(result.work, WORK_PREFIX + 294);
    let (v12, _) = result.result.unwrap();
    assert_eq!(
        &v12.canonical().canonical_bytes()[8..10],
        &12_u16.to_le_bytes()
    );
    assert_ne!(
        owner.identity().digest(),
        v12.canonical().identity().digest()
    );
}

#[test]
fn every_empty_admission_work_prefix_restores_floor_and_preserves_first_denial() {
    for source in [Module::new("m"), Module::new("")] {
        let measured = admit_source(&source, STORAGE_PREFIX, LIMIT, LIMIT, false);
        let total = measured.work;
        for work_limit in WORK_PREFIX..=total {
            let result = admit_source(&source, STORAGE_PREFIX, work_limit, LIMIT, true);
            assert_eq!(result.denied_work, Some(usize::MAX));
            assert_eq!(result.denied_storage, Some(usize::MAX));
            assert!(result.work <= work_limit);
            if work_limit < total {
                assert!(result.result.is_err());
                assert!(!matches!(
                    result.result,
                    Err(AdmissionError::Verification(_))
                ));
            } else if source.id.as_str().is_empty() {
                assert!(matches!(
                    result.result,
                    Err(AdmissionError::Verification(_))
                ));
            } else {
                assert!(result.result.is_ok());
            }
        }
    }
}
