use super::*;
use fe2o3_kernel_ir::{
    FixedVectorTypeV12, StorageFieldV1 as Field, StorageLayoutIdV1 as Id,
    StorageLayoutKindV1 as Kind, StorageLayoutV1 as Layout, StoragePointerV1,
    StorageVariantEncodingV1, StorageVariantV1, VectorLayoutV12,
};

fn field(offset: u64, layout: u32) -> Field {
    Field {
        offset,
        layout: Id(layout),
    }
}

fn all_layout_kinds(module: &mut Module) {
    scalar_metadata_v1763(module);
    module.storage_layouts.extend([
        Layout {
            size: 4,
            alignment: 4,
            kind: Kind::Record(vec![field(0, 0)].into_boxed_slice()),
        },
        Layout {
            size: 8,
            alignment: 4,
            kind: Kind::Array {
                element: Id(0),
                length: 2,
                stride: 4,
            },
        },
        Layout {
            size: 8,
            alignment: 8,
            kind: Kind::Pointer(StoragePointerV1 {
                pointee: Id(0),
                value_space: AddressSpace::Global,
                encoded_space: AddressSpace::Global,
                access: AccessMode::ReadOnly,
                stored_bits: 64,
            }),
        },
        Layout {
            size: 8,
            alignment: 8,
            kind: Kind::Scalar(ScalarType::Index),
        },
        Layout {
            size: 16,
            alignment: 8,
            kind: Kind::Slice {
                element: Id(0),
                value_space: AddressSpace::Global,
                access: AccessMode::ReadOnly,
                data: field(0, 4),
                length: field(8, 5),
            },
        },
        Layout {
            size: 16,
            alignment: 16,
            kind: Kind::Vector(FixedVectorTypeV12::new(
                ScalarType::U32,
                4,
                VectorLayoutV12::Contiguous,
            )),
        },
        Layout {
            size: 4,
            alignment: 4,
            kind: Kind::Union(vec![field(0, 0), field(0, 2)].into_boxed_slice()),
        },
        Layout {
            size: 4,
            alignment: 4,
            kind: Kind::Variants {
                encoding: StorageVariantEncodingV1::Direct { tag: field(0, 0) },
                variants: vec![
                    StorageVariantV1 {
                        discriminant: 11,
                        direct_tag_bits: Some(0),
                        uninhabited: false,
                        layout: Id(1),
                    },
                    StorageVariantV1 {
                        discriminant: 17,
                        direct_tag_bits: Some(1),
                        uninhabited: false,
                        layout: Id(2),
                    },
                ]
                .into_boxed_slice(),
            },
        },
    ]);
}

#[test]
fn native_v18_all_validated_inert_layout_kinds_lower_on_both_targets() {
    let mut module = module_v18();
    all_layout_kinds(&mut module);
    let mut work = Work::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000_000);
    budget.reserve_storage(29).unwrap();
    let (owner, retained) = admit_v18(&module, &mut budget);
    let before = (budget.work(), budget.storage(), budget.peak_storage());
    let original_bytes = owner.canonical_bytes().to_vec();
    for (cpu, llvm) in [
        ("gfx942", lower_canonical_v18_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner).unwrap()),
        ("gfx950", lower_canonical_v18_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner).unwrap()),
    ] {
        assert!(llvm.contains("define amdgpu_kernel void @kernel("));
        assert!(llvm.contains(&format!("\"target-cpu\"=\"{cpu}\"")));
        assert!(llvm.contains(&format!("sha256:{}", lower_hex(owner.identity().digest()))));
        assert!(llvm.contains("kir-version:18"));
    }
    assert_eq!(owner.module().storage_layouts, module.storage_layouts);
    assert_eq!(owner.canonical_bytes(), original_bytes);
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        before
    );
    assert!(reject_unsupported_v12_module(owner.module()).is_err());
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 29);
}

