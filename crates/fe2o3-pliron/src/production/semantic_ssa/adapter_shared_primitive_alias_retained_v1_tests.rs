//! Private inert DATA seeds only; no admitted ownership, SSA or real producer.
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

#[derive(Clone, Copy, Debug)]
enum Operation {
    Tree,
    Reserve,
    Exhaust,
    AddLive,
    Deactivate,
    Release,
    Discard,
    Poison,
    SourceWrite,
    PoisonHolder,
    EndLocal,
    MalformedActive,
    MalformedWords,
}
fn alias_row(candidate: usize) -> Alias {
    Alias {
        candidate,
        fields: if candidate == 0 { vec![1] } else { vec![1, 1] },
        live: true,
    }
}
fn seeded<'a>(operation: Operation) -> (RetainedAliasStateV1<'a>, usize) {
    let mut owner = RetainedAliasStateV1::new();
    owner.data.candidates = (0..2)
        .map(|index| Candidate {
            site: SemanticTransparentBorrowSiteV1 {
                block: 0,
                statement: index as u32,
            },
            source: if index == 0 { 1 } else { 5 },
            pointee: ty(0),
            reference: ty(1),
            live: 1,
            valid: true,
            read: index == 0,
        })
        .collect();
    owner
        .data
        .holders
        .insert(2, vec![alias_row(0), alias_row(1)]);
    owner.data.active = [
        (1, [0usize].into_iter().collect()),
        (5, [1usize].into_iter().collect()),
    ]
    .into_iter()
    .collect();
    owner.data.alias_words = 5;
    let mut prefix = owner.data.candidates.capacity() * size_of::<Candidate>();
    for aliases in owner.data.holders.values() {
        prefix += size_of::<(u32, Vec<Alias>)>() + aliases.capacity() * size_of::<Alias>();
        for alias in aliases {
            prefix += alias.fields.capacity() * size_of::<u32>();
        }
    }
    for values in owner.data.active.values() {
        prefix += size_of::<(u32, BTreeSet<usize>)>() + values.len() * size_of::<usize>();
    }
    match operation {
        Operation::Deactivate
        | Operation::Release
        | Operation::MalformedActive
        | Operation::MalformedWords => {
            owner.current = Some(owner.data.holders.get_mut(&2).unwrap().remove(0));
        }
        Operation::Discard | Operation::Poison => {
            owner.drain = Some(owner.data.holders.remove(&2).unwrap().into_iter());
        }
        Operation::AddLive => {
            owner.data.candidates[0].live = 0;
            owner.data.active.remove(&1);
            owner.data.holders.get_mut(&2).unwrap()[0].live = false;
        }
        _ => {}
    }
    if matches!(operation, Operation::MalformedActive) {
        owner.data.active.remove(&1);
    }
    if matches!(operation, Operation::MalformedWords) {
        owner.data.alias_words = 0;
    }
    (owner, prefix)
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
fn parked_rows(
    owner: &RetainedAliasStateV1<'_>,
) -> (
    Option<(u32, usize, AliasRows)>,
    Option<AliasRows>,
    Option<(usize, Vec<u32>, bool)>,
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
    )
}
fn operate(session: &mut RetainedAliasSessionV1<'_, '_, '_>, operation: Operation) -> Result<bool> {
    match operation {
        Operation::Tree => session.tree().map(|()| true),
        Operation::Reserve => session.reserve(3),
        Operation::Exhaust => session.reserve(CAP + 1),
        Operation::AddLive => session.add_live(0).map(|()| true),
        Operation::Deactivate => session.deactivate(Slot::Current).map(|()| true),
        Operation::Release | Operation::MalformedActive | Operation::MalformedWords => {
            session.release_current().map(|()| true)
        }
        Operation::Discard => session.discard_pending(false).map(|()| true),
        Operation::Poison => session.discard_pending(true).map(|()| true),
        Operation::SourceWrite => session.source_write(1).map(|()| true),
        Operation::PoisonHolder => session.poison_holder(2).map(|()| true),
        Operation::EndLocal => session.end_local(2).map(|()| true),
    }
}
struct OriginalMeter<'a, 'w>(&'a mut Budget<'w>);
impl BorrowWork for OriginalMeter<'_, '_> {
    fn work(&mut self, units: usize) -> Result<()> {
        self.0
            .charge_work(units)
            .map_err(|_| Error::ResourceOverflow)
    }
}
fn original(
    operation: Operation,
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    work_limit: usize,
) -> (
    std::result::Result<bool, String>,
    DataRows,
    usize,
    Option<usize>,
) {
    let (seed, _) = seeded(operation);
    let AliasData {
        candidates,
        holders,
        active,
        alias_words,
        ..
    } = seed.data;
    let mut current = seed.current;
    let pending: Vec<_> = seed.drain.into_iter().flatten().collect();
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut meter = OriginalMeter(&mut budget);
    let mut observer = NoReads;
    let mut analysis = Analysis {
        function,
        types,
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
        Operation::Tree => analysis.tree().map(|()| true),
        Operation::Reserve => analysis.reserve(3),
        Operation::Exhaust => analysis.reserve(CAP + 1),
        Operation::AddLive => analysis.add_live(0).map(|()| true),
        Operation::Deactivate => analysis
            .deactivate(current.as_mut().unwrap())
            .map(|()| true),
        Operation::Release | Operation::MalformedActive | Operation::MalformedWords => {
            analysis.release(current.take().unwrap()).map(|()| true)
        }
        Operation::Discard => analysis.discard(pending, false).map(|()| true),
        Operation::Poison => analysis.discard(pending, true).map(|()| true),
        Operation::SourceWrite => analysis.source_write(1).map(|()| true),
        Operation::PoisonHolder => analysis.poison_holder(2).map(|()| true),
        Operation::EndLocal => analysis.end_local(2).map(|()| true),
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
fn all_operations() -> [Operation; 11] {
    [
        Operation::Tree,
        Operation::Reserve,
        Operation::Exhaust,
        Operation::AddLive,
        Operation::Deactivate,
        Operation::Release,
        Operation::Discard,
        Operation::Poison,
        Operation::SourceWrite,
        Operation::PoisonHolder,
        Operation::EndLocal,
    ]
}
fn measure(
    operation: Operation,
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
) -> (usize, usize, usize) {
    let (mut owner, prefix) = seeded(operation);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR + prefix).unwrap();
    let mut owned = prefix;
    let mut session = RetainedAliasSessionV1::begin(
        &mut owner,
        function,
        types,
        CAP,
        TREE,
        &mut budget,
        &mut owned,
    )
    .unwrap();
    operate(&mut session, operation).unwrap();
    session.finish().unwrap();
    let values = (budget.work(), budget.storage(), prefix);
    drop(owner);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    values
}
include!("adapter_shared_primitive_alias_retained_v1_cases_tests.rs");
