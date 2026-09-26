use super::super::{CapacityLimitedText, LoweringDiagnosticCode};
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    Module, ScalarType, StorageLayoutLimitsV1, StorageLayoutV1, check_storage_layouts_v1,
};

const POINTEE: StorageLayoutIdV1 = StorageLayoutIdV1(0);
const POINTER: StorageLayoutIdV1 = StorageLayoutIdV1(1);
const PROFILES: [LoweringTarget; 2] = [
    LoweringTarget::Gfx942XnackMinusV1,
    LoweringTarget::Gfx950XnackMinusV1,
];
const SPACES: [AddressSpace; 5] = [
    AddressSpace::Private,
    AddressSpace::Workgroup,
    AddressSpace::Global,
    AddressSpace::Constant,
    AddressSpace::Generic,
];
const RIGHTS: [AccessMode; 3] = [
    AccessMode::ReadOnly,
    AccessMode::WriteOnly,
    AccessMode::ReadWrite,
];

fn rows(value_space: AddressSpace, encoded_space: AddressSpace, bits: u16) -> Vec<StorageLayoutV1> {
    vec![
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
        },
        StorageLayoutV1 {
            size: u64::from(bits / 8),
            alignment: u32::from(bits / 8),
            kind: StorageLayoutKindV1::Pointer(StoragePointerV1 {
                pointee: POINTEE,
                value_space,
                encoded_space,
                access: AccessMode::ReadOnly,
                stored_bits: bits,
            }),
        },
    ]
}

fn checked(rows: &[StorageLayoutV1]) -> StructurallyCheckedStorageLayoutsV1<'_> {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    check_storage_layouts_v1(
        rows,
        StorageLayoutLimitsV1 {
            rows: 8,
            edges: 16,
            containment_depth: 8,
            object_bytes: 4096,
        },
        &mut budget,
    )
    .unwrap()
}

fn context<'table, 'rows>(
    table: &'table StructurallyCheckedStorageLayoutsV1<'rows>,
    profile: LoweringTarget,
) -> StorageTargetContextV1<'table, 'rows> {
    StorageTargetContextV1::new(
        table,
        profile,
        PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
        PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1,
    )
    .unwrap()
}

fn shape(space: AddressSpace) -> StoragePointerShapeV1 {
    StoragePointerShapeV1 {
        pointee: POINTEE,
        space,
        access: AccessMode::ReadOnly,
    }
}

#[test]
fn storage_pointer_selection_requires_exact_profile_and_both_layouts() {
    let rows = rows(AddressSpace::Private, AddressSpace::Generic, 64);
    let table = checked(&rows);
    for profile in PROFILES {
        context(&table, profile).pointer_recipe(POINTER).unwrap();
        for (source, worker) in [
            ("e-p:64:64", PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1),
            (PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1, "e-p:64:64"),
            (
                PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1,
                PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
            ),
        ] {
            assert_eq!(
                StorageTargetContextV1::new(&table, profile, source, worker).unwrap_err(),
                StoragePointerEmissionErrorV1::DataLayout
            );
        }
    }
    for profile in [
        LoweringTarget::Baseline,
        LoweringTarget::Gfx942StrictFloatV1,
    ] {
        assert_eq!(
            StorageTargetContextV1::new(
                &table,
                profile,
                PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
                PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1,
            )
            .unwrap_err(),
            StoragePointerEmissionErrorV1::TargetProfile
        );
    }
}

