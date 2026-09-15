//! Closed phase sinks and optimized carrier-copy occurrences in the existing
//! borrow graph. These facts do not issue a lease or authenticate live source.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticDefinedCapabilityContractV1 as C, SemanticDefinedReusablePhaseRecipeV1 as R,
    SemanticPhaseReferenceKindV1, SemanticPhaseReferenceV1,
    SemanticMutabilityV1, SemanticPointerKindV1, SemanticPointerMetadataV1, SemanticTypeShapeV1,
};
use fe2o3_mir_model::SemanticCallInstanceIdV1;
use fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticExpandedDefinedCapabilityV1;

pub(super) struct Source<'a> {
    pub semantic:&'a AdmittedInertSemanticMirV1,
    pub view:&'a SemanticExpandedRootV1,
    pub bindings:&'a [SemanticExpandedDefinedCapabilityV1],
}
#[derive(Default)]
pub(super) struct Facts<'a> {
    pub pairs:BTreeMap<SemanticTypeIdV1,SemanticTypeIdV1>,
    pub unique:BTreeMap<SemanticTypeIdV1,SemanticTypeIdV1>,
    sinks:BTreeMap<SemanticTransparentBorrowSiteV1,(&'a SemanticStatementKindV1,[Option<u32>;2])>,
    scopes:Vec<Scope>,
    source:Option<Source<'a>>,
}
struct Scope {
    outer:SemanticCallInstanceIdV1,
    wrapper:SemanticCallInstanceIdV1,
    closure:SemanticCallInstanceIdV1,
    capture:SemanticLocalIdV1,
    wrapper_environment:SemanticLocalIdV1,
    closure_environment:SemanticLocalIdV1,
}
fn mismatch()->ProductionSemanticSsaErrorV1 {ProductionSemanticSsaErrorV1::ReplayMismatch}
impl<'a> Facts<'a> {
    pub(super) fn new(source:Option<Source<'a>>,budget:&mut Budget)->Result<Self,ProductionSemanticSsaErrorV1> {
        let mut result=Self::default();
        let Some(source)=source else {return Ok(result);};
        for binding in source.bindings {
            budget.charge(1)?;
            if binding.root()!=source.view.root() {continue;}
            let C::ReusablePhase(record)=binding.contract() else {continue;};
            if binding.root_identity()!=source.view.identity() {return Err(mismatch());}
            match record.recipe() {
                R::Bind {phase_reference,storage_reference,..}=> {
                    result.pair(source.semantic.types(),phase_reference,budget)?;
                    result.pair(source.semantic.types(),storage_reference,budget)?;
                    let [phase,storage]=binding.callee_arguments() else {return Err(mismatch());};
                    let mut found=false;
                    for (block,origin) in source.view.block_origins().iter().enumerate() {
                        budget.charge(1)?;
                        for (statement,marker) in origin.statements().iter().enumerate() {
                            budget.charge(1)?;
                            if *marker==(SemanticExpandedStatementOriginV1::ReturnTransfer {callee:binding.callee_instance()}) {
                                if found || origin.instance()!=binding.callee_instance() || origin.function()!=record.function() {return Err(mismatch());}
                                found=true;
                                let kind=source.view.body().blocks()[block].statements()[statement].kind();
                                let SemanticStatementKindV1::Assign(a)=kind else {return Err(mismatch());};
                                if a.destination()!=binding.destination()
                                    || !matches!(a.value().kind(),SemanticRvalueKindV1::Use(SemanticOperandV1::Move(p))
                                        if p.local()==binding.callee_return() && p.projections().is_empty()) {return Err(mismatch());}
                                result.sink(SemanticTransparentBorrowSiteV1 {block:block as u32,statement:statement as u32},kind,
                                    [Some(phase.index()),Some(storage.index())],budget)?;
                            }
                        }
                    }
                    if !found {return Err(mismatch());}
                }
                R::Issue {owner_reference,..}=> {
                    result.pair(source.semantic.types(),owner_reference,budget)?;
                    let [owner]=binding.callee_arguments() else {return Err(mismatch());};
                    let block=binding.expanded_entry_block();
                    let origin=&source.view.block_origins()[block.index() as usize];
                    for statement in 0..2 {
                        budget.charge(8)?;
                        if origin.instance()!=binding.callee_instance() || origin.function()!=record.function()
                            || origin.statements().get(statement)!=Some(&SemanticExpandedStatementOriginV1::Source {statement:statement as u32}) {return Err(mismatch());}
                        let kind=source.view.body().blocks()[block.index() as usize].statements()[statement].kind();
                        let SemanticStatementKindV1::Assign(a)=kind else {return Err(mismatch());};
                        let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(p))=a.value().kind() else {return Err(mismatch());};
                        if p.local()!=*owner || !matches!(p.projections(),[d,f]
                            if d.kind()==SemanticProjectionKindV1::Dereference
                                && f.kind()==SemanticProjectionKindV1::Field(statement as u32)) {return Err(mismatch());}
                        result.sink(SemanticTransparentBorrowSiteV1 {block:block.index(),statement:statement as u32},kind,
                            [Some(owner.index()),None],budget)?;
                    }
                }
                R::WithPhase {owner_reference,invoke_block,relay,..}=> {
                    result.pair(source.semantic.types(),owner_reference,budget)?;
                    let [_,environment]=binding.callee_arguments() else {return Err(mismatch());};
                    let [_,SemanticOperandV1::Move(capture)]=binding.arguments() else {return Err(mismatch());};
                    if !capture.projections().is_empty() {return Err(mismatch());}
                    let mut closure=None;
                    for call in source.bindings {
                        budget.charge(1)?;
                        if call.root()!=source.view.root() || call.caller_function()!=relay.closure_function
                            || call.call_block()!=relay.finish_call_block {continue;}
                        let frame=&source.view.instances()[call.caller_instance().index() as usize];
                        if frame.parent()==Some(binding.callee_instance()) && frame.call_block()==Some(invoke_block) {
                            if closure.replace(call.caller_instance()).is_some() {return Err(mismatch());}
                        }
                    }
                    let closure=closure.ok_or_else(mismatch)?;
                    let mut closure_environment=None;
                    for (local,origin) in source.view.local_origins().iter().enumerate() {
                        budget.charge(1)?;
                        if origin.instance()==closure && origin.function()==relay.closure_function
                            && source.semantic.functions()[relay.closure_function.index() as usize].locals()[origin.local().index() as usize].role()==SemanticLocalRoleV1::Argument(0) {
                            if closure_environment.replace(SemanticLocalIdV1::from_index(local as u32)).is_some() {return Err(mismatch());}
                        }
                    }
                    budget.charge(12)?;
                    result.scopes.push(Scope {outer:binding.caller_instance(),wrapper:binding.callee_instance(),closure,
                        capture:capture.local(),wrapper_environment:*environment,closure_environment:closure_environment.ok_or_else(mismatch)?});
                }
                R::OwnerConvert {..} | R::Finish {..}=>{},
            }
        }
        result.source=Some(source);
        Ok(result)
    }
    fn pair(&mut self,types:&[SemanticTypeDeclV1],pair:SemanticPhaseReferenceV1,budget:&mut Budget)->Result<(),ProductionSemanticSsaErrorV1> {
        budget.charge(16)?;
        let mutability=match pair.kind {SemanticPhaseReferenceKindV1::Shared=>SemanticMutabilityV1::Immutable,
            SemanticPhaseReferenceKindV1::Unique=>SemanticMutabilityV1::Mutable};
        if pair.reference==pair.pointee || !matches!(types.get(pair.reference.index() as usize).map(|t|t.shape()),
            Some(SemanticTypeShapeV1::Pointer(p)) if p.kind()==SemanticPointerKindV1::Reference
                && p.mutability()==mutability && p.pointee()==pair.pointee
                && p.metadata()==SemanticPointerMetadataV1::None && p.address_space()==0 && p.pointer_width_bits()==64)
            || self.pairs.insert(pair.reference,pair.pointee).is_some_and(|old|old!=pair.pointee) {return Err(mismatch());}
        if pair.kind==SemanticPhaseReferenceKindV1::Unique {self.unique.insert(pair.reference,pair.pointee);}
        Ok(())
    }
    fn sink(&mut self,site:SemanticTransparentBorrowSiteV1,kind:&'a SemanticStatementKindV1,locals:[Option<u32>;2],budget:&mut Budget)->Result<(),ProductionSemanticSsaErrorV1> {
        budget.charge(12)?;
        if self.sinks.insert(site,(kind,locals)).is_some() {return Err(mismatch());}
        Ok(())
    }
    pub(super) fn captured(&self,site:SemanticTransparentBorrowSiteV1,kind:&SemanticStatementKindV1)->Option<[Option<u32>;2]> {
        let (expected,locals)=self.sinks.get(&site)?;
        std::ptr::eq(*expected,kind).then_some(*locals)
    }
    /// Caller invokes this only for a candidate already matched by the exact
    /// typed single-leaf carrier route. The existing ordered graph still audits
    /// every alias, source mutation, sibling, escape and consuming use.
    pub(super) fn accepts_alias(&self,function:&SemanticFunctionDeclV1,candidate:SemanticBorrowCandidateV1,budget:&mut Budget)->Result<bool,ProductionSemanticSsaErrorV1> {
        let Some(source)=&self.source else {return Ok(false);};
        if !std::ptr::eq(function,source.view.body()) {return Err(mismatch());}
        let block=candidate.site.block as usize;
        let statement=candidate.site.statement as usize;
        let origin=source.view.block_origins().get(block).ok_or_else(mismatch)?;
        let Some(SemanticStatementKindV1::Assign(a))=function.blocks().get(block)
            .and_then(|b|b.statements().get(statement)).map(|s|s.kind()) else {return Ok(false);};
        if !a.destination().projections().is_empty() {return Ok(false);}
        for scope in &self.scopes {
            budget.charge(8)?;
            match origin.statements().get(statement) {
                Some(SemanticExpandedStatementOriginV1::Source {..}) if origin.instance()==scope.outer
                    && a.destination().local()==scope.capture
                    && matches!(a.value().kind(),SemanticRvalueKindV1::Aggregate(_))=>return Ok(true),
                Some(SemanticExpandedStatementOriginV1::ParameterTransfer {callee,argument:0})
                    if *callee==scope.closure && origin.instance()==scope.wrapper
                        && a.destination().local()==scope.closure_environment
                        && matches!(a.value().kind(),SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(p))
                            if p.local()==scope.wrapper_environment && p.projections().is_empty())=>return Ok(true),
                Some(SemanticExpandedStatementOriginV1::Source {..}) if origin.instance()==scope.closure
                    && matches!(a.value().kind(),SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(p))
                        if p.local()==scope.closure_environment && !p.projections().is_empty())=>return Ok(true),
                _=>{},
            }
        }
        Ok(false)
    }
}
