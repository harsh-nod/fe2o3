//! Exact-use projection of the existing, owner-borrowed mixed-SSA capture.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_mir_model::{SsaEventV1, SsaResolvedEventV1, SsaValueV1};
use fe2o3_pliron::{
    ProductionSemanticSsaEntryOriginV1 as EntryOrigin, ProductionSemanticSsaEventRoleV1 as Role,
    ProductionSemanticSsaFunctionOccurrencesV1 as Occurrences,
    ProductionSemanticSsaOccurrenceSiteV1 as Site,
    ProductionSemanticSsaOperandRoleV1 as OperandRole, ProductionSemanticSsaOwnerV1,
};
use std::{
    cell::{Cell, RefCell},
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

#[path = "pipeline_scalar_live_out_v1.rs"]
mod live_out_v1;
pub(super) use live_out_v1::Resolution;

type Result<T> = std::result::Result<T, ProductionRankedProjectionErrorV1>;

#[derive(Clone, Copy)]
pub(super) struct Source<'s> {
    pub(super) owner: &'s ProductionSemanticSsaOwnerV1,
    pub(super) function: SemanticFunctionIdV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Origin {
    Argument {
        local: usize,
        argument: u32,
    },
    Assignment {
        local: usize,
        site: ScalarAssignmentSiteV1,
    },
}

#[derive(Clone, Copy)]
pub(super) enum Failure {
    Resource(Resource),
    Incomplete(&'static str),
}

impl Failure {
    fn error(self) -> ProductionRankedProjectionErrorV1 {
        match self {
            Self::Resource(error) => resource(error),
            Self::Incomplete(reason) => reject(reason),
        }
    }
}

fn resource(error: Resource) -> ProductionRankedProjectionErrorV1 {
    ranked_projection_source_v1::resource(error)
}

fn reject(reason: &'static str) -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::Incomplete(reason)
}

fn sum(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| resource(Resource::Arithmetic))
}

fn bytes<T>(count: usize) -> Result<usize> {
    count
        .checked_mul(size_of::<T>())
        .ok_or_else(|| resource(Resource::Arithmetic))
}

pub(super) struct Index<'s> {
    source: Source<'s>,
    function: &'s SemanticFunctionDeclV1,
    occurrences: Occurrences<'s>,
    definitions: Vec<Option<Origin>>,
    statement_offsets: Vec<usize>,
    identity: (usize, Ledger),
    minimum: usize,
    failure: Cell<Option<Failure>>,
    live_out: RefCell<Option<live_out_v1::State>>,
    extra_owned: Cell<usize>,
}

