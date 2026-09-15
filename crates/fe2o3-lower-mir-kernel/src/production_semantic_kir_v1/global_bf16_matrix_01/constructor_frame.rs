// Replayed call-frame coordinates only. Provider authentication and the
// checked Result/extent theorem must still validate this constructor pair.
use fe2o3_mir_model::{
    SemanticCallInstanceIdV1, SemanticExpandedStatementOriginV1, SemanticExpandedTerminatorOriginV1,
};

#[path = "constructor_forwarder.rs"]
mod constructor_forwarder;

#[path = "constructor_forwarder_observation.rs"]
mod constructor_forwarder_observation;

pub(super) struct GlobalBf16ConstructorFrameV1<'a> {
    owner: &'a ProductionSemanticSsaOwnerV1,
    view: &'a SemanticExpandedRootV1,
    wrapper: SemanticCallInstanceIdV1,
    checked: SemanticCallInstanceIdV1,
    receiver: GlobalBf16SourceOperandV1<'a>,
}

impl<'a> GlobalBf16SourceUseV1<'a> {
    fn constructor_frame(
        &self,
        query: &mut GlobalBf16SourceQueryV1<'_, 'a>,
    ) -> Result<GlobalBf16ConstructorFrameV1<'a>, ProductionSemanticKirErrorV1> {
        if !std::ptr::eq(self.view.body(), query.view.body())
            || self.owner.source_semantic().semantic_sha256()
                != query.owner.source_semantic().semantic_sha256()
            || self.owner.execution_expansion().identity()
                != query.owner.execution_expansion().identity()
            || self.view.root() != query.view.root()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let view = self.view;
        let construction = self.inputs.construction;
        let origin = view
            .block_origins()
            .get(construction.block as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let statement = construction
            .statement
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if !matches!(
            origin.statements().get(statement as usize),
            Some(SemanticExpandedStatementOriginV1::Source { .. })
        ) {
            return Err(reject(
                "global BF16 aggregate is not an original constructor statement",
            ));
        }
        let checked_id = origin.instance();
        let checked = view
            .instances()
            .get(checked_id.index() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if checked.function() != origin.function() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let wrapper_id = checked
            .parent()
            .ok_or_else(|| reject("global BF16 construction has no actual wrapper instance"))?;
        let wrapper = view
            .instances()
            .get(wrapper_id.index() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let source = self.owner.source_semantic();
        let body = source
            .functions()
            .get(wrapper.function().index() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let work = body
            .locals()
            .len()
            .checked_add(body.blocks().len())
            .and_then(|n| n.checked_add(body.abi().source_input_types().len()))
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        query.graph.charge(work)?;
        let Some(receiver_local) = constructor_forwarder::exact_forwarder_at(
            body, source.callables(), checked.function(), checked.call_block(),
        ) else {
            constructor_forwarder_observation::emit(
                body,
                source.callables(),
                checked.function(),
                checked.call_block(),
                &constructor_forwarder_observation::Context {
                    root: view.root().index(),
                    view: *view.identity(),
                    expanded_body: *view.body().identity().as_bytes(),
                    wrapper: (wrapper_id.index(), wrapper.function().index()),
                    checked_instance: checked_id.index(),
                    construction: (construction.block, construction.statement, construction.local),
                },
            );
            return Err(reject(
                "global BF16 constructor lost its exact original forwarding wrapper",
            ));
        };
        let caller_id = wrapper
            .parent()
            .ok_or_else(|| reject("global BF16 wrapper lacks a source caller"))?;
        let caller = view
            .instances()
            .get(caller_id.index() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let original_call = wrapper
            .call_block()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let expanded_call = caller
            .block_start()
            .checked_add(original_call.index())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let call_origin = view
            .block_origins()
            .get(expanded_call as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let actual = view
            .body()
            .blocks()
            .get(expanded_call as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if call_origin.instance() != caller_id
            || call_origin.function() != caller.function()
            || call_origin.block() != original_call
            || call_origin.terminator()
                != (SemanticExpandedTerminatorOriginV1::CallEntry { callee: wrapper_id })
            || call_origin.statements().len() != actual.statements().len()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let receiver_local = wrapper
            .local_start()
            .checked_add(receiver_local.index())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let mut receiver = None;
        query.graph.charge(actual.statements().len())?;
        for (index, (statement, origin)) in actual
            .statements()
            .iter()
            .zip(call_origin.statements())
            .enumerate()
        {
            if *origin
                != (SemanticExpandedStatementOriginV1::ParameterTransfer {
                    callee: wrapper_id,
                    argument: 0,
                })
            {
                continue;
            }
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            let SemanticRvalueKindV1::Use(operand) = assignment.value().kind() else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            if assignment.destination().local().index() != receiver_local
                || !assignment.destination().projections().is_empty()
                || assignment.destination().ty() != body.abi().source_input_types()[0]
                || operand.ty() != assignment.destination().ty()
                || !matches!(
                    operand,
                    SemanticOperandV1::Copy(_) | SemanticOperandV1::Move(_)
                )
                || receiver
                    .replace(GlobalBf16SourceOperandV1 {
                        site: CapabilityDefinitionSiteV1 {
                            block: expanded_call,
                            statement: Some(index as u32),
                            local: receiver_local,
                        },
                        operand,
                    })
                    .is_some()
            {
                return Err(reject("global BF16 wrapper receiver transfer changed"));
            }
        }
        let receiver =
            receiver.ok_or_else(|| reject("global BF16 wrapper receiver transfer is missing"))?;
        query.graph.charge(
            std::mem::size_of::<GlobalBf16ConstructorFrameV1<'_>>()
                .div_ceil(std::mem::size_of::<usize>()),
        )?;
        Ok(GlobalBf16ConstructorFrameV1 {
            owner: self.owner,
            view,
            wrapper: wrapper_id,
            checked: checked_id,
            receiver,
        })
    }
}

impl<'a> GlobalBf16ConstructorFrameV1<'a> {
    pub(super) fn owner(&self) -> &'a ProductionSemanticSsaOwnerV1 {
        self.owner
    }
    pub(super) fn view(&self) -> &'a SemanticExpandedRootV1 {
        self.view
    }
    pub(super) fn wrapper(&self) -> SemanticCallInstanceIdV1 {
        self.wrapper
    }
    pub(super) fn checked(&self) -> SemanticCallInstanceIdV1 {
        self.checked
    }
    pub(super) fn receiver(&self) -> GlobalBf16SourceOperandV1<'a> {
        self.receiver
    }
}
