use super::super::storage_native_v18::tests::*;
use super::*;
use fe2o3_kernel_ir::{StorageCopyOverlapV1, VerifiedCanonicalKernelIrModuleV18 as Owner};

pub(super) fn inline_type(id: u32) -> Type {
    Type::pointer(object(id), Space::Constant, AccessMode::ReadOnly)
}

pub(super) fn record_rows(alignment: u32) -> Vec<Row> {
    vec![
        scalar(),
        Row {
            size: if alignment == 64 { 64 } else { 24 },
            alignment,
            kind: Kind::Record(
                vec![
                    Field {
                        offset: 0,
                        layout: Id(0),
                    },
                    Field {
                        offset: 16,
                        layout: Id(0),
                    },
                ]
                .into_boxed_slice(),
            ),
        },
    ]
}

pub(super) fn root_module(rows: Vec<Row>, schema: u32, profile: Profile, mixed: bool) -> Module {
    let alignment = rows[schema as usize].alignment;
    let source = if mixed { 1 } else { 0 };
    let operations = vec![
        allocate(20, schema, Space::Private, alignment),
        Operation::new(
            vec![],
            OperationKind::Storage(Storage::CopyObject {
                source: ValueId(source),
                destination: ValueId(20),
                source_access: MemoryAccess::new(Space::Constant, alignment),
                destination_access: MemoryAccess::new(Space::Private, alignment),
                overlap: StorageCopyOverlapV1::NonOverlapping,
            }),
        ),
    ];
    let mut result = module(rows, operations, profile);
    let parameters = if mixed {
        vec![
            Type::Scalar(ScalarType::U8),
            inline_type(schema),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                Space::Global,
                AccessMode::ReadWrite,
            ),
            Type::slice(
                Type::Scalar(ScalarType::U32),
                Space::Global,
                AccessMode::ReadOnly,
            ),
            Type::Scalar(ScalarType::Index),
        ]
    } else {
        vec![inline_type(schema)]
    };
    result.functions[0].body.as_mut().unwrap().parameters = (0..parameters.len())
        .map(|index| ValueId(index as u32))
        .collect();
    result.functions[0].signature.parameters = parameters;
    bind(&mut result, profile);
    result
}

