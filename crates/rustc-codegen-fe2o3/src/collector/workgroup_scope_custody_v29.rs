//! Private constructor anchors. These describe source occurrences, not call
//! instances, capability issuance, lifecycle proofs or executable authority.

use crate::production_semantic_body_v1::ProductionSemanticBodyErrorV1 as Error;
use fe2o3_mir_model::semantic_mir_v1::*;

fn mismatch() -> Error {
    Error::IdentityTableMismatch {
        table: "authenticated workgroup scope custody",
    }
}

fn allocation() -> Error {
    Error::Allocation {
        resource: SemanticMirResourceV1::ValidationWork,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScopeCallableV29 {
    Ordinary,
    Provider {
        function: SemanticFunctionIdV1,
        identity: SemanticFunctionIdentityV1,
    },
    Derive {
        binding: SemanticFunctionIdentityV1,
        operation: SemanticCompilerIntrinsicIdentityV1,
        context: SemanticTypeIdV1,
        workgroup: SemanticTypeIdV1,
    },
}

impl ScopeCallableV29 {
    pub(crate) fn derive_from_constructed(
        callable: &SemanticCallableDeclV1,
    ) -> Result<Self, Error> {
        match callable {
            SemanticCallableDeclV1::CompilerIntrinsic {
                binding,
                operation:
                    SemanticCompilerIntrinsicOperationV1::Execution(
                        SemanticExecutionOperationV29::WorkgroupDerive { context, workgroup },
                    ),
                operation_identity,
            } => Ok(Self::Derive {
                binding: binding.identity(),
                operation: *operation_identity,
                context: *context,
                workgroup: *workgroup,
            }),
            _ => Err(mismatch()),
        }
    }

    fn check(self, index: usize, semantic: &AdmittedInertSemanticMirV1) -> Result<(), Error> {
        let actual = semantic.callables().get(index).ok_or_else(mismatch)?;
        match self {
            Self::Ordinary => {
                if matches!(
                    actual,
                    SemanticCallableDeclV1::CompilerIntrinsic {
                        operation: SemanticCompilerIntrinsicOperationV1::Execution(
                            SemanticExecutionOperationV29::WorkgroupDerive { .. }
                        ),
                        ..
                    }
                ) {
                    return Err(mismatch());
                }
            }
            Self::Provider { function, identity } => {
                let body = semantic
                    .functions()
                    .get(function.index() as usize)
                    .ok_or_else(mismatch)?;
                if function.index() as usize != index
                    || body.identity() != identity
                    || body.role() != SemanticFunctionRoleV1::InternalHelper
                    || body.kernel_entry().is_some()
                    || !matches!(actual, SemanticCallableDeclV1::Defined { function: found } if *found == function)
                {
                    return Err(mismatch());
                }
            }
            expected @ Self::Derive { .. } => {
                if Self::derive_from_constructed(actual)? != expected {
                    return Err(mismatch());
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScopeCallKindV29 {
    Ordinary,
    Provider,
    Derive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScopeEventKindV29 {
    Call {
        callee: SemanticCallableIdV1,
        kind: ScopeCallKindV29,
    },
    Return,
    Assert,
    Unreachable,
    UnwindResume,
    UnwindTerminate,
    Abort,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ScopeEventV29 {
    pub(crate) function: SemanticFunctionIdV1,
    pub(crate) block: SemanticBlockIdV1,
    pub(crate) statement_count: usize,
    pub(crate) kind: ScopeEventKindV29,
}

fn event_kind(
    classes: &[ScopeCallableV29],
    function: SemanticFunctionIdV1,
    terminator: &SemanticTerminatorKindV1,
) -> Result<Option<ScopeEventKindV29>, Error> {
    let provider = matches!(
        classes
            .get(function.index() as usize)
            .ok_or_else(mismatch)?,
        ScopeCallableV29::Provider { .. }
    );
    let (callee, tail) = match terminator {
        SemanticTerminatorKindV1::Call(call) => (call.callee(), false),
        SemanticTerminatorKindV1::TailCall(call) => (call.callee(), true),
        SemanticTerminatorKindV1::Return if provider => return Ok(Some(ScopeEventKindV29::Return)),
        SemanticTerminatorKindV1::Assert { .. } if provider => {
            return Ok(Some(ScopeEventKindV29::Assert));
        }
        SemanticTerminatorKindV1::Unreachable if provider => {
            return Ok(Some(ScopeEventKindV29::Unreachable));
        }
        SemanticTerminatorKindV1::UnwindResume if provider => {
            return Ok(Some(ScopeEventKindV29::UnwindResume));
        }
        SemanticTerminatorKindV1::UnwindTerminate if provider => {
            return Ok(Some(ScopeEventKindV29::UnwindTerminate));
        }
        SemanticTerminatorKindV1::Abort if provider => return Ok(Some(ScopeEventKindV29::Abort)),
        SemanticTerminatorKindV1::Drop { .. } | SemanticTerminatorKindV1::FalseEdge { .. }
            if provider =>
        {
            return Err(mismatch());
        }
        _ => return Ok(None),
    };
    let class = classes.get(callee.index() as usize).ok_or_else(mismatch)?;
    let kind = match class {
        ScopeCallableV29::Provider { .. } => ScopeCallKindV29::Provider,
        ScopeCallableV29::Derive { .. } if provider => ScopeCallKindV29::Derive,
        ScopeCallableV29::Derive { .. } => return Err(mismatch()),
        ScopeCallableV29::Ordinary if provider => ScopeCallKindV29::Ordinary,
        ScopeCallableV29::Ordinary => return Ok(None),
    };
    if tail {
        return Err(mismatch());
    }
    // Destination, unwind and complete payload stay in the committed function.
    // A call without a destination is not a normal scope exit.
    Ok(Some(ScopeEventKindV29::Call { callee, kind }))
}

pub(crate) struct PendingWorkgroupScopesV29 {
    classes: Vec<ScopeCallableV29>,
    declarations: SemanticDeclarationTablesCommitmentV1,
    target: SemanticTargetDataLayoutV1,
    function_count: usize,
    completed: usize,
    events: Vec<ScopeEventV29>,
}

pub(crate) struct PreparedScopeEventsV29<'a> {
    owner: &'a mut PendingWorkgroupScopesV29,
    events: Vec<ScopeEventV29>,
}

impl PreparedScopeEventsV29<'_> {
    pub(crate) fn publish(mut self) {
        self.owner.events.append(&mut self.events);
        self.owner.completed += 1;
    }
}

impl PendingWorkgroupScopesV29 {
    pub(crate) fn new(
        classes: Vec<ScopeCallableV29>,
        declarations: SemanticDeclarationTablesCommitmentV1,
        target: SemanticTargetDataLayoutV1,
        function_count: usize,
    ) -> Result<Self, Error> {
        if function_count == 0 || function_count > classes.len() {
            return Err(mismatch());
        }
        Ok(Self {
            classes,
            declarations,
            target,
            function_count,
            completed: 0,
            events: Vec::new(),
        })
    }

    pub(crate) fn capture(
        &self,
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        statement_count: usize,
        terminator: &SemanticTerminatorKindV1,
        events: &mut Vec<ScopeEventV29>,
        mut charge: impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<(), Error> {
        charge(1)?;
        if function.index() as usize != self.completed || self.completed >= self.function_count {
            return Err(mismatch());
        }
        let Some(kind) = event_kind(&self.classes, function, terminator)? else {
            return Ok(());
        };
        if events.last().is_some_and(|last| last.block >= block) {
            return Err(mismatch());
        }
        if events.len() == events.capacity() {
            charge(events.len().checked_add(1).ok_or_else(mismatch)?)?;
            events.try_reserve(1).map_err(|_| allocation())?;
        }
        events.push(ScopeEventV29 {
            function,
            block,
            statement_count,
            kind,
        });
        Ok(())
    }

    pub(crate) fn prepare(
        &mut self,
        function: SemanticFunctionIdV1,
        events: Vec<ScopeEventV29>,
        mut charge: impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<PreparedScopeEventsV29<'_>, Error> {
        charge(events.len().checked_add(1).ok_or_else(mismatch)?)?;
        if function.index() as usize != self.completed
            || self.completed >= self.function_count
            || events.iter().any(|event| event.function != function)
        {
            return Err(mismatch());
        }
        let required = self
            .events
            .len()
            .checked_add(events.len())
            .ok_or_else(mismatch)?;
        if required > self.events.capacity() {
            charge(required)?;
            self.events
                .try_reserve(events.len())
                .map_err(|_| allocation())?;
        }
        Ok(PreparedScopeEventsV29 {
            owner: self,
            events,
        })
    }

    pub(crate) fn begin_census(
        self,
        semantic: &AdmittedInertSemanticMirV1,
        declarations: SemanticDeclarationTablesCommitmentV1,
        mut charge: impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<ScopeCensusV29, Error> {
        charge(1)?;
        if self.declarations != declarations
            || self.target != semantic.target()
            || declarations.wire_version() != SemanticMirWireVersionV1::V29
            || self.completed != self.function_count
            || self.function_count != semantic.functions().len()
            || self.classes.len() != semantic.callables().len()
            // The current importer constructs these tables empty. Extending
            // that source profile requires retaining their constructor identity.
            || !semantic.allocations().is_empty()
            || !semantic.statics().is_empty()
            || !semantic.vtables().is_empty()
        {
            return Err(mismatch());
        }
        for (index, class) in self.classes.iter().copied().enumerate() {
            charge(1)?;
            class.check(index, semantic)?;
        }
        Ok(ScopeCensusV29 {
            scopes: RetainedWorkgroupScopesV29 {
                classes: self.classes,
                events: self.events,
            },
            next: 0,
        })
    }
}

pub(crate) struct ScopeCensusV29 {
    scopes: RetainedWorkgroupScopesV29,
    next: usize,
}

impl ScopeCensusV29 {
    pub(crate) fn observe(
        &mut self,
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        data: &SemanticBasicBlockV1,
        mut charge: impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<(), Error> {
        charge(1)?;
        let Some(kind) = event_kind(&self.scopes.classes, function, data.terminator().kind())?
        else {
            return Ok(());
        };
        let expected = ScopeEventV29 {
            function,
            block,
            statement_count: data.statements().len(),
            kind,
        };
        if self.scopes.events.get(self.next) != Some(&expected) {
            return Err(mismatch());
        }
        self.next += 1;
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<RetainedWorkgroupScopesV29, Error> {
        if self.next != self.scopes.events.len() {
            return Err(mismatch());
        }
        Ok(self.scopes)
    }
}

/// Retained only beside the existing sealed source receipt. No public constructor,
/// clone, decoder or conversion to lifecycle/execution authority is provided.
pub(crate) struct RetainedWorkgroupScopesV29 {
    classes: Vec<ScopeCallableV29>,
    events: Vec<ScopeEventV29>,
}

impl RetainedWorkgroupScopesV29 {
    pub(crate) fn classes(&self) -> &[ScopeCallableV29] {
        &self.classes
    }
    pub(crate) fn events(&self) -> &[ScopeEventV29] {
        &self.events
    }
}
