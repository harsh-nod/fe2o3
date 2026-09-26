// Source CFG, sparse index and definition census relocated from the backend.

/// Numeric recipe inventory, not assertion success evidence.
pub struct SemanticAssertionDefinitionInventoryV1 {
    counts: Vec<u8>,
    blocks: Vec<Vec<usize>>,
    assignments: Vec<Option<ScalarAssignmentSiteV1>>,
    address_escaped: Vec<bool>,
}
impl SemanticAssertionDefinitionInventoryV1 {
    pub fn into_parts(
        self,
    ) -> (
        Vec<u8>,
        Vec<Vec<usize>>,
        Vec<Option<ScalarAssignmentSiteV1>>,
        Vec<bool>,
    ) {
        (
            self.counts,
            self.blocks,
            self.assignments,
            self.address_escaped,
        )
    }
}
/// Source-derived graph data. No constructor accepts arbitrary graph rows.
pub struct SemanticAssertionCfgV1 {
    successors: Vec<Vec<usize>>,
    predecessors: Vec<Vec<usize>>,
    reachable: Vec<bool>,
    entry: usize,
}
impl SemanticAssertionCfgV1 {
    pub fn successors(&self) -> &[Vec<usize>] {
        &self.successors
    }
    pub fn predecessors(&self) -> &[Vec<usize>] {
        &self.predecessors
    }
    pub fn reachable(&self) -> &[bool] {
        &self.reachable
    }
    pub const fn entry(&self) -> usize {
        self.entry
    }
    pub fn into_parts(self) -> (Vec<Vec<usize>>, Vec<Vec<usize>>, Vec<bool>, usize) {
        (
            self.successors,
            self.predecessors,
            self.reachable,
            self.entry,
        )
    }
}
pub fn semantic_assertion_cfg_metered_v1<M: SemanticAssertionMeterV1>(
    function: &SemanticFunctionDeclV1,
    limits: SemanticAssertionLimitsV1,
    meter: &mut M,
) -> MR<SemanticAssertionCfgV1, M> {
    cfg_graph(function, &mut Budget::new(limits)?.metered(meter))
}
pub fn semantic_assertion_inventory_metered_v1<M: SemanticAssertionMeterV1>(
    function: &SemanticFunctionDeclV1,
    limits: SemanticAssertionLimitsV1,
    meter: &mut M,
) -> MR<SemanticAssertionDefinitionInventoryV1, M> {
    definition_inventory(function, &mut Budget::new(limits)?.metered(meter))
}

/// Runs the existing CFG helper with its additional live frame prepaid.
///
/// The body already pays the graph header and three temporary vector headers.
/// This entrance adds only the private budget, borrowed meter, and result frame.
/// All credits remain with the enclosing caller scope, including on refusal.
pub fn semantic_assertion_cfg_live_v18<M: SemanticAssertionMeterV1>(
    function: &SemanticFunctionDeclV1,
    limits: SemanticAssertionLimitsV1,
    meter: &mut M,
) -> MR<SemanticAssertionCfgV1, M> {
    let bytes = assertion_live_frame_headers_v18::<SemanticAssertionCfgV1, M>()?;
    meter.reserve_storage(bytes).map_err(SemanticAssertionMeteredErrorV1::Meter)?;
    semantic_assertion_cfg_metered_v1(function,limits,meter)
}

/// Runs the same definition inventory with its actual live headers prepaid.
///
/// Unlike the CFG helper, the inventory body pays collection payloads only.
/// This entrance additionally pays its report and per-block temporary vector
/// headers. It does not authorize output correspondence or refund any credit.
pub fn semantic_assertion_inventory_live_v18<M: SemanticAssertionMeterV1>(
    function: &SemanticFunctionDeclV1,
    limits: SemanticAssertionLimitsV1,
    meter: &mut M,
) -> MR<SemanticAssertionDefinitionInventoryV1, M> {
    let bytes = assertion_inventory_live_headers_v18::<M>()?;
    meter.reserve_storage(bytes).map_err(SemanticAssertionMeteredErrorV1::Meter)?;
    semantic_assertion_inventory_metered_v1(function,limits,meter)
}

fn assertion_live_frame_headers_v18<T, M: SemanticAssertionMeterV1>()
    -> Result<usize, SemanticAssertionErrorV1>
{
    [std::mem::size_of::<Budget>(),std::mem::size_of::<Metered<'_, '_, M>>(),
        std::mem::size_of::<MR<T,M>>()]
        .into_iter().try_fold(0usize,|bytes,header| bytes.checked_add(header)
            .ok_or(SemanticAssertionErrorV1::Arithmetic))
}

fn assertion_inventory_live_headers_v18<M: SemanticAssertionMeterV1>()
    -> Result<usize, SemanticAssertionErrorV1>
{
    assertion_live_frame_headers_v18::<SemanticAssertionDefinitionInventoryV1,M>()?
        .checked_add(std::mem::size_of::<SemanticAssertionDefinitionInventoryV1>())
        .and_then(|bytes|bytes.checked_add(std::mem::size_of::<Vec<usize>>()))
        .ok_or(SemanticAssertionErrorV1::Arithmetic)
}

impl<'a> SemanticAssertionAnalysisV1<'a> {
    /// Constructs the same analysis with the additional live temporary frames prepaid.
    ///
    /// The original constructor already pays its own complete header (including
    /// its owned budget), the CFG header/scratch and collection payloads. This
    /// entrance pays only its result, borrowed meter and temporary definition
    /// inventory headers. It does not create source/output correspondence.
    pub fn new_live_v18<M: SemanticAssertionMeterV1>(
        types: &'a [SemanticTypeDeclV1],
        function: &'a SemanticFunctionDeclV1,
        limits: SemanticAssertionLimitsV1,
        meter: &mut M,
    ) -> MR<Self, M> {
        let bytes = assertion_analysis_live_headers_v18::<M>()?;
        meter.reserve_storage(bytes).map_err(SemanticAssertionMeteredErrorV1::Meter)?;
        Self::new_metered(types, function, limits, meter)
    }

