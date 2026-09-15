use super::*;

#[test]
fn retained_value_compact_encoding_roundtrips_all_tags_without_maximum_collisions() {
    assert_eq!(std::mem::size_of::<DefinitionRow>(), 8);
    assert_eq!(std::mem::size_of::<IncomingRow>(), 12);
    for origin in [
        DefinitionOrigin::Missing,
        DefinitionOrigin::Entry { argument: 0 },
        DefinitionOrigin::Entry {
            argument: u32::MAX as usize,
        },
        DefinitionOrigin::Event {
            block: SsaBlockIdV1::new(0),
            event: 0,
        },
        DefinitionOrigin::Event {
            block: SsaBlockIdV1::new((1 << 30) - 1),
            event: u32::MAX,
        },
        DefinitionOrigin::Edge {
            incoming: 0,
            definition: 0,
        },
        DefinitionOrigin::Edge {
            incoming: (1 << 30) - 1,
            definition: u32::MAX as usize,
        },
    ] {
        assert_eq!(DefinitionRow::new(origin).unwrap().origin(), origin);
    }
    for origin in [
        DefinitionOrigin::Event {
            block: SsaBlockIdV1::new(1 << 30),
            event: 0,
        },
        DefinitionOrigin::Event {
            block: SsaBlockIdV1::new(u32::MAX),
            event: 0,
        },
        DefinitionOrigin::Edge {
            incoming: 1 << 30,
            definition: 0,
        },
    ] {
        assert_eq!(
            DefinitionRow::new(origin),
            Err(ProductionSemanticSsaErrorV1::ResourceOverflow)
        );
    }
    if let Some(too_large) = (u32::MAX as usize).checked_add(1) {
        for origin in [
            DefinitionOrigin::Entry {
                argument: too_large,
            },
            DefinitionOrigin::Edge {
                incoming: 0,
                definition: too_large,
            },
        ] {
            assert_eq!(
                DefinitionRow::new(origin),
                Err(ProductionSemanticSsaErrorV1::ResourceOverflow)
            );
        }
    }
}

#[test]
fn retained_value_compact_edges_preserve_complete_role_and_maximum_valid_ordinal() {
    assert_eq!(fe2o3_mir_model::HARD_MAX_SSA_EDGES_V1, 65536);
    for ordinal in [0, 1, u16::MAX as u32] {
        for role in [0, 3, 4, u16::MAX] {
            let edge = SsaEdgeIdV1::new(SsaBlockIdV1::new(u32::MAX), ordinal);
            let target = SsaBlockIdV1::new(u32::MAX);
            let row = IncomingRow::new(edge, SsaEdgeRoleV1::new(role), target).unwrap();
            assert_eq!(row.edge(), edge);
            assert_eq!(row.role(), SsaEdgeRoleV1::new(role));
            assert_eq!(row.target(), target);
        }
    }
    for ordinal in [65536, u32::MAX] {
        assert_eq!(
            IncomingRow::new(
                SsaEdgeIdV1::new(SsaBlockIdV1::new(0), ordinal),
                SsaEdgeRoleV1::new(0),
                SsaBlockIdV1::new(0)
            ),
            Err(ProductionSemanticSsaErrorV1::ResourceOverflow)
        );
    }
}
