//! Genuine original-input block emission. This tests a constructed recipe only:
//! no mandatory-verifier success, normal admission, LLVM emission or GPU claim.
use super::*;
use crate::production_ranked_projection_v1::{
    bf16_nominal_source_preparation_v1::with_nominal_rich_source_preparation_v1,
    canonical_assertion_facts_v1::{
        ProjectedAssertionConditionV1 as Condition, ProjectedAssertionFactsV1,
        with_nominal_canonical_facts_observation_v1, with_nominal_prepared_control_flow_v1,
    },
};
use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
use fe2o3_lower_mir_kernel::{
    Bf16NominalCallQueryErrorV1 as QueryError, CheckedBf16CallInstanceV1,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticBasicBlockV1, SemanticStatementV1};
use std::panic::{AssertUnwindSafe, catch_unwind};
type Q<T> = std::result::Result<T, QueryError>;
const MAX_ROWS: usize = 32;
fn checked_add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| resource(Resource::Arithmetic))
}
fn checked_mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b)
        .ok_or_else(|| resource(Resource::Arithmetic))
}
// Selected observer-frame prepayment, not a larger canonical phase cap.
// Keep the existing four-owner/four-oracle/eight-index-array multiplicities.
// Pending owns its larger checked-view Controls inline through BlockStream.
fn header_bytes_for(
    pending: usize,
    rows: usize,
    observation: usize,
    indices: usize,
) -> Result<usize> {
    let mut bytes = 4096usize;
    for (size, count) in [(pending, 4), (rows, 4), (observation, 4), (indices, 8)] {
        bytes = checked_add(bytes, checked_mul(size, count)?)?;
    }
    Ok(bytes)
}
fn header_bytes() -> Result<usize> {
    header_bytes_for(
        size_of::<PendingActualRootPrefixIndicesV1>(),
        size_of::<[ExpectedRow; MAX_ROWS]>(),
        size_of::<Observation>(),
        size_of::<[usize; MAX_ROWS]>(),
    )
}
fn prepay_headers(budget: &mut Budget<'_>) -> Q<usize> {
    budget.check_prior_denials_v1()?;
    let bytes = header_bytes().map_err(projection)?;
    let work = checked_mul(4, bytes).map_err(projection)?;
    budget.charge_work(work)?;
    budget.reserve_storage(bytes)?;
    // The existing lexical owner retains these credits through all callbacks
    // and releases only after Pending has dropped; no paid object escapes.
    Ok(bytes)
}
fn projection(e: Error) -> QueryError {
    match e {
        Error::CanonicalAssertions(crate::production_ranked_projection_v1::canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(r))=>QueryError::Resource(r),
        Error::Incomplete(s)|Error::Unsupported(s)=>QueryError::Unavailable(s),
        _=>QueryError::Unavailable("genuine nominal block stream refused"),
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Observation {
    source_blocks: usize,
    ranked_blocks: usize,
    sites: usize,
    private_reads: usize,
    private_writes: usize,
    guarded: usize,
    tensors: usize,
    reads: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExpectedRow {
    kind: &'static str,
    count: usize,
    targets: [usize; MAX_ROWS],
}
impl ExpectedRow {
    fn empty(kind: &'static str) -> Self {
        Self {
            kind,
            count: 0,
            targets: [0; MAX_ROWS],
        }
    }
    fn append(&mut self, target: usize, blocks: usize) {
        assert!(target < blocks && blocks <= MAX_ROWS);
        if self.targets[..self.count].contains(&target) {
            return;
        }
        assert!(self.count < MAX_ROWS);
        self.targets[self.count] = target;
        self.count += 1;
    }
    fn branch(target: usize, blocks: usize) -> Self {
        let mut row = Self::empty("branch");
        row.append(target, blocks);
        row
    }
}
fn scan_work(edges: usize) -> Result<usize> {
    checked_add(1024, checked_mul(edges, MAX_ROWS + 8)?)
}
fn row_matches(actual: &ProjectedCfgTerminatorV1, expected: &ExpectedRow) -> bool {
    let targets = &expected.targets[..expected.count];
    match actual {
        ProjectedCfgTerminatorV1::AbsentMaterialized => {
            expected.kind == "absent" && targets.is_empty()
        }
        ProjectedCfgTerminatorV1::Branch(target) => {
            expected.kind == "branch" && targets == [*target]
        }
        ProjectedCfgTerminatorV1::AnalysisSplit {
            first_block,
            second_block,
        } => expected.kind == "split" && targets == [*first_block, *second_block],
        ProjectedCfgTerminatorV1::AnalysisMultiSplit { blocks } => {
            expected.kind == "multi" && targets == blocks.as_slice()
        }
        ProjectedCfgTerminatorV1::Return => expected.kind == "return" && targets.is_empty(),
        ProjectedCfgTerminatorV1::Trap => expected.kind == "trap" && targets.is_empty(),
        ProjectedCfgTerminatorV1::Predicate { .. } | ProjectedCfgTerminatorV1::ExactSwitch(_) => {
            false
        }
    }
}
// This oracle does not call either shared terminator projector or its switch
// helper. Fixed scratch has no dynamic backing and all source walks are prepaid.
fn expected_row(
    function: &SemanticFunctionDeclV1,
    index: usize,
    callables: &[SemanticCallableDeclV1],
    proved: bool,
    facts: &mut dyn ProjectedAssertionFactsV1,
) -> Result<ExpectedRow> {
    use SemanticTerminatorKindV1 as T;
    let blocks = function.blocks().len();
    assert!((1..=MAX_ROWS).contains(&blocks) && index < blocks);
    facts.charge_private_array_work(1024)?;
    if !facts.is_materialized_block(index)? {
        return Ok(ExpectedRow::empty("absent"));
    }
    let block = &function.blocks()[index];
    match block.terminator().kind() {
        T::Goto(edge) | T::Drop { target: edge, .. } => {
            Ok(ExpectedRow::branch(edge.target().index() as usize, blocks))
        }
        T::Call(call) => {
            assert!(!matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_)));
            if let Some(destination) = call.destination() {
                Ok(ExpectedRow::branch(
                    destination.edge().target().index() as usize,
                    blocks,
                ))
            } else if matches!(
                callables.get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::Trap,
                    ..
                })
            ) {
                Ok(ExpectedRow::empty("trap"))
            } else {
                Ok(ExpectedRow::empty("return"))
            }
        }
        T::Assert {
            expected,
            message,
            target,
            ..
        } => {
            if !matches!(message, SemanticAssertMessageV1::BoundsCheck { .. }) {
                let condition = facts.condition(index, *expected, target.target())?;
                match condition {
                    Condition::Bool(value) => assert_eq!(value, *expected),
                    Condition::Dormant
                    | Condition::Unknown
                    | Condition::Dynamic
                    | Condition::ElidedByExistingRule => assert!(proved),
                }
            }
            Ok(ExpectedRow::branch(
                target.target().index() as usize,
                blocks,
            ))
        }
        T::SwitchInt { targets, .. } => {
            let capacity = targets
                .values()
                .len()
                .checked_add(1)
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            assert!(capacity <= crate::production_ranked_projection_v1::MAX_RANKED_BOUNDS_EDGES);
            facts.charge_private_array_work(scan_work(capacity)?)?;
            let otherwise = targets.otherwise().target().index() as usize;
            assert!(otherwise < blocks);
            let fallback = &function.blocks()[otherwise];
            let elided = targets.values().len() == 2
                && targets.values()[0].value() == 0
                && targets.values()[1].value() == 1
                && fallback.statements().is_empty()
                && matches!(fallback.terminator().kind(), T::Unreachable);
            let mut row = ExpectedRow::empty("pending");
            for target in targets.values() {
                row.append(target.edge().target().index() as usize, blocks);
            }
            if !elided {
                row.append(otherwise, blocks);
            }
            row.kind = match row.count {
                0 => panic!("genuine source switch has no successor"),
                1 => "branch",
                2 => "split",
                _ => "multi",
            };
            Ok(row)
        }
        T::FalseEdge { .. } => panic!("genuine source retains an unsupported false edge"),
        T::Return
        | T::TailCall(_)
        | T::UnwindResume
        | T::UnwindTerminate
        | T::Abort
        | T::Unreachable => Ok(ExpectedRow::empty("return")),
    }
}

