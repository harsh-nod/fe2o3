use super::*;

/// A source-certified ambient initialization after one exact execution storage kill.
/// Constructed only while replaying original source plans against a checked view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSemanticSsaFrameInitializationV1 {
    block: SemanticBlockIdV1,
    statement: u32,
    local: SemanticLocalIdV1,
    origin: SemanticExpandedLocalOriginV1,
}

impl ProductionSemanticSsaFrameInitializationV1 {
    pub const fn block(&self) -> SemanticBlockIdV1 {
        self.block
    }
    pub const fn statement(&self) -> u32 {
        self.statement
    }
    pub const fn local(&self) -> SemanticLocalIdV1 {
        self.local
    }
    pub const fn origin(&self) -> SemanticExpandedLocalOriginV1 {
        self.origin
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct FrameInitializationsV1 {
    view_identity: Option<[u8; 32]>,
    entries: Box<[ProductionSemanticSsaFrameInitializationV1]>,
    resources: SemanticSsaAuxiliaryResourcesV1,
}

impl FrameInitializationsV1 {
    pub(super) fn derive(
        semantic: &AdmittedInertSemanticMirV1,
        view: &SemanticExpandedRootV1,
        source_plans: &[ProductionSemanticSsaFunctionPlanV1],
        limits: ProductionSemanticSsaLimitsV1,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        let mismatch = || ProductionSemanticSsaErrorV1::ReplayMismatch;
        let source_plan = |function: SemanticFunctionIdV1| {
            let original = semantic
                .functions()
                .get(function.index() as usize)
                .ok_or_else(mismatch)?;
            source_plans
                .get(function.index() as usize)
                .filter(|plan| {
                    plan.function() == function && plan.function_identity() == original.identity()
                })
                .ok_or_else(mismatch)
        };
        let count = view
            .instances()
            .iter()
            .skip(1)
            .try_fold(0_usize, |count, instance| {
                count
                    .checked_add(
                        source_plan(instance.function())?
                            .implicit_entry_variables()
                            .len(),
                    )
                    .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)
            })?;
        if count == 0 {
            let resources = SemanticSsaAuxiliaryResourcesV1 {
                storage_words: 0,
                work_units: view
                    .instances()
                    .len()
                    .checked_add(128)
                    .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?,
            };
            enforce_function_resource_limit_v1(view.source_body(), resources, limits)?;
            return Ok(Self {
                view_identity: Some(*view.identity()),
                resources,
                ..Self::default()
            });
        }
        let statements = view
            .body()
            .blocks()
            .iter()
            .try_fold(0_usize, |count, block| {
                count
                    .checked_add(block.statements().len())
                    .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)
            })?;
        // Dense expected/seen tables avoid an unbounded search per call instance.
        // Reserve relation rows, validation/hash work, and vector growth slack.
        let storage_words = view
            .body()
            .locals()
            .len()
            .checked_mul(4)
            .and_then(|words| words.checked_add(count.checked_mul(8)?))
            .and_then(|words| words.checked_add(16))
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        let work_units = view
            .body()
            .locals()
            .len()
            .checked_add(view.instances().len())
            .and_then(|work| work.checked_add(view.body().blocks().len()))
            .and_then(|work| work.checked_add(statements))
            .and_then(|work| work.checked_add(count.checked_mul(128)?))
            .and_then(|work| work.checked_add(128))
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        let resources = SemanticSsaAuxiliaryResourcesV1 {
            storage_words,
            work_units,
        };
        enforce_function_resource_limit_v1(view.source_body(), resources, limits)?;

        let mut expected = vec![None; view.body().locals().len()];
        for (index, instance) in view.instances().iter().enumerate().skip(1) {
            let plan = source_plan(instance.function())?;
            for variable in plan.implicit_entry_variables() {
                let local = instance
                    .local_start()
                    .checked_add(variable.get())
                    .ok_or_else(mismatch)?;
                let origin = *view
                    .local_origins()
                    .get(local as usize)
                    .ok_or_else(mismatch)?;
                let source = semantic.functions()[instance.function().index() as usize]
                    .locals()
                    .get(variable.get() as usize)
                    .ok_or_else(mismatch)?;
                let declaration = view
                    .body()
                    .locals()
                    .get(local as usize)
                    .ok_or_else(mismatch)?;
                if variable.get() >= instance.local_count()
                    || origin.instance().index() as usize != index
                    || origin.function() != instance.function()
                    || origin.local().index() != variable.get()
                    || source.role() != SemanticLocalRoleV1::Temporary
                    || declaration.role() != SemanticLocalRoleV1::Temporary
                    || source.ty() != declaration.ty()
                    || expected[local as usize].replace(origin).is_some()
                {
                    return Err(mismatch());
                }
            }
        }
        let mut entries = Vec::with_capacity(count);
        let mut seen = vec![false; expected.len()];
        for (block, origin) in view.block_origins().iter().enumerate() {
            for (statement, marker) in origin.statements().iter().enumerate() {
                let SemanticExpandedStatementOriginV1::FrameStorageLive { callee, local } = marker
                else {
                    continue;
                };
                let instance = view
                    .instances()
                    .get(callee.index() as usize)
                    .ok_or_else(mismatch)?;
                let expanded = instance
                    .local_start()
                    .checked_add(local.index())
                    .ok_or_else(mismatch)?;
                let Some(expected) = expected.get(expanded as usize).ok_or_else(mismatch)? else {
                    continue;
                };
                if seen[expanded as usize] {
                    return Err(mismatch());
                }
                seen[expanded as usize] = true;
                entries.push(ProductionSemanticSsaFrameInitializationV1 {
                    block: SemanticBlockIdV1::from_index(block as u32),
                    statement: statement as u32,
                    local: SemanticLocalIdV1::from_index(expanded),
                    origin: *expected,
                });
            }
        }
        if entries.len() != count {
            return Err(mismatch());
        }
        let relation = Self {
            view_identity: Some(*view.identity()),
            entries: entries.into_boxed_slice(),
            resources,
        };
        relation.verify_view(view)?;
        Ok(relation)
    }

