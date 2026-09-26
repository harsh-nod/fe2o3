use super::super::storage_native_v18::tests::*;
use super::*;

fn rows() -> Vec<Row> {
    vec![
        scalar(),
        Row {
            size: 8,
            alignment: 4,
            kind: Kind::Record(
                vec![Field {
                    offset: 4,
                    layout: Id(0),
                }]
                .into_boxed_slice(),
            ),
        },
        Row {
            size: 8,
            alignment: 4,
            kind: Kind::Variants {
                encoding: Encoding::Direct {
                    tag: Field {
                        offset: 0,
                        layout: Id(0),
                    },
                },
                variants: [10_u128, 20]
                    .map(|bits| Variant {
                        discriminant: bits,
                        direct_tag_bits: Some(bits),
                        uninhabited: false,
                        layout: Id(1),
                    })
                    .into(),
            },
        },
    ]
}
fn set(address: u32, variant: u32, alignment: u32) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Storage(Storage::SetDiscriminant {
            address: ValueId(address),
            variant,
            access: MemoryAccess::new(Space::Private, alignment),
        }),
    )
}
fn construction(id: u32, row: u32, base: u32, step: Projection) -> Operation {
    value(
        id,
        Type::pointer(object(row), Space::Private, AccessMode::WriteOnly),
        OperationKind::Storage(Storage::Project {
            base: ValueId(base),
            step,
        }),
    )
}
fn niche(rows: &mut [Row], tag: Field, start: u128) {
    let Kind::Variants { encoding, variants } = &mut rows[2].kind else {
        unreachable!()
    };
    *encoding = Encoding::Niche {
        tag,
        untagged_variant: 0,
        first_niche_variant: 1,
        last_niche_variant: 1,
        niche_start: start,
    };
    for (index, variant) in variants.iter_mut().enumerate() {
        variant.discriminant = index as u128;
        variant.direct_tag_bits = None;
    }
}

#[test]
fn construction_preserves_payload_before_tag_and_active_read_occurs_once() {
    let module = module(
        rows(),
        vec![
            allocate(0, 2, Space::Private, 4),
            construction(1, 1, 0, Projection::VariantForWrite { index: 1 }),
            construction(2, 0, 1, Projection::Field(0)),
            constant(3, 99),
            write(2, 3, Space::Private, 4),
            set(0, 1, 4),
            project(
                4,
                1,
                0,
                Projection::Variant {
                    index: 1,
                    access: MemoryAccess::new(Space::Private, 4),
                },
                Space::Private,
            ),
        ],
        Profile::Gfx942,
    );
    let text = emit(&module, Profile::Gfx942);
    let payload = text.find("store i32 99").unwrap();
    let tag = text.find("store i32 20").unwrap();
    let observe = text.find(".tag = load i32").unwrap();
    assert!(payload < tag && tag < observe);
    assert_eq!(text.matches(".tag = load ").count(), 1);
    assert_eq!(text.matches(" = alloca ").count(), 1);
    assert!(text.contains(".active = icmp eq i32 "));
}

#[test]
fn direct_signed_and_128_tags_keep_exact_physical_bits() {
    for (scalar, bytes, bits) in [
        (ScalarType::I32, 4, u128::from(u32::MAX)),
        (ScalarType::I128, 16, u128::MAX),
    ] {
        let mut rows = rows();
        rows[0] = Row {
            size: bytes,
            alignment: bytes as u32,
            kind: Kind::Scalar(scalar),
        };
        rows[1] = Row {
            size: bytes * 2,
            alignment: bytes as u32,
            kind: Kind::Record(
                vec![Field {
                    offset: bytes,
                    layout: Id(0),
                }]
                .into_boxed_slice(),
            ),
        };
        rows[2].size = bytes * 2;
        rows[2].alignment = bytes as u32;
        let Kind::Variants { variants, .. } = &mut rows[2].kind else {
            unreachable!()
        };
        variants[1].direct_tag_bits = Some(bits);
        let module = module(
            rows,
            vec![
                allocate(0, 2, Space::Private, bytes as u32),
                set(0, 1, bytes as u32),
            ],
            Profile::Gfx950,
        );
        let text = emit(&module, Profile::Gfx950);
        assert!(text.contains(&format!("store i{} {bits},", bytes * 8)));
        assert!(!text.contains("store i32 20,"));
    }
}

