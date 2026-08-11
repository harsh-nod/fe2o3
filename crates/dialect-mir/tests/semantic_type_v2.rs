#![allow(dead_code)]

#[path = "../src/semantic_type_v2.rs"]
mod semantic_type_v2;

use semantic_type_v2::*;

fn budgets() -> SemanticTypeGraphBudgetsV2 {
    SemanticTypeGraphBudgetsV2 {
        max_nodes: 128,
        max_edges: 512,
        max_fields: 512,
        max_variants: 128,
        max_validity_ranges: 128,
        max_name_bytes: 256,
        max_canonical_bytes: 64 * 1024,
        max_validation_work: 10_000,
    }
}

fn scalar(kind: SemanticScalarV2, bytes: u64) -> SemanticTypeNodeV2 {
    SemanticTypeNodeV2 {
        layout: SemanticTypeLayoutV2::sized(bytes, bytes),
        kind: SemanticTypeKindV2::Scalar(kind),
    }
}

fn validity_scalar(
    scalar: SemanticScalarV2,
    bytes: u64,
    valid_ranges: Vec<ScalarValidityRangeV2>,
) -> SemanticTypeNodeV2 {
    SemanticTypeNodeV2 {
        layout: SemanticTypeLayoutV2::sized(bytes, bytes),
        kind: SemanticTypeKindV2::ValidityScalar {
            scalar,
            valid_ranges,
        },
    }
}

fn first_field_niche_source() -> SemanticNicheSourceV2 {
    SemanticNicheSourceV2 {
        path: vec![SemanticNichePathComponentV2::Field(0)],
        expected_offset: 0,
    }
}

fn named(name: &str, offset: u64, ty: SemanticTypeNodeIdV2) -> SemanticFieldV2 {
    SemanticFieldV2 {
        name: Some(name.to_owned()),
        offset,
        ty,
    }
}

fn unnamed(offset: u64, ty: SemanticTypeNodeIdV2) -> SemanticFieldV2 {
    SemanticFieldV2 {
        name: None,
        offset,
        ty,
    }
}

fn build_recursive_list(pointer_first: bool) -> SemanticTypeGraphV2 {
    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let (pointer, list) = if pointer_first {
        let pointer = builder.declare("ptr<List>").unwrap();
        let list = builder.declare("crate::List").unwrap();
        (pointer, list)
    } else {
        let list = builder.declare("crate::List").unwrap();
        let pointer = builder.declare("ptr<List>").unwrap();
        (pointer, list)
    };
    builder
        .define(
            pointer,
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(8, 8),
                kind: SemanticTypeKindV2::RawPointer {
                    pointee: list,
                    mutability: SemanticMutabilityV2::Mutable,
                    address_space: 1,
                    data_pointer_bytes: 8,
                    metadata: PointerMetadataV2::None,
                },
            },
        )
        .unwrap();
    builder
        .define(
            list,
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(8, 8),
                kind: SemanticTypeKindV2::Struct {
                    identity: "crate::List".to_owned(),
                    fields: vec![named("next", 0, pointer)],
                },
            },
        )
        .unwrap();
    builder.finish(list).unwrap()
}

#[test]
fn recursive_pointer_graph_is_valid_and_order_independent() {
    let left = build_recursive_list(false);
    let right = build_recursive_list(true);
    assert_eq!(
        left.canonical_bytes().unwrap(),
        right.canonical_bytes().unwrap()
    );
    assert_eq!(
        left.untrusted_canonical_encoding().unwrap(),
        right.untrusted_canonical_encoding().unwrap()
    );
    assert_eq!(left.root_key(), "crate::List");
}

fn build_dst_family_in_order(order: &[&str]) -> SemanticTypeGraphV2 {
    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let mut ids = std::collections::BTreeMap::new();
    for key in order {
        ids.insert(*key, builder.declare(*key).unwrap());
    }
    let id = |key: &str| *ids.get(key).unwrap();
    builder
        .define(
            id("u8"),
            scalar(
                SemanticScalarV2::Int {
                    signed: false,
                    bits: 8,
                },
                1,
            ),
        )
        .unwrap();
    builder
        .define(
            id("u32"),
            scalar(
                SemanticScalarV2::Int {
                    signed: false,
                    bits: 32,
                },
                4,
            ),
        )
        .unwrap();
    builder
        .define(
            id("[u32]"),
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::dynamically_sized(4),
                kind: SemanticTypeKindV2::Slice { element: id("u32") },
            },
        )
        .unwrap();
    builder
        .define(
            id("Packet"),
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::dynamically_sized(4),
                kind: SemanticTypeKindV2::Struct {
                    identity: "Packet".into(),
                    fields: vec![
                        named("header", 0, id("u32")),
                        named("payload", 4, id("[u32]")),
                    ],
                },
            },
        )
        .unwrap();
    builder
        .define(
            id("*const Packet"),
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(16, 8),
                kind: SemanticTypeKindV2::RawPointer {
                    pointee: id("Packet"),
                    mutability: SemanticMutabilityV2::Immutable,
                    address_space: 0,
                    data_pointer_bytes: 8,
                    metadata: PointerMetadataV2::SliceLength,
                },
            },
        )
        .unwrap();
    builder
        .define(
            id("Root"),
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(24, 8),
                kind: SemanticTypeKindV2::Struct {
                    identity: "Root".into(),
                    fields: vec![
                        named("packet", 0, id("*const Packet")),
                        named("flag", 16, id("u8")),
                    ],
                },
            },
        )
        .unwrap();
    builder.finish(id("Root")).unwrap()
}

#[test]
fn every_declaration_permutation_has_one_untrusted_encoding() {
    let names = ["u8", "u32", "[u32]", "Packet", "*const Packet", "Root"];
    let expected = build_dst_family_in_order(&names).canonical_bytes().unwrap();
    let mut indices = [0_usize, 1, 2, 3, 4, 5];
    let mut checked = 0;
    loop {
        let order = indices.map(|index| names[index]);
        assert_eq!(
            build_dst_family_in_order(&order).canonical_bytes().unwrap(),
            expected
        );
        checked += 1;
        let Some(pivot) =
            (0..indices.len() - 1).rfind(|index| indices[*index] < indices[*index + 1])
        else {
            break;
        };
        let successor = (pivot + 1..indices.len())
            .rfind(|index| indices[*index] > indices[pivot])
            .unwrap();
        indices.swap(pivot, successor);
        indices[pivot + 1..].reverse();
    }
    assert_eq!(checked, 720);
}

#[test]
fn canonical_round_trip_preserves_every_byte() {
    let graph = build_recursive_list(false);
    let encoded = graph.canonical_bytes().unwrap();
    let decoded = SemanticTypeGraphV2::decode_canonical(&encoded, budgets()).unwrap();
    assert_eq!(decoded.canonical_bytes().unwrap(), encoded);
    assert_eq!(decoded.root_key(), "crate::List");
    assert_eq!(decoded.node_count(), 2);
}

#[test]
fn every_truncated_prefix_is_rejected() {
    let encoded = build_recursive_list(false).canonical_bytes().unwrap();
    for length in 0..encoded.len() {
        assert!(
            SemanticTypeGraphV2::decode_canonical(&encoded[..length], budgets()).is_err(),
            "accepted prefix {length}"
        );
    }
    assert!(SemanticTypeGraphV2::decode_canonical(&encoded, budgets()).is_ok());
}

