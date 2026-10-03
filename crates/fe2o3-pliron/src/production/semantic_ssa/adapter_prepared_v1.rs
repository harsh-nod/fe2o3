//! Transient classified state shared by real and numeric emission outputs.

use super::emission_v1::{
    SemanticSsaEmissionErrorV1 as EmissionError, SemanticSsaEmissionObserverV1,
    SemanticSsaEmissionSiteV1 as Site, SemanticSsaEntryOriginV1 as EntryOrigin,
    SemanticSsaEventBufferV1, SemanticSsaVisitV1 as Visit, emit_statement_events_with_buffer_v1,
    emit_terminator_events_with_buffer_v1, observer_error_v1,
};
use super::*;
use std::convert::Infallible;

pub(in crate::production::semantic_ssa) trait SemanticSsaBlockOutputV1 {
    type Error;
    type Events: SemanticSsaEventBufferV1<Error = Self::Error>;

    fn begin_block(&mut self) -> Result<Self::Events, Self::Error>;
    fn begin_successors(&mut self, capacity: usize) -> Result<(), Self::Error>;
    fn successor_count(&self) -> usize;
    fn push_successor(
        &mut self,
        role: SsaEdgeRoleV1,
        target: SsaBlockIdV1,
        definition: Option<SsaVariableIdV1>,
    ) -> Result<(), Self::Error>;
    fn finish_block(&mut self, events: Self::Events) -> Result<(), Self::Error>;
}

pub(in crate::production::semantic_ssa) trait SemanticSsaEntryOutputV1 {
    type Error;

    fn entry_count(&self) -> usize;
    fn push_entry(&mut self, variable: SsaVariableIdV1) -> Result<(), Self::Error>;
}

pub(in crate::production::semantic_ssa) struct PreparedSemanticSsaAdapterV1<'a> {
    function: &'a SemanticFunctionDeclV1,
    types: Option<&'a [SemanticTypeDeclV1]>,
    callables: &'a [SemanticCallableDeclV1],
    transparent_borrows: &'a BTreeSet<SemanticTransparentBorrowSiteV1>,
    promotable: Vec<bool>,
    elided_borrows: BTreeSet<SemanticTransparentBorrowSiteV1>,
    return_local: Option<usize>,
    analysis_work: usize,
}

pub(in crate::production::semantic_ssa) struct PreparedSemanticSsaEntriesV1<'a> {
    function: &'a SemanticFunctionDeclV1,
    promotable: Vec<bool>,
    blocks: Vec<SsaBlockInputV1>,
    implicit: Vec<SsaVariableIdV1>,
    analysis_work: usize,
}

pub(in crate::production::semantic_ssa) fn prepare_semantic_ssa_adapter_with_observer_v1<
    'a,
    O: SemanticSsaEmissionObserverV1,
>(
    function: &'a SemanticFunctionDeclV1,
    types: Option<&'a [SemanticTypeDeclV1]>,
    callables: &'a [SemanticCallableDeclV1],
    transparent_borrows: &'a BTreeSet<SemanticTransparentBorrowSiteV1>,
    observer: &mut O,
) -> Result<PreparedSemanticSsaAdapterV1<'a>, O::Error> {
    observer.visit(Visit::Function, Site::Function)?;
    let mut promotable = vec![true; function.locals().len()];
    classify_storage_observable_locals_v1(function, transparent_borrows, &mut promotable);
    let (elided_borrows, analysis_work) = authenticated_elided_grid_leader_borrow_sites_v1(
        function,
        types,
        callables,
        transparent_borrows,
    );
    let return_local = (!matches!(
        function.abi().return_value().mode(),
        SemanticAbiPassModeV1::Ignore,
    ))
    .then(|| {
        function
            .locals()
            .iter()
            .position(|declaration| matches!(declaration.role(), SemanticLocalRoleV1::Return))
    })
    .flatten();
    Ok(PreparedSemanticSsaAdapterV1 {
        function,
        types,
        callables,
        transparent_borrows,
        promotable,
        elided_borrows,
        return_local,
        analysis_work,
    })
}