fn op_work(op: &ProductionRankedOperationV1) -> Result<usize> {
    let n = match op {
        ProductionRankedOperationV1::ViewInSpace {
            shape,
            dynamic_extents,
            ..
        } => checked_add(shape.len(), dynamic_extents.len())?,
        ProductionRankedOperationV1::Access { indices, .. } => indices.len(),
        ProductionRankedOperationV1::ExecutionLayout { .. }
        | ProductionRankedOperationV1::InvocationIndex { .. }
        | ProductionRankedOperationV1::IndexConstant { .. }
        | ProductionRankedOperationV1::AllocationEffect { .. }
        | ProductionRankedOperationV1::TensorLayout { .. } => 0,
        _ => {
            return Err(Error::Incomplete(
                "genuine oracle operation outside fixture",
            ));
        }
    };
    checked_add(128, checked_mul(n, 32)?)
}
fn original_operation<'a>(
    prefix: &'a [ProductionRankedOperationV1],
    id: ProductionRankedValueIdV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<&'a ProductionRankedOperationV1> {
    for op in prefix {
        resources.work(32)?;
        let result = match op {
            ProductionRankedOperationV1::ViewInSpace { result, .. }
            | ProductionRankedOperationV1::IndexConstant { result, .. }
            | ProductionRankedOperationV1::InvocationIndex { result, .. } => Some(*result),
            _ => None,
        };
        if result == Some(id) {
            return Ok(op);
        }
    }
    Err(Error::Incomplete(
        "genuine oracle original prefix ID absent",
    ))
}
struct RowOracle<'a, 'b, 'g> {
    view: &'a ActualRootBlockStreamV1<'b, 'g>,
    rich: &'a RichNominalSourceTablesV1<'a>,
    types: &'a [SemanticTypeDeclV1],
    block: usize,
    cursor: usize,
    counts: &'a mut Observation,
}
impl RowOracle<'_, '_, '_> {
    fn place(
        &mut self,
        place: &SemanticPlaceV1,
        access: AccessKindAttr,
        site: ProjectedSemanticAccessSiteV1,
        source: SemanticSourceProvenanceV1,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<()> {
        resources.work(checked_add(
            128,
            checked_mul(place.projections().len(), 32)?,
        )?)?;
        let projection = place.projections();
        if projection.is_empty()
            || projection.iter().all(|p| {
                matches!(
                    p.kind(),
                    SemanticProjectionKindV1::Field(_)
                        | SemanticProjectionKindV1::Downcast(_)
                        | SemanticProjectionKindV1::OpaqueCast
                        | SemanticProjectionKindV1::Subtype
                )
            })
        {
            return Ok(());
        }
        let row = &self.view.rows[self.block];
        if matches!(projection[0].kind(), SemanticProjectionKindV1::Dereference) {
            assert!(projection[1..].iter().all(|p| matches!(
                p.kind(),
                SemanticProjectionKindV1::Field(_)
                    | SemanticProjectionKindV1::Downcast(_)
                    | SemanticProjectionKindV1::OpaqueCast
                    | SemanticProjectionKindV1::Subtype
            )));
            let origin = self.view.origins.origins[place.local().index() as usize]
                .expect("actual source checked-reference origin");
            if let Some(availability) = origin.availability {
                assert!(capability_availability_allows(
                    self.rich.option_dominance(),
                    self.rich.enum_payload_dominance(),
                    availability,
                    SemanticBlockIdV1::from_index(self.block as u32)
                ));
            }
            let i = match origin.source {
                CheckedReferenceSourceV1::GuardedAccess(i) => i,
                CheckedReferenceSourceV1::ProjectedSharedBorrow => {
                    assert_eq!(access, AccessKindAttr::Read);
                    return Ok(());
                }
            };
            let expected = &self.view.guarded.accesses[i];
            resources.work(checked_add(
                256,
                checked_mul(
                    checked_add(expected.indices.len(), expected.comparisons.len())?,
                    32,
                )?,
            )?)?;
            let Some(ProjectedBlockItemV1::Guarded(actual)) = row.items.get(self.cursor) else {
                panic!("missing original checked access")
            };
            assert_eq!(actual.view, expected.view);
            assert_eq!(actual.indices, expected.indices);
            assert_eq!(actual.comparisons, expected.comparisons);
            assert_eq!(actual.checked_success, expected.checked_success);
            assert_eq!(actual.output_extent, expected.output_extent);
            assert_eq!(actual.memory_space, expected.memory_space);
            assert_eq!(actual.access, access);
            assert_eq!(actual.source, source);
            assert_eq!(actual.semantic_site, Some(site));
            self.counts.guarded += 1;
        } else {
            // Independent source array/constant oracle. It never calls the new
            // coordinate classifier, source-use walker or private-view emitter.
            let [projection] = projection else {
                panic!("fixture private projection arity")
            };
            let decl = &self.view.function().locals()[place.local().index() as usize];
            assert!(!decl.role().is_entry_argument());
            let SemanticTypeShapeV1::Array { length, element } =
                self.types[decl.ty().index() as usize].shape()
            else {
                panic!("fixture array")
            };
            assert_eq!(place.ty(), *element);
            assert_eq!(projection.result_type(), *element);
            let coordinate = match projection.kind() {
                SemanticProjectionKindV1::ConstantIndex {
                    offset, from_end, ..
                } => {
                    if from_end {
                        length.checked_sub(offset).unwrap()
                    } else {
                        offset
                    }
                }
                SemanticProjectionKindV1::Index(local) => {
                    self.rich.constants()[local.index() as usize].expect("fixture constant index")
                }
                _ => panic!("fixture private projection"),
            };
            assert!(coordinate < *length);
            let Some(ProjectedBlockItemV1::Effect {
                operation:
                    ProductionRankedOperationV1::Access {
                        kind,
                        view: ProductionRankedValueV1::Local(view),
                        indices,
                    },
                source: Some(actual),
            }) = row.items.get(self.cursor)
            else {
                panic!("missing original private array access")
            };
            assert_eq!(*kind, access);
            assert_eq!(actual.access, access);
            assert_eq!(actual.memory_space, MemorySpaceAttr::Private);
            assert_eq!(actual.source, source);
            assert_eq!(actual.semantic_site, Some(site));
            assert_eq!(actual.output_extent, None);
            let [ProductionRankedValueV1::Local(index)] = indices.as_slice() else {
                panic!("private coordinate namespace")
            };
            let ProductionRankedOperationV1::IndexConstant { value, .. } =
                original_operation(self.view.entry_operations(), *index, resources)?
            else {
                panic!("constant index operation")
            };
            assert_eq!(*value, coordinate);
            let ProductionRankedOperationV1::ViewInSpace {
                element_width,
                writable,
                shape,
                dynamic_extents,
                memory_space,
                allocation_origin,
                noalias_class,
                ..
            } = original_operation(self.view.entry_operations(), *view, resources)?
            else {
                panic!("private view operation")
            };
            assert_eq!(*element_width, type_width(self.types, *element)?);
            assert!(*writable);
            assert_eq!(shape.as_slice(), &[*length]);
            assert!(dynamic_extents.is_empty());
            assert_eq!(*memory_space, MemorySpaceAttr::Private);
            let identity = PRIVATE_ALLOCATION_ORIGIN_TAG_V1
                .checked_add(place.local().index() as u64)
                .unwrap()
                .checked_add(1)
                .unwrap();
            assert_eq!((*allocation_origin, *noalias_class), (identity, identity));
            if access == AccessKindAttr::Read {
                self.counts.private_reads += 1
            } else {
                assert_eq!(access, AccessKindAttr::Write);
                self.counts.private_writes += 1
            }
        }
        self.cursor += 1;
        Ok(())
    }
    fn operand(
        &mut self,
        operand: &SemanticOperandV1,
        site: ProjectedSemanticAccessSiteV1,
        source: SemanticSourceProvenanceV1,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<()> {
        resources.work(16)?;
        match operand {
            SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p) => {
                self.place(p, AccessKindAttr::Read, site, source, resources)
            }
            SemanticOperandV1::Constant(_) => Ok(()),
        }
    }
    fn statement(
        &mut self,
        statement: &SemanticStatementV1,
        index: usize,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<()> {
        resources.work(64)?;
        let site = ProjectedSemanticAccessSiteV1 {
            block: self.block,
            statement: Some(index),
        };
        let source = statement.source();
        match statement.kind() {
            SemanticStatementKindV1::Assign(a) => {
                match a.value().kind() {
                    SemanticRvalueKindV1::Use(o)
                    | SemanticRvalueKindV1::Unary { operand: o, .. }
                    | SemanticRvalueKindV1::Cast { operand: o, .. } => {
                        self.operand(o, site, source, resources)?
                    }
                    SemanticRvalueKindV1::Binary { left, right, .. } => {
                        self.operand(left, site, source, resources)?;
                        self.operand(right, site, source, resources)?;
                    }
                    SemanticRvalueKindV1::CheckedBinary(b) => {
                        self.operand(b.left(), site, source, resources)?;
                        self.operand(b.right(), site, source, resources)?;
                    }
                    SemanticRvalueKindV1::UncheckedBinary(b) => {
                        self.operand(b.left(), site, source, resources)?;
                        self.operand(b.right(), site, source, resources)?;
                    }
                    SemanticRvalueKindV1::Aggregate(a) => {
                        for o in a.operands() {
                            self.operand(o, site, source, resources)?
                        }
                    }
                    SemanticRvalueKindV1::Load(load) => {
                        assert!(load.atomic().is_none());
                        self.place(load.source(), AccessKindAttr::Read, site, source, resources)?;
                    }
                    SemanticRvalueKindV1::Length(p) | SemanticRvalueKindV1::Discriminant(p) => {
                        self.place(p, AccessKindAttr::Read, site, source, resources)?
                    }
                    SemanticRvalueKindV1::AddressOf { place, .. }
                    | SemanticRvalueKindV1::Borrow { place, .. } => {
                        resources.work(place.projections().len())?;
                        assert!(!place.projections().iter().any(|p| matches!(
                            p.kind(),
                            SemanticProjectionKindV1::Index(_)
                                | SemanticProjectionKindV1::ConstantIndex { .. }
                                | SemanticProjectionKindV1::Subslice { .. }
                        )));
                    }
                }
                self.place(
                    a.destination(),
                    AccessKindAttr::Write,
                    site,
                    source,
                    resources,
                )?;
            }
            SemanticStatementKindV1::Store(s) => {
                assert!(s.atomic().is_none());
                self.operand(s.value(), site, source, resources)?;
                self.place(
                    s.destination(),
                    AccessKindAttr::Write,
                    site,
                    source,
                    resources,
                )?;
            }
            SemanticStatementKindV1::SetDiscriminant { place, .. }
            | SemanticStatementKindV1::Deinitialize(place) => {
                self.place(place, AccessKindAttr::Write, site, source, resources)?
            }
            SemanticStatementKindV1::Assume(o) => self.operand(o, site, source, resources)?,
            SemanticStatementKindV1::StorageLive(_)
            | SemanticStatementKindV1::StorageDead(_)
            | SemanticStatementKindV1::Nop => {}
            SemanticStatementKindV1::AtomicRmw(_)
            | SemanticStatementKindV1::AtomicCompareExchange(_) => {
                return Err(Error::Incomplete("genuine fixture does not cover atomics"));
            }
        }
        Ok(())
    }
    fn finish(
        &mut self,
        block: &SemanticBasicBlockV1,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<()> {
        resources.work(128)?;
        let site = ProjectedSemanticAccessSiteV1 {
            block: self.block,
            statement: None,
        };
        let source = block.terminator().source();
        match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => {
                for operand in call.arguments() {
                    self.operand(operand, site, source, resources)?
                }
                if let Some(dest) = call.destination() {
                    self.place(dest.place(), AccessKindAttr::Write, site, source, resources)?
                }
            }
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                self.operand(discriminant, site, source, resources)?
            }
            SemanticTerminatorKindV1::Assert { condition, .. } => {
                self.operand(condition, site, source, resources)?
            }
            SemanticTerminatorKindV1::Drop { .. } | SemanticTerminatorKindV1::TailCall(_) => {
                return Err(Error::Incomplete("genuine source has unsupported effect"));
            }
            _ => {}
        }
        let effects = &self.view.flow.effects().effects()[self.block];
        assert!(effects.transpose_workgroup.is_none() && effects.read_view.is_none());
        if let Some(layout) = &effects.layout {
            resources.work(op_work(layout)?)?;
            assert!(matches!(self.view.rows[self.block].items.get(self.cursor),
                Some(ProjectedBlockItemV1::Effect{operation,source:None}) if operation==layout));
            assert_eq!(
                layout,
                self.view.flow.effects().original().candidate().operation()
            );
            self.cursor += 1;
            self.counts.tensors += 1;
        }
        if let Some(read) = effects.global_read {
            assert!(matches!(self.view.rows[self.block].items.get(self.cursor),
                Some(ProjectedBlockItemV1::Effect{operation:ProductionRankedOperationV1::AllocationEffect{
                    kind:AccessKindAttr::Read,memory_space:MemorySpaceAttr::Global,allocation_origin,noalias_class},source:Some(s)})
                    if *allocation_origin==read.allocation_origin&&*noalias_class==read.noalias_class
                        &&s.source==source&&s.semantic_site==Some(site)&&s.access==AccessKindAttr::Read&&s.memory_space==MemorySpaceAttr::Global));
            self.cursor += 1;
            self.counts.reads += 1;
        }
        assert_eq!(
            self.cursor,
            self.view.rows[self.block].items.len(),
            "all source effects accounted in original order"
        );
        Ok(())
    }
}
fn check_block(
    view: &ActualRootBlockStreamV1<'_, '_>,
    index: usize,
    operations: &[ProductionRankedOperationV1],
    term: &ProductionRankedTerminatorV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    for op in operations {
        resources.work(op_work(op)?)?
    }
    resources.work(128)?;
    let block = &view.blocks()[index];
    assert_eq!(block.index_argument_count(), 0);
    assert_eq!(block.operations(), operations);
    assert_eq!(block.terminator(), term);
    Ok(())
}
fn inspect(
    view: &ActualRootBlockStreamV1<'_, '_>,
    rich: &RichNominalSourceTablesV1<'_>,
    context: &mut NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_>,
    rows: &mut [ExpectedRow; MAX_ROWS],
) -> Result<Observation> {
    let function = view.function();
    let count = function.blocks().len();
    assert!((1..=MAX_ROWS).contains(&count));
    assert_eq!(view.rows().len(), count);
    let semantic = context.facts.owner.semantic_ssa().source_semantic();
    context.with_facts(|facts| {
        for (i, row) in rows[..count].iter_mut().enumerate() {
            *row = expected_row(
                function,
                i,
                semantic.callables(),
                view.flow.assertions().decisions()[i],
                facts,
            )?;
            assert!(row_matches(&view.terminators()[i], row));
        }
        Ok(())
    })?;
    context.with_resources(|resources|{
        resources.work(header_bytes()?)?;
        let mut result=Observation{source_blocks:count,ranked_blocks:view.blocks().len(),sites:0,
            private_reads:0,private_writes:0,guarded:0,tensors:0,reads:0};
        for (i,block) in function.blocks().iter().enumerate(){
            let mut oracle=RowOracle{view,rich,types:semantic.types(),block:i,cursor:0,counts:&mut result};
            for (j,statement) in block.statements().iter().enumerate(){oracle.statement(statement,j,resources)?}
            oracle.finish(block,resources)?;
        }
        assert_eq!((result.tensors,result.reads),(1,2));
        assert!(result.private_reads>0&&result.guarded>0,"fixture must exercise actual array reads and guarded writes");
        assert_eq!(view.tensor_sites().len(),1);
        let tensor=view.tensor_sites()[0];
        assert_eq!(tensor.source_block,view.flow.effects().source_block().index()as usize);
        assert_eq!(&view.blocks()[tensor.ranked_block].operations()[tensor.ranked_operation],
            view.flow.effects().original().candidate().operation());
        // Independent fixed-array graph and expansion census, based on source
        // rows above. No emitter layout, successor, target or source-use helper.
        let mut live=[false;MAX_ROWS];let mut queue=[0usize;MAX_ROWS];
        let mut qlen=1;let mut qread=0;let entry=function.entry().index()as usize;
        queue[0]=entry;live[entry]=true;
        while qread<qlen{
            let source=queue[qread];qread+=1;
            resources.work(checked_add(32,checked_mul(rows[source].count,8)?)?)?;
            for &target in &rows[source].targets[..rows[source].count]{
                if !live[target]{live[target]=true;queue[qlen]=target;qlen+=1}
            }
        }
        let mut bases=[None;MAX_ROWS];let mut cursor=1usize;
        for i in 0..count{
            resources.work(32)?;
            if !live[i]{assert_eq!(view.source_block_base(i),None);continue}
            bases[i]=Some(cursor);assert_eq!(view.source_block_base(i),Some(cursor));
            cursor+=1;
            for item in &view.rows()[i].items{
                resources.work(32)?;
                if let ProjectedBlockItemV1::Guarded(g)=item{cursor=checked_add(cursor,checked_add(g.comparisons.len(),2)?)?}
            }
            if rows[i].count>2{cursor=checked_add(cursor,rows[i].count-2)?}
        }
        assert_eq!(cursor,view.blocks().len());
        check_block(view,0,view.entry_operations(),&ProductionRankedTerminatorV1::Branch{target:bases[entry].unwrap()as u32},resources)?;
        let mut access_cursor=0usize;let mut tensor_cursor=0usize;
        for i in 0..count{
            let Some(mut block)=bases[i]else{continue};
            let mut op=0usize;
            for item in &view.rows()[i].items{
                resources.work(128)?;
                match item{
                    ProjectedBlockItemV1::Effect{operation,source}=>{
                        resources.work(op_work(operation)?)?;
                        assert_eq!(view.blocks()[block].operations().get(op),Some(operation));
                        if matches!(operation,ProductionRankedOperationV1::TensorLayout{..}){
                            assert_eq!(view.tensor_sites()[tensor_cursor],emission::TensorSite{source_block:i,ranked_block:block,ranked_operation:op});tensor_cursor+=1;
                        }
                        if let Some(s)=source{
                            assert_source(view,access_cursor,block,op,s,resources)?;access_cursor+=1;
                        }op+=1;
                    }
                    ProjectedBlockItemV1::Guarded(g)=>{
                        let memory=checked_add(block,g.comparisons.len())?;let failure=memory+1;let next=failure+1;
                        for (n,(lhs,rhs)) in g.comparisons.iter().enumerate(){
                            resources.work(128)?;
                            assert_eq!(view.blocks()[block].operations().len(),op);
                            assert_eq!(view.blocks()[block].terminator(),&ProductionRankedTerminatorV1::IndexLessThan{
                                lhs:*lhs,rhs:*rhs,true_block:(if n+1==g.comparisons.len(){memory}else{block+1})as u32,false_block:failure as u32});
                            block+=1;op=0;
                        }
                        assert_eq!(block,memory);assert_eq!(view.blocks()[memory].operations().len(),1);
                        let ProductionRankedOperationV1::Access{kind,view:v,indices}=&view.blocks()[memory].operations()[0]else{panic!("actual guard access")};
                        resources.work(checked_mul(g.indices.len(),32)?)?;
                        assert_eq!(*kind,g.access);assert_eq!(*v,ProductionRankedValueV1::Local(g.view));assert_eq!(indices,&g.indices);
                        assert_eq!(view.blocks()[memory].terminator(),&ProductionRankedTerminatorV1::Branch{target:next as u32});
                        check_block(view,failure,&[],&ProductionRankedTerminatorV1::Trap,resources)?;
                        assert_source(view,access_cursor,memory,0,&ProjectedEffectSourceV1{
                            access:g.access,memory_space:g.memory_space,source:g.source,output_extent:g.output_extent,semantic_site:g.semantic_site},resources)?;
                        access_cursor+=1;block=next;
                    }
                    _=>return Err(Error::Incomplete("genuine emitted row outside profile")),
                }
            }
            assert_eq!(view.blocks()[block].operations().len(),op);
            let row=rows[i];let targets=&row.targets[..row.count];
            match row.kind{
                "branch"=>assert_eq!(view.blocks()[block].terminator(),&ProductionRankedTerminatorV1::Branch{target:bases[targets[0]].unwrap()as u32}),
                "return"=>assert_eq!(view.blocks()[block].terminator(),&ProductionRankedTerminatorV1::Return),
                "trap"=>assert_eq!(view.blocks()[block].terminator(),&ProductionRankedTerminatorV1::Trap),
                "split"|"multi"=>if !checked_switch_oracle(view,semantic.types(),semantic.callables(),i,block,&bases,resources)? { for n in 0..row.count-1{
                    resources.work(128)?;
                    let second=if n+2==row.count{bases[targets[n+1]].unwrap()}else{block+n+1};
                    if n>0{assert!(view.blocks()[block+n].operations().is_empty())}
                    assert_eq!(view.blocks()[block+n].terminator(),&ProductionRankedTerminatorV1::AnalysisSplit{
                        control_dependencies:Vec::new(),first_block:bases[targets[n]].unwrap()as u32,second_block:second as u32});
                }},
                _=>panic!("reachable genuine absent source"),
            }
            eprintln!("fe2o3-nominal-block-stream-row-v1 phase=callback source_block={} ranked_base={} items={} successors={:?}",i,bases[i].unwrap(),view.rows()[i].items.len(),targets);
        }
        assert_eq!(access_cursor,view.access_sources().len());assert_eq!(tensor_cursor,view.tensor_sites().len());
        assert_eq!(view.flow.checked_views().iter().flatten().count(),2);
        result.sites=access_cursor;Ok(result)
    })
}
// Independent source/discriminant oracle. It never calls the production query,
// namespace mapper or comparison builder. Genuine input identity is unchanged.
fn checked_switch_oracle(
    view: &ActualRootBlockStreamV1<'_, '_>,
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    source: usize,
    ranked: usize,
    bases: &[Option<usize>; MAX_ROWS],
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<bool> {
    resources.work(1024)?;
    let Some(fact) = view.flow.checked_views()[source] else {
        return Ok(false);
    };
    let function = view.function();
    let mut provider = None;
    for predecessor in function.blocks() {
        resources.work(64)?;
        let SemanticTerminatorKindV1::Call(call) = predecessor.terminator().kind() else {
            continue;
        };
        if !call
            .destination()
            .is_some_and(|d| d.edge().target().index() as usize == source)
        {
            continue;
        }
        if let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::Bf16MatrixViewRowMajor {
                    result,
                    view,
                    error,
                    ..
                },
            ..
        }) = callables.get(call.callee().index() as usize)
        {
            assert!(provider.replace((call, *result, *view, *error)).is_none());
        }
    }
    let (call, result_type, view_type, error_type) =
        provider.expect("original checked-view constructor");
    let SemanticTypeShapeV1::Enum { variants, .. } = types[result_type.index() as usize].shape()
    else {
        panic!("original Result enum")
    };
    resources.work(checked_mul(variants.len(), 32)?)?;
    let mut ok = None;
    let mut err = None;
    for variant in variants {
        if variant.fields().fields() == [view_type] {
            assert!(ok.replace(variant.discriminant()).is_none())
        }
        if variant.fields().fields() == [error_type] {
            assert!(err.replace(variant.discriminant()).is_none())
        }
    }
    let SemanticTerminatorKindV1::SwitchInt { targets, .. } =
        function.blocks()[source].terminator().kind()
    else {
        panic!("actual source switch")
    };
    let edge = |discriminant| {
        targets
            .values()
            .iter()
            .find(|t| t.value() == discriminant)
            .map_or(targets.otherwise().target(), |t| t.edge().target())
            .index() as usize
    };
    resources.work(checked_mul(targets.values().len(), 16)?)?;
    let success = edge(ok.expect("Ok discriminant"));
    let failure = edge(err.expect("Err discriminant"));
    assert_ne!(success, failure);
    assert_eq!((fact.success(), fact.failure()), (success, failure));
    let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = &call.arguments()[0]
    else {
        panic!("actual original slice input")
    };
    assert!(place.projections().is_empty());
    let local = place.local().index();
    let SemanticLocalRoleV1::Argument(argument) = function.locals()[local as usize].role() else {
        panic!("actual original input argument")
    };
    assert_eq!(
        (fact.source_local(), fact.source_argument()),
        (local, argument)
    );
    assert_eq!(fact.required(), 256);
    let ProductionRankedTerminatorV1::IndexLessThan {
        lhs,
        rhs,
        true_block,
        false_block,
    } = view.blocks()[ranked].terminator()
    else {
        panic!("actual checked length comparison")
    };
    let ProductionRankedValueV1::Argument(slot) = lhs else {
        panic!("dedicated original input length")
    };
    assert_ne!(*slot, 0);
    // The exact genuine profile has two distinct entry slice arguments. Source
    // order, not parameter numeric ID or the helper Return permutation, orders
    // their new slots. No slot aliases the original extent argument zero.
    let prior = view.flow.checked_views()[..source].iter().flatten().count();
    assert_eq!(*slot, prior as u32 + 1);
    for previous in view.flow.checked_views()[..source].iter().flatten() {
        assert_ne!(previous.source_argument(), argument);
        assert_ne!(previous.parameter(), fact.parameter());
    }
    assert_eq!(
        (*true_block, *false_block),
        (
            bases[failure].unwrap() as u32,
            bases[success].unwrap() as u32
        )
    );
    let ProductionRankedValueV1::Local(bound) = rhs else {
        panic!("actual prefix bound")
    };
    let mut count = 0;
    for operation in view.entry_operations() {
        resources.work(8)?;
        if let ProductionRankedOperationV1::IndexConstant { result, value } = operation {
            if result == bound {
                assert_eq!(*value, 256);
                count += 1
            }
        }
    }
    assert_eq!(count, 1);
    Ok(true)
}