#[test]
fn trailing_and_mutated_headers_are_rejected() {
    let encoded = build_recursive_list(false).canonical_bytes().unwrap();
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert!(SemanticTypeGraphV2::decode_canonical(&trailing, budgets()).is_err());

    let mut bad_magic = encoded.clone();
    bad_magic[0] ^= 1;
    assert!(SemanticTypeGraphV2::decode_canonical(&bad_magic, budgets()).is_err());

    let mut bad_version = encoded;
    let version = b"fe2o3.mir.semantic-type-graph".len();
    bad_version[version] = 3;
    assert!(SemanticTypeGraphV2::decode_canonical(&bad_version, budgets()).is_err());
}

#[test]
fn byte_flip_corpus_never_decodes_noncanonically() {
    let encoded =
        build_dst_family_in_order(&["Root", "Packet", "u8", "*const Packet", "[u32]", "u32"])
            .canonical_bytes()
            .unwrap();
    for index in 0..encoded.len() {
        let mut mutated = encoded.clone();
        mutated[index] ^= 1;
        if let Ok(graph) = SemanticTypeGraphV2::decode_canonical(&mutated, budgets()) {
            assert_eq!(graph.canonical_bytes().unwrap(), mutated, "byte {index}");
        }
    }
}

#[test]
fn direct_by_value_cycle_is_rejected() {
    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let left = builder.declare("Left").unwrap();
    let right = builder.declare("Right").unwrap();
    builder
        .define(
            left,
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(8, 8),
                kind: SemanticTypeKindV2::Struct {
                    identity: "Left".into(),
                    fields: vec![named("right", 0, right)],
                },
            },
        )
        .unwrap();
    builder
        .define(
            right,
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(8, 8),
                kind: SemanticTypeKindV2::Struct {
                    identity: "Right".into(),
                    fields: vec![named("left", 0, left)],
                },
            },
        )
        .unwrap();
    assert!(matches!(
        builder.finish(left),
        Err(SemanticTypeGraphErrorV2::ByValueCycle { .. })
    ));
}

#[test]
fn reference_cycle_is_valid_indirection() {
    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let structure = builder.declare("Borrowed").unwrap();
    let reference = builder.declare("&Borrowed").unwrap();
    builder
        .define(
            reference,
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(8, 8),
                kind: SemanticTypeKindV2::Reference {
                    referent: structure,
                    mutability: SemanticMutabilityV2::Immutable,
                    address_space: 0,
                    data_pointer_bytes: 8,
                    metadata: PointerMetadataV2::None,
                },
            },
        )
        .unwrap();
    builder
        .define(
            structure,
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(8, 8),
                kind: SemanticTypeKindV2::Struct {
                    identity: "Borrowed".into(),
                    fields: vec![named("this", 0, reference)],
                },
            },
        )
        .unwrap();
    builder.finish(structure).unwrap();
}

#[test]
fn slice_and_dst_tail_propagate_length_metadata() {
    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let u32_ty = builder
        .intern(
            "u32",
            scalar(
                SemanticScalarV2::Int {
                    signed: false,
                    bits: 32,
                },
                4,
            ),
        )
        .unwrap();
    let slice = builder
        .intern(
            "[u32]",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::dynamically_sized(4),
                kind: SemanticTypeKindV2::Slice { element: u32_ty },
            },
        )
        .unwrap();
    let packet = builder
        .intern(
            "Packet",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::dynamically_sized(4),
                kind: SemanticTypeKindV2::Struct {
                    identity: "Packet".into(),
                    fields: vec![named("header", 0, u32_ty), named("payload", 4, slice)],
                },
            },
        )
        .unwrap();
    let pointer = builder
        .intern(
            "*const Packet",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(16, 8),
                kind: SemanticTypeKindV2::RawPointer {
                    pointee: packet,
                    mutability: SemanticMutabilityV2::Immutable,
                    address_space: 0,
                    data_pointer_bytes: 8,
                    metadata: PointerMetadataV2::SliceLength,
                },
            },
        )
        .unwrap();
    let graph = builder.finish(pointer).unwrap();
    let bytes = graph.canonical_bytes().unwrap();
    SemanticTypeGraphV2::decode_canonical(&bytes, budgets()).unwrap();
}

#[test]
fn str_has_length_metadata_and_exact_alignment() {
    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let string = builder
        .intern(
            "str",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::dynamically_sized(1),
                kind: SemanticTypeKindV2::Str,
            },
        )
        .unwrap();
    let pointer = builder
        .intern(
            "*const str",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(16, 8),
                kind: SemanticTypeKindV2::RawPointer {
                    pointee: string,
                    mutability: SemanticMutabilityV2::Immutable,
                    address_space: 0,
                    data_pointer_bytes: 8,
                    metadata: PointerMetadataV2::SliceLength,
                },
            },
        )
        .unwrap();
    builder.finish(pointer).unwrap();
}

#[test]
fn wrong_dst_metadata_fails_closed() {
    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let string = builder
        .intern(
            "str",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::dynamically_sized(1),
                kind: SemanticTypeKindV2::Str,
            },
        )
        .unwrap();
    let pointer = builder
        .intern(
            "bad",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(8, 8),
                kind: SemanticTypeKindV2::RawPointer {
                    pointee: string,
                    mutability: SemanticMutabilityV2::Immutable,
                    address_space: 0,
                    data_pointer_bytes: 8,
                    metadata: PointerMetadataV2::None,
                },
            },
        )
        .unwrap();
    assert!(matches!(
        builder.finish(pointer),
        Err(SemanticTypeGraphErrorV2::Invalid { .. })
    ));
}

#[test]
fn opaque_dst_binds_vtable_identity() {
    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let dyn_ty = builder
        .intern(
            "dyn Trait",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::dynamically_sized(8),
                kind: SemanticTypeKindV2::OpaqueDst {
                    identity: "dyn Trait".into(),
                    metadata: PointerMetadataV2::VTable {
                        trait_identity: "crate::Trait".into(),
                    },
                },
            },
        )
        .unwrap();
    let pointer = builder
        .intern(
            "&dyn Trait",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(16, 8),
                kind: SemanticTypeKindV2::Reference {
                    referent: dyn_ty,
                    mutability: SemanticMutabilityV2::Immutable,
                    address_space: 0,
                    data_pointer_bytes: 8,
                    metadata: PointerMetadataV2::VTable {
                        trait_identity: "crate::Trait".into(),
                    },
                },
            },
        )
        .unwrap();
    builder.finish(pointer).unwrap();
}