impl<'a> PreparedSemanticSsaAdapterV1<'a> {
    pub(in crate::production::semantic_ssa) fn emit_blocks<S, O>(
        &self,
        output: &mut S,
        observer: &mut O,
    ) -> Result<(), EmissionError<O::Error, S::Error>>
    where
        S: SemanticSsaBlockOutputV1,
        O: SemanticSsaEmissionObserverV1,
    {
        observer
            .block_pass_begin(self.elided_borrows.len())
            .map_err(EmissionError::Observer)?;
        let mut elided = self.elided_borrows.iter().peekable();
        for (block_index, block) in self.function.blocks().iter().enumerate() {
            observer
                .visit(Visit::Block, Site::Block(block_index))
                .map_err(EmissionError::Observer)?;
            let mut events = output.begin_block().map_err(EmissionError::Output)?;
            for (statement_index, statement) in block.statements().iter().enumerate() {
                let site = SemanticTransparentBorrowSiteV1 {
                    block: block_index as u32,
                    statement: statement_index as u32,
                };
                let emission_site = Site::Statement {
                    block: block_index,
                    statement: statement_index,
                };
                observer
                    .statement_elision_lookup(emission_site, self.elided_borrows.len())
                    .map_err(EmissionError::Observer)?;
                let is_elided = match elided.peek() {
                    Some(candidate) => match (**candidate).cmp(&site) {
                        std::cmp::Ordering::Less => {
                            unreachable!("classified elision precedes its source site")
                        }
                        std::cmp::Ordering::Equal => {
                            elided.next();
                            true
                        }
                        std::cmp::Ordering::Greater => false,
                    },
                    None => false,
                };
                emit_statement_events_with_buffer_v1(
                    statement.kind(),
                    is_elided,
                    emission_site,
                    &mut events,
                    observer,
                )?;
            }
            emit_terminator_events_with_buffer_v1(
                block.terminator().kind(),
                self.return_local,
                Site::Terminator { block: block_index },
                &mut events,
                observer,
            )?;
            output
                .begin_successors(block.terminator().kind().edge_count())
                .map_err(EmissionError::Output)?;
            block.terminator().kind().try_for_each_edge(|edge| {
                let ordinal = output.successor_count();
                observer
                    .successor(block_index, ordinal, edge)
                    .map_err(EmissionError::Observer)?;
                let definition = call_edge_definition_v1(block.terminator().kind(), edge);
                if let Some(variable) = definition {
                    observer
                        .edge_definition(block_index, ordinal, edge, 0, variable)
                        .map_err(EmissionError::Observer)?;
                }
                output
                    .push_successor(
                        SsaEdgeRoleV1::new(semantic_edge_role_v1(edge.role())),
                        SsaBlockIdV1::new(edge.target().index()),
                        definition,
                    )
                    .map_err(EmissionError::Output)
            })?;
            observer
                .block_complete(block_index, events.event_count(), output.successor_count())
                .map_err(EmissionError::Observer)?;
            // Classifier sites originate in this exact ordered source traversal.
            assert!(
                elided
                    .peek()
                    .is_none_or(|site| site.block as usize > block_index)
            );
            output.finish_block(events).map_err(EmissionError::Output)?;
        }
        assert!(
            elided.next().is_none(),
            "classified elision is outside its source function"
        );
        Ok(())
    }

    pub(in crate::production::semantic_ssa) fn into_entries<O: SemanticSsaEmissionObserverV1>(
        self,
        observer: &mut O,
    ) -> Result<PreparedSemanticSsaEntriesV1<'a>, O::Error> {
        let mut output = RealBlocksV1 {
            blocks: Vec::with_capacity(self.function.blocks().len()),
            successors: Vec::new(),
        };
        self.emit_blocks(&mut output, observer)
            .map_err(observer_error_v1)?;
        let implicit = authenticated_implicit_entry_variables_v1(
            self.function,
            self.types,
            self.callables,
            self.transparent_borrows,
            &self.promotable,
            &output.blocks,
        );
        Ok(PreparedSemanticSsaEntriesV1 {
            function: self.function,
            promotable: self.promotable,
            blocks: output.blocks,
            implicit,
            analysis_work: self.analysis_work,
        })
    }
}

