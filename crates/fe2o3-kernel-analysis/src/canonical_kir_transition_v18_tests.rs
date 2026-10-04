use super::super::{DescendantKind, Origin, payload};
use super::*;
use crate::canonical_kir_inventory_v1::v18_tests::{admit, storage_module};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirBlockSegmentV1 as Segment,
    CanonicalKirBlockTransitionV1 as BlockRow, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirDefinitionDescendantV1 as Descendant,
    CanonicalKirDefinitionTransitionV1 as DefinitionRow,
    CanonicalKirEdgeArgumentTransitionV1 as EdgeArgumentRow,
    CanonicalKirEdgeTransitionV1 as EdgeRow,
    CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
    CanonicalKirFunctionTransitionV1 as FunctionRow,
    CanonicalKirOperationCoordinateV1 as OperationCoordinate,
    CanonicalKirOperationTransitionV1 as OperationRow, CanonicalKirTransitionRangeV1 as RowRange,
    CanonicalKirUseCoordinateV1 as Use, CanonicalKirUseTransitionV1 as UseRow, CastKind, Constant,
    ExecutionOperationV15 as Execution, ExecutionRoleV15 as Role, Function, Kernel, LaunchDomain,
    LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature,
    StorageCopyOverlapV1, StorageFieldV1, StorageLayoutIdV1 as Id, StorageOperationV1 as Storage,
    StorageProjectionV1 as Projection, StorageVariantEncodingV1 as Encoding, StorageVariantV1,
    Terminator, Type, ValueDef, ValueId,
};
use std::mem::size_of;

pub(super) const LIMIT: usize = 10_000_000;
pub(super) type Inventory<'a> = CanonicalKirInventoryV18<'a>;
const U32: Type = Type::Scalar(ScalarType::U32);
const RETAINED: DescendantKind = DescendantKind::Retained;

fn range(start: usize, len: usize) -> RowRange {
    RowRange {
        start: start.try_into().unwrap(),
        len: len.try_into().unwrap(),
    }
}
fn op(operation: u32) -> OperationCoordinate {
    OperationCoordinate {
        block: Block {
            function: FunctionCoordinate(0),
            block: 0,
        },
        operation,
    }
}
fn constant(id: u32, value: Constant) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), value.ty()),
        OperationKind::Constant(value),
    )
}
fn execution(id: Option<(u32, Role)>, kind: Execution) -> Operation {
    Operation::new(
        id.into_iter()
            .map(|(id, role)| ValueDef::new(ValueId(id), Type::Execution(role)))
            .collect(),
        OperationKind::Execution(kind),
    )
}

#[derive(Default)]
pub(super) struct Rows {
    functions: Vec<FunctionRow>,
    blocks: Vec<BlockRow>,
    segments: Vec<Segment>,
    operations: Vec<OperationRow>,
    definitions: Vec<DefinitionRow>,
    definition_outputs: Vec<Descendant>,
    uses: Vec<UseRow>,
    edges: Vec<EdgeRow>,
    edge_arguments: Vec<EdgeArgumentRow>,
}
impl Rows {
    pub(super) fn candidate(&self) -> Candidate<'_> {
        Candidate {
            functions: &self.functions,
            blocks: &self.blocks,
            segments: &self.segments,
            operations: &self.operations,
            definitions: &self.definitions,
            definition_outputs: &self.definition_outputs,
            uses: &self.uses,
            edges: &self.edges,
            edge_arguments: &self.edge_arguments,
        }
    }
    fn storage(&self) -> usize {
        size_of::<Self>()
            + self.functions.capacity() * size_of::<FunctionRow>()
            + self.blocks.capacity() * size_of::<BlockRow>()
            + self.segments.capacity() * size_of::<Segment>()
            + self.operations.capacity() * size_of::<OperationRow>()
            + self.definitions.capacity() * size_of::<DefinitionRow>()
            + self.definition_outputs.capacity() * size_of::<Descendant>()
            + self.uses.capacity() * size_of::<UseRow>()
            + self.edges.capacity() * size_of::<EdgeRow>()
            + self.edge_arguments.capacity() * size_of::<EdgeArgumentRow>()
    }
    pub(super) fn identity(input: &Inventory<'_>) -> Self {
        let mut rows = Self::default();
        rows.functions
            .extend(input.functions().iter().map(|row| FunctionRow {
                input: row.coordinate,
                output: row.coordinate,
            }));
        for row in input.blocks() {
            rows.blocks.push(BlockRow {
                output: row.coordinate,
                segments: range(rows.segments.len(), 1),
            });
            rows.segments.push(Segment {
                input: row.coordinate,
                connector: None,
            });
        }
        rows.operations
            .extend(input.operations().iter().map(|row| OperationRow {
                output: row.coordinate,
                origin: Origin::Retained(row.coordinate),
            }));
        for row in input.definitions() {
            rows.definitions.push(DefinitionRow {
                input: row.coordinate,
                outputs: range(rows.definition_outputs.len(), 1),
            });
            rows.definition_outputs.push(Descendant {
                output: row.coordinate,
                kind: RETAINED,
            });
        }
        rows.uses.extend(input.uses().iter().map(|row| UseRow {
            input: row.coordinate,
            output: row.coordinate,
        }));
        rows.edges.extend(input.edges().iter().map(|row| EdgeRow {
            input: row.coordinate,
            output: row.coordinate,
        }));
        rows.edge_arguments
            .extend(input.edge_arguments().iter().map(|row| EdgeArgumentRow {
                input: row.coordinate,
                output: row.coordinate,
            }));
        rows
    }
}

