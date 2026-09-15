use super::*;

fn mapped_local(view:&SemanticExpandedRootV1,instance:SemanticCallInstanceIdV1,
    function:SemanticFunctionIdV1,original:SemanticLocalIdV1,budget:&mut Budget)->Result<SemanticLocalIdV1> {
    let mut found=None;
    for (n,origin) in view.local_origins().iter().enumerate() {
        budget.charge(1,0)?;
        if origin.instance()==instance && origin.function()==function && origin.local()==original {
            if found.replace(SemanticLocalIdV1::from_index(n as u32)).is_some() {return Err(mismatch());}
        }
    }
    found.ok_or_else(mismatch)
}
fn mapped_block(view:&SemanticExpandedRootV1,instance:SemanticCallInstanceIdV1,
    function:SemanticFunctionIdV1,original:SemanticBlockIdV1,budget:&mut Budget)->Result<SemanticBlockIdV1> {
    let mut found=None;
    for (n,origin) in view.block_origins().iter().enumerate() {
        budget.charge(1,0)?;
        if origin.instance()==instance && origin.function()==function && origin.block()==original {
            if found.replace(SemanticBlockIdV1::from_index(n as u32)).is_some() {return Err(mismatch());}
        }
    }
    found.ok_or_else(mismatch)
}

fn returned(view:&SemanticExpandedRootV1,binding:&SemanticExpandedDefinedCapabilityV1,
    budget:&mut Budget)->Result<(SemanticBlockIdV1,u32)> {
    let mut found=None;
    for (block,origin) in view.block_origins().iter().enumerate() {
        budget.charge(1,0)?;
        for (statement,marker) in origin.statements().iter().enumerate() {
            budget.charge(1,0)?;
            if *marker==(S::ReturnTransfer {callee:binding.callee_instance()}) {
                if origin.instance()!=binding.callee_instance()
                    || origin.function()!=binding.contract().function()
                    || origin.terminator()!=(T::CallReturn {callee:binding.callee_instance()})
                    || found.replace((SemanticBlockIdV1::from_index(block as u32),statement as u32)).is_some() {
                    return Err(mismatch());
                }
            }
        }
    }
    let site=found.ok_or_else(mismatch)?;
    let transfer=assignment(view.body(),site.0,site.1)?;
    if transfer.destination()!=binding.destination()
        || !matches!(transfer.value().kind(),SemanticRvalueKindV1::Use(SemanticOperandV1::Move(p))
            if p.local()==binding.callee_return() && p.projections().is_empty()
                && p.ty()==binding.destination().ty()) {return Err(mismatch());}
    Ok(site)
}

