//! Genuine source-prefix ORIGINAL oracle. Test-only, independent of candidate queries.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_source_preparation_v1::NominalRootCfgSourceV1;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBasicBlockV1, SemanticConstantV1, SemanticControlFlowEdgeV1, SemanticScalarValueV1,
    SemanticTerminatorV1,
};

pub(in crate::production_ranked_projection_v1) const GENUINE_PREFIX_CAP: usize = 32;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) enum GenuineErrorDataV1 {
    Incomplete(&'static str),
    Unsupported(&'static str),
    Resource(Resource),
    UncoveredOwningError,
}
pub(in crate::production_ranked_projection_v1) fn genuine_error_data_v1(
    error: &Error,
) -> GenuineErrorDataV1 {
    match error {
        Error::Incomplete(message) => GenuineErrorDataV1::Incomplete(message),
        Error::Unsupported(message) => GenuineErrorDataV1::Unsupported(message),
        Error::CanonicalAssertions(
            crate::production_ranked_projection_v1::canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(error)
        ) => GenuineErrorDataV1::Resource(*error),
        _ => GenuineErrorDataV1::UncoveredOwningError,
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) enum GenuinePrefixEventV1 {
    Other,
    LiteralSkip {
        length: u64,
        index: u64,
    },
    FixedPending,
    Fixed {
        index: SemanticLocalIdV1,
        extent: u64,
    },
    NonFixedBoundary,
    MalformedBounds,
    Refused(GenuineErrorDataV1),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) enum GenuinePrefixStopV1 {
    End,
    NonFixedBoundary,
    MalformedBounds,
    OriginalRefusal,
    CoverageLimit,
    CheckpointUnwind,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) struct GenuineSourceStampV1 {
    pub function: usize,
    pub types: usize,
    pub type_count: usize,
    pub graph: usize,
    pub rich: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) struct GenuinePrefixRowV1 {
    pub block: usize,
    pub source_address: usize,
    pub success: Option<usize>,
    pub event: GenuinePrefixEventV1,
}
const EMPTY_ROW: GenuinePrefixRowV1 = GenuinePrefixRowV1 {
    block: usize::MAX,
    source_address: 0,
    success: None,
    event: GenuinePrefixEventV1::Other,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) struct GenuinePrefixExpectedV1 {
    pub source: Option<GenuineSourceStampV1>,
    pub rows: [GenuinePrefixRowV1; GENUINE_PREFIX_CAP],
    pub len: usize,
    pub fixed: usize,
    pub stop: GenuinePrefixStopV1,
}
impl Default for GenuinePrefixExpectedV1 {
    fn default() -> Self {
        Self {
            source: None,
            rows: [EMPTY_ROW; GENUINE_PREFIX_CAP],
            len: 0,
            fixed: 0,
            stop: GenuinePrefixStopV1::End,
        }
    }
}
pub(in crate::production_ranked_projection_v1) fn genuine_source_stamp_v1(
    cfg: &NominalRootCfgSourceV1<'_>,
) -> GenuineSourceStampV1 {
    GenuineSourceStampV1 {
        function: cfg.function() as *const _ as usize,
        types: cfg.types().as_ptr() as usize,
        type_count: cfg.types().len(),
        graph: cfg.graph() as *const _ as usize,
        rich: cfg.source_tables().rich() as *const _ as usize,
    }
}

// Original constant semantics, with local relocation only. Never a lazy visit.
#[derive(Clone, Copy)]
pub(super) enum OriginalConstantDefinition {
    Missing,
    Direct(u64),
    Alias(SemanticLocalIdV1),
    Invalid,
}
fn original_constant_definition(operand: &SemanticOperandV1) -> OriginalConstantDefinition {
    match operand {
        SemanticOperandV1::Constant(constant) => match constant.value() {
            SemanticConstantValueV1::Scalar(value) => u64::try_from(value.bits())
                .map(OriginalConstantDefinition::Direct)
                .unwrap_or(OriginalConstantDefinition::Invalid),
            _ => OriginalConstantDefinition::Invalid,
        },
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
            if place.projections().is_empty() =>
        {
            OriginalConstantDefinition::Alias(place.local())
        }
        SemanticOperandV1::Copy(_) | SemanticOperandV1::Move(_) => {
            OriginalConstantDefinition::Invalid
        }
    }
}
fn original_constant_operand_value(
    operand: &SemanticOperandV1,
    constants: &[Option<u64>],
) -> Option<u64> {
    match original_constant_definition(operand) {
        OriginalConstantDefinition::Direct(value) => Some(value),
        OriginalConstantDefinition::Alias(local) => {
            constants.get(local.index() as usize).copied().flatten()
        }
        OriginalConstantDefinition::Missing | OriginalConstantDefinition::Invalid => None,
    }
}
fn classify_original_row(
    kind: &SemanticTerminatorKindV1,
    constants: &[Option<u64>],
) -> (GenuinePrefixEventV1, Option<usize>) {
    match kind {
        SemanticTerminatorKindV1::Assert {
            expected,
            message: SemanticAssertMessageV1::BoundsCheck { length, index },
            target,
            unwind,
            ..
        } => {
            if !*expected || !matches!(unwind, SemanticUnwindActionV1::Unreachable) {
                return (GenuinePrefixEventV1::MalformedBounds, None);
            }
            let success = Some(target.target().index() as usize);
            if let Some(length_value) = original_constant_operand_value(length, constants) {
                if let Some(index_value) = original_constant_operand_value(index, constants) {
                    return (
                        GenuinePrefixEventV1::LiteralSkip {
                            length: length_value,
                            index: index_value,
                        },
                        success,
                    );
                }
            }
            if matches!(length, SemanticOperandV1::Constant(_)) {
                (GenuinePrefixEventV1::FixedPending, success)
            } else {
                (GenuinePrefixEventV1::NonFixedBoundary, success)
            }
        }
        SemanticTerminatorKindV1::SwitchInt { .. } => {
            (GenuinePrefixEventV1::NonFixedBoundary, None)
        }
        _ => (GenuinePrefixEventV1::Other, None),
    }
}
fn admit_genuine_prefix(resources: &mut Prep<'_, '_>) -> R<usize> {
    if !resources.is_metered() || resources.has_denial() {
        return Err(assertion_resource_accounting_v1());
    }
    let bytes = genuine_prefix_frame_v1()?;
    let work = bytes
        .checked_add(genuine_prefix_work_v1()?)
        .ok_or_else(assertion_resource_overflow_v1)?;
    resources.work(work)?;
    resources.reserve_storage(bytes)?;
    // B2 pays its unchanged original/query source vertices and EXACT three reads.
    let content = admit_content(resources)?;
    bytes
        .checked_add(content)
        .ok_or_else(assertion_resource_overflow_v1)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn with_original_genuine_prefix_v1<'a, 'b, 'w>(
    cfg: &'a NominalRootCfgSourceV1<'_>,
    resources: &'a mut Prep<'b, 'w>,
    state: &mut QueryState,
    slot: &mut Option<RetiredOriginalFixedOracleV1>,
    constructor_cuts: &mut Cuts,
    cuts: &mut QueryCuts,
    witness: &mut QueryWitness,
    observation: &mut ContentWitness,
    expected: &mut GenuinePrefixExpectedV1,
) -> R<GenuinePrefixExpectedV1> {
    if *state != QueryState::Fresh || slot.is_some() {
        return Err(assertion_resource_accounting_v1());
    }
    *state = QueryState::Terminal;
    if *witness != QueryWitness::default()
        || !cuts.is_fresh()
        || *observation != ContentWitness::default()
        || *expected != GenuinePrefixExpectedV1::default()
    {
        return Err(assertion_resource_accounting_v1());
    }
    // No preclassification. This checks only the fixed observation boundary.
    if cfg.function().blocks().len() > GENUINE_PREFIX_CAP {
        expected.stop = GenuinePrefixStopV1::CoverageLimit;
        return Err(Error::Unsupported(
            "genuine original source exceeds its closed prefix",
        ));
    }
    witness.prefix = admit_genuine_prefix(resources)?;
    witness.admitted = true;
    expected.source = Some(genuine_source_stamp_v1(cfg));
    constructor_cuts.prepare_payload();
    cuts.prepare_payload();
    *state = QueryState::Active;
    let source = QuerySource {
        types: cfg.types(),
        function: cfg.function(),
        graph: cfg.graph(),
        rich: cfg.source_tables().rich(),
    };
    let reserved = EmptySlot(slot);
    let mut owner = OriginalFixedOracleOwnerV1::new(source.inputs(), resources);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        for (block, row) in source.function.blocks().iter().enumerate() {
            let (event, success) =
                classify_original_row(row.terminator().kind(), source.rich.constants());
            expected.rows[block] = GenuinePrefixRowV1 {
                block,
                source_address: row as *const _ as usize,
                success,
                event,
            };
            expected.len = block + 1;
            match event {
                GenuinePrefixEventV1::MalformedBounds => {
                    expected.stop = GenuinePrefixStopV1::MalformedBounds;
                    return Err(Error::Incomplete(
                        "a Rust bounds check without the canonical success/unreachable shape",
                    ));
                }
                GenuinePrefixEventV1::NonFixedBoundary => {
                    expected.stop = GenuinePrefixStopV1::NonFixedBoundary;
                    break;
                }
                GenuinePrefixEventV1::FixedPending => {
                    witness.stopped_at = Some(block);
                    // Same independent ORIGINAL suffix, not a candidate result.
                    let result = owner_query(&mut owner, source, block, constructor_cuts);
                    let (index, extent) = match result {
                        Ok(data) => data,
                        Err(error) => {
                            expected.rows[block].event =
                                GenuinePrefixEventV1::Refused(genuine_error_data_v1(&error));
                            expected.stop = GenuinePrefixStopV1::OriginalRefusal;
                            return Err(error);
                        }
                    };
                    expected.rows[block].event = GenuinePrefixEventV1::Fixed { index, extent };
                    witness.summary.rows[expected.fixed] = Some(QueryDatum {
                        guard: block,
                        index,
                        extent,
                    });
                    expected.fixed += 1;
                    witness.summary.completed = expected.fixed;
                    witness.summary.initialized = true;
                    cuts.checkpoint();
                }
                GenuinePrefixEventV1::Other | GenuinePrefixEventV1::LiteralSkip { .. } => {}
                _ => unreachable!("classifier cannot return computed/refused query data"),
            }
        }
        witness.stopped_at = None;
        Ok(*expected)
    }));
    if outcome.is_err() {
        expected.stop = GenuinePrefixStopV1::CheckpointUnwind;
    }
    capture_live(&owner, witness);
    observation.before = Some(owner_content(&owner)); // Original read 1 of 3.
    let before = owner.snapshot();
    witness.before = Some(before);
    reserved.install(retire(owner));
    witness.installed = true;
    witness.after = slot.as_ref().map(|payload| payload.snapshot(before.phase));
    observation.after = slot
        .as_ref()
        .map(|payload| retained_content(payload, before.phase)); // Read 2.
    *state = QueryState::Terminal;
    match outcome {
        Ok(Ok(data)) => {
            if !observation
                .before
                .as_ref()
                .zip(observation.after.as_ref())
                .is_some_and(|(before, after)| exact_content_matches(before, after))
            {
                expected.stop = GenuinePrefixStopV1::CoverageLimit;
                return Err(Error::Unsupported(
                    "genuine original content is not completely observable",
                ));
            }
            Ok(data)
        }
        Ok(Err(error)) => Err(error),
        Err(payload) => resume_unwind(payload),
    }
}

