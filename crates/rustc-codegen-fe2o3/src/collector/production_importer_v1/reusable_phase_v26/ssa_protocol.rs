//! Join the private live source protocol to one real expanded SSA owner. No
//! independent solver, caller-supplied value roster, or ZST operand is used.
use super::{PhaseResult,rejected,linear_events,expanded_protocol::CheckedExpansion,source_calls::{reserve,spend}};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_mir_model::{SsaBlockIdV1,SsaVariableIdV1,SsaValueV1,SsaResolvedEventV1,SemanticExpandedStatementOriginV1 as Origin};
use fe2o3_pliron::{ProductionSemanticSsaOwnerV1,ProductionSemanticSsaSourceQueryV1,
    ProductionSemanticSsaSourceSiteV1 as Site,ProductionSemanticPhaseResultInputsV1 as Inputs};
use fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticExpandedDefinedCapabilityV1;

#[path = "ssa_boundaries.rs"]
mod boundaries;
#[path = "ssa_phase_argument.rs"]
mod phase_argument;
#[path = "ssa_closure_capture.rs"]
mod closure_capture;
#[path = "ssa_phase_order.rs"]
mod phase_order;

#[path = "emission_sites.rs"]
pub(super) mod emission_sites;
use emission_sites::{Boundary, BoundaryEvent};

pub(super) struct Lease {
    pub allocation:SsaValueV1,
    pub root_reference:SsaValueV1,
    pub phase_reference:SsaValueV1,
    pub storage_reference:SsaValueV1,
    pub result:SsaValueV1,
    pub close:linear_events::Point,
    pub borrowed_at:linear_events::Point,
    emission_definition:Boundary,
    emission_close:Boundary,
}
pub(super) struct Phase {
    pub owner_workgroup:SsaValueV1,
    pub converted_owner:SsaValueV1,
    pub owner_reference:SsaValueV1,
    pub issued_phase:SsaValueV1,
    pub closure_phase:SsaValueV1,
    pub relay:SsaValueV1,
    pub completion:SsaValueV1,
    pub leases:Vec<Lease>,
    pub begin:linear_events::Point,
    pub end:linear_events::Point,
    emission_owner:Boundary,
    emission_issue:Boundary,
    emission_finish:Boundary,
    emission_pack:Boundary,
    emission_drop:Boundary,
    emission_end:Boundary,
}
/// This is source/SSA correspondence, not backend or memory-publication proof.
/// The lifecycle adapter must consume the whole row set, never select one use.
pub(super) struct CheckedSsa<'mir,'view> {
    pub expansion:CheckedExpansion<'mir,'view>,
    pub owner:&'view ProductionSemanticSsaOwnerV1,
    pub phases:Vec<Phase>,
}

