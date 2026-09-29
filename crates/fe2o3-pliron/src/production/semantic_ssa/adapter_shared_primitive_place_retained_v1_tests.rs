//! Inert seeded DATA controls; no admitted ownership or claimed execution.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::*;
const LIMIT: usize = 1024 * 1024;
const FLOOR: usize = 19;
const CAP: usize = 64;
const TREE: usize = 256;

fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}

fn source() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}

fn declaration(index: usize, shape: SemanticTypeShapeV1) -> SemanticTypeDeclV1 {
    let layout = match &shape {
        SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_) => {
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap()
        }
        SemanticTypeShapeV1::Tuple(_) | SemanticTypeShapeV1::Aggregate(_) => {
            let (size, second) = if index == 4 {
                (24, 8)
            } else if index >= 10 {
                (1u64 << (index - 6), 1u64 << (index - 7))
            } else {
                (16, 8)
            };
            SemanticTypeLayoutV1::aggregate(
                Some(size),
                8,
                SemanticAggregateLayoutV1::new(vec![0, second], vec![]).unwrap(),
            )
            .unwrap()
        }
        _ => SemanticTypeLayoutV1::new(Some(if index == 7 { 16 } else { 8 }), 8).unwrap(),
    };
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([index as u8 + 1; 32]),
        SemanticLayoutIdentityV1::from_sha256([index as u8 + 40; 32]),
        layout,
        shape,
    )
}

fn types() -> Vec<SemanticTypeDeclV1> {
    let pointer = |pointee, kind, mutability, metadata| {
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(ty(pointee), kind, mutability, 5, 64, metadata)
                .unwrap(),
        )
    };
    let tuple = |fields: Vec<u32>| {
        SemanticTypeShapeV1::Tuple(
            SemanticAggregateTypeV1::new(fields.into_iter().map(ty).collect()).unwrap(),
        )
    };
    let scalar = SemanticScalarTypeV1::Integer {
        signed: false,
        bits: 32,
    };
    vec![
        SemanticTypeShapeV1::Scalar(scalar),
        pointer(
            0,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            SemanticPointerMetadataV1::None,
        ),
        tuple(vec![0, 1]),
        tuple(vec![1, 1]),
        tuple(vec![0, 2]),
        pointer(
            0,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            SemanticPointerMetadataV1::None,
        ),
        pointer(
            0,
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Immutable,
            SemanticPointerMetadataV1::None,
        ),
        pointer(
            0,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            SemanticPointerMetadataV1::SliceLength,
        ),
        SemanticTypeShapeV1::ValidityScalar(
            SemanticValidityScalarTypeV1::new(
                scalar,
                vec![SemanticScalarValidityRangeV1::new(1, u32::MAX as u128)],
            )
            .unwrap(),
        ),
        pointer(
            8,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            SemanticPointerMetadataV1::None,
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, shape)| declaration(index, shape))
    .collect()
}

fn block(
    index: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([index + 100; 32]),
        source(),
        statements,
        SemanticTerminatorV1::new(source(), terminator),
    )
    .unwrap()
}