// Shared READ-ONLY actual-side copier entry points. None constructs expected data.
pub(in crate::production_ranked_projection_v1) fn genuine_live_content_parts_v1(
    phase: u8,
    checked: &[Vec<usize>],
    dominance: Option<&AssertionCacheV1<'_>>,
    zero: Option<&AssertionCacheV1<'_>>,
    owned_side: bool,
) -> ContentResult<ProofContent> {
    if owned_side {
        return Err(ContentRefusal::OwnedSide);
    }
    Ok(ProofContent {
        phase,
        checked: checked_content(checked)?,
        dominance: optional_live(dominance)?,
        zero: optional_live(zero)?,
    })
}
pub(in crate::production_ranked_projection_v1) fn genuine_retired_content_parts_v1(
    phase: u8,
    checked: &[Vec<usize>],
    dominance: Option<&RetiredAssertionCacheV1>,
    zero: Option<&RetiredAssertionCacheV1>,
    owned_side: bool,
) -> ContentResult<ProofContent> {
    if owned_side {
        return Err(ContentRefusal::OwnedSide);
    }
    Ok(ProofContent {
        phase,
        checked: checked_content(checked)?,
        dominance: optional_retired(dominance)?,
        zero: optional_retired(zero)?,
    })
}

// New source-prefix vertices only. B2/Unit A source rows are a SEPARATE debit.
type GenuinePrefixCatch = (
    &'static QuerySource<'static, 'static>,
    &'static mut OriginalFixedOracleOwnerV1<'static, 'static, 'static>,
    &'static mut &'static mut GenuinePrefixExpectedV1,
    &'static mut &'static mut QueryWitness,
    &'static mut &'static mut Cuts,
    &'static mut &'static mut QueryCuts,
);
const PREFIX_ROWS: usize = 22;
fn genuine_prefix_rows_v1() -> R<[usize; PREFIX_ROWS]> {
    Ok([
        frame::<GenuinePrefixExpectedV1>(size_of::<(
            &NominalRootCfgSourceV1<'static>, &mut Prep<'static, 'static>,
            &mut QueryState, &mut Option<RetiredOriginalFixedOracleV1>,
            &mut Cuts, &mut QueryCuts, &mut QueryWitness, &mut ContentWitness,
            &mut GenuinePrefixExpectedV1, usize, QuerySource<'static, 'static>,
            EmptySlot<'static>, OriginalFixedOracleOwnerV1<'static, 'static, 'static>,
            std::thread::Result<R<GenuinePrefixExpectedV1>>, Snapshot,
            R<GenuinePrefixExpectedV1>, GenuinePrefixExpectedV1, Error, Panic,
        )>())?,
        frame::<R<GenuinePrefixExpectedV1>>(size_of::<(
            GenuinePrefixCatch, AssertUnwindSafe<GenuinePrefixCatch>,
            std::thread::Result<R<GenuinePrefixExpectedV1>>, R<GenuinePrefixExpectedV1>, Panic,
        )>())?,
        frame::<()>(size_of::<(
            std::iter::Enumerate<std::slice::Iter<'static, SemanticBasicBlockV1>>,
            Option<(usize, &SemanticBasicBlockV1)>, usize, &SemanticBasicBlockV1,
            GenuinePrefixEventV1, Option<usize>, (GenuinePrefixEventV1, Option<usize>),
            GenuinePrefixRowV1, &mut GenuinePrefixRowV1, usize, Option<usize>,
            R<FixedGuardDataV1>, FixedGuardDataV1, SemanticLocalIdV1, u64,
            QueryDatum, Option<QueryDatum>, Error,
        )>())?,
        frame::<(GenuinePrefixEventV1, Option<usize>)>(size_of::<(
            &SemanticTerminatorKindV1, &[Option<u64>], &bool, &SemanticUnwindActionV1,
            &SemanticOperandV1, &SemanticOperandV1, &SemanticControlFlowEdgeV1,
            Option<usize>, Option<u64>, Option<u64>, u64, u64, bool,
            GenuinePrefixEventV1,
        )>())?,
        frame::<OriginalConstantDefinition>(size_of::<(
            &SemanticOperandV1, &SemanticConstantV1, &SemanticConstantValueV1,
            &SemanticScalarValueV1, &SemanticPlaceV1, u128,
            std::result::Result<u64, std::num::TryFromIntError>,
            std::result::Result<OriginalConstantDefinition, std::num::TryFromIntError>,
            OriginalConstantDefinition, u64, SemanticLocalIdV1,
        )>())?,
        frame::<Option<u64>>(size_of::<(
            &SemanticOperandV1, &[Option<u64>], OriginalConstantDefinition,
            SemanticLocalIdV1, usize, Option<&Option<u64>>, Option<Option<u64>>,
            Option<u64>, u64,
        )>())?,
        frame::<GenuinePrefixExpectedV1>(size_of::<(
            Option<GenuineSourceStampV1>, [GenuinePrefixRowV1; GENUINE_PREFIX_CAP],
            GenuinePrefixRowV1, GenuinePrefixExpectedV1, usize, usize, GenuinePrefixStopV1,
        )>())?,
        frame::<GenuineErrorDataV1>(size_of::<(
            &Error, &&'static str, &Resource, Resource, GenuineErrorDataV1,
        )>())?,
        frame::<GenuineSourceStampV1>(size_of::<(
            &NominalRootCfgSourceV1<'static>, &SemanticFunctionDeclV1,
            &[SemanticTypeDeclV1], &ProjectedLoopCfgV1, &Rich<'static>,
            GenuineSourceStampV1, usize, usize, usize, usize, usize,
        )>())?,
        frame::<QuerySource<'static, 'static>>(size_of::<(
            &NominalRootCfgSourceV1<'static>, &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1, &ProjectedLoopCfgV1, &Rich<'static>,
            QuerySource<'static, 'static>,
        )>())?,
        // source-table/getter return vertices used by stamp and input construction
        frame::<&Rich<'static>>(size_of::<(
            &NominalRootCfgSourceV1<'static>,
            &crate::production_ranked_projection_v1::bf16_nominal_source_preparation_v1::NominalRootSourceTablesV1<'static>,
            &Rich<'static>, &SemanticFunctionDeclV1, &[SemanticTypeDeclV1],
            &ProjectedLoopCfgV1,
        )>())?,
        frame::<&[SemanticBasicBlockV1]>(size_of::<(
            &SemanticFunctionDeclV1, &[SemanticBasicBlockV1], &SemanticBasicBlockV1,
            &SemanticTerminatorV1, &SemanticTerminatorKindV1, &Rich<'static>, &[Option<u64>],
        )>())?,
        frame::<usize>(size_of::<(
            &mut Prep<'static, 'static>, bool, usize, usize, usize,
            R<usize>, R<usize>, R<()>, Option<usize>, Error,
        )>())?,
        frame::<bool>(size_of::<(
            &GenuinePrefixExpectedV1, GenuinePrefixExpectedV1, &QueryWitness,
            QueryWitness, &ContentWitness, ContentWitness, &QueryState,
            QueryState, &QueryCuts, &Option<RetiredOriginalFixedOracleV1>, bool,
        )>())?,
        frame::<Option<ContentResult<ProofContent>>>(size_of::<(
            &mut ContentWitness, ContentResult<ProofContent>, Option<ContentResult<ProofContent>>,
            Option<&RetiredOriginalFixedOracleV1>, &RetiredOriginalFixedOracleV1,
            &Snapshot, u8, Option<Snapshot>, Snapshot,
        )>())?,
        frame::<bool>(size_of::<(
            &Option<ContentResult<ProofContent>>, Option<&ContentResult<ProofContent>>,
            Option<&ContentResult<ProofContent>>,
            Option<(&ContentResult<ProofContent>, &ContentResult<ProofContent>)>,
            (&ContentResult<ProofContent>, &ContentResult<ProofContent>), bool,
        )>())?,
        frame::<()>(size_of::<(
            &mut GenuinePrefixExpectedV1, &mut QueryState, GenuinePrefixStopV1,
            QueryState, &std::thread::Result<R<GenuinePrefixExpectedV1>>, bool,
        )>())?,
        frame::<GenuinePrefixExpectedV1>(size_of::<(
            R<GenuinePrefixExpectedV1>, GenuinePrefixExpectedV1, Error, Panic,
            &mut GenuinePrefixExpectedV1, GenuinePrefixStopV1,
        )>())?,
        frame::<[usize; PREFIX_ROWS]>(size_of::<(
            [usize; PREFIX_ROWS], R<[usize; PREFIX_ROWS]>, &[usize],
        )>())?,
        frame::<usize>(size_of::<(
            &[usize], std::slice::Iter<'static, usize>, Option<&usize>,
            usize, &usize, Option<usize>, R<usize>, Error,
        )>())?,
        frame::<usize>(size_of::<(
            usize, usize, usize, Option<usize>, Option<usize>, R<usize>, Error, Resource,
        )>())?,
        frame::<usize>(size_of::<(
            [usize; PREFIX_ROWS], R<[usize; PREFIX_ROWS]>, &[usize], usize, R<usize>,
        )>())?,
    ])
}
pub(in crate::production_ranked_projection_v1) fn genuine_prefix_frame_v1() -> R<usize> {
    sum(&genuine_prefix_rows_v1()?)
}
fn genuine_prefix_work_v1() -> R<usize> {
    // Complete fixed row policy: three classifier probes plus row/copy/equality
    // byte-policy units. This is not a machine-instruction measurement.
    3usize
        .checked_mul(GENUINE_PREFIX_CAP)
        .and_then(|n| n.checked_add(size_of::<GenuinePrefixExpectedV1>()))
        .ok_or_else(assertion_resource_overflow_v1)
}
const PART_ROWS: usize = 5;
fn genuine_content_parts_rows_v1() -> R<[usize; PART_ROWS]> {
    Ok([
        frame::<ContentResult<ProofContent>>(size_of::<(
            u8,
            &[Vec<usize>],
            Option<&AssertionCacheV1<'static>>,
            Option<&AssertionCacheV1<'static>>,
            bool,
            CheckedContent,
            Option<CacheContent>,
            Option<CacheContent>,
            ContentRefusal,
            ContentResult<CheckedContent>,
            ContentResult<Option<CacheContent>>,
            ContentResult<Option<CacheContent>>,
            ProofContent,
        )>())?,
        frame::<ContentResult<ProofContent>>(size_of::<(
            u8,
            &[Vec<usize>],
            Option<&RetiredAssertionCacheV1>,
            Option<&RetiredAssertionCacheV1>,
            bool,
            CheckedContent,
            Option<CacheContent>,
            Option<CacheContent>,
            ContentRefusal,
            ContentResult<CheckedContent>,
            ContentResult<Option<CacheContent>>,
            ContentResult<Option<CacheContent>>,
            ProofContent,
        )>())?,
        frame::<[usize; PART_ROWS]>(size_of::<(
            [usize; PART_ROWS],
            R<[usize; PART_ROWS]>,
            &[usize],
        )>())?,
        frame::<usize>(size_of::<(
            usize,
            usize,
            usize,
            usize,
            Option<usize>,
            R<usize>,
            Error,
            [usize; CONTENT_ROWS],
            R<[usize; CONTENT_ROWS]>,
            [usize; PART_ROWS],
            R<[usize; PART_ROWS]>,
            &[usize],
        )>())?,
        frame::<usize>(size_of::<(
            &[usize],
            std::slice::Iter<'static, usize>,
            Option<&usize>,
            &usize,
            usize,
            Option<usize>,
            R<usize>,
            Error,
            Resource,
        )>())?,
    ])
}
pub(in crate::production_ranked_projection_v1) fn genuine_candidate_content_frame_v1() -> R<usize> {
    // Reserve the complete named B2 copier roster conservatively, including
    // original-side wrapper rows not reached by these actual-side adapters.
    // This is explicitly over-reserved source policy, not anonymous slack.
    let cache = cache_content_frame()?
        .checked_mul(2)
        .ok_or_else(assertion_resource_overflow_v1)?;
    let pass = sum(&content_rows()?)?
        .checked_add(sum(&genuine_content_parts_rows_v1()?)?)
        .and_then(|n| n.checked_add(cache))
        .ok_or_else(assertion_resource_overflow_v1)?;
    pass.checked_mul(3)
        .ok_or_else(assertion_resource_overflow_v1)
}
pub(in crate::production_ranked_projection_v1) fn genuine_candidate_content_work_v1() -> R<usize> {
    genuine_candidate_content_frame_v1()?
        .checked_add(content_scan_work()?)
        .ok_or_else(assertion_resource_overflow_v1)
}