    /// Borrows the existing recipe-query engine after paying its actual frames.
    ///
    /// Query facts remain bound to this immutable analysis; this method does
    /// not settle credits or turn a recipe fact into executable correspondence.
    pub fn recipe_queries_live_v18<'q, M: SemanticAssertionMeterV1>(
        &'q mut self,
        meter: &'q mut M,
    ) -> MR<SemanticAssertionRecipeQueriesV1<'q, 'a, M>, M> {
        let bytes = assertion_query_live_headers_v18::<M>()?
            .checked_add(size_of::<MR<SemanticAssertionRecipeQueriesV1<'q, 'a, M>, M>>())
            .ok_or(SemanticAssertionErrorV1::Arithmetic)?;
        meter.reserve_storage(bytes).map_err(SemanticAssertionMeteredErrorV1::Meter)?;
        Ok(self.recipe_queries_v1(meter))
    }

    /// Runs the unchanged legacy assertion classification with live query/result headers.
    ///
    /// Classification is recipe bookkeeping only. In particular, deferred bounds
    /// checks are not proofs, and all output/currentness obligations remain.
    pub fn legacy_recipe_assertions_live_v18<M: SemanticAssertionMeterV1>(
        &mut self,
        defined_callable: bool,
        meter: &mut M,
    ) -> MR<Vec<SemanticAssertionLegacyDispositionV1>, M> {
        let bytes = assertion_query_live_headers_v18::<M>()?
            .checked_add(size_of::<Vec<SemanticAssertionLegacyDispositionV1>>())
            .and_then(|bytes| bytes.checked_add(size_of::<MR<Vec<SemanticAssertionLegacyDispositionV1>, M>>()))
            .ok_or(SemanticAssertionErrorV1::Arithmetic)?;
        meter.reserve_storage(bytes).map_err(SemanticAssertionMeteredErrorV1::Meter)?;
        self.legacy_recipe_assertions_v1(defined_callable, meter)
    }
}

fn assertion_analysis_live_headers_v18<M: SemanticAssertionMeterV1>()
    -> Result<usize, SemanticAssertionErrorV1>
{
    [size_of::<MR<SemanticAssertionAnalysisV1<'_>, M>>(),
        size_of::<Metered<'_, '_, M>>(), size_of::<SemanticAssertionDefinitionInventoryV1>(),
        size_of::<Vec<usize>>()]
        .into_iter().try_fold(0usize, |bytes, header| bytes.checked_add(header)
            .ok_or(SemanticAssertionErrorV1::Arithmetic))
}

fn assertion_query_live_headers_v18<M: SemanticAssertionMeterV1>()
    -> Result<usize, SemanticAssertionErrorV1>
{
    size_of::<SemanticAssertionRecipeQueriesV1<'_, '_, M>>()
        .checked_add(size_of::<Metered<'_, '_, M>>())
        .ok_or(SemanticAssertionErrorV1::Arithmetic)
}

#[cfg(test)]
mod live_header_tests_v18 {
    use super::*;
    use crate::semantic_mir_v1::*;
    use std::mem::size_of;

    #[derive(Clone,Copy,Debug,Eq,PartialEq)]
    enum Refusal { Work(usize),Storage(usize) }
    struct Meter { work:usize,storage:usize,work_limit:usize,storage_limit:usize,first:Option<Refusal> }
    impl Meter {
        fn new(work_limit:usize,storage_limit:usize)->Self {
            Self {work:0,storage:0,work_limit,storage_limit,first:None}
        }
    }
    impl SemanticAssertionMeterV1 for Meter {
        type Error=Refusal;
        fn charge_work(&mut self,amount:usize)->Result<(),Refusal>{
            let next=self.work.checked_add(amount).unwrap_or(usize::MAX);
            if next>self.work_limit {return Err(*self.first.get_or_insert(Refusal::Work(next)));}
            self.work=next;Ok(())
        }
        fn reserve_storage(&mut self,bytes:usize)->Result<(),Refusal>{
            let next=self.storage.checked_add(bytes).unwrap_or(usize::MAX);
            if next>self.storage_limit {return Err(*self.first.get_or_insert(Refusal::Storage(next)));}
            self.storage=next;Ok(())
        }
    }

    fn fixture()->SemanticFunctionDeclV1 {
        let source=SemanticSourceProvenanceV1::unavailable();
        let unit=SemanticTypeIdV1::from_index(0);
        let abi=SemanticFunctionAbiV1::new(SemanticAbiIdentityV1::from_sha256([1;32]),
            SemanticLayoutIdentityV1::from_sha256([2;32]),SemanticCanonAbiV1::Rust,false,false,Vec::new(),
            SemanticAbiValueV1::new(unit,SemanticAbiPassModeV1::Ignore)).unwrap();
        let block=|tag,terminator|SemanticBasicBlockV1::new(SemanticBlockIdentityV1::from_sha256([tag;32]),source,Vec::new(),
            SemanticTerminatorV1::new(source,terminator)).unwrap();
        SemanticFunctionDeclV1::new(SemanticFunctionIdentityV1::from_sha256([3;32]),SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256([4;32]),SemanticMonomorphizationIdentityV1::from_sha256([5;32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([6;32]),SemanticConstGenericArgumentsIdentityV1::from_sha256([7;32]),
            source,abi,vec![SemanticLocalDeclV1::new(SemanticLocalIdentityV1::from_sha256([8;32]),unit,SemanticLocalRoleV1::Return,source)],
            SemanticBlockIdV1::from_index(0),vec![block(9,SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::Goto,SemanticBlockIdV1::from_index(1)))),block(10,SemanticTerminatorKindV1::Return)]).unwrap()
    }

