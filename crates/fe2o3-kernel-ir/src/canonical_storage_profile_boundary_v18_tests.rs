use super::tests::admit;
use crate::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CastKind, DiagnosticCode, Function, Module,
    Operation, OperationKind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
    verify_module_ref,
};

fn cast_module(kind: CastKind, from: Type, to: Type) -> Module {
    let mut module = Module::new("storage-profile-cast");
    let mut block = BasicBlock::new(BlockId(17));
    block.operations.push(Operation::new(
        vec![ValueDef::new(ValueId(1), to.clone())],
        OperationKind::Cast {
            kind,
            value: ValueId(0),
            to: to.clone(),
        },
    ));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(1)],
    });
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![from], vec![to]),
        vec![ValueId(0)],
        vec![block],
    ));
    module
}

fn legacy_owners_refuse(module: &Module, feature: &'static str) {
    macro_rules! check {
        ($version:literal, $owner:ident, $method:ident, $error:ident) => {{
            let mut work = crate::CanonicalKernelIrWorkBudgetV1::new(10_000_000);
            let mut budget = crate::CanonicalKernelIrVerificationResourceBudgetV1::new(
                &mut work,
                10_000_000,
            );
            budget.reserve_storage(23).unwrap();
            let result = crate::$owner::$method(module, &mut budget);
            assert!(matches!(result, Err(crate::$error::Encode(
                crate::KernelIrEncodeError::UnsupportedInVersion {
                    version: $version,
                    feature: actual,
                }
            )) if actual == feature), "version {}: {result:?}", $version);
            assert_eq!(budget.storage(), 23);
            assert_eq!(budget.failed_work(), None);
            assert_eq!(budget.failed_storage(), None);
        }};
    }
    check!(
        12,
        VerifiedCanonicalKernelIrModuleV12,
        from_module_ref_with_verification_budget_v12,
        CanonicalKernelIrReplayAdmissionErrorV12
    );
    check!(
        15,
        VerifiedCanonicalKernelIrModuleV15,
        from_module_ref_with_verification_budget_v15,
        CanonicalKernelIrReplayAdmissionErrorV15
    );
    check!(
        16,
        VerifiedCanonicalKernelIrModuleV16,
        from_module_ref_with_verification_budget_v16,
        CanonicalKernelIrReplayAdmissionErrorV16
    );
    check!(
        17,
        VerifiedCanonicalKernelIrModuleV17,
        from_module_ref_with_verification_budget_v17,
        CanonicalKernelIrReplayAdmissionErrorV17
    );
    check!(
        19,
        VerifiedCanonicalKernelIrModuleV19,
        from_module_ref_with_verification_budget_v19,
        CanonicalKernelIrReplayAdmissionErrorV19
    );
    check!(
        20,
        VerifiedCanonicalKernelIrModuleV20,
        from_module_ref_with_verification_budget_v20,
        CanonicalKernelIrReplayAdmissionErrorV20
    );
    check!(
        21,
        VerifiedCanonicalKernelIrModuleV21,
        from_module_ref_with_verification_budget_v21,
        CanonicalKernelIrReplayAdmissionErrorV21
    );
    check!(
        22,
        VerifiedCanonicalKernelIrModuleV22,
        from_module_ref_with_verification_budget_v22,
        CanonicalKernelIrReplayAdmissionErrorV22
    );
}

#[test]
fn canonical_storage_tables_never_enter_legacy_canonical_owners() {
    let mut module = Module::new("legacy-table-boundary");
    module.storage_layouts.push(crate::StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: crate::StorageLayoutKindV1::Scalar(ScalarType::U32),
    });
    assert!(verify_module_ref(&module).is_err());
    assert_eq!(admit(&module).module(), &module);
    legacy_owners_refuse(&module, "module storage layouts");
}