pub(super) fn inspect(
    input: Module,
    output: Module,
    make: impl FnOnce(&Inventory<'_>, &Inventory<'_>) -> Rows,
    check: impl FnOnce(&Inventory<'_>, &Inventory<'_>, &mut Rows, usize),
) {
    let (a, a_bytes) = admit(&input);
    let (b, b_bytes) = admit(&output);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(a_bytes + b_bytes + 23).unwrap();
    let (ai, ai_bytes) = Inventory::derive_v18(&a, &mut budget).unwrap();
    budget.reserve_storage(ai_bytes.retained_storage()).unwrap();
    let (bi, bi_bytes) = Inventory::derive_v18(&b, &mut budget).unwrap();
    budget.reserve_storage(bi_bytes.retained_storage()).unwrap();
    let mut rows = make(&ai, &bi);
    let floor = budget.storage() + rows.storage();
    check(&ai, &bi, &mut rows, floor);
    drop((rows, ai, bi));
    budget
        .release_storage(ai_bytes.retained_storage() + bi_bytes.retained_storage())
        .unwrap();
    drop((a, b));
    budget.release_storage(a_bytes + b_bytes).unwrap();
    assert_eq!(budget.storage(), 23);
}
fn accepted(a: &Inventory<'_>, b: &Inventory<'_>, rows: &Rows, floor: usize) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, floor + LIMIT);
    budget.reserve_storage(floor).unwrap();
    let (checked, receipt) =
        check_canonical_kir_transition_v18(a, b, rows.candidate(), &mut budget).unwrap();
    assert!(std::ptr::eq(checked.input(), a) && std::ptr::eq(checked.output(), b));
    assert!(!checked.grants_authority());
    assert_eq!(checked.rows().uses.len(), b.uses().len());
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let (index, index_receipt) =
        CheckedCanonicalKirControlIndexV18::derive_v18(&checked, &mut budget).unwrap();
    budget
        .reserve_storage(index_receipt.retained_storage())
        .unwrap();
    assert!(std::ptr::eq(index.input(), a) && std::ptr::eq(index.output(), b));
    drop(index);
    budget
        .release_storage(index_receipt.retained_storage())
        .unwrap();
    drop(checked);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}
fn rejected(a: &Inventory<'_>, b: &Inventory<'_>, rows: &Rows, floor: usize) -> Error {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, floor + LIMIT);
    budget.reserve_storage(floor).unwrap();
    let error =
        check_canonical_kir_transition_v18(a, b, rows.candidate(), &mut budget).unwrap_err();
    assert_eq!(budget.storage(), floor);
    error
}
pub(super) fn unit_module() -> Module {
    let mut module = Module::new("x");
    let mut block = BasicBlock::new(BlockId(u32::MAX));
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module
}

