use dialect_mir::{
    MirAddressSpace, MirAggregateLayout, MirEnumEncoding, MirEnumType, MirField, MirLayout,
    MirMutability, MirPadding, MirScalarType, MirSemanticType, MirStructType, MirTypeKind,
    MirVariant,
};

fn scalar(scalar: MirScalarType, size: u64, align: u64) -> MirSemanticType {
    MirSemanticType {
        layout: MirLayout::sized(size, align),
        kind: MirTypeKind::Scalar(scalar),
    }
}

fn u8_ty() -> MirSemanticType {
    scalar(
        MirScalarType::Int {
            signed: false,
            bits: 8,
        },
        1,
        1,
    )
}

fn u16_ty() -> MirSemanticType {
    scalar(
        MirScalarType::Int {
            signed: false,
            bits: 16,
        },
        2,
        2,
    )
}

fn u32_ty() -> MirSemanticType {
    scalar(
        MirScalarType::Int {
            signed: false,
            bits: 32,
        },
        4,
        4,
    )
}

fn empty_aggregate(size: u64) -> MirAggregateLayout {
    MirAggregateLayout {
        fields: Vec::new(),
        padding: (size != 0)
            .then_some(MirPadding { offset: 0, size })
            .into_iter()
            .collect(),
    }
}

#[test]
fn canonical_text_is_stable_and_preserves_rust_field_order() {
    let ty = MirSemanticType {
        layout: MirLayout::sized(8, 4),
        kind: MirTypeKind::Struct(MirStructType {
            identity: "demo::Reordered".into(),
            aggregate: MirAggregateLayout {
                fields: vec![
                    MirField {
                        name: Some("small".into()),
                        offset: 4,
                        ty: u8_ty(),
                    },
                    MirField {
                        name: Some("wide".into()),
                        offset: 0,
                        ty: u32_ty(),
                    },
                ],
                padding: vec![MirPadding { offset: 5, size: 3 }],
            },
        }),
    };

    assert_eq!(
        ty.canonical_text().unwrap(),
        "mir.type.v1{layout(size=8;align=4);kind=struct(name=15:demo::Reordered;aggregate(fields=[field(name=5:small;offset=4;type={layout(size=1;align=1);kind=scalar(u8)}),field(name=4:wide;offset=0;type={layout(size=4;align=4);kind=scalar(u32)})];padding=[5+3]))}"
    );
    assert_eq!(ty.canonical_text(), ty.clone().canonical_text());
}

#[test]
fn validates_recursive_pointer_slice_tuple_and_array_types() {
    let slice = MirSemanticType {
        layout: MirLayout::dynamically_sized(2),
        kind: MirTypeKind::Slice {
            element: Box::new(u16_ty()),
        },
    };
    let reference = MirSemanticType {
        layout: MirLayout::sized(16, 8),
        kind: MirTypeKind::Reference {
            referent: Box::new(slice),
            mutability: MirMutability::Immutable,
            address_space: MirAddressSpace(3),
        },
    };
    let array = MirSemanticType {
        layout: MirLayout::sized(4, 1),
        kind: MirTypeKind::Array {
            element: Box::new(u8_ty()),
            length: 4,
        },
    };
    let tuple = MirSemanticType {
        layout: MirLayout::sized(24, 8),
        kind: MirTypeKind::Tuple(MirAggregateLayout {
            fields: vec![
                MirField {
                    name: None,
                    offset: 0,
                    ty: reference,
                },
                MirField {
                    name: None,
                    offset: 16,
                    ty: array,
                },
            ],
            padding: vec![MirPadding {
                offset: 20,
                size: 4,
            }],
        }),
    };
    let raw = MirSemanticType {
        layout: MirLayout::sized(8, 8),
        kind: MirTypeKind::RawPointer {
            pointee: Box::new(tuple),
            mutability: MirMutability::Mutable,
            address_space: MirAddressSpace::DEFAULT,
        },
    };

    raw.validate().unwrap();
    let text = raw.canonical_text().unwrap();
    assert!(text.contains("raw(mut=mut;addrspace=0"));
    assert!(text.contains("ref(mut=const;addrspace=3"));
    assert!(text.contains("array(len=4"));

    let tail_struct = MirSemanticType {
        layout: MirLayout::dynamically_sized(2),
        kind: MirTypeKind::Struct(MirStructType {
            identity: "demo::HeaderAndTail".into(),
            aggregate: MirAggregateLayout {
                fields: vec![
                    MirField {
                        name: Some("header".into()),
                        offset: 0,
                        ty: u8_ty(),
                    },
                    MirField {
                        name: Some("tail".into()),
                        offset: 2,
                        ty: MirSemanticType {
                            layout: MirLayout::dynamically_sized(2),
                            kind: MirTypeKind::Slice {
                                element: Box::new(u16_ty()),
                            },
                        },
                    },
                ],
                padding: vec![MirPadding { offset: 1, size: 1 }],
            },
        }),
    };
    tail_struct.validate().unwrap();
}