#[test]
fn canonical_storage_generic_casts_require_the_distinct_profile_even_without_layout_rows() {
    for space in [
        AddressSpace::Global,
        AddressSpace::Constant,
        AddressSpace::Private,
        AddressSpace::Workgroup,
    ] {
        for access in [AccessMode::ReadOnly, AccessMode::ReadWrite] {
            if space == AddressSpace::Constant && access == AccessMode::ReadWrite {
                continue;
            }
            for slice in [false, true] {
                let carrier = |space| {
                    if slice {
                        Type::slice(Type::Scalar(ScalarType::U32), space, access)
                    } else {
                        Type::pointer(Type::Scalar(ScalarType::U32), space, access)
                    }
                };
                let kind = if slice {
                    CastKind::SliceToGeneric
                } else {
                    CastKind::PointerToGeneric
                };
                let module = cast_module(kind, carrier(space), carrier(AddressSpace::Generic));
                assert!(module.storage_layouts.is_empty());
                assert!(
                    verify_module_ref(&module)
                        .unwrap_err()
                        .contains(DiagnosticCode::InvalidCast)
                );
                let owner = admit(&module);
                assert_eq!(owner.module(), &module);
                assert_eq!(owner.verified_storage_module_ref_v1().module(), &module);
                legacy_owners_refuse(
                    &module,
                    if slice {
                        "slice to generic cast"
                    } else {
                        "pointer to generic cast"
                    },
                );
            }
        }
    }
}

#[test]
fn canonical_storage_existing_pointer_restriction_keeps_legacy_raw_admission() {
    let module = cast_module(
        CastKind::RestrictPointerAccess,
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Private,
            AccessMode::ReadWrite,
        ),
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Private,
            AccessMode::ReadOnly,
        ),
    );
    verify_module_ref(&module).unwrap();
    assert_eq!(admit(&module).module(), &module);
}

fn generic_cast_carrier(
    slice: bool,
    element: Type,
    space: AddressSpace,
    access: AccessMode,
) -> Type {
    if slice {
        Type::slice(element, space, access)
    } else {
        Type::pointer(element, space, access)
    }
}

fn generic_cast_module(kind: CastKind, from: Type, to: Type, storage: bool) -> Module {
    let mut module = cast_module(kind, from, to);
    if storage {
        // Same-shaped rows still have distinct identities within this module.
        for _ in 0..2 {
            module.storage_layouts.push(crate::StorageLayoutV1 {
                size: 4,
                alignment: 4,
                kind: crate::StorageLayoutKindV1::Scalar(ScalarType::U32),
            });
        }
    }
    module
}

#[test]
fn canonical_storage_generic_casts_preserve_all_concrete_spaces_rights_and_local_rows() {
    use crate::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work, VerifiedCanonicalKernelIrModuleV18 as Owner,
    };

    const FLOOR: usize = 23;
    for storage in [false, true] {
        let element = if storage {
            Type::StorageObject(crate::StorageLayoutIdV1(0))
        } else {
            Type::Scalar(ScalarType::U32)
        };
        for slice in [false, true] {
            let kind = if slice {
                CastKind::SliceToGeneric
            } else {
                CastKind::PointerToGeneric
            };
            for space in [
                AddressSpace::Global,
                AddressSpace::Private,
                AddressSpace::Workgroup,
                AddressSpace::Constant,
            ] {
                for access in [
                    AccessMode::ReadOnly,
                    AccessMode::WriteOnly,
                    AccessMode::ReadWrite,
                ] {
                    if space == AddressSpace::Constant && access != AccessMode::ReadOnly {
                        continue;
                    }
                    let module = generic_cast_module(
                        kind,
                        generic_cast_carrier(slice, element.clone(), space, access),
                        generic_cast_carrier(slice, element.clone(), AddressSpace::Generic, access),
                        storage,
                    );
                    let mut work = Work::new(10_000_000);
                    work.charge_work(11).unwrap();
                    let mut budget = Budget::new(&mut work, 10_000_000);
                    budget.reserve_storage(FLOOR).unwrap();
                    let (owner, receipt) = Owner::from_module_ref_with_verification_budget_v18(
                        &module,
                        super::tests::LIMITS,
                        &mut budget,
                    )
                    .unwrap();
                    assert_eq!(budget.storage(), FLOOR);
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    let paid_owner_floor = budget.storage();
                    let (decoded, decoded_receipt) =
                        Owner::from_canonical_bytes_with_verification_budget_v18(
                            owner.canonical_bytes(),
                            super::tests::LIMITS,
                            &mut budget,
                        )
                        .unwrap();
                    assert_eq!(budget.storage(), paid_owner_floor);
                    budget
                        .reserve_storage(decoded_receipt.retained_storage())
                        .unwrap();
                    assert_eq!(owner.module(), &module);
                    assert_eq!(decoded.module(), &module);
                    assert_eq!(owner.identity(), decoded.identity());
                    assert_eq!(owner.canonical_bytes(), decoded.canonical_bytes());
                    assert!(budget.work() > 11);
                    assert_eq!(budget.failed_work(), None);
                    assert_eq!(budget.failed_storage(), None);
                    drop(decoded);
                    budget
                        .release_storage(decoded_receipt.retained_storage())
                        .unwrap();
                    drop(owner);
                    budget.release_storage(receipt.retained_storage()).unwrap();
                    assert_eq!(budget.storage(), FLOOR);
                }
            }
        }
    }
}