pub(super) fn execution_module(elements: u16) -> Module {
    let slice = Type::slice(U32, AddressSpace::Global, AccessMode::ReadOnly);
    let mut block = BasicBlock::new(BlockId(7));
    block.operations = vec![
        execution(Some((10, Role::Context)), Execution::ContextIssue),
        execution(
            Some((11, Role::Workgroup)),
            Execution::WorkgroupDerive {
                context: ValueId(10),
            },
        ),
        execution(
            Some((
                12,
                Role::MaskedTileU32 {
                    lanes: 64,
                    elements,
                },
            )),
            Execution::MaskedTileLoadU32 {
                workgroup: ValueId(11),
                input: ValueId(0),
                base: ValueId(1),
                lanes: 64,
                elements,
            },
        ),
        execution(
            Some((
                13,
                Role::LaneFragmentU32 {
                    lanes: 64,
                    elements,
                },
            )),
            Execution::TileIntoFragmentU32 {
                tile: ValueId(12),
                lanes: 64,
                elements,
            },
        ),
        Operation::new(
            (0..u32::from(elements))
                .map(|i| ValueDef::new(ValueId(20 + i), U32))
                .chain(
                    (0..u32::from(elements))
                        .map(|i| ValueDef::new(ValueId(20 + u32::from(elements) + i), Type::BOOL)),
                )
                .collect(),
            OperationKind::Execution(Execution::FragmentIntoPartsU32 {
                fragment: ValueId(13),
                lanes: 64,
                elements,
            }),
        ),
        execution(
            None,
            Execution::ScopeEnd {
                workgroup: ValueId(11),
                discarded: vec![],
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("execution");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![slice, Type::INDEX], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    module.storage_layouts = storage_module(AddressSpace::Private, false).storage_layouts;
    module
}

#[test]
fn actual_storage_and_execution_owners_use_the_same_checked_relation() {
    for module in [
        storage_module(AddressSpace::Private, false),
        storage_module(AddressSpace::Workgroup, true),
        execution_module(1),
        execution_module(2),
    ] {
        inspect(
            module.clone(),
            module,
            |a, _| Rows::identity(a),
            |a, b, rows, floor| {
                assert!(!std::ptr::eq(a.owner(), b.owner()));
                accepted(a, b, rows, floor);
                let last = rows.uses.pop().unwrap();
                assert_eq!(rejected(a, b, rows, floor), Error::IncompleteRows);
                rows.uses.push(last);
                rows.operations[0].output.block.function = FunctionCoordinate(u32::MAX);
                assert_eq!(rejected(a, b, rows, floor), Error::IncompleteRows);
            },
        );
    }
}

#[test]
fn complete_table_is_checked_even_when_graph_does_not_use_changed_row() {
    let mut a = unit_module();
    a.storage_layouts = storage_module(AddressSpace::Private, false).storage_layouts;
    for mutate in 0..4 {
        let mut b = a.clone();
        match mutate {
            0 => b.storage_layouts[0].kind = Kind::Scalar(ScalarType::I64),
            1 => {
                b.storage_layouts[1].kind = Kind::Union(
                    vec![StorageFieldV1 {
                        offset: 0,
                        layout: Id(0),
                    }]
                    .into_boxed_slice(),
                )
            }
            2 => {
                b.storage_layouts[2].size = 48;
                if let Kind::Array { length, .. } = &mut b.storage_layouts[2].kind {
                    *length = 3;
                }
            }
            _ => {
                if let Kind::Variants { variants, .. } = &mut b.storage_layouts[3].kind {
                    variants[0].discriminant = 92;
                }
            }
        }
        inspect(
            a.clone(),
            b,
            |a, _| Rows::identity(a),
            |a, b, rows, floor| {
                // Table precedence is independent of missing candidate rows.
                rows.functions.clear();
                assert_eq!(
                    rejected(a, b, rows, floor),
                    Error::Rule("complete storage layout table")
                );
            },
        );
    }
}

#[test]
fn equal_physical_geometry_does_not_relabel_the_storage_object_type() {
    let mut a = unit_module();
    a.storage_layouts = vec![
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: Kind::Scalar(ScalarType::U32),
        },
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: Kind::Scalar(ScalarType::U32),
        },
    ];
    a.functions[0].signature.parameters = vec![Type::pointer(
        Type::StorageObject(Id(0)),
        AddressSpace::Private,
        AccessMode::ReadOnly,
    )];
    a.functions[0].body.as_mut().unwrap().parameters = vec![ValueId(1)];
    let mut b = a.clone();
    b.functions[0].signature.parameters[0] = Type::pointer(
        Type::StorageObject(Id(1)),
        AddressSpace::Private,
        AccessMode::ReadOnly,
    );
    inspect(
        a,
        b,
        |a, _| Rows::identity(a),
        |a, b, rows, floor| {
            assert!(matches!(rejected(a, b, rows, floor), Error::Rule(_)));
        },
    );
}

#[test]
fn actual_storage_payload_and_operand_changes_do_not_gain_rewrite_authority() {
    let a = storage_module(AddressSpace::Private, false);
    for mutation in 0..4 {
        let mut b = a.clone();
        let ops = &mut b.functions[0].body.as_mut().unwrap().blocks[0].operations;
        match mutation {
            0 => {
                if let OperationKind::Storage(Storage::WriteValue { access, .. }) = &mut ops[5].kind
                {
                    access.volatile = true;
                }
            }
            1 => {
                if let OperationKind::Storage(Storage::Project { step, .. }) = &mut ops[4].kind {
                    *step = Projection::Field(0);
                }
            }
            2 => {
                if let OperationKind::Storage(Storage::CopyObject { overlap, .. }) =
                    &mut ops[8].kind
                {
                    *overlap = StorageCopyOverlapV1::MayOverlap;
                }
            }
            _ => {
                if let OperationKind::Storage(Storage::CopyObject {
                    source,
                    destination,
                    ..
                }) = &mut ops[8].kind
                {
                    std::mem::swap(source, destination);
                }
            }
        }
        inspect(
            a.clone(),
            b,
            |a, _| Rows::identity(a),
            |a, b, rows, floor| {
                assert!(matches!(rejected(a, b, rows, floor), Error::Rule(_)));
            },
        );
    }
}

#[test]
fn actual_execution_operand_and_role_changes_are_not_type_only_equivalence() {
    let mut a = execution_module(1);
    let input = a.functions[0].signature.parameters[0].clone();
    a.functions[0].signature.parameters.push(input);
    a.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(2));
    let mut b = a.clone();
    let OperationKind::Execution(Execution::MaskedTileLoadU32 { input, .. }) =
        &mut b.functions[0].body.as_mut().unwrap().blocks[0].operations[2].kind
    else {
        unreachable!();
    };
    *input = ValueId(2);
    inspect(
        a,
        b,
        |a, _| Rows::identity(a),
        |a, b, rows, floor| {
            assert!(matches!(rejected(a, b, rows, floor), Error::Rule(_)));
        },
    );
    inspect(
        execution_module(1),
        execution_module(2),
        |a, _| Rows::identity(a),
        |a, b, rows, floor| {
            assert!(matches!(
                rejected(a, b, rows, floor),
                Error::IncompleteRows | Error::Rule(_)
            ));
        },
    );
}

