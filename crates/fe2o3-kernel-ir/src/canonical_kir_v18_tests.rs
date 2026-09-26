use super::*;
use crate::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Constant,
    Function, FunctionRole, MemoryAccess, Operation, OperationKind, ScalarType, Signature,
    StorageFieldV1, StorageLayoutKindV1, StorageLayoutV1, StorageOperationV1, StorageProjectionV1,
    Terminator, Type, ValueDef, ValueId, WorkgroupMemory, WorkgroupMemoryExtent,
};

pub(super) const LIMITS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 64,
    edges: 256,
    containment_depth: 32,
    object_bytes: 4096,
};
pub(super) fn scalar() -> StorageLayoutV1 {
    StorageLayoutV1 {
        size: 8,
        alignment: 8,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U64),
    }
}
pub(super) fn small_module() -> Module {
    let mut module = Module::new("m");
    module.storage_layouts.push(scalar());
    module
}
fn address(row: u32, space: AddressSpace) -> Type {
    Type::pointer(
        Type::StorageObject(StorageLayoutIdV1(row)),
        space,
        AccessMode::ReadWrite,
    )
}
pub(super) fn fixture(space: AddressSpace) -> Module {
    let mut module = small_module();
    module.storage_layouts.push(StorageLayoutV1 {
        size: 16,
        alignment: 8,
        kind: StorageLayoutKindV1::Record(
            vec![
                StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(0),
                },
                StorageFieldV1 {
                    offset: 8,
                    layout: StorageLayoutIdV1(0),
                },
            ]
            .into_boxed_slice(),
        ),
    });
    let allocation = if space == AddressSpace::Workgroup {
        OperationKind::WorkgroupMemory(WorkgroupMemory {
            element: Type::StorageObject(StorageLayoutIdV1(1)),
            extent: WorkgroupMemoryExtent::Static(1),
            alignment: 8,
        })
    } else {
        OperationKind::Alloca {
            element: Type::StorageObject(StorageLayoutIdV1(1)),
            count: None,
            address_space: space,
            alignment: 8,
        }
    };
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::new(
            vec![ValueDef::new(ValueId(0), address(1, space))],
            allocation,
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U64))],
            OperationKind::Constant(Constant::U64(11)),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(2), address(0, space))],
            OperationKind::Storage(StorageOperationV1::Project {
                base: ValueId(0),
                step: StorageProjectionV1::Field(1),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::Storage(StorageOperationV1::WriteValue {
                address: ValueId(2),
                value: ValueId(1),
                access: MemoryAccess::new(space, 8),
            }),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U64))],
            OperationKind::Storage(StorageOperationV1::ReadValue {
                address: ValueId(2),
                access: MemoryAccess::new(space, 8),
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(3)],
    });
    module.functions.push(Function::definition(
        "f",
        Signature::new(vec![], vec![Type::Scalar(ScalarType::U64)]),
        vec![],
        vec![block],
    ));
    module
}
pub(super) fn admit(module: &Module) -> VerifiedCanonicalKernelIrModuleV18 {
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    budget.reserve_storage(17).unwrap();
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            LIMITS,
            &mut budget,
        )
        .unwrap();
    assert_eq!(budget.storage(), 17);
    assert!(
        receipt.retained_storage()
            >= std::mem::size_of::<VerifiedCanonicalKernelIrModuleV18>()
                + owner.canonical_bytes().len()
    );
    owner
}
fn reject(module: &Module) -> CanonicalKernelIrReplayAdmissionErrorV18 {
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    budget.reserve_storage(17).unwrap();
    let result = VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        module,
        LIMITS,
        &mut budget,
    );
    assert_eq!(budget.storage(), 17);
    result.unwrap_err()
}

#[test]
fn canonical_storage_actual_private_and_workgroup_modules_roundtrip_without_graph_substitution() {
    for space in [AddressSpace::Private, AddressSpace::Workgroup] {
        let source = fixture(space);
        let owner = admit(&source);
        assert_eq!(owner.module(), &source);
        assert!(!std::ptr::eq(owner.module(), &source));
        let verified = owner.verified_storage_module_ref_v1();
        assert!(std::ptr::eq(verified.module(), owner.module()));
        assert!(std::ptr::eq(verified.storage().module(), owner.module()));
        assert!(std::ptr::eq(
            verified.storage().layouts().rows(),
            owner.module().storage_layouts.as_slice()
        ));
        let mut work = Work::new(10_000_000);
        let mut budget = Budget::new(&mut work, 10_000_000);
        budget.reserve_storage(23).unwrap();
        let (other, _) =
            VerifiedCanonicalKernelIrModuleV18::from_canonical_bytes_with_verification_budget_v18(
                owner.canonical_bytes(),
                LIMITS,
                &mut budget,
            )
            .unwrap();
        assert_eq!(budget.storage(), 23);
        assert_eq!(other.module(), owner.module());
        assert_eq!(other.identity(), owner.identity());
        assert!(!std::ptr::eq(other.module(), owner.module()));
    }
}