#[test]
fn tuple_array_struct_and_union_layouts_validate() {
    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let u8_ty = builder
        .intern(
            "u8",
            scalar(
                SemanticScalarV2::Int {
                    signed: false,
                    bits: 8,
                },
                1,
            ),
        )
        .unwrap();
    let u64_ty = builder
        .intern(
            "u64",
            scalar(
                SemanticScalarV2::Int {
                    signed: false,
                    bits: 64,
                },
                8,
            ),
        )
        .unwrap();
    let tuple = builder
        .intern(
            "(u8,u64)",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(16, 8),
                kind: SemanticTypeKindV2::Tuple {
                    fields: vec![unnamed(0, u8_ty), unnamed(8, u64_ty)],
                },
            },
        )
        .unwrap();
    let array = builder
        .intern(
            "[u64;2]",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(16, 8),
                kind: SemanticTypeKindV2::Array {
                    element: u64_ty,
                    length: 2,
                },
            },
        )
        .unwrap();
    let union = builder
        .intern(
            "Word",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(8, 8),
                kind: SemanticTypeKindV2::Union {
                    identity: "Word".into(),
                    fields: vec![named("byte", 0, u8_ty), named("wide", 0, u64_ty)],
                },
            },
        )
        .unwrap();
    let root = builder
        .intern(
            "Root",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(40, 8),
                kind: SemanticTypeKindV2::Struct {
                    identity: "Root".into(),
                    fields: vec![
                        named("tuple", 0, tuple),
                        named("array", 16, array),
                        named("word", 32, union),
                    ],
                },
            },
        )
        .unwrap();
    builder.finish(root).unwrap();
}

#[test]
fn malformed_union_offset_is_rejected() {
    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let byte = builder
        .intern(
            "u8",
            scalar(
                SemanticScalarV2::Int {
                    signed: false,
                    bits: 8,
                },
                1,
            ),
        )
        .unwrap();
    let union = builder
        .intern(
            "BadUnion",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(2, 1),
                kind: SemanticTypeKindV2::Union {
                    identity: "BadUnion".into(),
                    fields: vec![named("byte", 1, byte)],
                },
            },
        )
        .unwrap();
    assert!(matches!(
        builder.finish(union),
        Err(SemanticTypeGraphErrorV2::Invalid { .. })
    ));
}

#[test]
fn direct_payload_enum_validates() {
    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let payload = builder
        .intern(
            "u32",
            scalar(
                SemanticScalarV2::Int {
                    signed: false,
                    bits: 32,
                },
                4,
            ),
        )
        .unwrap();
    let root = builder
        .intern(
            "Message",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(8, 4),
                kind: SemanticTypeKindV2::Enum {
                    identity: "Message".into(),
                    discriminant: SemanticScalarV2::Int {
                        signed: false,
                        bits: 8,
                    },
                    encoding: SemanticEnumEncodingV2::Direct {
                        tag_offset: 0,
                        tag: SemanticScalarV2::Int {
                            signed: false,
                            bits: 8,
                        },
                    },
                    variants: vec![
                        SemanticVariantV2 {
                            name: "Empty".into(),
                            discriminant: 0,
                            fields: vec![],
                        },
                        SemanticVariantV2 {
                            name: "Value".into(),
                            discriminant: 1,
                            fields: vec![named("value", 4, payload)],
                        },
                    ],
                },
            },
        )
        .unwrap();
    builder.finish(root).unwrap();
}

#[test]
fn direct_tag_payload_overlap_is_rejected() {
    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let byte = builder
        .intern(
            "u8",
            scalar(
                SemanticScalarV2::Int {
                    signed: false,
                    bits: 8,
                },
                1,
            ),
        )
        .unwrap();
    let root = builder
        .intern(
            "Bad",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(1, 1),
                kind: SemanticTypeKindV2::Enum {
                    identity: "Bad".into(),
                    discriminant: SemanticScalarV2::Int {
                        signed: false,
                        bits: 8,
                    },
                    encoding: SemanticEnumEncodingV2::Direct {
                        tag_offset: 0,
                        tag: SemanticScalarV2::Int {
                            signed: false,
                            bits: 8,
                        },
                    },
                    variants: vec![SemanticVariantV2 {
                        name: "A".into(),
                        discriminant: 0,
                        fields: vec![named("value", 0, byte)],
                    }],
                },
            },
        )
        .unwrap();
    assert!(matches!(
        builder.finish(root),
        Err(SemanticTypeGraphErrorV2::Invalid { .. })
    ));
}

#[test]
fn bounded_niche_validity_accepts_invalid_zero() {
    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let unit = builder
        .intern(
            "()",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(0, 1),
                kind: SemanticTypeKindV2::Unit,
            },
        )
        .unwrap();
    let nonnull = builder
        .intern(
            "NonNull",
            validity_scalar(
                SemanticScalarV2::Int {
                    signed: false,
                    bits: 64,
                },
                8,
                vec![ScalarValidityRangeV2 {
                    start: 1,
                    end: u64::MAX as u128,
                }],
            ),
        )
        .unwrap();
    let option = builder
        .intern(
            "Option<NonNull>",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(8, 8),
                kind: SemanticTypeKindV2::Enum {
                    identity: "Option<NonNull>".into(),
                    discriminant: SemanticScalarV2::Int {
                        signed: false,
                        bits: 8,
                    },
                    encoding: SemanticEnumEncodingV2::Niche {
                        source: first_field_niche_source(),
                        niche_scalar: SemanticScalarV2::Int {
                            signed: false,
                            bits: 64,
                        },
                        valid_ranges: vec![ScalarValidityRangeV2 {
                            start: 1,
                            end: u64::MAX as u128,
                        }],
                        untagged_variant: 1,
                        niche_variants_start: 0,
                        niche_variants_end: 0,
                        niche_start: 0,
                    },
                    variants: vec![
                        SemanticVariantV2 {
                            name: "None".into(),
                            discriminant: 0,
                            fields: vec![named("zst", 0, unit)],
                        },
                        SemanticVariantV2 {
                            name: "Some".into(),
                            discriminant: 1,
                            fields: vec![named("value", 0, nonnull)],
                        },
                    ],
                },
            },
        )
        .unwrap();
    builder.finish(option).unwrap();
}

#[test]
fn niche_overlap_and_range_order_are_rejected() {
    for ranges in [
        vec![ScalarValidityRangeV2 { start: 0, end: 10 }],
        vec![
            ScalarValidityRangeV2 { start: 20, end: 30 },
            ScalarValidityRangeV2 { start: 10, end: 15 },
        ],
    ] {
        let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
        let constrained = builder
            .intern(
                "constrained",
                validity_scalar(
                    SemanticScalarV2::Int {
                        signed: false,
                        bits: 8,
                    },
                    1,
                    ranges.clone(),
                ),
            )
            .unwrap();
        let root = builder
            .intern(
                "N",
                SemanticTypeNodeV2 {
                    layout: SemanticTypeLayoutV2::sized(1, 1),
                    kind: SemanticTypeKindV2::Enum {
                        identity: "N".into(),
                        discriminant: SemanticScalarV2::Int {
                            signed: false,
                            bits: 8,
                        },
                        encoding: SemanticEnumEncodingV2::Niche {
                            source: first_field_niche_source(),
                            niche_scalar: SemanticScalarV2::Int {
                                signed: false,
                                bits: 8,
                            },
                            valid_ranges: ranges,
                            untagged_variant: 1,
                            niche_variants_start: 0,
                            niche_variants_end: 0,
                            niche_start: 0,
                        },
                        variants: vec![
                            SemanticVariantV2 {
                                name: "A".into(),
                                discriminant: 0,
                                fields: vec![],
                            },
                            SemanticVariantV2 {
                                name: "B".into(),
                                discriminant: 1,
                                fields: vec![named("value", 0, constrained)],
                            },
                        ],
                    },
                },
            )
            .unwrap();
        assert!(matches!(
            builder.finish(root),
            Err(SemanticTypeGraphErrorV2::Invalid { .. })
        ));
    }
}