#[test]
fn scalar_dead_code_can_drop_but_live_ordered_storage_cannot() {
    let original = storage_module(AddressSpace::Private, false);
    let mut input = original.clone();
    input.functions[1].body.as_mut().unwrap().blocks[0]
        .operations
        .push(constant(99, Constant::U32(7)));
    inspect(
        input,
        original,
        |a, b| {
            let mut rows = Rows::identity(a);
            rows.operations.pop();
            let dead = a
                .definitions()
                .iter()
                .position(|row| {
                    row.value == Some(ValueId(99))
                        && row.coordinate
                            == Definition::Result {
                                operation: OperationCoordinate {
                                    block: Block {
                                        function: FunctionCoordinate(1),
                                        block: 0,
                                    },
                                    operation: 1,
                                },
                                result: 0,
                            }
                })
                .unwrap();
            assert_eq!(dead, rows.definitions.len() - 1);
            rows.definitions[dead].outputs.len = 0;
            rows.definition_outputs.pop();
            assert_eq!(rows.operations.len(), b.operations().len());
            rows
        },
        |a, b, rows, floor| accepted(a, b, rows, floor),
    );

    let input = storage_module(AddressSpace::Private, false);
    let mut output = input.clone();
    // This write has no SSA result, but still cannot disappear.
    output.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .remove(5);
    inspect(
        input,
        output,
        |a, b| {
            let mut rows = Rows::identity(a);
            rows.operations.remove(5);
            for row in &mut rows.operations {
                if row.output.block.function == FunctionCoordinate(0)
                    && row.output.block.block == 0
                    && row.output.operation > 5
                {
                    row.output.operation -= 1;
                }
            }
            for row in &mut rows.definition_outputs {
                if let Definition::Result { operation, .. } = &mut row.output {
                    if operation.block.function == FunctionCoordinate(0)
                        && operation.block.block == 0
                        && operation.operation > 5
                    {
                        operation.operation -= 1;
                    }
                }
            }
            rows.uses.retain(|row| !matches!(row.input, Use::OperationOperand { operation, .. } if operation == op(5)));
            for row in &mut rows.uses {
                if let Use::OperationOperand { operation, .. } = &mut row.output {
                    if operation.block.function == FunctionCoordinate(0)
                        && operation.block.block == 0
                        && operation.operation > 5
                    {
                        operation.operation -= 1;
                    }
                }
            }
            assert_eq!(rows.operations.len(), b.operations().len());
            rows
        },
        |a, b, rows, floor| {
            assert_eq!(
                rejected(a, b, rows, floor),
                Error::Rule("executable ordered operation removed")
            );
        },
    );
}

