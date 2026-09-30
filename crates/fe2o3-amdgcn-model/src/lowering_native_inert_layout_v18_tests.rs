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

fn normalize_inert_owner_anchors(llvm: &str, owner: &Owner, target: &str) -> Option<String> {
    let identity = owner.identity();
    let anchor_digest = |domain: &[u8]| {
        let mut hasher = Sha256::new();
        hasher.update(domain);
        hasher.update([18]);
        hasher.update(identity.digest());
        hasher.update(identity.canonical_length().to_le_bytes());
        hasher.update((target.len() as u64).to_le_bytes());
        hasher.update(target.as_bytes());
        for value in [0_u64, 1, 1] {
            hasher.update(value.to_le_bytes());
        }
        let digest = hasher.finalize();
        u64::from_le_bytes(digest[..8].try_into().unwrap()).max(1)
    };
    let guid = anchor_digest(b"FE2O3/PRODUCTION-KIR-LLVM-ISA-ANCHOR-GUID/V1\0");
    let hash = anchor_digest(b"FE2O3/PRODUCTION-KIR-LLVM-ISA-ANCHOR-HASH/V1\0");
    let expected = [
        format!("  call void @llvm.pseudoprobe(i64 {guid}, i64 1, i32 0, i64 -1)"),
        format!("!1 = !{{i64 {guid}, i64 {hash}, !\"kernel\"}}"),
        format!(
            "!2 = !{{!\"sha256:{}\", !\"kir-version:18\", i64 {}, !\"target:{target}\", i64 {guid}, i64 {hash}, i64 1, i64 1}}",
            lower_hex(identity.digest()),
            identity.canonical_length(),
        ),
    ];
    // Validate complete expected records before normalizing only those lines.
    // Executable instructions and source-coordinate records remain untouched.
    for expected in &expected {
        if llvm.lines().filter(|line| *line == expected).count() != 1 {
            return None;
        }
    }
    let mut normalized = String::new();
    for line in llvm.split_inclusive('\n') {
        let content = line.strip_suffix('\n').unwrap_or(line);
        if let Some(ordinal) = expected.iter().position(|expected| content == expected) {
            normalized.push_str(&format!("<owner-bound-anchor-{ordinal}>"));
            if line.ends_with('\n') {
                normalized.push('\n');
            }
        } else {
            normalized.push_str(line);
        }
    }
    Some(normalized)
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
    for (target, before, after) in [
        (
            "gfx942:xnack-",
            lower_canonical_v18_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(&first).unwrap(),
            lower_canonical_v18_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(&second).unwrap(),
        ),
        (
            "gfx950:xnack-",
            lower_canonical_v18_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(&first).unwrap(),
            lower_canonical_v18_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(&second).unwrap(),
        ),
    ] {
        let first_anchor = format!("!\"sha256:{}\"", lower_hex(first.identity().digest()));
        let second_anchor = format!("!\"sha256:{}\"", lower_hex(second.identity().digest()));
        assert!(before.contains(&first_anchor));
        assert!(after.contains(&second_anchor));
        assert_ne!(before, after);
        // The owner digest also determines the pseudo-probe GUID and hash.
        assert_eq!(
            normalize_inert_owner_anchors(&before, &first, target).unwrap(),
            normalize_inert_owner_anchors(&after, &second, target).unwrap(),
        );
    }
    drop(second);
    budget.release_storage(b).unwrap();
    drop(first);
    budget.release_storage(a).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn native_v18_inert_anchor_oracle_preserves_executable_and_coordinate_differences() {
    let mut module = module_v18();
    all_layout_kinds(&mut module);
    let mut work = Work::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, 100_000_000);
    let (owner, storage) = admit_v18(&module, &mut budget);
    let llvm =
        lower_canonical_v18_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(
            &owner,
        )
        .unwrap();
    let normalized = normalize_inert_owner_anchors(&llvm, &owner, "gfx942:xnack-").unwrap();
    let probe = llvm
        .lines()
        .find(|line| line.starts_with("  call void @llvm.pseudoprobe("))
        .unwrap();
    let lookalike = format!("; not an anchor: {probe}\n");
    let prefixed = format!("{lookalike}{llvm}");
    let prefixed = normalize_inert_owner_anchors(&prefixed, &owner, "gfx942:xnack-").unwrap();
    assert_eq!(prefixed, format!("{lookalike}{normalized}"));
    for (before, after) in [
        ("%v1 = xor i32 %arg0, -1", "%v1 = xor i32 %arg0, 0"),
        (
            "!3 = !{i64 1, i64 0, i64 0, i64 0}",
            "!3 = !{i64 1, i64 0, i64 0, i64 1}",
        ),
    ] {
        assert_eq!(llvm.matches(before).count(), 1);
        let changed = llvm.replace(before, after);
        assert_ne!(
            normalize_inert_owner_anchors(&changed, &owner, "gfx942:xnack-").unwrap(),
            normalized,
        );
    }
    let wrong_version = llvm.replace("!\"kir-version:18\"", "!\"kir-version:12\"");
    assert!(normalize_inert_owner_anchors(&wrong_version, &owner, "gfx942:xnack-").is_none());
    assert!(normalize_inert_owner_anchors(&llvm, &owner, "gfx950:xnack-").is_none());
    drop(owner);
    budget.release_storage(storage).unwrap();
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
