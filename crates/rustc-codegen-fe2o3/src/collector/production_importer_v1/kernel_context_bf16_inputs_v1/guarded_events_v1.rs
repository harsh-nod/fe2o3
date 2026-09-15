//! Source-bound guarded/volatile subevents. This does not close final reads.
use super::*;
use fe2o3_lower_mir_kernel::ProductionGlobalBf16SourceOperandV1;
use fe2o3_mir_model::{SsaValueV1, semantic_mir_v1::*};
use fe2o3_pliron::{ProductionSemanticSsaSourceQueryV1, ProductionSemanticSsaSourceSiteV1};

#[path = "guarded_events_v1/formula.rs"]
mod formula;
#[path = "guarded_events_v1/live.rs"]
mod live;
#[path = "guarded_events_v1/reference_bindings.rs"]
mod reference_bindings;
#[path = "guarded_events_v1/source_final_reads.rs"]
mod source_final_reads;
#[path = "guarded_events_v1/root_physical.rs"]
mod root_physical;
pub(crate) use root_physical::RootPhysicalV1 as RootBf16PhysicalInputV1;
#[path = "guarded_events_v1/root_owner.rs"]
mod root_owner;

type E = ProductionSemanticKirErrorV1;
type Result<T> = std::result::Result<T, E>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceValue {
    Constant(u64),
    Ssa {
        variable: u32,
        value: SsaValueV1,
        event_range: [usize; 2],
        agreeing_uses: usize,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScalarUse {
    site: ProductionSemanticSsaSourceSiteV1,
    ty: SemanticTypeIdV1,
    value: SourceValue,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceKey {
    semantic: [u8; 32],
    expansion: [u8; 32],
    expanded_root: [u8; 32],
    root: SemanticFunctionIdV1,
    block: u32,
    original_load: [u32; 3],
    callee: SemanticCallableIdV1,
    destination: (SemanticLocalIdV1, SemanticTypeIdV1),
    normal_edge: SemanticControlFlowEdgeV1,
    unwind: SemanticUnwindActionV1,
    preceding_statements: usize,
    wrapper: u32,
    constructor: u32,
    global: SsaValueV1,
    matrix: SsaValueV1,
    physical_site: ProductionSemanticSsaSourceSiteV1,
    physical_type: SemanticTypeIdV1,
    physical_destination: u32,
    physical_slice: SemanticTypeIdV1,
    lane: ProductionScopedBf16LaneUseV1,
    scalar_uses: [ScalarUse; 6],
    reference_uses: [reference_bindings::SsaUse; 2],
}

struct EventRow {
    key: SourceKey,
    graph: live::LiveEvents,
}

/// Owns the still-open native/source session, not a detached memory receipt.
/// Every graph is relative to one exact source Global and its physical extent.
pub(crate) struct GuardedBf16SourceEventsV1<'native, 'source> {
    inputs: CapturedGlobalBf16SourceInputsV1<'native, 'source>,
    rows: Vec<EventRow>,
}

fn error(error: formula::Error) -> E {
    E::Unsupported {
        function: 0,
        block: None,
        statement: None,
        detail: match error {
            formula::Error::Budget => "BF16 guarded event exceeded the shared source work budget",
            formula::Error::Capacity => "BF16 guarded event exceeded its bounded formula storage",
            formula::Error::Source => "BF16 guarded event lost its original scalar SSA source",
            formula::Error::Graph => {
                "BF16 guarded event graph is not the exact read producer graph"
            }
            formula::Error::Changed => {
                "BF16 guarded event changed its source-bound guard, offset, extent or volatility"
            }
            formula::Error::Roster => {
                "BF16 guarded event changed its exact ordered four-read roster"
            }
        },
    }
}

fn source_key(
    row: ProductionGlobalBf16SourceRowV1<'_, '_>,
    lane: ProductionScopedBf16LaneUseV1,
    charge: &mut dyn FnMut(usize) -> Result<()>,
) -> Result<SourceKey> {
    charge(8 + std::mem::size_of::<SourceKey>().div_ceil(std::mem::size_of::<usize>()))?;
    if lane.contract() != row.contract()
        || row.contract().operand().wave_width != 64
        || row.contract().memory() != SemanticCapabilityMemoryContractV1::global_read_only()
    {
        return Err(E::CorrespondenceMismatch);
    }
    // The captured source row is private-construction, and retains the original
    // Global issuer/physical source. These are NOT noalias or allocation IDs.
    let root_count = row.owner().execution_expansion().roots().len();
    charge(4 * (usize::BITS as usize - root_count.leading_zeros() as usize + 1))?;
    let query = row
        .owner()
        .source_query_for_root(row.view().root(), row.view().body())
        .map_err(|_| E::CorrespondenceMismatch)?;
    let physical_slice = physical_extent_slice(
        row.owner().source_semantic().types(),
        row.physical().operand().ty(),
        row.contract().types().element,
        charge,
    )?;
    let reference_uses = [
        reference_bindings::reference_use(
            row.owner().source_semantic().types(), &query, row.physical(),
            physical_slice, SemanticPointerMetadataV1::SliceLength, charge,
        )?,
        reference_bindings::reference_use(
            row.owner().source_semantic().types(), &query, row.lane(),
            row.contract().types().lane, SemanticPointerMetadataV1::None, charge,
        )?,
    ];
    if reference_uses[1].value != lane.lane_reference() {
        return Err(E::CorrespondenceMismatch);
    }
    let geometry = row.geometry();
    let bases = row.bases();
    let origin = row
        .view()
        .block_origins()
        .get(row.load_block() as usize)
        .ok_or(E::CorrespondenceMismatch)?;
    let destination = row
        .call()
        .destination()
        .ok_or(E::CorrespondenceMismatch)?
        .place();
    let operands = [
        geometry[0],
        geometry[1],
        geometry[2],
        geometry[3],
        bases[0],
        bases[1],
    ];
    let mut uses = Vec::new();
    charge(6 * std::mem::size_of::<ScalarUse>().div_ceil(std::mem::size_of::<usize>()) + 3)?;
    uses.try_reserve_exact(6)
        .map_err(|_| E::CorrespondenceMismatch)?;
    for operand in operands {
        uses.push(scalar_use(
            row.owner().source_semantic().types(),
            &query,
            operand,
            charge,
        )?);
    }
    Ok(SourceKey {
        semantic: *row.owner().source_semantic().semantic_sha256().as_bytes(),
        expansion: *row.owner().execution_expansion().identity(),
        expanded_root: *row.view().identity(),
        root: row.view().root(),
        block: row.load_block(),
        wrapper: row.wrapper_instance().index(),
        original_load: [
            origin.instance().index(),
            origin.function().index(),
            origin.block().index(),
        ],
        callee: row.call().callee(),
        destination: (destination.local(), destination.ty()),
        normal_edge: row.call().destination().ok_or(E::CorrespondenceMismatch)?.edge(),
        unwind: row.call().unwind(),
        preceding_statements: row.view().body().blocks()
            .get(row.load_block() as usize).ok_or(E::CorrespondenceMismatch)?
            .statements().len(),
        constructor: row.checked_instance().index(),
        global: row.global_value(),
        matrix: row.matrix_value(),
        physical_site: row.physical().site(),
        physical_type: row.physical().operand().ty(),
        lane,
        physical_destination: row.physical().destination_local(),
        physical_slice,
        scalar_uses: uses.try_into().map_err(|_| E::CorrespondenceMismatch)?,
        reference_uses,
    })
}

fn physical_extent_slice(
    types: &[SemanticTypeDeclV1],
    physical: SemanticTypeIdV1,
    element: SemanticTypeIdV1,
    charge: &mut dyn FnMut(usize) -> Result<()>,
) -> Result<SemanticTypeIdV1> {
    charge(2)?;
    let Some(SemanticTypeShapeV1::Pointer(p)) = types
        .get(physical.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Err(E::CorrespondenceMismatch);
    };
    if p.kind() != SemanticPointerKindV1::Reference
        || p.mutability() != SemanticMutabilityV1::Immutable
        || p.pointer_width_bits() != 64
        || p.address_space() != 0
        || p.metadata() != SemanticPointerMetadataV1::SliceLength
        || !matches!(types.get(p.pointee().index() as usize).map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Slice{element:actual}) if *actual==element)
    {
        return Err(E::CorrespondenceMismatch);
    }
    Ok(p.pointee())
}

fn scalar_use<'source>(
    types: &[SemanticTypeDeclV1],
    query: &ProductionSemanticSsaSourceQueryV1<'source>,
    operand: ProductionGlobalBf16SourceOperandV1<'source>,
    charge: &mut dyn FnMut(usize) -> Result<()>,
) -> Result<ScalarUse> {
    charge(1)?;
    let ty = operand.operand().ty();
    if !matches!(
        types
            .get(ty.index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64
        }))
    ) {
        return Err(error(formula::Error::Source));
    }
    let value = match operand.operand() {
        SemanticOperandV1::Constant(c) => match c.value() {
            SemanticConstantValueV1::Scalar(c) => SourceValue::Constant(
                u64::try_from(c.bits()).map_err(|_| error(formula::Error::Source))?,
            ),
            _ => return Err(error(formula::Error::Source)),
        },
        SemanticOperandV1::Copy(_) | SemanticOperandV1::Move(_) => {
            reference_bindings::ssa_use(
                query, operand.site(), operand.operand(), charge,
            )?.scalar()
        }
    };
    Ok(ScalarUse {
        site: operand.site(),
        ty,
        value,
    })
}

