use super::recipe::{Effect, Expr, Id, Origin, Recipe};
use super::{ConditionalFillProgramErrorV1 as Error, ProgramShape, Value};
use crate::ValidatedConditionalCompilerProofInputsV1;
use fe2o3_lower_mir_kernel::InertCanonicalMirToKirCorrespondenceEvidenceV4 as Correspondence;
use fe2o3_mir_model::semantic_mir_v1::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Local {
    Output,
    Index,
    SharedIndex { referent: usize, generation: u64 },
    Raw,
    Low,
    MutableOutput,
    WriteResult,
    Unit,
}

pub(super) fn check_source(
    inputs: &ValidatedConditionalCompilerProofInputsV1,
    kir: &ProgramShape,
    symbol: &str,
) -> Result<Recipe, Error> {
    check_source_shape(inputs.semantic_mir(), inputs.correspondence(), kir, symbol)
}

pub(super) fn check_source_shape(
    source: &AdmittedInertSemanticMirV1,
    correspondence: &Correspondence,
    kir: &ProgramShape,
    symbol: &str,
) -> Result<Recipe, Error> {
    let [root] = source.roots() else {
        return Err(Error::Source("root"));
    };
    let selection = source
        .select_kernel_body_for_root_v1(*root)
        .ok_or(Error::Source("selection"))?;
    let function_id = selection.body().index();
    let function = &source.functions()[function_id as usize];
    let entry = source.functions()[root.index() as usize]
        .kernel_entry()
        .ok_or(Error::Source("entry"))?;
    if entry.export_symbol().as_bytes() != symbol.as_bytes() {
        return Err(Error::Source("entry symbol"));
    }
    check_body(source, correspondence, kir, function_id, function)
}