fn frames(semantic:&AdmittedInertSemanticMirV1,view:&SemanticExpandedRootV1,
    binding:&SemanticExpandedDefinedCapabilityV1,returned:(SemanticBlockIdV1,u32),budget:&mut Budget)->Result<()> {
    let source=&semantic.functions()[binding.contract().function().index() as usize];
    let entry=view.block_origins().get(binding.expanded_call_block().index() as usize).ok_or_else(mismatch)?;
    let exit=view.block_origins().get(returned.0.index() as usize).ok_or_else(mismatch)?;
    let arity=binding.callee_arguments().len();
    let count=source.locals().len();
    if entry.instance()!=binding.caller_instance() || entry.function()!=binding.caller_function()
        || entry.terminator()!=(T::CallEntry {callee:binding.callee_instance()})
        || returned.1!=0 || exit.statements().len()!=count+1
        || binding.arguments().len()!=arity {return Err(mismatch());}
    let start=entry.statements().len().checked_sub(count+arity).ok_or_else(mismatch)?;
    for (index,source_local) in source.locals().iter().enumerate() {
        budget.charge(2,0)?;
        let original=SemanticLocalIdV1::from_index(index as u32);
        let local=mapped_local(view,binding.callee_instance(),binding.contract().function(),original,budget)?;
        if entry.statements()[start+index]!=(S::FrameStorageLive {callee:binding.callee_instance(),local:original})
            || exit.statements()[index+1]!=(S::FrameStorageDead {callee:binding.callee_instance(),local:original})
            || view.body().blocks()[binding.expanded_call_block().index() as usize].statements()[start+index].kind()
                !=&SemanticStatementKindV1::StorageLive(local)
            || view.body().blocks()[returned.0.index() as usize].statements()[index+1].kind()
                !=&SemanticStatementKindV1::StorageDead(local) {return Err(mismatch());}
        match source_local.role() {
            SemanticLocalRoleV1::Argument(n) if binding.callee_arguments().get(n as usize)!=Some(&local)=>return Err(mismatch()),
            SemanticLocalRoleV1::Return if local!=binding.callee_return()=>return Err(mismatch()),
            _=>{},
        }
    }
    for index in 0..arity {
        budget.charge(1,0)?;
        let site=start+count+index;
        if entry.statements()[site]!=(S::ParameterTransfer {callee:binding.callee_instance(),argument:index as u32}) {return Err(mismatch());}
        let a=assignment(view.body(),binding.expanded_call_block(),site as u32)?;
        if a.destination().local()!=binding.callee_arguments()[index] || !a.destination().projections().is_empty()
            || a.destination().ty()!=binding.arguments()[index].ty()
            || !matches!(a.value().kind(),SemanticRvalueKindV1::Use(value)
                if value==&binding.arguments()[index]) {return Err(mismatch());}
    }
    Ok(())
}

pub(super) fn result(semantic:&AdmittedInertSemanticMirV1,view:&SemanticExpandedRootV1,
    binding:&SemanticExpandedDefinedCapabilityV1,record:SemanticDefinedReusablePhaseV1,budget:&mut Budget)->Result<ProductionSemanticPhaseResultV1> {
    let site=returned(view,binding,budget)?;
    frames(semantic,view,binding,site,budget)?;
    let inputs=match record.recipe() {
        R::Bind {phase_reference,storage_reference,phase_lds,..}=> {
            let [phase,storage]=binding.callee_arguments() else {return Err(mismatch());};
            if site.0!=binding.expanded_entry_block() || phase==storage
                || view.body().locals()[phase.index() as usize].ty()!=phase_reference.reference
                || view.body().locals()[storage.index() as usize].ty()!=storage_reference.reference
                || binding.destination().ty()!=phase_lds {return Err(mismatch());}
            ProductionSemanticPhaseResultInputsV1::Bind {phase_reference:*phase,storage_reference:*storage}
        }
        R::Finish {barrier,barrier_block,return_block,workgroup_after_barrier,completion,..}=> {
            if binding.callee_arguments().len()!=1 || binding.destination().ty()!=completion
                || site.0!=mapped_block(view,binding.callee_instance(),record.function(),return_block,budget)? {return Err(mismatch());}
            let block=mapped_block(view,binding.callee_instance(),record.function(),barrier_block,budget)?;
            if view.block_origins()[block.index() as usize].terminator()!=T::Source {return Err(mismatch());}
            let SemanticTerminatorKindV1::Call(call)=view.body().blocks()[block.index() as usize].terminator().kind() else {return Err(mismatch());};
            let output=call.destination().ok_or_else(mismatch)?;
            if call.callee()!=barrier.callable || call.unwind()!=SemanticUnwindActionV1::Unreachable
                || call.arguments().len()!=1 || output.place().ty()!=workgroup_after_barrier
                || !output.place().projections().is_empty() || output.edge().target()!=site.0
                || output.edge().role()!=SemanticEdgeRoleV1::CallReturn {return Err(mismatch());}
            let mut incoming=0usize;
            for (index,b) in view.body().blocks().iter().enumerate() {
                budget.charge(1,0)?;
                b.terminator().kind().try_for_each_edge::<ProductionSemanticSsaErrorV1>(|edge| {
                    budget.charge(1,0)?;
                    if edge.target()==site.0 {
                        if index!=block.index() as usize || edge.role()!=SemanticEdgeRoleV1::CallReturn {return Err(mismatch());}
                        incoming+=1;
                    }
                    Ok(())
                })?;
            }
            if incoming!=1 {return Err(mismatch());}
            ProductionSemanticPhaseResultInputsV1::Finish {barrier_output:output.place().local()}
        }
        _=>return Err(mismatch()),
    };
    Ok(ProductionSemanticPhaseResultV1 {record,caller:binding.caller_instance(),callee:binding.callee_instance(),
        block:site.0,statement:site.1,return_local:binding.callee_return(),destination:binding.destination().local(),inputs})
}

