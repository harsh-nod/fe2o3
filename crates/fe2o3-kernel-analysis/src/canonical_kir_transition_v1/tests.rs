use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1, CanonicalKirBlockCoordinateV1 as Block,
    CanonicalKirBlockSegmentV1 as Segment, CanonicalKirBlockTransitionV1 as BlockRow,
    CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirDefinitionDescendantV1 as Descendant,
    CanonicalKirDefinitionTransitionV1 as DefinitionRow,
    CanonicalKirEdgeArgumentCoordinateV1 as EdgeArgument,
    CanonicalKirEdgeArgumentTransitionV1 as EdgeArgumentRow, CanonicalKirEdgeCoordinateV1 as Edge,
    CanonicalKirEdgeTransitionV1 as EdgeRow,
    CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
    CanonicalKirFunctionTransitionV1 as FunctionRow,
    CanonicalKirOperationCoordinateV1 as OperationCoordinate,
    CanonicalKirOperationTransitionV1 as OperationRow, CanonicalKirTransitionRangeV1 as RowRange,
    CanonicalKirUseCoordinateV1 as Use, CanonicalKirUseTransitionV1 as UseRow, Constant, Function,
    Module, Operation, OperationKind, Signature, Terminator, Type, ValueDef, ValueId,
    VerifiedCanonicalKernelIrModuleV12,
};

mod catalog_transport;
mod commutative_bitwise_cse;
mod control;
mod control_index;
mod dominance_cse;
mod integer_identities;
mod receipt_wire;
mod resources;
mod scalar;

const LIMIT: usize = 10_000_000;
const F: FunctionCoordinate = FunctionCoordinate(0);
const R: DescendantKind = DescendantKind::Retained;
const S: DescendantKind = DescendantKind::Substituted;
const U32: Type = Type::Scalar(ScalarType::U32);

fn block(block: u32) -> Block {
    Block { function: F, block }
}
fn op(block_ordinal: u32, operation: u32) -> OperationCoordinate {
    OperationCoordinate {
        block: block(block_ordinal),
        operation,
    }
}
fn result(block: u32, operation: u32, result: u32) -> Definition {
    Definition::Result {
        operation: op(block, operation),
        result,
    }
}
fn edge(block_ordinal: u32, successor: u32) -> Edge {
    Edge {
        source: block(block_ordinal),
        successor,
    }
}
fn edge_arg(block: u32, successor: u32, argument: u32) -> EdgeArgument {
    EdgeArgument {
        edge: edge(block, successor),
        argument,
    }
}
fn operand(block: u32, operation: u32, operand: u32) -> Use {
    Use::OperationOperand {
        operation: op(block, operation),
        operand,
    }
}
fn term(block_ordinal: u32, operand: u32) -> Use {
    Use::TerminatorOperand {
        block: block(block_ordinal),
        operand,
    }
}
fn range(start: usize, len: usize) -> RowRange {
    RowRange {
        start: u32::try_from(start).unwrap(),
        len: u32::try_from(len).unwrap(),
    }
}
fn constant(id: u32, value: Constant) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), value.ty()),
        OperationKind::Constant(value),
    )
}
fn value(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn module(
    parameters: Vec<Type>,
    results: Vec<Type>,
    ids: Vec<u32>,
    blocks: Vec<BasicBlock>,
) -> Module {
    let mut module = Module::new("x");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(parameters, results),
        ids.into_iter().map(ValueId).collect(),
        blocks,
    ));
    module
}
fn returning(id: u32, operations: Vec<Operation>, values: &[u32]) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.operations = operations;
    block.terminator = Some(Terminator::Return {
        values: values.iter().copied().map(ValueId).collect(),
    });
    block
}
fn admit(module: &Module) -> (VerifiedCanonicalKernelIrModuleV12, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (owner, storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            module,
            &mut budget,
        )
        .unwrap();
    (owner, storage.retained_storage())
}
fn inventory(owner: &VerifiedCanonicalKernelIrModuleV12) -> (Inventory<'_>, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (inventory, storage) = Inventory::derive(owner, &mut budget).unwrap();
    (inventory, storage.retained_storage())
}