    #[test]
    fn transparent_place_live_walk_stops_at_the_original_projection_and_keeps_refusals() {
        let ty = SemanticTypeIdV1::from_index(0);
        let make = |kind| SemanticProjectionV1::new(kind, ty).unwrap();
        let valid = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0),
            vec![make(SemanticProjectionKindV1::OpaqueCast), make(SemanticProjectionKindV1::Field(0))], ty).unwrap();
        let invalid = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0),
            vec![make(SemanticProjectionKindV1::Dereference), make(SemanticProjectionKindV1::OpaqueCast)], ty).unwrap();
        let bytes = size_of::<Result<Option<&SemanticPlaceV1>, Refusal>>()
            + size_of::<std::slice::Iter<'_, SemanticProjectionV1>>() + size_of::<&mut Meter>();
        for (work, storage) in [(2, bytes), (1, bytes), (2, bytes - 1)] {
            let mut meter = Meter::new(work, storage);
            let result = transparent_place_with_meter_v18(&valid, &mut meter);
            if work == 2 && storage == bytes {
                assert!(std::ptr::eq(result.unwrap().unwrap(), &valid));
                assert_eq!(result.unwrap(), transparent_place(&valid));
                assert_eq!((meter.work, meter.storage), (2, bytes));
            } else {
                let expected = if storage < bytes { Refusal::Storage(bytes) } else { Refusal::Work(2) };
                assert_eq!(result, Err(expected));
                assert_eq!(meter.first, Some(expected));
            }
        }
        let mut meter = Meter::new(1, bytes);
        assert_eq!(transparent_place_with_meter_v18(&invalid, &mut meter).unwrap(), None);
        assert_eq!(meter.work, 1);
        assert_eq!(transparent_place(&invalid), None);
    }

    fn check<T:std::fmt::Debug+PartialEq>(
        old:impl Fn(&mut Meter)->Result<T,SemanticAssertionMeteredErrorV1<Refusal>>,
        live:impl Fn(&mut Meter)->Result<T,SemanticAssertionMeteredErrorV1<Refusal>>,
        prefix:usize,
    ) {
        let mut original=Meter::new(usize::MAX,usize::MAX);
        let expected=old(&mut original).unwrap();
        let mut source=Meter::new(usize::MAX,usize::MAX);
        assert_eq!(live(&mut source).unwrap(),expected);
        assert_eq!(source.work,original.work);
        assert_eq!(source.storage,original.storage+prefix);
        let mut exact=Meter::new(source.work,source.storage);
        assert_eq!(live(&mut exact).unwrap(),expected);
        for (work,storage) in [(source.work-1,source.storage),(source.work,source.storage-1)] {
            let mut short=Meter::new(work,storage);
            assert_eq!(live(&mut short).unwrap_err(),SemanticAssertionMeteredErrorV1::Meter(short.first.unwrap()));
        }
        let mut prefix_short=Meter::new(usize::MAX,prefix-1);
        assert_eq!(live(&mut prefix_short).unwrap_err(),SemanticAssertionMeteredErrorV1::Meter(Refusal::Storage(prefix)));
        assert_eq!((prefix_short.work,prefix_short.storage),(0,0));
        let mut prefix_exact=Meter::new(0,prefix);
        assert!(matches!(live(&mut prefix_exact),Err(SemanticAssertionMeteredErrorV1::Meter(Refusal::Work(_)))));
        assert_eq!((prefix_exact.work,prefix_exact.storage),(0,prefix));
    }

    #[test]
    fn live_cfg_prefix_excludes_preexisting_graph_and_three_scratch_headers() {
        let function=fixture();
        let limits=SemanticAssertionLimitsV1::new(usize::MAX,usize::MAX);
        let prefix=size_of::<Budget>()+size_of::<Metered<'_,'_,Meter>>()
            +size_of::<Result<SemanticAssertionCfgV1,SemanticAssertionMeteredErrorV1<Refusal>>>();
        assert_eq!(assertion_live_frame_headers_v18::<SemanticAssertionCfgV1,Meter>().unwrap(),prefix);
        check(|meter|semantic_assertion_cfg_metered_v1(&function,limits,meter).map(SemanticAssertionCfgV1::into_parts),
            |meter|semantic_assertion_cfg_live_v18(&function,limits,meter).map(SemanticAssertionCfgV1::into_parts),prefix);
    }

    #[test]
    fn live_inventory_prefix_adds_only_actual_missing_report_and_temporary_headers() {
        let function=fixture();
        let limits=SemanticAssertionLimitsV1::new(usize::MAX,usize::MAX);
        let prefix=size_of::<Budget>()+size_of::<Metered<'_,'_,Meter>>()
            +size_of::<Result<SemanticAssertionDefinitionInventoryV1,SemanticAssertionMeteredErrorV1<Refusal>>>()
            +size_of::<SemanticAssertionDefinitionInventoryV1>()+size_of::<Vec<usize>>();
        assert_eq!(assertion_inventory_live_headers_v18::<Meter>().unwrap(),prefix);
        check(|meter|semantic_assertion_inventory_metered_v1(&function,limits,meter).map(SemanticAssertionDefinitionInventoryV1::into_parts),
            |meter|semantic_assertion_inventory_live_v18(&function,limits,meter).map(SemanticAssertionDefinitionInventoryV1::into_parts),prefix);
    }

    #[test]
    fn live_analysis_prefix_does_not_repay_core_budget_or_cfg_headers() {
        let function = fixture();
        let limits = SemanticAssertionLimitsV1::new(usize::MAX, usize::MAX);
        let prefix = size_of::<Result<SemanticAssertionAnalysisV1<'_>, SemanticAssertionMeteredErrorV1<Refusal>>>()
            + size_of::<Metered<'_, '_, Meter>>() + size_of::<SemanticAssertionDefinitionInventoryV1>()
            + size_of::<Vec<usize>>();
        assert_eq!(assertion_analysis_live_headers_v18::<Meter>().unwrap(), prefix);
        let mut old = Meter::new(usize::MAX, usize::MAX);
        let original = SemanticAssertionAnalysisV1::new_metered(&[], &function, limits, &mut old).unwrap();
        let mut live = Meter::new(usize::MAX, usize::MAX);
        let current = SemanticAssertionAnalysisV1::new_live_v18(&[], &function, limits, &mut live).unwrap();
        assert_eq!(current.definition_counts(), original.definition_counts());
        assert_eq!(current.assignments(), original.assignments());
        assert_eq!(current.address_escaped(), original.address_escaped());
        assert_eq!((live.work, live.storage), (old.work, old.storage + prefix));
        for (work, storage) in [(live.work, live.storage), (live.work - 1, live.storage), (live.work, live.storage - 1)] {
            let mut meter = Meter::new(work, storage);
            let result = SemanticAssertionAnalysisV1::new_live_v18(&[], &function, limits, &mut meter);
            assert_eq!(result.is_ok(), (work, storage) == (live.work, live.storage));
        }
        let mut short = Meter::new(usize::MAX, prefix - 1);
        assert!(matches!(SemanticAssertionAnalysisV1::new_live_v18(&[], &function, limits, &mut short),
            Err(SemanticAssertionMeteredErrorV1::Meter(Refusal::Storage(bytes))) if bytes == prefix));
        assert_eq!((short.work, short.storage), (0, 0));
        let mut exact = Meter::new(usize::MAX, prefix);
        assert!(matches!(SemanticAssertionAnalysisV1::new_live_v18(&[], &function, limits, &mut exact),
            Err(SemanticAssertionMeteredErrorV1::Meter(Refusal::Storage(bytes)))
                if bytes == prefix + size_of::<SemanticAssertionAnalysisV1<'_>>()));
        assert_eq!((exact.work, exact.storage), (0, prefix));
    }

    #[test]
    fn live_recipe_frames_preserve_classification_and_original_first_refusal() {
        let function = fixture();
        let limits = SemanticAssertionLimitsV1::new(usize::MAX, usize::MAX);
        let prepare = || SemanticAssertionAnalysisV1::new_metered(&[], &function, limits,
            &mut Meter::new(usize::MAX, usize::MAX)).unwrap();
        let prefix = size_of::<SemanticAssertionRecipeQueriesV1<'_, '_, Meter>>()
            + size_of::<Metered<'_, '_, Meter>>()
            + size_of::<Vec<SemanticAssertionLegacyDispositionV1>>()
            + size_of::<Result<Vec<SemanticAssertionLegacyDispositionV1>, SemanticAssertionMeteredErrorV1<Refusal>>>();
        let mut old = Meter::new(usize::MAX, usize::MAX);
        let expected = prepare().legacy_recipe_assertions_v1(false, &mut old).unwrap();
        let mut live = Meter::new(usize::MAX, usize::MAX);
        assert_eq!(prepare().legacy_recipe_assertions_live_v18(false, &mut live).unwrap(), expected);
        assert_eq!((live.work, live.storage), (old.work, old.storage + prefix));
        for (work, storage) in [(live.work, live.storage), (live.work - 1, live.storage), (live.work, live.storage - 1)] {
            let mut meter = Meter::new(work, storage);
            let result = prepare().legacy_recipe_assertions_live_v18(false, &mut meter);
            assert_eq!(result.is_ok(), (work, storage) == (live.work, live.storage));
        }
        let mut short = Meter::new(0, prefix - 1);
        assert!(matches!(prepare().legacy_recipe_assertions_live_v18(false, &mut short),
            Err(SemanticAssertionMeteredErrorV1::Meter(Refusal::Storage(bytes))) if bytes == prefix));
        assert_eq!((short.work, short.storage), (0, 0));
        let query_prefix = size_of::<SemanticAssertionRecipeQueriesV1<'_, '_, Meter>>()
            + size_of::<Metered<'_, '_, Meter>>()
            + size_of::<Result<SemanticAssertionRecipeQueriesV1<'_, '_, Meter>, SemanticAssertionMeteredErrorV1<Refusal>>>();
        let mut core = prepare();
        let mut exact = Meter::new(0, query_prefix);
        drop(core.recipe_queries_live_v18(&mut exact).unwrap());
        assert_eq!((exact.work, exact.storage), (0, query_prefix));
    }
}

fn for_each_successor<M: SemanticAssertionMeterV1>(
    function: &SemanticFunctionDeclV1,
    kind: &SemanticTerminatorKindV1,
    mut visit: impl FnMut(usize) -> Result<(), ErrorFor<M>>,
) -> Result<(), ErrorFor<M>> {
    let mut checked_target = |target: SemanticBlockIdV1| {
        let target = target.index() as usize;
        if target >= function.blocks().len() {
            return Err(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
                "a semantic CFG edge outside the function during loop analysis",
            )));
        }
        visit(target)
    };
    match kind {
        SemanticTerminatorKindV1::Goto(edge) => checked_target(edge.target())?,
        SemanticTerminatorKindV1::SwitchInt { targets, .. } => {
            for target in targets.values() {
                checked_target(target.edge().target())?;
            }
            let otherwise = targets.otherwise().target().index() as usize;
            if otherwise >= function.blocks().len() {
                return Err(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
                    "a semantic CFG edge outside the function during loop analysis",
                )));
            }
            if !(targets.values().len() == 2
                && targets.values().iter().any(|target| target.value() == 0)
                && targets.values().iter().any(|target| target.value() == 1)
                && switch_fallback_is_empty_unreachable_v1(function, otherwise))
            {
                checked_target(targets.otherwise().target())?;
            }
        }
        SemanticTerminatorKindV1::Call(call) => {
            if let Some(destination) = call.destination() {
                checked_target(destination.edge().target())?;
            }
        }
        SemanticTerminatorKindV1::Assert { target, .. }
        | SemanticTerminatorKindV1::Drop { target, .. } => checked_target(target.target())?,
        SemanticTerminatorKindV1::FalseEdge { .. } => {
            return Err(analysis_error::<M>(SemanticAssertionErrorV1::Incomplete(
                "a false edge before uniform induction CFG normalization",
            )));
        }
        SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::TailCall(_)
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => {}
    }
    Ok(())
}

