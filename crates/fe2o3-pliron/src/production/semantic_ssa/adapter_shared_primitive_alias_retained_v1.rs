//! Retained alias ownership, including source-driven producers; no ordinary routing.
//! Actual Shared source/SSA admission and whole-root preparation remain separate.
use super::super::super::shared_primitive_reads_v1::RetainedSharedObserverV1;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1,
};
use std::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Fresh,
    Terminal,
    Active,
    Complete,
}
#[derive(Clone, Copy, Eq, PartialEq)]
struct Snapshot {
    budget_slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    counter_slot: usize,
    owned: usize,
    storage: usize,
    work: usize,
    peak: usize,
    denied_work: bool,
    denied_storage: bool,
}
fn snapshot(budget: &Budget<'_>, owned: &usize) -> Snapshot {
    Snapshot {
        budget_slot: budget as *const Budget<'_> as usize,
        ledger: budget.work_ledger_identity_v1(),
        counter_slot: owned as *const usize as usize,
        owned: *owned,
        storage: budget.storage(),
        work: budget.work(),
        peak: budget.peak_storage(),
        denied_work: budget.failed_work().is_some(),
        denied_storage: budget.failed_storage().is_some(),
    }
}
#[derive(Clone, Copy)]
struct Source<'a> {
    function: &'a SemanticFunctionDeclV1,
    types: &'a [SemanticTypeDeclV1],
    cap: usize,
    tree_work: usize,
}
#[derive(Debug)]
pub(super) enum RetainedAliasErrorV1 {
    Resource(Resource),
    Original(Error),
}
impl From<Resource> for RetainedAliasErrorV1 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
type RetainedResult<T> = std::result::Result<T, RetainedAliasErrorV1>;
struct AliasData {
    candidates: Vec<Candidate>,
    holders: BTreeMap<u32, Vec<Alias>>,
    active: BTreeMap<u32, BTreeSet<usize>>,
    alias_words: usize,
    cap: usize,
    tree_work: usize,
    exhausted: bool,
}
struct Held {
    local: u32,
    aliases: Vec<Alias>,
    cursor: usize,
}
#[derive(Clone, Copy)]
enum Slot {
    Current,
    Held(usize),
}
/// Production starts empty. Future statement-driven construction must attach
/// and fund every input owner before using these primitives. Tests alone seed
/// private inert Candidate/Alias DATA as an explicitly prepaid input prefix.
pub(super) struct RetainedAliasStateV1<'a> {
    phase: Phase,
    source: Option<Source<'a>>,
    entry: Option<Snapshot>,
    held: Option<Snapshot>,
    data: AliasData,
    removed: Option<Held>,
    drain: Option<std::vec::IntoIter<Alias>>,
    current: Option<Alias>,
    failure: Option<Resource>,
    output: Vec<Alias>,
    place: Option<&'a SemanticPlaceV1>,
    path: Option<Path<'a>>,
    selected: Vec<Alias>,
    fields: Vec<u32>,
    operand: Option<&'a SemanticOperandV1>,
    observer: RetainedSharedObserverV1<'a>,
    site: Option<SemanticTransparentBorrowSiteV1>,
    rhs: Vec<Alias>,
    installed: Option<Vec<Alias>>,
    replacement: Vec<u32>,
    value: Option<&'a SemanticRvalueV1>,
    statement: Option<&'a SemanticStatementKindV1>,
    terminator: Option<&'a SemanticTerminatorKindV1>,
}
impl<'a> RetainedAliasStateV1<'a> {
    pub(super) fn new() -> Self {
        Self {
            phase: Phase::Fresh,
            source: None,
            entry: None,
            held: None,
            data: AliasData {
                candidates: Vec::new(),
                holders: BTreeMap::new(),
                active: BTreeMap::new(),
                alias_words: 0,
                cap: 0,
                tree_work: 0,
                exhausted: false,
            },
            removed: None,
            drain: None,
            current: None,
            failure: None,
            output: Vec::new(),
            place: None,
            path: None,
            selected: Vec::new(),
            fields: Vec::new(),
            operand: None,
            observer: RetainedSharedObserverV1::new(),
            site: None,
            rhs: Vec::new(),
            installed: None,
            replacement: Vec::new(),
            value: None,
            statement: None,
            terminator: None,
        }
    }
    pub(super) fn prepare_observer_into(
        &mut self,
        function: &'a SemanticFunctionDeclV1,
        types: &'a [SemanticTypeDeclV1],
        scratch: usize,
        units: usize,
        budget: &mut Budget<'_>,
        owned: &mut usize,
    ) -> RetainedResult<()> {
        let fresh = self.phase == Phase::Fresh;
        self.phase = Phase::Terminal;
        if !fresh {
            return Err(self.failure.unwrap_or(Resource::Accounting).into());
        }
        self.observer.prepare_into(
            function,
            types,
            scratch,
            units,
            budget,
            owned,
            &mut self.failure,
        )?;
        self.phase = Phase::Fresh;
        Ok(())
    }
    fn check(
        &self,
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
        budget: &Budget<'_>,
        owned: &usize,
    ) -> RetainedResult<()> {
        self.observer
            .check_if_prepared(function, types, budget, owned, &self.failure)?;
        let source = self.source.ok_or(Resource::Accounting)?;
        let before = self.entry.ok_or(Resource::Accounting)?;
        let now = snapshot(budget, owned);
        let growth = now
            .owned
            .checked_sub(before.owned)
            .ok_or(Resource::Accounting)?;
        let expected = before
            .storage
            .checked_add(growth)
            .ok_or(Resource::Arithmetic)?;
        if let Some(held) = self.held {
            if now.owned < held.owned
                || now.storage < held.storage
                || now.work < held.work
                || now.peak < held.peak
            {
                return Err(Resource::Accounting.into());
            }
        }
        if !std::ptr::eq(source.function, function)
            || !std::ptr::eq(source.types, types)
            || source.cap != self.data.cap
            || source.tree_work != self.data.tree_work
            || before.budget_slot != now.budget_slot
            || before.ledger != now.ledger
            || before.counter_slot != now.counter_slot
            || now.storage != expected
            || now.work < before.work
            || now.peak < before.peak
            || now.denied_work
            || now.denied_storage
            || self.failure.is_some()
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }
    /// Custody observation only, not completion of Shared analysis or a query.
    pub(super) fn postflight_for(
        &self,
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
        budget: &Budget<'_>,
        owned: &usize,
    ) -> RetainedResult<()> {
        if self.phase != Phase::Complete {
            return Err(Resource::Accounting.into());
        }
        self.check(function, types, budget, owned)
    }
}
/// Narrow synchronous session over the original physical pair. Dropping it on
/// error/unwind never drops any alias payload or refunds the caller's credits.
pub(super) struct RetainedAliasSessionV1<'s, 'a, 'w> {
    owner: &'s mut RetainedAliasStateV1<'a>,
    budget: &'s mut Budget<'w>,
    owned: &'s mut usize,
    finished: bool,
}
impl<'s, 'a, 'w> RetainedAliasSessionV1<'s, 'a, 'w> {
    pub(super) fn begin(
        owner: &'s mut RetainedAliasStateV1<'a>,
        function: &'a SemanticFunctionDeclV1,
        types: &'a [SemanticTypeDeclV1],
        cap: usize,
        tree_work: usize,
        budget: &'s mut Budget<'w>,
        owned: &'s mut usize,
    ) -> RetainedResult<Self> {
        let fresh = owner.phase == Phase::Fresh;
        owner.phase = Phase::Terminal;
        let entry = snapshot(budget, owned);
        if !fresh || entry.denied_work || entry.denied_storage || entry.owned > entry.storage {
            return Err(Resource::Accounting.into());
        }
        owner
            .observer
            .check_if_prepared(function, types, budget, owned, &owner.failure)
            .map_err(|error| *owner.failure.get_or_insert(error))?;
        owner.source = Some(Source {
            function,
            types,
            cap,
            tree_work,
        });
        owner.entry = Some(entry);
        owner.data.cap = cap;
        owner.data.tree_work = tree_work;
        let mut session = Self {
            owner,
            budget,
            owned,
            finished: false,
        };
        session.accept_work(32)?;
        session.reserve_storage(frame()?)?;
        session.owner.phase = Phase::Active;
        Ok(session)
    }
    fn failed(&mut self, error: Resource) -> Resource {
        *self.owner.failure.get_or_insert(error)
    }
    fn accept_work(&mut self, units: usize) -> std::result::Result<(), Resource> {
        self.budget
            .charge_work(units)
            .map_err(|error| self.failed(error))
    }
    fn reserve_storage(&mut self, bytes: usize) -> std::result::Result<(), Resource> {
        let next = self
            .owned
            .checked_add(bytes)
            .ok_or(Resource::Arithmetic)
            .map_err(|error| self.failed(error))?;
        self.budget
            .reserve_storage(bytes)
            .map_err(|error| self.failed(error))?;
        *self.owned = next;
        Ok(())
    }
    fn alias(&self, slot: Slot) -> Result<&Alias> {
        match slot {
            Slot::Current => self.owner.current.as_ref(),
            Slot::Held(index) => self
                .owner
                .removed
                .as_ref()
                .and_then(|held| held.aliases.get(index)),
        }
        .ok_or(Error::ReplayMismatch)
    }
    fn alias_mut(&mut self, slot: Slot) -> Result<&mut Alias> {
        match slot {
            Slot::Current => self.owner.current.as_mut(),
            Slot::Held(index) => self
                .owner
                .removed
                .as_mut()
                .and_then(|held| held.aliases.get_mut(index)),
        }
        .ok_or(Error::ReplayMismatch)
    }
    pub(super) fn map_error(&self, error: Error) -> RetainedAliasErrorV1 {
        match self.owner.failure {
            Some(resource) => RetainedAliasErrorV1::Resource(resource),
            None => RetainedAliasErrorV1::Original(error),
        }
    }
    pub(super) fn finish(mut self) -> RetainedResult<()> {
        if self.owner.phase != Phase::Active {
            return Err(Resource::Accounting.into());
        }
        let source = self.owner.source.ok_or(Resource::Accounting)?;
        self.owner
            .check(source.function, source.types, self.budget, self.owned)?;
        self.owner.held = Some(snapshot(self.budget, self.owned));
        self.owner.phase = Phase::Complete;
        self.finished = true;
        Ok(())
    }
    fn tree(&mut self) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            alias_work(
                self.budget,
                &mut self.owner.failure,
                self.owner.data.tree_work,
            )
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    fn reserve(&mut self, words: usize) -> Result<bool> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            // Prepay teardown too, including resource-error or expansion fallback.
            alias_work(
                self.budget,
                &mut self.owner.failure,
                words.checked_add(8).ok_or(Error::ResourceOverflow)?,
            )?;
            let next = self
                .owner
                .data
                .alias_words
                .checked_add(words)
                .ok_or(Error::ResourceOverflow)?;
            if next > self.owner.data.cap {
                self.owner.data.exhausted = true;
                return Ok(false);
            }
            self.owner.data.alias_words = next;
            Ok(true)
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    fn add_live(&mut self, index: usize) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            self.tree()?;
            let mutation = active_mutation_frame().map_err(|error| {
                self.failed(error);
                Error::ResourceOverflow
            })?;
            self.reserve_storage(mutation)
                .map_err(|_| Error::ResourceOverflow)?;
            let candidate = &mut self.owner.data.candidates[index];
            if candidate.live == 0 {
                self.owner
                    .data
                    .active
                    .entry(candidate.source)
                    .or_default()
                    .insert(index);
            }
            candidate.live = candidate
                .live
                .checked_add(1)
                .ok_or(Error::ResourceOverflow)?;
            Ok(())
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    fn deactivate(&mut self, slot: Slot) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            alias_work(self.budget, &mut self.owner.failure, 1)?;
            if !self.alias(slot)?.live {
                return Ok(());
            }
            self.tree()?;
            let index = self.alias(slot)?.candidate;
            let candidate = &mut self.owner.data.candidates[index];
            candidate.live = candidate.live.checked_sub(1).ok_or(Error::ReplayMismatch)?;
            if candidate.live == 0 {
                let active = self
                    .owner
                    .data
                    .active
                    .get_mut(&candidate.source)
                    .ok_or(Error::ReplayMismatch)?;
                if !active.remove(&index) {
                    return Err(Error::ReplayMismatch);
                }
                if active.is_empty() {
                    self.owner.data.active.remove(&candidate.source);
                }
            }
            self.alias_mut(slot)?.live = false;
            Ok(())
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    fn release_current(&mut self) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            self.deactivate(Slot::Current)?;
            let length = self.alias(Slot::Current)?.fields.len();
            alias_work(self.budget, &mut self.owner.failure, length)?;
            self.owner.data.alias_words = self
                .owner
                .data
                .alias_words
                .checked_sub(1 + length)
                .ok_or(Error::ReplayMismatch)?;
            self.owner.current = None;
            Ok(())
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    fn discard_pending(&mut self, poison: bool) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            if self.owner.current.is_some() {
                return Err(Error::ReplayMismatch);
            }
            loop {
                let next = self
                    .owner
                    .drain
                    .as_mut()
                    .ok_or(Error::ReplayMismatch)?
                    .next();
                self.owner.current = next;
                if self.owner.current.is_none() {
                    break;
                }
                alias_work(self.budget, &mut self.owner.failure, 1)?;
                if poison {
                    let index = self.alias(Slot::Current)?.candidate;
                    self.owner.data.candidates[index].valid = false;
                }
                self.release_current()?;
            }
            self.owner.drain = None;
            Ok(())
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    fn source_write(&mut self, local: u32) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            self.tree()?;
            if let Some(active) = self.owner.data.active.get(&local) {
                for &index in active {
                    alias_work(self.budget, &mut self.owner.failure, 2)?;
                    self.owner.data.candidates[index].valid = false;
                }
            }
            Ok(())
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    fn poison_holder(&mut self, local: u32) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            self.tree()?;
            if let Some(aliases) = self.owner.data.holders.get(&local) {
                for alias in aliases {
                    alias_work(self.budget, &mut self.owner.failure, 2)?;
                    self.owner.data.candidates[alias.candidate].valid = false;
                }
            }
            Ok(())
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    fn end_local(&mut self, local: u32) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            if self.owner.removed.is_some() {
                return Err(Error::ReplayMismatch);
            }
            self.source_write(local)?;
            self.tree()?;
            self.owner.removed = self.owner.data.holders.remove(&local).map(|aliases| Held {
                local,
                aliases,
                cursor: 0,
            });
            if self.owner.removed.is_some() {
                while {
                    let held = self.owner.removed.as_ref().ok_or(Error::ReplayMismatch)?;
                    held.cursor < held.aliases.len()
                } {
                    let index = self
                        .owner
                        .removed
                        .as_ref()
                        .ok_or(Error::ReplayMismatch)?
                        .cursor;
                    self.deactivate(Slot::Held(index))?;
                    self.owner
                        .removed
                        .as_mut()
                        .ok_or(Error::ReplayMismatch)?
                        .cursor += 1;
                }
                self.tree()?;
                self.reserve_storage(size_of::<(u32, Vec<Alias>)>())
                    .map_err(|_| Error::ResourceOverflow)?;
                let held = self.owner.removed.take().ok_or(Error::ReplayMismatch)?;
                self.owner.data.holders.insert(held.local, held.aliases);
            }
            Ok(())
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }
}

