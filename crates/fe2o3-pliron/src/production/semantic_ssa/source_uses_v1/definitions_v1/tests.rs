use super::*;
use fe2o3_mir_model::{SsaBlockInputV1, SsaEdgeInputV1, SsaEventV1};

mod lookup_integrity;

fn fixture(extra_unreachable: usize) -> (SsaConstructionInputV1, SsaConstructionPlanV1) {
    let v = SsaVariableIdV1::new;
    let mut blocks = vec![
        SsaBlockInputV1::new(
            vec![SsaEventV1::Use(v(0)), SsaEventV1::Define(v(1))],
            vec![
                SsaEdgeInputV1::new(SsaEdgeRoleV1::new(3), SsaBlockIdV1::new(1), vec![v(2)]),
                SsaEdgeInputV1::new(SsaEdgeRoleV1::new(4), SsaBlockIdV1::new(2), vec![]),
            ],
        ),
        SsaBlockInputV1::new(
            vec![
                SsaEventV1::Use(v(0)),
                SsaEventV1::Use(v(1)),
                SsaEventV1::Use(v(2)),
            ],
            vec![],
        ),
        SsaBlockInputV1::new(vec![SsaEventV1::Use(v(0)), SsaEventV1::Use(v(1))], vec![]),
    ];
    for _ in 0..extra_unreachable {
        blocks.push(SsaBlockInputV1::new(
            vec![SsaEventV1::Define(v(1))],
            vec![SsaEdgeInputV1::new(
                SsaEdgeRoleV1::new(9),
                SsaBlockIdV1::new(1),
                vec![v(2)],
            )],
        ));
    }
    let input =
        SsaConstructionInputV1::new(SsaBlockIdV1::new(0), 3, vec![true; 3], vec![v(0)], blocks);
    let plan = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
    (input, plan)
}
fn hash(index: &ValueOriginsV1) -> [u8; 32] {
    let mut hash = Sha256::new();
    index.hash_into(&mut hash);
    hash.finalize().into()
}

#[test]
fn retained_value_index_entry_event_and_edge_rows_are_a_bijection_of_actual_definitions() {
    let (input, plan) = fixture(2);
    let index = ValueOriginsV1::build(&input, &plan).unwrap();
    assert_eq!(index.definitions.len(), 3);
    assert_eq!(index.incoming.len(), 2);
    assert_eq!(index.offsets, [0, 0, 1, 2, 2, 2]);
    for (id, row) in index.definitions.iter().enumerate() {
        let value = match row.origin() {
            DefinitionOrigin::Entry { argument } => {
                assert_eq!(
                    plan.entry_definitions()[argument].variable(),
                    SsaVariableIdV1::new(0)
                );
                plan.entry_definitions()[argument].value()
            }
            DefinitionOrigin::Event { block, event } => {
                let SsaResolvedEventV1::Define { variable, value } =
                    plan.resolved_event(block, event).unwrap()
                else {
                    panic!("actual Define")
                };
                assert_eq!(*variable, SsaVariableIdV1::new(1));
                *value
            }
            DefinitionOrigin::Edge {
                incoming,
                definition,
            } => {
                let row = index.incoming[incoming];
                assert_eq!(row.role(), SsaEdgeRoleV1::new(3));
                assert_eq!(
                    plan.edge_definitions(row.edge()).unwrap()[definition].variable(),
                    SsaVariableIdV1::new(2)
                );
                plan.edge_definitions(row.edge()).unwrap()[definition].value()
            }
            DefinitionOrigin::Missing => panic!("no definition holes"),
        };
        assert!(matches!(value,SsaValueV1::Definition(actual) if actual.get() as usize==id));
    }
    assert_eq!(index.incoming[1].role(), SsaEdgeRoleV1::new(4));
    assert!(
        plan.edge_definitions(index.incoming[1].edge())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn retained_value_index_same_length_substitutions_all_change_identity() {
    let (input, plan) = fixture(0);
    let index = ValueOriginsV1::build(&input, &plan).unwrap();
    let original = hash(&index);
    for mutation in 0..9 {
        let mut changed = index.clone();
        match mutation {
            0 => changed.entry = SsaBlockIdV1::new(1),
            1 => {
                changed.definitions[0] = DefinitionRow::MISSING;
            }
            2 => {
                changed.definitions[0] =
                    DefinitionRow::new(DefinitionOrigin::Entry { argument: 1 }).unwrap();
            }
            3 => {
                changed.definitions[1] = DefinitionRow::new(DefinitionOrigin::Event {
                    block: SsaBlockIdV1::new(0),
                    event: 0,
                })
                .unwrap();
            }
            4 => {
                changed.definitions[2] = DefinitionRow::new(DefinitionOrigin::Edge {
                    incoming: 1,
                    definition: 0,
                })
                .unwrap();
            }
            5 => changed.offsets[1] = 1,
            6 => {
                changed.incoming[0] = IncomingRow::new(
                    SsaEdgeIdV1::new(SsaBlockIdV1::new(0), 1),
                    index.incoming[0].role(),
                    index.incoming[0].target(),
                )
                .unwrap()
            }
            7 => {
                changed.incoming[0] = IncomingRow::new(
                    index.incoming[0].edge(),
                    SsaEdgeRoleV1::new(4),
                    index.incoming[0].target(),
                )
                .unwrap()
            }
            _ => {
                changed.incoming[0] = IncomingRow::new(
                    index.incoming[0].edge(),
                    index.incoming[0].role(),
                    SsaBlockIdV1::new(2),
                )
                .unwrap()
            }
        }
        assert_ne!(index, changed);
        assert_ne!(original, hash(&changed), "mutation {mutation}");
    }
}

#[test]
fn retained_value_index_storage_accounts_retained_replay_arrays_cursor_and_headers() {
    for extra in [0, 1, 64, 1024] {
        let (input, plan) = fixture(extra);
        let index = ValueOriginsV1::build(&input, &plan).unwrap();
        let resources = ValueOriginsV1::resources_for(&plan).unwrap();
        assert_eq!(index.definitions.capacity(), index.definitions.len());
        assert_eq!(index.incoming.capacity(), index.incoming.len());
        assert_eq!(index.offsets.capacity(), index.offsets.len());
        let word = std::mem::size_of::<usize>();
        let arrays = (index.definitions.capacity() * std::mem::size_of::<DefinitionRow>())
            .div_ceil(word)
            + (index.incoming.capacity() * std::mem::size_of::<IncomingRow>()).div_ceil(word)
            + (index.offsets.capacity() * std::mem::size_of::<u32>()).div_ceil(word);
        let observed = 2 * (arrays + std::mem::size_of::<ValueOriginsV1>().div_ceil(word))
            + (input.blocks().len() * std::mem::size_of::<u32>()).div_ceil(word)
            + std::mem::size_of::<Vec<u32>>().div_ceil(word);
        assert!(resources.storage_words >= observed);
        assert_eq!(
            resources.work_units,
            12 * (plan.resources().input_blocks()
                + plan.resources().input_edges()
                + plan.definition_count()
                + plan.resources().input_events()
                + 1)
        );
        assert_eq!(index.definitions.len(), 3);
        assert_eq!(index.incoming.len(), 2);
    }
}