#[test]
fn canonical_storage_explicit_device_export_and_external_import_roles_survive() {
    let mut module = small_module();
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::device_ffi_export(
        "export",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.functions.push(Function::external_import(
        "import",
        Signature::new(vec![], vec![]),
    ));
    let owner = admit(&module);
    assert_eq!(
        owner.module().functions[0].role,
        FunctionRole::DeviceFfiExport
    );
    assert_eq!(
        owner.module().functions[1].role,
        FunctionRole::ExternalImport
    );
    module.functions[0].role = FunctionRole::ExternalImport;
    assert!(matches!(
        reject(&module),
        CanonicalKernelIrReplayAdmissionErrorV18::Verification(_)
    ));
}

#[test]
fn canonical_storage_execution_lifecycle_uses_same_shared_engine() {
    let mut module = crate::verification_execution_lifecycle_v15::tests::fixture(1);
    module.storage_layouts.push(scalar());
    let owner = admit(&module);
    assert_eq!(owner.module(), &module);
}

#[test]
fn canonical_storage_all_layout_kinds_are_admitted_by_the_actual_shared_core() {
    let mut module = Module::new("layouts");
    module.storage_layouts = crate::wire::storage_v18_tests::rows();
    // The codec fixture preserves untrusted discriminants; niche admission
    // requires the source variant ordinals, unlike a direct tag encoding.
    assert!(matches!(
        reject(&module),
        CanonicalKernelIrReplayAdmissionErrorV18::Layout(StorageLayoutErrorV1::Invalid {
            row: 8,
            problem: crate::StorageLayoutProblemV1::Variant,
        })
    ));
    let StorageLayoutKindV1::Variants { variants, .. } = &mut module.storage_layouts[8].kind else {
        unreachable!()
    };
    for (index, variant) in variants.iter_mut().enumerate() {
        variant.discriminant = index as u128;
    }
    let owner = admit(&module);
    assert_eq!(owner.module(), &module);
    assert_eq!(
        owner
            .verified_storage_module_ref_v1()
            .storage()
            .layouts()
            .rows(),
        module.storage_layouts.as_slice()
    );
}

#[test]
fn canonical_storage_layout_identity_is_module_bound_not_a_bare_ordinal() {
    let left = admit(&small_module());
    let mut changed = small_module();
    changed.storage_layouts[0].kind = StorageLayoutKindV1::Scalar(ScalarType::I64);
    let right = admit(&changed);
    assert_ne!(left.identity(), right.identity());
    assert_ne!(
        left.layout_identity(StorageLayoutIdV1(0)),
        right.layout_identity(StorageLayoutIdV1(0))
    );
    assert_eq!(left.layout_identity(StorageLayoutIdV1(1)), None);
    assert_eq!(left.layout_identity(StorageLayoutIdV1(u32::MAX)), None);
    let equivalent = admit(&small_module());
    assert_eq!(equivalent.identity(), left.identity());
    assert!(!std::ptr::eq(equivalent.module(), left.module()));
}

#[test]
fn canonical_storage_rejects_invalid_table_and_stale_body_id_before_owner_creation() {
    let mut module = fixture(AddressSpace::Private);
    module.storage_layouts[1].kind = StorageLayoutKindV1::Record(
        vec![StorageFieldV1 {
            offset: 0,
            layout: StorageLayoutIdV1(99),
        }]
        .into_boxed_slice(),
    );
    assert!(matches!(
        reject(&module),
        CanonicalKernelIrReplayAdmissionErrorV18::Layout(_)
    ));
    module = fixture(AddressSpace::Private);
    module.storage_layouts.pop();
    assert!(matches!(
        reject(&module),
        CanonicalKernelIrReplayAdmissionErrorV18::Verification(_)
    ));
    module.storage_layouts.clear();
    assert!(matches!(
        reject(&module),
        CanonicalKernelIrReplayAdmissionErrorV18::Verification(_)
    ));
}

#[test]
fn canonical_storage_does_not_turn_wire_roundtrip_into_cfg_or_ssa_authority() {
    for mutation in 0..4 {
        let mut module = fixture(AddressSpace::Private);
        let body = module.functions[0].body.as_mut().unwrap();
        match mutation {
            0 => body.blocks[0].terminator = None,
            1 => body.blocks[0].operations[4].results[0].ty = Type::Scalar(ScalarType::U32),
            2 => body.blocks[0].operations.swap(0, 4),
            _ => {
                body.blocks[0].operations[2].results[0].ty =
                    Type::StorageObject(StorageLayoutIdV1(0))
            }
        }
        assert!(
            matches!(
                reject(&module),
                CanonicalKernelIrReplayAdmissionErrorV18::Verification(_)
            ),
            "mutation {mutation}"
        );
    }
}

#[test]
fn canonical_storage_table_mutation_reauthenticates_same_body_against_new_rows() {
    let mut module = fixture(AddressSpace::Private);
    let owner = admit(&module);
    module.storage_layouts[0].kind = StorageLayoutKindV1::Scalar(ScalarType::I64);
    assert!(matches!(
        reject(&module),
        CanonicalKernelIrReplayAdmissionErrorV18::Verification(_)
    ));
    assert_eq!(
        owner.module().storage_layouts[0].kind,
        StorageLayoutKindV1::Scalar(ScalarType::U64)
    );
}

#[test]
fn canonical_storage_old_profiles_and_old_verifier_remain_closed() {
    let module = fixture(AddressSpace::Private);
    let owner = admit(&module);
    for version in [1, 12, 15, 16, 17] {
        assert!(
            crate::wire::count_module_with_work_v1(
                &module,
                version,
                &mut Work::new(1_000_000),
                false
            )
            .is_err()
        );
    }
    assert!(crate::verify_module_ref(owner.module()).is_err());
    assert!(crate::encode_module_v12(owner.module()).is_err());
    assert!(crate::decode_module_v17(owner.canonical_bytes()).is_err());
    let bytes = crate::encode_module_v12(&Module::new("m")).unwrap();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    assert!(matches!(
        VerifiedCanonicalKernelIrModuleV18::from_canonical_bytes_with_verification_budget_v18(
            &bytes,
            LIMITS,
            &mut budget
        ),
        Err(CanonicalKernelIrReplayAdmissionErrorV18::Decode(
            KernelIrDecodeError::UnknownVersion(12)
        ))
    ));
    assert_eq!(budget.storage(), 0);
}

#[test]
fn canonical_storage_byte_entry_rejects_every_truncation_and_corrupt_header() {
    let owner = admit(&fixture(AddressSpace::Private));
    for end in 0..owner.canonical_bytes().len() {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        budget.reserve_storage(29).unwrap();
        assert!(
            VerifiedCanonicalKernelIrModuleV18::from_canonical_bytes_with_verification_budget_v18(
                &owner.canonical_bytes()[..end],
                LIMITS,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), 29);
    }
    let mut changed = owner.canonical_bytes().to_vec();
    changed[16] = 1;
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    assert!(matches!(
        VerifiedCanonicalKernelIrModuleV18::from_canonical_bytes_with_verification_budget_v18(
            &changed,
            LIMITS,
            &mut budget
        ),
        Err(CanonicalKernelIrReplayAdmissionErrorV18::Decode(
            KernelIrDecodeError::ReservedNonZero { .. }
        ))
    ));
}

#[test]
fn canonical_storage_caller_layout_limits_are_enforced_before_cfg() {
    let module = small_module();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let mut limits = LIMITS;
    limits.rows = 0;
    assert!(matches!(
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            &module,
            limits,
            &mut budget
        ),
        Err(CanonicalKernelIrReplayAdmissionErrorV18::Layout(
            StorageLayoutErrorV1::Invalid {
                problem: crate::StorageLayoutProblemV1::Rows,
                ..
            }
        ))
    ));
    assert_eq!(budget.storage(), 0);
}

