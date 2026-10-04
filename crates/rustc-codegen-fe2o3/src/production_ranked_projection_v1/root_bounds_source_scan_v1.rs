//! Coordinate-owned bounds source DATA; not completed checks or source admission.
//! All accepted credits and all partially allocated vectors remain caller-owned.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as WorkIdentity,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticControlFlowEdgeV1;
use std::mem::{size_of, size_of_val};

type Error = ProductionRankedProjectionErrorV1;
type Ledger = (usize, WorkIdentity);
type Values = Vec<Option<ProductionRankedValueV1>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DefinitionCoordinateV1 {
    block: usize,
    statement: usize,
}
#[derive(Clone, Copy, Default)]
struct BoundsDefinitionSiteV1 {
    count: u32,
    value: Option<DefinitionCoordinateV1>,
    length_source: Option<ProjectedBoundsExtentSourceV1>,
}
#[derive(Default)]
pub(super) struct BoundsSourceStorageV1 {
    definitions: Vec<BoundsDefinitionSiteV1>,
    predecessors: Vec<Vec<usize>>,
    local_values: Values,
    slice_extents: Values,
    source: Option<usize>,
    ledger: Option<Ledger>,
    scan_started: bool,
    scan_completed: bool,
    values_started: bool,
    values_completed: bool,
}
pub(super) struct BoundsSourceViewV1<'s, 'b> {
    function: &'s SemanticFunctionDeclV1,
    storage: &'b BoundsSourceStorageV1,
}