impl<'mir,'view> CheckedExpansion<'mir,'view> {
    pub(super) fn bind_ssa(mut self,owner:&'view ProductionSemanticSsaOwnerV1)->PhaseResult<CheckedSsa<'mir,'view>> {
        if !std::ptr::eq(owner.source_semantic(),self.source.semantic)
            || !std::ptr::eq(owner.execution_expansion(),self.expansion)
            || !owner.execution_view_for_root(self.view.root()).is_some_and(|v|std::ptr::eq(v,self.view)) {
            return Err(rejected("phase SSA substituted its source, expansion or root owner"));
        }
        owner.verify_replay().map_err(|_|rejected("phase SSA full production replay"))?;
        let query=owner.source_query_for_root(self.view.root(),self.view.body())
            .map_err(|_|rejected("phase SSA exact source query owner"))?;
        let work=&mut self.source.remaining_work;
        let mut phases=reserve(self.phases.len(),work)?;
        let mut expected_results=self.phases.len();
        for phase in &self.phases {
            spend(work,1)?;
            expected_results=expected_results.checked_add(phase.binds.len()).ok_or_else(||rejected("phase SSA occurrence count overflow"))?;
            let wrapper=&self.bindings[phase.wrapper];
            let finish=&self.bindings[phase.finish];
            let owner_binding=&self.bindings[phase.owner];
            let issue_binding=&self.bindings[phase.issue];
            let owner_site=boundaries::return_transfer(&query,self.view,owner_binding,work)?;
            let owner_workgroup=boundaries::parameter_input(&query,self.view,owner_binding,0,work)?;
            let converted_owner=returned_value(&query,owner_site,owner_binding,work)?;
            let emission_owner=Boundary::at(&query,owner_site,
                SsaVariableIdV1::new(owner_binding.destination().local().index()),
                converted_owner,BoundaryEvent::Define,work)?;
            let wrapper_owner=boundaries::parameter_transfer(&query,self.view,wrapper,0,work)?;
            let owner_reference=wrapper_owner.input;
            let begin=boundaries::owner_borrow(&query,self.view,wrapper,owner_binding,converted_owner,owner_reference,work)?;
            let issue_owner=boundaries::parameter_transfer(&query,self.view,issue_binding,0,work)?;
            if issue_owner.input!=wrapper_owner.output {
                return Err(rejected("phase Issue substituted the wrapper's actual mutable owner reference"));
            }
            let [issue_reference]=issue_binding.callee_arguments() else {return Err(rejected("phase Issue owner arity changed"));};
            for statement in 0..2 {
                if value_at(&query,Site::new(issue_binding.expanded_entry_block(),Some(statement)),
                    SsaVariableIdV1::new(issue_reference.index()),Event::Use,work)?!=issue_owner.output {
                    return Err(rejected("phase Issue source field read changed its owner reference SSA"));
                }
            }
            let issue_site=boundaries::return_transfer(&query,self.view,issue_binding,work)?;
            let issued_phase=returned_value(&query,issue_site,issue_binding,work)?;
            let emission_issue=Boundary::at(&query,issue_site,
                SsaVariableIdV1::new(issue_binding.destination().local().index()),
                issued_phase,BoundaryEvent::Define,work)?;
            let SemanticDefinedCapabilityContractV1::ReusablePhase(wrapper_record)=wrapper.contract() else {
                return Err(rejected("phase closure argument lost its exact wrapper recipe"));
            };
            let SemanticDefinedReusablePhaseRecipeV1::WithPhase {invoke_block,call_tuple,phase_workgroup,relay:source_relay,..}=wrapper_record.recipe() else {
                return Err(rejected("phase closure argument changed its wrapper role"));
            };
            let closure_phase=phase_argument::closure_argument(&query,self.view,phase_argument::Request {
                closure_source:&self.source.semantic.functions()[source_relay.closure_function.index() as usize],
                wrapper:wrapper.callee_instance(),closure:phase.closure,invoke_block,
                tuple_type:call_tuple,phase_type:phase_workgroup,
                issued_local:issue_binding.destination().local(),issued_value:issued_phase,
            },work)?;
            if finish.caller_instance()!=phase.closure {return Err(rejected("phase SSA Finish belongs to another closure instance"));}
            if !matches!(finish.arguments(),[SemanticOperandV1::Copy(p)|SemanticOperandV1::Move(p)]
                if p.local()==closure_phase.local && p.ty()==phase_workgroup && p.projections().is_empty())
                || boundaries::parameter_input(&query,self.view,finish,0,work)?!=closure_phase.value {
                return Err(rejected("phase Finish substituted its original closure Workgroup"));
            }
            let mut relay=None;
            for row in query.plan().defined_reusable_phase_relays() {
                spend(work,1)?;
                if row.wrapper()!=wrapper.callee_instance() {continue;}
                if row.record()!=match wrapper.contract() {SemanticDefinedCapabilityContractV1::ReusablePhase(r)=>r,_=>return Err(rejected("phase SSA wrapper role"))}
                    || row.closure()!=phase.closure || row.finish()!=finish.callee_instance()
                    || relay.replace(row).is_some() {return Err(rejected("phase SSA relay occurrence substituted"));}
            }
            let relay=relay.ok_or_else(||rejected("phase SSA has no exact completion relay"))?;
            let relay_value=value_at(&query,Site::new(relay.pack_block(),Some(relay.pack_statement())),relay.variable(),Event::Define,work)?;
            if value_at(&query,Site::new(relay.drop_block(),None),relay.variable(),Event::Use,work)?!=relay_value
                || value_at(&query,Site::new(relay.drop_block(),None),relay.variable(),Event::Kill,work)?!=relay_value {
                return Err(rejected("phase SSA completion relay changed value or was already consumed"));
            }
            exact_relay_roster(&query,relay.variable(),work)?;
            let end=boundaries::event_point(&query,Site::new(relay.drop_block(),None),relay.variable(),relay_value,Event::Kill,work)?;
            let emission_pack=Boundary::at(&query,Site::new(relay.pack_block(),Some(relay.pack_statement())),
                relay.variable(),relay_value,BoundaryEvent::Define,work)?;
            let emission_drop=Boundary::at(&query,Site::new(relay.drop_block(),None),
                relay.variable(),relay_value,BoundaryEvent::Use,work)?;
            let emission_end=Boundary::at(&query,Site::new(relay.drop_block(),None),
                relay.variable(),relay_value,BoundaryEvent::Kill,work)?;
            if emission_end.point()!=end {return Err(rejected("phase emission End changed its existing relay kill"));}
            if !phase_order::precedes(&query,begin,end,work)? {
                return Err(rejected("phase End is not ordered after its exact owner borrow"));
            }
            let returned=result_row(&query,finish,work)?;
            let Inputs::Finish {barrier_output}=returned.inputs() else {return Err(rejected("phase SSA Finish lost its actual barrier output"));};
            let site=Site::new(returned.block(),Some(returned.statement()));
            if value_at(&query,site,SsaVariableIdV1::new(barrier_output.index()),Event::Use,work)?
                !=value_at(&query,site,SsaVariableIdV1::new(barrier_output.index()),Event::Kill,work)? {
                return Err(rejected("phase SSA Finish changed its consumed advanced workgroup"));
            }
            let defined=value_at(&query,site,SsaVariableIdV1::new(returned.return_local().index()),Event::Define,work)?;
            if value_at(&query,site,SsaVariableIdV1::new(returned.return_local().index()),Event::Use,work)?!=defined {
                return Err(rejected("phase SSA completion return transfer substituted its definition"));
            }
            let completion=value_at(&query,site,SsaVariableIdV1::new(returned.destination().index()),Event::Define,work)?;
            let emission_finish=Boundary::at(&query,site,SsaVariableIdV1::new(returned.destination().index()),
                completion,BoundaryEvent::Define,work)?;
            if value_at(&query,Site::new(relay.pack_block(),Some(relay.pack_statement())),
                SsaVariableIdV1::new(relay.finish_result().index()),Event::Use,work)?!=completion {
                return Err(rejected("phase SSA erased pack substituted the actual Finish completion"));
            }
            let mut leases=reserve(phase.binds.len(),work)?;
            for bind in &phase.binds {
                let binding=&self.bindings[bind.call];
                if binding.caller_instance()!=phase.closure {return Err(rejected("phase SSA Bind belongs to another closure instance"));}
                let returned=result_row(&query,binding,work)?;
                let Inputs::Bind {phase_reference,storage_reference}=returned.inputs() else {return Err(rejected("phase SSA Bind lost both typed references"));};
                let site=Site::new(returned.block(),Some(returned.statement()));
                let SemanticDefinedReusablePhaseRecipeV1::Bind {phase_reference:reference_type,storage_reference:storage_type,..}=returned.record().recipe() else {
                    return Err(rejected("phase SSA Bind changed its typed recipe"));
                };
                let phase_reference_value=phase_argument::shared_bind_reference(&query,self.view,binding,
                    &closure_phase,phase_workgroup,reference_type.reference,work)?;
                if value_at(&query,site,SsaVariableIdV1::new(phase_reference.index()),Event::Use,work)?!=phase_reference_value {
                    return Err(rejected("phase SSA Bind consumed a different phase reference"));
                }
                let storage=&self.bindings[bind.storage_conversion];
                let mut allocation=None;
                for row in query.plan().defined_reusable_lds_results() {
                    spend(work,1)?;
                    if row.callee()!=storage.callee_instance() {continue;}
                    if storage.contract()!=SemanticDefinedCapabilityContractV1::ReusableLdsConversion(row.record())
                        || row.caller()!=wrapper.caller_instance() || allocation.replace(row).is_some() {
                        return Err(rejected("phase SSA reused a different allocation conversion"));
                    }
                }
                let allocation=allocation.ok_or_else(||rejected("phase SSA missing original reusable allocation result"))?;
                let allocation_value=value_at(&query,Site::new(allocation.return_block(),Some(allocation.return_statement())),
                    SsaVariableIdV1::new(allocation.destination().index()),Event::Define,work)?;
                let captured=allocation_capture(&self.bindings,self.view,&query,phase,bind,allocation.destination(),allocation_value,work)?;
                let wrapper_environment=boundaries::parameter_transfer(&query,self.view,wrapper,1,work)?;
                if wrapper_environment.input!=captured.environment {
                    return Err(rejected("phase wrapper substituted its exact allocation capture"));
                }
                let [_,environment]=wrapper.callee_arguments() else {return Err(rejected("phase wrapper lost its exact closure environment"));};
                let [_,SemanticOperandV1::Copy(storage_place)|SemanticOperandV1::Move(storage_place)]=binding.arguments() else {
                    return Err(rejected("phase Bind lost its retained storage reference"));
                };
                if storage_place.ty()!=storage_type.reference || !storage_place.projections().is_empty() {
                    return Err(rejected("phase Bind storage reference changed its exact type or place"));
                }
                let extracted=closure_capture::field_value(&query,self.view,closure_capture::Request {
                    closure_source:&self.source.semantic.functions()[source_relay.closure_function.index() as usize],
                    wrapper:wrapper.callee_instance(),closure:phase.closure,invoke_block,
                    environment:*environment,environment_value:wrapper_environment.output,
                    field:captured.field,destination:storage_place,
                },work)?;
                let storage_transfer=boundaries::parameter_transfer(&query,self.view,binding,1,work)?;
                if storage_transfer.input!=extracted
                    || value_at(&query,site,SsaVariableIdV1::new(storage_reference.index()),Event::Use,work)?!=storage_transfer.output {
                    return Err(rejected("phase Bind consumed a different captured storage reference"));
                }
                let result=returned_value(&query,site,binding,work)?;
                let close=boundaries::close_lease(&query,self.view,phase.closure,returned.destination(),result,site,work)?;
                let emission_definition=Boundary::at(&query,site,SsaVariableIdV1::new(returned.destination().index()),
                    result,BoundaryEvent::Define,work)?;
                let emission_close=Boundary::existing(&query,close,SsaVariableIdV1::new(returned.destination().index()),
                    result,BoundaryEvent::Kill,work)?;
                if !phase_order::precedes(&query,close,end,work)? {
                    return Err(rejected("phase End can bypass the exact dormant lease close"));
                }
                leases.push(Lease {allocation:allocation_value,root_reference:captured.reference,
                    phase_reference:phase_reference_value,
                    storage_reference:storage_transfer.output,
                    result,close,borrowed_at:captured.borrowed_at,emission_definition,emission_close});
            }
            phases.push(Phase {owner_workgroup,converted_owner,owner_reference,issued_phase,closure_phase:closure_phase.value,relay:relay_value,completion,leases,begin,end,
                emission_owner,emission_issue,emission_finish,emission_pack,emission_drop,emission_end});
        }
        if query.plan().defined_reusable_phase_relays().len()!=self.phases.len()
            || query.plan().defined_reusable_phase_results().len()!=expected_results {
            return Err(rejected("phase SSA omitted an expanded result or completion occurrence"));
        }
        for (index,phase) in phases.iter().enumerate() {
            for other in &phases[..index] {
                spend(work,1)?;
                if phase.converted_owner==other.converted_owner
                    && !phase_order::precedes(&query,phase.end,other.begin,work)?
                    && !phase_order::precedes(&query,other.end,phase.begin,work)? {
                    return Err(rejected("phase owner reused before its exact previous End"));
                }
                for lease in &phase.leases {
                    for prior in &other.leases {
                        spend(work,1)?;
                        if lease.allocation==prior.allocation
                            && !phase_order::precedes(&query,phase.end,prior.borrowed_at,work)?
                            && !phase_order::precedes(&query,other.end,lease.borrowed_at,work)? {
                            return Err(rejected("phase allocation borrowed again before its exact previous End"));
                        }
                    }
                }
            }
        }
        Ok(CheckedSsa {expansion:self,owner,phases})
    }
}