#[test]
fn bool_niche_is_raw_i8_and_not_an_invalid_bool_ssa_value() {
    let mut rows = rows();
    rows[0] = Row {
        size: 1,
        alignment: 1,
        kind: Kind::Scalar(ScalarType::Bool),
    };
    niche(
        &mut rows,
        Field {
            offset: 0,
            layout: Id(0),
        },
        254,
    );
    let module = module(
        rows,
        vec![
            allocate(0, 2, Space::Private, 4),
            set(0, 1, 1),
            project(
                1,
                1,
                0,
                Projection::Variant {
                    index: 1,
                    access: MemoryAccess::new(Space::Private, 1),
                },
                Space::Private,
            ),
        ],
        Profile::Gfx942,
    );
    let text = emit(&module, Profile::Gfx942);
    assert!(text.contains("store i8 254,"));
    assert!(text.contains(".tag = load i8,"));
    assert!(text.contains(".delta = sub i8 "));
    assert!(!text.contains("i1 254"));
}

#[test]
fn untagged_niche_and_construction_do_not_read_or_write_the_tag() {
    let mut rows = rows();
    niche(
        &mut rows,
        Field {
            offset: 0,
            layout: Id(0),
        },
        0,
    );
    let module = module(
        rows,
        vec![
            allocate(0, 2, Space::Private, 4),
            construction(1, 1, 0, Projection::VariantForWrite { index: 0 }),
            set(0, 0, 4),
        ],
        Profile::Gfx942,
    );
    let text = emit(&module, Profile::Gfx942);
    assert!(!text.contains(" = load "));
    assert!(!text.contains("  store "));
    assert!(!text.contains("call void @llvm.trap()"));
    assert!(text.contains("%v1 = getelementptr i8, ptr addrspace(5) %v0, i64 0"));
}

#[test]
fn volatile_variant_tag_read_is_not_silently_downgraded() {
    let mut access = MemoryAccess::new(Space::Private, 4);
    access.volatile = true;
    let module = module(
        rows(),
        vec![
            allocate(0, 2, Space::Private, 4),
            set(0, 1, 4),
            project(
                1,
                1,
                0,
                Projection::Variant { index: 1, access },
                Space::Private,
            ),
        ],
        Profile::Gfx942,
    );
    let text = emit(&module, Profile::Gfx942);
    assert_eq!(text.matches(".tag = load volatile i32,").count(), 1);
    assert!(!text.contains(".tag = load i32,"));
}

#[test]
fn pointer_niche_store_is_physical_and_active_decode_checks_exact_target_width() {
    let mut rows = rows();
    rows.push(Row {
        size: 8,
        alignment: 8,
        kind: Kind::Pointer(StoragePointerV1 {
            pointee: Id(0),
            value_space: Space::Global,
            encoded_space: Space::Generic,
            access: AccessMode::ReadOnly,
            stored_bits: 64,
        }),
    });
    rows[2].alignment = 8;
    niche(
        &mut rows,
        Field {
            offset: 0,
            layout: Id(3),
        },
        0,
    );
    let mut module = module(
        rows,
        vec![allocate(0, 2, Space::Private, 8), set(0, 1, 8)],
        Profile::Gfx942,
    );
    let text = emit(&module, Profile::Gfx942);
    assert!(text.contains("store i64 0,"));
    assert!(!text.contains("inttoptr"));
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(project(
            1,
            1,
            0,
            Projection::Variant {
                index: 1,
                access: MemoryAccess::new(Space::Private, 8),
            },
            Space::Private,
        ));
    let decoded = emit(&module, Profile::Gfx942);
    assert!(decoded.contains(".tag = load i64,"));
    assert!(decoded.contains(".active = icmp eq i64"));
    let row = &mut module.storage_layouts[3];
    row.size = 4;
    row.alignment = 4;
    let Kind::Pointer(pointer) = &mut row.kind else { unreachable!() };
    pointer.stored_bits = 32;
    with_owner(&module, |owner, budget| {
        assert!(lower_canonical_storage_module_v18(owner, Profile::Gfx942, budget).is_err())
    });
}

fn read_discriminant(result: u32, address: u32, alignment: u32) -> Operation {
    value(result, Type::Scalar(ScalarType::U128), OperationKind::Storage(Storage::ReadDiscriminant {
        address: ValueId(address),
        access: MemoryAccess::new(Space::Private, alignment),
    }))
}