fn refuse(message: &'static str) -> Error {
    Error::Unsupported(message)
}
fn arithmetic() -> Error {
    bf16_nominal_preparation_resources_v1::resource(Resource::Arithmetic)
}
fn frame(parts: &[usize]) -> Result<usize, Error> {
    parts.iter().try_fold(0usize, |sum, value| {
        sum.checked_add(*value).ok_or_else(arithmetic)
    })
}
fn pay_frame(resources: &mut PreparationResourcesV1<'_, '_>, bytes: usize) -> Result<(), Error> {
    resources.work(bytes)?;
    resources.reserve_storage(bytes)
}
fn source_id(function: &SemanticFunctionDeclV1) -> usize {
    function as *const SemanticFunctionDeclV1 as usize
}
fn occupied<T>(values: &Vec<T>) -> bool {
    !values.is_empty() || values.capacity() != 0
}
impl BoundsSourceStorageV1 {
    pub(super) fn new() -> Self {
        Self::default()
    }
    fn occupied(&self) -> bool {
        self.source.is_some()
            || self.ledger.is_some()
            || self.scan_started
            || self.scan_completed
            || self.values_started
            || self.values_completed
            || occupied(&self.definitions)
            || occupied(&self.predecessors)
            || occupied(&self.local_values)
            || occupied(&self.slice_extents)
    }
    fn current(
        &self,
        function: &SemanticFunctionDeclV1,
        resources: &PreparationResourcesV1<'_, '_>,
    ) -> Result<(), Error> {
        if resources.has_denial()
            || self.source != Some(source_id(function))
            || self.ledger.is_none()
            || self.ledger != resources.original_ledger_v1()
        {
            return Err(refuse(
                "bounds source scratch outside its original lexical source and ledger",
            ));
        }
        Ok(())
    }
    pub(super) fn scan(
        &mut self,
        function: &SemanticFunctionDeclV1,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<(), Error> {
        if self.occupied() || !resources.is_metered() || resources.has_denial() {
            return Err(refuse(
                "bounds source scratch requires a fresh original-ledger owner",
            ));
        }
        // The owner already exists in the caller. Mark it before fallible admission;
        // neither failed admission nor an unwind may make it reusable.
        self.scan_started = true;
        self.source = Some(source_id(function));
        self.ledger = resources.original_ledger_v1();
        pay_frame(resources, scan_frame_v1()?)?;
        resources.work(function.locals().len())?;
        resources.reserve(&mut self.definitions, function.locals().len())?;
        self.definitions
            .resize(function.locals().len(), BoundsDefinitionSiteV1::default());
        resources.work(function.blocks().len())?;
        resources.reserve(&mut self.predecessors, function.blocks().len())?;
        self.predecessors
            .resize_with(function.blocks().len(), Vec::new);
        scan_shared(
            function,
            DefinitionSink::Paid(&mut self.definitions),
            &mut self.predecessors,
            resources,
        )?;
        self.current(function, resources)?;
        self.scan_completed = true;
        Ok(())
    }
    pub(super) fn initialize_values(
        &mut self,
        function: &SemanticFunctionDeclV1,
        known: &[Option<ProjectedDisjointIndexV1>],
        ordinary: &[Option<ProjectedOrdinaryIndexV1>],
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<(), Error> {
        self.current(function, resources)?;
        if !self.scan_completed
            || self.values_started
            || self.values_completed
            || occupied(&self.local_values)
            || occupied(&self.slice_extents)
        {
            return Err(refuse(
                "bounds initial values require one completed unused source scan",
            ));
        }
        self.values_started = true;
        pay_frame(resources, value_frame_v1()?)?;
        validate_index_shapes(function.locals().len(), known, ordinary)?;
        let n = function.locals().len();
        resources.work(n)?;
        resources.reserve(&mut self.local_values, n)?;
        if known.is_empty() {
            self.local_values.resize(n, None);
        } else {
            // Exact capacity was admitted above; no allocation inside this iterator.
            self.local_values.extend(known_values(known));
        }
        resources.work(n)?;
        resources.reserve(&mut self.slice_extents, n)?;
        self.slice_extents.resize(n, None);
        self.current(function, resources)?;
        self.values_completed = true;
        Ok(())
    }
    pub(super) fn view<'s, 'b>(
        &'b self,
        function: &'s SemanticFunctionDeclV1,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<BoundsSourceViewV1<'s, 'b>, Error> {
        resources.work(1)?;
        self.current(function, resources)?;
        if !self.scan_completed
            || !self.values_completed
            || self.definitions.len() != function.locals().len()
            || self.predecessors.len() != function.blocks().len()
            || self.local_values.len() != function.locals().len()
            || self.slice_extents.len() != function.locals().len()
        {
            return Err(refuse(
                "bounds source view requires completed exact scratch rosters",
            ));
        }
        Ok(BoundsSourceViewV1 {
            function,
            storage: self,
        })
    }
}
impl<'s> BoundsSourceViewV1<'s, '_> {
    /// Lexical source equality only, not a new source capability or global ID.
    pub(super) fn require_same_source_v1(
        &self,
        function: &SemanticFunctionDeclV1,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<(), Error> {
        resources.work(1)?;
        if !std::ptr::eq(self.function, function) {
            return Err(refuse(
                "bounds source view differs from the requested lexical source",
            ));
        }
        self.storage.current(function, resources)
    }
    pub(super) fn definition(
        &self,
        local: usize,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<BoundsLocalDefinitionV1<'s>, Error> {
        resources.work(1)?;
        self.storage.current(self.function, resources)?;
        let site = self
            .storage
            .definitions
            .get(local)
            .ok_or_else(|| refuse("bounds source query outside the semantic local table"))?;
        let value = if let Some(coordinate) = site.value {
            resources.work(1)?;
            let statement = self
                .function
                .blocks()
                .get(coordinate.block)
                .and_then(|block| block.statements().get(coordinate.statement))
                .ok_or_else(|| {
                    refuse("bounds definition source coordinate outside its original function")
                })?;
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                return Err(refuse(
                    "bounds definition source coordinate is not its local assignment",
                ));
            };
            if !assignment.destination().projections().is_empty()
                || assignment.destination().local().index() as usize != local
            {
                return Err(refuse(
                    "bounds definition source coordinate is not its local assignment",
                ));
            }
            Some(assignment.value())
        } else {
            None
        };
        Ok(BoundsLocalDefinitionV1 {
            count: site.count,
            value,
            length_source: site.length_source,
        })
    }
    pub(super) fn predecessors(
        &self,
        block: usize,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<&[usize], Error> {
        resources.work(1)?;
        self.storage.current(self.function, resources)?;
        self.storage
            .predecessors
            .get(block)
            .map(Vec::as_slice)
            .ok_or_else(|| refuse("bounds predecessor query outside the semantic block table"))
    }
    pub(super) fn local_values(
        &self,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<&[Option<ProductionRankedValueV1>], Error> {
        resources.work(1)?;
        self.storage.current(self.function, resources)?;
        Ok(&self.storage.local_values)
    }
    pub(super) fn slice_extents(
        &self,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<&[Option<ProductionRankedValueV1>], Error> {
        resources.work(1)?;
        self.storage.current(self.function, resources)?;
        Ok(&self.storage.slice_extents)
    }
}

enum DefinitionSink<'a, 's> {
    Legacy(&'a mut [BoundsLocalDefinitionV1<'s>]),
    Paid(&'a mut [BoundsDefinitionSiteV1]),
}
enum DefinitionSlot<'a, 's> {
    Legacy(&'a mut BoundsLocalDefinitionV1<'s>),
    Paid(&'a mut BoundsDefinitionSiteV1),
}
impl<'s> DefinitionSink<'_, 's> {
    fn slot(
        &mut self,
        local: usize,
        message: &'static str,
    ) -> Result<DefinitionSlot<'_, 's>, Error> {
        match self {
            Self::Legacy(rows) => rows.get_mut(local).map(DefinitionSlot::Legacy),
            Self::Paid(rows) => rows.get_mut(local).map(DefinitionSlot::Paid),
        }
        .ok_or_else(|| refuse(message))
    }
}
impl<'s> DefinitionSlot<'_, 's> {
    fn assignment(&mut self, value: &'s SemanticRvalueV1, coordinate: DefinitionCoordinateV1) {
        match self {
            Self::Legacy(row) => {
                row.count = row.count.saturating_add(1);
                row.value = Some(value);
            }
            Self::Paid(row) => {
                row.count = row.count.saturating_add(1);
                row.value = Some(coordinate);
            }
        }
    }
    fn set_length(&mut self, source: Option<ProjectedBoundsExtentSourceV1>) {
        match self {
            Self::Legacy(row) => row.length_source = source,
            Self::Paid(row) => row.length_source = source,
        }
    }
    fn call(&mut self) {
        match self {
            Self::Legacy(row) => {
                row.count = row.count.saturating_add(1);
                row.length_source = None;
                row.value = None;
            }
            Self::Paid(row) => {
                row.count = row.count.saturating_add(1);
                row.length_source = None;
                row.value = None;
            }
        }
    }
}
fn length_source(
    value: &SemanticRvalueV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<Option<ProjectedBoundsExtentSourceV1>, Error> {
    resources.work(1)?;
    Ok(match value.kind() {
        SemanticRvalueKindV1::Length(place) => {
            let mut field = false;
            for projection in place.projections() {
                resources.work(1)?;
                if matches!(projection.kind(), SemanticProjectionKindV1::Field(_)) {
                    field = true;
                    break;
                }
            }
            Some(if field {
                ProjectedBoundsExtentSourceV1::CanonicalSlice
            } else {
                ProjectedBoundsExtentSourceV1::Slice(place.local())
            })
        }
        SemanticRvalueKindV1::Unary {
            operation: SemanticUnaryOpV1::PointerMetadata,
            operand,
        } => match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                Some(if place.projections().is_empty() {
                    ProjectedBoundsExtentSourceV1::Slice(place.local())
                } else {
                    ProjectedBoundsExtentSourceV1::CanonicalSlice
                })
            }
            SemanticOperandV1::Constant(_) => None,
        },
        _ => None,
    })
}
fn scan_shared<'s>(
    function: &'s SemanticFunctionDeclV1,
    mut definitions: DefinitionSink<'_, 's>,
    predecessors: &mut [Vec<usize>],
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<(), Error> {
    for (block_index, block) in function.blocks().iter().enumerate() {
        resources.work(1)?;
        for (statement_index, statement) in block.statements().iter().enumerate() {
            resources.work(1)?;
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            if !assignment.destination().projections().is_empty() {
                continue;
            }
            resources.work(1)?;
            let mut definition = definitions.slot(
                assignment.destination().local().index() as usize,
                "a Rust bounds-check definition outside the semantic local table",
            )?;
            definition.assignment(
                assignment.value(),
                DefinitionCoordinateV1 {
                    block: block_index,
                    statement: statement_index,
                },
            );
            definition.set_length(length_source(assignment.value(), resources)?);
        }
        resources.work(1)?;
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && let Some(destination) = call.destination()
            && destination.place().projections().is_empty()
        {
            resources.work(1)?;
            definitions
                .slot(
                    destination.place().local().index() as usize,
                    "a Rust bounds-check call result outside the semantic local table",
                )?
                .call();
        }
        for_edges(block.terminator().kind(), resources, |edge, resources| {
            resources.work(1)?;
            let target = edge.target().index() as usize;
            let target_predecessors = predecessors.get_mut(target).ok_or(Error::Unsupported(
                "a Rust bounds-check CFG edge outside the semantic block table",
            ))?;
            if resources.is_metered() {
                resources.push(target_predecessors, block_index)?;
            } else {
                target_predecessors.push(block_index);
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn for_edges<'b, 'w, F>(
    terminator: &SemanticTerminatorKindV1,
    resources: &mut PreparationResourcesV1<'b, 'w>,
    mut visitor: F,
) -> Result<(), Error>
where
    F: FnMut(SemanticControlFlowEdgeV1, &mut PreparationResourcesV1<'b, 'w>) -> Result<(), Error>,
{
    // Admit the exact supplied capture type plus the two mutable references
    // transferred to try_for_each_edge. This is retained logical storage, not
    // a whole-stack/RSS bound. Legacy pay_frame is a no-op.
    pay_frame(
        resources,
        frame(&[
            size_of_val(&visitor),
            size_of::<(&mut F, &mut PreparationResourcesV1<'b, 'w>)>(),
            size_of::<Result<(), Error>>(),
        ])?,
    )?;
    terminator.try_for_each_edge::<Error>(|edge| visitor(edge, resources))
}

/// No metering or reserve policy is introduced into the ordinary allocation path.
pub(super) fn scan_legacy_v1<'s>(
    function: &'s SemanticFunctionDeclV1,
    definitions: &mut [BoundsLocalDefinitionV1<'s>],
    predecessors: &mut [Vec<usize>],
) -> Result<(), Error> {
    scan_shared(
        function,
        DefinitionSink::Legacy(definitions),
        predecessors,
        &mut PreparationResourcesV1::unmetered(),
    )
}
fn validate_index_shapes(
    locals: usize,
    known: &[Option<ProjectedDisjointIndexV1>],
    ordinary: &[Option<ProjectedOrdinaryIndexV1>],
) -> Result<(), Error> {
    if !ordinary.is_empty() && ordinary.len() != locals {
        return Err(refuse(
            "ordinary intrinsic index facts do not match the semantic local table",
        ));
    }
    if !known.is_empty() && known.len() != locals {
        return Err(refuse(
            "intrinsic index facts do not match the semantic local table",
        ));
    }
    Ok(())
}
fn known_values(
    known: &[Option<ProjectedDisjointIndexV1>],
) -> impl ExactSizeIterator<Item = Option<ProductionRankedValueV1>> + '_ {
    known.iter().map(|index| index.map(|index| index.value))
}
pub(super) fn initial_values_legacy_v1(
    function: &SemanticFunctionDeclV1,
    known: &[Option<ProjectedDisjointIndexV1>],
    ordinary: &[Option<ProjectedOrdinaryIndexV1>],
) -> Result<(Values, Values), Error> {
    validate_index_shapes(function.locals().len(), known, ordinary)?;
    let local_values = if known.is_empty() {
        vec![None; function.locals().len()]
    } else {
        known_values(known).collect()
    };
    // Separate MIR Len temporaries for one stable slice describe one ranked extent.
    let slice_extents = vec![None; function.locals().len()];
    Ok((local_values, slice_extents))
}
// Retained logical source-frame rosters, not an optimized stack/RSS bound.
// Each call row includes two result/value transfers through the same checked
// helper as the borrowed-query envelope. Dynamic Vec payloads remain separately
// admitted by PreparationResourcesV1; no fixed row substitutes for those debits.
fn scan_frame_rows_v1() -> Result<[usize; 18], Error> {
    use fe2o3_mir_model::semantic_mir_v1 as mir;
    Ok([
        // 0: the outer physical owner and its eventual borrowed view.
        frame(&[
            size_of::<BoundsSourceStorageV1>(),
            size_of::<BoundsSourceViewV1<'_, '_>>(),
        ])?,
        // 1: scan caller retained through all nested work and its identity setup.
        query_call_frame_v1::<()>(size_of::<(
            &mut BoundsSourceStorageV1,
            &SemanticFunctionDeclV1,
            &mut PreparationResourcesV1<'_, '_>,
            Option<usize>,
            Option<Ledger>,
            bool,
        )>())?,
        // 2: occupied owner and the four generic Vec header checks.
        query_call_frame_v1::<bool>(size_of::<(
            &BoundsSourceStorageV1,
            &Vec<BoundsDefinitionSiteV1>,
            &Vec<Vec<usize>>,
            &Values,
            &Values,
            usize,
            bool,
        )>())?,
        // 3: source-sized initialization callers (payloads are paid separately).
        query_call_frame_v1::<()>(size_of::<(
            &mut Vec<BoundsDefinitionSiteV1>,
            &mut Vec<Vec<usize>>,
            usize,
            usize,
            BoundsDefinitionSiteV1,
            Vec<usize>,
        )>())?,
        // 4: scan_shared owns this sink and retains all four arguments.
        query_call_frame_v1::<()>(size_of::<(
            &SemanticFunctionDeclV1,
            DefinitionSink<'_, '_>,
            &mut [Vec<usize>],
            &mut PreparationResourcesV1<'_, '_>,
        )>())?,
        // 5: source block enumerate state, yielded value and branch borrow.
        query_call_frame_v1::<()>(size_of::<(
            std::iter::Enumerate<std::slice::Iter<'_, mir::SemanticBasicBlockV1>>,
            Option<(usize, &mir::SemanticBasicBlockV1)>,
            usize,
            &mir::SemanticBasicBlockV1,
            &SemanticTerminatorKindV1,
        )>())?,
        // 6: nested statement iterator, yielded pair and live assignment slot.
        query_call_frame_v1::<()>(size_of::<(
            std::iter::Enumerate<std::slice::Iter<'_, mir::SemanticStatementV1>>,
            Option<(usize, &mir::SemanticStatementV1)>,
            usize,
            &mir::SemanticStatementV1,
            &mir::SemanticAssignmentV1,
            DefinitionSlot<'_, '_>,
            DefinitionCoordinateV1,
        )>())?,
        // 7: DefinitionSink::slot match arms and the error-closure message.
        query_call_frame_v1::<DefinitionSlot<'_, '_>>(size_of::<(
            &mut DefinitionSink<'_, '_>,
            usize,
            &'static str,
            Option<&mut BoundsLocalDefinitionV1<'_>>,
            Option<&mut BoundsDefinitionSiteV1>,
        )>())?,
        // 8: assignment source coordinates and saturating count update.
        query_call_frame_v1::<()>(size_of::<(
            &mut DefinitionSlot<'_, '_>,
            &SemanticRvalueV1,
            DefinitionCoordinateV1,
            u32,
            Option<DefinitionCoordinateV1>,
            Option<&SemanticRvalueV1>,
        )>())?,
        // 9: set_length caller, value and selected arm reference.
        query_call_frame_v1::<()>(size_of::<(
            &mut DefinitionSlot<'_, '_>,
            Option<ProjectedBoundsExtentSourceV1>,
            &mut BoundsLocalDefinitionV1<'_>,
            &mut BoundsDefinitionSiteV1,
        )>())?,
        // 10: call-result clearing, with the same explicit matched row borrows.
        query_call_frame_v1::<()>(size_of::<(
            &mut DefinitionSlot<'_, '_>,
            u32,
            &mut BoundsLocalDefinitionV1<'_>,
            &mut BoundsDefinitionSiteV1,
        )>())?,
        // 11: length_source's reached projection iterator and unary branch.
        query_call_frame_v1::<Option<ProjectedBoundsExtentSourceV1>>(size_of::<(
            &SemanticRvalueV1,
            &mut PreparationResourcesV1<'_, '_>,
            &SemanticPlaceV1,
            &SemanticOperandV1,
            bool,
            std::slice::Iter<'_, mir::SemanticProjectionV1>,
            Option<&mir::SemanticProjectionV1>,
            &mir::SemanticProjectionV1,
            SemanticLocalIdV1,
        )>())?,
        // 12: the optional whole-local Call destination branch.
        query_call_frame_v1::<()>(size_of::<(
            &mir::SemanticDirectCallV1,
            Option<&mir::SemanticCallDestinationV1>,
            &mir::SemanticCallDestinationV1,
            &SemanticPlaceV1,
            DefinitionSlot<'_, '_>,
        )>())?,
        // 13: fixed edge callback inputs/lookup state. The exact generic F
        // capture and F borrow/transfer are still paid by unchanged for_edges.
        query_call_frame_v1::<()>(size_of::<(
            &SemanticTerminatorKindV1,
            &mut PreparationResourcesV1<'_, '_>,
            SemanticControlFlowEdgeV1,
            usize,
            usize,
            &mut [Vec<usize>],
            Option<&mut Vec<usize>>,
            &mut Vec<usize>,
            bool,
        )>())?,
        // 14: nonallocating model edge traversal and visit_unwind state.
        // The bridge closure's captured (&mut F, &mut resources) is paid by
        // for_edges. Its additional mutable-reference argument is thin:
        // the Sized capture representation below is size-only, never a value,
        // closure identity, pointer dereference, or authority token.
        query_call_frame_v1::<()>(size_of::<(
            &SemanticTerminatorKindV1,
            std::slice::Iter<'_, mir::SemanticSwitchTargetV1>,
            Option<&mir::SemanticSwitchTargetV1>,
            &mir::SemanticSwitchTargetV1,
            &mir::SemanticSwitchTargetsV1,
            SemanticControlFlowEdgeV1,
            mir::SemanticUnwindActionV1,
            &mir::SemanticUnwindActionV1,
            Option<&mir::SemanticCallDestinationV1>,
            &mir::SemanticCallDestinationV1,
            &mir::SemanticDirectCallV1,
            &mir::SemanticDirectTailCallV1,
            &SemanticControlFlowEdgeV1,
            &SemanticControlFlowEdgeV1,
            &mut (&mut (), &mut PreparationResourcesV1<'_, '_>),
        )>())?,
        // 15: pay_frame keeps work-before-storage ordering unchanged.
        query_call_frame_v1::<()>(size_of::<(
            &mut PreparationResourcesV1<'_, '_>,
            usize,
            Result<(), Error>,
        )>())?,
        // 16: scan_frame_rows + scan_frame caller arrays, fold argument and
        // result transfers. Sum/failure helper state is retained in row 17.
        query_call_frame_v1::<[usize; 18]>(size_of::<(
            [usize; 18],
            &[usize],
            usize,
            Result<usize, Error>,
        )>())?,
        // 17: one standalone borrowed-query envelope, retained by this scan.
        // initialize_values does not pay it again; B1 conservatively does.
        query_frame_v1()?,
    ])
}
pub(super) fn scan_frame_v1() -> Result<usize, Error> {
    frame(&scan_frame_rows_v1()?)
}
fn value_frame_rows_v1() -> Result<[usize; 9], Error> {
    Ok([
        // 0: initialize_values caller and all input borrows through completion.
        query_call_frame_v1::<()>(size_of::<(
            &mut BoundsSourceStorageV1,
            &SemanticFunctionDeclV1,
            &[Option<ProjectedDisjointIndexV1>],
            &[Option<ProjectedOrdinaryIndexV1>],
            &mut PreparationResourcesV1<'_, '_>,
        )>())?,
        // 1: live n and the separate vector initialization/reserve callers.
        query_call_frame_v1::<()>(size_of::<(
            usize,
            &mut Values,
            &mut Values,
            Option<ProductionRankedValueV1>,
            bool,
        )>())?,
        // 2: exact ordinary-first shape validation; no predicate/order change.
        query_call_frame_v1::<()>(size_of::<(
            usize,
            &[Option<ProjectedDisjointIndexV1>],
            &[Option<ProjectedOrdinaryIndexV1>],
            bool,
            bool,
        )>())?,
        // 3: actual opaque map iterator type, including transfers. Merely
        // constructing known_values(&[]) does not iterate or allocate.
        frame(&[
            size_of_val(&known_values(&[])),
            size_of_val(&known_values(&[])),
        ])?,
        // 4: known_values caller/underlying iterator and yielded element.
        query_call_frame_v1::<Option<ProductionRankedValueV1>>(size_of::<(
            &[Option<ProjectedDisjointIndexV1>],
            std::slice::Iter<'_, Option<ProjectedDisjointIndexV1>>,
            Option<&Option<ProjectedDisjointIndexV1>>,
            &Option<ProjectedDisjointIndexV1>,
        )>())?,
        // 5: the nested Option::map payload and exact source value transfer.
        query_call_frame_v1::<Option<ProductionRankedValueV1>>(size_of::<(
            Option<ProjectedDisjointIndexV1>,
            ProjectedDisjointIndexV1,
            ProductionRankedValueV1,
            Option<ProductionRankedValueV1>,
        )>())?,
        // 6: resize/extend source callers, not a substitute for Vec payloads.
        query_call_frame_v1::<()>(size_of::<(
            &mut Values,
            usize,
            Option<ProductionRankedValueV1>,
            Option<Option<ProductionRankedValueV1>>,
        )>())?,
        // 7: admission caller remains live through work then storage.
        query_call_frame_v1::<()>(size_of::<(
            &mut PreparationResourcesV1<'_, '_>,
            usize,
            Result<(), Error>,
        )>())?,
        // 8: value_frame_rows + value_frame caller arrays/results.
        // query/current/arithmetic closure is already retained by scan H.
        query_call_frame_v1::<[usize; 9]>(size_of::<(
            [usize; 9],
            &[usize],
            usize,
            Result<usize, Error>,
        )>())?,
    ])
}
pub(super) fn value_frame_v1() -> Result<usize, Error> {
    frame(&value_frame_rows_v1()?)
}

// Private borrowed-query frame closure. This function adds NO debit by itself.
// B0 scan H admits it exactly once before any payload; completion and the
// one-shot owner preserve that credit through every later borrowed query.
// Value initialization adds no second Q. B1a separately admits Q in its own H;
// the combined B0+B1 route is deliberately conservative, not a single debit.
fn query_call_frame_v1<T>(locals: usize) -> Result<usize, Error> {
    frame(&[
        locals,
        size_of::<T>(),
        size_of::<T>(),
        size_of::<Result<T, Error>>(),
        size_of::<Result<T, Error>>(),
    ])
}
pub(super) fn query_frame_v1() -> Result<usize, Error> {
    frame(&[
        // definition: coordinate/site and source lookup closure temporaries.
        query_call_frame_v1::<BoundsLocalDefinitionV1<'_>>(size_of::<(
            &BoundsSourceViewV1<'_, '_>,
            usize,
            &mut PreparationResourcesV1<'_, '_>,
            &BoundsDefinitionSiteV1,
            Option<&BoundsDefinitionSiteV1>,
            DefinitionCoordinateV1,
            &DefinitionCoordinateV1,
            Option<&fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1>,
            Option<&fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1>,
            &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
            Option<&SemanticRvalueV1>,
            BoundsLocalDefinitionV1<'_>,
        )>())?,
        // current: original source and ledger return/comparison copies.
        query_call_frame_v1::<()>(size_of::<(
            &BoundsSourceStorageV1,
            &SemanticFunctionDeclV1,
            &PreparationResourcesV1<'_, '_>,
            usize,
            Option<usize>,
            Option<Ledger>,
            Option<Ledger>,
            bool,
        )>())?,
        // require_same_source_v1: all caller arguments remain live at current.
        query_call_frame_v1::<()>(size_of::<(
            &BoundsSourceViewV1<'_, '_>,
            &SemanticFunctionDeclV1,
            &mut PreparationResourcesV1<'_, '_>,
            bool,
        )>())?,
        // source_id.
        query_call_frame_v1::<usize>(size_of::<(&SemanticFunctionDeclV1, usize)>())?,
        // refuse, including error result/transfer rather than static-string bytes.
        query_call_frame_v1::<Error>(size_of::<(&'static str, Error)>())?,
        // query_frame: complete size array and sum/return.
        query_call_frame_v1::<usize>(size_of::<([usize; 12], &[usize], Result<usize, Error>)>())?,
        // query_call_frame's own input/array/result lives across frame().
        query_call_frame_v1::<usize>(
            size_of::<(usize, [usize; 5], &[usize], Result<usize, Error>)>(),
        )?,
        // frame checked fold/arithmetic-refusal closure state.
        query_call_frame_v1::<usize>(size_of::<(
            &[usize],
            std::slice::Iter<'static, usize>,
            usize,
            &usize,
            Option<usize>,
            Result<usize, Error>,
        )>())?,
        // view: full standalone entry and exact-roster predicates.
        query_call_frame_v1::<BoundsSourceViewV1<'_, '_>>(size_of::<(
            &BoundsSourceStorageV1,
            &SemanticFunctionDeclV1,
            &mut PreparationResourcesV1<'_, '_>,
            usize,
            bool,
        )>())?,
        // predecessors: entry plus borrowed row/Option mapping state.
        query_call_frame_v1::<&[usize]>(size_of::<(
            &BoundsSourceViewV1<'_, '_>,
            usize,
            &mut PreparationResourcesV1<'_, '_>,
            Option<&Vec<usize>>,
            &Vec<usize>,
            Option<&[usize]>,
        )>())?,
        // local_values and slice_extents are distinct nested call vertices.
        query_call_frame_v1::<&[Option<ProductionRankedValueV1>]>(size_of::<(
            &BoundsSourceViewV1<'_, '_>,
            &mut PreparationResourcesV1<'_, '_>,
            &Values,
            &[Option<ProductionRankedValueV1>],
        )>())?,
        query_call_frame_v1::<&[Option<ProductionRankedValueV1>]>(size_of::<(
            &BoundsSourceViewV1<'_, '_>,
            &mut PreparationResourcesV1<'_, '_>,
            &Values,
            &[Option<ProductionRankedValueV1>],
        )>())?,
    ])
}

#[cfg(test)]
pub(super) mod test_access {
    use super::*;
    pub(in crate::production_ranked_projection_v1) fn query_frame_overflow_control() {
        assert!(query_call_frame_v1::<BoundsLocalDefinitionV1<'_>>(usize::MAX).is_err());
        let query = query_frame_v1().unwrap();
        let rows = scan_frame_rows_v1().unwrap();
        assert!(query > 0);
        assert_eq!(rows[17], query);
        assert_eq!(
            scan_frame_v1().unwrap(),
            frame(&rows[..17]).unwrap() + query
        );
    }
    pub(in crate::production_ranked_projection_v1) fn frame_rosters_control() {
        let scan = scan_frame_rows_v1().unwrap();
        let values = value_frame_rows_v1().unwrap();
        assert_eq!(scan.len(), 18);
        assert_eq!(values.len(), 9);
        for row in scan.into_iter().chain(values) {
            assert!(row > 0);
            assert!(frame(&[usize::MAX, row]).is_err());
        }
        assert_eq!(scan_frame_v1().unwrap(), frame(&scan).unwrap());
        assert_eq!(value_frame_v1().unwrap(), frame(&values).unwrap());
        // These independent type expressions pin previously omitted live
        // vertices, rather than assuming the owner header covers their stack.
        assert!(
            scan[5]
                >= size_of::<
                    std::iter::Enumerate<
                        std::slice::Iter<
                            '_,
                            fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1,
                        >,
                    >,
                >()
        );
        assert!(
            scan[6]
                >= size_of::<
                    std::iter::Enumerate<
                        std::slice::Iter<'_, fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1>,
                    >,
                >()
        );
        assert!(
            scan[11]
                >= size_of::<
                    std::slice::Iter<'_, fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1>,
                >()
        );
        assert!(
            scan[14]
                >= size_of::<
                    std::slice::Iter<'_, fe2o3_mir_model::semantic_mir_v1::SemanticSwitchTargetV1>,
                >()
        );
        assert!(
            values[0]
                >= size_of::<(
                    &mut BoundsSourceStorageV1,
                    &SemanticFunctionDeclV1,
                    &[Option<ProjectedDisjointIndexV1>],
                    &[Option<ProjectedOrdinaryIndexV1>],
                    &mut PreparationResourcesV1<'_, '_>,
                )>()
        );
        assert!(values[3] >= size_of_val(&known_values(&[])));
    }
    pub(in crate::production_ranked_projection_v1) fn standalone_query_frame_v1() -> usize {
        query_frame_v1().unwrap()
    }
    pub(in crate::production_ranked_projection_v1) fn occupy_capacity(
        storage: &mut BoundsSourceStorageV1,
        which: usize,
    ) {
        match which {
            0 => storage.definitions.reserve_exact(1),
            1 => storage.predecessors.reserve_exact(1),
            2 => storage.local_values.reserve_exact(1),
            3 => storage.slice_extents.reserve_exact(1),
            _ => unreachable!(),
        }
    }
    pub(in crate::production_ranked_projection_v1) fn coordinate(
        storage: &mut BoundsSourceStorageV1,
        local: usize,
        block: usize,
        statement: usize,
    ) {
        storage.definitions[local].value = Some(DefinitionCoordinateV1 { block, statement });
    }
    pub(in crate::production_ranked_projection_v1) fn state(
        storage: &BoundsSourceStorageV1,
    ) -> (bool, bool, bool, bool, usize, usize) {
        (
            storage.scan_started,
            storage.scan_completed,
            storage.values_started,
            storage.values_completed,
            storage.definitions.capacity(),
            storage.predecessors.iter().map(Vec::capacity).sum(),
        )
    }
    pub(in crate::production_ranked_projection_v1) fn saturated_assignment_and_call(
        function: &SemanticFunctionDeclV1,
    ) {
        let SemanticStatementKindV1::Assign(assignment) =
            function.blocks()[0].statements()[0].kind()
        else {
            panic!();
        };
        let mut borrowed = BoundsLocalDefinitionV1 {
            count: u32::MAX,
            value: None,
            length_source: None,
        };
        let mut owned = BoundsDefinitionSiteV1 {
            count: u32::MAX,
            ..Default::default()
        };
        let coordinate = DefinitionCoordinateV1 {
            block: 0,
            statement: 0,
        };
        DefinitionSlot::Legacy(&mut borrowed).assignment(assignment.value(), coordinate);
        DefinitionSlot::Paid(&mut owned).assignment(assignment.value(), coordinate);
        assert_eq!(borrowed.count, u32::MAX);
        assert_eq!(owned.count, u32::MAX);
        DefinitionSlot::Legacy(&mut borrowed).call();
        DefinitionSlot::Paid(&mut owned).call();
        assert_eq!(borrowed.count, u32::MAX);
        assert_eq!(owned.count, u32::MAX);
        assert!(borrowed.value.is_none() && owned.value.is_none());
    }
    pub(in crate::production_ranked_projection_v1) fn checked_frame_arithmetic() {
        assert!(matches!(
            frame(&[usize::MAX, 1]),
            Err(Error::CanonicalAssertions(
                CanonicalAssertionErrorV1::Resource(Resource::Arithmetic)
            ))
        ));
    }

    pub(in crate::production_ranked_projection_v1) fn value_lengths(
        storage: &BoundsSourceStorageV1,
    ) -> (usize, usize) {
        (storage.local_values.len(), storage.slice_extents.len())
    }
    pub(in crate::production_ranked_projection_v1) fn nested_growth_overflow(
        storage: &mut BoundsSourceStorageV1,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) {
        let edges = storage
            .predecessors
            .iter_mut()
            .find(|rows| !rows.is_empty())
            .unwrap();
        let before = edges.clone();
        assert!(matches!(
            resources.reserve(edges, usize::MAX),
            Err(Error::CanonicalAssertions(
                CanonicalAssertionErrorV1::Resource(Resource::Arithmetic)
            ))
        ));
        assert_eq!(*edges, before);
    }

    pub(in crate::production_ranked_projection_v1) fn definition_count(
        storage: &BoundsSourceStorageV1,
        local: usize,
    ) -> u32 {
        storage.definitions[local].count
    }
}
