//! Original Analysis/Reads DATA oracle and inert retained read controls.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::*;
const LIMIT: usize = 1024 * 1024;
const FLOOR: usize = 19;
const CAP: usize = 64;
const TREE: usize = 256;
const SCRATCH: usize = 97;
const ROW_CAP: usize = 3;

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

struct OriginalMeter<'a, 'w>(&'a mut Budget<'w>);

impl BorrowWork for OriginalMeter<'_, '_> {
    fn work(&mut self, units: usize) -> Result<()> {
        self.0
            .charge_work(units)
            .map_err(|_| Error::ResourceOverflow)
    }
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
struct Read {
    borrow: SemanticTransparentBorrowSiteV1,
    source: u32,
    block: u32,
    statement: u32,
    // Compared only, never dereferenced. The view borrows the immutable owner.
    place: usize,
}

struct Reads {
    rows: Vec<Read>,
    cap: usize,
}

impl ReadObserver for Reads {
    fn read(
        &mut self,
        borrow: SemanticTransparentBorrowSiteV1,
        source: u32,
        site: (u32, u32),
        place: &SemanticPlaceV1,
        meter: &mut impl BorrowWork,
    ) -> Result<()> {
        meter.work(1)?;
        if self.rows.len() == self.cap {
            return Err(ProductionSemanticSsaErrorV1::ResourceOverflow);
        }
        self.rows.push(Read {
            borrow,
            source,
            block: site.0,
            statement: site.1,
            place: place as *const SemanticPlaceV1 as usize,
        });
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
enum Operation {
    Read(bool),
    Operand(bool),
    Scalar(bool),
    Constant,
}
fn operations() -> [Operation; 7] {
    [
        Operation::Read(false),
        Operation::Read(true),
        Operation::Operand(false),
        Operation::Operand(true),
        Operation::Scalar(false),
        Operation::Scalar(true),
        Operation::Constant,
    ]
}
fn operand_for(operation: Operation, place: &SemanticPlaceV1) -> SemanticOperandV1 {
    match operation {
        Operation::Constant => SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty(0),
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 4).unwrap()),
        )),
        Operation::Operand(true) | Operation::Scalar(true) => {
            SemanticOperandV1::Move(place.clone())
        }
        _ => SemanticOperandV1::Copy(place.clone()),
    }
}
type ReadRows = Vec<(u32, u32, u32, u32, u32, usize)>;
fn read_rows(reads: &Reads) -> ReadRows {
    reads
        .rows
        .iter()
        .map(|row| {
            (
                row.borrow.block,
                row.borrow.statement,
                row.source,
                row.block,
                row.statement,
                row.place,
            )
        })
        .collect()
}
fn current_site() -> SemanticTransparentBorrowSiteV1 {
    SemanticTransparentBorrowSiteV1 {
        block: 0,
        statement: 9,
    }
}
fn operate<'a>(
    session: &mut RetainedAliasSessionV1<'_, 'a, '_>,
    operation: Operation,
    place: &'a SemanticPlaceV1,
    operand: &'a SemanticOperandV1,
) -> Result<()> {
    match operation {
        Operation::Read(consume) => session.read_place(place, consume),
        Operation::Operand(_) | Operation::Constant => session.operand(operand),
        Operation::Scalar(_) => session.scalar_operand(operand),
    }
}
fn original(
    operation: Operation,
    case: &Case,
    operand: &SemanticOperandV1,
    work_limit: usize,
    row_cap: usize,
    cap: usize,
    observed: bool,
) -> (
    std::result::Result<AliasRows, String>,
    DataRows,
    ReadRows,
    usize,
    Option<usize>,
) {
    let (owner, _) = seeded(false, false, 0);
    let AliasData {
        candidates,
        holders,
        active,
        alias_words,
        ..
    } = owner.data;
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut meter = OriginalMeter(&mut budget);
    let mut observer = Reads {
        rows: Vec::with_capacity(row_cap),
        cap: row_cap,
    };
    let mut analysis = Analysis {
        function: &case.function,
        types: &case.types,
        meter: &mut meter,
        observer: &mut observer,
        site: observed.then_some(current_site()),
        candidates,
        holders,
        active,
        alias_words,
        cap,
        tree_work: TREE,
        exhausted: false,
    };
    let result = match operation {
        Operation::Read(consume) => analysis.read_place(&case.place, consume),
        Operation::Operand(_) | Operation::Constant => analysis.operand(operand),
        Operation::Scalar(_) => analysis.scalar_operand(operand).map(|()| Vec::new()),
    }
    .map(|selected| aliases(&selected))
    .map_err(|e| format!("{e:?}"));
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
        read_rows(&observer),
        budget.work(),
        budget.failed_work(),
    )
}
fn parked(
    owner: &RetainedAliasStateV1<'_>,
) -> (
    Option<AliasRows>,
    Option<(usize, Vec<u32>, bool)>,
    AliasRows,
    AliasRows,
    Vec<u32>,
) {
    (
        owner.drain.as_ref().map(|drain| aliases(drain.as_slice())),
        owner
            .current
            .as_ref()
            .map(|alias| (alias.candidate, alias.fields.clone(), alias.live)),
        aliases(&owner.output),
        aliases(&owner.selected),
        owner.fields.clone(),
    )
}
fn measure(
    operation: Operation,
    case: &Case,
    operand: &SemanticOperandV1,
) -> (usize, usize, usize) {
    let (mut owner, prefix) = seeded(false, false, 0);
    owner.site = Some(current_site());
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR + prefix).unwrap();
    let mut owned = prefix;
    owner
        .prepare_observer_into(
            &case.function,
            &case.types,
            SCRATCH,
            ROW_CAP,
            &mut budget,
            &mut owned,
        )
        .unwrap();
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
    operate(&mut session, operation, &case.place, operand).unwrap();
    session.finish().unwrap();
    let result = (budget.work(), budget.storage(), prefix);
    drop(owner);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    result
}
#[test]
fn retained_read_all_paths_and_operands_match_original_selected_data_rows_and_work() {
    for case in cases() {
        for operation in operations() {
            let operand = operand_for(operation, &case.place);
            for observed in [false, true] {
                let expected = original(operation, &case, &operand, LIMIT, ROW_CAP, CAP, observed);
                let (mut owner, prefix) = seeded(false, false, 0);
                owner.site = observed.then_some(current_site());
                let mut work = Work::new(LIMIT);
                let mut budget = Budget::new(&mut work, LIMIT);
                budget.reserve_storage(FLOOR + prefix).unwrap();
                let mut owned = prefix;
                owner
                    .prepare_observer_into(
                        &case.function,
                        &case.types,
                        SCRATCH,
                        ROW_CAP,
                        &mut budget,
                        &mut owned,
                    )
                    .unwrap();
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
                let result = operate(&mut session, operation, &case.place, &operand)
                    .map(|()| aliases(&session.owner.selected))
                    .map_err(|e| format!("{e:?}"));
                assert_eq!(result, expected.0, "{} {operation:?} {observed}", case.name);
                session.finish().unwrap();
                assert_eq!(data_rows(&owner.data), expected.1);
                assert_eq!(owner.observer.test_rows(), expected.2);
                assert_eq!(budget.work(), expected.3 + 32);
                assert!(owner.current.is_none() && owner.drain.is_none());
                assert!(owner.output.is_empty() && owner.fields.is_empty());
                assert!(
                    owner
                        .postflight_for(&case.function, &case.types, &budget, &owned)
                        .is_ok()
                );
                assert!(
                    owner
                        .observer
                        .postflight_for(
                            &case.function,
                            &case.types,
                            &budget,
                            &owned,
                            &owner.failure
                        )
                        .is_ok()
                );
                assert_eq!(budget.storage(), FLOOR + owned);
                drop(owner);
                budget.release_storage(owned).unwrap();
                assert_eq!(budget.storage(), FLOOR);
            }
        }
    }
}
#[test]
fn retained_read_every_work_cut_matches_original_partial_state_rows_and_first_denial() {
    let mut saw_selected = false;
    let mut saw_output = false;
    let mut saw_current = false;
    for case in cases().into_iter().filter(|case| {
        matches!(
            case.name,
            "prefix" | "deref" | "missing-field" | "whole" | "nonmatching"
        )
    }) {
        for operation in operations() {
            let operand = operand_for(operation, &case.place);
            let needed = measure(operation, &case, &operand).0;
            for limit in 0..needed {
                let (mut owner, prefix) = seeded(false, false, 0);
                owner.site = Some(current_site());
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, LIMIT);
                budget.reserve_storage(FLOOR + prefix).unwrap();
                let mut owned = prefix;
                owner
                    .prepare_observer_into(
                        &case.function,
                        &case.types,
                        SCRATCH,
                        ROW_CAP,
                        &mut budget,
                        &mut owned,
                    )
                    .unwrap();
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
                        let error = operate(&mut session, operation, &case.place, &operand)
                            .err()
                            .unwrap();
                        Err(session.map_error(error))
                    }
                };
                let RetainedAliasErrorV1::Resource(Resource::Work(first)) = result.err().unwrap()
                else {
                    panic!("wrong refusal");
                };
                assert_eq!(owner.failure, Some(Resource::Work(first)));
                if limit >= 32 {
                    let expected =
                        original(operation, &case, &operand, limit - 32, ROW_CAP, CAP, true);
                    assert!(expected.0.is_err());
                    assert_eq!(
                        data_rows(&owner.data),
                        expected.1,
                        "{} {operation:?} {limit}",
                        case.name
                    );
                    assert_eq!(owner.observer.test_rows(), expected.2);
                    assert_eq!(budget.work(), expected.3 + 32);
                    assert_eq!(first.actual(), expected.4.unwrap() + 32);
                } else {
                    assert_eq!(budget.work(), 0);
                }
                saw_selected |= !owner.selected.is_empty();
                saw_output |= !owner.output.is_empty();
                saw_current |= owner.current.is_some();
                let payload = parked(&owner);
                let rows = owner.observer.test_rows();
                assert!(
                    owner
                        .postflight_for(&case.function, &case.types, &budget, &owned)
                        .is_err()
                );
                assert_eq!(parked(&owner), payload);
                assert_eq!(owner.observer.test_rows(), rows);
                assert_eq!(budget.storage(), FLOOR + owned);
                drop(owner);
                budget.release_storage(owned).unwrap();
                assert_eq!(budget.storage(), FLOOR);
            }
        }
    }
    assert!(saw_selected && saw_output && saw_current);
}
#[test]
fn retained_read_every_storage_cut_keeps_fields_selected_retained_and_current_owners() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    let operation = Operation::Read(false);
    let operand = operand_for(operation, &case.place);
    let (_, needed, prefix) = measure(operation, &case, &operand);
    let mut saw_fields = false;
    let mut saw_selected = false;
    let mut saw_output = false;
    let mut saw_current = false;
    let mut saw_reinsert = false;
    for limit in FLOOR + prefix..needed {
        let (mut owner, _) = seeded(false, false, 0);
        owner.site = Some(current_site());
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(FLOOR + prefix).unwrap();
        let mut owned = prefix;
        let result: RetainedResult<()> = owner
            .prepare_observer_into(
                &case.function,
                &case.types,
                SCRATCH,
                ROW_CAP,
                &mut budget,
                &mut owned,
            )
            .and_then(|()| {
                match RetainedAliasSessionV1::begin(
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
                        let error = session.read_place(&case.place, false).err().unwrap();
                        Err(session.map_error(error))
                    }
                }
            });
        assert!(matches!(
            result,
            Err(RetainedAliasErrorV1::Resource(Resource::Storage(_)))
        ));
        assert_eq!(budget.storage(), FLOOR + owned);
        assert_eq!(owner.phase, Phase::Terminal);
        saw_fields |= !owner.fields.is_empty();
        saw_selected |= !owner.selected.is_empty();
        saw_output |= !owner.output.is_empty();
        saw_current |= owner.current.is_some();
        if owner.current.is_none() && owner.drain.is_none() && !owner.output.is_empty() {
            saw_reinsert = true;
            assert_eq!(
                aliases(&owner.output),
                vec![
                    (0, vec![0], true),
                    (1, vec![1, 1], true),
                    (2, vec![1, 1, 0], true)
                ]
            );
            assert_eq!(
                aliases(&owner.selected),
                vec![(1, vec![1], true), (2, vec![1, 0], true)]
            );
        }
        if let Some(current) = &owner.current {
            let expected = [vec![0], vec![1, 1], vec![1, 1, 0]];
            assert_eq!(current.fields, expected[current.candidate]);
            for alias in owner.drain.as_ref().unwrap().as_slice() {
                assert_eq!(alias.fields, expected[alias.candidate]);
            }
            for alias in &owner.selected {
                assert_eq!(alias.fields, expected[alias.candidate][1..]);
            }
        }
        drop(owner);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
    assert!(saw_fields && saw_selected && saw_output && saw_current && saw_reinsert);
}
#[test]
fn retained_read_zero_observer_cap_preserves_original_read_side_effect_before_refusal() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "deref")
        .unwrap();
    let operation = Operation::Read(false);
    let operand = operand_for(operation, &case.place);
    let expected = original(operation, &case, &operand, LIMIT, 0, CAP, true);
    let (mut owner, prefix) = seeded(false, false, 0);
    owner.site = Some(current_site());
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(prefix).unwrap();
    let mut owned = prefix;
    owner
        .prepare_observer_into(
            &case.function,
            &case.types,
            SCRATCH,
            0,
            &mut budget,
            &mut owned,
        )
        .unwrap();
    let capacity = owner.observer.test_capacity();
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
        let error = session.read_place(&case.place, false).err().unwrap();
        assert_eq!(format!("{error:?}"), expected.0.err().unwrap());
        assert!(matches!(
            session.map_error(error),
            RetainedAliasErrorV1::Original(_)
        ));
    }
    assert_eq!(data_rows(&owner.data), expected.1);
    assert_eq!(owner.observer.test_rows(), expected.2);
    assert_eq!(budget.work(), expected.3 + 32);
    assert!(owner.failure.is_none());
    assert!(owner.data.candidates[1].read);
    assert_eq!(owner.current.as_ref().unwrap().fields, vec![1, 1]);
    assert_eq!(aliases(&owner.output), vec![(0, vec![0], true)]);
    assert_eq!(
        aliases(owner.drain.as_ref().unwrap().as_slice()),
        vec![(2, vec![1, 1, 0], true)]
    );
    assert_eq!(owner.observer.test_capacity(), capacity);
    drop(owner);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_read_expansion_fallback_preserves_original_data_and_empty_selected_owner() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    for operation in [
        Operation::Read(false),
        Operation::Read(true),
        Operation::Scalar(false),
    ] {
        let operand = operand_for(operation, &case.place);
        let expected = original(operation, &case, &operand, LIMIT, ROW_CAP, 9, true);
        let (mut owner, prefix) = seeded(false, false, 0);
        owner.site = Some(current_site());
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        owner
            .prepare_observer_into(
                &case.function,
                &case.types,
                SCRATCH,
                ROW_CAP,
                &mut budget,
                &mut owned,
            )
            .unwrap();
        let mut session = RetainedAliasSessionV1::begin(
            &mut owner,
            &case.function,
            &case.types,
            9,
            TREE,
            &mut budget,
            &mut owned,
        )
        .unwrap();
        operate(&mut session, operation, &case.place, &operand).unwrap();
        session.finish().unwrap();
        assert_eq!(data_rows(&owner.data), expected.1);
        assert_eq!(budget.work(), expected.3 + 32);
        assert!(owner.data.exhausted);
        assert!(owner.selected.is_empty());
        assert!(owner.failure.is_none());
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_read_terminal_guards_preserve_every_payload_and_observer_after_either_error() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "deref")
        .unwrap();
    let constant = operand_for(Operation::Constant, &case.place);
    for resource in [false, true] {
        let (mut owner, prefix) = seeded(false, false, 0);
        owner.site = Some(current_site());
        let mut work = Work::new(if resource { 33 } else { LIMIT });
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        owner
            .prepare_observer_into(
                &case.function,
                &case.types,
                SCRATCH,
                0,
                &mut budget,
                &mut owned,
            )
            .unwrap();
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
        assert!(session.read_place(&case.place, false).is_err());
        let before = (
            parked(session.owner),
            data_rows(&session.owner.data),
            session.owner.observer.test_rows(),
            session.owner.observer.test_capacity(),
            session.owner.failure,
            snapshot(session.budget, session.owned),
        );
        assert!(session.read_place(&case.place, false).is_err());
        assert!(session.operand(&constant).is_err());
        assert!(session.scalar_operand(&constant).is_err());
        assert!(session.read_alias_capacity(ReadOutput::Selected).is_err());
        assert!(session.read_field_capacity(1).is_err());
        let after = (
            parked(session.owner),
            data_rows(&session.owner.data),
            session.owner.observer.test_rows(),
            session.owner.observer.test_capacity(),
            session.owner.failure,
            snapshot(session.budget, session.owned),
        );
        assert!(before == after);
        assert!(session.finish().is_err());
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_read_empty_selected_and_field_allocations_survive_entry_and_work_refusal() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    for limit in [0, 32, 33] {
        let (mut owner, mut prefix) = seeded(false, false, 0);
        owner.selected = Vec::with_capacity(3);
        owner.fields = Vec::with_capacity(5);
        prefix += owner.selected.capacity() * size_of::<Alias>()
            + owner.fields.capacity() * size_of::<u32>();
        let payload = (
            owner.selected.as_ptr(),
            owner.selected.capacity(),
            owner.fields.as_ptr(),
            owner.fields.capacity(),
        );
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        owner
            .prepare_observer_into(
                &case.function,
                &case.types,
                SCRATCH,
                ROW_CAP,
                &mut budget,
                &mut owned,
            )
            .unwrap();
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
                assert!(session.read_place(&case.place, false).is_err());
            }
        }
        assert!(owner.selected.is_empty() && owner.fields.is_empty());
        assert_eq!(
            payload,
            (
                owner.selected.as_ptr(),
                owner.selected.capacity(),
                owner.fields.as_ptr(),
                owner.fields.capacity()
            )
        );
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_read_pending_selected_input_is_not_overwritten_by_another_operand() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    let constant = operand_for(Operation::Constant, &case.place);
    for scalar in [false, true] {
        let (mut owner, prefix) = seeded(false, false, 0);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        owner
            .prepare_observer_into(
                &case.function,
                &case.types,
                SCRATCH,
                ROW_CAP,
                &mut budget,
                &mut owned,
            )
            .unwrap();
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
        session.read_place(&case.place, false).unwrap();
        let selected = aliases(&session.owner.selected);
        assert!(!selected.is_empty());
        let before = snapshot(session.budget, session.owned);
        if scalar {
            assert!(session.scalar_operand(&constant).is_err());
        } else {
            assert!(session.operand(&constant).is_err());
        }
        assert_eq!(aliases(&session.owner.selected), selected);
        assert!(before == snapshot(session.budget, session.owned));
        drop(session);
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_read_observer_preparation_is_one_shot_and_keeps_existing_alias_payloads() {
    let case = cases().remove(0);
    let (mut owner, prefix) = seeded(false, false, 0);
    let before = data_rows(&owner.data);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(prefix).unwrap();
    let mut owned = prefix;
    owner
        .prepare_observer_into(
            &case.function,
            &case.types,
            SCRATCH,
            ROW_CAP,
            &mut budget,
            &mut owned,
        )
        .unwrap();
    let capacity = owner.observer.test_capacity();
    let checkpoint = snapshot(&budget, &owned);
    assert!(
        owner
            .prepare_observer_into(&case.function, &case.types, 0, 0, &mut budget, &mut owned)
            .is_err()
    );
    assert_eq!(owner.phase, Phase::Terminal);
    assert_eq!(data_rows(&owner.data), before);
    assert_eq!(owner.observer.test_capacity(), capacity);
    assert!(checkpoint == snapshot(&budget, &owned));
    drop(owner);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_read_caller_unwind_retains_selected_fields_and_real_observer_rows() {
    for name in ["prefix", "deref"] {
        let case = cases().into_iter().find(|case| case.name == name).unwrap();
        let (mut owner, prefix) = seeded(false, false, 0);
        owner.site = Some(current_site());
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        owner
            .prepare_observer_into(
                &case.function,
                &case.types,
                SCRATCH,
                ROW_CAP,
                &mut budget,
                &mut owned,
            )
            .unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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
            session.read_place(&case.place, false).unwrap();
            panic!("inert outer caller unwind");
        }));
        assert!(result.is_err());
        assert_eq!(owner.phase, Phase::Terminal);
        if name == "prefix" {
            assert_eq!(
                aliases(&owner.selected),
                vec![(1, vec![1], true), (2, vec![1, 0], true)]
            );
        } else {
            assert_eq!(owner.observer.test_rows().len(), 1);
        }
        assert_eq!(budget.storage(), owned);
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_read_source_order_matches_real_selected_and_observer_construction_points() {
    let source: String = include_str!("adapter_shared_primitive_read_retained_v1.rs")
        .split_whitespace()
        .collect::<String>()
        .replace(",)", ")");
    let read = source
        .split("fnread_place")
        .nth(1)
        .unwrap()
        .split("fnoperand")
        .next()
        .unwrap();
    let reserve = read.find("self.reserve(1+length)?").unwrap();
    let work = read
        .find("alias_work(self.budget,&mutself.owner.failure,length)?")
        .unwrap();
    let live = read.find("self.add_live(index)?").unwrap();
    let fields = read.find("self.read_field_capacity(length)?").unwrap();
    let copy = read.find("self.owner.fields.extend_from_slice").unwrap();
    let selected = read
        .find("self.read_alias_capacity(ReadOutput::Selected)?")
        .unwrap();
    let push = read.find("self.owner.selected.push").unwrap();
    let consume = read.find("self.deactivate(Slot::Current)?").unwrap();
    assert!(
        reserve < work
            && work < live
            && live < fields
            && fields < copy
            && copy < selected
            && selected < push
            && push < consume
    );
    assert!(
        read.find("candidate.read=true").unwrap()
            < read.find("self.owner.observer.record").unwrap()
    );
    let scalar = source
        .split("fnscalar_operand")
        .nth(1)
        .unwrap()
        .split("fnread_alias_capacity")
        .next()
        .unwrap();
    assert!(
        scalar.find("self.owner.drain=Some").unwrap()
            < scalar.find("self.discard_pending(true)").unwrap()
    );
    assert!(!source.contains("Budget::new("));
    assert!(!source.contains("release_storage("));
}

#[test]
fn retained_read_prepared_observer_binding_is_checked_before_session_work() {
    let case = cases().remove(0);
    let other_function = case.function.clone();
    let other_types = case.types.clone();
    for changed_types in [false, true] {
        let (mut owner, prefix) = seeded(false, false, 0);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        owner
            .prepare_observer_into(
                &case.function,
                &case.types,
                SCRATCH,
                ROW_CAP,
                &mut budget,
                &mut owned,
            )
            .unwrap();
        let before = (
            data_rows(&owner.data),
            owner.observer.test_rows(),
            owner.observer.test_capacity(),
            snapshot(&budget, &owned),
        );
        let function = if changed_types {
            &case.function
        } else {
            &other_function
        };
        let types = if changed_types {
            &other_types[..]
        } else {
            &case.types[..]
        };
        assert!(
            RetainedAliasSessionV1::begin(
                &mut owner,
                function,
                types,
                CAP,
                TREE,
                &mut budget,
                &mut owned
            )
            .is_err()
        );
        let after = (
            data_rows(&owner.data),
            owner.observer.test_rows(),
            owner.observer.test_capacity(),
            snapshot(&budget, &owned),
        );
        assert!(before == after);
        assert_eq!(budget.work(), 0);
        assert_eq!(owner.phase, Phase::Terminal);
        assert_eq!(owner.failure, Some(Resource::Accounting));
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