#[test]
fn validates_direct_enum_layout_and_discriminants() {
    let ty = MirSemanticType {
        layout: MirLayout::sized(8, 4),
        kind: MirTypeKind::Enum(MirEnumType {
            identity: "demo::Value".into(),
            discriminant: MirScalarType::Int {
                signed: false,
                bits: 32,
            },
            encoding: MirEnumEncoding::Direct {
                tag_offset: 0,
                tag: MirScalarType::Int {
                    signed: false,
                    bits: 8,
                },
            },
            variants: vec![
                MirVariant {
                    index: 0,
                    name: "Empty".into(),
                    discriminant: 0,
                    aggregate: MirAggregateLayout {
                        fields: Vec::new(),
                        padding: vec![MirPadding { offset: 1, size: 7 }],
                    },
                },
                MirVariant {
                    index: 1,
                    name: "Number".into(),
                    discriminant: 7,
                    aggregate: MirAggregateLayout {
                        fields: vec![MirField {
                            name: Some("value".into()),
                            offset: 4,
                            ty: u32_ty(),
                        }],
                        padding: vec![MirPadding { offset: 1, size: 3 }],
                    },
                },
            ],
        }),
    };

    ty.validate().unwrap();
    let canonical = ty.canonical_text().unwrap();
    assert!(canonical.contains("encoding=direct(offset=0;tag=u8)"));
    assert!(
        canonical.contains(
            "variant(index=1;name=6:Number;discriminant=00000000000000000000000000000007"
        )
    );
}

#[test]
fn validates_niche_enum_layout() {
    let pointer = MirSemanticType {
        layout: MirLayout::sized(8, 8),
        kind: MirTypeKind::RawPointer {
            pointee: Box::new(u8_ty()),
            mutability: MirMutability::Immutable,
            address_space: MirAddressSpace::DEFAULT,
        },
    };
    let ty = MirSemanticType {
        layout: MirLayout::sized(8, 8),
        kind: MirTypeKind::Enum(MirEnumType {
            identity: "core::option::Option<*const u8>".into(),
            discriminant: MirScalarType::Int {
                signed: false,
                bits: 8,
            },
            encoding: MirEnumEncoding::Niche {
                niche_offset: 0,
                niche_bits: 64,
                untagged_variant: 1,
                niche_variants_start: 0,
                niche_variants_end: 0,
                niche_start: 0,
            },
            variants: vec![
                MirVariant {
                    index: 0,
                    name: "None".into(),
                    discriminant: 0,
                    aggregate: MirAggregateLayout {
                        fields: Vec::new(),
                        padding: Vec::new(),
                    },
                },
                MirVariant {
                    index: 1,
                    name: "Some".into(),
                    discriminant: 1,
                    aggregate: MirAggregateLayout {
                        fields: vec![MirField {
                            name: Some("0".into()),
                            offset: 0,
                            ty: pointer,
                        }],
                        padding: Vec::new(),
                    },
                },
            ],
        }),
    };

    ty.validate().unwrap();
    assert!(ty.canonical_text().unwrap().contains(
        "niche(offset=0;bits=64;untagged=1;variants=0..=0;start=00000000000000000000000000000000)"
    ));
}

#[test]
fn rejects_invalid_layouts_and_missing_padding() {
    let bad_alignment = MirSemanticType {
        layout: MirLayout::sized(4, 3),
        kind: MirTypeKind::Scalar(MirScalarType::Float { bits: 32 }),
    };
    assert_eq!(
        bad_alignment.validate().unwrap_err().to_string(),
        "type: alignment must be a nonzero power of two"
    );

    let missing_padding = MirSemanticType {
        layout: MirLayout::sized(4, 4),
        kind: MirTypeKind::Struct(MirStructType {
            identity: "demo::Padded".into(),
            aggregate: MirAggregateLayout {
                fields: vec![MirField {
                    name: Some("byte".into()),
                    offset: 0,
                    ty: u8_ty(),
                }],
                padding: Vec::new(),
            },
        }),
    };
    let error = missing_padding.canonical_text().unwrap_err();
    assert_eq!(error.path(), "type.padding");
    assert!(error.reason().contains("offset: 1, size: 3"));
}