fn cfg_graph<M: SemanticAssertionMeterV1>(
    function: &SemanticFunctionDeclV1,
    context: &mut Metered<'_, '_, M>,
) -> Result<SemanticAssertionCfgV1, ErrorFor<M>> {
    context.charge(4)?;
    let count = function.blocks().len();
    if count == 0 || count > MAX_SEMANTIC_ASSERTION_BLOCKS_V1 {
        return Err(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
            "semantic CFG exceeds the ranked block limit before loop analysis",
        )));
    }
    context.reserve(sum(
        std::mem::size_of::<SemanticAssertionCfgV1>(),
        std::mem::size_of::<Vec<usize>>()
            .checked_mul(3)
            .ok_or_else(|| analysis_error::<M>(SemanticAssertionErrorV1::Arithmetic))?,
    )?)?;
    let mut successors = context.backing::<Vec<usize>>(count)?;
    let mut predecessors = context.backing::<Vec<usize>>(count)?;
    let mut degree = context.backing::<usize>(count)?;
    context.charge(count)?;
    degree.resize(count, 0);
    let mut edges = 0usize;
    for block in function.blocks() {
        context.charge(2)?;
        let mut capacity = 0usize;
        for_each_successor::<M>(function, block.terminator().kind(), |_| {
            context.charge(2)?;
            capacity = sum(capacity, 1)?;
            Ok(())
        })?;
        let mut row = context.backing::<usize>(capacity)?;
        for_each_successor::<M>(function, block.terminator().kind(), |target| {
            context.charge(2)?;
            if row.len() == row.capacity() {
                return Err(analysis_error::<M>(SemanticAssertionErrorV1::Accounting));
            }
            row.push(target);
            Ok(())
        })?;
        let logarithm = sum(1, usize::BITS as usize - row.len().leading_zeros() as usize)?;
        context.charge(
            row.len()
                .checked_mul(logarithm)
                .ok_or_else(|| analysis_error::<M>(SemanticAssertionErrorV1::Arithmetic))?,
        )?;
        row.sort_unstable();
        row.dedup();
        edges = sum(edges, row.len())?;
        if edges > MAX_SEMANTIC_ASSERTION_EDGES_V1 {
            return Err(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
                "semantic CFG exceeds the ranked edge limit before loop analysis",
            )));
        }
        for &target in &row {
            context.charge(2)?;
            degree[target] = sum(degree[target], 1)?;
        }
        successors.push(row);
    }
    for &size in &degree {
        context.charge(1)?;
        predecessors.push(context.backing::<usize>(size)?);
    }
    for (source, targets) in successors.iter().enumerate() {
        context.charge(1)?;
        for &target in targets {
            context.charge(2)?;
            if predecessors[target].len() == predecessors[target].capacity() {
                return Err(analysis_error::<M>(SemanticAssertionErrorV1::Accounting));
            }
            predecessors[target].push(source);
        }
    }
    let entry = function.entry().index() as usize;
    if entry >= count {
        return Err(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
            "semantic entry block outside the function during loop analysis",
        )));
    }
    let mut reachable = context.backing::<bool>(count)?;
    let mut pending = context.backing::<usize>(count)?;
    context.charge(sum(count, 1)?)?;
    reachable.resize(count, false);
    reachable[entry] = true;
    pending.push(entry);
    while !pending.is_empty() {
        context.charge(2)?;
        let block = pending
            .pop()
            .ok_or_else(|| analysis_error::<M>(SemanticAssertionErrorV1::Accounting))?;
        for &target in &successors[block] {
            context.charge(3)?;
            if !reachable[target] {
                if pending.len() == pending.capacity() {
                    return Err(analysis_error::<M>(SemanticAssertionErrorV1::Accounting));
                }
                reachable[target] = true;
                pending.push(target);
            }
        }
    }
    // Caller owns this graph inside a paid temporary scope. Degree/pending
    // backing also stays charged until both have been destroyed here.
    Ok(SemanticAssertionCfgV1 {
        successors,
        predecessors,
        reachable,
        entry,
    })
}