fn function_with_locals(
    local_types: &[u32],
    blocks: Vec<SemanticBasicBlockV1>,
    role: SemanticFunctionRoleV1,
) -> SemanticFunctionDeclV1 {
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([201; 32]),
        SemanticLayoutIdentityV1::from_sha256([202; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(ty(local_types[0]), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([203; 32]),
        role,
        SemanticItemDefinitionIdentityV1::from_sha256([204; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([205; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([206; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([207; 32]),
        source(),
        abi,
        local_types
            .iter()
            .enumerate()
            .map(|(index, &kind)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([index as u8 + 60; 32]),
                    ty(kind),
                    if index == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    source(),
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

fn function_with_end(
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticFunctionDeclV1 {
    function_with_locals(
        &[0, 0, 1, 1, 2, 0, 3, 4, 9, 8, 5, 6, 7, 8],
        vec![block(0, statements, terminator)],
        SemanticFunctionRoleV1::KernelRoot,
    )
}

fn function(statements: Vec<SemanticStatementV1>) -> SemanticFunctionDeclV1 {
    function_with_end(statements, SemanticTerminatorKindV1::Return)
}

fn local(index: u32) -> SemanticLocalIdV1 {
    SemanticLocalIdV1::from_index(index)
}

fn place(index: u32, kind: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(local(index), vec![], ty(kind)).unwrap()
}

fn projected(index: u32, projections: &[(SemanticProjectionKindV1, u32)]) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        local(index),
        projections
            .iter()
            .map(|&(kind, result)| SemanticProjectionV1::new(kind, ty(result)).unwrap())
            .collect(),
        ty(projections.last().unwrap().1),
    )
    .unwrap()
}

fn dereference(index: u32, kind: u32) -> SemanticPlaceV1 {
    projected(index, &[(SemanticProjectionKindV1::Dereference, kind)])
}

type CandidateRows = Vec<(u32, u32, u32, u32, u32, usize, bool, bool)>;
type AliasRows = Vec<(usize, Vec<u32>, bool)>;
type DataRows = (
    CandidateRows,
    Vec<(u32, AliasRows)>,
    Vec<(u32, Vec<usize>)>,
    usize,
    usize,
    usize,
    bool,
);
fn aliases(values: &[Alias]) -> AliasRows {
    values
        .iter()
        .map(|alias| (alias.candidate, alias.fields.clone(), alias.live))
        .collect()
}
fn data_rows(data: &AliasData) -> DataRows {
    (
        data.candidates
            .iter()
            .map(|c| {
                (
                    c.site.block,
                    c.site.statement,
                    c.source,
                    c.pointee.index(),
                    c.reference.index(),
                    c.live,
                    c.valid,
                    c.read,
                )
            })
            .collect(),
        data.holders
            .iter()
            .map(|(&key, values)| (key, aliases(values)))
            .collect(),
        data.active
            .iter()
            .map(|(&key, values)| (key, values.iter().copied().collect()))
            .collect(),
        data.alias_words,
        data.cap,
        data.tree_work,
        data.exhausted,
    )
}
struct OriginalMeter<'a, 'w>(&'a mut Budget<'w>);
impl BorrowWork for OriginalMeter<'_, '_> {
    fn work(&mut self, units: usize) -> Result<()> {
        self.0
            .charge_work(units)
            .map_err(|_| Error::ResourceOverflow)
    }
}

#[derive(Clone, Copy, Debug)]
enum Operation {
    Path,
    Deinitialize,
    Write,
}
struct Case {
    name: &'static str,
    function: SemanticFunctionDeclV1,
    types: Vec<SemanticTypeDeclV1>,
    place: SemanticPlaceV1,
}
fn cases() -> Vec<Case> {
    use SemanticProjectionKindV1::{Dereference as D, Field as F};
    let places = vec![
        ("whole", place(7, 4)),
        ("prefix", projected(7, &[(F(1), 2)])),
        ("first", projected(7, &[(F(0), 0)])),
        ("nonmatching", projected(7, &[(F(1), 2), (F(0), 0)])),
        ("nested", projected(7, &[(F(1), 2), (F(1), 1)])),
        ("deref", projected(7, &[(F(1), 2), (F(1), 1), (D, 0)])),
        ("validity", dereference(8, 8)),
        ("empty", place(6, 3)),
        ("source", place(1, 0)),
        ("missing-local", place(99, 0)),
        ("missing-field", projected(7, &[(F(9), 0)])),
        ("field-type-mismatch", projected(7, &[(F(1), 0)])),
        ("final-type-mismatch", place(7, 0)),
        (
            "unsupported-index",
            projected(7, &[(SemanticProjectionKindV1::Index(local(1)), 0)]),
        ),
        ("wrong-base-shape", dereference(7, 0)),
        (
            "nonterminal-deref",
            projected(7, &[(F(1), 2), (F(1), 1), (D, 0), (F(0), 0)]),
        ),
        ("mutable", dereference(10, 0)),
        ("raw", dereference(11, 0)),
        ("metadata", dereference(12, 0)),
        ("pointee-mismatch", dereference(2, 8)),
    ];
    let mut result: Vec<_> = places
        .into_iter()
        .map(|(name, place)| Case {
            name,
            function: function(vec![]),
            types: types(),
            place,
        })
        .collect();
    let mut missing = types();
    missing.truncate(4);
    result.push(Case {
        name: "missing-type",
        function: function(vec![]),
        types: missing,
        place: projected(7, &[(F(1), 2)]),
    });
    for (name, pointee) in [("nonprimitive", 2), ("missing-pointee", 99)] {
        let mut types = types();
        types[1] = declaration(
            1,
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(pointee),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    5,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        );
        result.push(Case {
            name,
            function: function(vec![]),
            types,
            place: dereference(2, pointee),
        });
    }
    result
}
fn seeded<'a>(
    empty_holder: bool,
    empty_output: bool,
    malformed: u8,
) -> (RetainedAliasStateV1<'a>, usize) {
    let mut owner = RetainedAliasStateV1::new();
    owner.data.candidates = (0..3)
        .map(|index| Candidate {
            site: SemanticTransparentBorrowSiteV1 {
                block: 0,
                statement: index as u32,
            },
            source: [1, 5, 9][index],
            pointee: ty(0),
            reference: ty(1),
            live: if empty_holder { 0 } else { 1 },
            valid: true,
            read: index == 0,
        })
        .collect();
    let aliases = if empty_holder {
        Vec::with_capacity(3)
    } else {
        vec![
            Alias {
                candidate: 0,
                fields: vec![0],
                live: true,
            },
            Alias {
                candidate: 1,
                fields: vec![1, 1],
                live: true,
            },
            Alias {
                candidate: 2,
                fields: vec![1, 1, 0],
                live: true,
            },
        ]
    };
    owner.data.holders.insert(7, aliases);
    if !empty_holder {
        owner.data.active = [
            (1, [0usize].into_iter().collect()),
            (5, [1usize].into_iter().collect()),
            (9, [2usize].into_iter().collect()),
        ]
        .into_iter()
        .collect();
        owner.data.alias_words = 9;
    }
    if empty_output {
        owner.output = Vec::with_capacity(4);
    }
    if malformed == 1 {
        owner.data.active.remove(&5);
    }
    if malformed == 2 {
        owner.data.alias_words = 0;
    }
    let mut prefix = owner.data.candidates.capacity() * size_of::<Candidate>()
        + owner.output.capacity() * size_of::<Alias>();
    for aliases in owner.data.holders.values() {
        prefix += size_of::<(u32, Vec<Alias>)>() + aliases.capacity() * size_of::<Alias>();
        for alias in aliases {
            prefix += alias.fields.capacity() * size_of::<u32>();
        }
    }
    for values in owner.data.active.values() {
        prefix += size_of::<(u32, BTreeSet<usize>)>() + values.len() * size_of::<usize>();
    }
    (owner, prefix)
}
type PathRows = (Vec<SemanticProjectionKindV1>, bool, u32);
fn path_rows(path: Option<Path<'_>>) -> Option<PathRows> {
    path.map(|path| {
        (
            path.fields.iter().map(|field| field.kind()).collect(),
            path.dereference,
            path.selected.index(),
        )
    })
}
fn parked_rows(
    owner: &RetainedAliasStateV1<'_>,
) -> (
    Option<(u32, usize, AliasRows)>,
    Option<AliasRows>,
    Option<(usize, Vec<u32>, bool)>,
    AliasRows,
) {
    (
        owner
            .removed
            .as_ref()
            .map(|held| (held.local, held.cursor, aliases(&held.aliases))),
        owner.drain.as_ref().map(|drain| aliases(drain.as_slice())),
        owner
            .current
            .as_ref()
            .map(|alias| (alias.candidate, alias.fields.clone(), alias.live)),
        aliases(&owner.output),
    )
}
fn operate<'a>(
    session: &mut RetainedAliasSessionV1<'_, 'a, '_>,
    operation: Operation,
    place: &'a SemanticPlaceV1,
) -> Result<Option<PathRows>> {
    match operation {
        Operation::Path => session.path(place).map(path_rows),
        Operation::Deinitialize => session.deinitialize(place).map(|()| None),
        Operation::Write => session.write_place(place).map(path_rows),
    }
}
fn original(
    operation: Operation,
    case: &Case,
    limit: usize,
    empty_holder: bool,
    malformed: u8,
) -> (
    std::result::Result<Option<PathRows>, String>,
    DataRows,
    usize,
    Option<usize>,
) {
    let (owner, _) = seeded(empty_holder, false, malformed);
    let AliasData {
        candidates,
        holders,
        active,
        alias_words,
        ..
    } = owner.data;
    let mut work = Work::new(limit);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut meter = OriginalMeter(&mut budget);
    let mut observer = NoReads;
    let mut analysis = Analysis {
        function: &case.function,
        types: &case.types,
        meter: &mut meter,
        observer: &mut observer,
        site: None,
        candidates,
        holders,
        active,
        alias_words,
        cap: CAP,
        tree_work: TREE,
        exhausted: false,
    };
    let result = match operation {
        Operation::Path => analysis.path(&case.place).map(path_rows),
        Operation::Deinitialize => analysis.deinitialize(&case.place).map(|()| None),
        Operation::Write => analysis.write_place(&case.place).map(path_rows),
    }
    .map_err(|error| format!("{error:?}"));
    let data = AliasData {
        candidates: analysis.candidates,
        holders: analysis.holders,
        active: analysis.active,
        alias_words: analysis.alias_words,
        cap: analysis.cap,
        tree_work: analysis.tree_work,
        exhausted: analysis.exhausted,
    };
    (
        result,
        data_rows(&data),
        budget.work(),
        budget.failed_work(),
    )
}
fn total_frame() -> usize {
    super::super::frame().unwrap()
}
fn measure(
    operation: Operation,
    case: &Case,
    empty_holder: bool,
    empty_output: bool,
) -> (usize, usize, usize) {
    let (mut owner, prefix) = seeded(empty_holder, empty_output, 0);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR + prefix).unwrap();
    let mut owned = prefix;
    let mut session = RetainedAliasSessionV1::begin(
        &mut owner,
        &case.function,
        &case.types,
        CAP,
        TREE,
        &mut budget,
        &mut owned,
    )
    .unwrap();
    operate(&mut session, operation, &case.place).unwrap();
    session.finish().unwrap();
    let result = (budget.work(), budget.storage(), prefix);
    drop(owner);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    result
}
#[test]
fn retained_place_all_paths_match_complete_original_data_and_work() {
    for case in cases() {
        for operation in [Operation::Path, Operation::Deinitialize, Operation::Write] {
            let expected = original(operation, &case, LIMIT, false, 0);
            let (mut owner, prefix) = seeded(false, false, 0);
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR + prefix).unwrap();
            let mut owned = prefix;
            let mut session = RetainedAliasSessionV1::begin(
                &mut owner,
                &case.function,
                &case.types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            )
            .unwrap();
            let actual =
                operate(&mut session, operation, &case.place).map_err(|e| format!("{e:?}"));
            assert_eq!(actual, expected.0, "{} {operation:?}", case.name);
            session.finish().unwrap();
            assert_eq!(
                data_rows(&owner.data),
                expected.1,
                "{} {operation:?}",
                case.name
            );
            assert_eq!(budget.work(), expected.2 + 32);
            assert!(std::ptr::eq(owner.place.unwrap(), &case.place));
            assert!(
                owner
                    .postflight_for(&case.function, &case.types, &budget, &owned)
                    .is_ok()
            );
            assert_eq!(budget.storage(), FLOOR + owned);
            assert!(owner.removed.is_none() && owner.current.is_none() && owner.drain.is_none());
            assert!(owner.output.is_empty());
            drop(owner);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}
#[test]
fn retained_place_every_work_cut_matches_original_partial_data_and_first_denial() {
    let mut saw_current = false;
    let mut saw_drain = false;
    let mut saw_output = false;
    let mut saw_removed = false;
    for case in cases() {
        for operation in [Operation::Path, Operation::Deinitialize, Operation::Write] {
            let needed = measure(operation, &case, false, false).0;
            for limit in 0..needed {
                let (mut owner, prefix) = seeded(false, false, 0);
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, LIMIT);
                budget.reserve_storage(FLOOR + prefix).unwrap();
                let mut owned = prefix;
                let result: RetainedResult<()> = match RetainedAliasSessionV1::begin(
                    &mut owner,
                    &case.function,
                    &case.types,
                    CAP,
                    TREE,
                    &mut budget,
                    &mut owned,
                ) {
                    Err(error) => Err(error),
                    Ok(mut session) => {
                        let error = operate(&mut session, operation, &case.place).err().unwrap();
                        Err(session.map_error(error))
                    }
                };
                let RetainedAliasErrorV1::Resource(Resource::Work(first)) = result.err().unwrap()
                else {
                    panic!("wrong refusal");
                };
                assert_eq!(owner.failure, Some(Resource::Work(first)));
                assert_eq!(owner.phase, Phase::Terminal);
                assert_eq!(budget.storage(), FLOOR + owned);
                if limit >= 32 {
                    let expected = original(operation, &case, limit - 32, false, 0);
                    assert!(expected.0.is_err());
                    assert_eq!(
                        data_rows(&owner.data),
                        expected.1,
                        "{} {operation:?} {limit}",
                        case.name
                    );
                    assert_eq!(budget.work(), expected.2 + 32);
                    assert_eq!(first.actual(), expected.3.unwrap() + 32);
                    assert!(std::ptr::eq(owner.place.unwrap(), &case.place));
                } else {
                    assert_eq!(owned, prefix);
                    assert_eq!(budget.work(), 0);
                    assert!(owner.place.is_none());
                }
                saw_current |= owner.current.is_some();
                saw_drain |= owner
                    .drain
                    .as_ref()
                    .is_some_and(|d| !d.as_slice().is_empty());
                saw_output |= !owner.output.is_empty();
                saw_removed |= owner.removed.is_some();
                let parked = parked_rows(&owner);
                assert!(
                    owner
                        .postflight_for(&case.function, &case.types, &budget, &owned)
                        .is_err()
                );
                assert_eq!(parked, parked_rows(&owner));
                drop(owner);
                budget.release_storage(owned).unwrap();
                assert_eq!(budget.storage(), FLOOR);
                assert_eq!(budget.failed_work(), Some(first.actual()));
            }
        }
    }
    assert!(saw_current && saw_drain && saw_output && saw_removed);
}
#[test]
fn retained_place_every_storage_cut_retains_current_output_and_reinsert_buffers() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    let mut saw_current = false;
    let mut saw_output_reinsert = false;
    let mut saw_removed_reinsert = false;
    for operation in [Operation::Deinitialize, Operation::Write] {
        let (_, needed, prefix) = measure(operation, &case, false, false);
        for limit in FLOOR + prefix..needed {
            let (mut owner, actual_prefix) = seeded(false, false, 0);
            assert_eq!(actual_prefix, prefix);
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(FLOOR + prefix).unwrap();
            let mut owned = prefix;
            let result: RetainedResult<()> = match RetainedAliasSessionV1::begin(
                &mut owner,
                &case.function,
                &case.types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            ) {
                Err(error) => Err(error),
                Ok(mut session) => {
                    let error = operate(&mut session, operation, &case.place).err().unwrap();
                    Err(session.map_error(error))
                }
            };
            let RetainedAliasErrorV1::Resource(Resource::Storage(first)) = result.err().unwrap()
            else {
                panic!("wrong refusal");
            };
            assert_eq!(owner.failure, Some(Resource::Storage(first)));
            assert_eq!(budget.storage(), FLOOR + owned);
            assert_eq!(budget.failed_storage(), Some(first.actual()));
            if owned > prefix {
                assert!(std::ptr::eq(owner.place.unwrap(), &case.place));
                if let Some(current) = &owner.current {
                    saw_current = true;
                    assert_eq!(current.candidate, 0);
                    assert_eq!(current.fields, vec![0]);
                    assert_eq!(owner.drain.as_ref().unwrap().as_slice().len(), 2);
                    assert!(owner.output.is_empty());
                } else if let Some(held) = &owner.removed {
                    saw_removed_reinsert = true;
                    assert_eq!(held.cursor, 3);
                    assert_eq!(
                        aliases(&held.aliases),
                        vec![
                            (0, vec![0], true),
                            (1, vec![1, 1], false),
                            (2, vec![1, 1, 0], false)
                        ]
                    );
                } else {
                    saw_output_reinsert = true;
                    assert_eq!(aliases(&owner.output), vec![(0, vec![0], true)]);
                    assert!(owner.drain.is_none());
                }
                assert!(!owner.data.holders.contains_key(&7));
            }
            let peak = budget.peak_storage();
            drop(owner);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), peak);
        }
    }
    assert!(saw_current && saw_output_reinsert && saw_removed_reinsert);
}
#[test]
fn retained_place_write_parks_exact_current_unvisited_and_retained_aliases() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    let needed = measure(Operation::Write, &case, false, false).0;
    let mut seen = [false; 3];
    for limit in 32..needed {
        let (mut owner, prefix) = seeded(false, false, 0);
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        {
            let mut session = RetainedAliasSessionV1::begin(
                &mut owner,
                &case.function,
                &case.types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            )
            .unwrap();
            assert!(session.write_place(&case.place).is_err());
        }
        if let Some(current) = &owner.current {
            seen[current.candidate] = true;
            let expected = [vec![0], vec![1, 1], vec![1, 1, 0]];
            assert_eq!(current.fields, expected[current.candidate]);
            let drain = owner.drain.as_ref().unwrap().as_slice();
            assert_eq!(drain.len(), 2 - current.candidate);
            for alias in drain {
                assert_eq!(alias.fields, expected[alias.candidate]);
            }
            if current.candidate > 0 {
                assert_eq!(aliases(&owner.output), vec![(0, vec![0], true)]);
            } else {
                assert!(owner.output.is_empty());
            }
        }
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
    assert!(seen.into_iter().all(|value| value));
}
#[test]
fn retained_place_deinitialize_cursors_and_all_removed_fields_survive() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    let needed = measure(Operation::Deinitialize, &case, false, false).0;
    let mut seen = [false; 4];
    for limit in 32..needed {
        let (mut owner, prefix) = seeded(false, false, 0);
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        {
            let mut session = RetainedAliasSessionV1::begin(
                &mut owner,
                &case.function,
                &case.types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            )
            .unwrap();
            assert!(session.deinitialize(&case.place).is_err());
        }
        if let Some(held) = &owner.removed {
            seen[held.cursor] = true;
            assert_eq!(held.local, 7);
            assert_eq!(held.aliases.len(), 3);
            assert_eq!(held.aliases[0].fields, vec![0]);
            assert_eq!(held.aliases[1].fields, vec![1, 1]);
            assert_eq!(held.aliases[2].fields, vec![1, 1, 0]);
            assert!(!owner.data.holders.contains_key(&7));
        }
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
    assert!(seen.into_iter().all(|value| value));
}
#[test]
fn retained_place_empty_holders_follow_distinct_original_reinsert_rules() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    for operation in [Operation::Deinitialize, Operation::Write] {
        let expected = original(operation, &case, LIMIT, true, 0);
        let (mut owner, prefix) = seeded(true, false, 0);
        let pointer = owner.data.holders[&7].as_ptr();
        let capacity = owner.data.holders[&7].capacity();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        let mut session = RetainedAliasSessionV1::begin(
            &mut owner,
            &case.function,
            &case.types,
            CAP,
            TREE,
            &mut budget,
            &mut owned,
        )
        .unwrap();
        operate(&mut session, operation, &case.place).unwrap();
        session.finish().unwrap();
        assert_eq!(data_rows(&owner.data), expected.1);
        assert_eq!(budget.work(), expected.2 + 32);
        if matches!(operation, Operation::Deinitialize) {
            assert!(owner.data.holders[&7].is_empty());
            assert_eq!(owner.data.holders[&7].as_ptr(), pointer);
            assert_eq!(owner.data.holders[&7].capacity(), capacity);
            assert_eq!(
                owned,
                prefix + total_frame() + size_of::<(u32, Vec<Alias>)>()
            );
        } else {
            assert!(!owner.data.holders.contains_key(&7));
            assert_eq!(owned, prefix + total_frame());
        }
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_place_empty_allocated_output_survives_none_and_early_refusals() {
    for case in cases().into_iter().filter(|case| {
        original(Operation::Write, case, LIMIT, false, 0)
            .0
            .unwrap()
            .is_none()
    }) {
        for limit in [0, 32, 32 + TREE, LIMIT] {
            let (mut owner, prefix) = seeded(false, true, 0);
            let pointer = owner.output.as_ptr();
            let capacity = owner.output.capacity();
            assert!(capacity > 0);
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(prefix).unwrap();
            let mut owned = prefix;
            match RetainedAliasSessionV1::begin(
                &mut owner,
                &case.function,
                &case.types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            ) {
                Err(_) => {}
                Ok(mut session) => {
                    if session.write_place(&case.place).is_ok() {
                        session.finish().unwrap();
                    }
                }
            }
            assert!(owner.output.is_empty());
            assert_eq!(owner.output.as_ptr(), pointer);
            assert_eq!(owner.output.capacity(), capacity);
            drop(owner);
            budget.release_storage(owned).unwrap();
        }
    }
}
#[test]
fn retained_place_empty_output_capacity_is_attached_before_population() {
    let case = cases().remove(0);
    let (mut owner, prefix) = seeded(false, false, 0);
    let mut work = Work::new(33);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(prefix).unwrap();
    let mut owned = prefix;
    {
        let mut session = RetainedAliasSessionV1::begin(
            &mut owner,
            &case.function,
            &case.types,
            CAP,
            TREE,
            &mut budget,
            &mut owned,
        )
        .unwrap();
        session.output_capacity().unwrap();
        assert!(session.owner.output.is_empty());
        assert!(session.owner.output.capacity() >= 1);
        let pointer = session.owner.output.as_ptr();
        let capacity = session.owner.output.capacity();
        let counter = *session.owned;
        assert!(session.tree().is_err());
        assert!(session.write_place(&case.place).is_err());
        assert!(session.output_capacity().is_err());
        assert_eq!(session.owner.output.as_ptr(), pointer);
        assert_eq!(session.owner.output.capacity(), capacity);
        assert_eq!(*session.owned, counter);
    }
    assert!(owner.output.is_empty() && owner.output.capacity() >= 1);
    drop(owner);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_place_original_errors_preserve_mutations_and_parked_fields() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    for (operation, malformed) in [
        (Operation::Deinitialize, 1),
        (Operation::Write, 1),
        (Operation::Write, 2),
    ] {
        let expected = original(operation, &case, LIMIT, false, malformed);
        let (mut owner, prefix) = seeded(false, false, malformed);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        {
            let mut session = RetainedAliasSessionV1::begin(
                &mut owner,
                &case.function,
                &case.types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            )
            .unwrap();
            let error = operate(&mut session, operation, &case.place).err().unwrap();
            assert_eq!(format!("{error:?}"), expected.0.err().unwrap());
            assert!(matches!(
                session.map_error(error),
                RetainedAliasErrorV1::Original(_)
            ));
        }
        assert_eq!(data_rows(&owner.data), expected.1);
        assert_eq!(budget.work(), expected.2 + 32);
        assert!(owner.failure.is_none());
        assert_eq!(owner.phase, Phase::Terminal);
        if matches!(operation, Operation::Write) {
            assert_eq!(owner.current.as_ref().unwrap().fields, vec![1, 1]);
            assert_eq!(aliases(&owner.output), vec![(0, vec![0], true)]);
            assert_eq!(
                aliases(owner.drain.as_ref().unwrap().as_slice()),
                vec![(2, vec![1, 1, 0], true)]
            );
        } else {
            assert_eq!(owner.removed.as_ref().unwrap().cursor, 1);
        }
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_place_terminal_guards_preserve_inputs_outputs_and_first_failure() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    for resource in [false, true] {
        let (mut owner, prefix) = seeded(false, true, if resource { 0 } else { 1 });
        let mut work = Work::new(if resource { 33 } else { LIMIT });
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        let mut session = RetainedAliasSessionV1::begin(
            &mut owner,
            &case.function,
            &case.types,
            CAP,
            TREE,
            &mut budget,
            &mut owned,
        )
        .unwrap();
        if resource {
            assert!(session.tree().is_err());
        } else {
            assert!(session.deinitialize(&case.place).is_err());
        }
        let before = (
            data_rows(&session.owner.data),
            parked_rows(session.owner),
            path_rows(session.owner.path),
            session.owner.place.map(|p| p as *const _),
            session.owner.output.as_ptr(),
            session.owner.output.capacity(),
            session.owner.failure,
            snapshot(session.budget, session.owned),
        );
        assert!(session.path(&case.place).is_err());
        assert!(
            session
                .matches(Slot::Current, case.place.projections())
                .is_err()
        );
        assert!(session.deinitialize(&case.place).is_err());
        assert!(session.write_place(&case.place).is_err());
        assert!(session.output_capacity().is_err());
        let after = (
            data_rows(&session.owner.data),
            parked_rows(session.owner),
            path_rows(session.owner.path),
            session.owner.place.map(|p| p as *const _),
            session.owner.output.as_ptr(),
            session.owner.output.capacity(),
            session.owner.failure,
            snapshot(session.budget, session.owned),
        );
        assert!(before == after); // Snapshot's ledger identity intentionally has no Debug.
        assert!(session.finish().is_err());
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_place_incompatible_slots_fail_without_overwriting_payloads() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    for operation in [Operation::Deinitialize, Operation::Write] {
        for slot in 0..4 {
            let (mut owner, prefix) = seeded(false, false, 0);
            let aliases = owner.data.holders.remove(&7).unwrap();
            match slot {
                0 => {
                    owner.removed = Some(Held {
                        local: 7,
                        aliases,
                        cursor: 0,
                    })
                }
                1 => owner.drain = Some(aliases.into_iter()),
                2 => {
                    let mut values = aliases;
                    owner.current = Some(values.remove(0));
                    owner.data.holders.insert(7, values);
                }
                _ => owner.output = aliases,
            }
            let before = parked_rows(&owner);
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(prefix).unwrap();
            let mut owned = prefix;
            let mut session = RetainedAliasSessionV1::begin(
                &mut owner,
                &case.function,
                &case.types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            )
            .unwrap();
            let entry = snapshot(session.budget, session.owned);
            assert!(operate(&mut session, operation, &case.place).is_err());
            assert_eq!(before, parked_rows(session.owner));
            assert!(entry == snapshot(session.budget, session.owned));
            assert!(session.owner.place.is_none());
            drop(session);
            drop(owner);
            budget.release_storage(owned).unwrap();
        }
    }
}
#[test]
fn retained_place_path_and_projection_borrows_bind_exact_input_lifetime() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "deref")
        .unwrap();
    let other = case.place.clone();
    let (mut owner, prefix) = seeded(false, false, 0);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(prefix).unwrap();
    let mut owned = prefix;
    let mut session = RetainedAliasSessionV1::begin(
        &mut owner,
        &case.function,
        &case.types,
        CAP,
        TREE,
        &mut budget,
        &mut owned,
    )
    .unwrap();
    let path = session.path(&case.place).unwrap().unwrap();
    assert_eq!(path.fields.as_ptr(), case.place.projections().as_ptr());
    assert_eq!(path.fields.len(), 2);
    assert_ne!(path.fields.as_ptr(), other.projections().as_ptr());
    session.finish().unwrap();
    assert!(std::ptr::eq(owner.place.unwrap(), &case.place));
    assert_eq!(
        owner.path.unwrap().fields.as_ptr(),
        case.place.projections().as_ptr()
    );
    // This is lifetime/pointer custody only, not proof of membership in function.
    drop(owner);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_place_match_slot_preserves_prefix_semantics_and_work() {
    for case in cases() {
        for index in 0..3 {
            for current in [false, true] {
                let (mut owner, prefix) = seeded(false, false, 0);
                let aliases = owner.data.holders.remove(&7).unwrap();
                let fields = aliases[index].fields.clone();
                if current {
                    let mut values = aliases;
                    owner.current = Some(values.remove(index));
                    owner.data.holders.insert(7, values);
                } else {
                    owner.removed = Some(Held {
                        local: 7,
                        aliases,
                        cursor: 0,
                    });
                }
                let mut work = Work::new(LIMIT);
                let mut budget = Budget::new(&mut work, LIMIT);
                budget.reserve_storage(prefix).unwrap();
                let mut owned = prefix;
                let mut session = RetainedAliasSessionV1::begin(
                    &mut owner,
                    &case.function,
                    &case.types,
                    CAP,
                    TREE,
                    &mut budget,
                    &mut owned,
                )
                .unwrap();
                let prefix_fields = case.place.projections();
                let value = session
                    .matches(
                        if current {
                            Slot::Current
                        } else {
                            Slot::Held(index)
                        },
                        prefix_fields,
                    )
                    .unwrap();
                let mut old_work = Work::new(LIMIT);
                let mut old_budget = Budget::new(&mut old_work, LIMIT);
                let mut meter = OriginalMeter(&mut old_budget);
                let mut observer = NoReads;
                let mut old = Analysis {
                    function: &case.function,
                    types: &case.types,
                    meter: &mut meter,
                    observer: &mut observer,
                    site: None,
                    candidates: vec![],
                    holders: BTreeMap::new(),
                    active: BTreeMap::new(),
                    alias_words: 0,
                    cap: CAP,
                    tree_work: TREE,
                    exhausted: false,
                };
                assert_eq!(value, old.matches(&fields, prefix_fields).unwrap());
                assert_eq!(session.budget.work(), old_budget.work() + 32);
                session.finish().unwrap();
                drop(owner);
                budget.release_storage(owned).unwrap();
            }
        }
    }
}
#[test]
fn retained_place_source_capacity_and_custody_order_is_explicit_without_route_activation() {
    fn compact(value: &str) -> String {
        value.split_whitespace().collect()
    }
    let source = compact(include_str!(
        "adapter_shared_primitive_place_retained_v1.rs"
    ));
    let owner = compact(include_str!(
        "adapter_shared_primitive_alias_retained_v1.rs"
    ));
    let parent = include_str!("adapter_shared_primitive_v29.rs");
    assert!(source.contains("place:&'aSemanticPlaceV1"));
    assert!(
        source.contains(
            "self.owner.drain=self.owner.data.holders.remove(&local).map(Vec::into_iter)"
        )
    );
    let capacity = source
        .split("fnoutput_capacity")
        .nth(1)
        .unwrap()
        .split("pub(super)fnframe")
        .next()
        .unwrap();
    let debit = capacity.find("self.reserve_storage(growth)").unwrap();
    let allocate = capacity
        .find("self.owner.output.try_reserve_exact(1)")
        .unwrap();
    let observed = capacity
        .find("letexcess=self.owner.output.capacity()")
        .unwrap();
    let excess = capacity.find("self.reserve_storage(excess)").unwrap();
    assert!(debit < allocate && allocate < observed && observed < excess);
    assert!(!capacity.contains("mem::take"));
    assert!(owner.contains("output:Vec::new(),place:None,path:None"));
    let ordinary = parent
        .split("// Inert retained alias-state primitives")
        .next()
        .unwrap();
    assert!(!ordinary.contains("RetainedAliasSessionV1"));
    assert!(!ordinary.contains("place_ops"));
}

#[test]
fn retained_place_all_output_growth_cuts_keep_prior_current_and_unvisited_payloads() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "nonmatching")
        .unwrap();
    let (_, needed, prefix) = measure(Operation::Write, &case, false, false);
    let mut seen = [false; 3];
    let mut saw_reinsert = false;
    for limit in FLOOR + prefix + total_frame()..needed {
        let (mut owner, _) = seeded(false, false, 0);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(FLOOR + prefix).unwrap();
        let mut owned = prefix;
        {
            let mut session = RetainedAliasSessionV1::begin(
                &mut owner,
                &case.function,
                &case.types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            )
            .unwrap();
            assert!(session.write_place(&case.place).is_err());
        }
        assert_eq!(owner.phase, Phase::Terminal);
        assert!(!owner.data.holders.contains_key(&7));
        if let Some(current) = &owner.current {
            seen[current.candidate] = true;
            assert_eq!(owner.output.len(), current.candidate);
            assert_eq!(
                owner.drain.as_ref().unwrap().as_slice().len(),
                2 - current.candidate
            );
            let expected = [vec![0], vec![1, 1], vec![1, 1, 0]];
            assert_eq!(current.fields, expected[current.candidate]);
            for alias in &owner.output {
                assert_eq!(alias.fields, expected[alias.candidate]);
            }
            for alias in owner.drain.as_ref().unwrap().as_slice() {
                assert_eq!(alias.fields, expected[alias.candidate]);
            }
        } else {
            saw_reinsert = true;
            assert_eq!(owner.output.len(), 3);
            assert!(owner.drain.is_none());
        }
        assert_eq!(budget.storage(), FLOOR + owned);
        drop(owner);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
    // Exact reserve may grant extra capacity; assert a first growth denial and
    // final reinsert denial, and independently validate every observed growth.
    assert!(seen[0] && saw_reinsert);
}
#[test]
fn retained_place_empty_allocated_output_survives_storage_entry_refusal() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    let (mut owner, prefix) = seeded(false, true, 0);
    let pointer = owner.output.as_ptr();
    let capacity = owner.output.capacity();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, prefix + total_frame() - 1);
    budget.reserve_storage(prefix).unwrap();
    let mut owned = prefix;
    assert!(
        RetainedAliasSessionV1::begin(
            &mut owner,
            &case.function,
            &case.types,
            CAP,
            TREE,
            &mut budget,
            &mut owned
        )
        .is_err()
    );
    assert!(owner.output.is_empty() && capacity > 0);
    assert_eq!(owner.output.as_ptr(), pointer);
    assert_eq!(owner.output.capacity(), capacity);
    assert_eq!(owned, prefix);
    assert!(matches!(owner.failure, Some(Resource::Storage(_))));
    drop(owner);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_place_empty_removed_holder_survives_final_tree_and_reinsert_refusals() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    let full = original(Operation::Deinitialize, &case, LIMIT, true, 0).2;
    for storage in [false, true] {
        let (mut owner, prefix) = seeded(true, false, 0);
        let pointer = owner.data.holders[&7].as_ptr();
        let capacity = owner.data.holders[&7].capacity();
        let mut work = Work::new(if storage { LIMIT } else { 32 + full - 1 });
        let mut budget = Budget::new(
            &mut work,
            if storage {
                prefix + total_frame()
            } else {
                LIMIT
            },
        );
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        {
            let mut session = RetainedAliasSessionV1::begin(
                &mut owner,
                &case.function,
                &case.types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            )
            .unwrap();
            assert!(session.deinitialize(&case.place).is_err());
        }
        let held = owner.removed.as_ref().unwrap();
        assert_eq!(held.local, 7);
        assert_eq!(held.cursor, 0);
        assert!(held.aliases.is_empty() && capacity > 0);
        assert_eq!(held.aliases.as_ptr(), pointer);
        assert_eq!(held.aliases.capacity(), capacity);
        assert!(!owner.data.holders.contains_key(&7));
        assert_eq!(owned, prefix + total_frame());
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