#[test]
fn pointer_niche_reads_actual_stored_bits_in_every_admitted_encoding() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for space in [Space::Private, Space::Workgroup, Space::Global, Space::Constant, Space::Generic] {
            let llvm_space = super::super::storage_operations_v18::space_v18(space);
            let recipe = profile.pointer_encoding(llvm_space).unwrap();
            let bytes = u64::from(recipe.bits() / 8);
            let mut rows = rows();
            rows.push(Row { size: bytes, alignment: bytes as u32,
                kind: Kind::Pointer(StoragePointerV1 { pointee: Id(0), value_space: space,
                    encoded_space: space, access: AccessMode::ReadOnly, stored_bits: recipe.bits() }) });
            rows[2].alignment = bytes as u32;
            niche(&mut rows, Field { offset: 0, layout: Id(3) }, recipe.null_bits());
            let module = module(rows, vec![allocate(0, 2, Space::Private, bytes as u32),
                set(0, 1, bytes as u32), read_discriminant(1, 0, bytes as u32)], profile);
            let text = emit(&module, profile);
            assert!(text.contains(&format!("store i{} {},", recipe.bits(), recipe.null_bits())));
            assert_eq!(text.matches(&format!(".tag = load i{},", recipe.bits())).count(), 1);
            assert!(text.contains(&format!(".delta = sub i{} %storage.bb0.op2.tag, {}", recipe.bits(), recipe.null_bits())));
            assert!(text.contains("i128 1, i128 %storage"));
            assert!(!text.contains("inttoptr"));
        }
    }
}

fn llvm_amdgpu_help_lists_cpu(help: &str, cpu: &str) -> bool {
    let Some((_, cpus)) = help.split_once("Available CPUs for this target:") else {
        return false;
    };
    let Some((cpus, _)) = cpus.split_once("Available features for this target:") else {
        return false;
    };
    cpus.lines().any(|line| {
        let mut fields = line.split_whitespace();
        fields.next() == Some(cpu) && fields.next() == Some("-")
    })
}

#[test]
fn llvm_cpu_qualification_requires_an_exact_entry_in_the_cpu_section() {
    let help = "Available CPUs for this target:\n\n  gfx942 - Select the gfx942 processor.\n  gfx9500 - Select the gfx9500 processor.\n\nAvailable features for this target:\n  gfx950 - Not a CPU.\n";
    assert!(llvm_amdgpu_help_lists_cpu(help, "gfx942"));
    assert!(!llvm_amdgpu_help_lists_cpu(help, "gfx950"));
    assert!(!llvm_amdgpu_help_lists_cpu(help, "gfx94"));
    assert!(!llvm_amdgpu_help_lists_cpu("gfx950 - Select the gfx950 processor.", "gfx950"));
    assert!(!llvm_amdgpu_help_lists_cpu("'gfx950' is not a recognized processor for this target (ignoring processor)", "gfx950"));
}

#[test]
fn target_null_recipe_agrees_with_actual_llvm_amdgpu_address_space_conversion() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let supported = Command::new("llc")
        .args(["-mtriple=amdgcn-amd-amdhsa", "-mcpu=help"])
        .stdin(Stdio::null()).output()
        .expect("llc with both required AMDGPU CPUs is required for null-encoding qualification");
    assert!(supported.status.success(), "{}", String::from_utf8_lossy(&supported.stderr));
    let help = format!("{}\n{}", String::from_utf8(supported.stdout).unwrap(),
        String::from_utf8(supported.stderr).unwrap());
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        assert!(llvm_amdgpu_help_lists_cpu(&help, profile.cpu()),
            "llc does not list required CPU {}; this target is not qualified", profile.cpu());
    }
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let mut input = format!("target datalayout = \"{}\"\ntarget triple = \"amdgcn-amd-amdhsa\"\n",
            fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1);
        input.push_str("@null_0 = addrspace(1) global ptr null\n");
        for space in [1, 3, 4, 5] {
            input.push_str(&format!("@null_{space} = addrspace(1) global ptr addrspace({space}) addrspacecast (ptr null to ptr addrspace({space}))\n"));
        }
        let mut child = Command::new("llc")
            .args(["-mtriple=amdgcn-amd-amdhsa", "-mcpu", profile.cpu(), "-o", "-", "-"])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn().expect("llc is required to qualify AMDGPU pointer null encoding");
        let written = child.stdin.take().unwrap().write_all(input.as_bytes());
        let output = child.wait_with_output().expect("wait for AMDGPU null encoding");
        assert!(written.is_ok() && output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert!(output.stderr.is_empty(), "llc emitted diagnostics for required CPU {}: {}",
            profile.cpu(), String::from_utf8_lossy(&output.stderr));
        let assembly = String::from_utf8(output.stdout).unwrap();
        for space in [0, 1, 3, 4, 5] {
            let recipe = profile.pointer_encoding(space).unwrap();
            let section = assembly.split(&format!("null_{space}:")).nth(1).expect("actual emitted global");
            let directive = section.lines().map(str::trim)
                .find(|line| line.starts_with(".long") || line.starts_with(".quad") || line.starts_with(".zero"))
                .expect("actual pointer data directive");
            let fields = directive.split_whitespace().take(2).collect::<Vec<_>>();
            if recipe.bits() == 32 {
                assert_eq!(fields[0], ".long");
                assert!(fields[1] == "-1" || fields[1] == "4294967295", "{directive}");
                assert_eq!(recipe.null_bits(), u32::MAX as u128);
            } else {
                assert!(fields == [".quad", "0"] || fields == [".zero", "8"], "{directive}");
                assert_eq!(recipe.null_bits(), 0);
            }
        }
    }
}