fn definition_inventory<M: SemanticAssertionMeterV1>(
    function: &SemanticFunctionDeclV1,
    paid: &mut Metered<'_, '_, M>,
) -> Result<SemanticAssertionDefinitionInventoryV1, ErrorFor<M>> {
    let local_count = function.locals().len();
    let mut counts = paid.table(local_count, 0_u8)?;
    let mut assignments = paid.table(local_count, None)?;
    let mut address_escaped = paid.table(local_count, false)?;
    let mut blocks = Vec::new();
    paid.grow(&mut blocks, function.blocks().len())?;
    for (block_index, block) in function.blocks().iter().enumerate() {
        paid.charge(3)?;
        let mut definitions = Vec::new();
        let capacity = block
            .statements()
            .len()
            .checked_mul(2)
            .and_then(|value| value.checked_add(1))
            .ok_or(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
                "assertion proof block-definition capacity overflowed",
            )))?;
        paid.grow(&mut definitions, capacity)?;
        for (statement_index, statement) in block.statements().iter().enumerate() {
            paid.charge(4)?;
            if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
                if assignment.destination().projections().is_empty()
                    && let Some(local) = local_definition_index(assignment.destination())
                {
                    let Some(slot) = assignments.get_mut(local) else {
                        return Err(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
                            "an assertion proof assignment is outside the semantic local table",
                        )));
                    };
                    if slot.is_none() {
                        *slot = Some(ScalarAssignmentSiteV1 {
                            block: block_index,
                            statement: statement_index,
                        });
                    }
                }
                if let Some(slot) = address_escaped_local_index_v1(assignment.value().kind())
                    .and_then(|local| address_escaped.get_mut(local))
                {
                    *slot = true;
                }
            }
            visit_statement_definition_places(statement.kind(), &mut |place| {
                if let Some(local) = local_definition_index(place) {
                    if let Some(slot) = counts.get_mut(local) {
                        *slot = slot.saturating_add(1);
                    }
                    definitions.push(local);
                }
            });
        }
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && let Some(local) = call
                .destination()
                .and_then(|destination| local_definition_index(destination.place()))
        {
            if let Some(slot) = counts.get_mut(local) {
                *slot = slot.saturating_add(1);
            }
            definitions.push(local);
        }
        paid.sort_work(definitions.len(), 1)?;
        definitions.sort_unstable();
        paid.charge(definitions.len())?;
        definitions.dedup();
        blocks.push(definitions);
    }
    paid.charge(assignments.len())?;
    for (local, assignment) in assignments.iter_mut().enumerate() {
        if counts.get(local).copied() != Some(1) {
            *assignment = None;
        }
    }
    Ok(SemanticAssertionDefinitionInventoryV1 {
        counts,
        blocks,
        assignments,
        address_escaped,
    })
}