#[test]
fn genuine_prefix_empty_data_has_no_synthetic_fixed_or_source_authority() {
    let empty = GenuinePrefixExpectedV1::default();
    assert_eq!((empty.len, empty.fixed, empty.source), (0, 0, None));
    assert_eq!(empty.stop, GenuinePrefixStopV1::End);
    assert!(empty.rows.iter().all(|row| *row == EMPTY_ROW));
}
#[test]
fn genuine_prefix_error_data_preserves_closed_variant_and_message() {
    assert_eq!(
        genuine_error_data_v1(&Error::Incomplete("first")),
        GenuineErrorDataV1::Incomplete("first")
    );
    assert_ne!(
        genuine_error_data_v1(&Error::Incomplete("first")),
        genuine_error_data_v1(&Error::Unsupported("first"))
    );
    assert_ne!(
        genuine_error_data_v1(&Error::Incomplete("first")),
        genuine_error_data_v1(&Error::Incomplete("second"))
    );
}
#[test]
fn genuine_prefix_source_header_and_three_pass_content_amount_are_explicit() {
    assert_eq!(genuine_prefix_rows_v1().unwrap().len(), PREFIX_ROWS);
    assert_eq!(
        genuine_prefix_frame_v1().unwrap(),
        genuine_prefix_rows_v1().unwrap().iter().sum::<usize>()
    );
    assert_eq!(
        genuine_candidate_content_frame_v1().unwrap(),
        3 * (content_rows().unwrap().iter().sum::<usize>()
            + genuine_content_parts_rows_v1()
                .unwrap()
                .iter()
                .sum::<usize>()
            + 2 * cache_content_frame().unwrap())
    );
    assert!(sum(&[usize::MAX, 1]).is_err());
}