#[test]
fn storage_pointer_selection_checks_encoded_width_alignment_and_kind() {
    for profile in PROFILES {
        for bits in [8, 16, 32, 128] {
            let rows = rows(AddressSpace::Private, AddressSpace::Generic, bits);
            let table = checked(&rows);
            assert_eq!(
                context(&table, profile)
                    .pointer_recipe(POINTER)
                    .unwrap_err(),
                StoragePointerEmissionErrorV1::Representation
            );
        }
        let mut rows = rows(AddressSpace::Private, AddressSpace::Generic, 64);
        rows[1].alignment = 4;
        let table = checked(&rows);
        let ctx = context(&table, profile);
        assert_eq!(
            ctx.pointer_recipe(POINTER).unwrap_err(),
            StoragePointerEmissionErrorV1::Representation
        );
        assert_eq!(
            ctx.pointer_recipe(POINTEE).unwrap_err(),
            StoragePointerEmissionErrorV1::NotPointer
        );
        assert_eq!(
            ctx.pointer_recipe(StorageLayoutIdV1(u32::MAX)).unwrap_err(),
            StoragePointerEmissionErrorV1::InvalidLayout
        );
    }
}

#[test]
fn storage_pointer_selection_rejects_unproven_space_conversion_and_zst() {
    for profile in PROFILES {
        for (value, stored, bits) in [
            (AddressSpace::Private, AddressSpace::Global, 64),
            (AddressSpace::Private, AddressSpace::Workgroup, 32),
            (AddressSpace::Workgroup, AddressSpace::Private, 32),
            (AddressSpace::Global, AddressSpace::Private, 32),
            (AddressSpace::Generic, AddressSpace::Private, 32),
            (AddressSpace::Generic, AddressSpace::Global, 64),
            (AddressSpace::Constant, AddressSpace::Private, 32),
            (AddressSpace::Global, AddressSpace::Constant, 64),
        ] {
            let rows = rows(value, stored, bits);
            let table = checked(&rows);
            assert_eq!(
                context(&table, profile)
                    .pointer_recipe(POINTER)
                    .unwrap_err(),
                StoragePointerEmissionErrorV1::Representation
            );
        }
        let mut rows = rows(AddressSpace::Private, AddressSpace::Generic, 64);
        rows[0] = StorageLayoutV1 {
            size: 0,
            alignment: 1,
            kind: StorageLayoutKindV1::Record(Vec::new().into_boxed_slice()),
        };
        let table = checked(&rows);
        assert_eq!(
            context(&table, profile)
                .pointer_recipe(POINTER)
                .unwrap_err(),
            StoragePointerEmissionErrorV1::ZeroSizedReferent
        );
    }
}

#[test]
fn storage_pointer_emits_real_addrspacecasts_for_both_profiles() {
    for profile in PROFILES {
        for (space, native) in [
            (AddressSpace::Private, "ptr addrspace(5)"),
            (AddressSpace::Workgroup, "ptr addrspace(3)"),
            (AddressSpace::Global, "ptr addrspace(1)"),
            (AddressSpace::Constant, "ptr addrspace(4)"),
        ] {
            let rows = rows(space, AddressSpace::Generic, 64);
            let table = checked(&rows);
            let ctx = context(&table, profile);
            let recipe = ctx.pointer_recipe(POINTER).unwrap();
            let module = Module::new("storage_pointer_encoding");
            let mut output = CapacityLimitedText::try_new(&module, 4096).unwrap();
            let stored = recipe
                .emit(
                    &ctx,
                    shape(space),
                    StoragePointerDirectionV1::ValueToStored,
                    "%v1",
                    "%v2",
                    &mut output,
                )
                .unwrap();
            assert_eq!(stored.name(), "%v2");
            assert_eq!(stored.llvm_type(), "ptr");
            let value = recipe
                .emit(
                    &ctx,
                    shape(space),
                    StoragePointerDirectionV1::StoredToValue,
                    stored.name(),
                    "%v3",
                    &mut output,
                )
                .unwrap();
            assert_eq!(value.name(), "%v3");
            assert_eq!(value.llvm_type(), native);
            let text = output.finish(&module).unwrap();
            assert_eq!(
                text,
                format!(
                    "  %v2 = addrspacecast {native} %v1 to ptr\n  %v3 = addrspacecast ptr %v2 to {native}\n"
                )
            );
            for forbidden in [
                "ptrtoint", "inttoptr", "bitcast", "zext", "trunc", "nonnull",
            ] {
                assert!(!text.contains(forbidden));
            }
        }
    }
}