impl Index<'_> {
    pub(super) fn visit_key(
        &self,
        function: &SemanticFunctionDeclV1,
        site: ScalarAssignmentSiteV1,
        facts: &mut dyn ProjectedAssertionFactsV1,
    ) -> Result<usize> {
        self.charge(facts, 1)?;
        if !std::ptr::eq(function, self.function) {
            return self.fail("a pipeline scalar cycle key substituted its source function");
        }
        let Some(block) = function.blocks().get(site.block) else {
            return self.fail("a pipeline scalar cycle key has no source block");
        };
        if site.statement >= block.statements().len() {
            return self.fail("a pipeline scalar cycle key has no assignment statement");
        }
        self.statement_offsets[site.block]
            .checked_add(site.statement)
            .ok_or_else(|| {
                let error = resource(Resource::Arithmetic);
                self.record(&error);
                error
            })
    }

    fn record(&self, error: &ProductionRankedProjectionErrorV1) {
        if self.failure.get().is_some() {
            return;
        }
        let failure = match error {
            ProductionRankedProjectionErrorV1::CanonicalAssertions(
                canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(error),
            ) => Failure::Resource(*error),
            ProductionRankedProjectionErrorV1::Incomplete(reason) => Failure::Incomplete(reason),
            _ => Failure::Resource(Resource::Accounting),
        };
        self.failure.set(Some(failure));
    }

    fn fail<T>(&self, reason: &'static str) -> Result<T> {
        let failure = self.failure.get().unwrap_or(Failure::Incomplete(reason));
        self.failure.set(Some(failure));
        Err(failure.error())
    }

    fn check(&self, facts: &mut dyn ProjectedAssertionFactsV1) -> Result<()> {
        let actual = facts
            .helper_value_ledger_v1()
            .inspect_err(|error| self.record(error))?;
        if actual != self.identity {
            let error = resource(Resource::Accounting);
            self.record(&error);
            return Err(error);
        }
        if let Some(failure) = self.failure.get() {
            return Err(failure.error());
        }
        if facts
            .scalar_private_storage_v1()
            .inspect_err(|error| self.record(error))?
            < sum(self.minimum, self.extra_owned.get())?
        {
            self.failure
                .set(Some(Failure::Resource(Resource::Accounting)));
            return Err(resource(Resource::Accounting));
        }
        Ok(())
    }

    fn charge(&self, facts: &mut dyn ProjectedAssertionFactsV1, amount: usize) -> Result<()> {
        self.check(facts)?;
        if let Err(error) = facts.charge_private_array_work(amount) {
            self.record(&error);
            return Err(error);
        }
        Ok(())
    }

    fn site(&self, site: Site) -> Result<(usize, usize)> {
        let (block, statement) = match site {
            Site::Statement { block, statement } => {
                (block.get() as usize, Some(statement as usize))
            }
            Site::Terminator { block } => (block.get() as usize, None),
        };
        let source = self
            .function
            .blocks()
            .get(block)
            .ok_or_else(|| reject("a pipeline SSA occurrence has no source block"))?;
        let statement = statement.unwrap_or(source.statements().len());
        if statement > source.statements().len() {
            return Err(reject("a pipeline SSA occurrence has no source statement"));
        }
        Ok((block, statement))
    }

    fn resolved_value(
        &self,
        function: &SemanticFunctionDeclV1,
        local: usize,
        use_site: ScalarAssignmentSiteV1,
        facts: &mut dyn ProjectedAssertionFactsV1,
    ) -> Result<SsaValueV1> {
        self.charge(facts, 4)?;
        if !std::ptr::eq(function, self.function)
            || !std::ptr::eq(self.occurrences.owner(), self.source.owner)
            || self.occurrences.function() != self.source.function
        {
            return self.fail("a pipeline scalar SSA query substituted its source function");
        }
        let key = (use_site.block, use_site.statement);
        let rows = self.occurrences.events();
        let mut low = 0;
        let mut high = rows.len();
        while low < high {
            self.charge(facts, 1)?;
            let middle = low + (high - low) / 2;
            if self.site(rows[middle].site())? < key {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        let mut value = None;
        for row in &rows[low..] {
            self.charge(facts, 2)?;
            if self.site(row.site())? != key {
                break;
            }
            if row.role() != Role::BaseUse || row.event().variable().get() as usize != local {
                continue;
            }
            let Some(SsaResolvedEventV1::Use {
                variable,
                value: actual,
            }) = row.resolved()
            else {
                return self
                    .fail("a pipeline scalar use is killed, unreachable, or retained in memory");
            };
            if !row.is_reachable()
                || !row.is_promoted()
                || variable.get() as usize != local
                || row.event() != SsaEventV1::Use(variable)
                || value.is_some_and(|previous| previous != actual)
            {
                return self.fail("a pipeline scalar source site has no unique promoted SSA use");
            }
            value = Some(actual);
        }
        self.charge(facts, 1)?;
        let Some(value) = value else {
            return self.fail("a pipeline scalar has no captured use at its exact source site");
        };
        Ok(value)
    }

    pub(super) fn resolve(
        &self,
        function: &SemanticFunctionDeclV1,
        local: usize,
        use_site: ScalarAssignmentSiteV1,
        facts: &mut dyn ProjectedAssertionFactsV1,
    ) -> Result<Origin> {
        let value = self.resolved_value(function, local, use_site, facts)?;
        self.origin(local, value)
    }

    fn origin(&self, local: usize, value: SsaValueV1) -> Result<Origin> {
        let SsaValueV1::Definition(id) = value else {
            return self.fail("a pipeline scalar requires an unsupported SSA block argument");
        };
        match self.definitions.get(id.get() as usize).copied().flatten() {
            Some(
                origin @ (Origin::Argument { local: actual, .. }
                | Origin::Assignment { local: actual, .. }),
            ) if actual == local => Ok(origin),
            _ => self.fail("a pipeline scalar SSA definition has no supported original producer"),
        }
    }
}

fn reserve(
    facts: &mut dyn ProjectedAssertionFactsV1,
    owned: &mut usize,
    amount: usize,
) -> Result<()> {
    let total = sum(*owned, amount)?;
    facts.reserve_scalar_private_storage_v1(amount)?;
    *owned = total;
    Ok(())
}

fn construct<'s>(
    source: Source<'s>,
    function: &'s SemanticFunctionDeclV1,
    facts: &mut dyn ProjectedAssertionFactsV1,
    identity: (usize, Ledger),
    floor: usize,
    owned: &mut usize,
) -> Result<Index<'s>> {
    facts.charge_private_array_work(6)?;
    if source
        .owner
        .source_semantic()
        .functions()
        .get(source.function.index() as usize)
        .is_none_or(|actual| !std::ptr::eq(actual, function))
    {
        return Err(reject(
            "a pipeline scalar SSA index substituted its source function",
        ));
    }
    let plan = source
        .owner
        .plan_for_function(source.function)
        .filter(|plan| plan.function_identity() == function.identity())
        .ok_or_else(|| reject("a pipeline scalar has no exact source SSA plan"))?;
    let occurrences = source
        .owner
        .occurrences_v1()
        .and_then(|view| view.function(source.function))
        .ok_or_else(|| reject("a pipeline scalar has no captured source SSA occurrences"))?;
    let count = plan.plan().definition_count();
    facts.charge_private_array_work(count)?;
    reserve(facts, owned, bytes::<Option<Origin>>(count)?)?;
    let mut definitions = Vec::new();
    definitions
        .try_reserve_exact(count)
        .map_err(|_| resource(Resource::Allocation))?;
    reserve(
        facts,
        owned,
        bytes::<Option<Origin>>(
            definitions
                .capacity()
                .checked_sub(count)
                .ok_or_else(|| resource(Resource::Accounting))?,
        )?,
    )?;
    definitions.resize(count, None);
    for row in occurrences.entry_definitions() {
        facts.charge_private_array_work(3)?;
        let (Some(SsaValueV1::Definition(id)), EntryOrigin::Argument(argument)) =
            (row.value(), row.origin())
        else {
            continue;
        };
        let local = row.variable().get() as usize;
        if function
            .locals()
            .get(local)
            .is_none_or(|decl| decl.role() != SemanticLocalRoleV1::Argument(argument))
        {
            return Err(reject(
                "a pipeline scalar SSA entry is not the original argument",
            ));
        }
        install(
            &mut definitions,
            id.get() as usize,
            Origin::Argument { local, argument },
        )?;
    }
    let block_count = sum(function.blocks().len(), 1)?;
    facts.charge_private_array_work(block_count)?;
    reserve(facts, owned, bytes::<usize>(block_count)?)?;
    let mut statement_offsets = Vec::new();
    statement_offsets
        .try_reserve_exact(block_count)
        .map_err(|_| resource(Resource::Allocation))?;
    reserve(
        facts,
        owned,
        bytes::<usize>(
            statement_offsets
                .capacity()
                .checked_sub(block_count)
                .ok_or_else(|| resource(Resource::Accounting))?,
        )?,
    )?;
    statement_offsets.push(0);
    let mut offset = 0;
    for block in function.blocks() {
        offset = sum(offset, block.statements().len())?;
        statement_offsets.push(offset);
    }
    let mut index = Index {
        source,
        function,
        occurrences,
        definitions,
        statement_offsets,
        identity,
        minimum: sum(floor, *owned)?,
        failure: Cell::new(None),
        live_out: RefCell::new(None),
        extra_owned: Cell::new(0),
    };
    let mut previous = None;
    for row in index.occurrences.events() {
        index.charge(facts, 4)?;
        let key = index.site(row.site())?;
        if previous.is_some_and(|previous| previous > key) {
            return Err(reject(
                "pipeline scalar SSA occurrences changed source order",
            ));
        }
        previous = Some(key);
        let Some(SsaResolvedEventV1::Define {
            variable,
            value: SsaValueV1::Definition(id),
        }) = row.resolved()
        else {
            continue;
        };
        if row.role() != Role::DestinationDefine
            || row.operand() != OperandRole::Destination
            || !row.is_promoted()
            || !row.is_reachable()
        {
            continue;
        }
        let Site::Statement { .. } = row.site() else {
            continue;
        };
        let Some(statement) = function.blocks()[key.0].statements().get(key.1) else {
            return Err(reject(
                "a pipeline scalar definition is not a source statement",
            ));
        };
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            continue;
        };
        let local = variable.get() as usize;
        if assignment.destination().local().index() as usize != local
            || !assignment.destination().projections().is_empty()
        {
            return Err(reject(
                "a pipeline scalar SSA definition changed its destination",
            ));
        }
        install(
            &mut index.definitions,
            id.get() as usize,
            Origin::Assignment {
                local,
                site: ScalarAssignmentSiteV1 {
                    block: key.0,
                    statement: key.1,
                },
            },
        )?;
    }
    Ok(index)
}