#[test]
fn native_v18_inert_layout_changes_remain_in_the_exact_anchor_identity() {
    let mut module = module_v18();
    all_layout_kinds(&mut module);
    let mut work = Work::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000_000);
    let (first, a) = admit_v18(&module, &mut budget);
    // This unused declaration changes, not the executable graph.
    module.storage_layouts[3].size = 12;
    let Kind::Array { length, .. } = &mut module.storage_layouts[3].kind else {
        unreachable!()
    };
    *length = 3;
    let (second, b) = admit_v18(&module, &mut budget);
    assert_ne!(first.identity().digest(), second.identity().digest());
    assert!(
        SemanticAnchorInputV1::NativeV18(&first)
            .validate(second.module())
            .is_err()
    );
    assert_eq!(
        first.identity().canonical_length(),
        second.identity().canonical_length()
    );
    for (before, after) in [
        (
            lower_canonical_v18_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(&first).unwrap(),
            lower_canonical_v18_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(&second).unwrap(),
        ),
        (
            lower_canonical_v18_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(&first).unwrap(),
            lower_canonical_v18_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(&second).unwrap(),
        ),
    ] {
        let first_anchor = format!("!\"sha256:{}\"", lower_hex(first.identity().digest()));
        let second_anchor = format!("!\"sha256:{}\"", lower_hex(second.identity().digest()));
        assert!(before.contains(&first_anchor));
        assert!(after.contains(&second_anchor));
        assert_ne!(before, after);
        // Only the exact quoted owner-identity metadata may differ.
        assert_eq!(
            before.replace(&first_anchor, "!\"sha256:<owner>\""),
            after.replace(&second_anchor, "!\"sha256:<owner>\""),
        );
    }
    drop(second);
    budget.release_storage(b).unwrap();
    drop(first);
    budget.release_storage(a).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn native_v18_malformed_unused_layouts_never_obtain_an_owner_for_lowering() {
    for corrupt in 0..4 {
        let mut module = module_v18();
        all_layout_kinds(&mut module);
        match corrupt {
            0 => module.storage_layouts[3].size += 1,
            1 => {
                module.storage_layouts[2].kind = Kind::Record(vec![field(0, 2)].into_boxed_slice())
            }
            2 => {
                let Kind::Pointer(pointer) = &mut module.storage_layouts[4].kind else {
                    unreachable!()
                };
                pointer.pointee = Id(999);
            }
            _ => {
                let Kind::Slice { length, .. } = &mut module.storage_layouts[6].kind else {
                    unreachable!()
                };
                length.offset = 0;
            }
        }
        let mut work = Work::new(1_000_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000_000);
        budget.reserve_storage(29).unwrap();
        assert!(
            Owner::from_module_ref_with_verification_budget_v18(
                &module,
                StorageLayoutLimitsV1 {
                    rows: 64,
                    edges: 256,
                    containment_depth: 32,
                    object_bytes: 4096
                },
                &mut budget,
            )
            .is_err()
        );
        assert_eq!(budget.storage(), 29);
    }
}

#[test]
fn native_v18_inert_tables_do_not_admit_storage_in_uncalled_signatures() {
    for ty in [
        Type::pointer(
            Type::StorageObject(Id(2)),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
        Type::slice(
            Type::pointer(
                Type::StorageObject(Id(2)),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
    ] {
        let mut module = module_v18();
        all_layout_kinds(&mut module);
        module.functions.push(Function::external_import(
            "uncalled",
            Signature::new(vec![ty], vec![]),
        ));
        let mut work = Work::new(1_000_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000_000);
        let (owner, retained) = admit_v18(&module, &mut budget);
        for error in [
            lower_canonical_v18_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner).unwrap_err(),
            lower_canonical_v18_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner).unwrap_err(),
        ] {
            assert!(error.contains(LoweringDiagnosticCode::UnsupportedType));
            assert!(error.diagnostics()[0].location.function.is_some());
        }
        drop(owner);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}