fn check_body(
    source: &AdmittedInertSemanticMirV1,
    correspondence: &Correspondence,
    kir: &ProgramShape,
    function_id: u32,
    function: &SemanticFunctionDeclV1,
) -> Result<Recipe, Error> {
    if function.abi().source_input_types().len() != 1
        || function.abi().source_argument_ownership()
            != [SemanticSourceArgumentOwnershipV1::ExclusiveOwner]
        || !unit(source, function.abi().source_output_type())
        || function.blocks().is_empty()
        || function.blocks().len() > 64
        || function.locals().len() > 128
    {
        return Err(Error::Source("signature"));
    }
    let mut arguments = function
        .locals()
        .iter()
        .enumerate()
        .filter(|(_, local)| matches!(local.role(), SemanticLocalRoleV1::Argument(_)));
    let (output, output_local) = arguments.next().ok_or(Error::Source("output"))?;
    if arguments.next().is_some() || output_local.role() != SemanticLocalRoleV1::Argument(0) {
        return Err(Error::Source("output"));
    }
    let mut parameters = correspondence
        .parameter_bindings()
        .iter()
        .filter(|binding| binding.semantic_function() == function_id);
    let parameter = parameters.next().ok_or(Error::Source("parameter"))?;
    if parameters.next().is_some()
        || parameter.semantic_local() as usize != output
        || parameter.kernel_ir_value() != kir.output.0
        || kir.values.get(&kir.output) != Some(&Value::Output)
    {
        return Err(Error::Source("parameter"));
    }

    let mut statement_spans = BTreeMap::new();
    for span in correspondence
        .statement_spans()
        .iter()
        .filter(|s| s.semantic_function() == function_id)
    {
        if statement_spans
            .insert(
                (span.semantic_block(), span.statement()),
                (
                    span.kernel_ir_block(),
                    span.first_operation(),
                    span.operation_count(),
                ),
            )
            .is_some()
        {
            return Err(Error::Source("duplicate statement span"));
        }
    }
    let mut terminator_spans = BTreeMap::new();
    for span in correspondence
        .terminator_spans()
        .iter()
        .filter(|s| s.semantic_function() == function_id)
    {
        if terminator_spans
            .insert(
                span.semantic_block(),
                (
                    span.kernel_ir_block(),
                    span.first_operation(),
                    span.operation_count(),
                ),
            )
            .is_some()
        {
            return Err(Error::Source("duplicate terminator span"));
        }
    }
    let mut recipe = Recipe::default();
    let origin = Origin([
        function_id,
        u32::MAX,
        0,
        output as u32,
        output_local.ty().index(),
    ]);
    let parameter = recipe.push(origin, Expr::Output)?;
    let mut state = SourceState {
        source,
        function,
        output,
        locals: BTreeMap::from([(output, Local::Output)]),
        generations: BTreeMap::new(),
        expressions: BTreeMap::from([(output, parameter)]),
        recipe,
        origin,
    };
    let mut visited = BTreeSet::new();
    let mut consumed = BTreeSet::new();
    let mut block_index = function.entry().index();
    let mut phase = 0;
    let mut witness = None;
    let mut raw = None;
    loop {
        if !visited.insert(block_index) {
            return Err(Error::Source("cycle"));
        }
        let block = function
            .blocks()
            .get(block_index as usize)
            .ok_or(Error::Source("block"))?;
        for (index, statement) in block.statements().iter().enumerate() {
            state.origin = Origin([function_id, block_index, index as u32, u32::MAX, u32::MAX]);
            let expected = state.statement(statement)?;
            let span = statement_spans
                .remove(&(block_index, index as u32))
                .ok_or(Error::Source("statement span"))?;
            check_span(kir, span, expected, &mut consumed)?;
            state.recipe.span(state.origin, span)?;
        }
        let span = terminator_spans
            .remove(&block_index)
            .ok_or(Error::Source("terminator span"))?;
        state.origin = Origin([
            function_id,
            block_index,
            block.statements().len() as u32,
            u32::MAX,
            u32::MAX,
        ]);
        state.recipe.span(state.origin, span)?;
        match block.terminator().kind() {
            SemanticTerminatorKindV1::Goto(edge) if edge.role() == SemanticEdgeRoleV1::Goto => {
                check_span(kir, span, &[], &mut consumed)?;
                block_index = edge.target().index();
            }
            SemanticTerminatorKindV1::Return if phase == 3 => {
                check_span(kir, span, &[], &mut consumed)?;
                break;
            }
            SemanticTerminatorKindV1::Call(call) => {
                if call.unwind() != SemanticUnwindActionV1::Unreachable
                    || !call.variadic_argument_abis().is_empty()
                {
                    return Err(Error::Source("call control"));
                }
                let destination = call
                    .destination()
                    .ok_or(Error::Source("call destination"))?;
                if destination.edge().role() != SemanticEdgeRoleV1::CallReturn {
                    return Err(Error::Source("call edge"));
                }
                let local = state.plain(destination.place())?;
                state.origin.0[3] = local as u32;
                state.origin.0[4] = destination.place().ty().index();
                if local == output {
                    return Err(Error::Source("output reassignment"));
                }
                let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
                    source.callables().get(call.callee().index() as usize)
                else {
                    return Err(Error::Source("callee"));
                };
                let tag = match (phase, operation) {
                    (
                        0,
                        SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                            index_witness,
                            raw_index,
                        },
                    ) if call.arguments().is_empty()
                        && destination.place().ty() == *index_witness
                        && unsigned(source, *raw_index, 64) =>
                    {
                        witness = Some(*index_witness);
                        raw = Some(*raw_index);
                        check_span(kir, span, &[Some(Value::Index)], &mut consumed)?;
                        (
                            Local::Index,
                            state.recipe.push(state.origin, Expr::ThreadIndex)?,
                        )
                    }
                    (
                        1,
                        SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
                            index_witness,
                            raw_index,
                        },
                    ) if Some(*index_witness) == witness
                        && Some(*raw_index) == raw
                        && destination.place().ty() == *raw_index =>
                    {
                        let [index] = call.arguments() else {
                            return Err(Error::Source("get arity"));
                        };
                        let (tag, input) = state.operand(index)?;
                        if !matches!(tag, Local::SharedIndex { .. }) {
                            return Err(Error::Source("get receiver"));
                        }
                        check_span(kir, span, &[], &mut consumed)?;
                        (
                            Local::Raw,
                            state.recipe.push(state.origin, Expr::IndexGet(input))?,
                        )
                    }
                    (
                        2,
                        SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
                            disjoint_slice,
                            witness: write_witness,
                            element,
                            raw_index,
                            index_space: SemanticDisjointIndexSpaceV1::Index1d,
                            kind: SemanticWriteOnlyDisjointWriteKindV1::Thread { disjoint: false },
                        },
                    ) if *disjoint_slice == output_local.ty()
                        && Some(*write_witness) == witness
                        && Some(*raw_index) == raw
                        && unsigned(source, *element, 32)
                        && matches!(
                            source.types()[destination.place().ty().index() as usize].shape(),
                            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
                        ) =>
                    {
                        let [output, index, value] = call.arguments() else {
                            return Err(Error::Source("write arity"));
                        };
                        let (output_tag, output_expression) = state.operand(output)?;
                        let (index_tag, index_expression) = state.consume_index(index)?;
                        let (value_tag, value_expression) = state.operand(value)?;
                        if value.ty() != *element
                            || output_tag != Local::MutableOutput
                            || index.ty() != *write_witness
                            || index_tag != Local::Index
                            || value_tag != Local::Low
                        {
                            return Err(Error::Source("write operands"));
                        }
                        check_span(
                            kir,
                            span,
                            &[
                                Some(Value::Length),
                                Some(Value::InBounds),
                                Some(Value::Zero),
                                Some(Value::SafeIndex),
                                Some(Value::Base),
                                Some(Value::Pointer),
                                None,
                            ],
                            &mut consumed,
                        )?;
                        state.recipe.set_effect(
                            state.origin,
                            Effect::MirWrite {
                                output: output_expression,
                                index: index_expression,
                                value: value_expression,
                            },
                        )?;
                        (
                            Local::WriteResult,
                            state.recipe.push(state.origin, Expr::WriteAccepted)?,
                        )
                    }
                    _ => return Err(Error::Source("call sequence")),
                };
                state.assign(local, tag)?;
                phase += 1;
                block_index = destination.edge().target().index();
            }
            _ => return Err(Error::Source("control flow")),
        }
    }
    if visited.len() != function.blocks().len()
        || !statement_spans.is_empty()
        || !terminator_spans.is_empty()
        || consumed.len() != kir.operations.len()
    {
        return Err(Error::Source("complete coverage"));
    }
    Ok(state.recipe)
}