pub fn switch_fallback_is_empty_unreachable_v1(
    function: &SemanticFunctionDeclV1,
    block: usize,
) -> bool {
    function.blocks().get(block).is_some_and(|block| {
        block.statements().is_empty()
            && matches!(
                block.terminator().kind(),
                SemanticTerminatorKindV1::Unreachable
            )
    })
}

pub fn tuple_field_operand_local_v1(
    operand: &SemanticOperandV1,
    field: u32,
) -> Option<SemanticLocalIdV1> {
    let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
        return None;
    };
    matches!(
        place.projections(),
        [projection] if projection.kind() == SemanticProjectionKindV1::Field(field)
    )
    .then_some(place.local())
}

pub fn same_semantic_operand_value_v1(left: &SemanticOperandV1, right: &SemanticOperandV1) -> bool {
    match (left, right) {
        (SemanticOperandV1::Constant(left), SemanticOperandV1::Constant(right)) => left == right,
        (
            SemanticOperandV1::Copy(left) | SemanticOperandV1::Move(left),
            SemanticOperandV1::Copy(right) | SemanticOperandV1::Move(right),
        ) => left == right,
        _ => false,
    }
}

pub fn visit_statement_definition_places(
    kind: &SemanticStatementKindV1,
    visitor: &mut impl FnMut(&SemanticPlaceV1),
) {
    match kind {
        SemanticStatementKindV1::Assign(assignment) => visitor(assignment.destination()),
        SemanticStatementKindV1::Store(store) => visitor(store.destination()),
        SemanticStatementKindV1::AtomicRmw(atomic) => {
            visitor(atomic.destination());
            visitor(atomic.address());
        }
        SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
            visitor(atomic.destination());
            visitor(atomic.address());
        }
        SemanticStatementKindV1::SetDiscriminant { place, .. }
        | SemanticStatementKindV1::Deinitialize(place) => visitor(place),
        SemanticStatementKindV1::StorageLive(_)
        | SemanticStatementKindV1::StorageDead(_)
        | SemanticStatementKindV1::Assume(_)
        | SemanticStatementKindV1::Nop => {}
    }
}

pub fn local_definition_index(place: &SemanticPlaceV1) -> Option<usize> {
    (!matches!(
        place
            .projections()
            .first()
            .map(|projection| projection.kind()),
        Some(SemanticProjectionKindV1::Dereference)
    ))
    .then_some(place.local().index() as usize)
}

pub fn address_escaped_local_index_v1(kind: &SemanticRvalueKindV1) -> Option<usize> {
    match kind {
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Mutable,
            place,
        }
        | SemanticRvalueKindV1::AddressOf { place, .. } => local_definition_index(place),
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared | SemanticBorrowKindV1::Fake,
            ..
        } => None,
        _ => None,
    }
}

pub fn raw_operand_place(operand: &SemanticOperandV1) -> Option<&SemanticPlaceV1> {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => Some(place),
        SemanticOperandV1::Constant(_) => None,
    }
}