pub(super) fn with_roles<T>(
    owner: &Owner,
    roles: &[RootParameterRoleV29],
    consume: impl FnOnce(&RootRolesV29<'_>) -> T,
) -> T {
    let kernel = &owner.module().kernels[0];
    let ordinal = owner
        .module()
        .functions
        .iter()
        .position(|function| function.id == kernel.entry)
        .unwrap();
    let rows = [RootKernelRolesV29 {
        kernel: &kernel.id,
        entry: &kernel.entry,
        entry_function_ordinal: ordinal,
        parameters: roles,
    }];
    consume(&RootRolesV29 { roots: &rows })
}

fn tool(name: &str, arguments: &[&str], input: &str) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let version = Command::new(name)
        .arg("--version")
        .output()
        .unwrap_or_else(|error| panic!("required pinned LLVM22 tool {name}: {error}"));
    assert!(
        version.status.success()
            && String::from_utf8_lossy(&version.stdout).contains("LLVM version 22."),
        "{name} is not the required LLVM22 tool: {}{}",
        String::from_utf8_lossy(&version.stdout),
        String::from_utf8_lossy(&version.stderr)
    );
    let mut child = Command::new(name)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("required LLVM tool");
    let written = child.stdin.take().unwrap().write_all(input.as_bytes());
    let output = child.wait_with_output().expect("reap LLVM tool");
    assert!(
        written.is_ok() && output.status.success(),
        "{name} rejected full module: {}\n{input}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn emit_roles(module: &Module, profile: Profile, roles: &[RootParameterRoleV29]) -> String {
    let text = with_owner(module, |owner, budget| {
        with_roles(owner, roles, |roles| {
            lower_canonical_storage_module_with_root_roles_v29(owner, profile, roles, budget)
                .unwrap()
        })
    });
    check_llvm(&text, profile);
    text
}

fn check_llvm(text: &str, profile: Profile) {
    tool("opt", &["-passes=verify", "-disable-output", "-"], text);
    tool(
        "llc",
        &[
            "-",
            "-mtriple=amdgcn-amd-amdhsa",
            "-mcpu",
            profile.cpu(),
            "-filetype=null",
            "-o",
            "/dev/null",
        ],
        text,
    );
}

#[test]
fn actual_inline_records_arrays_and_alignment_reach_both_llvm_targets_without_host_pointers() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for case in 0..3 {
            let rows = if case == 2 {
                vec![
                    scalar(),
                    Row {
                        size: 24,
                        alignment: 4,
                        kind: Kind::Array {
                            element: Id(0),
                            length: 6,
                            stride: 4,
                        },
                    },
                ]
            } else {
                record_rows(if case == 1 { 64 } else { 8 })
            };
            let size = rows[1].size;
            let alignment = rows[1].alignment;
            let mut module = root_module(rows, 1, profile, true);
            let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
            let step = if case == 2 {
                operations.push(value(
                    21,
                    Type::Scalar(ScalarType::Index),
                    OperationKind::Constant(Constant::Index(1)),
                ));
                Projection::ArrayIndex(ValueId(21))
            } else {
                Projection::Field(0)
            };
            operations.push(project(22, 0, 20, step, Space::Private));
            operations.push(read(
                23,
                22,
                Type::Scalar(ScalarType::U32),
                Space::Private,
                4,
            ));
            operations.push(Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(2),
                    value: ValueId(23),
                    access: MemoryAccess::new(Space::Global, 4),
                },
            ));
            bind(&mut module, profile);
            let roles = [
                RootParameterRoleV29::Existing,
                RootParameterRoleV29::InlineObject(Id(1)),
                RootParameterRoleV29::Existing,
                RootParameterRoleV29::Existing,
                RootParameterRoleV29::Existing,
            ];
            let text = emit_roles(&module, profile, &roles);
            assert!(text.contains(&format!(
                "ptr addrspace(4) byref([{size} x i8]) align {alignment} %arg1"
            )));
            assert!(text.contains("i8 %arg0"));
            assert!(text.contains("ptr addrspace(1) %arg2"));
            assert!(text.contains("ptr addrspace(1) %arg3.data, i64 %arg3.len"));
            assert!(text.contains("i64 %arg4"));
            assert!(text.contains("@llvm.memcpy.p5.p4.i64"));
            assert!(text.contains("load i32") && text.contains("store i32"));
            assert_eq!(text.matches(" byref(").count(), 1);
            assert!(!text.contains("byval(") && !text.contains("inttoptr"));
            with_owner(&module, |owner, budget| {
                assert!(lower_canonical_storage_module_v18(owner, profile, budget).is_err());
            });
        }
    }
}

fn variant_rows(pointer: bool, niche: bool) -> Vec<Row> {
    let mut rows = vec![
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
            alignment: if pointer { 8 } else { 4 },
            kind: Kind::Variants {
                encoding: if niche {
                    Encoding::Niche {
                        tag: Field {
                            offset: 0,
                            layout: Id(if pointer { 3 } else { 0 }),
                        },
                        untagged_variant: 0,
                        first_niche_variant: 1,
                        last_niche_variant: 1,
                        niche_start: 0,
                    }
                } else {
                    Encoding::Direct {
                        tag: Field {
                            offset: 0,
                            layout: Id(0),
                        },
                    }
                },
                variants: (0..2)
                    .map(|index| Variant {
                        discriminant: index as u128,
                        direct_tag_bits: (!niche).then_some(index as u128),
                        uninhabited: false,
                        layout: Id(1),
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            },
        },
    ];
    if pointer {
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
    }
    rows
}

#[test]
fn incoming_direct_integer_niche_and_pointer_niche_tags_are_read_only_and_keep_the_same_copy_schema()
 {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for (pointer, niche) in [(false, false), (false, true), (true, true)] {
            let rows = variant_rows(pointer, niche);
            let alignment = rows[2].alignment;
            let mut module = root_module(rows, 2, profile, false);
            module.functions[0].body.as_mut().unwrap().blocks[0]
                .operations
                .push(value(
                    21,
                    Type::Scalar(ScalarType::U128),
                    OperationKind::Storage(Storage::ReadDiscriminant {
                        address: ValueId(0),
                        access: MemoryAccess::new(Space::Constant, alignment),
                    }),
                ));
            bind(&mut module, profile);
            let text = emit_roles(
                &module,
                profile,
                &[RootParameterRoleV29::InlineObject(Id(2))],
            );
            assert!(text.contains(&format!("byref([8 x i8]) align {alignment}")));
            assert!(text.contains("@llvm.memcpy.p5.p4.i64"));
            assert!(text.contains(".tag = load "));
            assert!(!text.contains("inttoptr") && !text.contains("store i64 0,"));
        }
    }
}