fn check_span(
    kir: &ProgramShape,
    span: (u32, u32, u32),
    expected: &[Option<Value>],
    consumed: &mut BTreeSet<(u32, u32)>,
) -> Result<(), Error> {
    let (block, first, count) = span;
    if count as usize != expected.len() {
        return Err(Error::Source("span length"));
    }
    for (index, expected) in expected.iter().enumerate() {
        let site = (
            block,
            first
                .checked_add(index as u32)
                .ok_or(Error::Source("span overflow"))?,
        );
        if kir.operations.get(&site) != Some(expected) || !consumed.insert(site) {
            return Err(Error::Source("span operation"));
        }
    }
    Ok(())
}

struct SourceState<'a> {
    source: &'a AdmittedInertSemanticMirV1,
    function: &'a SemanticFunctionDeclV1,
    output: usize,
    locals: BTreeMap<usize, Local>,
    generations: BTreeMap<usize, u64>,
    expressions: BTreeMap<usize, Id>,
    recipe: Recipe,
    origin: Origin,
}

impl SourceState<'_> {
    fn consume_index(&mut self, operand: &SemanticOperandV1) -> Result<(Local, Id), Error> {
        let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
            return Err(Error::Source("write witness"));
        };
        let local = self.plain(place)?;
        if self.locals.get(&local) != Some(&Local::Index) {
            return Err(Error::Source("write witness"));
        }
        // Post-borrow-check MIR may mark this terminal non-Copy witness use as Copy.
        // The recognized consumer still spends the witness and invalidates its borrows.
        let expression = self.expression(local)?;
        self.invalidate(local)?;
        Ok((Local::Index, expression))
    }
    fn invalidate(&mut self, local: usize) -> Result<(), Error> {
        let generation = self.generations.entry(local).or_default();
        *generation = generation
            .checked_add(1)
            .ok_or(Error::Source("local generation"))?;
        self.locals.remove(&local);
        self.expressions.remove(&local);
        Ok(())
    }
    fn assign(&mut self, local: usize, (tag, expression): (Local, Id)) -> Result<(), Error> {
        self.invalidate(local)?;
        self.locals.insert(local, tag);
        self.expressions.insert(local, expression);
        Ok(())
    }
    fn expression(&self, local: usize) -> Result<Id, Error> {
        self.expressions
            .get(&local)
            .copied()
            .ok_or(Error::Source("recipe local"))
    }
    fn plain(&self, place: &SemanticPlaceV1) -> Result<usize, Error> {
        let index = place.local().index() as usize;
        if !place.projections().is_empty()
            || self
                .function
                .locals()
                .get(index)
                .is_none_or(|local| local.ty() != place.ty())
        {
            return Err(Error::Source("projected or mistyped local"));
        }
        Ok(index)
    }
    fn operand(&mut self, operand: &SemanticOperandV1) -> Result<(Local, Id), Error> {
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                let local = self.plain(place)?;
                let tag = *self
                    .locals
                    .get(&local)
                    .ok_or(Error::Source("uninitialized local"))?;
                let expression = self.expression(local)?;
                if let Local::SharedIndex {
                    referent,
                    generation,
                } = tag
                    && (self.locals.get(&referent) != Some(&Local::Index)
                        || self.generations.get(&referent).copied().unwrap_or(0) != generation)
                {
                    return Err(Error::Source("invalidated witness borrow"));
                }
                if matches!(operand, SemanticOperandV1::Move(_)) {
                    if tag == Local::Output {
                        return Err(Error::Source("output move"));
                    }
                    self.invalidate(local)?;
                } else if matches!(tag, Local::Output | Local::Index) {
                    return Err(Error::Source("owner copy"));
                }
                Ok((tag, expression))
            }
            SemanticOperandV1::Constant(constant)
                if matches!(constant.value(), SemanticConstantValueV1::ZeroSized)
                    && unit(self.source, constant.ty()) =>
            {
                Ok((Local::Unit, self.recipe.push(self.origin, Expr::Unit)?))
            }
            _ => Err(Error::Source("operand")),
        }
    }
    fn statement(
        &mut self,
        statement: &SemanticStatementV1,
    ) -> Result<&'static [Option<Value>], Error> {
        let mut expected: &'static [Option<Value>] = &[];
        match statement.kind() {
            SemanticStatementKindV1::Nop => {}
            SemanticStatementKindV1::StorageLive(local)
            | SemanticStatementKindV1::StorageDead(local) => {
                let index = local.index() as usize;
                if index == self.output {
                    return Err(Error::Source("output storage"));
                }
                self.invalidate(index)?;
            }
            SemanticStatementKindV1::Assign(assignment) => {
                let destination = self.plain(assignment.destination())?;
                self.origin.0[3] = destination as u32;
                self.origin.0[4] = assignment.destination().ty().index();
                if destination == self.output
                    || assignment.destination().ty() != assignment.value().result_type()
                {
                    return Err(Error::Source("assignment destination"));
                }
                let tag = match assignment.value().kind() {
                    SemanticRvalueKindV1::Use(operand)
                        if operand.ty() == assignment.destination().ty() =>
                    {
                        let (tag, input) = self.operand(operand)?;
                        (tag, self.recipe.push(self.origin, Expr::Use(input))?)
                    }
                    SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::Integer,
                        operand,
                    } if unsigned(self.source, operand.ty(), 64)
                        && unsigned(self.source, assignment.destination().ty(), 32) =>
                    {
                        let (tag, input) = self.operand(operand)?;
                        if tag != Local::Raw {
                            return Err(Error::Source("cast input"));
                        }
                        expected = &[Some(Value::IndexU64), Some(Value::TruncatedIndex)];
                        (
                            Local::Low,
                            self.recipe
                                .push(self.origin, Expr::IntegerTruncate(input))?,
                        )
                    }
                    SemanticRvalueKindV1::Borrow { kind, place } => {
                        let source = self.plain(place)?;
                        let Some(SemanticTypeShapeV1::Pointer(pointer)) = self
                            .source
                            .types()
                            .get(assignment.destination().ty().index() as usize)
                            .map(|ty| ty.shape())
                        else {
                            return Err(Error::Source("borrow type"));
                        };
                        if pointer.kind() != SemanticPointerKindV1::Reference
                            || pointer.pointee() != place.ty()
                        {
                            return Err(Error::Source("borrow type"));
                        }
                        match (kind, self.locals.get(&source)) {
                            (SemanticBorrowKindV1::Shared, Some(Local::Index))
                                if pointer.mutability() == SemanticMutabilityV1::Immutable =>
                            {
                                let tag = Local::SharedIndex {
                                    referent: source,
                                    generation: self.generations.get(&source).copied().unwrap_or(0),
                                };
                                let input = self.expression(source)?;
                                (
                                    tag,
                                    self.recipe.push(self.origin, Expr::SharedBorrow(input))?,
                                )
                            }
                            (SemanticBorrowKindV1::Mutable, Some(Local::Output))
                                if pointer.mutability() == SemanticMutabilityV1::Mutable =>
                            {
                                let input = self.expression(source)?;
                                (
                                    Local::MutableOutput,
                                    self.recipe.push(self.origin, Expr::MutableBorrow(input))?,
                                )
                            }
                            _ => return Err(Error::Source("borrow origin")),
                        }
                    }
                    _ => return Err(Error::Source("rvalue")),
                };
                self.assign(destination, tag)?;
            }
            _ => return Err(Error::Source("statement")),
        }
        Ok(expected)
    }
}

fn unsigned(source: &AdmittedInertSemanticMirV1, ty: SemanticTypeIdV1, bits: u16) -> bool {
    matches!(source.types().get(ty.index() as usize).map(|ty| ty.shape()),
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed: false, bits: actual })) if *actual == bits)
}
fn unit(source: &AdmittedInertSemanticMirV1, ty: SemanticTypeIdV1) -> bool {
    matches!(
        source.types().get(ty.index() as usize).map(|ty| ty.shape()),
        Some(SemanticTypeShapeV1::Unit)
    )
}

#[cfg(test)]
mod tests;