#[test]
fn slice_widening_and_storage_types_keep_exact_payload_and_source_operand() {
    let source = Type::slice(U32, AddressSpace::Global, AccessMode::ReadOnly);
    let target = Type::slice(U32, AddressSpace::Generic, AccessMode::ReadOnly);
    let mut module = unit_module();
    let f = &mut module.functions[0];
    f.signature = Signature::new(vec![source], vec![target.clone()]);
    let body = f.body.as_mut().unwrap();
    body.parameters = vec![ValueId(1)];
    body.blocks[0].operations.push(Operation::effect_free(
        ValueDef::new(ValueId(2), target.clone()),
        OperationKind::Cast {
            kind: CastKind::SliceToGeneric,
            value: ValueId(1),
            to: target,
        },
    ));
    body.blocks[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    module.storage_layouts = storage_module(AddressSpace::Private, false).storage_layouts;
    inspect(
        module.clone(),
        module,
        |a, _| Rows::identity(a),
        |a, b, rows, floor| accepted(a, b, rows, floor),
    );
}

#[test]
fn actual_scalar_fold_transports_the_exact_storage_write_operand() {
    let pointer = Type::pointer(
        Type::StorageObject(Id(0)),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let mut a = unit_module();
    a.storage_layouts.push(StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: Kind::Scalar(ScalarType::U32),
    });
    let f = &mut a.functions[0];
    f.signature = Signature::new(vec![pointer], vec![U32]);
    let body = f.body.as_mut().unwrap();
    body.parameters = vec![ValueId(99)];
    body.blocks[0].operations = vec![
        constant(1, Constant::U32(10)),
        constant(2, Constant::U32(20)),
        Operation::effect_free(
            ValueDef::new(ValueId(3), U32),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Storage(Storage::WriteValue {
                address: ValueId(99),
                value: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
    ];
    body.blocks[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(3)],
    });
    for (answer, retain_add) in [(30, true), (31, true), (30, false)] {
        let mut b = a.clone();
        let block = &mut b.functions[0].body.as_mut().unwrap().blocks[0];
        let replacement = constant(30, Constant::U32(answer));
        if retain_add {
            block.operations.insert(2, replacement);
        } else {
            block.operations[2] = replacement;
        }
        let OperationKind::Storage(Storage::WriteValue { value, .. }) =
            &mut block.operations[3 + usize::from(retain_add)].kind
        else {
            unreachable!();
        };
        *value = ValueId(30);
        block.terminator = Some(Terminator::Return {
            values: vec![ValueId(30)],
        });
        inspect(
            a.clone(),
            b,
            |a, _| {
                let mut rows = Rows::identity(a);
                let folded = Definition::Result {
                    operation: op(2),
                    result: 0,
                };
                rows.operations[2].origin = Origin::ConstantFrom(folded);
                rows.definition_outputs
                    .iter_mut()
                    .find(|row| row.output == folded)
                    .unwrap()
                    .kind = DescendantKind::Substituted;
                if retain_add {
                    // Ordinary Add stays ordered under the existing policy;
                    // only its consumers change to the independently checked constant.
                    rows.operations.insert(
                        3,
                        OperationRow {
                            output: op(3),
                            origin: Origin::Retained(op(2)),
                        },
                    );
                    rows.operations[4].output = op(4);
                    let definition = rows.definitions.last_mut().unwrap();
                    assert_eq!(definition.input, folded);
                    assert_eq!(
                        definition.outputs.start + definition.outputs.len,
                        u32::try_from(rows.definition_outputs.len()).unwrap()
                    );
                    definition.outputs.len += 1;
                    rows.definition_outputs.push(Descendant {
                        output: Definition::Result {
                            operation: op(3),
                            result: 0,
                        },
                        kind: RETAINED,
                    });
                    for row in &mut rows.uses {
                        if let Use::OperationOperand { operation, .. } = &mut row.output {
                            if operation.operation >= 2 {
                                operation.operation += 1;
                            }
                        }
                    }
                } else {
                    rows.uses.retain(|row| !matches!(row.input, Use::OperationOperand { operation, .. } if operation == op(2)));
                }
                rows
            },
            |a, b, rows, floor| {
                if !retain_add {
                    assert_eq!(
                        rejected(a, b, rows, floor),
                        Error::Rule("executable ordered operation removed")
                    );
                } else if answer == 30 {
                    accepted(a, b, rows, floor);
                } else {
                    assert_eq!(
                        rejected(a, b, rows, floor),
                        Error::Rule("constant synthesis bits")
                    );
                }
            },
        );
    }
}

#[test]
fn pure_scalar_cse_preserves_ordered_storage_and_exact_source_values() {
    let pointer = Type::pointer(
        Type::StorageObject(Id(0)),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let mut a = unit_module();
    a.storage_layouts.push(StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: Kind::Scalar(ScalarType::U32),
    });
    a.functions[0].signature = Signature::new(vec![pointer], vec![U32]);
    let body = a.functions[0].body.as_mut().unwrap();
    body.parameters = vec![ValueId(99)];
    body.blocks[0].operations = vec![
        constant(1, Constant::U32(7)),
        constant(2, Constant::U32(7)),
        Operation::new(
            vec![],
            OperationKind::Storage(Storage::WriteValue {
                address: ValueId(99),
                value: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
    ];
    body.blocks[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    let mut b = a.clone();
    let body = b.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.remove(1);
    if let OperationKind::Storage(Storage::WriteValue { value, .. }) =
        &mut body.blocks[0].operations[1].kind
    {
        *value = ValueId(1);
    }
    body.blocks[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(1)],
    });
    for original_bits in [7, 8] {
        let mut a = a.clone();
        a.functions[0].body.as_mut().unwrap().blocks[0].operations[1] =
            constant(2, Constant::U32(original_bits));
        inspect(
            a,
            b.clone(),
            |a, _| {
                let mut rows = Rows::identity(a);
                rows.operations.remove(1);
                rows.operations[1].output = op(1);
                let second = Definition::Result {
                    operation: op(1),
                    result: 0,
                };
                let row = rows
                    .definition_outputs
                    .iter_mut()
                    .find(|row| row.output == second)
                    .unwrap();
                row.output = Definition::Result {
                    operation: op(0),
                    result: 0,
                };
                row.kind = DescendantKind::Substituted;
                for row in &mut rows.uses {
                    if let Use::OperationOperand { operation, .. } = &mut row.output {
                        *operation = op(1);
                    }
                }
                rows
            },
            |a, b, rows, floor| {
                if original_bits == 7 {
                    accepted(a, b, rows, floor);
                } else {
                    assert!(matches!(rejected(a, b, rows, floor), Error::Rule(_)));
                }
            },
        );
    }
}

#[test]
fn cyclic_phi_edges_keep_the_actual_backedge_definition_with_v18_tables() {
    let mut a = unit_module();
    a.storage_layouts = all_layouts();
    a.functions[0].signature = Signature::new(vec![Type::BOOL, U32], vec![U32]);
    let body = a.functions[0].body.as_mut().unwrap();
    body.parameters = vec![ValueId(0), ValueId(1)];
    let mut entry = BasicBlock::new(BlockId(7));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(8),
        arguments: vec![ValueId(1)],
    });
    let mut header = BasicBlock::new(BlockId(8));
    header.parameters = vec![ValueDef::new(ValueId(2), U32)];
    header.operations = vec![
        constant(3, Constant::U32(1)),
        Operation::effect_free(
            ValueDef::new(ValueId(4), U32),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ),
    ];
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(8),
        then_arguments: vec![ValueId(4)],
        else_target: BlockId(9),
        else_arguments: vec![ValueId(2)],
    });
    let mut exit = BasicBlock::new(BlockId(9));
    exit.parameters = vec![ValueDef::new(ValueId(5), U32)];
    exit.terminator = Some(Terminator::Return {
        values: vec![ValueId(5)],
    });
    body.blocks = vec![entry, header, exit];
    for changed in [false, true] {
        let mut b = a.clone();
        if changed {
            if let Some(Terminator::ConditionalBranch { then_arguments, .. }) =
                &mut b.functions[0].body.as_mut().unwrap().blocks[1].terminator
            {
                then_arguments[0] = ValueId(1);
            }
        }
        inspect(
            a.clone(),
            b,
            |a, _| Rows::identity(a),
            |a, b, rows, floor| {
                if changed {
                    assert!(matches!(rejected(a, b, rows, floor), Error::Rule(_)));
                } else {
                    accepted(a, b, rows, floor);
                }
            },
        );
    }
}