fn returned_value(query:&ProductionSemanticSsaSourceQueryV1<'_>,site:Site,
    binding:&SemanticExpandedDefinedCapabilityV1,work:&mut usize)->PhaseResult<SsaValueV1> {
    let returned=SsaVariableIdV1::new(binding.callee_return().index());
    let used=value_at(query,site,returned,Event::Use,work)?;
    if value_at(query,site,returned,Event::Kill,work)?!=used {
        return Err(rejected("phase return transfer lost its exact consumed source value"));
    }
    value_at(query,site,SsaVariableIdV1::new(binding.destination().local().index()),Event::Define,work)
}

#[derive(Clone,Copy)]
enum Event {Use,Define,Kill}
fn value_at(query:&ProductionSemanticSsaSourceQueryV1<'_>,site:Site,variable:SsaVariableIdV1,kind:Event,work:&mut usize)->PhaseResult<SsaValueV1> {
    let block=SsaBlockIdV1::new(site.block().index());
    let events=query.plan().plan().resolved_events(block).ok_or_else(||rejected("phase SSA site is unreachable"))?;
    let mut found=None;
    for (event,resolved) in events {
        spend(work,1)?;
        let value=match (kind,resolved) {
            (Event::Use,SsaResolvedEventV1::Use {variable:v,value})
                | (Event::Define,SsaResolvedEventV1::Define {variable:v,value}) if *v==variable=>Some(*value),
            (Event::Kill,SsaResolvedEventV1::Kill {variable:v,previous:Some(value)}) if *v==variable=>Some(*value),
            _=>None,
        };
        let Some(value)=value else {continue;};
        let actual=query.event_site(block,*event,&mut ||spend(work,1).is_ok())
            .map_err(|_|rejected("phase SSA event has no exact original site or exhausted work"))?;
        if actual!=site {continue;}
        if found.is_some_and(|old|old!=value) {return Err(rejected("phase SSA disagrees within one source site"));}
        found=Some(value);
    }
    found.ok_or_else(||rejected("phase SSA required value was not promoted at its exact source site"))
}
fn result_row<'a>(query:&ProductionSemanticSsaSourceQueryV1<'a>,binding:&SemanticExpandedDefinedCapabilityV1,
    work:&mut usize)->PhaseResult<&'a fe2o3_pliron::ProductionSemanticPhaseResultV1> {
    let mut found=None;
    for row in query.plan().defined_reusable_phase_results() {
        spend(work,1)?;
        if row.callee()!=binding.callee_instance() {continue;}
        if binding.contract()!=SemanticDefinedCapabilityContractV1::ReusablePhase(row.record())
            || row.caller()!=binding.caller_instance() || row.return_local()!=binding.callee_return()
            || row.destination()!=binding.destination().local() || found.replace(row).is_some() {
            return Err(rejected("phase SSA typed result occurrence substitution"));
        }
    }
    found.ok_or_else(||rejected("phase SSA missing exact defined result occurrence"))
}
fn exact_relay_roster(query:&ProductionSemanticSsaSourceQueryV1<'_>,variable:SsaVariableIdV1,work:&mut usize)->PhaseResult<()> {
    let mut counts=[0usize;3];
    for block in 0..query.function().blocks().len() {
        spend(work,1)?;
        let Some(events)=query.plan().plan().resolved_events(SsaBlockIdV1::new(block as u32)) else {continue;};
        for (_,event) in events {
            spend(work,1)?;
            let slot=match event {
                SsaResolvedEventV1::Define {variable:v,..} if *v==variable=>0,
                SsaResolvedEventV1::Use {variable:v,..} if *v==variable=>1,
                SsaResolvedEventV1::Kill {variable:v,..} if *v==variable=>2,
                _=>continue,
            };
            counts[slot]+=1;
        }
    }
    if counts!=[1,1,1] {return Err(rejected("phase SSA relay is not one linear definition/use/kill"));}
    Ok(())
}