fn assert_source(
    view: &ActualRootBlockStreamV1<'_, '_>,
    i: usize,
    block: usize,
    op: usize,
    expected: &ProjectedEffectSourceV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    resources.work(128)?;
    let actual = &view.access_sources()[i];
    assert_eq!((actual.block, actual.operation), (block, op));
    assert_eq!(actual.access, expected.access);
    assert_eq!(actual.memory_space, expected.memory_space);
    assert_eq!(actual.source, expected.source);
    assert_eq!(actual.output_extent, expected.output_extent);
    assert_eq!(actual.semantic_site, expected.semantic_site);
    Ok(())
}
pub(crate) fn observe_actual_root_block_stream_for_test_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    actual_inputs: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
    budget: &mut Budget<'_>,
) -> Q<()> {
    assert!(actual_inputs.belongs_to(owner));
    let observed = owner.with_bf16_nominal_entry_resources_v1(inventory, budget, |budget| {
        let slot = budget as *const Budget<'_> as usize;
        let ledger = budget.work_ledger_identity_v1();
        let before = budget.storage();
        let work = budget.work();
        let peak = budget.peak_storage();
        let mut owned = prepay_headers(budget)?;
        let mut pending = PendingActualRootPrefixIndicesV1::new();
        let mut rows = [ExpectedRow::empty("unused"); MAX_ROWS];
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            with_nominal_prepared_control_flow_v1(
                owner,
                inventory,
                source.root(),
                source.call_block(),
                source.source_call(),
                budget,
                |flow, budget| {
                    owner.with_checked_bf16_nominal_call_v1(
                        inventory,
                        source.root(),
                        source.root(),
                        source.call_block(),
                        source.source_call(),
                        budget,
                        |checked, budget| {
                            assert_eq!(
                                flow.effects().original().candidate().permutation(),
                                source.return_permutation()
                            );
                            with_nominal_rich_source_preparation_v1(
                                owner,
                                inventory,
                                source.root(),
                                source.root(),
                                source.call_block(),
                                source.source_call(),
                                budget,
                                |rich, budget| {
                                    with_nominal_canonical_facts_observation_v1(
                                        owner,
                                        inventory,
                                        source.root(),
                                        source.root(),
                                        source.call_block(),
                                        source.source_call(),
                                        budget,
                                        |facts| {
                                            with_nominal_recipe_resources_v1(
                                                facts,
                                                rich,
                                                &mut owned,
                                                |context| {
                                                    context.with_actual_root_block_stream_v1(
                                                        checked,
                                                        rich,
                                                        actual_inputs,
                                                        flow,
                                                        &mut pending,
                                                        |view, context| {
                                                            inspect(&view, rich, context, &mut rows)
                                                        },
                                                    )
                                                },
                                            )
                                            .map_err(projection)
                                        },
                                    )
                                },
                            )
                        },
                    )
                },
            )
        }));
        let result = match outcome {
            Ok(result) => result,
            Err(payload) => {
                drop(payload);
                Err(QueryError::CallbackPanicked)
            }
        };
        drop(pending);
        if slot != budget as *const Budget<'_> as usize
            || ledger != budget.work_ledger_identity_v1()
            || budget.storage()
                < before
                    .checked_add(owned)
                    .ok_or(QueryError::Resource(Resource::Arithmetic))?
            || budget.work() < work
            || budget.peak_storage() < peak
        {
            let _ = result;
            return Err(QueryError::Resource(Resource::Accounting));
        }
        budget.release_storage(owned)?;
        if result.is_ok() && (budget.failed_work().is_some() || budget.failed_storage().is_some()) {
            return Err(QueryError::Resource(Resource::Accounting));
        }
        assert_eq!(budget.storage(), before);
        result
    })?;
    eprintln!(
        "fe2o3-nominal-block-stream-complete-v1 root={} source_call_block={} permutation={:?} source_blocks={} ranked_blocks={} sites={} private_reads={} private_writes={} guarded={} layouts={} global_reads={} same_ledger=true storage_restored=true mandatory_verifier=false normal_admission=false",
        source.root().index(),
        source.call_block().index(),
        source.return_permutation(),
        observed.source_blocks,
        observed.ranked_blocks,
        observed.sites,
        observed.private_reads,
        observed.private_writes,
        observed.guarded,
        observed.tensors,
        observed.reads
    );
    ranked_consumer_genuine::observe(owner, source, inventory, actual_inputs, budget)?;
    Ok(())
}
#[path = "bf16_nominal_ranked_consumer_genuine_v1_tests.rs"]
mod ranked_consumer_genuine;
#[test]
fn genuine_block_stream_oracle_rejects_changed_target_not_only_counts() {
    let row = ExpectedRow::branch(3, 4);
    assert!(row_matches(&ProjectedCfgTerminatorV1::Branch(3), &row));
    assert!(!row_matches(&ProjectedCfgTerminatorV1::Branch(2), &row));
    assert!(!row_matches(&ProjectedCfgTerminatorV1::Return, &row));
}
#[test]
fn genuine_block_stream_fixed_headers_cover_rows_owner_and_oracle_stack() {
    let bytes = header_bytes().unwrap();
    assert_eq!(
        bytes,
        4 * size_of::<PendingActualRootPrefixIndicesV1>()
            + 4 * size_of::<[ExpectedRow; MAX_ROWS]>()
            + 4 * size_of::<Observation>()
            + 8 * size_of::<[usize; MAX_ROWS]>()
            + 4096
    );
    assert!(bytes < crate::production_canonical_phase_policy_v1::STORAGE_LIMIT);
    assert!(scan_work(usize::MAX).is_err());
    for values in [
        [usize::MAX, 0, 0, 0],
        [0, usize::MAX, 0, 0],
        [0, 0, usize::MAX, 0],
        [0, 0, 0, usize::MAX],
        [usize::MAX / 4, 0, 0, 0],
    ] {
        assert!(header_bytes_for(values[0], values[1], values[2], values[3]).is_err());
    }
}

