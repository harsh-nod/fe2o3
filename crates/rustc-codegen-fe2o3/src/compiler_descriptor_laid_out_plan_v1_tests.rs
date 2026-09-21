use super::super::{KernelBindingIdV1, SemanticTypeIdentityV1, TypedArgumentListV1};
use super::*;
use fe2o3_artifacts::{
    RustDisjointIndexSpaceV1, RustLayoutEvidenceV1, RustPhysicalComponentKindV1 as ComponentKind,
    RustPhysicalComponentV1 as Component, RustPointerMutabilityV1 as Mutability,
    RustTypeEvidenceV1,
};

fn argument(kind: Kind, index: u8) -> Argument {
    let (layout, access) = match kind {
        Kind::CompilerLaidOutByValue | Kind::CompilerLaidOutUsize | Kind::CompilerLaidOutIsize => {
            (None, AccessMode::ByValue)
        }
        Kind::Scalar(scalar) => {
            let scalar = match scalar {
                ScalarTypeV1::U8 => Scalar::U8,
                ScalarTypeV1::U16 => Scalar::U16,
                ScalarTypeV1::U32 => Scalar::U32,
                ScalarTypeV1::U64 => Scalar::U64,
                _ => panic!("fixture scalar"),
            };
            let size = scalar.size_bytes();
            (
                Some(
                    RustLayoutEvidenceV1::new(
                        RustTypeEvidenceV1::new(Shape::scalar(scalar)),
                        RustcAbiClassV1::Scalar,
                        PointerWidth::Bits64,
                        size,
                        size as u32,
                        vec![
                            Component::new(0, size, size as u32, ComponentKind::Scalar { scalar })
                                .unwrap(),
                        ],
                    )
                    .unwrap(),
                ),
                AccessMode::ByValue,
            )
        }
        Kind::SharedSlice(ScalarTypeV1::U32)
        | Kind::DisjointSlice(ScalarTypeV1::U32)
        | Kind::GlobalMutPointer(ScalarTypeV1::U32) => {
            let mutable = !matches!(kind, Kind::SharedSlice(_));
            let pointer = matches!(kind, Kind::GlobalMutPointer(_));
            let shape = if pointer {
                Shape::global_mut_pointer(Scalar::U32)
            } else if mutable {
                Shape::disjoint_slice(Scalar::U32, RustDisjointIndexSpaceV1::Index1D)
            } else {
                Shape::shared_slice(Scalar::U32)
            };
            let mut components = vec![
                Component::new(
                    0,
                    8,
                    8,
                    ComponentKind::Pointer {
                        mutability: if mutable {
                            Mutability::Mut
                        } else {
                            Mutability::Const
                        },
                        pointee: Scalar::U32,
                    },
                )
                .unwrap(),
            ];
            if !pointer {
                components.push(Component::new(8, 8, 8, ComponentKind::Usize).unwrap());
            }
            (
                Some(
                    RustLayoutEvidenceV1::new(
                        RustTypeEvidenceV1::new(shape),
                        if pointer {
                            RustcAbiClassV1::Scalar
                        } else {
                            RustcAbiClassV1::ScalarPair
                        },
                        PointerWidth::Bits64,
                        if pointer { 8 } else { 16 },
                        8,
                        components,
                    )
                    .unwrap(),
                ),
                if mutable {
                    AccessMode::ReadWrite
                } else {
                    AccessMode::ReadOnly
                },
            )
        }
        _ => panic!("fixture pointer element"),
    };
    Argument {
        name: format!("arg{index}"),
        kind,
        access,
        offset: 0,
        source_size: layout.as_ref().map_or(8, RustLayoutEvidenceV1::size),
        source_alignment: layout
            .as_ref()
            .map_or(8, RustLayoutEvidenceV1::abi_alignment),
        rustc_abi_class: layout
            .as_ref()
            .map_or(RustcAbiClassV1::Scalar, RustLayoutEvidenceV1::abi_class),
        layout,
        semantic_type_identity: SemanticTypeIdentityV1::from_sha256([index + 1; 32]),
    }
}

