//! Shared source-stable metadata decisions, not completed bounds or source authority.
//! Paid state owns only original-ledger argument slots; rich rows and B0 stay borrowed.
use super::bf16_nominal_source_preparation_v1::RichNominalSourceTablesV1;
use super::root_bounds_source_scan_v1::BoundsSourceViewV1;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as WorkIdentity,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1;
use ranked_projection_source_v1::resource;
use std::mem::size_of;

type Error = ProductionRankedProjectionErrorV1;
type Ledger = (usize, WorkIdentity);

enum ExtentMeterV1<'m, 'p, 'w> {
    Legacy(&'m mut dyn ProjectedAssertionFactsV1),
    Paid(&'m mut PreparationResourcesV1<'p, 'w>),
}
impl ExtentMeterV1<'_, '_, '_> {
    fn operand_scan(&mut self, operand: &SemanticOperandV1) -> Result<(), Error> {
        match self {
            Self::Legacy(_) => Ok(()),
            Self::Paid(resources) => {
                let projections = match operand {
                    SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                        place.projections().len()
                    }
                    SemanticOperandV1::Constant(_) => 0,
                };
                // transparent_place may inspect every projection before the
                // unchanged simple_operand_local rejects a nonempty place.
                resources.work(projections)
            }
        }
    }
    fn work(&mut self, amount: usize) -> Result<(), Error> {
        match self {
            Self::Legacy(facts) => facts.charge_private_array_work(amount),
            Self::Paid(resources) => resources.work(amount),
        }
    }
}
enum ExtentDefinitionsV1<'d, 's, 'b> {
    Legacy(&'d [BoundsLocalDefinitionV1<'s>]),
    Paid(&'d BoundsSourceViewV1<'s, 'b>),
}
struct ExtentDecisionsV1<'d, 's, 'b, 'm, 'p, 'w> {
    function: &'s SemanticFunctionDeclV1,
    origins: &'d [Option<u32>],
    arguments: &'d mut [Option<u32>],
    counts: &'d [u8],
    escaped: &'d [bool],
    definitions: ExtentDefinitionsV1<'d, 's, 'b>,
    meter: ExtentMeterV1<'m, 'p, 'w>,
}
impl<'s> ExtentDecisionsV1<'_, 's, '_, '_, '_, '_> {
    fn operand_local(
        &mut self,
        operand: &SemanticOperandV1,
    ) -> Result<Option<SemanticLocalIdV1>, Error> {
        self.meter.operand_scan(operand)?;
        Ok(simple_operand_local(operand))
    }
    fn definition(&mut self, local: usize) -> Result<Option<BoundsLocalDefinitionV1<'s>>, Error> {
        match (&self.definitions, &mut self.meter) {
            (ExtentDefinitionsV1::Legacy(rows), ExtentMeterV1::Legacy(_)) => {
                Ok(rows.get(local).copied())
            }
            (ExtentDefinitionsV1::Paid(view), ExtentMeterV1::Paid(resources)) => {
                // Match Legacy's optional table lookup, not a new out-of-range
                // error. A real in-range coordinate query retains B0's charge.
                if local >= self.function.locals().len() {
                    return Ok(None);
                }
                view.definition(local, resources).map(Some)
            }
            _ => Err(resource(Resource::Accounting)),
        }
    }

    pub(super) fn extent(
        &mut self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        length: SemanticLocalIdV1,
        next_argument: &mut usize,
    ) -> Result<Option<ProductionRankedValueV1>, Error> {
        self.meter.work(16)?;
        let local = length.index() as usize;
        let Some(definition) = self.definition(local)? else {
            return Ok(None);
        };
        let Some(value) = definition.value else {
            return Ok(None);
        };
        if definition.count != 1
            || self.counts.get(local) != Some(&1)
            || self.escaped.get(local) != Some(&false)
            || function
                .locals()
                .get(local)
                .is_none_or(|local| local.ty() != value.result_type())
        {
            return Ok(None);
        }
        let receiver = match value.kind() {
            SemanticRvalueKindV1::Length(place)
                if matches!(place.projections(), [projection]
                if projection.kind() == SemanticProjectionKindV1::Dereference) =>
            {
                place.local()
            }
            SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::PointerMetadata,
                operand,
            } => match self.operand_local(operand)? {
                Some(local) => local,
                None => return Ok(None),
            },
            _ => return Ok(None),
        };
        if !self.metadata_preserving_origin(types, function, receiver, value.result_type())? {
            return Ok(None);
        }
        let origin = self.origins[receiver.index() as usize]
            .ok_or_else(|| resource(Resource::Accounting))? as usize;
        let Some(slot) = self.arguments.get(origin) else {
            return Err(resource(Resource::Accounting));
        };
        if slot.is_none() && *next_argument >= fe2o3_pliron::HARD_MAX_PRODUCTION_RANKED_ARGUMENTS {
            return Err(Error::Unsupported(
                "slice metadata exceeds the ranked argument limit",
            ));
        }
        self.meter.work(8)?;
        let value = project_runtime_slice_extent_argument_v1(
            receiver.index() as usize,
            &self.origins,
            &mut self.arguments,
            next_argument,
        )?;
        Ok(Some(value))
    }

    fn metadata_preserving_origin(
        &mut self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        receiver: SemanticLocalIdV1,
        length_type: SemanticTypeIdV1,
    ) -> Result<bool, Error> {
        let Some(declaration) = function.locals().get(receiver.index() as usize) else {
            return Ok(false);
        };
        let ty = declaration.ty();
        let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
            .get(ty.index() as usize)
            .map(SemanticTypeDeclV1::shape)
        else {
            return Ok(false);
        };
        if pointer.kind() != SemanticPointerKindV1::Reference
            || pointer.metadata() != SemanticPointerMetadataV1::SliceLength
            || !matches!(
                types
                    .get(pointer.pointee().index() as usize)
                    .map(SemanticTypeDeclV1::shape),
                Some(SemanticTypeShapeV1::Slice { .. })
            )
        {
            return Ok(false);
        }
        // Older admitted MIR records usize structurally. The exact metadata
        // operation, not a nominal tag or any arbitrary integer, supplies length.
        if types.get(length_type.index() as usize).is_none_or(|length| {
            !matches!(
                length.rust_type_kind(),
                SemanticRustTypeKindV1::Ordinary | SemanticRustTypeKindV1::Usize
            ) || !matches!(
                length.shape(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed: false, bits })
                    if *bits == pointer.pointer_width_bits()
            )
        }) {
            return Ok(false);
        }
        let Some(origin) = self
            .origins
            .get(receiver.index() as usize)
            .copied()
            .flatten()
        else {
            return Ok(false);
        };
        let mut current = receiver;
        // Stable scalar provenance also follows casts. Metadata equality needs
        // an identical-type copy/move or exact whole-slice shared reborrow.
        for _ in 0..64 {
            self.meter.work(16)?;
            let index = current.index() as usize;
            let Some(local) = function.locals().get(index) else {
                return Ok(false);
            };
            let Some(definition) = self.definition(index)? else {
                return Ok(false);
            };
            if local.ty() != ty
                || self.escaped.get(index) != Some(&false)
                || self.origins.get(index) != Some(&Some(origin))
            {
                return Ok(false);
            }
            if let SemanticLocalRoleV1::Argument(argument) = local.role() {
                return Ok(argument == origin
                    && definition.count == 0
                    && self.counts.get(index) == Some(&0)
                    && function
                        .abi()
                        .adjusted_arguments()
                        .get(argument as usize)
                        .is_some_and(|argument| argument.ty() == ty));
            }
            if definition.count != 1 || self.counts.get(index) != Some(&1) {
                return Ok(false);
            }
            let Some(value) = definition.value else {
                return Ok(false);
            };
            let source = match value.kind() {
                SemanticRvalueKindV1::Use(operand) => {
                    let source = self.operand_local(operand)?;
                    if operand.ty() != ty {
                        return Ok(false);
                    }
                    source
                }
                SemanticRvalueKindV1::Borrow { .. } => {
                    self.meter.work(16)?;
                    bf16_nominal_source_algorithms_v1::exact_shared_slice_reborrow_source_v1(
                        types, function, value,
                    )
                }
                _ => None,
            };
            if value.result_type() != ty {
                return Ok(false);
            }
            let Some(source) = source else {
                return Ok(false);
            };
            current = source;
        }
        Ok(false)
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn extent_legacy_v1(
    scratch: &mut slice_extent_projection_v1::Scratch,
    facts: &mut dyn ProjectedAssertionFactsV1,
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    length: SemanticLocalIdV1,
    definitions: &[BoundsLocalDefinitionV1<'_>],
    next_argument: &mut usize,
) -> Result<Option<ProductionRankedValueV1>, Error> {
    ExtentDecisionsV1 {
        function,
        origins: &scratch.origins,
        arguments: &mut scratch.arguments,
        counts: &scratch.definitions,
        escaped: &scratch.escaped,
        definitions: ExtentDefinitionsV1::Legacy(definitions),
        meter: ExtentMeterV1::Legacy(facts),
    }
    .extent(types, function, length, next_argument)
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct SourceIdentityV1 {
    function: usize,
    types_pointer: usize,
    types_len: usize,
}
fn source_identity(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
) -> SourceIdentityV1 {
    SourceIdentityV1 {
        function: function as *const SemanticFunctionDeclV1 as usize,
        types_pointer: types.as_ptr() as usize,
        types_len: types.len(),
    }
}
fn refuse(message: &'static str) -> Error {
    Error::Unsupported(message)
}
fn occupied<T>(values: &Vec<T>) -> bool {
    !values.is_empty() || values.capacity() != 0
}

#[derive(Default)]
pub(super) struct BoundsExtentArgumentsV1 {
    argument_slots: Vec<Option<u32>>,
    next_argument: usize,
    source: Option<SourceIdentityV1>,
    ledger: Option<Ledger>,
    // Budget slot + Work identity do not authenticate the caller's owned-credit
    // counter. The actual factory must keep the one outer owner/counter coupled.
    started: bool,
    completed: bool,
}
pub(super) struct BoundsExtentArgumentsViewV1<'a> {
    pub(super) argument_slots: &'a [Option<u32>],
    pub(super) next_argument: usize,
}
impl BoundsExtentArgumentsV1 {
    pub(super) fn new() -> Self {
        Self::default()
    }
    fn occupied(&self) -> bool {
        occupied(&self.argument_slots)
            || self.next_argument != 0
            || self.source.is_some()
            || self.ledger.is_some()
            || self.started
            || self.completed
    }
    fn inputs_current(
        &self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        rich: &RichNominalSourceTablesV1<'_>,
        definitions: &BoundsSourceViewV1<'_, '_>,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<(), Error> {
        resources.work(1)?;
        if resources.has_denial()
            || !self.started
            || self.source != Some(source_identity(types, function))
            || self.ledger.is_none()
            || self.ledger != resources.original_ledger_v1()
            || !std::ptr::eq(function, rich.function())
            || !self
                .ledger
                .is_some_and(|ledger| rich.belongs_to_original_ledger_v1(ledger))
        {
            return Err(refuse(
                "bounds extent state outside its original source, types and ledger",
            ));
        }
        definitions.require_same_source_v1(function, resources)?;
        let n = function.locals().len();
        if rich.stable_argument_origins().len() != n
            || rich.scalar_counts().len() != n
            || rich.address_escaped().len() != n
        {
            return Err(refuse(
                "bounds extent rich rows do not match their semantic local table",
            ));
        }
        Ok(())
    }
    /// The caller must supply its existing producer state. This component cannot
    /// prove that a source profile permits empty slots or any particular counter.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn initialize(
        &mut self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        rich: &RichNominalSourceTablesV1<'_>,
        definitions: &BoundsSourceViewV1<'_, '_>,
        initial_arguments: &[Option<u32>],
        next_argument: usize,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<(), Error> {
        if self.occupied() || !resources.is_metered() || resources.has_denial() {
            return Err(refuse(
                "bounds extent state requires a fresh original-ledger owner",
            ));
        }
        // Retain every started state, header and allocation on any subsequent
        // Result failure or unwind. The outer physical owner alone refunds.
        self.started = true;
        self.source = Some(source_identity(types, function));
        self.ledger = resources.original_ledger_v1();
        let frame = extent_frame_v1()?;
        resources.work(frame)?;
        resources.reserve_storage(frame)?;
        self.inputs_current(types, function, rich, definitions, resources)?;
        if initial_arguments.len() != function.locals().len() {
            return Err(refuse(
                "bounds extent argument slots do not match the semantic local table",
            ));
        }
        // No fallible temporary filled vector may drop a partial allocation.
        resources.work(initial_arguments.len())?;
        resources.reserve(&mut self.argument_slots, initial_arguments.len())?;
        self.argument_slots.extend_from_slice(initial_arguments);
        self.next_argument = next_argument;
        if resources.has_denial() {
            return Err(resource(Resource::Accounting));
        }
        self.completed = true;
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn extent(
        &mut self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        rich: &RichNominalSourceTablesV1<'_>,
        definitions: &BoundsSourceViewV1<'_, '_>,
        length: SemanticLocalIdV1,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<Option<ProductionRankedValueV1>, Error> {
        self.inputs_current(types, function, rich, definitions, resources)?;
        if !self.completed || self.argument_slots.len() != function.locals().len() {
            return Err(refuse(
                "bounds extent query requires completed argument state",
            ));
        }
        let result = ExtentDecisionsV1 {
            function,
            origins: rich.stable_argument_origins(),
            arguments: &mut self.argument_slots,
            counts: rich.scalar_counts(),
            escaped: rich.address_escaped(),
            definitions: ExtentDefinitionsV1::Paid(definitions),
            meter: ExtentMeterV1::Paid(resources),
        }
        .extent(types, function, length, &mut self.next_argument);
        if result.is_ok() && resources.has_denial() {
            return Err(resource(Resource::Accounting));
        }
        result
    }
    pub(super) fn view<'a>(
        &'a self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        rich: &RichNominalSourceTablesV1<'_>,
        definitions: &BoundsSourceViewV1<'_, '_>,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<BoundsExtentArgumentsViewV1<'a>, Error> {
        self.inputs_current(types, function, rich, definitions, resources)?;
        if !self.completed || self.argument_slots.len() != function.locals().len() {
            return Err(refuse(
                "bounds extent view requires completed argument state",
            ));
        }
        Ok(BoundsExtentArgumentsViewV1 {
            argument_slots: &self.argument_slots,
            next_argument: self.next_argument,
        })
    }
}
fn frame(parts: &[usize]) -> Result<usize, Error> {
    parts.iter().try_fold(0usize, |n, x| {
        n.checked_add(*x)
            .ok_or_else(|| resource(Resource::Arithmetic))
    })
}
// Each row is a distinct source-level vertex; owner/view padding is never
// treated as spare credit for an unlisted callee. Primitive model getters and
// the already-metered PreparationResources operations retain their own contracts.
fn call_frame_v1<T>(locals: usize) -> Result<usize, Error> {
    frame(&[
        locals,
        size_of::<T>(),
        size_of::<T>(),
        size_of::<Result<T, Error>>(),
        size_of::<Result<T, Error>>(),
    ])
}
const EXTENT_FRAME_ROWS_V1: usize = 23;
fn extent_frame_roster_v1() -> Result<[usize; EXTENT_FRAME_ROWS_V1], Error> {
    Ok([
        // 0: persistent physical destination owner (not spare call-frame space).
        size_of::<BoundsExtentArgumentsV1>(),
        // 1: initialize arguments, frame/length, identity temporaries, denial.
        call_frame_v1::<()>(size_of::<(
            &mut BoundsExtentArgumentsV1,
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            &RichNominalSourceTablesV1<'_>,
            &BoundsSourceViewV1<'_, '_>,
            &[Option<u32>],
            usize,
            &mut PreparationResourcesV1<'_, '_>,
            usize,
            usize,
            Option<SourceIdentityV1>,
            Option<Ledger>,
            bool,
        )>())?,
        // 2: inputs_current; self and all borrowed arguments remain live while
        // source_identity and B0 current/same-source execute. Ledger temporaries
        // include the is_some_and capture and the comparison's returned pair.
        call_frame_v1::<()>(size_of::<(
            &BoundsExtentArgumentsV1,
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            &RichNominalSourceTablesV1<'_>,
            &BoundsSourceViewV1<'_, '_>,
            &mut PreparationResourcesV1<'_, '_>,
            SourceIdentityV1,
            Option<SourceIdentityV1>,
            Option<Ledger>,
            Option<Ledger>,
            Ledger,
            usize,
            bool,
        )>())?,
        // 3: source_identity, including input borrows and all three scalar fields.
        call_frame_v1::<SourceIdentityV1>(size_of::<(
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            usize,
            usize,
            usize,
        )>())?,
        // 4: paid extent entry owns the closed adapter/context while the shared
        // decision engine runs; its returned Result remains live at denial check.
        call_frame_v1::<Option<ProductionRankedValueV1>>(size_of::<(
            &mut BoundsExtentArgumentsV1,
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            &RichNominalSourceTablesV1<'_>,
            &BoundsSourceViewV1<'_, '_>,
            SemanticLocalIdV1,
            &mut PreparationResourcesV1<'_, '_>,
            ExtentDecisionsV1<'_, '_, '_, '_, '_, '_>,
            Result<Option<ProductionRankedValueV1>, Error>,
            bool,
        )>())?,
        // 5: shared extent's arguments, entry row/value, receiver/origin/slot,
        // type-check closure borrow and the nested metadata result.
        call_frame_v1::<Option<ProductionRankedValueV1>>(size_of::<(
            &mut ExtentDecisionsV1<'_, '_, '_, '_, '_, '_>,
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            SemanticLocalIdV1,
            &mut usize,
            usize,
            Option<BoundsLocalDefinitionV1<'_>>,
            BoundsLocalDefinitionV1<'_>,
            Option<&SemanticRvalueV1>,
            &SemanticRvalueV1,
            SemanticLocalIdV1,
            Option<SemanticLocalIdV1>,
            usize,
            Option<u32>,
            &Option<u32>,
            &fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1,
            Result<bool, Error>,
            ProductionRankedValueV1,
        )>())?,
        // 6: metadata walk. These are simultaneously live caller inputs, original
        // declaration/pointer/origin, loop state, current row/value/operand and
        // terminal ABI comparison borrows, not a single shared scalar allowance.
        call_frame_v1::<bool>(size_of::<(
            &mut ExtentDecisionsV1<'_, '_, '_, '_, '_, '_>,
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            SemanticLocalIdV1,
            SemanticTypeIdV1,
            &fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1,
            SemanticTypeIdV1,
            &fe2o3_mir_model::semantic_mir_v1::SemanticPointerTypeV1,
            Option<&SemanticTypeShapeV1>,
            &SemanticTypeDeclV1,
            Option<u32>,
            u32,
            SemanticLocalIdV1,
            std::ops::Range<usize>,
            usize,
            &fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1,
            Option<BoundsLocalDefinitionV1<'_>>,
            BoundsLocalDefinitionV1<'_>,
            Option<&SemanticRvalueV1>,
            &SemanticRvalueV1,
            &SemanticOperandV1,
            Option<SemanticLocalIdV1>,
            SemanticLocalIdV1,
            u32,
            &fe2o3_mir_model::semantic_mir_v1::SemanticAbiArgumentV1,
            bool,
        )>())?,
        // 7: definition's closed match arms and source optional-result transfer.
        call_frame_v1::<Option<BoundsLocalDefinitionV1<'_>>>(size_of::<(
            &mut ExtentDecisionsV1<'_, '_, '_, '_, '_, '_>,
            usize,
            &ExtentDefinitionsV1<'_, '_, '_>,
            &mut ExtentMeterV1<'_, '_, '_>,
            &[BoundsLocalDefinitionV1<'_>],
            Option<&BoundsLocalDefinitionV1<'_>>,
            &BoundsSourceViewV1<'_, '_>,
            &mut PreparationResourcesV1<'_, '_>,
            Result<BoundsLocalDefinitionV1<'_>, Error>,
        )>())?,
        // 8: original project_runtime_slice_extent_argument_v1 remains a nested
        // call: copied/flattened origin, live mutable slot and counter are paid.
        call_frame_v1::<ProductionRankedValueV1>(size_of::<(
            usize,
            &[Option<u32>],
            &mut [Option<u32>],
            &mut usize,
            Option<&Option<u32>>,
            Option<Option<u32>>,
            Option<u32>,
            usize,
            &mut Option<u32>,
            u32,
            Option<usize>,
            Result<u32, std::num::TryFromIntError>,
        )>())?,
        // 9: immutable view entry and its explicit returned borrowed view.
        call_frame_v1::<BoundsExtentArgumentsViewV1<'_>>(size_of::<(
            &BoundsExtentArgumentsV1,
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            &RichNominalSourceTablesV1<'_>,
            &BoundsSourceViewV1<'_, '_>,
            &mut PreparationResourcesV1<'_, '_>,
            BoundsExtentArgumentsViewV1<'_>,
            bool,
        )>())?,
        // 10: the nested closed meter dispatch, not the original primitive meter.
        call_frame_v1::<()>(size_of::<(
            &mut ExtentMeterV1<'_, '_, '_>,
            usize,
            &mut PreparationResourcesV1<'_, '_>,
        )>())?,
        // 11: simple_operand_local called with live parent receiver/operand state.
        call_frame_v1::<Option<SemanticLocalIdV1>>(size_of::<(
            &SemanticOperandV1,
            &SemanticPlaceV1,
            SemanticLocalIdV1,
            bool,
        )>())?,
        // 12: roster + extent_frame return/argument transfer. The complete array
        // is included independently of the callee rows whose numbers it holds.
        call_frame_v1::<[usize; EXTENT_FRAME_ROWS_V1]>(size_of::<(
            [usize; EXTENT_FRAME_ROWS_V1],
            &[usize],
            Result<usize, Error>,
            usize,
        )>())?,
        // 13: call_frame_v1 and frame's separate live parameter/iteration state.
        frame(&[
            call_frame_v1::<usize>(
                size_of::<(usize, [usize; 5], &[usize], Result<usize, Error>)>(),
            )?,
            call_frame_v1::<usize>(size_of::<(
                &[usize],
                std::slice::Iter<'static, usize>,
                usize,
                &usize,
                Option<usize>,
                Result<usize, Error>,
            )>())?,
        ])?,
        // 14: B0 query/current/source-equality locals are private to B0. This
        // explicit additional debit belongs to B1a H, not B0's old scan header.
        super::root_bounds_source_scan_v1::query_frame_v1()?,
        // 15: both freshness predicates remain live during the nested Vec check.
        frame(&[
            call_frame_v1::<bool>(size_of::<(&BoundsExtentArgumentsV1, bool)>())?,
            call_frame_v1::<bool>(size_of::<(&Vec<Option<u32>>, usize, usize, bool)>())?,
        ])?,
        // 16: this module's refusal constructor/error transfer is independent
        // of the B0 refusal constructor accounted by row 14.
        call_frame_v1::<Error>(size_of::<(&'static str, Error)>())?,
        // 17: closed operand-local wrapper retains receiver and operand while
        // the paid precharge and the unchanged original helper run.
        call_frame_v1::<Option<SemanticLocalIdV1>>(size_of::<(
            &mut ExtentDecisionsV1<'_, '_, '_, '_, '_, '_>,
            &SemanticOperandV1,
            Option<SemanticLocalIdV1>,
        )>())?,
        // 18: the closed meter's paid/legacy scan selector and count.
        call_frame_v1::<()>(size_of::<(
            &mut ExtentMeterV1<'_, '_, '_>,
            &SemanticOperandV1,
            &SemanticPlaceV1,
            usize,
            &mut PreparationResourcesV1<'_, '_>,
        )>())?,
        // 19: raw_operand_place returns a borrowed place without payload copies.
        call_frame_v1::<Option<&SemanticPlaceV1>>(
            size_of::<(&SemanticOperandV1, &SemanticPlaceV1)>(),
        )?,
        // 20: transparent_operand_place keeps its caller operand and the raw
        // borrowed-place result live while transparent_place scans.
        call_frame_v1::<Option<&SemanticPlaceV1>>(size_of::<(
            &SemanticOperandV1,
            Option<&SemanticPlaceV1>,
            &SemanticPlaceV1,
        )>())?,
        // 21: transparent_place's all() iterator and projection-kind predicate.
        call_frame_v1::<Option<&SemanticPlaceV1>>(size_of::<(
            &SemanticPlaceV1,
            &[fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1],
            std::slice::Iter<'static, fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1>,
            &fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1,
            SemanticProjectionKindV1,
            bool,
            Option<&SemanticPlaceV1>,
        )>())?,
        // 22: exact shared reborrow classification is a distinct nested call.
        bf16_nominal_source_algorithms_v1::shared_slice_reborrow_frame_v1(),
    ])
}
pub(super) fn extent_frame_v1() -> Result<usize, Error> {
    frame(&extent_frame_roster_v1()?)
}
#[cfg(test)]
pub(super) mod test_access {
    use super::*;
    pub(in crate::production_ranked_projection_v1) fn state(
        owner: &BoundsExtentArgumentsV1,
    ) -> (bool, bool, usize, usize, usize) {
        (
            owner.started,
            owner.completed,
            owner.argument_slots.len(),
            owner.argument_slots.capacity(),
            owner.next_argument,
        )
    }
    pub(in crate::production_ranked_projection_v1) fn occupy_capacity(
        owner: &mut BoundsExtentArgumentsV1,
    ) {
        owner.argument_slots.reserve_exact(1);
    }
    pub(in crate::production_ranked_projection_v1) fn copy_rows(
        owner: &BoundsExtentArgumentsV1,
    ) -> Vec<Option<u32>> {
        owner.argument_slots.clone()
    }
    pub(in crate::production_ranked_projection_v1) fn audit_frame_rows() {
        fn expected<T>(locals: usize) -> usize {
            locals + 2 * size_of::<T>() + 2 * size_of::<Result<T, Error>>()
        }
        let rows = extent_frame_roster_v1().unwrap();
        assert_eq!(rows.len(), 23);
        assert_eq!(rows[0], size_of::<BoundsExtentArgumentsV1>());
        // Independently spell the live shared-call inputs and retained outputs:
        // even without the remaining branch locals, neither may use another
        // vertex's row or physical-owner padding to meet this lower bound.
        let extent_inputs = size_of::<(
            &mut ExtentDecisionsV1<'_, '_, '_, '_, '_, '_>,
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            SemanticLocalIdV1,
            &mut usize,
            BoundsLocalDefinitionV1<'_>,
            &SemanticRvalueV1,
            SemanticLocalIdV1,
            usize,
            &Option<u32>,
            Result<bool, Error>,
        )>();
        assert!(rows[5] >= expected::<Option<ProductionRankedValueV1>>(extent_inputs));
        let metadata_inputs = size_of::<(
            &mut ExtentDecisionsV1<'_, '_, '_, '_, '_, '_>,
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            SemanticLocalIdV1,
            SemanticTypeIdV1,
            &fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1,
            &fe2o3_mir_model::semantic_mir_v1::SemanticPointerTypeV1,
            u32,
            SemanticLocalIdV1,
            std::ops::Range<usize>,
            usize,
            BoundsLocalDefinitionV1<'_>,
            &SemanticRvalueV1,
            &SemanticOperandV1,
        )>();
        assert!(rows[6] >= expected::<bool>(metadata_inputs));
        let helper_inputs = size_of::<(
            usize,
            &[Option<u32>],
            &mut [Option<u32>],
            &mut usize,
            usize,
            &mut Option<u32>,
            u32,
        )>();
        assert!(rows[8] >= expected::<ProductionRankedValueV1>(helper_inputs));
        assert_eq!(
            rows[14],
            super::super::root_bounds_source_scan_v1::query_frame_v1().unwrap()
        );
        assert!(rows[14] > 0);
        let mut sum = 0usize;
        for row in rows {
            sum = sum.checked_add(row).unwrap();
        }
        assert_eq!(extent_frame_v1().unwrap(), sum);
        assert!(rows[1..23].iter().all(|n| *n > 0));
        let reborrow_inputs = size_of::<(
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            &SemanticRvalueV1,
            &SemanticPlaceV1,
            &fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1,
            &fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1,
            &fe2o3_mir_model::semantic_mir_v1::SemanticPointerTypeV1,
            &SemanticTypeShapeV1,
            Option<SemanticLocalIdV1>,
            Option<SemanticLocalIdV1>,
        )>();
        assert!(rows[22] >= reborrow_inputs);
        assert_eq!(
            rows[22],
            bf16_nominal_source_algorithms_v1::shared_slice_reborrow_frame_v1()
        );
    }
    pub(in crate::production_ranked_projection_v1) fn audit_projection_frame_rows() {
        let rows = extent_frame_roster_v1().unwrap();
        fn expected<T>(locals: usize) -> usize {
            locals + 2 * size_of::<T>() + 2 * size_of::<Result<T, Error>>()
        }
        assert_eq!(
            rows[19],
            expected::<Option<&SemanticPlaceV1>>(
                size_of::<(&SemanticOperandV1, &SemanticPlaceV1)>()
            )
        );
        assert_eq!(
            rows[20],
            expected::<Option<&SemanticPlaceV1>>(size_of::<(
                &SemanticOperandV1,
                Option<&SemanticPlaceV1>,
                &SemanticPlaceV1
            )>())
        );
        assert_eq!(
            rows[21],
            expected::<Option<&SemanticPlaceV1>>(size_of::<(
                &SemanticPlaceV1,
                &[fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1],
                std::slice::Iter<'static, fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1>,
                &fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1,
                SemanticProjectionKindV1,
                bool,
                Option<&SemanticPlaceV1>,
            )>())
        );
    }
    pub(in crate::production_ranked_projection_v1) fn frame_call_overflow() {
        assert!(matches!(
            call_frame_v1::<Option<ProductionRankedValueV1>>(usize::MAX),
            Err(Error::CanonicalAssertions(
                CanonicalAssertionErrorV1::Resource(Resource::Arithmetic)
            ))
        ));
        super::super::root_bounds_source_scan_v1::test_access::query_frame_overflow_control();
    }
    pub(in crate::production_ranked_projection_v1) fn checked_frame_overflow() {
        assert!(matches!(
            frame(&[usize::MAX, 1]),
            Err(Error::CanonicalAssertions(
                CanonicalAssertionErrorV1::Resource(Resource::Arithmetic)
            ))
        ));
    }
}