fn enum_construction_fixture(niche: bool, variant: u32) -> Module {
    use crate::{StorageVariantEncodingV1, StorageVariantV1};
    let mut module = small_module();
    module.storage_layouts.push(StorageLayoutV1 {
        size: 8,
        alignment: 8,
        kind: StorageLayoutKindV1::Record(
            vec![StorageFieldV1 {
                offset: 0,
                layout: StorageLayoutIdV1(0),
            }]
            .into_boxed_slice(),
        ),
    });
    let tag = StorageFieldV1 {
        offset: if niche { 0 } else { 8 },
        layout: StorageLayoutIdV1(0),
    };
    module.storage_layouts.push(StorageLayoutV1 {
        size: if niche { 8 } else { 16 },
        alignment: 8,
        kind: StorageLayoutKindV1::Variants {
            encoding: if niche {
                StorageVariantEncodingV1::Niche {
                    tag,
                    untagged_variant: 0,
                    first_niche_variant: 1,
                    last_niche_variant: 1,
                    niche_start: u64::MAX as u128,
                }
            } else {
                StorageVariantEncodingV1::Direct { tag }
            },
            variants: [3_u128, 9]
                .into_iter()
                .enumerate()
                .map(|(index, bits)| StorageVariantV1 {
                    discriminant: if niche {
                        index as u128
                    } else {
                        91 + index as u128
                    },
                    direct_tag_bits: (!niche).then_some(bits),
                    uninhabited: false,
                    layout: StorageLayoutIdV1(1),
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        },
    });
    let space = AddressSpace::Private;
    let memory = MemoryAccess::new(space, 8);
    let write_only = |row| {
        Type::pointer(
            Type::StorageObject(StorageLayoutIdV1(row)),
            space,
            AccessMode::WriteOnly,
        )
    };
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::new(
            vec![ValueDef::new(ValueId(0), address(2, space))],
            OperationKind::Alloca {
                element: Type::StorageObject(StorageLayoutIdV1(2)),
                count: None,
                address_space: space,
                alignment: 8,
            },
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U64))],
            OperationKind::Constant(Constant::U64(11)),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(2), write_only(1))],
            OperationKind::Storage(StorageOperationV1::Project {
                base: ValueId(0),
                step: StorageProjectionV1::VariantForWrite { index: variant },
            }),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(3), write_only(0))],
            OperationKind::Storage(StorageOperationV1::Project {
                base: ValueId(2),
                step: StorageProjectionV1::Field(0),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::Storage(StorageOperationV1::WriteValue {
                address: ValueId(3),
                value: ValueId(1),
                access: memory,
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::Storage(StorageOperationV1::SetDiscriminant {
                address: ValueId(0),
                variant,
                access: memory,
            }),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(4), address(1, space))],
            OperationKind::Storage(StorageOperationV1::Project {
                base: ValueId(0),
                step: StorageProjectionV1::Variant {
                    index: variant,
                    access: memory,
                },
            }),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(5), address(0, space))],
            OperationKind::Storage(StorageOperationV1::Project {
                base: ValueId(4),
                step: StorageProjectionV1::Field(0),
            }),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(6), Type::Scalar(ScalarType::U64))],
            OperationKind::Storage(StorageOperationV1::ReadValue {
                address: ValueId(5),
                access: memory,
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(6)],
    });
    module.functions.push(Function::definition(
        "enum",
        Signature::new(vec![], vec![Type::Scalar(ScalarType::U64)]),
        vec![],
        vec![block],
    ));
    module
}