#[derive(Default)]
struct Rows {
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
    fn candidate(&self) -> Candidate<'_> {
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
    fn identity(input: &Inventory<'_>) -> Self {
        let mut rows = Self::default();
        for function in input.functions() {
            rows.functions.push(FunctionRow {
                input: function.coordinate,
                output: function.coordinate,
            });
        }
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
        for row in input.operations() {
            rows.operations.push(OperationRow {
                output: row.coordinate,
                origin: Origin::Retained(row.coordinate),
            });
        }
        for row in input.definitions() {
            rows.definitions.push(DefinitionRow {
                input: row.coordinate,
                outputs: range(rows.definition_outputs.len(), 1),
            });
            rows.definition_outputs.push(Descendant {
                output: row.coordinate,
                kind: R,
            });
        }
        for row in input.uses() {
            rows.uses.push(UseRow {
                input: row.coordinate,
                output: row.coordinate,
            });
        }
        for row in input.edges() {
            rows.edges.push(EdgeRow {
                input: row.coordinate,
                output: row.coordinate,
            });
        }
        for row in input.edge_arguments() {
            rows.edge_arguments.push(EdgeArgumentRow {
                input: row.coordinate,
                output: row.coordinate,
            });
        }
        rows
    }
}

// These are explicit candidate fixtures, not a substitute for live occurrence
// capture. Origin lists are authored per transformation, never inferred from
// equal post-pass ordinals or equal raw ValueIds.
#[derive(Default)]
struct Plan {
    chains: Vec<Vec<(u32, Option<Edge>)>>,
    operations: Vec<Origin>,
    relations: Vec<(u32, u32, DescendantKind)>,
    uses: Vec<Use>,
    edges: Vec<Edge>,
    edge_arguments: Vec<EdgeArgument>,
}
impl Plan {
    fn rows(self, input: &Inventory<'_>, output: &Inventory<'_>) -> Rows {
        assert_eq!(input.functions().len(), 1);
        assert_eq!(output.functions().len(), 1);
        let mut rows = Rows {
            functions: vec![FunctionRow {
                input: F,
                output: F,
            }],
            ..Rows::default()
        };
        assert_eq!(self.chains.len(), output.blocks().len());
        for (row, chain) in output.blocks().iter().zip(self.chains) {
            rows.blocks.push(BlockRow {
                output: row.coordinate,
                segments: range(rows.segments.len(), chain.len()),
            });
            rows.segments
                .extend(chain.into_iter().map(|(input, connector)| Segment {
                    input: block(input),
                    connector,
                }));
        }
        assert_eq!(self.operations.len(), output.operations().len());
        rows.operations
            .extend(
                output
                    .operations()
                    .iter()
                    .zip(self.operations)
                    .map(|(row, origin)| OperationRow {
                        output: row.coordinate,
                        origin,
                    }),
            );
        for source in input.definitions() {
            let start = rows.definition_outputs.len();
            for (a, b, kind) in &self.relations {
                if source.value == Some(ValueId(*a)) {
                    let target = output
                        .definitions()
                        .iter()
                        .find(|row| row.value == Some(ValueId(*b)))
                        .unwrap();
                    rows.definition_outputs.push(Descendant {
                        output: target.coordinate,
                        kind: *kind,
                    });
                }
            }
            rows.definition_outputs[start..].sort_by_key(|row| row.output);
            rows.definitions.push(DefinitionRow {
                input: source.coordinate,
                outputs: range(start, rows.definition_outputs.len() - start),
            });
        }
        assert_eq!(self.uses.len(), output.uses().len());
        rows.uses.extend(
            output
                .uses()
                .iter()
                .zip(self.uses)
                .map(|(row, input)| UseRow {
                    output: row.coordinate,
                    input,
                }),
        );
        assert_eq!(self.edges.len(), output.edges().len());
        rows.edges.extend(
            output
                .edges()
                .iter()
                .zip(self.edges)
                .map(|(row, input)| EdgeRow {
                    output: row.coordinate,
                    input,
                }),
        );
        assert_eq!(self.edge_arguments.len(), output.edge_arguments().len());
        rows.edge_arguments
            .extend(
                output
                    .edge_arguments()
                    .iter()
                    .zip(self.edge_arguments)
                    .map(|(row, input)| EdgeArgumentRow {
                        output: row.coordinate,
                        input,
                    }),
            );
        rows
    }
}