fn constants(key: &SourceKey) -> [Option<u64>; 8] {
    let mut result = [None; 8];
    for (slot, use_) in [1usize, 2, 3, 4, 5, 6]
        .into_iter()
        .zip(key.scalar_uses.iter())
    {
        if let SourceValue::Constant(value) = use_.value {
            result[slot] = Some(value);
        }
    }
    result
}

fn with_formula_budget<T>(
    charge: &mut dyn FnMut(usize) -> Result<()>,
    run: impl FnOnce(formula::Charge<'_>) -> formula::Result<T>,
) -> Result<T> {
    let mut failure = None;
    let result = run(&mut |words| match charge(words) {
        Ok(()) => true,
        Err(e) => {
            failure = Some(e);
            false
        }
    });
    if let Some(e) = failure {
        return Err(e);
    }
    result.map_err(error)
}

impl<'native, 'source> CapturedGlobalBf16SourceInputsV1<'native, 'source> {
    /// Materializes and checks each original terminal's four guarded volatile
    /// source subevents, retaining the still-open final-read session.
    pub(crate) fn prepare_guarded_volatile_events(
        mut self,
    ) -> Result<GuardedBf16SourceEventsV1<'native, 'source>> {
        let mut rows = Vec::new();
        // Reserve the batch only while borrowing its existing source work owner.
        for index in 0..self.len() {
            let row = self.batch.row(index).ok_or(E::CorrespondenceMismatch)?;
            let count = self.len();
            let event =
                self.session
                    .with_checked_global_bf16_read_source(row, |row, lane, charge| {
                        if index == 0 {
                            let words = count
                                .checked_mul(std::mem::size_of::<EventRow>())
                                .and_then(|n| n.checked_add(std::mem::size_of::<Vec<EventRow>>()))
                                .ok_or(E::CorrespondenceMismatch)?
                                .div_ceil(std::mem::size_of::<usize>());
                            charge(words)?;
                            rows.try_reserve_exact(count)
                                .map_err(|_| E::CorrespondenceMismatch)?;
                            if rows.capacity() != count {
                                return Err(E::CorrespondenceMismatch);
                            }
                        }
                        let key = source_key(row, lane, charge)?;
                        let role_b =
                            key.lane.contract().operand().role == SemanticMfmaOperandRoleV1::B;
                        let graph = with_formula_budget(charge, |f| {
                            let actual = formula::projected(role_b, f)?;
                            let graph = live::LiveEvents::new(&actual, constants(&key), f)?;
                            let expected = formula::reference(role_b, f)?;
                            graph.verify(&expected, f)?;
                            Ok(graph)
                        })?;
                        Ok(EventRow { key, graph })
                    })?;
            rows.push(event);
        }
        if rows.len() != self.len() || rows.is_empty() {
            return Err(E::CorrespondenceMismatch);
        }
        Ok(GuardedBf16SourceEventsV1 { inputs: self, rows })
    }
}

impl GuardedBf16SourceEventsV1<'_, '_> {
    pub(crate) fn source_event_count(&self) -> usize {
        self.rows.len() * 4
    }
    /// Replays source bindings and live graph identities, never just labels.
    pub(crate) fn verify_source_events(&mut self) -> Result<()> {
        if self.rows.len() != self.inputs.len() {
            return Err(E::CorrespondenceMismatch);
        }
        for (index, event) in self.rows.iter().enumerate() {
            let row = self
                .inputs
                .batch
                .row(index)
                .ok_or(E::CorrespondenceMismatch)?;
            self.inputs.session.with_checked_global_bf16_read_source(
                row,
                |row, lane, charge| {
                    if source_key(row, lane, charge)? != event.key {
                        return Err(E::CorrespondenceMismatch);
                    }
                    with_formula_budget(charge, |f| {
                        let expected = formula::reference(
                            lane.contract().operand().role == SemanticMfmaOperandRoleV1::B,
                            f,
                        )?;
                        event.graph.verify(&expected, f)?;
                        let values = event.graph.results()?;
                        for (i, value) in values.iter().enumerate() {
                            for previous in &values[..i] {
                                formula::charge(f, 1)?;
                                if value == previous {
                                    return Err(formula::Error::Roster);
                                }
                            }
                        }
                        Ok(())
                    })
                },
            )?;
        }
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn test_finish_without_reads(self) -> Result<()> {
        // These source graphs are not final memory effects. Keep the existing
        // session rejection until the final producer roster is actually joined.
        self.inputs.test_finish_without_reads()
    }
    #[cfg(test)]
    pub(crate) fn test_reject_source_substitutions(&mut self) {
        let original = self.rows[0].key.clone();
        for case in 0..31 {
            self.rows[0].key = original.clone();
            let key = &mut self.rows[0].key;
            match case {
                0 => key.semantic[0] ^= 1,
                1 => key.expansion[0] ^= 1,
                2 => key.expanded_root[0] ^= 1,
                3 => key.root = SemanticFunctionIdV1::from_index(key.root.index().wrapping_add(1)),
                4 => key.block = key.block.wrapping_add(1),
                5 => key.wrapper = key.wrapper.wrapping_add(1),
                6 => {
                    key.physical_type =
                        SemanticTypeIdV1::from_index(key.physical_type.index().wrapping_add(1))
                }
                7 => {
                    key.scalar_uses[0].ty =
                        SemanticTypeIdV1::from_index(key.scalar_uses[0].ty.index().wrapping_add(1))
                }
                8 => key.constructor = key.constructor.wrapping_add(1),
                9 => key.original_load[0] = key.original_load[0].wrapping_add(1),
                10 => key.original_load[1] = key.original_load[1].wrapping_add(1),
                11 => key.original_load[2] = key.original_load[2].wrapping_add(1),
                12 => {
                    key.callee =
                        SemanticCallableIdV1::from_index(key.callee.index().wrapping_add(1))
                }
                13 => {
                    key.destination.0 =
                        SemanticLocalIdV1::from_index(key.destination.0.index().wrapping_add(1))
                }
                14 => key.physical_destination = key.physical_destination.wrapping_add(1),
                15 => {
                    key.physical_slice =
                        SemanticTypeIdV1::from_index(key.physical_slice.index().wrapping_add(1))
                }
                16 => match &mut key
                    .scalar_uses
                    .iter_mut()
                    .find(|s| matches!(s.value, SourceValue::Ssa { .. }))
                    .expect("actual dynamic source geometry")
                    .value
                {
                    SourceValue::Ssa { event_range, .. } => {
                        event_range[0] = event_range[0].wrapping_add(1)
                    }
                    _ => unreachable!(),
                },
                17 => {
                    key.scalar_uses[0].value = match key.scalar_uses[0].value {
                        SourceValue::Constant(value) => SourceValue::Constant(value ^ 1),
                        SourceValue::Ssa {
                            variable,
                            value,
                            event_range,
                            agreeing_uses,
                        } => SourceValue::Ssa {
                            variable: variable.wrapping_add(1),
                            value,
                            event_range,
                            agreeing_uses,
                        },
                    }
                }
                28 => key.normal_edge = SemanticControlFlowEdgeV1::new(
                    key.normal_edge.role(),
                    SemanticBlockIdV1::from_index(key.normal_edge.target().index().wrapping_add(1)),
                ),
                29 => key.unwind = if key.unwind == SemanticUnwindActionV1::Unreachable {
                    SemanticUnwindActionV1::Terminate
                } else {
                    SemanticUnwindActionV1::Unreachable
                },
                30 => key.preceding_statements = key.preceding_statements.wrapping_add(1),
                18..28 => {
                    let use_ = &mut key.reference_uses[(case - 18) / 5];
                    match (case - 18) % 5 {
                        0 => use_.ty = SemanticTypeIdV1::from_index(use_.ty.index().wrapping_add(1)),
                        1 => use_.variable = use_.variable.wrapping_add(1),
                        2 => use_.event_range[0] = use_.event_range[0].wrapping_add(1),
                        3 => use_.event_range[1] = use_.event_range[1].wrapping_add(1),
                        4 => use_.agreeing_uses = use_.agreeing_uses.wrapping_add(1),
                        _ => unreachable!(),
                    }
                }
                _ => unreachable!(),
            }
            let result = self.verify_source_events();
            self.rows[0].key = original.clone();
            assert!(
                matches!(result, Err(E::CorrespondenceMismatch)),
                "source mutation {case}: {result:?}"
            );
        }
        self.rows.swap(0, 1);
        let reordered = self.verify_source_events();
        self.rows.swap(0, 1);
        assert!(
            matches!(reordered, Err(E::CorrespondenceMismatch)),
            "{reordered:?}"
        );
        let last = self.rows.pop().unwrap();
        let omitted = self.verify_source_events();
        self.rows.push(last);
        assert!(
            matches!(omitted, Err(E::CorrespondenceMismatch)),
            "{omitted:?}"
        );
        let original = self.rows[1].key.clone();
        self.rows[1].key = self.rows[0].key.clone();
        let duplicate = self.verify_source_events();
        self.rows[1].key = original;
        assert!(
            matches!(duplicate, Err(E::CorrespondenceMismatch)),
            "{duplicate:?}"
        );
    }
}

#[cfg(test)]
#[path = "guarded_events_v1/source_tests.rs"]
mod tests;