#[test]
fn uninhabited_states_are_explicit() {
    let mut never_builder = SemanticTypeGraphBuilderV2::new(budgets());
    let never = never_builder
        .intern(
            "!",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(0, 1),
                kind: SemanticTypeKindV2::Never,
            },
        )
        .unwrap();
    never_builder.finish(never).unwrap();

    let mut enum_builder = SemanticTypeGraphBuilderV2::new(budgets());
    let empty = enum_builder
        .intern(
            "Void",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(0, 1),
                kind: SemanticTypeKindV2::Enum {
                    identity: "Void".into(),
                    discriminant: SemanticScalarV2::Int {
                        signed: false,
                        bits: 8,
                    },
                    encoding: SemanticEnumEncodingV2::Uninhabited,
                    variants: vec![],
                },
            },
        )
        .unwrap();
    enum_builder.finish(empty).unwrap();
}

#[test]
fn array_and_field_arithmetic_overflow_fail_closed() {
    let mut array_builder = SemanticTypeGraphBuilderV2::new(budgets());
    let wide = array_builder
        .intern(
            "u128",
            scalar(
                SemanticScalarV2::Int {
                    signed: false,
                    bits: 128,
                },
                16,
            ),
        )
        .unwrap();
    let array = array_builder
        .intern(
            "Huge",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(0, 16),
                kind: SemanticTypeKindV2::Array {
                    element: wide,
                    length: u64::MAX,
                },
            },
        )
        .unwrap();
    assert!(matches!(
        array_builder.finish(array),
        Err(SemanticTypeGraphErrorV2::Invalid { .. })
    ));

    let mut field_builder = SemanticTypeGraphBuilderV2::new(budgets());
    let wide = field_builder
        .intern(
            "u128",
            scalar(
                SemanticScalarV2::Int {
                    signed: false,
                    bits: 128,
                },
                16,
            ),
        )
        .unwrap();
    let structure = field_builder
        .intern(
            "Overflow",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(16, 16),
                kind: SemanticTypeKindV2::Struct {
                    identity: "Overflow".into(),
                    fields: vec![named("field", u64::MAX - 7, wide)],
                },
            },
        )
        .unwrap();
    assert!(matches!(
        field_builder.finish(structure),
        Err(SemanticTypeGraphErrorV2::Invalid { .. })
    ));
}

#[test]
fn malformed_layouts_and_duplicate_definitions_fail_closed() {
    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let id = builder.declare("bad").unwrap();
    builder
        .define(
            id,
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(4, 3),
                kind: SemanticTypeKindV2::Scalar(SemanticScalarV2::Int {
                    signed: false,
                    bits: 32,
                }),
            },
        )
        .unwrap();
    assert!(matches!(
        builder.define(
            id,
            scalar(
                SemanticScalarV2::Int {
                    signed: false,
                    bits: 32
                },
                4
            )
        ),
        Err(SemanticTypeGraphErrorV2::DuplicateDefinition { .. })
    ));
    assert!(matches!(
        builder.finish(id),
        Err(SemanticTypeGraphErrorV2::Invalid { .. })
    ));
}

#[test]
fn undefined_and_unreachable_nodes_are_rejected() {
    let mut undefined = SemanticTypeGraphBuilderV2::new(budgets());
    let root = undefined.declare("root").unwrap();
    assert!(matches!(
        undefined.finish(root),
        Err(SemanticTypeGraphErrorV2::UndefinedNode { .. })
    ));

    let mut unreachable = SemanticTypeGraphBuilderV2::new(budgets());
    let root = unreachable
        .intern(
            "root",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(0, 1),
                kind: SemanticTypeKindV2::Unit,
            },
        )
        .unwrap();
    unreachable
        .intern(
            "hidden",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(0, 1),
                kind: SemanticTypeKindV2::Unit,
            },
        )
        .unwrap();
    assert!(matches!(
        unreachable.finish(root),
        Err(SemanticTypeGraphErrorV2::UnreachableNode { .. })
    ));
}

#[test]
fn node_name_and_canonical_byte_budgets_are_enforced() {
    let mut tiny = budgets();
    tiny.max_name_bytes = 3;
    let mut builder = SemanticTypeGraphBuilderV2::new(tiny);
    assert!(matches!(
        builder.declare("four"),
        Err(SemanticTypeGraphErrorV2::NameTooLong { .. })
    ));

    let mut tiny = budgets();
    tiny.max_canonical_bytes = 8;
    let mut builder = SemanticTypeGraphBuilderV2::new(tiny);
    let root = builder
        .intern(
            "u",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(0, 1),
                kind: SemanticTypeKindV2::Unit,
            },
        )
        .unwrap();
    assert!(matches!(
        builder.finish(root),
        Err(SemanticTypeGraphErrorV2::ResourceLimit {
            resource: "canonical bytes",
            ..
        })
    ));
}