fn inspect(
    input: Module,
    output: Module,
    make: impl FnOnce(&Inventory<'_>, &Inventory<'_>) -> Rows,
    check: impl FnOnce(&Inventory<'_>, &Inventory<'_>, &mut Rows, usize),
) {
    let (a, a_storage) = admit(&input);
    let (b, b_storage) = admit(&output);
    let (a_inventory, ai_storage) = inventory(&a);
    let (b_inventory, bi_storage) = inventory(&b);
    let mut rows = make(&a_inventory, &b_inventory);
    let floor = a_storage + b_storage + ai_storage + bi_storage + rows.storage() + 23;
    check(&a_inventory, &b_inventory, &mut rows, floor);
}
fn accepted(a: &Inventory<'_>, b: &Inventory<'_>, rows: &Rows, floor: usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, floor + LIMIT);
    budget.reserve_storage(floor).unwrap();
    let storage = {
        let (checked, storage) =
            check_canonical_kir_transition_v1(a, b, rows.candidate(), &mut budget).unwrap();
        assert!(std::ptr::eq(checked.input(), a));
        assert!(std::ptr::eq(checked.output(), b));
        assert!(!checked.grants_authority());
        assert_eq!(checked.checked_rules().len(), 6);
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert_eq!(checked.rows().uses.len(), b.uses().len());
        storage
    };
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
    receipt_wire::roundtrip(a, b, rows, &mut budget);
}
fn rejected(a: &Inventory<'_>, b: &Inventory<'_>, rows: &Rows, floor: usize) -> Error {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, floor + LIMIT);
    budget.reserve_storage(floor).unwrap();
    let error = check_canonical_kir_transition_v1(a, b, rows.candidate(), &mut budget).unwrap_err();
    assert_eq!(budget.storage(), floor);
    assert!(budget.work() > 0);
    error
}

#[test]
fn identity_rows_preserve_distinct_owner_binding_and_reject_missing_or_wrong_coordinates() {
    let original = module(
        vec![],
        vec![U32],
        vec![],
        vec![returning(
            u32::MAX,
            vec![constant(77, Constant::U32(7))],
            &[77],
        )],
    );
    inspect(
        original.clone(),
        original,
        |a, _| Rows::identity(a),
        |a, b, rows, floor| {
            assert!(!std::ptr::eq(a.owner(), b.owner()));
            accepted(a, b, rows, floor);
            let old = rows.uses.pop().unwrap();
            assert_eq!(rejected(a, b, rows, floor), Error::IncompleteRows);
            rows.uses.push(old);
            rows.uses[0].input = term(1, 0);
            assert_eq!(
                rejected(a, b, rows, floor),
                Error::Rule("selector or return operand occurrence")
            );
        },
    );
}

#[test]
fn unchanged_declaration_positions_do_not_require_body_values() {
    let mut original = Module::new("x");
    original.functions.push(Function::declaration(
        "external",
        Signature::new(vec![U32], vec![U32]),
    ));
    inspect(
        original.clone(),
        original,
        |a, _| Rows::identity(a),
        |a, b, rows, floor| {
            assert_eq!(a.definitions()[0].value, None);
            accepted(a, b, rows, floor);
            rows.definition_outputs[0].kind = S;
            assert!(matches!(
                rejected(a, b, rows, floor),
                Error::Rule("complete final definition anchors")
            ));
        },
    );
}

#[test]
fn separately_admitted_declaration_type_change_is_not_a_transition() {
    let mut input = Module::new("x");
    input.functions.push(Function::declaration(
        "external",
        Signature::new(vec![U32], vec![]),
    ));
    let mut output = Module::new("x");
    output.functions.push(Function::declaration(
        "external",
        Signature::new(vec![Type::Scalar(ScalarType::U64)], vec![]),
    ));
    inspect(
        input,
        output,
        |a, _| Rows::identity(a),
        |a, b, rows, floor| {
            assert_eq!(
                rejected(a, b, rows, floor),
                Error::Rule("exact function signature and declaration")
            );
        },
    );
}