pub fn transparent_operand_place(operand: &SemanticOperandV1) -> Option<&SemanticPlaceV1> {
    transparent_place(raw_operand_place(operand)?)
}

pub fn transparent_place(place: &SemanticPlaceV1) -> Option<&SemanticPlaceV1> {
    transparent_place_core_v18(place, || Ok::<(), std::convert::Infallible>(()))
        .expect("the original transparent-place predicate cannot refuse resources")
}

/// Runs the original transparent-place predicate with a debit before each
/// inspected projection and preserves the exact caller refusal as an error.
pub fn transparent_place_with_meter_v18<'p, M: SemanticAssertionMeterV1>(
    place: &'p SemanticPlaceV1, meter: &mut M,
) -> Result<Option<&'p SemanticPlaceV1>, M::Error> {
    meter.reserve_storage(std::mem::size_of::<Result<Option<&SemanticPlaceV1>, M::Error>>())?;
    meter.reserve_storage(std::mem::size_of::<std::slice::Iter<'_, crate::semantic_mir_v1::SemanticProjectionV1>>())?;
    meter.reserve_storage(std::mem::size_of::<&mut M>())?;
    transparent_place_core_v18(place, || meter.charge_work(1))
}

fn transparent_place_core_v18<E>(place: &SemanticPlaceV1, mut charge: impl FnMut() -> Result<(), E>)
    -> Result<Option<&SemanticPlaceV1>, E>
{
    for projection in place.projections() {
        charge()?;
        if !matches!(projection.kind(), SemanticProjectionKindV1::Field(_)
            | SemanticProjectionKindV1::Downcast(_) | SemanticProjectionKindV1::OpaqueCast
            | SemanticProjectionKindV1::Subtype) { return Ok(None); }
    }
    Ok(Some(place))
}

pub fn simple_operand_local(operand: &SemanticOperandV1) -> Option<SemanticLocalIdV1> {
    raw_operand_place(operand)
        .filter(|place| place.projections().is_empty())
        .map(SemanticPlaceV1::local)
}

/// Sparse source positions only; no expression or cross-function proof cache.
pub struct SemanticStatementDefinitionIndexV1<'a> {
    function: &'a SemanticFunctionDeclV1,
    rows: Vec<(usize, usize, usize)>,
}
impl<'a> SemanticStatementDefinitionIndexV1<'a> {
    pub fn new_metered<M: SemanticAssertionMeterV1>(
        function: &'a SemanticFunctionDeclV1,
        limits: SemanticAssertionLimitsV1,
        meter: &mut M,
    ) -> MR<Self, M> {
        Self::build(function, &mut Budget::new(limits)?.metered(meter))
    }
    fn build<M: SemanticAssertionMeterV1>(
        function: &'a SemanticFunctionDeclV1,
        paid: &mut Metered<'_, '_, M>,
    ) -> MR<Self, M> {
        let mut count = Some(0_usize);
        for block in function.blocks() {
            paid.charge(1)?;
            for statement in block.statements() {
                paid.charge(3)?;
                visit_statement_definition_places(statement.kind(), &mut |place| {
                    if local_definition_index(place).is_some() {
                        count = count.and_then(|n| n.checked_add(1));
                    }
                });
            }
        }
        let count = count.ok_or(SemanticAssertionErrorV1::Arithmetic)?;
        let mut rows = paid.backing(count)?;
        for (block, body) in function.blocks().iter().enumerate() {
            paid.charge(1)?;
            for (statement, value) in body.statements().iter().enumerate() {
                paid.charge(3)?;
                visit_statement_definition_places(value.kind(), &mut |place| {
                    if let Some(local) = local_definition_index(place) {
                        rows.push((local, block, statement));
                    }
                });
            }
        }
        paid.sort_work(rows.len(), 3)?;
        rows.sort_unstable();
        Ok(Self { function, rows })
    }
    pub fn function(&self) -> &'a SemanticFunctionDeclV1 {
        self.function
    }
    pub fn row_capacity(&self) -> usize {
        self.rows.capacity()
    }
    pub fn rows(&self) -> &[(usize, usize, usize)] {
        &self.rows
    }
    fn before(&self, local: usize, block: usize, statement: usize) -> Option<usize> {
        let end = self
            .rows
            .partition_point(|row| *row < (local, block, statement));
        let &(found_local, found_block, found_statement) = self.rows.get(end.checked_sub(1)?)?;
        (found_local == local && found_block == block).then_some(found_statement)
    }
    pub fn before_metered<M: SemanticAssertionMeterV1>(
        &self,
        local: usize,
        block: usize,
        statement: usize,
        meter: &mut M,
    ) -> MR<Option<usize>, M> {
        meter
            .charge_work(3 * (1 + (usize::BITS - self.rows.len().leading_zeros()) as usize))
            .map_err(SemanticAssertionMeteredErrorV1::Meter)?;
        Ok(self.before(local, block, statement))
    }
}