#[test]
fn storage_pointer_native_identity_aliases_without_duplicate_definition() {
    for profile in PROFILES {
        for space in SPACES {
            let bits = if matches!(space, AddressSpace::Private | AddressSpace::Workgroup) {
                32
            } else {
                64
            };
            let rows = rows(space, space, bits);
            let table = checked(&rows);
            let ctx = context(&table, profile);
            let recipe = ctx.pointer_recipe(POINTER).unwrap();
            let mut text = String::new();
            for direction in [
                StoragePointerDirectionV1::ValueToStored,
                StoragePointerDirectionV1::StoredToValue,
            ] {
                let alias = recipe
                    .emit(&ctx, shape(space), direction, "%v1", "%v2", &mut text)
                    .unwrap();
                assert_eq!(alias.name(), "%v1");
                assert_eq!(alias.llvm_type(), PointerWordV1::for_space(space).llvm);
            }
            assert!(text.is_empty());
        }
    }
}

#[test]
fn storage_pointer_constant_identity_and_flat_encoding_never_grant_writes() {
    for profile in PROFILES {
        for encoded in [AddressSpace::Constant, AddressSpace::Generic] {
            for access in RIGHTS {
                let mut rows = rows(AddressSpace::Constant, encoded, 64);
                let StorageLayoutKindV1::Pointer(pointer) = &mut rows[1].kind else {
                    unreachable!()
                };
                pointer.access = access;
                let table = checked(&rows);
                let ctx = context(&table, profile);
                let recipe = ctx.pointer_recipe(POINTER);
                assert_eq!(recipe.is_ok(), access == AccessMode::ReadOnly);
                if access != AccessMode::ReadOnly {
                    assert_eq!(
                        recipe.unwrap_err(),
                        StoragePointerEmissionErrorV1::PointerShape
                    );
                }
            }
        }
    }
}

#[test]
fn storage_pointer_foreign_context_or_table_never_emits() {
    for profile in PROFILES {
        let rows = rows(AddressSpace::Private, AddressSpace::Generic, 64);
        let table = checked(&rows);
        let second_table = checked(&rows);
        let ctx = context(&table, profile);
        let recipe = ctx.pointer_recipe(POINTER).unwrap();
        for other in [context(&table, profile), context(&second_table, profile)] {
            let mut text = String::new();
            assert_eq!(
                recipe
                    .emit(
                        &other,
                        shape(AddressSpace::Private),
                        StoragePointerDirectionV1::ValueToStored,
                        "%v1",
                        "%v2",
                        &mut text,
                    )
                    .unwrap_err(),
                StoragePointerEmissionErrorV1::TableIdentity
            );
            assert!(text.is_empty());
        }
    }
}

#[test]
fn storage_pointer_current_shape_cannot_change_pointee_space_or_rights() {
    for profile in PROFILES {
        for wanted in RIGHTS {
            let mut rows = rows(AddressSpace::Private, AddressSpace::Generic, 64);
            let StorageLayoutKindV1::Pointer(pointer) = &mut rows[1].kind else {
                unreachable!()
            };
            pointer.access = wanted;
            let table = checked(&rows);
            let ctx = context(&table, profile);
            let recipe = ctx.pointer_recipe(POINTER).unwrap();
            for actual in RIGHTS {
                let mut text = String::new();
                let result = recipe.emit(
                    &ctx,
                    StoragePointerShapeV1 {
                        access: actual,
                        ..shape(AddressSpace::Private)
                    },
                    StoragePointerDirectionV1::ValueToStored,
                    "%v1",
                    "%v2",
                    &mut text,
                );
                assert_eq!(result.is_ok(), wanted == actual);
                if wanted != actual {
                    assert_eq!(
                        result.unwrap_err(),
                        StoragePointerEmissionErrorV1::PointerShape
                    );
                    assert!(text.is_empty());
                }
            }
            for wrong in [
                StoragePointerShapeV1 {
                    pointee: POINTER,
                    access: wanted,
                    ..shape(AddressSpace::Private)
                },
                StoragePointerShapeV1 {
                    access: wanted,
                    ..shape(AddressSpace::Workgroup)
                },
                StoragePointerShapeV1 {
                    access: wanted,
                    ..shape(AddressSpace::Global)
                },
            ] {
                let mut text = String::new();
                assert_eq!(
                    recipe
                        .emit(
                            &ctx,
                            wrong,
                            StoragePointerDirectionV1::StoredToValue,
                            "%v1",
                            "%v2",
                            &mut text
                        )
                        .unwrap_err(),
                    StoragePointerEmissionErrorV1::PointerShape
                );
                assert!(text.is_empty());
            }
        }
    }
}