#[test]
fn genuine_prefix_actual_partial_admission_keeps_only_accepted_credits_until_cleanup() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    let prefix_frame = genuine_prefix_frame_v1().unwrap();
    let prefix_work = prefix_frame + genuine_prefix_work_v1().unwrap();
    let total_frame = prefix_frame + content_added_frame().unwrap();
    let total_work = prefix_work + content_added_work().unwrap();
    for (work_limit, storage_limit, failure) in [
        (total_work, total_frame, 0),
        (total_work - 1, total_frame, 1),
        (total_work, total_frame - 1, 2),
    ] {
        let mut ledger = Work::new(work_limit);
        let mut budget = Budget::new(&mut ledger, storage_limit);
        let mut owned = 0;
        let result = admit_genuine_prefix(&mut Prep::new(&mut budget, &mut owned));
        if failure == 0 {
            assert_eq!(result.unwrap(), total_frame);
            assert_eq!(
                (owned, budget.storage(), budget.work()),
                (total_frame, total_frame, total_work)
            );
        } else {
            assert!(result.is_err());
            // Prefix was accepted; failed B2 work/storage was not credited.
            assert_eq!((owned, budget.storage()), (prefix_frame, prefix_frame));
            assert_eq!(budget.failed_work().is_some(), failure == 1);
            assert_eq!(budget.failed_storage().is_some(), failure == 2);
            if failure == 2 {
                assert_eq!(budget.work(), total_work);
            }
        }
        let denial = (budget.failed_work(), budget.failed_storage());
        // No owner/source/callback exists in this debit-only component control.
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
        assert_eq!((budget.failed_work(), budget.failed_storage()), denial);
    }
}