#[test]
fn logical_discriminant_read_preserves_u128_bits_and_reloads_after_retag() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let mut rows = rows();
        let Kind::Variants { variants, .. } = &mut rows[2].kind else { unreachable!() };
        variants[0].discriminant = 1_u128 << 100;
        variants[1].discriminant = u128::MAX;
        let module = module(rows, vec![
            allocate(0, 2, Space::Private, 4),
            set(0, 0, 4),
            read_discriminant(1, 0, 4),
            set(0, 1, 4),
            read_discriminant(2, 0, 4),
        ], profile);
        let text = emit(&module, profile);
        assert_eq!(text.matches(".tag = load i32,").count(), 2);
        assert_eq!(text.matches("call void @llvm.trap()").count(), 2);
        assert!(text.contains(&format!("i128 {}, i128 0", 1_u128 << 100)));
        assert!(text.contains(&format!("i128 {}, i128 %storage", u128::MAX)));
        assert!(text.find("%v1 = add i128").unwrap() < text.find("store i32 20,").unwrap());
        assert!(text.find("store i32 20,").unwrap() < text.find("%v2 = add i128").unwrap());
        assert!(!text.contains("load [8 x i8]"));
        assert_eq!(text.matches(" = alloca ").count(), 1);
    }
}

#[test]
fn niche_discriminant_read_uses_wrapped_physical_bits_and_original_logical_map() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let mut rows = rows();
        rows[0] = Row { size: 1, alignment: 1, kind: Kind::Scalar(ScalarType::Bool) };
        niche(&mut rows, Field { offset: 0, layout: Id(0) }, 255);
        let Kind::Variants { variants, encoding } = &mut rows[2].kind else { unreachable!() };
        *variants = [
            Variant { discriminant: 0, direct_tag_bits: None, uninhabited: false, layout: Id(1) },
            Variant { discriminant: 1, direct_tag_bits: None, uninhabited: false, layout: Id(1) },
            Variant { discriminant: 2, direct_tag_bits: None, uninhabited: false, layout: Id(1) },
        ].into();
        let Encoding::Niche { last_niche_variant, .. } = encoding else { unreachable!() };
        *last_niche_variant = 2;
        let module = module(rows, vec![
            allocate(0, 2, Space::Private, 4),
            set(0, 2, 1),
            read_discriminant(1, 0, 1),
        ], profile);
        let text = emit(&module, profile);
        assert!(text.contains("store i8 0,"));
        assert!(text.contains(".delta = sub i8 %storage.bb0.op2.tag, 255"));
        assert!(text.contains("icmp ugt i8 %storage.bb0.op2.delta, 1"));
        assert!(text.contains("icmp eq i8 %storage.bb0.op2.delta, 0"));
        assert!(text.contains("icmp eq i8 %storage.bb0.op2.delta, 1"));
        assert!(text.contains("i128 2, i128 %storage"));
        assert_eq!(text.matches(".tag = load i8,").count(), 1);
        assert!(!text.contains("load i1,"));
    }
}

#[test]
fn discriminant_read_trap_continuation_is_the_real_phi_predecessor() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let mut module = module(rows(), vec![
            allocate(0, 2, Space::Private, 4),
            set(0, 1, 4),
            read_discriminant(1, 0, 4),
        ], profile);
        let body = module.functions[0].body.as_mut().unwrap();
        body.blocks[0].terminator = Some(Terminator::Branch { target: BlockId(1), arguments: vec![ValueId(1)] });
        let mut next = BasicBlock::new(BlockId(1));
        next.parameters.push(ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U128)));
        next.terminator = Some(Terminator::Return { values: vec![] });
        body.blocks.push(next);
        let text = emit(&module, profile);
        assert!(text.contains("phi i128 [ %v1, %storage_bb0_op2_continue ]"));
        assert_eq!(text.matches("call void @llvm.trap()").count(), 1);
    }
}