pub(super) fn multiple_module(profile: Profile) -> Module {
    let mut base = root_module(record_rows(8), 1, profile, false);
    let function = base.functions[0].clone();
    let kernel = base.kernels[0].clone();
    base.functions.clear();
    base.kernels.clear();
    for name in ["d", "a", "c", "b"] {
        let mut function = function.clone();
        function.id = FunctionId::from(format!("{name}_entry"));
        let mut kernel = kernel.clone();
        kernel.id = KernelId::from(name);
        kernel.entry = function.id.clone();
        base.functions.push(function);
        base.kernels.push(kernel);
    }
    base
}

pub(super) fn multiple_roles<'a>(
    owner: &'a Owner,
    parameters: &'a [RootParameterRoleV29],
) -> Vec<RootKernelRolesV29<'a>> {
    let mut rows: Vec<_> = owner
        .module()
        .kernels
        .iter()
        .map(|kernel| RootKernelRolesV29 {
            kernel: &kernel.id,
            entry: &kernel.entry,
            entry_function_ordinal: owner
                .module()
                .functions
                .iter()
                .position(|f| f.id == kernel.entry)
                .unwrap(),
            parameters,
        })
        .collect();
    rows.sort_by(|left, right| left.kernel.cmp(right.kernel));
    rows
}

#[test]
fn complete_roles_use_checked_ordinals_without_sorting_or_rebinding_original_owner_vectors() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let module = multiple_module(profile);
        let text = with_owner(&module, |owner, budget| {
            let parameters = [RootParameterRoleV29::InlineObject(Id(1))];
            let rows = multiple_roles(owner, &parameters);
            assert_eq!(
                rows.iter()
                    .map(|row| row.kernel.as_str())
                    .collect::<Vec<_>>(),
                ["a", "b", "c", "d"]
            );
            assert_eq!(
                owner
                    .module()
                    .kernels
                    .iter()
                    .map(|row| row.id.as_str())
                    .collect::<Vec<_>>(),
                ["d", "a", "c", "b"]
            );
            lower_canonical_storage_module_with_root_roles_v29(
                owner,
                profile,
                &RootRolesV29 { roots: &rows },
                budget,
            )
            .unwrap()
        });
        assert_eq!(text.matches("byref([24 x i8]) align 8").count(), 4);
        check_llvm(&text, profile);
    }
}

#[test]
fn repeated_equal_shapes_keep_distinct_actual_schema_ids() {
    let mut rows = record_rows(8);
    rows.push(rows[1].clone());
    let mut module = root_module(rows, 1, Profile::Gfx942, false);
    module.functions[0]
        .signature
        .parameters
        .push(inline_type(2));
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(1));
    let text = emit_roles(
        &module,
        Profile::Gfx942,
        &[
            RootParameterRoleV29::InlineObject(Id(1)),
            RootParameterRoleV29::InlineObject(Id(2)),
        ],
    );
    assert_eq!(text.matches("byref([24 x i8]) align 8").count(), 2);
    with_owner(&module, |owner, budget| {
        with_roles(
            owner,
            &[
                RootParameterRoleV29::InlineObject(Id(1)),
                RootParameterRoleV29::InlineObject(Id(1)),
            ],
            |roles| {
                assert!(
                    lower_canonical_storage_module_with_root_roles_v29(
                        owner,
                        Profile::Gfx942,
                        roles,
                        budget
                    )
                    .is_err()
                );
            },
        )
    });
}

#[test]
fn malformed_complete_rosters_never_authorize_missing_duplicate_reordered_or_substituted_roots() {
    let module = multiple_module(Profile::Gfx942);
    with_owner(&module, |owner, budget| {
        let parameters = [RootParameterRoleV29::InlineObject(Id(1))];
        let empty: [RootParameterRoleV29; 0] = [];
        let extra = [
            RootParameterRoleV29::InlineObject(Id(1)),
            RootParameterRoleV29::Existing,
        ];
        let wrong_id = KernelId::from("not_an_actual_kernel");
        let wrong_entry = FunctionId::from("not_an_actual_entry");
        for change in 0..10 {
            let mut rows = multiple_roles(owner, &parameters);
            match change {
                0 => {
                    rows.pop();
                }
                1 => rows.swap(0, 1),
                2 => rows[1].kernel = rows[0].kernel,
                3 => rows[3].kernel = &wrong_id,
                4 => rows[0].entry = &wrong_entry,
                5 => rows[0].entry_function_ordinal = usize::MAX,
                6 => rows[0].entry_function_ordinal = rows[1].entry_function_ordinal,
                7 => rows[0].parameters = &empty,
                8 => rows[0].parameters = &extra,
                9 => rows.push(RootKernelRolesV29 {
                    kernel: &wrong_id,
                    entry: &wrong_entry,
                    entry_function_ordinal: 0,
                    parameters: &parameters,
                }),
                _ => unreachable!(),
            }
            assert!(
                lower_canonical_storage_module_with_root_roles_v29(
                    owner,
                    Profile::Gfx942,
                    &RootRolesV29 { roots: &rows },
                    budget
                )
                .is_err(),
                "change {change}"
            );
        }
    });
}