fn root(arguments: Vec<Argument>, extent: Extent) -> Root {
    Root {
        logical_name: "packing".to_owned(),
        export_name: "packing_entry".to_owned(),
        kernel_binding: KernelBindingIdV1::from_bytes([9; 32]),
        arguments: TypedArgumentListV1::new(arguments).unwrap(),
        explicit_argument_bytes: extent.bytes,
        kernarg_alignment_bytes: extent.alignment,
        source_launch: None,
    }
}

fn mismatch_is(error: CompilerDescriptorError, reason: &'static str) {
    assert!(
        matches!(error, CompilerDescriptorError::ProductionDescriptorMismatch(actual) if actual == reason)
    );
}

#[test]
fn laid_out_capture_reuses_complete_rows_with_exact_mixed_offsets() {
    let mut rows = [
        Kind::Scalar(ScalarTypeV1::U8),
        Kind::CompilerLaidOutUsize,
        Kind::SharedSlice(ScalarTypeV1::U32),
        Kind::Scalar(ScalarTypeV1::U16),
        Kind::CompilerLaidOutIsize,
        Kind::GlobalMutPointer(ScalarTypeV1::U32),
        Kind::DisjointSlice(ScalarTypeV1::U32),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, kind)| argument(kind, i as u8))
    .collect::<Vec<_>>();
    let before = rows.clone();
    let allocation = (rows.as_ptr(), rows.len(), rows.capacity());
    let extent = capture(&mut rows).unwrap().unwrap();
    assert_eq!(
        extent,
        Extent {
            bytes: 72,
            alignment: 8
        }
    );
    assert_eq!((rows.as_ptr(), rows.len(), rows.capacity()), allocation);
    for ((actual, expected), offset) in rows.iter().zip(&before).zip([0, 8, 16, 32, 40, 48, 56]) {
        let mut original = expected.clone();
        original.offset = offset;
        assert_eq!(actual, &original);
    }
    let retained = root(rows, extent);
    check(&retained).unwrap();
    assert_eq!(retained.arguments.as_slice().as_ptr(), allocation.0);
}

#[test]
fn laid_out_capture_handles_nominal_arguments_at_each_position() {
    for (kinds, offsets) in [
        (
            [
                Kind::CompilerLaidOutUsize,
                Kind::Scalar(ScalarTypeV1::U8),
                Kind::SharedSlice(ScalarTypeV1::U32),
            ],
            [0, 8, 16],
        ),
        (
            [
                Kind::Scalar(ScalarTypeV1::U8),
                Kind::CompilerLaidOutIsize,
                Kind::SharedSlice(ScalarTypeV1::U32),
            ],
            [0, 8, 16],
        ),
        (
            [
                Kind::Scalar(ScalarTypeV1::U8),
                Kind::SharedSlice(ScalarTypeV1::U32),
                Kind::CompilerLaidOutUsize,
            ],
            [0, 8, 24],
        ),
    ] {
        let mut rows = kinds
            .into_iter()
            .enumerate()
            .map(|(i, k)| argument(k, i as u8))
            .collect::<Vec<_>>();
        let extent = capture(&mut rows).unwrap().unwrap();
        assert_eq!(
            extent,
            Extent {
                bytes: 32,
                alignment: 8
            }
        );
        for (row, offset) in rows.iter().zip(offsets) {
            assert_eq!(row.offset, offset);
        }
        check(&root(rows, extent)).unwrap();
    }
}

#[test]
fn laid_out_capture_preserves_old_fixed_and_aggregate_deferred_roots() {
    for kinds in [
        vec![Kind::Scalar(ScalarTypeV1::U64)],
        vec![Kind::CompilerLaidOutUsize, Kind::CompilerLaidOutByValue],
        vec![Kind::CompilerLaidOutByValue],
    ] {
        let mut rows = kinds
            .into_iter()
            .enumerate()
            .map(|(i, k)| argument(k, i as u8))
            .collect::<Vec<_>>();
        let before = rows.clone();
        assert_eq!(capture(&mut rows).unwrap(), None);
        assert_eq!(rows, before);
        check(&root(
            rows,
            Extent {
                bytes: 0,
                alignment: 1,
            },
        ))
        .unwrap();
    }
}