#[test]
fn selected_branch_merge_checks_exact_original_connector_with_storage_table() {
    use fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1 as Edge;
    let mut a = unit_module();
    a.storage_layouts = all_layouts();
    a.functions[0].signature.results = vec![U32];
    let mut entry = BasicBlock::new(BlockId(90));
    entry.operations.push(constant(1, Constant::Bool(true)));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(91),
        then_arguments: vec![],
        else_target: BlockId(92),
        else_arguments: vec![],
    });
    let mut yes = BasicBlock::new(BlockId(91));
    yes.operations.push(constant(2, Constant::U32(9)));
    yes.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    let mut no = BasicBlock::new(BlockId(92));
    no.operations.push(constant(3, Constant::U32(8)));
    no.terminator = Some(Terminator::Return {
        values: vec![ValueId(3)],
    });
    a.functions[0].body.as_mut().unwrap().blocks = vec![entry, yes.clone(), no];
    let mut b = a.clone();
    b.functions[0].body.as_mut().unwrap().blocks = vec![yes];
    inspect(
        a,
        b,
        |a, _| {
            let mut rows = Rows::default();
            rows.functions.push(FunctionRow {
                input: FunctionCoordinate(0),
                output: FunctionCoordinate(0),
            });
            rows.blocks.push(BlockRow {
                output: op(0).block,
                segments: range(0, 2),
            });
            rows.segments = vec![
                Segment {
                    input: op(0).block,
                    connector: Some(Edge {
                        source: op(0).block,
                        successor: 0,
                    }),
                },
                Segment {
                    input: Block {
                        block: 1,
                        ..op(0).block
                    },
                    connector: None,
                },
            ];
            let source = OperationCoordinate {
                block: Block {
                    block: 1,
                    ..op(0).block
                },
                operation: 0,
            };
            rows.operations.push(OperationRow {
                output: op(0),
                origin: Origin::Retained(source),
            });
            for def in a.definitions() {
                let start = rows.definition_outputs.len();
                if def.coordinate
                    == (Definition::Result {
                        operation: source,
                        result: 0,
                    })
                {
                    rows.definition_outputs.push(Descendant {
                        output: Definition::Result {
                            operation: op(0),
                            result: 0,
                        },
                        kind: RETAINED,
                    });
                }
                rows.definitions.push(DefinitionRow {
                    input: def.coordinate,
                    outputs: range(start, rows.definition_outputs.len() - start),
                });
            }
            rows.uses.push(UseRow {
                input: Use::TerminatorOperand {
                    block: source.block,
                    operand: 0,
                },
                output: Use::TerminatorOperand {
                    block: op(0).block,
                    operand: 0,
                },
            });
            rows
        },
        |a, b, rows, floor| {
            accepted(a, b, rows, floor);
            rows.segments[0].connector.as_mut().unwrap().successor = 1;
            assert!(matches!(rejected(a, b, rows, floor), Error::Rule(_)));
        },
    );
}