pub(super) fn relay(semantic:&AdmittedInertSemanticMirV1,view:&SemanticExpandedRootV1,
    bindings:&[SemanticExpandedDefinedCapabilityV1],wrapper:&SemanticExpandedDefinedCapabilityV1,
    record:SemanticDefinedReusablePhaseV1,variable:SsaVariableIdV1,budget:&mut Budget)->Result<ProductionSemanticPhaseRelayV1> {
    let R::WithPhase {relay,invoke_block,drop_block,drop_completion,..}=record.recipe() else {return Err(mismatch());};
    let Some(SemanticCallableDeclV1::Defined {function:finish_function})=semantic.callables().get(relay.finish.callable.index() as usize) else {return Err(mismatch());};
    let mut finish=None;
    for candidate in bindings {
        budget.charge(1,0)?;
        if candidate.root()!=view.root() || candidate.caller_function()!=relay.closure_function
            || candidate.call_block()!=relay.finish_call_block {continue;}
        let closure=view.instances().get(candidate.caller_instance().index() as usize).ok_or_else(mismatch)?;
        if closure.parent()!=Some(wrapper.callee_instance()) || closure.call_block()!=Some(invoke_block) {continue;}
        if candidate.contract().function()!=*finish_function || !matches!(candidate.contract(),C::ReusablePhase(r) if matches!(r.recipe(),R::Finish {..}))
            || finish.replace(candidate).is_some() {return Err(mismatch());}
    }
    let finish=finish.ok_or_else(mismatch)?;
    let pack_block=mapped_block(view,finish.caller_instance(),relay.closure_function,relay.closure_pack_block,budget)?;
    let origin=&view.block_origins()[pack_block.index() as usize];
    let mut pack_statement=None;
    for (index,statement) in origin.statements().iter().enumerate() {
        budget.charge(1,0)?;
        if *statement==(S::Source {statement:relay.closure_pack_statement})
            && pack_statement.replace(index as u32).is_some() {return Err(mismatch());}
    }
    let pack_statement=pack_statement.ok_or_else(mismatch)?;
    let drop_block=mapped_block(view,wrapper.callee_instance(),record.function(),drop_block,budget)?;
    // The real core::mem::drop call may itself be expanded. Match that exact
    // child/normal edge, never require an unexpanded source-call spelling.
    match view.block_origins()[drop_block.index() as usize].terminator() {
        T::Source=> {
            let SemanticTerminatorKindV1::Call(call)=view.body().blocks()[drop_block.index() as usize].terminator().kind() else {return Err(mismatch());};
            if call.callee()!=drop_completion.callable {return Err(mismatch());}
        }
        T::CallEntry {callee}=> {
            let frame=view.instances().get(callee.index() as usize).ok_or_else(mismatch)?;
            if frame.parent()!=Some(wrapper.callee_instance())
                || semantic.callables().get(drop_completion.callable.index() as usize)
                    !=Some(&SemanticCallableDeclV1::Defined {function:frame.function()}) {return Err(mismatch());}
        }
        T::CallReturn {..}=>return Err(mismatch()),
    }
    Ok(ProductionSemanticPhaseRelayV1 {record,wrapper:wrapper.callee_instance(),closure:finish.caller_instance(),
        finish:finish.callee_instance(),pack_block,pack_statement,drop_block,
        finish_result:finish.destination().local(),erased:matches!(relay.completion_field,SemanticPhaseCompletionFieldV1::ErasedZstConstant {..}),variable})
}