#[test]
fn node_edge_field_variant_range_and_work_budgets_are_enforced() {
    let mut node_budget = budgets();
    node_budget.max_nodes = 1;
    let mut builder = SemanticTypeGraphBuilderV2::new(node_budget);
    builder.declare("a").unwrap();
    assert!(matches!(
        builder.declare("b"),
        Err(SemanticTypeGraphErrorV2::ResourceLimit {
            resource: "nodes",
            ..
        })
    ));

    let mut edge_budget = budgets();
    edge_budget.max_edges = 0;
    let mut builder = SemanticTypeGraphBuilderV2::new(edge_budget);
    let byte = builder
        .intern(
            "u8",
            scalar(
                SemanticScalarV2::Int {
                    signed: false,
                    bits: 8,
                },
                1,
            ),
        )
        .unwrap();
    let array = builder
        .intern(
            "[u8;1]",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(1, 1),
                kind: SemanticTypeKindV2::Array {
                    element: byte,
                    length: 1,
                },
            },
        )
        .unwrap();
    assert!(matches!(
        builder.finish(array),
        Err(SemanticTypeGraphErrorV2::ResourceLimit {
            resource: "edges",
            ..
        })
    ));

    let mut field_budget = budgets();
    field_budget.max_fields = 0;
    let mut builder = SemanticTypeGraphBuilderV2::new(field_budget);
    let byte = builder
        .intern(
            "u8",
            scalar(
                SemanticScalarV2::Int {
                    signed: false,
                    bits: 8,
                },
                1,
            ),
        )
        .unwrap();
    let structure = builder
        .intern(
            "S",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(1, 1),
                kind: SemanticTypeKindV2::Struct {
                    identity: "S".into(),
                    fields: vec![named("x", 0, byte)],
                },
            },
        )
        .unwrap();
    assert!(matches!(
        builder.finish(structure),
        Err(SemanticTypeGraphErrorV2::ResourceLimit {
            resource: "fields",
            ..
        })
    ));

    let mut variant_budget = budgets();
    variant_budget.max_variants = 0;
    let mut builder = SemanticTypeGraphBuilderV2::new(variant_budget);
    let enumeration = builder
        .intern(
            "E",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(0, 1),
                kind: SemanticTypeKindV2::Enum {
                    identity: "E".into(),
                    discriminant: SemanticScalarV2::Int {
                        signed: false,
                        bits: 8,
                    },
                    encoding: SemanticEnumEncodingV2::Single { variant: 0 },
                    variants: vec![SemanticVariantV2 {
                        name: "A".into(),
                        discriminant: 0,
                        fields: vec![],
                    }],
                },
            },
        )
        .unwrap();
    assert!(matches!(
        builder.finish(enumeration),
        Err(SemanticTypeGraphErrorV2::ResourceLimit {
            resource: "variants",
            ..
        })
    ));

    let mut range_budget = budgets();
    range_budget.max_validity_ranges = 0;
    let mut builder = SemanticTypeGraphBuilderV2::new(range_budget);
    let constrained = builder
        .intern(
            "constrained",
            validity_scalar(
                SemanticScalarV2::Int {
                    signed: false,
                    bits: 8,
                },
                1,
                vec![ScalarValidityRangeV2 { start: 1, end: 255 }],
            ),
        )
        .unwrap();
    let enumeration = builder
        .intern(
            "N",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(1, 1),
                kind: SemanticTypeKindV2::Enum {
                    identity: "N".into(),
                    discriminant: SemanticScalarV2::Int {
                        signed: false,
                        bits: 8,
                    },
                    encoding: SemanticEnumEncodingV2::Niche {
                        source: first_field_niche_source(),
                        niche_scalar: SemanticScalarV2::Int {
                            signed: false,
                            bits: 8,
                        },
                        valid_ranges: vec![ScalarValidityRangeV2 { start: 1, end: 255 }],
                        untagged_variant: 1,
                        niche_variants_start: 0,
                        niche_variants_end: 0,
                        niche_start: 0,
                    },
                    variants: vec![
                        SemanticVariantV2 {
                            name: "A".into(),
                            discriminant: 0,
                            fields: vec![],
                        },
                        SemanticVariantV2 {
                            name: "B".into(),
                            discriminant: 1,
                            fields: vec![named("value", 0, constrained)],
                        },
                    ],
                },
            },
        )
        .unwrap();
    assert!(matches!(
        builder.finish(enumeration),
        Err(SemanticTypeGraphErrorV2::ResourceLimit {
            resource: "validity ranges",
            ..
        })
    ));

    let mut work_budget = budgets();
    work_budget.max_validation_work = 0;
    let mut builder = SemanticTypeGraphBuilderV2::new(work_budget);
    let root = builder
        .intern(
            "root",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(0, 1),
                kind: SemanticTypeKindV2::Unit,
            },
        )
        .unwrap();
    assert!(matches!(
        builder.finish(root),
        Err(SemanticTypeGraphErrorV2::ResourceLimit {
            resource: "validation work",
            ..
        })
    ));
}

#[test]
fn decode_checks_input_budget_before_parsing() {
    let encoded = build_recursive_list(false).canonical_bytes().unwrap();
    let mut tiny = budgets();
    tiny.max_canonical_bytes = (encoded.len() - 1) as u32;
    assert!(matches!(
        SemanticTypeGraphV2::decode_canonical(&encoded, tiny),
        Err(SemanticTypeGraphErrorV2::ResourceLimit {
            resource: "canonical bytes",
            ..
        })
    ));
}

#[test]
fn forged_collection_counts_are_rejected_before_allocation() {
    let mut node_count = build_recursive_list(false).canonical_bytes().unwrap();
    let node_count_offset = b"fe2o3.mir.semantic-type-graph".len() + 2;
    node_count[node_count_offset..node_count_offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    let mut permissive = budgets();
    permissive.max_nodes = u32::MAX;
    assert!(matches!(
        SemanticTypeGraphV2::decode_canonical(&node_count, permissive),
        Err(SemanticTypeGraphErrorV2::Decode { .. })
    ));

    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let empty_struct = builder
        .intern(
            "S",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(0, 1),
                kind: SemanticTypeKindV2::Struct {
                    identity: "S".into(),
                    fields: vec![],
                },
            },
        )
        .unwrap();
    let mut field_count = builder
        .finish(empty_struct)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let field_count_offset = field_count.len() - 4;
    field_count[field_count_offset..].copy_from_slice(&u32::MAX.to_le_bytes());
    let mut permissive = budgets();
    permissive.max_fields = u32::MAX;
    permissive.max_edges = u32::MAX;
    assert!(matches!(
        SemanticTypeGraphV2::decode_canonical(&field_count, permissive),
        Err(SemanticTypeGraphErrorV2::Decode { .. })
    ));

    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let empty_enum = builder
        .intern(
            "Void",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(0, 1),
                kind: SemanticTypeKindV2::Enum {
                    identity: "Void".into(),
                    discriminant: SemanticScalarV2::Int {
                        signed: false,
                        bits: 8,
                    },
                    encoding: SemanticEnumEncodingV2::Uninhabited,
                    variants: vec![],
                },
            },
        )
        .unwrap();
    let mut variant_count = builder
        .finish(empty_enum)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let variant_count_offset = variant_count.len() - 4;
    variant_count[variant_count_offset..].copy_from_slice(&u32::MAX.to_le_bytes());
    let mut permissive = budgets();
    permissive.max_variants = u32::MAX;
    assert!(matches!(
        SemanticTypeGraphV2::decode_canonical(&variant_count, permissive),
        Err(SemanticTypeGraphErrorV2::Decode { .. })
    ));
}

fn build_nested_niche(
    source: SemanticNicheSourceV2,
    niche_scalar: SemanticScalarV2,
    niche_ranges: Vec<ScalarValidityRangeV2>,
    terminal_ranges: Vec<ScalarValidityRangeV2>,
) -> Result<SemanticTypeGraphV2, SemanticTypeGraphErrorV2> {
    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let unit = builder.intern(
        "unit",
        SemanticTypeNodeV2 {
            layout: SemanticTypeLayoutV2::sized(0, 1),
            kind: SemanticTypeKindV2::Unit,
        },
    )?;
    let terminal = builder.intern(
        "valid-u8",
        validity_scalar(
            SemanticScalarV2::Int {
                signed: false,
                bits: 8,
            },
            1,
            terminal_ranges,
        ),
    )?;
    let array = builder.intern(
        "[valid-u8;2]",
        SemanticTypeNodeV2 {
            layout: SemanticTypeLayoutV2::sized(2, 1),
            kind: SemanticTypeKindV2::Array {
                element: terminal,
                length: 2,
            },
        },
    )?;
    let tuple = builder.intern(
        "([valid-u8;2],)",
        SemanticTypeNodeV2 {
            layout: SemanticTypeLayoutV2::sized(2, 1),
            kind: SemanticTypeKindV2::Tuple {
                fields: vec![unnamed(0, array)],
            },
        },
    )?;
    let wrapper = builder.intern(
        "Wrapper",
        SemanticTypeNodeV2 {
            layout: SemanticTypeLayoutV2::sized(2, 1),
            kind: SemanticTypeKindV2::Struct {
                identity: "crate::Wrapper".into(),
                fields: vec![named("values", 0, tuple)],
            },
        },
    )?;
    let root = builder.intern(
        "NestedOption",
        SemanticTypeNodeV2 {
            layout: SemanticTypeLayoutV2::sized(4, 1),
            kind: SemanticTypeKindV2::Enum {
                identity: "crate::NestedOption".into(),
                discriminant: SemanticScalarV2::Int {
                    signed: false,
                    bits: 8,
                },
                encoding: SemanticEnumEncodingV2::Niche {
                    source,
                    niche_scalar,
                    valid_ranges: niche_ranges,
                    untagged_variant: 1,
                    niche_variants_start: 0,
                    niche_variants_end: 0,
                    niche_start: 0,
                },
                variants: vec![
                    SemanticVariantV2 {
                        name: "None".into(),
                        discriminant: 0,
                        fields: vec![named("zst", 0, unit)],
                    },
                    SemanticVariantV2 {
                        name: "Some".into(),
                        discriminant: 1,
                        fields: vec![named("wrapper", 2, wrapper)],
                    },
                ],
            },
        },
    )?;
    builder.finish(root)
}