#[test]
fn laid_out_capture_refuses_physical_mutants_before_any_offset_write() {
    for case in 0..8 {
        let mut rows = vec![
            argument(Kind::Scalar(ScalarTypeV1::U8), 0),
            argument(Kind::CompilerLaidOutUsize, 1),
        ];
        match case {
            0 => rows[1].source_size = 16,
            1 => rows[1].source_alignment = 4,
            2 => rows[1].rustc_abi_class = RustcAbiClassV1::Aggregate,
            3 => rows[1].access = AccessMode::ReadOnly,
            4 => rows[1].layout = argument(Kind::Scalar(ScalarTypeV1::U64), 1).layout,
            5 => rows[0].layout = None,
            6 => rows[0].layout = argument(Kind::Scalar(ScalarTypeV1::U16), 0).layout,
            _ => rows[0].access = AccessMode::WriteOnly,
        }
        let before = rows.clone();
        assert!(matches!(
            capture(&mut rows),
            Err(CompilerDescriptorError::ProductionDescriptorMismatch(_))
        ));
        assert_eq!(rows, before);
    }
}

#[test]
fn laid_out_checker_refuses_changed_offsets_and_root_extents() {
    for case in 0..3 {
        let mut rows = vec![
            argument(Kind::Scalar(ScalarTypeV1::U8), 0),
            argument(Kind::CompilerLaidOutIsize, 1),
        ];
        let extent = capture(&mut rows).unwrap().unwrap();
        if case == 0 {
            rows[1].offset = 0;
        }
        let mut retained = root(rows, extent);
        if case == 1 {
            retained.explicit_argument_bytes += 8;
        }
        if case == 2 {
            retained.kernarg_alignment_bytes = 4;
        }
        mismatch_is(
            check(&retained).unwrap_err(),
            if case == 0 {
                "retained whole-root argument offset"
            } else {
                "retained whole-root packing extent"
            },
        );
    }
}

#[test]
fn laid_out_capture_checks_the_actual_bounded_argument_roster() {
    let mut rows = (0..MAX_ABI_FIELDS)
        .map(|i| {
            argument(
                if i == 0 {
                    Kind::CompilerLaidOutUsize
                } else {
                    Kind::SharedSlice(ScalarTypeV1::U32)
                },
                i as u8,
            )
        })
        .collect::<Vec<_>>();
    let extent = capture(&mut rows).unwrap().unwrap();
    assert_eq!(
        extent,
        Extent {
            bytes: 1016,
            alignment: 8
        }
    );
    assert_eq!(rows.last().unwrap().offset, 1000);
    check(&root(rows.clone(), extent)).unwrap();
    rows.push(argument(Kind::CompilerLaidOutIsize, 64));
    let before = rows.clone();
    mismatch_is(
        capture(&mut rows).unwrap_err(),
        "scalar packing actual argument count",
    );
    assert_eq!(rows, before);
    let mut cursor = Cursor {
        end: u64::MAX,
        alignment: 8,
    };
    mismatch_is(
        cursor.advance(8, 8).unwrap_err(),
        "scalar packing offset arithmetic",
    );
    let mut cursor = Cursor {
        end: MAX_ABI_BYTES,
        alignment: 8,
    };
    mismatch_is(
        cursor.advance(8, 8).unwrap_err(),
        "scalar packing explicit byte limit",
    );
}

#[test]
fn empty_argument_roster_keeps_fixed_abi_without_nominal_packing() {
    assert_eq!(capture(&mut []).unwrap(), None);
    let retained = root(
        Vec::new(),
        Extent {
            bytes: 0,
            alignment: 1,
        },
    );
    check(&retained).unwrap();
    assert!(retained.arguments.as_slice().is_empty());
    assert_eq!(retained.explicit_argument_bytes, 0);
    assert_eq!(retained.kernarg_alignment_bytes, 1);
}

