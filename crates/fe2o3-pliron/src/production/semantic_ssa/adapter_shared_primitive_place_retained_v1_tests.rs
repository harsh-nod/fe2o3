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
include!("adapter_shared_primitive_place_retained_v1_cases_tests.rs");