#[test]
fn storage_pointer_formatter_rejects_integer_poison_and_injected_names() {
    let rows = rows(AddressSpace::Private, AddressSpace::Generic, 64);
    let table = checked(&rows);
    let ctx = context(&table, PROFILES[0]);
    let recipe = ctx.pointer_recipe(POINTER).unwrap();
    for bad in [
        "0",
        "null",
        "undef",
        "poison",
        "%",
        "%1",
        "%v1\nret void",
        "%v 1",
    ] {
        let mut text = String::new();
        assert_eq!(
            recipe
                .emit(
                    &ctx,
                    shape(AddressSpace::Private),
                    StoragePointerDirectionV1::ValueToStored,
                    bad,
                    "%v2",
                    &mut text
                )
                .unwrap_err(),
            StoragePointerEmissionErrorV1::OperandName
        );
        assert!(text.is_empty());
    }
    for bad in ["@destination", "%v1", "0", "%v2 = inttoptr"] {
        let mut text = String::new();
        assert_eq!(
            recipe
                .emit(
                    &ctx,
                    shape(AddressSpace::Private),
                    StoragePointerDirectionV1::ValueToStored,
                    "%v1",
                    bad,
                    &mut text
                )
                .unwrap_err(),
            StoragePointerEmissionErrorV1::OperandName
        );
        assert!(text.is_empty());
    }
}

#[test]
fn storage_pointer_uses_existing_bounded_output_exact_and_one_short() {
    let rows = rows(AddressSpace::Private, AddressSpace::Generic, 64);
    let table = checked(&rows);
    let ctx = context(&table, PROFILES[0]);
    let recipe = ctx.pointer_recipe(POINTER).unwrap();
    let module = Module::new("storage_text_bound");
    let expected = "  %v2 = addrspacecast ptr addrspace(5) %v1 to ptr\n";
    for limit in [expected.len(), expected.len() - 1] {
        let mut output = CapacityLimitedText::try_new(&module, limit).unwrap();
        recipe
            .emit(
                &ctx,
                shape(AddressSpace::Private),
                StoragePointerDirectionV1::ValueToStored,
                "%v1",
                "%v2",
                &mut output,
            )
            .unwrap();
        let result = output.finish(&module);
        if limit == expected.len() {
            assert_eq!(result.unwrap(), expected);
        } else {
            assert!(
                result
                    .unwrap_err()
                    .contains(LoweringDiagnosticCode::ResourceLimit)
            );
        }
    }
}

#[test]
fn storage_pointer_propagates_output_failure_without_a_text_operand() {
    struct Refuse;
    impl fmt::Write for Refuse {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            Err(fmt::Error)
        }
    }
    let rows = rows(AddressSpace::Workgroup, AddressSpace::Generic, 64);
    let table = checked(&rows);
    let ctx = context(&table, PROFILES[1]);
    assert_eq!(
        ctx.pointer_recipe(POINTER)
            .unwrap()
            .emit(
                &ctx,
                shape(AddressSpace::Workgroup),
                StoragePointerDirectionV1::ValueToStored,
                "@__fe2o3_lds_object",
                "%v2",
                &mut Refuse
            )
            .unwrap_err(),
        StoragePointerEmissionErrorV1::Output
    );
}