impl<M: SemanticAssertionMeterV1> SemanticAssertionRecipeQueriesV1<'_, '_, M> {
    pub fn quotient_strict_product_upper_bound_v1(
        &mut self,
        numerator: &SemanticOperandV1,
        divisor: &SemanticOperandV1,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<Option<AuthenticatedQuotientStrictBoundV1>, ErrorFor<M>> {
        let (Some(numerator_local), Some(divisor_local)) = (
            simple_operand_local(numerator).map(|local| local.index() as usize),
            simple_operand_local(divisor).map(|local| local.index() as usize),
        ) else {
            return Ok(None);
        };
        if self.core.address_escaped.get(numerator_local).copied() != Some(false)
            || self.core.address_escaped.get(divisor_local).copied() != Some(false)
            || self.core.definition_counts.get(divisor_local).copied() != Some(1)
            || self
                .range_at_operand(divisor, use_site.block, use_site.statement)?
                .is_none_or(|range| range.minimum == 0)
        {
            return Ok(None);
        }

        let block_count = self.core.function.blocks().len();
        let can_reach_use = self.blocks_reaching(use_site.block)?;
        let mut stability_visited = Vec::new();
        self.paid().grow(&mut stability_visited, block_count)?;
        self.paid().charge(block_count)?;
        stability_visited.resize(block_count, 0_usize);
        let mut stability_pending = self.paid().deque(block_count)?;
        let mut stability_generation = 0_usize;
        let mut proven_bound: Option<AuthenticatedQuotientStrictBoundV1> = None;

        for (switch_block, block) in self.core.function.blocks().iter().enumerate() {
            self.charge(1)?;
            let SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } = block.terminator().kind()
            else {
                continue;
            };
            let [zero_target] = targets.values() else {
                continue;
            };
            if zero_target.value() != 0
                || zero_target.edge().role() != SemanticEdgeRoleV1::SwitchValue
                || targets.otherwise().role() != SemanticEdgeRoleV1::SwitchOtherwise
            {
                continue;
            }
            let false_target = zero_target.edge().target().index() as usize;
            let true_target = targets.otherwise().target().index() as usize;
            if false_target == true_target {
                continue;
            }
            let Some(condition_local) = simple_operand_local(discriminant) else {
                continue;
            };
            let condition_local = condition_local.index() as usize;
            if self.core.definition_counts.get(condition_local).copied() != Some(1)
                || self.core.address_escaped.get(condition_local).copied() != Some(false)
            {
                continue;
            }
            let Some(condition_site) = self
                .core
                .assignments
                .get(condition_local)
                .copied()
                .flatten()
            else {
                continue;
            };
            if condition_site.block != switch_block
                || !self.assignment_dominates_use(
                    condition_site,
                    switch_block,
                    block.statements().len(),
                )?
            {
                continue;
            }
            let SemanticStatementKindV1::Assign(condition) =
                block.statements()[condition_site.statement].kind()
            else {
                continue;
            };
            let SemanticRvalueKindV1::Binary {
                operation,
                left,
                right,
            } = condition.value().kind()
            else {
                continue;
            };
            let (tested, bound, success_target) = match operation {
                SemanticBinaryOpV1::LessThan => (left, right, true_target),
                SemanticBinaryOpV1::GreaterOrEqual => (left, right, false_target),
                SemanticBinaryOpV1::GreaterThan => (right, left, true_target),
                SemanticBinaryOpV1::LessOrEqual => (right, left, false_target),
                _ => continue,
            };
            let Some(tested_local) = simple_operand_local(tested) else {
                continue;
            };
            if !self.local_is_value_preserving_alias_of(
                tested_local.index() as usize,
                numerator_local,
                condition_site.block,
                condition_site.statement,
                None,
            )? || self.block_defines_local(switch_block, numerator_local)?
            {
                continue;
            }
            let Some(product) =
                self.authenticated_checked_binary_source_v1(bound, condition_site)?
            else {
                continue;
            };
            if product.checked.operation() != SemanticCheckedBinaryOpV1::Multiply {
                continue;
            }
            let factor = if self.same_operand(product.checked.left(), divisor)? {
                product.checked.right()
            } else if self.same_operand(product.checked.right(), divisor)? {
                product.checked.left()
            } else {
                continue;
            };
            let Some(factor_range) = self.range_at_operand(
                factor,
                product.definition.block,
                product.definition.statement,
            )?
            else {
                continue;
            };
            let Some(candidate) = factor_range.maximum.checked_sub(1) else {
                continue;
            };
            let SemanticTerminatorKindV1::Assert { target, .. } = self.core.function.blocks()
                [product.assertion_block]
                .terminator()
                .kind()
            else {
                continue;
            };
            if target.role() != SemanticEdgeRoleV1::AssertSuccess {
                continue;
            }
            let product_success = target.target().index() as usize;
            stability_generation =
                stability_generation
                    .checked_add(1)
                    .ok_or(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
                        "quotient-bound stability generation overflowed",
                    )))?;
            if !self.local_is_stable_between_edge_and_use(
                numerator_local,
                success_target,
                use_site.block,
                &can_reach_use,
                &mut stability_visited,
                &mut stability_pending,
                stability_generation,
            )? {
                continue;
            }
            stability_generation =
                stability_generation
                    .checked_add(1)
                    .ok_or(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
                        "quotient-bound stability generation overflowed",
                    )))?;
            if !self.local_is_stable_between_edge_and_use(
                divisor_local,
                product_success,
                use_site.block,
                &can_reach_use,
                &mut stability_visited,
                &mut stability_pending,
                stability_generation,
            )? {
                continue;
            }
            let mut edge = HashSet::new();
            self.paid().set_reserve(&mut edge, 1)?;
            self.paid()
                .set_insert(&mut edge, (switch_block, success_target))?;
            if !self.edge_set_dominates(&edge, use_site.block)? {
                continue;
            }
            if proven_bound
                .as_ref()
                .is_none_or(|current| candidate < current.maximum)
            {
                proven_bound = Some(AuthenticatedQuotientStrictBoundV1 {
                    maximum: candidate,
                    factor: self.paid().clone_operand(factor)?,
                    factor_use: product.definition,
                });
            }
        }
        Ok(proven_bound)
    }
}