struct AllocationCapture {
    reference:SsaValueV1,
    environment:SsaValueV1,
    field:u32,
    borrowed_at:linear_events::Point,
}
fn allocation_capture(bindings:&[SemanticExpandedDefinedCapabilityV1],view:&fe2o3_mir_model::SemanticExpandedRootV1,query:&ProductionSemanticSsaSourceQueryV1<'_>,
    phase:&super::expanded_protocol::Phase,bind:&super::expanded_protocol::Bind,
    allocation:SemanticLocalIdV1,allocation_value:SsaValueV1,work:&mut usize)->PhaseResult<AllocationCapture> {
    let wrapper=&bindings[phase.wrapper];
    let binding=&bindings[bind.call];
    let SemanticDefinedCapabilityContractV1::ReusablePhase(record)=binding.contract() else {return Err(rejected("phase capture Bind role"));};
    let SemanticDefinedReusablePhaseRecipeV1::Bind {storage_reference,..}=record.recipe() else {return Err(rejected("phase capture Bind type"));};
    let [_,SemanticOperandV1::Move(capture)]=wrapper.arguments() else {return Err(rejected("phase capture must be its original moved closure"));};
    let block=wrapper.expanded_call_block();
    let origin=&view.block_origins()[block.index() as usize];
    if origin.instance()!=wrapper.caller_instance() || origin.function()!=wrapper.caller_function()
        || !capture.projections().is_empty() {
        return Err(rejected("phase capture changed its original caller or whole environment"));
    }
    let mut borrowed=None;
    let mut captured=None;
    for (statement,item) in query.function().blocks()[block.index() as usize].statements().iter().enumerate() {
        spend(work,1)?;
        if !matches!(origin.statements().get(statement),Some(Origin::Source {..})) {continue;}
        let SemanticStatementKindV1::Assign(a)=item.kind() else {continue;};
        if matches!(a.value().kind(),SemanticRvalueKindV1::Borrow {kind:SemanticBorrowKindV1::Mutable,place}
            if place.local()==allocation && place.ty()==storage_reference.pointee && place.projections().is_empty()) {
            if a.destination().ty()!=storage_reference.reference || !a.destination().projections().is_empty()
                || borrowed.replace((statement as u32,a.destination().local())).is_some() {
                return Err(rejected("phase capture allocation borrow is ambiguous or substituted"));
            }
        }
        if a.destination()==capture {
            let SemanticRvalueKindV1::Aggregate(aggregate)=a.value().kind() else {return Err(rejected("phase capture has no original closure aggregate"));};
            if captured.replace((statement as u32,aggregate)).is_some() {return Err(rejected("phase capture closure was redefined"));}
        }
    }
    let (borrow_site,reference)=borrowed.ok_or_else(||rejected("phase capture has no exact mutable allocation borrow"))?;
    let (capture_site,aggregate)=captured.ok_or_else(||rejected("phase capture has no exact closure construction"))?;
    let borrow_site=Site::new(block,Some(borrow_site));
    if value_at(query,borrow_site,SsaVariableIdV1::new(allocation.index()),Event::Use,work)?!=allocation_value {
        return Err(rejected("phase capture changed or killed the owned allocation SSA value"));
    }
    let reference_value=value_at(query,borrow_site,SsaVariableIdV1::new(reference.index()),Event::Define,work)?;
    let mut selected=None;
    for (field,operand) in aggregate.operands().iter().enumerate() {
        spend(work,1)?;
        if matches!(operand,SemanticOperandV1::Copy(p)|SemanticOperandV1::Move(p)
            if p.local()==reference && p.ty()==storage_reference.reference && p.projections().is_empty()) {
            if selected.replace(field).is_some() {
                return Err(rejected("phase closure duplicated its unique storage reference"));
            }
        }
    }
    let field=selected.ok_or_else(||rejected("phase closure omitted its unique storage reference"))?;
    let field=u32::try_from(field).map_err(|_|rejected("phase capture field index overflow"))?;
    let capture_site=Site::new(block,Some(capture_site));
    if value_at(query,capture_site,SsaVariableIdV1::new(reference.index()),Event::Use,work)?!=reference_value {
        return Err(rejected("phase closure substituted or duplicated its unique storage reference"));
    }
    let environment=value_at(query,capture_site,SsaVariableIdV1::new(capture.local().index()),Event::Define,work)?;
    let borrowed_at=boundaries::event_point(query,borrow_site,SsaVariableIdV1::new(reference.index()),reference_value,Event::Define,work)?;
    Ok(AllocationCapture {reference:reference_value,environment,field,borrowed_at})
}