impl Drop for RetainedAliasSessionV1<'_, '_, '_> {
    fn drop(&mut self) {
        if !self.finished {
            self.owner.phase = Phase::Terminal;
        }
    }
}
fn alias_work(budget: &mut Budget<'_>, failure: &mut Option<Resource>, units: usize) -> Result<()> {
    budget.charge_work(units).map_err(|error| {
        failure.get_or_insert(error);
        Error::ResourceOverflow
    })
}
fn active_mutation_frame() -> std::result::Result<usize, Resource> {
    size_of::<(u32, BTreeSet<usize>)>()
        .checked_add(size_of::<usize>())
        .ok_or(Resource::Arithmetic)
}
fn frame() -> std::result::Result<usize, Resource> {
    // Logical typed source-policy envelopes; not allocator/RSS accounting.
    let rows = [
        size_of::<RetainedAliasStateV1<'static>>(),
        size_of::<RetainedAliasSessionV1<'static, 'static, 'static>>(),
        size_of::<(
            Phase,
            Source<'static>,
            Option<Source<'static>>,
            Snapshot,
            Option<Snapshot>,
            Option<Snapshot>,
            AliasData,
            Option<Held>,
            Option<Alias>,
            Option<std::vec::IntoIter<Alias>>,
            Option<Resource>,
        )>(),
        size_of::<(
            Candidate,
            Alias,
            Vec<Candidate>,
            Vec<Alias>,
            Vec<u32>,
            BTreeMap<u32, Vec<Alias>>,
            BTreeMap<u32, BTreeSet<usize>>,
            BTreeSet<usize>,
        )>(),
        size_of::<(
            Held,
            Option<Held>,
            Slot,
            std::vec::IntoIter<Alias>,
            &mut std::vec::IntoIter<Alias>,
            Option<&mut std::vec::IntoIter<Alias>>,
            Alias,
            Option<Alias>,
            Vec<Alias>,
            Option<Vec<Alias>>,
        )>(),
        size_of::<(
            &mut RetainedAliasStateV1<'static>,
            &RetainedAliasStateV1<'static>,
            &mut RetainedAliasSessionV1<'static, 'static, 'static>,
            &RetainedAliasSessionV1<'static, 'static, 'static>,
            &SemanticFunctionDeclV1,
            &[SemanticTypeDeclV1],
            &mut AliasData,
        )>(),
        size_of::<(
            &Budget<'static>,
            &mut Budget<'static>,
            &usize,
            &mut usize,
            *const Budget<'static>,
            *const usize,
            CanonicalKernelIrWorkLedgerIdentityV1,
            Snapshot,
            Option<Snapshot>,
            usize,
            usize,
            usize,
            usize,
            usize,
            bool,
            bool,
        )>(),
        size_of::<(
            &mut Candidate,
            &Alias,
            &mut Alias,
            Option<&Alias>,
            Option<&mut Alias>,
            &Held,
            &mut Held,
            Option<&Held>,
            Option<&mut Held>,
            &mut Option<Resource>,
            &mut Resource,
            Option<Resource>,
            Resource,
        )>(),
        size_of::<(
            std::collections::btree_map::Entry<'static, u32, BTreeSet<usize>>,
            &mut BTreeSet<usize>,
            Option<&mut BTreeSet<usize>>,
            &BTreeSet<usize>,
            Option<&BTreeSet<usize>>,
            u32,
            usize,
            bool,
        )>(),
        size_of::<(
            std::collections::btree_set::Iter<'static, usize>,
            &usize,
            std::slice::Iter<'static, Alias>,
            &Vec<Alias>,
            Option<&Vec<Alias>>,
            &Alias,
            &mut Candidate,
            u32,
            usize,
            bool,
        )>(),
        size_of::<(
            std::vec::IntoIter<Alias>,
            &mut std::vec::IntoIter<Alias>,
            Option<Alias>,
            Alias,
            Option<Vec<Alias>>,
            Vec<Alias>,
            &Vec<Alias>,
            &mut Vec<Alias>,
            &mut Vec<u32>,
            &[u32],
            usize,
            usize,
            bool,
        )>(),
        size_of::<(
            Result<()>,
            Result<bool>,
            Result<&Alias>,
            Result<&mut Alias>,
            Error,
            Resource,
            RetainedAliasErrorV1,
            RetainedResult<()>,
            RetainedResult<RetainedAliasSessionV1<'static, 'static, 'static>>,
            std::result::Result<(), Resource>,
            std::result::Result<usize, Resource>,
            Option<usize>,
            Option<Source<'static>>,
            Option<Snapshot>,
            std::result::Result<Source<'static>, Resource>,
            std::result::Result<Snapshot, Resource>,
            Result<Held>,
            Result<&Held>,
            Result<&mut Held>,
            Result<&mut std::vec::IntoIter<Alias>>,
            Result<&mut BTreeSet<usize>>,
        )>(),
        size_of::<(
            fe2o3_kernel_ir::CanonicalKernelIrWorkLimitV1,
            fe2o3_kernel_ir::CanonicalKernelIrVerificationStorageLimitV1,
            Resource,
            &mut Option<Resource>,
            RetainedAliasErrorV1,
        )>(),
        size_of::<(
            u32,
            usize,
            usize,
            usize,
            usize,
            usize,
            bool,
            bool,
            Option<usize>,
            Result<usize>,
            (u32, BTreeSet<usize>),
            (u32, Vec<Alias>),
        )>(),
        size_of::<(
            &Vec<Candidate>,
            &mut Vec<Candidate>,
            &BTreeMap<u32, Vec<Alias>>,
            &mut BTreeMap<u32, Vec<Alias>>,
            &BTreeMap<u32, BTreeSet<usize>>,
            &mut BTreeMap<u32, BTreeSet<usize>>,
            Option<BTreeSet<usize>>,
            &u32,
            &usize,
            &Slot,
            &bool,
            &mut Resource,
        )>(),
        size_of::<(
            [usize; 19],
            std::array::IntoIter<usize, 19>,
            usize,
            usize,
            std::result::Result<usize, Resource>,
            Resource,
        )>(),
        place_ops::frame()?,
        read_ops::frame()?,
        engine_ops::frame()?,
    ];
    rows.into_iter().try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Resource::Arithmetic)
    })
}
#[path = "adapter_shared_primitive_engine_retained_v1.rs"]
mod engine_ops;
#[path = "adapter_shared_primitive_place_retained_v1.rs"]
mod place_ops;
#[path = "adapter_shared_primitive_read_retained_v1.rs"]
mod read_ops;
#[allow(unused_imports)]
pub(super) use engine_ops::RetainedSharedEngineV1;
#[cfg(test)]
#[path = "adapter_shared_primitive_alias_retained_v1_tests.rs"]
mod tests;