#[test]
fn rejects_overlapping_fields_and_recursive_invalidity() {
    let overlapping = MirSemanticType {
        layout: MirLayout::sized(4, 2),
        kind: MirTypeKind::Tuple(MirAggregateLayout {
            fields: vec![
                MirField {
                    name: None,
                    offset: 0,
                    ty: u16_ty(),
                },
                MirField {
                    name: None,
                    offset: 1,
                    ty: u16_ty(),
                },
            ],
            padding: vec![MirPadding { offset: 3, size: 1 }],
        }),
    };
    assert_eq!(
        overlapping.validate().unwrap_err().to_string(),
        "type: non-zero-sized aggregate fields overlap"
    );

    let invalid_child = MirSemanticType {
        layout: MirLayout::sized(8, 8),
        kind: MirTypeKind::Reference {
            referent: Box::new(MirSemanticType {
                layout: MirLayout::sized(2, 1),
                kind: MirTypeKind::Scalar(MirScalarType::Bool),
            }),
            mutability: MirMutability::Immutable,
            address_space: MirAddressSpace::DEFAULT,
        },
    };
    assert_eq!(
        invalid_child.validate().unwrap_err().to_string(),
        "type.referent: scalar requires size 1"
    );
}

#[test]
fn rejects_malformed_enum_evidence() {
    let duplicate_discriminants = MirSemanticType {
        layout: MirLayout::sized(1, 1),
        kind: MirTypeKind::Enum(MirEnumType {
            identity: "demo::Bad".into(),
            discriminant: MirScalarType::Int {
                signed: false,
                bits: 8,
            },
            encoding: MirEnumEncoding::Direct {
                tag_offset: 0,
                tag: MirScalarType::Int {
                    signed: false,
                    bits: 8,
                },
            },
            variants: vec![
                MirVariant {
                    index: 0,
                    name: "A".into(),
                    discriminant: 0,
                    aggregate: empty_aggregate(0),
                },
                MirVariant {
                    index: 1,
                    name: "B".into(),
                    discriminant: 0,
                    aggregate: empty_aggregate(0),
                },
            ],
        }),
    };
    assert_eq!(
        duplicate_discriminants.validate().unwrap_err().to_string(),
        "type.enum.variant[1]: variant discriminants must be unique"
    );

    let bad_niche = MirSemanticType {
        layout: MirLayout::sized(1, 1),
        kind: MirTypeKind::Enum(MirEnumType {
            identity: "demo::BadNiche".into(),
            discriminant: MirScalarType::Int {
                signed: false,
                bits: 8,
            },
            encoding: MirEnumEncoding::Niche {
                niche_offset: 0,
                niche_bits: 7,
                untagged_variant: 0,
                niche_variants_start: 0,
                niche_variants_end: 0,
                niche_start: 0,
            },
            variants: vec![MirVariant {
                index: 0,
                name: "Only".into(),
                discriminant: 0,
                aggregate: empty_aggregate(1),
            }],
        }),
    };
    assert_eq!(
        bad_niche.validate().unwrap_err().to_string(),
        "type.enum.encoding: niche width must be 8..=128 whole bits"
    );
}

#[test]
fn validates_uninhabited_enums_and_rejects_narrow_direct_tags() {
    let never = MirSemanticType {
        layout: MirLayout::sized(0, 1),
        kind: MirTypeKind::Enum(MirEnumType {
            identity: "demo::Never".into(),
            discriminant: MirScalarType::Int {
                signed: false,
                bits: 8,
            },
            encoding: MirEnumEncoding::Uninhabited,
            variants: Vec::new(),
        }),
    };
    assert!(
        never
            .canonical_text()
            .unwrap()
            .contains("encoding=uninhabited")
    );
    assert!(!never.is_inhabited().unwrap());
    assert!(!never.has_single_zero_sized_value().unwrap());

    let unit = MirSemanticType {
        layout: MirLayout::sized(0, 1),
        kind: MirTypeKind::Unit,
    };
    assert!(unit.is_inhabited().unwrap());
    assert!(unit.has_single_zero_sized_value().unwrap());

    let narrow_tag = MirSemanticType {
        layout: MirLayout::sized(1, 1),
        kind: MirTypeKind::Enum(MirEnumType {
            identity: "demo::WideDiscriminant".into(),
            discriminant: MirScalarType::Int {
                signed: false,
                bits: 32,
            },
            encoding: MirEnumEncoding::Direct {
                tag_offset: 0,
                tag: MirScalarType::Int {
                    signed: false,
                    bits: 8,
                },
            },
            variants: vec![MirVariant {
                index: 0,
                name: "Large".into(),
                discriminant: 256,
                aggregate: empty_aggregate(0),
            }],
        }),
    };
    assert_eq!(
        narrow_tag.validate().unwrap_err().to_string(),
        "type.enum.encoding: a logical discriminant does not fit the physical direct tag"
    );
}