#[derive(Clone, Copy)]
enum UnionNichePath {
    Direct,
    NestedStruct,
    NestedArray,
}

fn build_union_niche(
    path_kind: UnionNichePath,
) -> Result<SemanticTypeGraphV2, SemanticTypeGraphErrorV2> {
    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let unit = builder.intern(
        "unit",
        SemanticTypeNodeV2 {
            layout: SemanticTypeLayoutV2::sized(0, 1),
            kind: SemanticTypeKindV2::Unit,
        },
    )?;
    let nonzero = builder.intern(
        "NonZeroU8",
        validity_scalar(
            SemanticScalarV2::Int {
                signed: false,
                bits: 8,
            },
            1,
            nonzero_u8_ranges(),
        ),
    )?;
    let unrestricted = builder.intern(
        "u8",
        scalar(
            SemanticScalarV2::Int {
                signed: false,
                bits: 8,
            },
            1,
        ),
    )?;
    let union = builder.intern(
        "NonZeroOrByte",
        SemanticTypeNodeV2 {
            layout: SemanticTypeLayoutV2::sized(1, 1),
            kind: SemanticTypeKindV2::Union {
                identity: "NonZeroOrByte".into(),
                fields: vec![
                    named("nonzero", 0, nonzero),
                    named("unrestricted", 0, unrestricted),
                ],
            },
        },
    )?;
    let (payload, path, expected_offset, size) = match path_kind {
        UnionNichePath::Direct => (
            union,
            vec![
                SemanticNichePathComponentV2::Field(0),
                SemanticNichePathComponentV2::Field(0),
            ],
            0,
            1,
        ),
        UnionNichePath::NestedStruct => {
            let wrapper = builder.intern(
                "UnionWrapper",
                SemanticTypeNodeV2 {
                    layout: SemanticTypeLayoutV2::sized(1, 1),
                    kind: SemanticTypeKindV2::Struct {
                        identity: "UnionWrapper".into(),
                        fields: vec![named("inner", 0, union)],
                    },
                },
            )?;
            (
                wrapper,
                vec![
                    SemanticNichePathComponentV2::Field(0),
                    SemanticNichePathComponentV2::Field(0),
                    SemanticNichePathComponentV2::Field(0),
                ],
                0,
                1,
            )
        }
        UnionNichePath::NestedArray => {
            let array = builder.intern(
                "[NonZeroOrByte;2]",
                SemanticTypeNodeV2 {
                    layout: SemanticTypeLayoutV2::sized(2, 1),
                    kind: SemanticTypeKindV2::Array {
                        element: union,
                        length: 2,
                    },
                },
            )?;
            (
                array,
                vec![
                    SemanticNichePathComponentV2::Field(0),
                    SemanticNichePathComponentV2::ArrayElement(1),
                    SemanticNichePathComponentV2::Field(0),
                ],
                1,
                2,
            )
        }
    };
    let root = builder.intern(
        "OptionUnionPayload",
        SemanticTypeNodeV2 {
            layout: SemanticTypeLayoutV2::sized(size, 1),
            kind: SemanticTypeKindV2::Enum {
                identity: "OptionUnionPayload".into(),
                discriminant: SemanticScalarV2::Int {
                    signed: false,
                    bits: 8,
                },
                encoding: SemanticEnumEncodingV2::Niche {
                    source: SemanticNicheSourceV2 {
                        path,
                        expected_offset,
                    },
                    niche_scalar: SemanticScalarV2::Int {
                        signed: false,
                        bits: 8,
                    },
                    valid_ranges: nonzero_u8_ranges(),
                    untagged_variant: 1,
                    niche_variants_start: 0,
                    niche_variants_end: 0,
                    niche_start: 0,
                },
                variants: vec![
                    SemanticVariantV2 {
                        name: "None".into(),
                        discriminant: 0,
                        fields: vec![named("zst", 0, unit)],
                    },
                    SemanticVariantV2 {
                        name: "Some".into(),
                        discriminant: 1,
                        fields: vec![named("payload", 0, payload)],
                    },
                ],
            },
        },
    )?;
    builder.finish(root)
}

fn nested_source() -> SemanticNicheSourceV2 {
    SemanticNicheSourceV2 {
        path: vec![
            SemanticNichePathComponentV2::Field(0),
            SemanticNichePathComponentV2::Field(0),
            SemanticNichePathComponentV2::Field(0),
            SemanticNichePathComponentV2::ArrayElement(1),
        ],
        expected_offset: 3,
    }
}

fn nonzero_u8_ranges() -> Vec<ScalarValidityRangeV2> {
    vec![ScalarValidityRangeV2 { start: 1, end: 255 }]
}

fn build_niche_partition(
    variant_count: u32,
    untagged_variant: u32,
    niche_variants_start: u32,
    niche_variants_end: u32,
    valid_start: u128,
) -> Result<SemanticTypeGraphV2, SemanticTypeGraphErrorV2> {
    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let payload = builder.intern(
        "NichePayload",
        validity_scalar(
            SemanticScalarV2::Int {
                signed: false,
                bits: 8,
            },
            1,
            vec![ScalarValidityRangeV2 {
                start: valid_start,
                end: 255,
            }],
        ),
    )?;
    let variants = (0..variant_count)
        .map(|index| SemanticVariantV2 {
            name: format!("V{index}"),
            discriminant: u128::from(index),
            fields: if index == untagged_variant {
                vec![named("payload", 0, payload)]
            } else {
                vec![]
            },
        })
        .collect();
    let root = builder.intern(
        "PartitionedNiche",
        SemanticTypeNodeV2 {
            layout: SemanticTypeLayoutV2::sized(1, 1),
            kind: SemanticTypeKindV2::Enum {
                identity: "PartitionedNiche".into(),
                discriminant: SemanticScalarV2::Int {
                    signed: false,
                    bits: 8,
                },
                encoding: SemanticEnumEncodingV2::Niche {
                    source: first_field_niche_source(),
                    niche_scalar: SemanticScalarV2::Int {
                        signed: false,
                        bits: 8,
                    },
                    valid_ranges: vec![ScalarValidityRangeV2 {
                        start: valid_start,
                        end: 255,
                    }],
                    untagged_variant,
                    niche_variants_start,
                    niche_variants_end,
                    niche_start: 0,
                },
                variants,
            },
        },
    )?;
    builder.finish(root)
}

