use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticAggregateKindV1,SemanticConstantValueV1};

fn consume(local:SemanticLocalIdV1,events:&mut Vec<SsaEventV1>) {
    let variable=SsaVariableIdV1::new(local.index());
    events.push(SsaEventV1::Use(variable));
    events.push(SsaEventV1::Kill(variable));
}
fn emit_result(inputs:ProductionSemanticPhaseResultInputsV1,returned:SemanticLocalIdV1,events:&mut Vec<SsaEventV1>) {
    match inputs {
        ProductionSemanticPhaseResultInputsV1::Bind {phase_reference,storage_reference}=> {
            events.push(SsaEventV1::Use(SsaVariableIdV1::new(phase_reference.index())));
            events.push(SsaEventV1::Use(SsaVariableIdV1::new(storage_reference.index())));
        }
        ProductionSemanticPhaseResultInputsV1::Finish {barrier_output}=>consume(barrier_output,events),
    }
    events.push(SsaEventV1::Define(SsaVariableIdV1::new(returned.index())));
}
fn emit_pack(result:SemanticLocalIdV1,erased:bool,relay:SsaVariableIdV1,events:&mut Vec<SsaEventV1>) {
    if erased {consume(result,events);}
    events.push(SsaEventV1::Define(relay));
}
fn emit_drop(relay:SsaVariableIdV1,events:&mut Vec<SsaEventV1>) {
    events.push(SsaEventV1::Use(relay));
    events.push(SsaEventV1::Kill(relay));
}
impl DefinedReusablePhaseResultsV1 {
    pub(in super::super) fn verify_markers(&self,body:&SemanticFunctionDeclV1)->Result<()> {
        if self.view.is_none() && self.results.is_empty() && self.relays.is_empty() {return Ok(());}
        if body.locals().len()!=self.locals {return Err(mismatch());}
        for row in &self.results {
            let transfer=assignment(body,row.block,row.statement)?;
            let output=match row.record.recipe() {
                R::Bind {phase_lds,phase_reference,storage_reference,..}=> {
                    let ProductionSemanticPhaseResultInputsV1::Bind {phase_reference:phase,storage_reference:storage}=row.inputs else {return Err(mismatch());};
                    if phase==storage || body.locals().get(phase.index() as usize).is_none_or(|l|l.ty()!=phase_reference.reference)
                        || body.locals().get(storage.index() as usize).is_none_or(|l|l.ty()!=storage_reference.reference) {return Err(mismatch());}
                    phase_lds
                }
                R::Finish {completion,workgroup_after_barrier,..}=> {
                    let ProductionSemanticPhaseResultInputsV1::Finish {barrier_output}=row.inputs else {return Err(mismatch());};
                    if body.locals().get(barrier_output.index() as usize).is_none_or(|l|l.ty()!=workgroup_after_barrier) {return Err(mismatch());}
                    completion
                }
                _=>return Err(mismatch()),
            };
            if transfer.destination().local()!=row.destination || transfer.destination().ty()!=output
                || !transfer.destination().projections().is_empty()
                || !matches!(transfer.value().kind(),SemanticRvalueKindV1::Use(SemanticOperandV1::Move(p))
                    if p.local()==row.return_local && p.ty()==output && p.projections().is_empty()) {return Err(mismatch());}
        }
        for (index,row) in self.relays.iter().enumerate() {
            let expected=self.locals.checked_add(index).and_then(|n|u32::try_from(n).ok()).ok_or_else(mismatch)?;
            if row.variable.get()!=expected {return Err(mismatch());}
            let R::WithPhase {completion,result_pair,..}=row.record.recipe() else {return Err(mismatch());};
            let pack=assignment(body,row.pack_block,row.pack_statement)?;
            let SemanticRvalueKindV1::Aggregate(a)=pack.value().kind() else {return Err(mismatch());};
            if a.kind()!=&SemanticAggregateKindV1::Tuple || a.operands().len()!=2
                || pack.destination().ty()!=result_pair || !pack.destination().projections().is_empty()
                || a.operands()[0].ty()!=completion {return Err(mismatch());}
            let good=if row.erased {
                matches!(&a.operands()[0],SemanticOperandV1::Constant(c)
                    if matches!(c.value(),SemanticConstantValueV1::ZeroSized))
            } else {
                matches!(&a.operands()[0],SemanticOperandV1::Move(p)
                    if p.local()==row.finish_result && p.projections().is_empty())
            };
            if !good || body.blocks().get(row.drop_block.index() as usize).is_none() {return Err(mismatch());}
        }
        Ok(())
    }

    pub(in super::super) fn append_events(&self,block:u32,statement:Option<u32>,events:&mut Vec<SsaEventV1>) {
        let Ok(index)=self.events.binary_search_by_key(&(SemanticBlockIdV1::from_index(block),statement),|e|(e.block,e.statement)) else {return;};
        match self.events[index].action {
            Action::Result(index)=> {
                let row=self.results[index];
                emit_result(row.inputs,row.return_local,events);
            }
            Action::Pack(index)=> {
                let row=self.relays[index];
                emit_pack(row.finish_result,row.erased,row.variable,events);
            }
            Action::Drop(index)=> {
                let row=self.relays[index];
                emit_drop(row.variable,events);
            }
        }
    }

    pub(in super::super) fn hash_into(&self,digest:&mut Sha256) {
        if self.results.is_empty() && self.relays.is_empty() {return;}
        digest.update(b"fe2o3.execution-reusable-phase-results.v26\0");
        digest.update(self.view.expect("derived phase rows require the exact view"));
        digest.update((self.locals as u64).to_le_bytes());
        digest.update((self.results.len() as u64).to_le_bytes());
        for row in &self.results {
            digest.update(row.record.body_identity());
            digest.update(row.record.source_binding());
            for n in [row.record.function().index(),row.caller.index(),row.callee.index(),row.block.index(),row.statement,
                row.return_local.index(),row.destination.index()] {digest.update(n.to_le_bytes());}
            match row.inputs {
                ProductionSemanticPhaseResultInputsV1::Bind {phase_reference,storage_reference}=> {
                    digest.update([0]);digest.update(phase_reference.index().to_le_bytes());digest.update(storage_reference.index().to_le_bytes());
                }
                ProductionSemanticPhaseResultInputsV1::Finish {barrier_output}=> {
                    digest.update([1]);digest.update(barrier_output.index().to_le_bytes());
                }
            }
        }
        digest.update((self.relays.len() as u64).to_le_bytes());
        for row in &self.relays {
            digest.update(row.record.body_identity());digest.update(row.record.source_binding());
            for n in [row.wrapper.index(),row.closure.index(),row.finish.index(),row.pack_block.index(),row.pack_statement,
                row.drop_block.index(),row.finish_result.index(),row.variable.get()] {digest.update(n.to_le_bytes());}
            digest.update([u8::from(row.erased)]);
        }
    }
}

#[cfg(test)]
#[path = "event_tests.rs"]
mod tests;