fn install(definitions: &mut [Option<Origin>], id: usize, origin: Origin) -> Result<()> {
    let slot = definitions
        .get_mut(id)
        .ok_or_else(|| reject("a pipeline scalar SSA definition is out of range"))?;
    if slot.replace(origin).is_some() {
        return Err(reject(
            "a pipeline scalar SSA definition has duplicate producers",
        ));
    }
    Ok(())
}

pub(super) fn with_index<'s, 'facts, T, C>(
    source: Source<'s>,
    function: &'s SemanticFunctionDeclV1,
    facts: &mut (dyn ProjectedAssertionFactsV1 + 'facts),
    consume: C,
) -> Result<T>
where
    C: FnOnce(&Index<'s>, &mut (dyn ProjectedAssertionFactsV1 + 'facts)) -> Result<T>,
{
    let mut pending = Some(consume);
    let mut owned = 0;
    let mut index = None;
    let entry = facts.helper_value_ledger_v1().and_then(|identity| {
        facts
            .scalar_private_storage_v1()
            .map(|floor| (identity, floor))
    });
    let (identity, floor) = match entry {
        Ok(entry) => entry,
        Err(error) => {
            drain(pending.take());
            return Err(error);
        }
    };
    let mut result = catch_unwind(AssertUnwindSafe(|| {
        facts.charge_private_array_work(3)?;
        let headers = [
            size_of::<Option<C>>(),
            size_of::<Option<Index<'s>>>(),
            size_of::<Result<Index<'s>>>(),
            size_of::<std::thread::Result<Result<T>>>(),
            size_of::<Result<T>>(),
            size_of::<Result<()>>(),
            size_of::<Option<Failure>>(),
            size_of::<(usize, Ledger)>(),
            size_of::<Box<dyn std::any::Any + Send>>(),
        ]
        .into_iter()
        .try_fold(0, sum)?;
        reserve(facts, &mut owned, headers)?;
        index = Some(construct(
            source, function, facts, identity, floor, &mut owned,
        )?);
        pending.take().expect("one pipeline SSA continuation")(index.as_ref().unwrap(), facts)
    }));
    // A refused construction never invokes the owned continuation. Drain its
    // captures before restoring credit, preserving the selected refusal.
    drain(pending.take());
    let failure = index.as_ref().and_then(|index| index.failure.get());
    if let Some(failure) = failure {
        drain(result);
        result = Ok(Err(failure.error()));
    }
    let total_owned = owned.checked_add(index.as_ref().map_or(0, |index| index.extra_owned.get()));
    let custody = total_owned
        .ok_or_else(|| resource(Resource::Arithmetic))
        .and_then(|total| {
            facts.helper_value_ledger_v1().and_then(|actual| {
                if actual != identity || facts.scalar_private_storage_v1()? < sum(floor, total)? {
                    Err(resource(Resource::Accounting))
                } else {
                    Ok(())
                }
            })
        });
    drop(index);
    let cleanup = match custody {
        Ok(()) => {
            facts.release_scalar_private_storage_v1(total_owned.expect("checked owned credit"))
        }
        Err(error) => {
            if matches!(result, Ok(Ok(_))) {
                drain(result);
                result = Ok(Err(error));
            } else {
                drain(error);
            }
            Ok(())
        }
    };
    if let Err(error) = cleanup {
        if matches!(result, Ok(Ok(_))) {
            drain(result);
            result = Ok(Err(error));
        } else {
            drain(error);
        }
    }
    match result {
        Ok(result) => result,
        Err(payload) => resume_unwind(payload),
    }
}

fn drain<T>(value: T) {
    let mut dropped = catch_unwind(AssertUnwindSafe(|| drop(value)));
    while let Err(payload) = dropped {
        dropped = catch_unwind(AssertUnwindSafe(|| drop(payload)));
    }
}

pub(super) fn with_pipeline_index<'s, 'facts, T, C>(
    source: Option<Source<'s>>,
    types: &'s [SemanticTypeDeclV1],
    function: &'s SemanticFunctionDeclV1,
    facts: Option<&mut (dyn ProjectedAssertionFactsV1 + 'facts)>,
    consume: C,
) -> Result<T>
where
    C: FnOnce(Option<(&Index<'s>, &mut (dyn ProjectedAssertionFactsV1 + 'facts))>) -> Result<T>,
{
    let Some(source) = source else {
        return consume(None);
    };
    let Some(facts) = facts else {
        drain(consume);
        return Err(reject(
            "a pipeline scalar SSA projection has no source ledger",
        ));
    };
    with_index(source, function, facts, |index, facts| {
        if !std::ptr::eq(types, source.owner.source_semantic().types()) {
            return index.fail("a pipeline scalar SSA projection substituted its source types");
        }
        consume(Some((index, facts)))
    })
}