pub(super) fn all_layouts() -> Vec<StorageLayoutV1> {
    use fe2o3_kernel_ir::{FixedVectorTypeV12, StoragePointerV1, VectorLayoutV12};
    let field = |offset, layout| StorageFieldV1 {
        offset,
        layout: Id(layout),
    };
    let variant = |discriminant, direct_tag_bits, uninhabited, layout| StorageVariantV1 {
        discriminant,
        direct_tag_bits,
        uninhabited,
        layout: Id(layout),
    };
    vec![
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: Kind::Scalar(ScalarType::U64),
        },
        StorageLayoutV1 {
            size: 16,
            alignment: 16,
            kind: Kind::Vector(FixedVectorTypeV12::new(
                ScalarType::U32,
                4,
                VectorLayoutV12::Contiguous,
            )),
        },
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: Kind::Pointer(StoragePointerV1 {
                pointee: Id(3),
                value_space: AddressSpace::Private,
                encoded_space: AddressSpace::Generic,
                access: AccessMode::ReadWrite,
                stored_bits: 64,
            }),
        },
        StorageLayoutV1 {
            size: 24,
            alignment: 8,
            kind: Kind::Record(vec![field(0, 2), field(16, 0)].into_boxed_slice()),
        },
        StorageLayoutV1 {
            size: 16,
            alignment: 16,
            kind: Kind::Union(vec![field(0, 0), field(0, 1)].into_boxed_slice()),
        },
        StorageLayoutV1 {
            size: 24,
            alignment: 8,
            kind: Kind::Array {
                element: Id(0),
                length: 3,
                stride: 8,
            },
        },
        StorageLayoutV1 {
            size: 16,
            alignment: 8,
            kind: Kind::Slice {
                element: Id(3),
                value_space: AddressSpace::Private,
                access: AccessMode::ReadWrite,
                data: field(0, 2),
                length: field(8, 9),
            },
        },
        StorageLayoutV1 {
            size: 32,
            alignment: 8,
            kind: Kind::Variants {
                encoding: Encoding::Direct { tag: field(24, 0) },
                variants: vec![
                    variant(91, Some(3), false, 3),
                    variant(u128::MAX, Some(9), false, 5),
                ]
                .into_boxed_slice(),
            },
        },
        StorageLayoutV1 {
            size: 24,
            alignment: 8,
            kind: Kind::Variants {
                encoding: Encoding::Niche {
                    tag: field(0, 0),
                    untagged_variant: 0,
                    first_niche_variant: 1,
                    last_niche_variant: 1,
                    niche_start: u64::MAX as u128,
                },
                variants: vec![
                    variant(0, None, false, 3),
                    variant(1, None, false, 5),
                    variant(2, None, true, 0),
                ]
                .into_boxed_slice(),
            },
        },
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: Kind::Scalar(ScalarType::Index),
        },
    ]
}

#[path = "canonical_kir_transition_v18_payload_tests.rs"]
mod payload_tests;

#[path = "canonical_kir_transition_v18_ordered_tests.rs"]
mod ordered_tests;