#[test]
fn genuine_block_stream_header_prepayment_exact_and_one_short_preserve_owner_floor() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    let bytes = header_bytes().unwrap();
    let work = checked_mul(4, bytes).unwrap();
    for (work_limit, storage_limit, succeeds) in [
        (11 + work, 7 + bytes, true),
        (11 + work - 1, 7 + bytes, false),
        (11 + work, 7 + bytes - 1, false),
    ] {
        let mut work_budget = Work::new(work_limit);
        let mut budget = Budget::new(&mut work_budget, storage_limit);
        budget.charge_work(11).unwrap();
        budget.reserve_storage(7).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = prepay_headers(&mut budget);
        if succeeds {
            let owned = result.unwrap();
            assert_eq!(owned, bytes);
            assert_eq!(budget.work(), 11 + work);
            assert_eq!(budget.storage(), 7 + bytes);
            assert_eq!(budget.peak_storage(), 7 + bytes);
            assert_eq!(budget.failed_work(), None);
            assert_eq!(budget.failed_storage(), None);
            // The same ordering used by the observer: no owner is constructed
            // before accepted prepayment, and accepted credits outlive it.
            let pending = PendingActualRootPrefixIndicesV1::new();
            drop(pending);
            budget.release_storage(owned).unwrap();
        } else if work_limit < 11 + work {
            assert!(
                matches!(result, Err(QueryError::Resource(Resource::Work(error)))
                if error.actual() == 11 + work && error.limit() == work_limit)
            );
            assert_eq!(budget.work(), 11);
            assert_eq!(budget.failed_work(), Some(11 + work));
            assert_eq!(budget.failed_storage(), None);
            assert_eq!(budget.peak_storage(), 7);
            assert!(budget.check_prior_denials_v1().is_err());
        } else {
            assert!(
                matches!(result, Err(QueryError::Resource(Resource::Storage(error)))
                if error.actual() == 7 + bytes && error.limit() == storage_limit)
            );
            assert_eq!(budget.work(), 11 + work);
            assert_eq!(budget.failed_work(), None);
            assert_eq!(budget.failed_storage(), Some(7 + bytes));
            assert_eq!(budget.peak_storage(), 7);
            assert!(budget.check_prior_denials_v1().is_err());
        }
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage(), 7);
    }
}

#[test]
fn genuine_block_stream_header_prepayment_preserves_prior_denial_without_new_charge() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    let mut work = Work::new(1);
    let mut budget = Budget::new(&mut work, 7 + header_bytes().unwrap());
    budget.reserve_storage(7).unwrap();
    assert!(budget.charge_work(2).is_err());
    assert!(matches!(prepay_headers(&mut budget),
        Err(QueryError::Resource(Resource::Work(error)))
        if error.actual() == 2 && error.limit() == 1));
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.failed_work(), Some(2));
    assert_eq!(budget.storage(), 7);
    assert_eq!(budget.peak_storage(), 7);
    assert_eq!(budget.failed_storage(), None);
}
