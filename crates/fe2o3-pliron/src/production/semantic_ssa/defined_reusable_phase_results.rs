//! Defined9 result transport and the explicit completion relay. This is an
//! inert replay relation: production consumers must additionally join the live
//! source boundary and the existing ordered reference/loan proof.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAssignmentV1, SemanticDefinedCapabilityContractV1 as C,
    SemanticDefinedReusablePhaseRecipeV1 as R, SemanticDefinedReusablePhaseV1,
    SemanticPhaseCompletionFieldV1, SemanticUnwindActionV1,
};
use fe2o3_mir_model::{
    SemanticCallInstanceIdV1,
    SemanticExpandedStatementOriginV1 as S, SemanticExpandedTerminatorOriginV1 as T,
};
use std::mem::size_of;
use fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticExpandedDefinedCapabilityV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSemanticPhaseResultInputsV1 {
    Bind {
        phase_reference: SemanticLocalIdV1,
        storage_reference: SemanticLocalIdV1,
    },
    Finish { barrier_output: SemanticLocalIdV1 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSemanticPhaseResultV1 {
    record: SemanticDefinedReusablePhaseV1,
    caller: SemanticCallInstanceIdV1,
    callee: SemanticCallInstanceIdV1,
    block: SemanticBlockIdV1,
    statement: u32,
    return_local: SemanticLocalIdV1,
    destination: SemanticLocalIdV1,
    inputs: ProductionSemanticPhaseResultInputsV1,
}
impl ProductionSemanticPhaseResultV1 {
    pub const fn record(&self) -> SemanticDefinedReusablePhaseV1 { self.record }
    pub const fn caller(&self) -> SemanticCallInstanceIdV1 { self.caller }
    pub const fn callee(&self) -> SemanticCallInstanceIdV1 { self.callee }
    pub const fn block(&self) -> SemanticBlockIdV1 { self.block }
    pub const fn statement(&self) -> u32 { self.statement }
    pub const fn return_local(&self) -> SemanticLocalIdV1 { self.return_local }
    pub const fn destination(&self) -> SemanticLocalIdV1 { self.destination }
    pub const fn inputs(&self) -> ProductionSemanticPhaseResultInputsV1 { self.inputs }
}

/// One variable per exact wrapper occurrence, outside the source-local roster.
/// Its only definition is the checked closure pack, and its only consumption is
/// the checked wrapper drop. It is not a Rust local or an allocation identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSemanticPhaseRelayV1 {
    record: SemanticDefinedReusablePhaseV1,
    wrapper: SemanticCallInstanceIdV1,
    closure: SemanticCallInstanceIdV1,
    finish: SemanticCallInstanceIdV1,
    pack_block: SemanticBlockIdV1,
    pack_statement: u32,
    drop_block: SemanticBlockIdV1,
    finish_result: SemanticLocalIdV1,
    erased: bool,
    variable: SsaVariableIdV1,
}
impl ProductionSemanticPhaseRelayV1 {
    pub const fn record(&self) -> SemanticDefinedReusablePhaseV1 { self.record }
    pub const fn wrapper(&self) -> SemanticCallInstanceIdV1 { self.wrapper }
    pub const fn closure(&self) -> SemanticCallInstanceIdV1 { self.closure }
    pub const fn finish(&self) -> SemanticCallInstanceIdV1 { self.finish }
    pub const fn pack_block(&self) -> SemanticBlockIdV1 { self.pack_block }
    pub const fn pack_statement(&self) -> u32 { self.pack_statement }
    pub const fn drop_block(&self) -> SemanticBlockIdV1 { self.drop_block }
    pub const fn finish_result(&self) -> SemanticLocalIdV1 { self.finish_result }
    pub const fn variable(&self) -> SsaVariableIdV1 { self.variable }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action { Result(usize), Pack(usize), Drop(usize) }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Event { block: SemanticBlockIdV1, statement: Option<u32>, action: Action }
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct DefinedReusablePhaseResultsV1 {
    view: Option<[u8;32]>,
    locals: usize,
    results: Vec<ProductionSemanticPhaseResultV1>,
    relays: Vec<ProductionSemanticPhaseRelayV1>,
    events: Vec<Event>,
    resources: SemanticSsaAuxiliaryResourcesV1,
}

#[path = "defined_reusable_phase_results/occurrences.rs"]
mod occurrences;
#[path = "defined_reusable_phase_results/events.rs"]
mod events;

fn mismatch() -> ProductionSemanticSsaErrorV1 { ProductionSemanticSsaErrorV1::ReplayMismatch }
type Result<T> = std::result::Result<T, ProductionSemanticSsaErrorV1>;
struct Budget {
    resources: SemanticSsaAuxiliaryResourcesV1,
    function: SemanticFunctionIdV1,
    limits: ProductionSemanticSsaLimitsV1,
}
impl Budget {
    fn charge(&mut self, work: usize, bytes: usize) -> Result<()> {
        self.resources.work_units = self.resources.work_units.checked_add(work)
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        self.resources.storage_words = self.resources.storage_words.checked_add(bytes.div_ceil(size_of::<usize>()))
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        enforce_function_resource_limit_v1(self.function, self.resources, self.limits)
    }
    fn vector<V>(&mut self, count: usize) -> Result<Vec<V>> {
        let bytes = count.checked_mul(size_of::<V>()).ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        self.charge(count, bytes)?;
        let mut values = Vec::new();
        values.try_reserve_exact(count).map_err(|_| ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        let excess = values.capacity().checked_sub(count).and_then(|n| n.checked_mul(size_of::<V>()))
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        self.charge(0, excess)?;
        Ok(values)
    }
}
fn assignment(body: &SemanticFunctionDeclV1, block: SemanticBlockIdV1, statement: u32) -> Result<&SemanticAssignmentV1> {
    match body.blocks().get(block.index() as usize)
        .and_then(|b| b.statements().get(statement as usize)).map(|s|s.kind()) {
        Some(SemanticStatementKindV1::Assign(a)) => Ok(a), _ => Err(mismatch()),
    }
}

impl DefinedReusablePhaseResultsV1 {
    pub(super) fn derive(semantic: &AdmittedInertSemanticMirV1, expansion: &SemanticCallExpansionV1,
        view: &SemanticExpandedRootV1, limits: ProductionSemanticSsaLimitsV1) -> Result<Self> {
        if !expansion.root(view.root()).is_some_and(|root|std::ptr::eq(root,view)) { return Err(mismatch()); }
        let mut budget = Budget { resources: SemanticSsaAuxiliaryResourcesV1 { storage_words: 16, work_units: 0 },
            function: view.source_body(), limits };
        budget.charge(semantic.functions().len(),0)?;
        if !semantic.functions().iter().any(|f| matches!(f.defined_capability_contract(),
            Some(C::ReusablePhase(r)) if r.provenance().root() == view.root())) {
            return Ok(Self { resources: budget.resources, ..Self::default() });
        }
        // The common replay meters its own temporary construction. Keep its
        // returned storage charged while all derived rows are simultaneously live.
        let bindings = expansion.defined_capability_bindings(semantic)
            .map_err(ProductionSemanticSsaErrorV1::CallExpansion)?;
        budget.charge(bindings.len(), bindings.capacity().checked_mul(size_of::<SemanticExpandedDefinedCapabilityV1>())
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?)?;
        for binding in &bindings {
            budget.charge(binding.arguments().len()+1,binding.retained_auxiliary_bytes()
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?)?;
        }
        let mut result_count=0usize;
        let mut relay_count=0usize;
        for binding in &bindings {
            budget.charge(1,0)?;
            if binding.root()!=view.root() { continue; }
            let C::ReusablePhase(record)=binding.contract() else {continue;};
            match record.recipe() {
                R::Bind {..} | R::Finish {..} => result_count+=1,
                R::WithPhase {..} => relay_count+=1,
                R::OwnerConvert {..} | R::Issue {..} => {},
            }
        }
        let event_count=relay_count.checked_mul(2).and_then(|n|n.checked_add(result_count))
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        let mut results=budget.vector(result_count)?;
        let mut relays=budget.vector(relay_count)?;
        let mut events=budget.vector(event_count)?;
        for binding in &bindings {
            budget.charge(1,0)?;
            if binding.root()!=view.root() {continue;}
            let C::ReusablePhase(record)=binding.contract() else {continue;};
            if binding.root_identity()!=view.identity() || binding.expansion_identity()!=expansion.identity() {
                return Err(mismatch());
            }
            match record.recipe() {
                R::Bind {..} | R::Finish {..} => {
                    let result=occurrences::result(semantic,view,binding,record,&mut budget)?;
                    events.push(Event {block:result.block,statement:Some(result.statement),action:Action::Result(results.len())});
                    results.push(result);
                }
                R::WithPhase {..} => {
                    let variable=view.body().locals().len().checked_add(relays.len())
                        .and_then(|n|u32::try_from(n).ok()).ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
                    let relay=occurrences::relay(semantic,view,&bindings,binding,record,SsaVariableIdV1::new(variable),&mut budget)?;
                    events.push(Event {block:relay.pack_block,statement:Some(relay.pack_statement),action:Action::Pack(relays.len())});
                    events.push(Event {block:relay.drop_block,statement:None,action:Action::Drop(relays.len())});
                    relays.push(relay);
                }
                R::OwnerConvert {..} | R::Issue {..} => {},
            }
        }
        if results.len()!=result_count || relays.len()!=relay_count || events.len()!=event_count { return Err(mismatch()); }
        let search=(usize::BITS-events.len().leading_zeros()) as usize+1;
        budget.charge(events.len().checked_mul(search).ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?,0)?;
        events.sort_unstable_by_key(|e|(e.block,e.statement));
        if events.windows(2).any(|p|(p[0].block,p[0].statement)==(p[1].block,p[1].statement)) {return Err(mismatch());}
        let mut sites=0usize;
        for block in view.body().blocks() {
            budget.charge(1,0)?;
            sites=sites.checked_add(block.statements().len()).and_then(|n|n.checked_add(1))
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        }
        // Adapter binary searches, emitted events, and the extra promotable
        // flags are included in this same function's existing resource ceiling.
        budget.charge(sites.checked_mul(search).and_then(|n|n.checked_add(event_count.checked_mul(6)?))
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?,relay_count)?;
        let value=Self {view:Some(*view.identity()),locals:view.body().locals().len(),results,relays,events,resources:budget.resources};
        value.verify_markers(view.body())?;
        Ok(value)
    }
    pub(super) fn results(&self)->&[ProductionSemanticPhaseResultV1] { &self.results }
    pub(super) fn relays(&self)->&[ProductionSemanticPhaseRelayV1] { &self.relays }
    pub(super) fn resources(&self)->SemanticSsaAuxiliaryResourcesV1 { self.resources }
    pub(super) fn synthetic_variables(&self)->usize { self.relays.len() }
    pub(super) fn verify_view(&self,view:&SemanticExpandedRootV1)->Result<()> {
        if self.view.is_none() && self.results.is_empty() && self.relays.is_empty() {return Ok(());}
        if self.view!=Some(*view.identity()) {return Err(mismatch());}
        self.verify_markers(view.body())
    }
}