    pub(super) fn entries(&self) -> &[ProductionSemanticSsaFrameInitializationV1] {
        &self.entries
    }
    pub(super) const fn resources(&self) -> SemanticSsaAuxiliaryResourcesV1 {
        self.resources
    }

    pub(super) fn verify_view(
        &self,
        view: &SemanticExpandedRootV1,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        if self.view_identity != Some(*view.identity()) {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        self.verify_markers(view.body())?;
        for entry in &self.entries {
            let origin = view
                .block_origins()
                .get(entry.block.index() as usize)
                .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
            let frame = view
                .instances()
                .get(entry.origin.instance().index() as usize)
                .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
            if view.local_origins().get(entry.local.index() as usize) != Some(&entry.origin)
                || frame.parent() != Some(origin.instance())
                || frame.call_block() != Some(origin.block())
                || origin.statements().get(entry.statement as usize)
                    != Some(&SemanticExpandedStatementOriginV1::FrameStorageLive {
                        callee: entry.origin.instance(),
                        local: entry.origin.local(),
                    })
            {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            }
        }
        Ok(())
    }

    pub(super) fn verify_markers(
        &self,
        function: &SemanticFunctionDeclV1,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        let mut previous = None;
        for entry in &self.entries {
            let site = (entry.block.index(), entry.statement);
            if previous.is_some_and(|previous| previous >= site)
                || !matches!(function.blocks().get(site.0 as usize)
                    .and_then(|block| block.statements().get(site.1 as usize)).map(|statement| statement.kind()),
                    Some(SemanticStatementKindV1::StorageLive(local)) if *local == entry.local)
            {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            }
            previous = Some(site);
        }
        Ok(())
    }

    pub(super) fn hash_into(&self, digest: &mut Sha256) {
        digest.update(b"fe2o3.execution-frame-initializations.v1\0");
        digest.update([u8::from(self.view_identity.is_some())]);
        if let Some(identity) = self.view_identity {
            digest.update(identity);
        }
        digest.update((self.entries.len() as u64).to_le_bytes());
        for entry in &self.entries {
            for value in [
                entry.block.index(),
                entry.statement,
                entry.local.index(),
                entry.origin.instance().index(),
                entry.origin.function().index(),
                entry.origin.local().index(),
            ] {
                digest.update(value.to_le_bytes());
            }
        }
    }
}

#[cfg(test)]
pub(super) mod tests;