#[test]
fn canonical_storage_enum_construction_is_lossless_for_direct_encoded_and_untagged_niches() {
    // Canonical admission is structural only. It does not execute or certify the
    // tag/payload state, and an encoded niche can overwrite the same payload bytes.
    for niche in [false, true] {
        for variant in [0, 1] {
            let module = enum_construction_fixture(niche, variant);
            let owner = admit(&module);
            assert_eq!(owner.module(), &module);
            let mut work = Work::new(10_000_000);
            let mut budget = Budget::new(&mut work, 10_000_000);
            budget.reserve_storage(31).unwrap();
            let (decoded, _) =
                VerifiedCanonicalKernelIrModuleV18::from_canonical_bytes_with_verification_budget_v18(
                    owner.canonical_bytes(), LIMITS, &mut budget).unwrap();
            assert_eq!(decoded.module(), &module);
            assert_eq!(decoded.identity(), owner.identity());
            assert_eq!(budget.storage(), 31);
            let view = decoded.verified_storage_module_ref_v1();
            assert!(std::ptr::eq(view.module(), decoded.module()));
            assert!(std::ptr::eq(
                view.storage().layouts().rows(),
                decoded.module().storage_layouts.as_slice()
            ));
        }
    }
}

#[test]
fn canonical_storage_enum_construction_cannot_manufacture_read_rights_or_variant_authority() {
    for mutation in 0..4 {
        let mut module = enum_construction_fixture(false, 0);
        match mutation {
            0 => {
                module.functions[0].body.as_mut().unwrap().blocks[0].operations[2].results[0].ty =
                    address(1, AddressSpace::Private)
            }
            1 => {
                let OperationKind::Storage(StorageOperationV1::SetDiscriminant { access, .. }) =
                    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[5].kind
                else {
                    unreachable!()
                };
                access.volatile = true;
            }
            2 => {
                let OperationKind::Storage(StorageOperationV1::SetDiscriminant { variant, .. }) =
                    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[5].kind
                else {
                    unreachable!()
                };
                *variant = u32::MAX;
            }
            _ => {
                let StorageLayoutKindV1::Variants { variants, .. } =
                    &mut module.storage_layouts[2].kind
                else {
                    unreachable!()
                };
                variants[0].uninhabited = true;
            }
        }
        assert!(
            matches!(
                reject(&module),
                CanonicalKernelIrReplayAdmissionErrorV18::Verification(_)
            ),
            "mutation {mutation}"
        );
    }
}