#[test]
fn laid_out_nominal_capture_does_not_admit_descriptor_or_semantic_substitution() {
    for kind in [
        Kind::CompilerLaidOutByValue,
        Kind::CompilerLaidOutUsize,
        Kind::CompilerLaidOutIsize,
    ] {
        assert!(kind.is_compiler_laid_out());
        for scalar in [
            fe2o3_kernel_ir::ScalarType::U64,
            fe2o3_kernel_ir::ScalarType::I64,
            fe2o3_kernel_ir::ScalarType::Index,
        ] {
            assert!(
                !super::super::production_descriptor_argument_matches_kernel_type_v1(
                    kind,
                    AccessMode::ByValue,
                    &fe2o3_kernel_ir::Type::Scalar(scalar)
                )
            );
        }
        let row = argument(kind, 0);
        mismatch_is(
            super::super::require_production_descriptor_argument_semantic_type_v1(
                &row,
                SemanticTypeIdentityV1::from_sha256([2; 32]),
            )
            .unwrap_err(),
            "rustc semantic argument type identity",
        );
    }
    assert_ne!(Kind::CompilerLaidOutUsize, Kind::CompilerLaidOutIsize);
    assert_ne!(Kind::CompilerLaidOutUsize, Kind::Scalar(ScalarTypeV1::U64));
}

#[test]
fn laid_out_nominal_variants_preserve_exact_old_retained_layouts() {
    use crate::rust_type_layout_v3::{
        GeneralTypedArgumentKindV3, GeneralTypedArgumentV3, GeneralTypedKernelContractV3,
    };
    use std::mem::{align_of, size_of};
    // Layout-only copies of the exact predecessor declarations, never authority or allocations.
    #[allow(dead_code)]
    enum OldGeneralKind {
        Scalar(Scalar),
        SharedSlice(Scalar),
        WriteOnlyDisjointSlice(Scalar),
        DisjointSlice(Scalar),
        GlobalMutPointer(Scalar),
        CompilerLaidOutByValue,
    }
    #[allow(dead_code)]
    struct OldGeneralArgument {
        kind: OldGeneralKind,
        layout: Option<RustLayoutEvidenceV1>,
        size: u64,
        alignment: u32,
        abi_class: RustcAbiClassV1,
    }
    #[allow(dead_code)]
    struct OldContract {
        arguments: Vec<OldGeneralArgument>,
        abi: fe2o3_artifacts::AbiLayout,
        launch: fe2o3_artifacts::LaunchContract,
    }
    #[allow(dead_code)]
    enum OldDescriptorKind {
        SharedSlice(ScalarTypeV1),
        DisjointSlice(ScalarTypeV1),
        GlobalMutPointer(ScalarTypeV1),
        Scalar(ScalarTypeV1),
        CompilerLaidOutByValue,
    }
    #[allow(dead_code)]
    struct OldArgument {
        name: String,
        kind: OldDescriptorKind,
        access: AccessMode,
        offset: u32,
        layout: Option<RustLayoutEvidenceV1>,
        source_size: u64,
        source_alignment: u32,
        rustc_abi_class: RustcAbiClassV1,
        semantic_type_identity: SemanticTypeIdentityV1,
    }
    #[allow(dead_code)]
    struct OldRoot {
        logical_name: String,
        export_name: String,
        kernel_binding: KernelBindingIdV1,
        arguments: TypedArgumentListV1<OldArgument>,
        explicit_argument_bytes: u32,
        kernarg_alignment_bytes: u32,
        source_launch: Option<fe2o3_artifacts::LaunchContract>,
    }
    macro_rules! same {
        ($old:ty, $new:ty) => {
            assert_eq!(
                (size_of::<$old>(), align_of::<$old>()),
                (size_of::<$new>(), align_of::<$new>())
            );
        };
    }
    same!(OldGeneralKind, GeneralTypedArgumentKindV3);
    same!(OldGeneralArgument, GeneralTypedArgumentV3);
    same!(OldContract, GeneralTypedKernelContractV3);
    same!(OldDescriptorKind, Kind);
    same!(OldArgument, Argument);
    same!(OldRoot, Root);
}