#[test]
fn root_roles_do_not_turn_arbitrary_pointer_spaces_access_or_pointee_types_into_inline_objects() {
    for ty in [
        Type::pointer(object(1), Space::Private, AccessMode::ReadOnly),
        Type::pointer(object(1), Space::Generic, AccessMode::ReadOnly),
        Type::pointer(object(1), Space::Workgroup, AccessMode::ReadOnly),
        Type::pointer(object(1), Space::Global, AccessMode::ReadOnly),
        Type::pointer(object(1), Space::Constant, AccessMode::ReadWrite),
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            Space::Constant,
            AccessMode::ReadOnly,
        ),
        Type::slice(
            Type::Scalar(ScalarType::U32),
            Space::Constant,
            AccessMode::ReadOnly,
        ),
        Type::Scalar(ScalarType::U64),
    ] {
        let mut module = root_module(record_rows(8), 1, Profile::Gfx942, false);
        module.functions[0].signature.parameters[0] = ty;
        module.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .clear();
        bind(&mut module, Profile::Gfx942);
        with_owner(&module, |owner, budget| {
            with_roles(
                owner,
                &[RootParameterRoleV29::InlineObject(Id(1))],
                |roles| {
                    assert!(
                        lower_canonical_storage_module_with_root_roles_v29(
                            owner,
                            Profile::Gfx942,
                            roles,
                            budget
                        )
                        .is_err()
                    );
                },
            )
        });
    }
    let module = root_module(record_rows(8), 1, Profile::Gfx942, false);
    with_owner(&module, |owner, budget| {
        for role in [
            RootParameterRoleV29::Existing,
            RootParameterRoleV29::InlineObject(Id(0)),
            RootParameterRoleV29::InlineObject(Id(u32::MAX)),
        ] {
            with_roles(owner, &[role], |roles| {
                assert!(
                    lower_canonical_storage_module_with_root_roles_v29(
                        owner,
                        Profile::Gfx942,
                        roles,
                        budget
                    )
                    .is_err()
                );
            });
        }
    });
}

#[test]
fn ordinary_root_roles_preserve_the_exact_existing_llvm_module() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let mut module = module(vec![scalar()], vec![], profile);
        module.functions[0].signature.parameters = vec![
            Type::Scalar(ScalarType::U32),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                Space::Global,
                AccessMode::ReadWrite,
            ),
            Type::slice(
                Type::Scalar(ScalarType::U32),
                Space::Global,
                AccessMode::ReadOnly,
            ),
        ];
        module.functions[0].body.as_mut().unwrap().parameters =
            vec![ValueId(0), ValueId(1), ValueId(2)];
        let (old, new) = with_owner(&module, |owner, budget| {
            let old = lower_canonical_storage_module_v18(owner, profile, budget).unwrap();
            let new = with_roles(owner, &[RootParameterRoleV29::Existing; 3], |roles| {
                lower_canonical_storage_module_with_root_roles_v29(owner, profile, roles, budget)
                    .unwrap()
            });
            (old, new)
        });
        assert_eq!(old, new);
        assert!(!new.contains("byref("));
        check_llvm(&new, profile);
    }
}

#[test]
fn inline_entry_preserves_the_original_exact_target_profile_refusal() {
    let module = root_module(record_rows(8), 1, Profile::Gfx942, false);
    with_owner(&module, |owner, budget| {
        with_roles(
            owner,
            &[RootParameterRoleV29::InlineObject(Id(1))],
            |roles| {
                assert!(
                    lower_canonical_storage_module_with_root_roles_v29(
                        owner,
                        Profile::Gfx950,
                        roles,
                        budget,
                    )
                    .is_err()
                );
            },
        );
    });
}