impl PreparedSemanticSsaEntriesV1<'_> {
    pub(in crate::production::semantic_ssa) fn emit_entries<S, O>(
        &self,
        output: &mut S,
        observer: &mut O,
    ) -> Result<(), EmissionError<O::Error, S::Error>>
    where
        S: SemanticSsaEntryOutputV1,
        O: SemanticSsaEmissionObserverV1,
    {
        observer
            .entry_pass_begin(self.function.locals().len(), self.implicit.len())
            .map_err(EmissionError::Observer)?;
        let mut implicit = self.implicit.iter().peekable();
        for (local, declaration) in self.function.locals().iter().enumerate() {
            observer
                .visit(Visit::EntryCandidate, Site::Local(local))
                .map_err(EmissionError::Observer)?;
            let is_implicit = match implicit.peek() {
                Some(variable) => match (variable.get() as usize).cmp(&local) {
                    std::cmp::Ordering::Less => {
                        unreachable!("implicit entry IDs are not strictly ordered")
                    }
                    std::cmp::Ordering::Equal => {
                        implicit.next();
                        true
                    }
                    std::cmp::Ordering::Greater => false,
                },
                None => false,
            };
            let origin = match declaration.role() {
                SemanticLocalRoleV1::Argument(argument) => Some(EntryOrigin::Argument(argument)),
                _ if is_implicit => Some(EntryOrigin::ImplicitCapability),
                _ => None,
            };
            if let Some(origin) = origin {
                let variable = SsaVariableIdV1::new(local as u32);
                observer
                    .entry_definition(output.entry_count(), variable, origin)
                    .map_err(EmissionError::Observer)?;
                output.push_entry(variable).map_err(EmissionError::Output)?;
            }
        }
        observer
            .input_complete(self.blocks.len(), output.entry_count())
            .map_err(EmissionError::Observer)?;
        assert!(
            implicit.next().is_none(),
            "implicit entry ID is outside its source function"
        );
        Ok(())
    }

    pub(in crate::production::semantic_ssa) fn finish<O: SemanticSsaEmissionObserverV1>(
        self,
        observer: &mut O,
    ) -> Result<(SsaConstructionInputV1, Vec<SsaVariableIdV1>, usize), O::Error> {
        let mut entries = Vec::new();
        self.emit_entries(&mut entries, observer)
            .map_err(observer_error_v1)?;
        Ok((
            SsaConstructionInputV1::new(
                SsaBlockIdV1::new(self.function.entry().index()),
                self.function.locals().len() as u32,
                self.promotable,
                entries,
                self.blocks,
            ),
            self.implicit,
            self.analysis_work,
        ))
    }
}

struct RealBlocksV1 {
    blocks: Vec<SsaBlockInputV1>,
    successors: Vec<SsaEdgeInputV1>,
}

impl SemanticSsaBlockOutputV1 for RealBlocksV1 {
    type Error = Infallible;
    type Events = Vec<SsaEventV1>;

    fn begin_block(&mut self) -> Result<Self::Events, Infallible> {
        Ok(Vec::new())
    }
    fn begin_successors(&mut self, capacity: usize) -> Result<(), Infallible> {
        self.successors = Vec::with_capacity(capacity);
        Ok(())
    }
    fn successor_count(&self) -> usize {
        self.successors.len()
    }
    fn push_successor(
        &mut self,
        role: SsaEdgeRoleV1,
        target: SsaBlockIdV1,
        definition: Option<SsaVariableIdV1>,
    ) -> Result<(), Infallible> {
        let definitions = match definition {
            Some(variable) => vec![variable],
            None => Vec::new(),
        };
        self.successors
            .push(SsaEdgeInputV1::new(role, target, definitions));
        Ok(())
    }
    fn finish_block(&mut self, events: Self::Events) -> Result<(), Infallible> {
        self.blocks.push(SsaBlockInputV1::new(
            events,
            std::mem::take(&mut self.successors),
        ));
        Ok(())
    }
}

impl SemanticSsaEntryOutputV1 for Vec<SsaVariableIdV1> {
    type Error = Infallible;

    fn entry_count(&self) -> usize {
        self.len()
    }
    fn push_entry(&mut self, variable: SsaVariableIdV1) -> Result<(), Infallible> {
        self.push(variable);
        Ok(())
    }
}