fn build_direct_partition(
    tag_bits: u16,
    payload_offset: u64,
) -> Result<SemanticTypeGraphV2, SemanticTypeGraphErrorV2> {
    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let payload = builder.intern(
        "u8",
        scalar(
            SemanticScalarV2::Int {
                signed: false,
                bits: 8,
            },
            1,
        ),
    )?;
    let root = builder.intern(
        "DirectControl",
        SemanticTypeNodeV2 {
            layout: SemanticTypeLayoutV2::sized(2, 1),
            kind: SemanticTypeKindV2::Enum {
                identity: "DirectControl".into(),
                discriminant: SemanticScalarV2::Int {
                    signed: false,
                    bits: 8,
                },
                encoding: SemanticEnumEncodingV2::Direct {
                    tag_offset: 0,
                    tag: SemanticScalarV2::Int {
                        signed: false,
                        bits: tag_bits,
                    },
                },
                variants: (0_u32..3)
                    .map(|index| SemanticVariantV2 {
                        name: format!("V{index}"),
                        discriminant: u128::from(index),
                        fields: vec![named("payload", payload_offset, payload)],
                    })
                    .collect(),
            },
        },
    )?;
    builder.finish(root)
}

#[test]
fn niche_source_is_derived_through_nested_payload_layout() {
    let graph = build_nested_niche(
        nested_source(),
        SemanticScalarV2::Int {
            signed: false,
            bits: 8,
        },
        nonzero_u8_ranges(),
        nonzero_u8_ranges(),
    )
    .unwrap();
    let encoded = graph.canonical_bytes().unwrap();
    SemanticTypeGraphV2::decode_canonical(&encoded, budgets()).unwrap();
}

#[test]
fn niche_encoding_rejects_an_inhabited_omitted_third_variant() {
    let expected = SemanticTypeGraphErrorV2::Invalid {
        key: "PartitionedNiche".into(),
        reason: "niche encoding must cover every non-untagged variant exactly once".into(),
    };
    assert_eq!(build_niche_partition(3, 1, 0, 0, 1).unwrap_err(), expected);
}

#[test]
fn niche_partition_boundaries_are_exact_and_gapless() {
    for (variant_count, untagged, start, end) in [(2, 1, 0, 0), (3, 2, 0, 1), (3, 0, 1, 2)] {
        build_niche_partition(variant_count, untagged, start, end, 128).unwrap();
    }

    for (variant_count, untagged, start, end) in [(3, 2, 0, 3), (3, 2, 2, 1), (3, 1, 0, 1)] {
        assert!(matches!(
            build_niche_partition(variant_count, untagged, start, end, 128),
            Err(SemanticTypeGraphErrorV2::Invalid { .. })
        ));
    }
}

#[test]
fn niche_partition_rejects_holes_and_multiple_omitted_variants() {
    let expected = SemanticTypeGraphErrorV2::Invalid {
        key: "PartitionedNiche".into(),
        reason: "niche encoding must cover every non-untagged variant exactly once".into(),
    };
    for arguments in [(4, 0, 1, 2), (6, 0, 1, 1), (6, 5, 3, 4)] {
        assert_eq!(
            build_niche_partition(arguments.0, arguments.1, arguments.2, arguments.3, 128,)
                .unwrap_err(),
            expected
        );
    }
}

#[test]
fn direct_tagged_encoding_controls_are_unchanged() {
    build_direct_partition(8, 1).unwrap();
    assert!(matches!(
        build_direct_partition(1, 1),
        Err(SemanticTypeGraphErrorV2::Invalid { .. })
    ));
    assert!(matches!(
        build_direct_partition(8, 0),
        Err(SemanticTypeGraphErrorV2::Invalid { .. })
    ));
}

#[test]
fn union_fields_cannot_supply_niche_validity() {
    let expected = SemanticTypeGraphErrorV2::Invalid {
        key: "OptionUnionPayload".into(),
        reason: "niche source path cannot traverse a union without authenticated active-field or union-wide validity evidence".into(),
    };
    for path in [
        UnionNichePath::Direct,
        UnionNichePath::NestedStruct,
        UnionNichePath::NestedArray,
    ] {
        assert_eq!(build_union_niche(path).unwrap_err(), expected);
        assert_eq!(build_union_niche(path).unwrap_err(), expected);
    }
}

#[test]
fn fifty_thousand_niche_path_oracle_cases_include_unions() {
    let scalar = SemanticScalarV2::Int {
        signed: false,
        bits: 8,
    };
    for case in 0..50_000_u32 {
        match case % 4 {
            0 => assert!(matches!(
                build_union_niche(UnionNichePath::Direct),
                Err(SemanticTypeGraphErrorV2::Invalid { .. })
            )),
            1 => assert!(matches!(
                build_union_niche(UnionNichePath::NestedStruct),
                Err(SemanticTypeGraphErrorV2::Invalid { .. })
            )),
            2 => assert!(matches!(
                build_union_niche(UnionNichePath::NestedArray),
                Err(SemanticTypeGraphErrorV2::Invalid { .. })
            )),
            _ => {
                let graph = build_nested_niche(
                    nested_source(),
                    scalar,
                    nonzero_u8_ranges(),
                    nonzero_u8_ranges(),
                )
                .unwrap();
                let encoded = graph.canonical_bytes().unwrap();
                SemanticTypeGraphV2::decode_canonical(&encoded, budgets()).unwrap();
            }
        }
    }
}

#[test]
fn niche_source_rejects_padding_missing_fields_offsets_scalars_and_ranges() {
    let scalar = SemanticScalarV2::Int {
        signed: false,
        bits: 8,
    };
    let bad_sources = [
        SemanticNicheSourceV2 {
            path: vec![SemanticNichePathComponentV2::Field(1)],
            expected_offset: 3,
        },
        SemanticNicheSourceV2 {
            path: nested_source().path,
            expected_offset: 2,
        },
        SemanticNicheSourceV2 {
            path: vec![
                SemanticNichePathComponentV2::Field(0),
                SemanticNichePathComponentV2::Field(0),
                SemanticNichePathComponentV2::Field(0),
                SemanticNichePathComponentV2::ArrayElement(2),
            ],
            expected_offset: 4,
        },
        SemanticNicheSourceV2 {
            path: vec![],
            expected_offset: 0,
        },
    ];
    for source in bad_sources {
        assert!(matches!(
            build_nested_niche(source, scalar, nonzero_u8_ranges(), nonzero_u8_ranges()),
            Err(SemanticTypeGraphErrorV2::Invalid { .. })
        ));
    }
    assert!(matches!(
        build_nested_niche(
            nested_source(),
            SemanticScalarV2::Int {
                signed: false,
                bits: 16,
            },
            nonzero_u8_ranges(),
            nonzero_u8_ranges()
        ),
        Err(SemanticTypeGraphErrorV2::Invalid { .. })
    ));
    assert!(matches!(
        build_nested_niche(
            nested_source(),
            scalar,
            vec![ScalarValidityRangeV2 { start: 2, end: 255 }],
            nonzero_u8_ranges()
        ),
        Err(SemanticTypeGraphErrorV2::Invalid { .. })
    ));
}

