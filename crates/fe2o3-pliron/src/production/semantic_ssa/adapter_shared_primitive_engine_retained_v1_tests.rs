//! Full original-analysis oracle, complete DATA and source-driven controls.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::*;

fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}
fn source() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn local(index: u32) -> SemanticLocalIdV1 {
    SemanticLocalIdV1::from_index(index)
}
fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(source(), kind)
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
fn field_read(index: u32) -> SemanticPlaceV1 {
    projected(
        index,
        &[
            (SemanticProjectionKindV1::Field(1), 1),
            (SemanticProjectionKindV1::Dereference, 0),
        ],
    )
}
fn constant(kind: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty(kind),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(17, 4).unwrap()),
    ))
}
fn assign(destination: SemanticPlaceV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    let result = destination.ty();
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        destination,
        SemanticRvalueV1::new(result, value),
    )))
}
fn assign_operand(destination: SemanticPlaceV1, value: SemanticOperandV1) -> SemanticStatementV1 {
    assign(destination, SemanticRvalueKindV1::Use(value))
}
fn borrow(
    destination: u32,
    reference: u32,
    referent: u32,
    pointee: u32,
    kind: SemanticBorrowKindV1,
) -> SemanticStatementV1 {
    assign(
        place(destination, reference),
        SemanticRvalueKindV1::Borrow {
            kind,
            place: place(referent, pointee),
        },
    )
}
fn shared() -> SemanticStatementV1 {
    borrow(2, 1, 1, 0, SemanticBorrowKindV1::Shared)
}
fn read(index: u32) -> SemanticStatementV1 {
    assign_operand(place(5, 0), SemanticOperandV1::Copy(dereference(index, 0)))
}
fn dead(index: u32) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::StorageDead(local(index)))
}
fn tuple_holder() -> SemanticStatementV1 {
    assign(
        place(4, 2),
        SemanticRvalueKindV1::aggregate(
            SemanticAggregateKindV1::Tuple,
            vec![constant(0), SemanticOperandV1::Move(place(2, 1))],
        )
        .unwrap(),
    )
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

const LIMIT: usize = 8 * 1024 * 1024;
const FLOOR: usize = 23;
const ROWS: usize = 64;
const SCRATCH: usize = 97;

fn rows(reads: &Reads) -> Vec<(u32, u32, u32, u32, u32, usize)> {
    reads
        .rows
        .iter()
        .map(|r| {
            (
                r.borrow.block,
                r.borrow.statement,
                r.source,
                r.block,
                r.statement,
                r.place,
            )
        })
        .collect()
}
fn edge(target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(
        SemanticEdgeRoleV1::Goto,
        SemanticBlockIdV1::from_index(target),
    )
}
fn corpus() -> Vec<(
    &'static str,
    SemanticFunctionDeclV1,
    Vec<SemanticTypeDeclV1>,
)> {
    let mut result = vec![
        ("empty", function(vec![]), types()),
        (
            "direct",
            function(vec![shared(), read(2), dead(2)]),
            types(),
        ),
        (
            "copy",
            function(vec![
                shared(),
                assign_operand(place(3, 1), SemanticOperandV1::Copy(place(2, 1))),
                read(2),
                read(3),
                dead(2),
                dead(3),
            ]),
            types(),
        ),
        (
            "tuple",
            function(vec![
                shared(),
                tuple_holder(),
                assign_operand(place(5, 0), SemanticOperandV1::Copy(field_read(4))),
                dead(4),
            ]),
            types(),
        ),
        (
            "self-assignment",
            function(vec![
                shared(),
                tuple_holder(),
                assign_operand(place(4, 2), SemanticOperandV1::Copy(place(4, 2))),
                assign_operand(place(5, 0), SemanticOperandV1::Copy(field_read(4))),
            ]),
            types(),
        ),
        (
            "rhs-referent-write",
            function(vec![
                shared(),
                assign_operand(place(1, 0), SemanticOperandV1::Copy(dereference(2, 0))),
                read(2),
            ]),
            types(),
        ),
        (
            "wrong-aggregate",
            function(vec![
                shared(),
                assign(
                    place(4, 2),
                    SemanticRvalueKindV1::aggregate(
                        SemanticAggregateKindV1::Tuple,
                        vec![constant(8), SemanticOperandV1::Move(place(2, 1))],
                    )
                    .unwrap(),
                ),
                read(2),
            ]),
            types(),
        ),
        (
            "invalid-write",
            function(vec![
                shared(),
                assign_operand(place(3, 0), SemanticOperandV1::Copy(place(2, 1))),
                read(2),
            ]),
            types(),
        ),
        (
            "deinit",
            function(vec![
                shared(),
                read(2),
                statement(SemanticStatementKindV1::Deinitialize(place(2, 1))),
            ]),
            types(),
        ),
        (
            "storage-live",
            function(vec![
                shared(),
                read(2),
                statement(SemanticStatementKindV1::StorageLive(local(2))),
            ]),
            types(),
        ),
        (
            "set-discriminant",
            function(vec![
                shared(),
                read(2),
                statement(SemanticStatementKindV1::SetDiscriminant {
                    place: place(2, 1),
                    variant_index: 0,
                }),
            ]),
            types(),
        ),
        (
            "missing-local",
            function(vec![assign_operand(place(99, 0), constant(0))]),
            types(),
        ),
    ];
    let pointer = || SemanticOperandV1::Copy(place(2, 1));
    let scalar = || SemanticOperandV1::Copy(dereference(2, 0));
    for (name, value) in [
        (
            "unary",
            SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::Not,
                operand: pointer(),
            },
        ),
        (
            "binary",
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Add,
                left: pointer(),
                right: pointer(),
            },
        ),
        (
            "checked",
            SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                SemanticCheckedBinaryOpV1::Add,
                pointer(),
                pointer(),
            )),
        ),
        (
            "unchecked",
            SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
                SemanticUncheckedBinaryOpV1::Add,
                pointer(),
                pointer(),
            )),
        ),
        (
            "cast",
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::PointerExposeProvenance,
                operand: pointer(),
            },
        ),
        (
            "load",
            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                dereference(2, 0),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        ),
        ("length", SemanticRvalueKindV1::Length(place(2, 1))),
        (
            "discriminant",
            SemanticRvalueKindV1::Discriminant(place(2, 1)),
        ),
        (
            "address",
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: place(2, 1),
            },
        ),
        (
            "fake-borrow",
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Fake,
                place: place(2, 1),
            },
        ),
    ] {
        result.push((
            name,
            function(vec![shared(), read(2), assign(place(5, 0), value)]),
            types(),
        ));
    }
    let access = SemanticAtomicAccessV1::new(
        SemanticAtomicOrderingV1::Relaxed,
        SemanticAtomicScopeV1::Device,
    );
    for (name, effect) in [
        (
            "store",
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                place(3, 1),
                pointer(),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        ),
        (
            "atomic-rmw",
            SemanticStatementKindV1::AtomicRmw(SemanticAtomicRmwV1::new(
                place(5, 0),
                dereference(2, 0),
                scalar(),
                SemanticAtomicRmwOpV1::Add,
                access,
            )),
        ),
        (
            "compare-exchange",
            SemanticStatementKindV1::AtomicCompareExchange(SemanticAtomicCompareExchangeV1::new(
                place(5, 0),
                dereference(2, 0),
                scalar(),
                pointer(),
                access,
                SemanticAtomicOrderingV1::Relaxed,
                false,
            )),
        ),
        ("assume", SemanticStatementKindV1::Assume(pointer())),
        ("nop", SemanticStatementKindV1::Nop),
    ] {
        result.push((
            name,
            function(vec![shared(), read(2), statement(effect)]),
            types(),
        ));
    }
    for (name, terminator) in [
        (
            "call",
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(0),
                    vec![pointer(), scalar()],
                    None,
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        ),
        (
            "tail-call",
            SemanticTerminatorKindV1::TailCall(
                SemanticDirectTailCallV1::new(
                    SemanticFunctionIdV1::from_index(0),
                    vec![pointer(), scalar()],
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        ),
        (
            "drop",
            SemanticTerminatorKindV1::Drop {
                place: place(2, 1),
                drop_glue: SemanticFunctionIdV1::from_index(0),
                target: SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::DropReturn,
                    SemanticBlockIdV1::from_index(0),
                ),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        (
            "switch",
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: pointer(),
                targets: SemanticSwitchTargetsV1::new(
                    vec![],
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::SwitchOtherwise,
                        SemanticBlockIdV1::from_index(0),
                    ),
                )
                .unwrap(),
            },
        ),
        ("goto-cycle", SemanticTerminatorKindV1::Goto(edge(0))),
        ("abort", SemanticTerminatorKindV1::Abort),
    ] {
        result.push((
            name,
            function_with_end(vec![shared(), read(2)], terminator),
            types(),
        ));
    }
    for (name, message) in [
        (
            "assert-bounds",
            SemanticAssertMessageV1::BoundsCheck {
                length: pointer(),
                index: scalar(),
            },
        ),
        (
            "assert-overflow",
            SemanticAssertMessageV1::Overflow {
                operation: SemanticBinaryOpV1::Add,
                left: pointer(),
                right: scalar(),
            },
        ),
        (
            "assert-division",
            SemanticAssertMessageV1::DivisionByZero(pointer()),
        ),
        (
            "assert-remainder",
            SemanticAssertMessageV1::RemainderByZero(pointer()),
        ),
        (
            "assert-alignment",
            SemanticAssertMessageV1::MisalignedPointerDereference {
                required_alignment: pointer(),
                found_alignment: scalar(),
            },
        ),
        (
            "assert-null",
            SemanticAssertMessageV1::NullPointerDereference,
        ),
        (
            "assert-resumed-return",
            SemanticAssertMessageV1::ResumedAfterReturn,
        ),
        (
            "assert-resumed-panic",
            SemanticAssertMessageV1::ResumedAfterPanic,
        ),
    ] {
        result.push((
            name,
            function_with_end(
                vec![shared(), read(2)],
                SemanticTerminatorKindV1::Assert {
                    condition: constant(0),
                    expected: true,
                    message,
                    target: SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::AssertSuccess,
                        SemanticBlockIdV1::from_index(0),
                    ),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ),
            types(),
        ));
    }
    result.push(("two-blocks", two_blocks(), types()));
    result
}
fn two_blocks() -> SemanticFunctionDeclV1 {
    function_with_locals(
        &[0, 0, 1, 1, 2, 0, 3, 4, 9, 8, 5, 6, 7, 8],
        vec![
            block(
                0,
                vec![shared(), read(2), dead(2)],
                SemanticTerminatorKindV1::Goto(edge(1)),
            ),
            block(
                1,
                vec![
                    shared(),
                    tuple_holder(),
                    assign_operand(place(5, 0), SemanticOperandV1::Copy(field_read(4))),
                ],
                SemanticTerminatorKindV1::Return,
            ),
        ],
        SemanticFunctionRoleV1::KernelRoot,
    )
}
struct Oracle {
    outcome: std::result::Result<BTreeSet<SemanticTransparentBorrowSiteV1>, String>,
    blocks: Vec<DataRows>,
    rows: Vec<(u32, u32, u32, u32, u32, usize)>,
    work: usize,
    denied: Option<usize>,
}
fn oracle(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    cap: usize,
    row_cap: usize,
    limit: usize,
) -> Oracle {
    let mut work = Work::new(limit);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut meter = OriginalMeter(&mut budget);
    let mut observer = Reads {
        rows: Vec::with_capacity(row_cap),
        cap: row_cap,
    };
    let mut blocks = Vec::new();
    let outcome = (|| {
        let height = (usize::BITS - cap.max(1).leading_zeros()) as usize + 1;
        let tree_work = height.checked_mul(32).ok_or(Error::ResourceOverflow)?;
        let mut result = BTreeSet::new();
        let schedule = liveness::Schedule::new(function, &mut meter)?;
        for (block_index, block) in function.blocks().iter().enumerate() {
            meter.work(1)?;
            let mut analysis = Analysis {
                function,
                types,
                meter: &mut meter,
                observer: &mut observer,
                site: None,
                candidates: Vec::new(),
                holders: BTreeMap::new(),
                active: BTreeMap::new(),
                alias_words: 0,
                cap,
                tree_work,
                exhausted: false,
            };
            let outcome: Result<bool> = (|| {
                for (statement_index, statement) in block.statements().iter().enumerate() {
                    let site = SemanticTransparentBorrowSiteV1 {
                        block: u32::try_from(block_index).map_err(|_| Error::ResourceOverflow)?,
                        statement: u32::try_from(statement_index)
                            .map_err(|_| Error::ResourceOverflow)?,
                    };
                    analysis.statement(site, statement.kind())?;
                    schedule.completed(&mut analysis, site, statement.kind())?;
                    if analysis.exhausted {
                        return Ok(true);
                    }
                }
                analysis.close_block(block.terminator().kind())?;
                if analysis.exhausted {
                    return Ok(true);
                }
                analysis.meter.work(
                    analysis
                        .alias_words
                        .checked_mul(4)
                        .ok_or(Error::ResourceOverflow)?,
                )?;
                // Non-owning observation only: same old charge/predicate/insert;
                // retain original DATA so a failing iteration is observable.
                for candidate in &analysis.candidates {
                    analysis.meter.work(tree_work)?;
                    if candidate.valid && candidate.read {
                        result.insert(candidate.site);
                    }
                }
                Ok(false)
            })();
            let data = AliasData {
                candidates: analysis.candidates,
                holders: analysis.holders,
                active: analysis.active,
                alias_words: analysis.alias_words,
                cap: analysis.cap,
                tree_work: analysis.tree_work,
                exhausted: analysis.exhausted,
            };
            blocks.push(data_rows(&data));
            if outcome? {
                return Ok(BTreeSet::new());
            }
        }
        Ok(result)
    })()
    .map_err(|error: Error| format!("{error:?}"));
    Oracle {
        outcome,
        blocks,
        rows: rows(&observer),
        work: budget.work(),
        denied: budget.failed_work(),
    }
}
fn whole_original(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    cap: usize,
    row_cap: usize,
    limit: usize,
) -> (
    std::result::Result<BTreeSet<SemanticTransparentBorrowSiteV1>, String>,
    Vec<(u32, u32, u32, u32, u32, usize)>,
    usize,
    Option<usize>,
) {
    let mut work = Work::new(limit);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut meter = OriginalMeter(&mut budget);
    let mut reads = Reads {
        rows: Vec::with_capacity(row_cap),
        cap: row_cap,
    };
    let result = analyze_observed(function, types, cap, &mut meter, &mut reads)
        .map_err(|e| format!("{e:?}"));
    (result, rows(&reads), budget.work(), budget.failed_work())
}
fn semantic_error(error: RetainedAliasErrorV1) -> String {
    match error {
        RetainedAliasErrorV1::Original(error) => format!("{error:?}"),
        RetainedAliasErrorV1::Resource(_) => format!("{:?}", Error::ResourceOverflow),
    }
}
fn engine_data(engine: &RetainedSharedEngineV1<'_>) -> Vec<DataRows> {
    engine
        .retired
        .iter()
        .map(data_rows)
        .chain(std::iter::once(data_rows(&engine.aliases.data)))
        .collect()
}
fn run(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    cap: usize,
    row_cap: usize,
) -> (usize, usize) {
    let expected = oracle(function, types, cap, row_cap, LIMIT);
    let original = whole_original(function, types, cap, row_cap, LIMIT);
    assert_eq!(
        (
            &expected.outcome,
            &expected.rows,
            expected.work,
            expected.denied
        ),
        (&original.0, &original.1, original.2, original.3)
    );
    let mut engine = RetainedSharedEngineV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    engine
        .prepare_observer_into(function, types, SCRATCH, row_cap, &mut budget, &mut owned)
        .unwrap();
    let result = engine
        .analyze_into(function, types, cap, &mut budget, &mut owned)
        .map(|()| {
            engine
                .accepted_for(function, types, &budget, &owned)
                .unwrap()
                .clone()
        })
        .map_err(semantic_error);
    assert_eq!(result, expected.outcome);
    assert_eq!(engine.aliases.observer.test_rows(), expected.rows);
    assert_eq!(budget.work(), expected.work + 64);
    if !expected.blocks.is_empty() {
        assert_eq!(engine_data(&engine), expected.blocks);
    }
    if result.is_ok() {
        assert_eq!(engine.phase, EnginePhase::Complete);
        assert!(engine.aliases.rhs.is_empty() && engine.aliases.installed.is_none());
        assert!(engine.aliases.current.is_none() && engine.aliases.drain.is_none());
    } else {
        assert_eq!(engine.phase, EnginePhase::Terminal);
    }
    assert_eq!(budget.storage(), FLOOR + owned);
    let measured = (budget.work(), budget.storage());
    drop(engine);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    measured
}
#[test]
fn retained_engine_all_source_producers_match_unchanged_whole_oracle_data_rows_and_work() {
    for (name, function, types) in corpus() {
        for cap in [0, 1, 4, 64] {
            let _context = (name, cap);
            run(&function, &types, cap, ROWS);
        }
    }
}
#[test]
fn retained_engine_every_work_cut_matches_original_partial_data_rows_and_first_denial() {
    for (name, function, types) in corpus().into_iter().filter(|(name, _, _)| {
        matches!(
            *name,
            "direct" | "tuple" | "self-assignment" | "two-blocks" | "assert-bounds"
        )
    }) {
        let needed = run(&function, &types, 64, ROWS).0;
        for limit in 0..needed {
            let mut engine = RetainedSharedEngineV1::new();
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR).unwrap();
            let mut owned = 0;
            engine
                .prepare_observer_into(&function, &types, SCRATCH, ROWS, &mut budget, &mut owned)
                .unwrap();
            let result = engine.analyze_into(&function, &types, 64, &mut budget, &mut owned);
            assert!(
                matches!(
                    result,
                    Err(RetainedAliasErrorV1::Resource(Resource::Work(_)))
                ),
                "{name} {limit}"
            );
            assert_eq!(engine.phase, EnginePhase::Terminal);
            if limit >= 64 {
                let expected = oracle(&function, &types, 64, ROWS, limit - 64);
                assert!(expected.outcome.is_err());
                assert_eq!(budget.work(), expected.work + 64, "{name} {limit}");
                assert_eq!(
                    budget.failed_work(),
                    expected.denied.map(|actual| actual + 64)
                );
                assert_eq!(engine.aliases.observer.test_rows(), expected.rows);
                if !expected.blocks.is_empty() {
                    assert_eq!(engine_data(&engine), expected.blocks, "{name} {limit}");
                }
            }
            assert_eq!(budget.storage(), FLOOR + owned);
            let before = (
                engine_data(&engine),
                engine.aliases.observer.test_rows(),
                budget.work(),
                budget.storage(),
                owned,
            );
            assert!(
                engine
                    .analyze_into(&function, &types, 64, &mut budget, &mut owned)
                    .is_err()
            );
            assert!(
                engine
                    .prepare_observer_into(&function, &types, 0, 0, &mut budget, &mut owned)
                    .is_err()
            );
            assert_eq!(
                before,
                (
                    engine_data(&engine),
                    engine.aliases.observer.test_rows(),
                    budget.work(),
                    budget.storage(),
                    owned
                )
            );
            drop(engine);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}
fn pending(
    engine: &RetainedSharedEngineV1<'_>,
) -> (
    AliasRows,
    Option<AliasRows>,
    Vec<u32>,
    Option<AliasRows>,
    Option<(usize, Vec<u32>, bool)>,
    AliasRows,
    AliasRows,
    Vec<u32>,
) {
    let owner = &engine.aliases;
    (
        aliases(&owner.rhs),
        owner.installed.as_ref().map(|v| aliases(v)),
        owner.replacement.clone(),
        owner.drain.as_ref().map(|v| aliases(v.as_slice())),
        owner
            .current
            .as_ref()
            .map(|a| (a.candidate, a.fields.clone(), a.live)),
        aliases(&owner.output),
        aliases(&owner.selected),
        owner.fields.clone(),
    )
}
#[test]
fn retained_engine_every_storage_cut_keeps_all_partial_owners_and_coupled_credits() {
    let mut rhs = false;
    let mut installed = false;
    let mut replacement = false;
    let mut selected = false;
    let mut current = false;
    let mut retired = false;
    // Fresh destinations in two-blocks never debit storage while RHS is live.
    // A two-reference aggregate retains its first real RHS alias while reading
    // the second operand; add_live admits active-map storage at that point.
    let mut cases: Vec<_> = corpus()
        .into_iter()
        .filter(|(name, _, _)| matches!(*name, "two-blocks" | "self-assignment"))
        .collect();
    cases.push((
        "two-alias-aggregate",
        function(vec![
            shared(),
            assign(
                place(6, 3),
                SemanticRvalueKindV1::aggregate(
                    SemanticAggregateKindV1::Tuple,
                    vec![
                        SemanticOperandV1::Copy(place(2, 1)),
                        SemanticOperandV1::Copy(place(2, 1)),
                    ],
                )
                .unwrap(),
            ),
            assign_operand(
                place(5, 0),
                SemanticOperandV1::Copy(projected(
                    6,
                    &[
                        (SemanticProjectionKindV1::Field(0), 1),
                        (SemanticProjectionKindV1::Dereference, 0),
                    ],
                )),
            ),
            assign_operand(
                place(5, 0),
                SemanticOperandV1::Copy(projected(
                    6,
                    &[
                        (SemanticProjectionKindV1::Field(1), 1),
                        (SemanticProjectionKindV1::Dereference, 0),
                    ],
                )),
            ),
            dead(6),
        ]),
        types(),
    ));
    for (name, function, types) in cases {
        let required = run(&function, &types, 64, ROWS).1;
        for limit in FLOOR..required {
            let mut engine = RetainedSharedEngineV1::new();
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(FLOOR).unwrap();
            let mut owned = 0;
            let result = engine
                .prepare_observer_into(&function, &types, SCRATCH, ROWS, &mut budget, &mut owned)
                .and_then(|()| engine.analyze_into(&function, &types, 64, &mut budget, &mut owned));
            assert!(
                matches!(
                    result,
                    Err(RetainedAliasErrorV1::Resource(Resource::Storage(_)))
                ),
                "{name} {limit}"
            );
            rhs |= !engine.aliases.rhs.is_empty();
            installed |= engine.aliases.installed.is_some();
            replacement |= engine.aliases.replacement.capacity() != 0;
            selected |= !engine.aliases.selected.is_empty();
            current |= engine.aliases.current.is_some();
            retired |= !engine.retired.is_empty();
            assert_eq!(budget.storage(), FLOOR + owned);
            let before = (
                engine_data(&engine),
                pending(&engine),
                engine.aliases.observer.test_rows(),
                budget.work(),
                budget.storage(),
                owned,
            );
            assert!(
                engine
                    .analyze_into(&function, &types, 64, &mut budget, &mut owned)
                    .is_err()
            );
            assert_eq!(
                before,
                (
                    engine_data(&engine),
                    pending(&engine),
                    engine.aliases.observer.test_rows(),
                    budget.work(),
                    budget.storage(),
                    owned
                )
            );
            drop(engine);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
    assert!(rhs, "no storage denial retained the source-produced RHS");
    assert!(
        installed,
        "no storage denial retained the removed installed holder"
    );
    assert!(
        replacement,
        "no storage denial retained replacement-field capacity"
    );
    assert!(selected, "no storage denial retained selected aliases");
    assert!(current, "no storage denial retained the current alias");
    assert!(retired, "no storage denial retained prior-block DATA");
}
#[test]
fn retained_engine_exhausted_fallback_preserves_previous_accepted_set_and_data() {
    let function = two_blocks();
    let types = types();
    let mut saw = false;
    for cap in 1..16 {
        let mut engine = RetainedSharedEngineV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        engine
            .prepare_observer_into(&function, &types, SCRATCH, ROWS, &mut budget, &mut owned)
            .unwrap();
        engine
            .analyze_into(&function, &types, cap, &mut budget, &mut owned)
            .unwrap();
        let expected = whole_original(&function, &types, cap, ROWS, LIMIT);
        assert_eq!(
            engine
                .accepted_for(&function, &types, &budget, &owned)
                .unwrap(),
            expected.0.as_ref().unwrap()
        );
        if engine.exhausted && !engine.accepted.is_empty() {
            saw = true;
            assert!(engine.empty_result.is_empty());
            assert_eq!(engine.retired.len(), 1);
            assert!(engine.aliases.data.exhausted);
        }
    }
    assert!(saw);
}
#[test]
fn retained_engine_zero_observer_cap_keeps_original_error_and_real_candidate_side_effect() {
    let function = function(vec![shared(), read(2)]);
    let types = types();
    let mut engine = RetainedSharedEngineV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    engine
        .prepare_observer_into(&function, &types, 0, 0, &mut budget, &mut owned)
        .unwrap();
    assert!(matches!(
        engine.analyze_into(&function, &types, 64, &mut budget, &mut owned),
        Err(RetainedAliasErrorV1::Original(Error::ResourceOverflow))
    ));
    assert!(engine.aliases.failure.is_none());
    assert!(engine.aliases.data.candidates.iter().any(|c| c.read));
    assert!(engine.aliases.current.is_some());
    assert!(engine.aliases.observer.test_rows().is_empty());
    let before = (
        engine_data(&engine),
        pending(&engine),
        budget.work(),
        budget.storage(),
        owned,
    );
    assert!(
        engine
            .analyze_into(&function, &types, 64, &mut budget, &mut owned)
            .is_err()
    );
    assert_eq!(
        before,
        (
            engine_data(&engine),
            pending(&engine),
            budget.work(),
            budget.storage(),
            owned
        )
    );
}
#[test]
fn retained_engine_missing_preparation_and_wrong_source_refuse_without_semantic_work() {
    let function = function(vec![shared(), read(2)]);
    let types = types();
    let other = function.clone();
    for prepared in [false, true] {
        let mut engine = RetainedSharedEngineV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        if prepared {
            engine
                .prepare_observer_into(&function, &types, 0, ROWS, &mut budget, &mut owned)
                .unwrap();
        }
        let before = (budget.work(), budget.storage(), owned);
        assert!(
            engine
                .analyze_into(&other, &types, 64, &mut budget, &mut owned)
                .is_err()
        );
        assert_eq!(before, (budget.work(), budget.storage(), owned));
        assert!(engine.aliases.data.candidates.is_empty());
    }
}
#[test]
fn retained_engine_completed_view_binds_exact_source_pair_and_held_floor() {
    let function = function(vec![shared(), read(2)]);
    let types = types();
    let other = function.clone();
    let mut engine = RetainedSharedEngineV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    engine
        .prepare_observer_into(&function, &types, 0, ROWS, &mut budget, &mut owned)
        .unwrap();
    engine
        .analyze_into(&function, &types, 64, &mut budget, &mut owned)
        .unwrap();
    assert!(
        engine
            .accepted_for(&function, &types, &budget, &owned)
            .is_ok()
    );
    assert!(
        engine
            .accepted_for(&other, &types, &budget, &owned)
            .is_err()
    );
    let other_types = types.clone();
    assert!(
        engine
            .accepted_for(&function, &other_types, &budget, &owned)
            .is_err()
    );
    let other_owned = owned;
    assert!(
        engine
            .accepted_for(&function, &types, &budget, &other_owned)
            .is_err()
    );
    budget.reserve_storage(7).unwrap();
    owned += 7;
    assert!(
        engine
            .accepted_for(&function, &types, &budget, &owned)
            .is_ok()
    );
    budget.release_storage(8).unwrap();
    owned -= 8;
    assert!(
        engine
            .accepted_for(&function, &types, &budget, &owned)
            .is_err()
    );
}
#[test]
fn retained_engine_ignored_original_error_cannot_restart_or_replace_any_pending_arena() {
    let function = function(vec![shared(), read(2)]);
    let types = types();
    let mut owner = RetainedAliasStateV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    owner
        .prepare_observer_into(&function, &types, 0, 0, &mut budget, &mut owned)
        .unwrap();
    let mut session = RetainedAliasSessionV1::begin(
        &mut owner,
        &function,
        &types,
        64,
        256,
        &mut budget,
        &mut owned,
    )
    .unwrap();
    session
        .statement(
            SemanticTransparentBorrowSiteV1 {
                block: 0,
                statement: 0,
            },
            function.blocks()[0].statements()[0].kind(),
        )
        .unwrap();
    assert!(
        session
            .statement(
                SemanticTransparentBorrowSiteV1 {
                    block: 0,
                    statement: 1
                },
                function.blocks()[0].statements()[1].kind()
            )
            .is_err()
    );
    let before = (
        data_rows(&session.owner.data),
        session.owner.current.as_ref().map(|a| a.fields.clone()),
        session.budget.work(),
        session.budget.storage(),
        *session.owned,
    );
    assert!(
        session
            .close_block(function.blocks()[0].terminator().kind())
            .is_err()
    );
    assert!(
        session
            .statement(
                SemanticTransparentBorrowSiteV1 {
                    block: 0,
                    statement: 0
                },
                function.blocks()[0].statements()[0].kind()
            )
            .is_err()
    );
    assert!(session.discard_rhs(true).is_err());
    assert!(session.expiry_drop_local(2).is_err());
    assert_eq!(
        before,
        (
            data_rows(&session.owner.data),
            session.owner.current.as_ref().map(|a| a.fields.clone()),
            session.budget.work(),
            session.budget.storage(),
            *session.owned
        )
    );
}
#[test]
fn retained_engine_new_statement_and_close_do_not_overwrite_unconsumed_rhs() {
    let function = function(vec![
        shared(),
        assign_operand(place(3, 1), SemanticOperandV1::Copy(place(2, 1))),
    ]);
    let types = types();
    for close in [false, true] {
        let mut owner = RetainedAliasStateV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        owner
            .prepare_observer_into(&function, &types, 0, ROWS, &mut budget, &mut owned)
            .unwrap();
        let mut session = RetainedAliasSessionV1::begin(
            &mut owner,
            &function,
            &types,
            64,
            256,
            &mut budget,
            &mut owned,
        )
        .unwrap();
        session
            .statement(
                SemanticTransparentBorrowSiteV1 {
                    block: 0,
                    statement: 0,
                },
                function.blocks()[0].statements()[0].kind(),
            )
            .unwrap();
        let SemanticStatementKindV1::Assign(assignment) =
            function.blocks()[0].statements()[1].kind()
        else {
            unreachable!()
        };
        session.rvalue(assignment.value()).unwrap();
        assert!(!session.owner.rhs.is_empty());
        let before = (
            aliases(&session.owner.rhs),
            data_rows(&session.owner.data),
            session.budget.work(),
            session.budget.storage(),
            *session.owned,
        );
        let result = if close {
            session.close_block(function.blocks()[0].terminator().kind())
        } else {
            session.statement(
                SemanticTransparentBorrowSiteV1 {
                    block: 0,
                    statement: 1,
                },
                function.blocks()[0].statements()[1].kind(),
            )
        };
        assert!(result.is_err());
        assert_eq!(
            before,
            (
                aliases(&session.owner.rhs),
                data_rows(&session.owner.data),
                session.budget.work(),
                session.budget.storage(),
                *session.owned
            )
        );
    }
}
#[test]
fn retained_engine_caller_unwind_keeps_source_created_rhs_until_drop_before_refund() {
    let function = function(vec![
        shared(),
        assign_operand(place(3, 1), SemanticOperandV1::Copy(place(2, 1))),
    ]);
    let types = types();
    let mut owner = RetainedAliasStateV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    owner
        .prepare_observer_into(&function, &types, 0, ROWS, &mut budget, &mut owned)
        .unwrap();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut session = RetainedAliasSessionV1::begin(
            &mut owner,
            &function,
            &types,
            64,
            256,
            &mut budget,
            &mut owned,
        )
        .unwrap();
        session
            .statement(
                SemanticTransparentBorrowSiteV1 {
                    block: 0,
                    statement: 0,
                },
                function.blocks()[0].statements()[0].kind(),
            )
            .unwrap();
        let SemanticStatementKindV1::Assign(assignment) =
            function.blocks()[0].statements()[1].kind()
        else {
            unreachable!()
        };
        session.rvalue(assignment.value()).unwrap();
        panic!("controlled caller unwind");
    }));
    assert!(caught.is_err());
    assert_eq!(owner.phase, Phase::Terminal);
    assert!(!owner.rhs.is_empty());
    assert!(!owner.data.candidates.is_empty());
    assert_eq!(budget.storage(), FLOOR + owned);
    drop(owner);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
#[test]
fn retained_engine_original_liveness_original_error_keeps_partial_liveness_and_terminal_state() {
    let function = function(vec![assign_operand(place(99, 0), constant(0))]);
    let types = types();
    let mut engine = RetainedSharedEngineV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    engine
        .prepare_observer_into(&function, &types, 0, ROWS, &mut budget, &mut owned)
        .unwrap();
    assert!(matches!(
        engine.analyze_into(&function, &types, 64, &mut budget, &mut owned),
        Err(RetainedAliasErrorV1::Original(Error::ReplayMismatch))
    ));
    assert_eq!(engine.phase, EnginePhase::Terminal);
    assert_eq!(engine.aliases.phase, Phase::Terminal);
    assert!(engine.aliases.failure.is_none());
    assert!(
        engine
            .liveness
            .completed_for(&function, &budget, &owned)
            .is_err()
    );
    assert_eq!(budget.storage(), owned);
}
#[test]
fn retained_engine_source_order_keeps_expiry_before_fallback_and_filter_after_close() {
    fn compact(value: &str) -> String {
        let mut value: String = value.chars().filter(|c| !c.is_whitespace()).collect();
        while value.contains(",)") {
            value = value.replace(",)", ")");
        }
        value
    }
    let source = compact(include_str!(
        "adapter_shared_primitive_engine_retained_v1.rs"
    ));
    let order = [
        "session.statement(site,statement.kind())?;",
        "schedule.completed_retained(&mutsession,site,statement.kind())?;",
        "ifsession.owner.data.exhausted",
        "session.close_block(block.terminator().kind())?;",
        "let teardown = session.owner.data.alias_words.checked_mul(4)",
        "session.expiry_work(tree_work)?;",
        "self.accepted.insert(site);",
    ];
    let mut offset = 0;
    for marker in order {
        let marker = compact(marker);
        let next = source[offset..].find(&marker).expect(&marker);
        offset += next + marker.len();
    }
    assert!(source.contains("std::mem::swap(&mutself.owner.rhs,&mutself.owner.selected)"));
    assert!(!source.contains("self.accepted.clear()"));
    assert!(!source.contains("has_candidates("));
    assert!(!source.contains("scan_size("));
}

#[test]
fn retained_engine_expiry_original_error_terminalizes_even_when_the_caller_ignores_it() {
    let function = function(vec![statement(SemanticStatementKindV1::Nop)]);
    let types = types();
    let invalid = SemanticStatementKindV1::StorageDead(local(99));
    let mut owner = RetainedAliasStateV1::new();
    let mut live = liveness::RetainedSharedLivenessV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    owner
        .prepare_observer_into(&function, &types, 0, ROWS, &mut budget, &mut owned)
        .unwrap();
    live.prepare_into(&function, &mut budget, &mut owned)
        .unwrap();
    let mut session = RetainedAliasSessionV1::begin(
        &mut owner,
        &function,
        &types,
        64,
        256,
        &mut budget,
        &mut owned,
    )
    .unwrap();
    let schedule = live
        .completed_for(&function, session.budget, session.owned)
        .unwrap();
    assert!(matches!(
        schedule.completed_retained(
            &mut session,
            SemanticTransparentBorrowSiteV1 {
                block: 0,
                statement: 0
            },
            &invalid
        ),
        Err(Error::ReplayMismatch)
    ));
    assert_eq!(session.owner.phase, Phase::Terminal);
    let before = (
        session.budget.work(),
        session.budget.storage(),
        *session.owned,
        data_rows(&session.owner.data),
    );
    assert!(
        session
            .statement(
                SemanticTransparentBorrowSiteV1 {
                    block: 0,
                    statement: 0
                },
                function.blocks()[0].statements()[0].kind()
            )
            .is_err()
    );
    assert_eq!(
        before,
        (
            session.budget.work(),
            session.budget.storage(),
            *session.owned,
            data_rows(&session.owner.data)
        )
    );
}
#[test]
fn retained_engine_empty_allocated_producer_owners_survive_entry_and_work_refusal() {
    let function = function(vec![statement(SemanticStatementKindV1::Nop)]);
    let types = types();
    for incompatible_installed in [false, true] {
        for close in [false, true] {
            // Inert prepaid empty-capacity fixture only; no admitted source facts.
            let mut owner = RetainedAliasStateV1::new();
            owner.rhs = Vec::with_capacity(3);
            owner.replacement = Vec::with_capacity(5);
            if incompatible_installed {
                owner.installed = Some(Vec::with_capacity(7));
            }
            let prefix = owner.rhs.capacity() * size_of::<Alias>()
                + owner.replacement.capacity() * size_of::<u32>()
                + owner
                    .installed
                    .as_ref()
                    .map_or(0, |v| v.capacity() * size_of::<Alias>());
            let mut work = Work::new(32);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR + prefix).unwrap();
            let mut owned = prefix;
            owner
                .prepare_observer_into(&function, &types, 0, ROWS, &mut budget, &mut owned)
                .unwrap();
            let pointers = (
                owner.rhs.as_ptr(),
                owner.rhs.capacity(),
                owner.replacement.as_ptr(),
                owner.replacement.capacity(),
                owner.installed.as_ref().map(|v| (v.as_ptr(), v.capacity())),
            );
            {
                let mut session = RetainedAliasSessionV1::begin(
                    &mut owner,
                    &function,
                    &types,
                    64,
                    256,
                    &mut budget,
                    &mut owned,
                )
                .unwrap();
                let result = if close {
                    session.close_block(function.blocks()[0].terminator().kind())
                } else {
                    session.statement(
                        SemanticTransparentBorrowSiteV1 {
                            block: 0,
                            statement: 0,
                        },
                        function.blocks()[0].statements()[0].kind(),
                    )
                };
                assert!(result.is_err());
                assert_eq!(session.budget.work(), 32);
            }
            assert_eq!(
                pointers,
                (
                    owner.rhs.as_ptr(),
                    owner.rhs.capacity(),
                    owner.replacement.as_ptr(),
                    owner.replacement.capacity(),
                    owner.installed.as_ref().map(|v| (v.as_ptr(), v.capacity()))
                )
            );
            assert_eq!(budget.storage(), FLOOR + owned);
            drop(owner);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}