#[test]
fn canonical_storage_generic_casts_reject_malformed_shapes_in_the_v18_owner() {
    use crate::{
        BorrowedKernelIrVerificationErrorV1 as VerificationError,
        CanonicalKernelIrReplayAdmissionErrorV18 as AdmissionError,
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work, VerifiedCanonicalKernelIrModuleV18 as Owner,
    };

    const FLOOR: usize = 23;
    for storage in [false, true] {
        let element = if storage {
            Type::StorageObject(crate::StorageLayoutIdV1(0))
        } else {
            Type::Scalar(ScalarType::U32)
        };
        let different = if storage {
            Type::StorageObject(crate::StorageLayoutIdV1(1))
        } else {
            Type::Scalar(ScalarType::U64)
        };
        for slice in [false, true] {
            let kind = if slice {
                CastKind::SliceToGeneric
            } else {
                CastKind::PointerToGeneric
            };
            let carrier =
                |space, access| generic_cast_carrier(slice, element.clone(), space, access);
            let opposite =
                |space, access| generic_cast_carrier(!slice, element.clone(), space, access);
            use AccessMode::{ReadOnly, ReadWrite, WriteOnly};
            use AddressSpace::{Constant, Generic, Global, Private};
            let cases = [
                (
                    "changed pointee or element",
                    carrier(Global, ReadOnly),
                    generic_cast_carrier(slice, different.clone(), Generic, ReadOnly),
                ),
                (
                    "strengthened access",
                    carrier(Global, ReadOnly),
                    carrier(Generic, ReadWrite),
                ),
                (
                    "changed weaker access",
                    carrier(Global, ReadWrite),
                    carrier(Generic, ReadOnly),
                ),
                (
                    "concrete destination",
                    carrier(Global, ReadOnly),
                    carrier(Private, ReadOnly),
                ),
                (
                    "generic source",
                    carrier(Generic, ReadOnly),
                    carrier(Generic, ReadOnly),
                ),
                (
                    "constant write-only source",
                    carrier(Constant, WriteOnly),
                    carrier(Generic, WriteOnly),
                ),
                (
                    "constant read-write source",
                    carrier(Constant, ReadWrite),
                    carrier(Generic, ReadWrite),
                ),
                (
                    "wrong source carrier",
                    opposite(Global, ReadOnly),
                    carrier(Generic, ReadOnly),
                ),
                (
                    "wrong destination carrier",
                    carrier(Global, ReadOnly),
                    opposite(Generic, ReadOnly),
                ),
                (
                    "wrong opcode for both carriers",
                    opposite(Global, ReadOnly),
                    opposite(Generic, ReadOnly),
                ),
            ];
            for (case, from, to) in cases {
                let module = generic_cast_module(kind, from, to, storage);
                let mut work = Work::new(10_000_000);
                work.charge_work(11).unwrap();
                let mut budget = Budget::new(&mut work, 10_000_000);
                budget.reserve_storage(FLOOR).unwrap();
                let result = Owner::from_module_ref_with_verification_budget_v18(
                    &module,
                    super::tests::LIMITS,
                    &mut budget,
                );
                assert_eq!(budget.storage(), FLOOR, "{case}");
                assert!(budget.work() > 11, "{case}");
                assert_eq!(budget.failed_work(), None, "{case}");
                assert_eq!(budget.failed_storage(), None, "{case}");
                assert!(
                    matches!(&result,
                        Err(AdmissionError::Verification(VerificationError::Verification(errors)))
                            if errors.contains(DiagnosticCode::InvalidCast)),
                    "storage={storage}, slice={slice}, case={case}: {result:?}"
                );
            }
        }
    }
}