#[test]
fn counterfeit_padding_niche_without_a_terminal_scalar_is_rejected() {
    let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
    let root = builder
        .intern(
            "Counterfeit",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(8, 8),
                kind: SemanticTypeKindV2::Enum {
                    identity: "crate::Counterfeit".into(),
                    discriminant: SemanticScalarV2::Int {
                        signed: false,
                        bits: 8,
                    },
                    encoding: SemanticEnumEncodingV2::Niche {
                        source: SemanticNicheSourceV2 {
                            path: vec![SemanticNichePathComponentV2::Field(0)],
                            expected_offset: 0,
                        },
                        niche_scalar: SemanticScalarV2::Int {
                            signed: false,
                            bits: 64,
                        },
                        valid_ranges: vec![ScalarValidityRangeV2 {
                            start: 1,
                            end: u64::MAX as u128,
                        }],
                        untagged_variant: 1,
                        niche_variants_start: 0,
                        niche_variants_end: 0,
                        niche_start: 0,
                    },
                    variants: vec![
                        SemanticVariantV2 {
                            name: "None".into(),
                            discriminant: 0,
                            fields: vec![],
                        },
                        SemanticVariantV2 {
                            name: "Some".into(),
                            discriminant: 1,
                            fields: vec![],
                        },
                    ],
                },
            },
        )
        .unwrap();
    assert!(matches!(
        builder.finish(root),
        Err(SemanticTypeGraphErrorV2::Invalid { .. })
    ));
}

#[test]
fn nominal_and_exact_definition_duplicates_are_rejected() {
    let mut nominal = SemanticTypeGraphBuilderV2::new(budgets());
    let byte = nominal
        .intern(
            "u8",
            scalar(
                SemanticScalarV2::Int {
                    signed: false,
                    bits: 8,
                },
                1,
            ),
        )
        .unwrap();
    let left = nominal
        .intern(
            "left",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(1, 1),
                kind: SemanticTypeKindV2::Struct {
                    identity: "crate::Same".into(),
                    fields: vec![named("value", 0, byte)],
                },
            },
        )
        .unwrap();
    let right = nominal
        .intern(
            "right",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(1, 1),
                kind: SemanticTypeKindV2::Union {
                    identity: "crate::Same".into(),
                    fields: vec![named("value", 0, byte)],
                },
            },
        )
        .unwrap();
    let root = nominal
        .intern(
            "root",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(2, 1),
                kind: SemanticTypeKindV2::Tuple {
                    fields: vec![unnamed(0, left), unnamed(1, right)],
                },
            },
        )
        .unwrap();
    assert!(matches!(
        nominal.finish(root),
        Err(SemanticTypeGraphErrorV2::Invalid { .. })
    ));

    let mut exact = SemanticTypeGraphBuilderV2::new(budgets());
    let first = exact
        .intern(
            "first",
            scalar(
                SemanticScalarV2::Int {
                    signed: false,
                    bits: 16,
                },
                2,
            ),
        )
        .unwrap();
    let second = exact
        .intern(
            "second",
            scalar(
                SemanticScalarV2::Int {
                    signed: false,
                    bits: 16,
                },
                2,
            ),
        )
        .unwrap();
    let root = exact
        .intern(
            "root",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(4, 2),
                kind: SemanticTypeKindV2::Tuple {
                    fields: vec![unnamed(0, first), unnamed(2, second)],
                },
            },
        )
        .unwrap();
    assert!(matches!(
        exact.finish(root),
        Err(SemanticTypeGraphErrorV2::Invalid { .. })
    ));
}

#[test]
fn caller_keys_are_explicitly_part_of_untrusted_canonicalization() {
    fn graph(key: &str) -> SemanticTypeGraphV2 {
        let mut builder = SemanticTypeGraphBuilderV2::new(budgets());
        let root = builder
            .intern(
                key,
                SemanticTypeNodeV2 {
                    layout: SemanticTypeLayoutV2::sized(0, 1),
                    kind: SemanticTypeKindV2::Unit,
                },
            )
            .unwrap();
        builder.finish(root).unwrap()
    }
    let left = graph("caller-key-a");
    let right = graph("caller-key-b");
    assert_ne!(
        left.canonical_bytes().unwrap(),
        right.canonical_bytes().unwrap()
    );
    assert_eq!(
        left.untrusted_canonical_encoding().unwrap().as_bytes(),
        left.canonical_bytes().unwrap()
    );
}

#[test]
fn decoder_charges_all_edges_and_niche_paths_before_allocation() {
    let encoded = build_nested_niche(
        nested_source(),
        SemanticScalarV2::Int {
            signed: false,
            bits: 8,
        },
        nonzero_u8_ranges(),
        nonzero_u8_ranges(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let mut constrained = budgets();
    constrained.max_edges = 6;
    assert!(matches!(
        SemanticTypeGraphV2::decode_canonical(&encoded, constrained),
        Err(SemanticTypeGraphErrorV2::ResourceLimit {
            resource: "edges",
            ..
        })
    ));
}

#[test]
fn hundred_thousand_malformed_inputs_never_panic_or_decode_noncanonically() {
    let seed = build_nested_niche(
        nested_source(),
        SemanticScalarV2::Int {
            signed: false,
            bits: 8,
        },
        nonzero_u8_ranges(),
        nonzero_u8_ranges(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let mut state = 0x8b5a_2d17_c4e3_91f0_u64;
    for iteration in 0..100_000_u32 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let mut input = seed.clone();
        let index = (state as usize) % input.len();
        input[index] ^= ((state >> 32) as u8).wrapping_add(iteration as u8 | 1);
        if iteration % 7 == 0 {
            input.truncate((state as usize) % (input.len() + 1));
        }
        if let Ok(graph) = SemanticTypeGraphV2::decode_canonical(&input, budgets()) {
            assert_eq!(graph.canonical_bytes().unwrap(), input);
        }
    }
}

#[test]
fn validation_work_budget_stops_deep_reference_amplifier() {
    let limited = SemanticTypeGraphBudgetsV2 {
        max_nodes: 2_000,
        max_edges: 2_000,
        max_fields: 0,
        max_variants: 0,
        max_validity_ranges: 0,
        max_name_bytes: 64,
        max_canonical_bytes: 1_000_000,
        max_validation_work: 100,
    };
    let mut builder = SemanticTypeGraphBuilderV2::new(limited);
    let mut current = builder
        .intern(
            "base",
            SemanticTypeNodeV2 {
                layout: SemanticTypeLayoutV2::sized(0, 1),
                kind: SemanticTypeKindV2::Unit,
            },
        )
        .unwrap();
    for index in 0..1_500 {
        current = builder
            .intern(
                format!("ptr-{index:04}"),
                SemanticTypeNodeV2 {
                    layout: SemanticTypeLayoutV2::sized(8, 8),
                    kind: SemanticTypeKindV2::Reference {
                        referent: current,
                        mutability: SemanticMutabilityV2::Immutable,
                        address_space: 0,
                        data_pointer_bytes: 8,
                        metadata: PointerMetadataV2::None,
                    },
                },
            )
            .unwrap();
    }
    assert!(matches!(
        builder.finish(current),
        Err(SemanticTypeGraphErrorV2::ResourceLimit {
            resource: "validation work",
            ..
        })
    ));
}
