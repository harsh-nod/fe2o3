//! Bounded construction of semantic function bodies from the reviewed rustc MIR subset.
//!
//! This module is deliberately a pure producer. It consumes caller-frozen
//! canonical identities and source records, checks them against the live rustc
//! body, and constructs inert semantic-MIR records. It does not qualify,
//! admit, lower, compile, or authorize the resulting records.

use std::collections::HashMap;
use std::fmt;

use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateKindV1, SemanticAssertMessageV1, SemanticAssignmentV1, SemanticBasicBlockV1,
    SemanticBinaryOpV1, SemanticBlockIdV1, SemanticBlockIdentityV1, SemanticBorrowKindV1,
    SemanticCallDestinationV1, SemanticCallableIdV1, SemanticConstGenericArgumentsIdentityV1,
    SemanticConstantBytesV1, SemanticConstantV1, SemanticConstantValueV1,
    SemanticControlFlowEdgeV1, SemanticDirectCallV1, SemanticEdgeRoleV1, SemanticFunctionAbiV1,
    SemanticFunctionDeclV1, SemanticFunctionIdV1, SemanticFunctionIdentityV1,
    SemanticFunctionRoleV1, SemanticGenericTypeArgumentsIdentityV1,
    SemanticItemDefinitionIdentityV1, SemanticKernelEntryV1, SemanticLinkSymbolV1,
    SemanticLocalDeclV1, SemanticLocalIdV1, SemanticLocalIdentityV1, SemanticLocalRoleV1,
    SemanticMemoryLoadV1, SemanticMirErrorV1, SemanticMirLimitsV1, SemanticMirResourceV1,
    SemanticMonomorphizationIdentityV1, SemanticOperandV1, SemanticPlaceV1,
    SemanticProjectionKindV1, SemanticProjectionV1, SemanticRvalueKindV1, SemanticRvalueV1,
    SemanticScalarValueV1, SemanticSourceProvenanceV1, SemanticStatementKindV1,
    SemanticStatementV1, SemanticSwitchTargetV1, SemanticSwitchTargetsV1, SemanticTerminatorKindV1,
    SemanticTerminatorV1, SemanticTypeIdV1, SemanticUnwindActionV1, SemanticVolatilityV1,
};
use rustc_middle::mir::interpret::GlobalAlloc;
use rustc_middle::mir::{
    AggregateKind, AssertKind, BinOp, Body, BorrowKind, ConstValue, MutBorrowKind, Operand, Place,
    PlaceTy, ProjectionElem, RETURN_PLACE, Rvalue, START_BLOCK, StatementKind, TerminatorKind,
    UnwindAction,
};
use rustc_middle::ty::layout::{LayoutCx, LayoutOf};
use rustc_middle::ty::{EarlyBinder, Instance, Ty, TyCtxt, TyKind, TypingEnv};

use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1;

const MAX_ERROR_COMPONENT_CHARS_V1: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionSemanticFunctionIdentitiesV1 {
    identity: SemanticFunctionIdentityV1,
    item_definition: SemanticItemDefinitionIdentityV1,
    monomorphization: SemanticMonomorphizationIdentityV1,
    generic_type_arguments: SemanticGenericTypeArgumentsIdentityV1,
    const_generic_arguments: SemanticConstGenericArgumentsIdentityV1,
}

impl ProductionSemanticFunctionIdentitiesV1 {
    pub(crate) const fn new(
        identity: SemanticFunctionIdentityV1,
        item_definition: SemanticItemDefinitionIdentityV1,
        monomorphization: SemanticMonomorphizationIdentityV1,
        generic_type_arguments: SemanticGenericTypeArgumentsIdentityV1,
        const_generic_arguments: SemanticConstGenericArgumentsIdentityV1,
    ) -> Self {
        Self {
            identity,
            item_definition,
            monomorphization,
            generic_type_arguments,
            const_generic_arguments,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProductionSemanticFunctionExportV1 {
    None,
    Kernel(SemanticKernelEntryV1),
    DeviceFfi(SemanticLinkSymbolV1),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProductionSemanticTypeBindingV1<'tcx> {
    rustc_type: Ty<'tcx>,
    semantic_type: SemanticTypeIdV1,
}

impl<'tcx> ProductionSemanticTypeBindingV1<'tcx> {
    pub(crate) const fn new(rustc_type: Ty<'tcx>, semantic_type: SemanticTypeIdV1) -> Self {
        Self {
            rustc_type,
            semantic_type,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionSemanticLocalBindingV1 {
    rustc_local: u32,
    semantic_local: SemanticLocalIdV1,
    identity: SemanticLocalIdentityV1,
    source: SemanticSourceProvenanceV1,
}

impl ProductionSemanticLocalBindingV1 {
    pub(crate) const fn new(
        rustc_local: u32,
        semantic_local: SemanticLocalIdV1,
        identity: SemanticLocalIdentityV1,
        source: SemanticSourceProvenanceV1,
    ) -> Self {
        Self {
            rustc_local,
            semantic_local,
            identity,
            source,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProductionSemanticBlockBindingV1 {
    rustc_block: u32,
    semantic_block: SemanticBlockIdV1,
    identity: SemanticBlockIdentityV1,
    source: SemanticSourceProvenanceV1,
    statement_sources: Box<[SemanticSourceProvenanceV1]>,
    terminator_source: SemanticSourceProvenanceV1,
}

impl ProductionSemanticBlockBindingV1 {
    pub(crate) fn new(
        rustc_block: u32,
        semantic_block: SemanticBlockIdV1,
        identity: SemanticBlockIdentityV1,
        source: SemanticSourceProvenanceV1,
        statement_sources: Vec<SemanticSourceProvenanceV1>,
        terminator_source: SemanticSourceProvenanceV1,
    ) -> Self {
        Self {
            rustc_block,
            semantic_block,
            identity,
            source,
            statement_sources: statement_sources.into_boxed_slice(),
            terminator_source,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProductionSemanticDirectCallBindingV1<'tcx> {
    caller: SemanticFunctionIdV1,
    rustc_block: u32,
    expected_callee: Instance<'tcx>,
}

impl<'tcx> ProductionSemanticDirectCallBindingV1<'tcx> {
    pub(crate) const fn new(
        caller: SemanticFunctionIdV1,
        rustc_block: u32,
        expected_callee: Instance<'tcx>,
    ) -> Self {
        Self {
            caller,
            rustc_block,
            expected_callee,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProductionSemanticTerminalExpansionRecipeV1<'tcx> {
    caller: SemanticFunctionIdV1,
    rustc_block: u32,
    expected_callee: Instance<'tcx>,
    expansion: ProductionTerminalExpansionV1,
}

impl<'tcx> ProductionSemanticTerminalExpansionRecipeV1<'tcx> {
    pub(crate) const fn new(
        caller: SemanticFunctionIdV1,
        rustc_block: u32,
        expected_callee: Instance<'tcx>,
        expansion: ProductionTerminalExpansionV1,
    ) -> Self {
        Self {
            caller,
            rustc_block,
            expected_callee,
            expansion,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ProductionSemanticCallableOwnerEntryV1<'tcx> {
    Defined {
        rustc_instance: Instance<'tcx>,
        semantic_callable: SemanticCallableIdV1,
    },
    Terminal {
        rustc_instance: Instance<'tcx>,
        expansion: ProductionTerminalExpansionV1,
        semantic_callable: SemanticCallableIdV1,
    },
}

impl<'tcx> ProductionSemanticCallableOwnerEntryV1<'tcx> {
    pub(crate) const fn defined(
        rustc_instance: Instance<'tcx>,
        semantic_callable: SemanticCallableIdV1,
    ) -> Self {
        Self::Defined {
            rustc_instance,
            semantic_callable,
        }
    }

    pub(crate) const fn terminal(
        rustc_instance: Instance<'tcx>,
        expansion: ProductionTerminalExpansionV1,
        semantic_callable: SemanticCallableIdV1,
    ) -> Self {
        Self::Terminal {
            rustc_instance,
            expansion,
            semantic_callable,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProductionSemanticCallableOwnerKindV1 {
    Defined,
    Terminal(ProductionTerminalExpansionV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProductionSemanticCallableOwnerRecordV1 {
    kind: ProductionSemanticCallableOwnerKindV1,
    semantic_callable: SemanticCallableIdV1,
}

/// Request-owned authority for callable identities and cumulative body limits.
///
/// The importer constructs this once, in canonical callable order, from the
/// authenticated preflight plan. Individual call-site recipes cannot name a
/// semantic callable and therefore cannot substitute an ABI-compatible callee.
pub(crate) struct ProductionSemanticBodyRequestOwnerV1<'tcx> {
    limits: SemanticMirLimitsV1,
    totals: ConstructionTotalsV1,
    callables: HashMap<Instance<'tcx>, ProductionSemanticCallableOwnerRecordV1>,
}

impl<'tcx> ProductionSemanticBodyRequestOwnerV1<'tcx> {
    pub(crate) fn new(
        limits: SemanticMirLimitsV1,
        type_count: usize,
        callable_entries: &[ProductionSemanticCallableOwnerEntryV1<'tcx>],
    ) -> Result<Self, ProductionSemanticBodyErrorV1> {
        let mut totals = ConstructionTotalsV1::default();
        totals.charge(SemanticMirResourceV1::Types, type_count, limits)?;
        totals.charge(
            SemanticMirResourceV1::Callables,
            callable_entries.len(),
            limits,
        )?;
        let mut callables = HashMap::new();
        callables
            .try_reserve(callable_entries.len())
            .map_err(|_| allocation(SemanticMirResourceV1::Callables))?;
        for (index, entry) in callable_entries.iter().copied().enumerate() {
            totals.charge(SemanticMirResourceV1::ValidationWork, 1, limits)?;
            let (rustc_instance, record) = match entry {
                ProductionSemanticCallableOwnerEntryV1::Defined {
                    rustc_instance,
                    semantic_callable,
                } => (
                    rustc_instance,
                    ProductionSemanticCallableOwnerRecordV1 {
                        kind: ProductionSemanticCallableOwnerKindV1::Defined,
                        semantic_callable,
                    },
                ),
                ProductionSemanticCallableOwnerEntryV1::Terminal {
                    rustc_instance,
                    expansion,
                    semantic_callable,
                } => (
                    rustc_instance,
                    ProductionSemanticCallableOwnerRecordV1 {
                        kind: ProductionSemanticCallableOwnerKindV1::Terminal(expansion),
                        semantic_callable,
                    },
                ),
            };
            require_canonical_callable_id_v1(index, record.semantic_callable)?;
            if callables.insert(rustc_instance, record).is_some() {
                return Err(table("callable owner table"));
            }
        }
        Ok(Self {
            limits,
            totals,
            callables,
        })
    }

    fn charge(
        &mut self,
        resource: SemanticMirResourceV1,
        amount: usize,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        self.totals.charge(resource, amount, self.limits)
    }

    fn defined_callable(
        &self,
        rustc_instance: Instance<'tcx>,
    ) -> Result<SemanticCallableIdV1, ProductionSemanticBodyErrorV1> {
        let record = self
            .callables
            .get(&rustc_instance)
            .copied()
            .ok_or_else(|| table("callable owner table"))?;
        resolve_owned_callable_record_v1(record, ProductionSemanticCallableOwnerKindV1::Defined)
    }

    fn terminal_callable(
        &self,
        rustc_instance: Instance<'tcx>,
        expansion: ProductionTerminalExpansionV1,
    ) -> Result<SemanticCallableIdV1, ProductionSemanticBodyErrorV1> {
        let record = self
            .callables
            .get(&rustc_instance)
            .copied()
            .ok_or_else(|| table("callable owner table"))?;
        resolve_owned_callable_record_v1(
            record,
            ProductionSemanticCallableOwnerKindV1::Terminal(expansion),
        )
    }
}

fn resolve_owned_callable_record_v1(
    record: ProductionSemanticCallableOwnerRecordV1,
    expected: ProductionSemanticCallableOwnerKindV1,
) -> Result<SemanticCallableIdV1, ProductionSemanticBodyErrorV1> {
    if record.kind != expected {
        return Err(table("callable owner table"));
    }
    Ok(record.semantic_callable)
}

fn require_canonical_callable_id_v1(
    canonical_index: usize,
    semantic_callable: SemanticCallableIdV1,
) -> Result<(), ProductionSemanticBodyErrorV1> {
    let expected = u32::try_from(canonical_index).map_err(|_| table("callable owner table"))?;
    if semantic_callable.index() != expected {
        return Err(table("callable owner table"));
    }
    Ok(())
}

pub(crate) struct ProductionSemanticBodyInputV1<'a, 'tcx> {
    pub(crate) tcx: TyCtxt<'tcx>,
    pub(crate) instance: Instance<'tcx>,
    pub(crate) body: &'a Body<'tcx>,
    pub(crate) function: SemanticFunctionIdV1,
    pub(crate) identities: ProductionSemanticFunctionIdentitiesV1,
    pub(crate) role: SemanticFunctionRoleV1,
    pub(crate) export: ProductionSemanticFunctionExportV1,
    pub(crate) source: SemanticSourceProvenanceV1,
    pub(crate) abi: SemanticFunctionAbiV1,
    pub(crate) type_bindings: &'a [ProductionSemanticTypeBindingV1<'tcx>],
    pub(crate) local_bindings: &'a [ProductionSemanticLocalBindingV1],
    pub(crate) block_bindings: &'a [ProductionSemanticBlockBindingV1],
    pub(crate) entry: SemanticBlockIdV1,
    pub(crate) direct_calls: &'a [ProductionSemanticDirectCallBindingV1<'tcx>],
    pub(crate) terminal_expansions: &'a [ProductionSemanticTerminalExpansionRecipeV1<'tcx>],
}

#[derive(Debug)]
pub(crate) enum ProductionSemanticBodyErrorV1 {
    LimitExceeded {
        resource: SemanticMirResourceV1,
        actual: u64,
        maximum: u64,
    },
    Allocation {
        resource: SemanticMirResourceV1,
    },
    IdentityTableMismatch {
        table: &'static str,
    },
    Unsupported {
        construct: String,
        block: Option<u32>,
        statement: Option<u32>,
    },
    Schema(SemanticMirErrorV1),
}

impl fmt::Display for ProductionSemanticBodyErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExceeded {
                resource,
                actual,
                maximum,
            } => write!(
                formatter,
                "semantic body construction exceeded {resource:?}: {actual} > {maximum}",
            ),
            Self::Allocation { resource } => {
                write!(
                    formatter,
                    "semantic body construction could not allocate {resource:?}"
                )
            }
            Self::IdentityTableMismatch { table } => {
                write!(
                    formatter,
                    "semantic body construction rejected inconsistent {table}"
                )
            }
            Self::Unsupported {
                construct,
                block,
                statement,
            } => match (block, statement) {
                (Some(block), Some(statement)) => write!(
                    formatter,
                    "semantic body construction rejected {construct} in block {block}, statement {statement}",
                ),
                (Some(block), None) => write!(
                    formatter,
                    "semantic body construction rejected {construct} in block {block}, terminator",
                ),
                (None, _) => write!(
                    formatter,
                    "semantic body construction rejected {construct} in function metadata",
                ),
            },
            Self::Schema(error) => write!(
                formatter,
                "semantic body construction produced an invalid schema record: {error}",
            ),
        }
    }
}

impl std::error::Error for ProductionSemanticBodyErrorV1 {}

impl From<SemanticMirErrorV1> for ProductionSemanticBodyErrorV1 {
    fn from(error: SemanticMirErrorV1) -> Self {
        Self::Schema(error)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ConstructionTotalsV1 {
    types: u64,
    functions: u64,
    callables: u64,
    locals: u64,
    blocks: u64,
    statements: u64,
    projections: u64,
    operands: u64,
    call_arguments: u64,
    switch_targets: u64,
    constant_bytes: u64,
    validation_work: u64,
}

impl ConstructionTotalsV1 {
    fn slot_mut(&mut self, resource: SemanticMirResourceV1) -> Option<&mut u64> {
        match resource {
            SemanticMirResourceV1::Types => Some(&mut self.types),
            SemanticMirResourceV1::Functions => Some(&mut self.functions),
            SemanticMirResourceV1::Callables => Some(&mut self.callables),
            SemanticMirResourceV1::Locals => Some(&mut self.locals),
            SemanticMirResourceV1::Blocks => Some(&mut self.blocks),
            SemanticMirResourceV1::Statements => Some(&mut self.statements),
            SemanticMirResourceV1::Projections => Some(&mut self.projections),
            SemanticMirResourceV1::Operands => Some(&mut self.operands),
            SemanticMirResourceV1::CallArguments => Some(&mut self.call_arguments),
            SemanticMirResourceV1::SwitchTargets => Some(&mut self.switch_targets),
            SemanticMirResourceV1::ConstantBytes => Some(&mut self.constant_bytes),
            SemanticMirResourceV1::ValidationWork => Some(&mut self.validation_work),
            SemanticMirResourceV1::Allocations
            | SemanticMirResourceV1::Statics
            | SemanticMirResourceV1::VTables
            | SemanticMirResourceV1::Roots
            | SemanticMirResourceV1::Relocations
            | SemanticMirResourceV1::LinkSymbolBytes
            | SemanticMirResourceV1::CanonicalBytes => None,
        }
    }

    fn charge(
        &mut self,
        resource: SemanticMirResourceV1,
        amount: usize,
        limits: SemanticMirLimitsV1,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        let maximum = limits.limit(resource);
        let amount = u64::try_from(amount).unwrap_or(u64::MAX);
        let slot = self.slot_mut(resource).ok_or(
            ProductionSemanticBodyErrorV1::IdentityTableMismatch {
                table: "construction accounting domain",
            },
        )?;
        *slot = slot
            .checked_add(amount)
            .ok_or(ProductionSemanticBodyErrorV1::LimitExceeded {
                resource,
                actual: u64::MAX,
                maximum,
            })?;
        if *slot > maximum {
            return Err(ProductionSemanticBodyErrorV1::LimitExceeded {
                resource,
                actual: *slot,
                maximum,
            });
        }
        Ok(())
    }
}

struct BodyProducerV1<'a, 'owner, 'tcx> {
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &'a Body<'tcx>,
    function: SemanticFunctionIdV1,
    type_ids: HashMap<Ty<'tcx>, SemanticTypeIdV1>,
    locals_by_raw: Vec<&'a ProductionSemanticLocalBindingV1>,
    locals_by_semantic: Vec<&'a ProductionSemanticLocalBindingV1>,
    blocks_by_raw: Vec<&'a ProductionSemanticBlockBindingV1>,
    blocks_by_semantic: Vec<&'a ProductionSemanticBlockBindingV1>,
    direct_calls_by_raw: Vec<Option<&'a ProductionSemanticDirectCallBindingV1<'tcx>>>,
    terminal_expansions_by_raw: Vec<Option<&'a ProductionSemanticTerminalExpansionRecipeV1<'tcx>>>,
    consumed_direct_calls: Vec<bool>,
    consumed_terminal_expansions: Vec<bool>,
    owner: &'owner mut ProductionSemanticBodyRequestOwnerV1<'tcx>,
}

type DirectCallTableV1<'a, 'tcx> = Vec<Option<&'a ProductionSemanticDirectCallBindingV1<'tcx>>>;
type TerminalExpansionTableV1<'a, 'tcx> =
    Vec<Option<&'a ProductionSemanticTerminalExpansionRecipeV1<'tcx>>>;
type CallTablesV1<'a, 'tcx> = (
    DirectCallTableV1<'a, 'tcx>,
    TerminalExpansionTableV1<'a, 'tcx>,
);

pub(crate) fn construct_production_semantic_body_v1<'a, 'owner, 'tcx>(
    input: ProductionSemanticBodyInputV1<'a, 'tcx>,
    owner: &'owner mut ProductionSemanticBodyRequestOwnerV1<'tcx>,
) -> Result<SemanticFunctionDeclV1, ProductionSemanticBodyErrorV1> {
    validate_export_role_v1(input.role, &input.export)?;
    let abi = input.abi.clone();
    let export = input.export.clone();
    let entry = input.entry;
    let mut producer = BodyProducerV1::new(&input, owner)?;
    let locals = producer.construct_locals()?;
    let blocks = producer.construct_blocks()?;
    producer.require_all_call_bindings_consumed()?;

    let ProductionSemanticFunctionIdentitiesV1 {
        identity,
        item_definition,
        monomorphization,
        generic_type_arguments,
        const_generic_arguments,
    } = input.identities;
    let mut function = SemanticFunctionDeclV1::new(
        identity,
        input.role,
        item_definition,
        monomorphization,
        generic_type_arguments,
        const_generic_arguments,
        input.source,
        abi,
        locals,
        entry,
        blocks,
    )?;
    function = match export {
        ProductionSemanticFunctionExportV1::None => function,
        ProductionSemanticFunctionExportV1::Kernel(entry) => function.with_kernel_entry(entry),
        ProductionSemanticFunctionExportV1::DeviceFfi(symbol) => {
            function.with_device_ffi_export_symbol(symbol)
        }
    };
    Ok(function)
}

impl<'a, 'owner, 'tcx> BodyProducerV1<'a, 'owner, 'tcx> {
    fn new(
        input: &'a ProductionSemanticBodyInputV1<'a, 'tcx>,
        owner: &'owner mut ProductionSemanticBodyRequestOwnerV1<'tcx>,
    ) -> Result<Self, ProductionSemanticBodyErrorV1> {
        let owned_callable = owner.defined_callable(input.instance)?;
        if owned_callable.index() != input.function.index() {
            return Err(table("function body owner"));
        }
        owner.charge(SemanticMirResourceV1::Functions, 1)?;
        owner.charge(SemanticMirResourceV1::Locals, input.body.local_decls.len())?;
        owner.charge(SemanticMirResourceV1::Blocks, input.body.basic_blocks.len())?;

        let type_ids = build_type_table_v1(input.tcx, input.instance, input.type_bindings, owner)?;
        let (locals_by_raw, locals_by_semantic) =
            build_local_tables_v1(input.body, input.local_bindings, owner)?;
        let (blocks_by_raw, blocks_by_semantic) =
            build_block_tables_v1(input.body, input.block_bindings, owner)?;
        if blocks_by_raw
            .get(START_BLOCK.index())
            .map(|binding| binding.semantic_block)
            != Some(input.entry)
        {
            return Err(table("entry block binding"));
        }
        let (direct_calls_by_raw, terminal_expansions_by_raw) = build_call_tables_v1(
            input.function,
            input.body.basic_blocks.len(),
            input.direct_calls,
            input.terminal_expansions,
            owner,
        )?;
        let consumed_direct_calls = try_filled_vec_v1(
            input.body.basic_blocks.len(),
            false,
            SemanticMirResourceV1::Blocks,
        )?;
        let consumed_terminal_expansions = try_filled_vec_v1(
            input.body.basic_blocks.len(),
            false,
            SemanticMirResourceV1::Blocks,
        )?;
        Ok(Self {
            tcx: input.tcx,
            instance: input.instance,
            body: input.body,
            function: input.function,
            type_ids,
            locals_by_raw,
            locals_by_semantic,
            blocks_by_raw,
            blocks_by_semantic,
            direct_calls_by_raw,
            terminal_expansions_by_raw,
            consumed_direct_calls,
            consumed_terminal_expansions,
            owner,
        })
    }

    fn construct_locals(
        &mut self,
    ) -> Result<Vec<SemanticLocalDeclV1>, ProductionSemanticBodyErrorV1> {
        let mut locals = try_vec_v1(self.locals_by_semantic.len(), SemanticMirResourceV1::Locals)?;
        for index in 0..self.locals_by_semantic.len() {
            let binding = self.locals_by_semantic[index];
            self.work()?;
            let raw = usize::try_from(binding.rustc_local).map_err(|_| table("local table"))?;
            let declaration = self
                .body
                .local_decls
                .get(rustc_middle::mir::Local::from_usize(raw))
                .ok_or_else(|| table("local table"))?;
            let ty = self.type_id(declaration.ty, None, None)?;
            locals.push(SemanticLocalDeclV1::new(
                binding.identity,
                ty,
                semantic_local_role_v1(binding.rustc_local, self.body.arg_count)?,
                binding.source,
            ));
        }
        Ok(locals)
    }

    fn construct_blocks(
        &mut self,
    ) -> Result<Vec<SemanticBasicBlockV1>, ProductionSemanticBodyErrorV1> {
        let mut blocks = try_vec_v1(self.blocks_by_semantic.len(), SemanticMirResourceV1::Blocks)?;
        for index in 0..self.blocks_by_semantic.len() {
            let binding = self.blocks_by_semantic[index];
            let raw_block = binding.rustc_block;
            let raw_index = usize::try_from(raw_block).map_err(|_| table("block table"))?;
            let data = self
                .body
                .basic_blocks
                .get(rustc_middle::mir::BasicBlock::from_usize(raw_index))
                .ok_or_else(|| table("block table"))?;
            if binding.statement_sources.len() != data.statements.len() {
                return Err(table("statement source table"));
            }
            self.owner
                .charge(SemanticMirResourceV1::Statements, data.statements.len())?;
            let mut statements =
                try_vec_v1(data.statements.len(), SemanticMirResourceV1::Statements)?;
            for (statement_index, statement) in data.statements.iter().enumerate() {
                self.work()?;
                let index =
                    u32::try_from(statement_index).map_err(|_| table("statement source table"))?;
                let source = *binding
                    .statement_sources
                    .get(statement_index)
                    .ok_or_else(|| table("statement source table"))?;
                let kind = self.construct_statement(raw_block, index, &statement.kind)?;
                statements.push(SemanticStatementV1::new(source, kind));
            }
            let terminator = data.terminator.as_ref().ok_or_else(|| {
                unsupported("basic block without a terminator", Some(raw_block), None)
            })?;
            self.work()?;
            let kind = self.construct_terminator(raw_block, &terminator.kind)?;
            blocks.push(SemanticBasicBlockV1::new(
                binding.identity,
                binding.source,
                statements,
                SemanticTerminatorV1::new(binding.terminator_source, kind),
            )?);
        }
        Ok(blocks)
    }

    fn construct_statement(
        &mut self,
        block: u32,
        statement: u32,
        kind: &StatementKind<'tcx>,
    ) -> Result<SemanticStatementKindV1, ProductionSemanticBodyErrorV1> {
        let site = (Some(block), Some(statement));
        match kind {
            StatementKind::Assign(assignment) => {
                let (destination, value) = &**assignment;
                let destination = self.construct_place(*destination, site.0, site.1)?;
                let value = self.construct_rvalue(value, site.0, site.1)?;
                Ok(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    destination,
                    value,
                )))
            }
            StatementKind::StorageLive(local) => Ok(SemanticStatementKindV1::StorageLive(
                self.local_id(local.index())?,
            )),
            StatementKind::StorageDead(local) => Ok(SemanticStatementKindV1::StorageDead(
                self.local_id(local.index())?,
            )),
            StatementKind::SetDiscriminant {
                place,
                variant_index,
            } => Ok(SemanticStatementKindV1::SetDiscriminant {
                place: self.construct_place(**place, site.0, site.1)?,
                variant_index: u32::try_from(variant_index.index())
                    .map_err(|_| table("enum variant index"))?,
            }),
            StatementKind::Nop => Ok(SemanticStatementKindV1::Nop),
            StatementKind::FakeRead(..) => Err(unsupported("FakeRead statement", site.0, site.1)),
            StatementKind::Intrinsic(..) => Err(unsupported("intrinsic statement", site.0, site.1)),
            StatementKind::Retag(..) => Err(unsupported("Retag statement", site.0, site.1)),
            StatementKind::PlaceMention(..) => {
                Err(unsupported("PlaceMention statement", site.0, site.1))
            }
            StatementKind::AscribeUserType(..) => {
                Err(unsupported("AscribeUserType statement", site.0, site.1))
            }
            StatementKind::Coverage(..) => Err(unsupported("Coverage statement", site.0, site.1)),
            StatementKind::ConstEvalCounter => {
                Err(unsupported("ConstEvalCounter statement", site.0, site.1))
            }
            StatementKind::BackwardIncompatibleDropHint { .. } => Err(unsupported(
                "BackwardIncompatibleDropHint statement",
                site.0,
                site.1,
            )),
        }
    }

    fn construct_rvalue(
        &mut self,
        value: &Rvalue<'tcx>,
        block: Option<u32>,
        statement: Option<u32>,
    ) -> Result<SemanticRvalueV1, ProductionSemanticBodyErrorV1> {
        let result_type =
            self.type_id(value.ty(&self.body.local_decls, self.tcx), block, statement)?;
        let kind = match value {
            Rvalue::Use(operand) => {
                SemanticRvalueKindV1::Use(self.construct_operand(operand, block, statement)?)
            }
            Rvalue::Ref(_, borrow, place) => {
                let kind = semantic_borrow_kind_v1(*borrow, block, statement)?;
                SemanticRvalueKindV1::Borrow {
                    kind,
                    place: self.construct_place(*place, block, statement)?,
                }
            }
            Rvalue::Discriminant(place) => {
                SemanticRvalueKindV1::Discriminant(self.construct_place(*place, block, statement)?)
            }
            Rvalue::CopyForDeref(place) => SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                self.construct_place(*place, block, statement)?,
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
            Rvalue::Aggregate(kind, operands) => {
                let aggregate_kind = match &**kind {
                    AggregateKind::Array(_) => SemanticAggregateKindV1::Array,
                    AggregateKind::Tuple => SemanticAggregateKindV1::Tuple,
                    AggregateKind::Adt(definition, variant, ..) => {
                        let definition = self.tcx.adt_def(*definition);
                        if definition.is_union() {
                            return Err(unsupported("union aggregate rvalue", block, statement));
                        }
                        if definition.is_enum() {
                            SemanticAggregateKindV1::EnumVariant(
                                u32::try_from(variant.index())
                                    .map_err(|_| table("enum variant index"))?,
                            )
                        } else {
                            SemanticAggregateKindV1::Aggregate
                        }
                    }
                    AggregateKind::Closure(..) => {
                        return Err(unsupported("closure aggregate rvalue", block, statement));
                    }
                    AggregateKind::CoroutineClosure(..) => {
                        return Err(unsupported(
                            "coroutine-closure aggregate rvalue",
                            block,
                            statement,
                        ));
                    }
                    AggregateKind::Coroutine(..) => {
                        return Err(unsupported("coroutine aggregate rvalue", block, statement));
                    }
                    AggregateKind::RawPtr(..) => {
                        return Err(unsupported(
                            "raw-pointer aggregate rvalue",
                            block,
                            statement,
                        ));
                    }
                };
                let mut semantic_operands =
                    try_vec_v1(operands.len(), SemanticMirResourceV1::Operands)?;
                for operand in operands {
                    semantic_operands.push(self.construct_operand(operand, block, statement)?);
                }
                SemanticRvalueKindV1::aggregate(aggregate_kind, semantic_operands)?
            }
            Rvalue::Repeat(operand, count) => {
                let count = count
                    .try_to_target_usize(self.tcx)
                    .and_then(|count| usize::try_from(count).ok())
                    .ok_or_else(|| {
                        unsupported("Repeat count outside host bounds", block, statement)
                    })?;
                let mut semantic_operands = try_vec_v1(count, SemanticMirResourceV1::Operands)?;
                if count != 0 {
                    let operand = self.construct_operand(operand, block, statement)?;
                    self.owner
                        .charge(SemanticMirResourceV1::Operands, count - 1)?;
                    semantic_operands.resize(count, operand);
                }
                SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::Array, semantic_operands)?
            }
            Rvalue::RawPtr(..) => {
                return Err(unsupported("RawPtr rvalue", block, statement));
            }
            Rvalue::Cast(..) => return Err(unsupported("Cast rvalue", block, statement)),
            Rvalue::BinaryOp(operation, operands) => {
                let operation = semantic_binary_operation(*operation)
                    .ok_or_else(|| unsupported("unsupported BinaryOp rvalue", block, statement))?;
                SemanticRvalueKindV1::Binary {
                    operation,
                    left: self.construct_operand(&operands.0, block, statement)?,
                    right: self.construct_operand(&operands.1, block, statement)?,
                }
            }
            Rvalue::UnaryOp(..) => {
                return Err(unsupported("UnaryOp rvalue", block, statement));
            }
            Rvalue::ThreadLocalRef(..) => {
                return Err(unsupported("ThreadLocalRef rvalue", block, statement));
            }
            Rvalue::WrapUnsafeBinder(..) => {
                return Err(unsupported("WrapUnsafeBinder rvalue", block, statement));
            }
        };
        Ok(SemanticRvalueV1::new(result_type, kind))
    }

    fn construct_terminator(
        &mut self,
        raw_block: u32,
        terminator: &TerminatorKind<'tcx>,
    ) -> Result<SemanticTerminatorKindV1, ProductionSemanticBodyErrorV1> {
        let block = Some(raw_block);
        match terminator {
            TerminatorKind::Return => Ok(SemanticTerminatorKindV1::Return),
            TerminatorKind::Unreachable => Ok(SemanticTerminatorKindV1::Unreachable),
            TerminatorKind::Goto { target } => Ok(SemanticTerminatorKindV1::Goto(
                self.edge(SemanticEdgeRoleV1::Goto, target.index())?,
            )),
            TerminatorKind::SwitchInt { discr, targets } => {
                self.owner
                    .charge(SemanticMirResourceV1::SwitchTargets, targets.iter().count())?;
                let discriminant = self.construct_operand(discr, block, None)?;
                let mut values =
                    try_vec_v1(targets.iter().count(), SemanticMirResourceV1::SwitchTargets)?;
                for (value, target) in targets.iter() {
                    values.push(SemanticSwitchTargetV1::new(
                        value,
                        self.edge(SemanticEdgeRoleV1::SwitchValue, target.index())?,
                    ));
                }
                values.sort_unstable_by_key(|target| target.value());
                let otherwise = self.edge(
                    SemanticEdgeRoleV1::SwitchOtherwise,
                    targets.otherwise().index(),
                )?;
                Ok(SemanticTerminatorKindV1::SwitchInt {
                    discriminant,
                    targets: SemanticSwitchTargetsV1::new(values, otherwise)?,
                })
            }
            TerminatorKind::Call {
                func,
                args,
                destination,
                target,
                unwind,
                ..
            } => {
                self.owner
                    .charge(SemanticMirResourceV1::CallArguments, args.len())?;
                let resolved = resolve_direct_call_v1(self.tcx, self.instance, self.body, func)
                    .map_err(|construct| unsupported(construct, block, None))?;
                let semantic_callee = self.resolve_call_binding(raw_block, resolved, args.len())?;
                let mut arguments = try_vec_v1(args.len(), SemanticMirResourceV1::CallArguments)?;
                for argument in args {
                    arguments.push(self.construct_operand(&argument.node, block, None)?);
                }
                let destination = if let Some(target) = target {
                    Some(SemanticCallDestinationV1::new(
                        self.construct_place(*destination, block, None)?,
                        self.edge(SemanticEdgeRoleV1::CallReturn, target.index())?,
                    ))
                } else {
                    None
                };
                let unwind = match unwind {
                    UnwindAction::Continue => SemanticUnwindActionV1::Continue,
                    UnwindAction::Unreachable => SemanticUnwindActionV1::Unreachable,
                    UnwindAction::Terminate(_) | UnwindAction::Cleanup(_) => {
                        return Err(unsupported("call with executable unwind edge", block, None));
                    }
                };
                Ok(SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        semantic_callee,
                        arguments,
                        destination,
                        unwind,
                    )?,
                ))
            }
            TerminatorKind::TailCall { .. } => Err(unsupported("TailCall terminator", block, None)),
            TerminatorKind::Drop { .. } => Err(unsupported("Drop terminator", block, None)),
            TerminatorKind::Assert {
                cond,
                expected,
                msg,
                target,
                unwind,
            } => {
                let message = match msg.as_ref() {
                    AssertKind::BoundsCheck { len, index } => {
                        SemanticAssertMessageV1::BoundsCheck {
                            length: self.construct_operand(len, block, None)?,
                            index: self.construct_operand(index, block, None)?,
                        }
                    }
                    _ => return Err(unsupported("non-bounds Assert terminator", block, None)),
                };
                let unwind = match unwind {
                    UnwindAction::Continue => SemanticUnwindActionV1::Continue,
                    UnwindAction::Unreachable => SemanticUnwindActionV1::Unreachable,
                    UnwindAction::Terminate(_) | UnwindAction::Cleanup(_) => {
                        return Err(unsupported(
                            "assert with executable unwind edge",
                            block,
                            None,
                        ));
                    }
                };
                Ok(SemanticTerminatorKindV1::Assert {
                    condition: self.construct_operand(cond, block, None)?,
                    expected: *expected,
                    message,
                    target: self.edge(SemanticEdgeRoleV1::AssertSuccess, target.index())?,
                    unwind,
                })
            }
            TerminatorKind::UnwindResume => {
                Err(unsupported("UnwindResume terminator", block, None))
            }
            TerminatorKind::UnwindTerminate(..) => {
                Err(unsupported("UnwindTerminate terminator", block, None))
            }
            TerminatorKind::Yield { .. } => Err(unsupported("Yield terminator", block, None)),
            TerminatorKind::CoroutineDrop => {
                Err(unsupported("CoroutineDrop terminator", block, None))
            }
            TerminatorKind::FalseEdge { .. } => {
                Err(unsupported("FalseEdge terminator", block, None))
            }
            TerminatorKind::FalseUnwind { .. } => {
                Err(unsupported("FalseUnwind terminator", block, None))
            }
            TerminatorKind::InlineAsm { .. } => {
                Err(unsupported("InlineAsm terminator", block, None))
            }
        }
    }

    fn construct_operand(
        &mut self,
        operand: &Operand<'tcx>,
        block: Option<u32>,
        statement: Option<u32>,
    ) -> Result<SemanticOperandV1, ProductionSemanticBodyErrorV1> {
        self.owner.charge(SemanticMirResourceV1::Operands, 1)?;
        self.work()?;
        match operand {
            Operand::Copy(place) => Ok(SemanticOperandV1::Copy(
                self.construct_place(*place, block, statement)?,
            )),
            Operand::Move(place) => Ok(SemanticOperandV1::Move(
                self.construct_place(*place, block, statement)?,
            )),
            Operand::Constant(constant) => {
                let normalized = self
                    .instance
                    .try_instantiate_mir_and_normalize_erasing_regions(
                        self.tcx,
                        TypingEnv::fully_monomorphized(),
                        EarlyBinder::bind(constant.const_),
                    )
                    .map_err(|_| {
                        unsupported(
                            "constant that failed monomorphic normalization",
                            block,
                            statement,
                        )
                    })?;
                let ty = self.type_id(normalized.ty(), block, statement)?;
                let value =
                    self.construct_constant_value(normalized, constant.span, block, statement)?;
                Ok(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    ty, value,
                )))
            }
            Operand::RuntimeChecks(..) => {
                Err(unsupported("RuntimeChecks operand", block, statement))
            }
        }
    }

    fn construct_place(
        &mut self,
        place: Place<'tcx>,
        block: Option<u32>,
        statement: Option<u32>,
    ) -> Result<SemanticPlaceV1, ProductionSemanticBodyErrorV1> {
        self.owner
            .charge(SemanticMirResourceV1::Projections, place.projection.len())?;
        let local = self.local_id(place.local.index())?;
        let local_ty = self.body.local_decls[place.local].ty;
        let mut derived = PlaceTy::from_ty(local_ty);
        let mut projections =
            try_vec_v1(place.projection.len(), SemanticMirResourceV1::Projections)?;
        for projection in place.projection {
            self.work()?;
            let kind = match projection {
                ProjectionElem::Deref => SemanticProjectionKindV1::Dereference,
                ProjectionElem::Field(field, _) => SemanticProjectionKindV1::Field(
                    u32::try_from(field.index()).map_err(|_| table("field projection"))?,
                ),
                ProjectionElem::Index(index) => {
                    if !matches!(derived.ty.kind(), TyKind::Array(..)) {
                        return Err(unsupported(
                            "Index projection on a non-array place",
                            block,
                            statement,
                        ));
                    }
                    SemanticProjectionKindV1::Index(self.local_id(index.index())?)
                }
                ProjectionElem::ConstantIndex {
                    offset,
                    min_length,
                    from_end,
                } => {
                    if !matches!(derived.ty.kind(), TyKind::Array(..)) {
                        return Err(unsupported(
                            "ConstantIndex projection on a non-array place",
                            block,
                            statement,
                        ));
                    }
                    SemanticProjectionKindV1::ConstantIndex {
                        offset,
                        minimum_length: min_length,
                        from_end,
                    }
                }
                ProjectionElem::Subslice { from, to, from_end } => {
                    if !matches!(derived.ty.kind(), TyKind::Array(..)) {
                        return Err(unsupported(
                            "Subslice projection on a non-array place",
                            block,
                            statement,
                        ));
                    }
                    SemanticProjectionKindV1::Subslice { from, to, from_end }
                }
                ProjectionElem::Downcast(_, variant) => SemanticProjectionKindV1::Downcast(
                    u32::try_from(variant.index()).map_err(|_| table("downcast projection"))?,
                ),
                ProjectionElem::OpaqueCast(_) => SemanticProjectionKindV1::OpaqueCast,
                ProjectionElem::UnwrapUnsafeBinder(_) => {
                    return Err(unsupported(
                        "UnwrapUnsafeBinder projection",
                        block,
                        statement,
                    ));
                }
            };
            derived = derived.projection_ty(self.tcx, projection);
            let result_type = self.type_id(derived.ty, block, statement)?;
            projections.push(SemanticProjectionV1::new(kind, result_type)?);
        }
        let ty = self.type_id(derived.ty, block, statement)?;
        Ok(SemanticPlaceV1::new(local, projections, ty)?)
    }

    fn construct_constant_value(
        &mut self,
        constant: rustc_middle::mir::Const<'tcx>,
        span: rustc_span::Span,
        block: Option<u32>,
        statement: Option<u32>,
    ) -> Result<SemanticConstantValueV1, ProductionSemanticBodyErrorV1> {
        if matches!(
            constant,
            rustc_middle::mir::Const::Val(ConstValue::ZeroSized, _)
        ) {
            return Ok(SemanticConstantValueV1::ZeroSized);
        }
        if let Some(value) =
            constant.try_eval_scalar_int(self.tcx, TypingEnv::fully_monomorphized())
        {
            let size_bytes = u8::try_from(value.size().bytes()).map_err(|_| {
                unsupported("scalar constant wider than 128 bits", block, statement)
            })?;
            return Ok(SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(
                value.to_bits(value.size()),
                size_bytes,
            )?));
        }

        let layout = LayoutCx::new(self.tcx, TypingEnv::fully_monomorphized())
            .layout_of(constant.ty())
            .map_err(|_| unsupported("constant without target layout", block, statement))?;
        let size = usize::try_from(layout.size.bytes())
            .map_err(|_| unsupported("constant byte size outside host usize", block, statement))?;
        self.owner
            .charge(SemanticMirResourceV1::ConstantBytes, size)?;
        let evaluated = constant
            .eval(self.tcx, TypingEnv::fully_monomorphized(), span)
            .map_err(|_| unsupported("constant evaluation failure", block, statement))?;
        match evaluated {
            ConstValue::ZeroSized => Ok(SemanticConstantValueV1::ZeroSized),
            ConstValue::Scalar(value) => {
                let scalar = value.try_to_scalar_int().map_err(|_| {
                    unsupported("scalar constant with pointer provenance", block, statement)
                })?;
                let size_bytes = u8::try_from(scalar.size().bytes()).map_err(|_| {
                    unsupported("scalar constant wider than 128 bits", block, statement)
                })?;
                Ok(SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(
                    scalar.to_bits(scalar.size()),
                    size_bytes,
                )?))
            }
            ConstValue::Slice { .. } => Err(unsupported(
                "slice constant requiring allocation provenance",
                block,
                statement,
            )),
            ConstValue::Indirect { alloc_id, offset } => {
                let GlobalAlloc::Memory(allocation) = self.tcx.global_alloc(alloc_id) else {
                    return Err(unsupported(
                        "indirect constant backed by a non-memory allocation",
                        block,
                        statement,
                    ));
                };
                let allocation = allocation.inner();
                let start = offset.bytes_usize();
                let end = start.checked_add(size).ok_or_else(|| {
                    unsupported("indirect constant range overflow", block, statement)
                })?;
                if end > allocation.len() {
                    return Err(unsupported(
                        "indirect constant outside its allocation",
                        block,
                        statement,
                    ));
                }
                let pointer_width = self.tcx.data_layout.pointer_size().bytes_usize();
                for (at, _) in allocation.provenance().ptrs().iter() {
                    self.work()?;
                    let pointer_start = at.bytes_usize();
                    let pointer_end = pointer_start.saturating_add(pointer_width);
                    if pointer_start < end && pointer_end > start {
                        return Err(unsupported(
                            "indirect constant with pointer provenance",
                            block,
                            statement,
                        ));
                    }
                }
                let raw = allocation.inspect_with_uninit_and_ptr_outside_interpreter(start..end);
                self.owner
                    .charge(SemanticMirResourceV1::ValidationWork, raw.len())?;
                let mut bytes = try_vec_v1(size, SemanticMirResourceV1::ConstantBytes)?;
                for (index, byte) in raw.iter().copied().enumerate() {
                    if !allocation
                        .init_mask()
                        .get(rustc_abi::Size::from_bytes(start + index))
                    {
                        return Err(unsupported(
                            "indirect constant with uninitialized bytes",
                            block,
                            statement,
                        ));
                    }
                    bytes.push(byte);
                }
                Ok(SemanticConstantValueV1::Bytes(
                    SemanticConstantBytesV1::new(bytes)?,
                ))
            }
        }
    }

    fn resolve_call_binding(
        &mut self,
        raw_block: u32,
        resolved: Instance<'tcx>,
        argument_count: usize,
    ) -> Result<SemanticCallableIdV1, ProductionSemanticBodyErrorV1> {
        let index = usize::try_from(raw_block).map_err(|_| table("call binding table"))?;
        let classified =
            crate::production_semantic_terminal_v1::classify(self.tcx, resolved.def_id());
        if let Some(rule) = classified {
            let crate::production_semantic_terminal_v1::ProductionSemanticTerminalRuleV1::Expand(
                expansion,
            ) = rule
            else {
                return Err(unsupported(
                    "reviewed terminal without a production expansion",
                    Some(raw_block),
                    None,
                ));
            };
            let recipe = self
                .terminal_expansions_by_raw
                .get(index)
                .copied()
                .flatten()
                .ok_or_else(|| table("terminal expansion table"))?;
            if recipe.caller != self.function
                || recipe.expected_callee != resolved
                || recipe.expansion != expansion
                || terminal_argument_count_v1(expansion) != Some(argument_count)
            {
                return Err(table("terminal expansion table"));
            }
            self.consumed_terminal_expansions[index] = true;
            if self.direct_calls_by_raw[index].is_some() {
                return Err(table("call binding table"));
            }
            self.owner.terminal_callable(resolved, expansion)
        } else {
            let binding = self
                .direct_calls_by_raw
                .get(index)
                .copied()
                .flatten()
                .ok_or_else(|| table("direct-call binding table"))?;
            if binding.caller != self.function || binding.expected_callee != resolved {
                return Err(table("direct-call binding table"));
            }
            self.consumed_direct_calls[index] = true;
            if self.terminal_expansions_by_raw[index].is_some() {
                return Err(table("call binding table"));
            }
            self.owner.defined_callable(resolved)
        }
    }

    fn require_all_call_bindings_consumed(&self) -> Result<(), ProductionSemanticBodyErrorV1> {
        for (index, binding) in self.direct_calls_by_raw.iter().enumerate() {
            if binding.is_some() && !self.consumed_direct_calls[index] {
                return Err(table("unused direct-call binding"));
            }
        }
        for (index, recipe) in self.terminal_expansions_by_raw.iter().enumerate() {
            if recipe.is_some() && !self.consumed_terminal_expansions[index] {
                return Err(table("unused terminal expansion recipe"));
            }
        }
        Ok(())
    }

    fn type_id(
        &mut self,
        raw: Ty<'tcx>,
        block: Option<u32>,
        statement: Option<u32>,
    ) -> Result<SemanticTypeIdV1, ProductionSemanticBodyErrorV1> {
        self.work()?;
        let ty = normalize_type_v1(self.tcx, self.instance, raw).map_err(|_| {
            unsupported(
                "type that failed monomorphic normalization",
                block,
                statement,
            )
        })?;
        self.type_ids
            .get(&ty)
            .copied()
            .ok_or_else(|| table("canonical type binding table"))
    }

    fn local_id(
        &self,
        raw_local: usize,
    ) -> Result<SemanticLocalIdV1, ProductionSemanticBodyErrorV1> {
        self.locals_by_raw
            .get(raw_local)
            .map(|binding| binding.semantic_local)
            .ok_or_else(|| table("local table"))
    }

    fn block_id(
        &self,
        raw_block: usize,
    ) -> Result<SemanticBlockIdV1, ProductionSemanticBodyErrorV1> {
        self.blocks_by_raw
            .get(raw_block)
            .map(|binding| binding.semantic_block)
            .ok_or_else(|| table("block table"))
    }

    fn edge(
        &self,
        role: SemanticEdgeRoleV1,
        raw_target: usize,
    ) -> Result<SemanticControlFlowEdgeV1, ProductionSemanticBodyErrorV1> {
        Ok(SemanticControlFlowEdgeV1::new(
            role,
            self.block_id(raw_target)?,
        ))
    }

    fn work(&mut self) -> Result<(), ProductionSemanticBodyErrorV1> {
        self.owner.charge(SemanticMirResourceV1::ValidationWork, 1)
    }
}

fn build_type_table_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    bindings: &[ProductionSemanticTypeBindingV1<'tcx>],
    owner: &mut ProductionSemanticBodyRequestOwnerV1<'tcx>,
) -> Result<HashMap<Ty<'tcx>, SemanticTypeIdV1>, ProductionSemanticBodyErrorV1> {
    let mut by_type = HashMap::new();
    by_type
        .try_reserve(bindings.len())
        .map_err(|_| allocation(SemanticMirResourceV1::Types))?;
    let mut by_id = HashMap::new();
    by_id
        .try_reserve(bindings.len())
        .map_err(|_| allocation(SemanticMirResourceV1::Types))?;
    for binding in bindings {
        owner.charge(SemanticMirResourceV1::ValidationWork, 1)?;
        let normalized = normalize_type_v1(tcx, instance, binding.rustc_type)
            .map_err(|_| table("canonical type binding table"))?;
        if normalized != binding.rustc_type
            || by_type
                .insert(binding.rustc_type, binding.semantic_type)
                .is_some()
            || by_id
                .insert(binding.semantic_type, binding.rustc_type)
                .is_some()
        {
            return Err(table("canonical type binding table"));
        }
    }
    Ok(by_type)
}

fn build_local_tables_v1<'a, 'tcx>(
    body: &Body<'_>,
    bindings: &'a [ProductionSemanticLocalBindingV1],
    owner: &mut ProductionSemanticBodyRequestOwnerV1<'tcx>,
) -> Result<
    (
        Vec<&'a ProductionSemanticLocalBindingV1>,
        Vec<&'a ProductionSemanticLocalBindingV1>,
    ),
    ProductionSemanticBodyErrorV1,
> {
    if bindings.len() != body.local_decls.len() {
        return Err(table("local table"));
    }
    let mut by_raw = try_filled_vec_v1(bindings.len(), None, SemanticMirResourceV1::Locals)?;
    let mut by_semantic = try_filled_vec_v1(bindings.len(), None, SemanticMirResourceV1::Locals)?;
    for binding in bindings {
        owner.charge(SemanticMirResourceV1::ValidationWork, 1)?;
        insert_dense_binding_v1(&mut by_raw, binding.rustc_local, binding, "local table")?;
        insert_dense_binding_v1(
            &mut by_semantic,
            binding.semantic_local.index(),
            binding,
            "local table",
        )?;
    }
    Ok((
        collect_dense_bindings_v1(by_raw, "local table")?,
        collect_dense_bindings_v1(by_semantic, "local table")?,
    ))
}

fn build_block_tables_v1<'a, 'tcx>(
    body: &Body<'_>,
    bindings: &'a [ProductionSemanticBlockBindingV1],
    owner: &mut ProductionSemanticBodyRequestOwnerV1<'tcx>,
) -> Result<
    (
        Vec<&'a ProductionSemanticBlockBindingV1>,
        Vec<&'a ProductionSemanticBlockBindingV1>,
    ),
    ProductionSemanticBodyErrorV1,
> {
    if bindings.len() != body.basic_blocks.len() {
        return Err(table("block table"));
    }
    let mut by_raw = try_filled_vec_v1(bindings.len(), None, SemanticMirResourceV1::Blocks)?;
    let mut by_semantic = try_filled_vec_v1(bindings.len(), None, SemanticMirResourceV1::Blocks)?;
    for binding in bindings {
        owner.charge(SemanticMirResourceV1::ValidationWork, 1)?;
        insert_dense_binding_v1(&mut by_raw, binding.rustc_block, binding, "block table")?;
        insert_dense_binding_v1(
            &mut by_semantic,
            binding.semantic_block.index(),
            binding,
            "block table",
        )?;
    }
    Ok((
        collect_dense_bindings_v1(by_raw, "block table")?,
        collect_dense_bindings_v1(by_semantic, "block table")?,
    ))
}

fn build_call_tables_v1<'a, 'tcx>(
    function: SemanticFunctionIdV1,
    block_count: usize,
    direct_calls: &'a [ProductionSemanticDirectCallBindingV1<'tcx>],
    terminal_expansions: &'a [ProductionSemanticTerminalExpansionRecipeV1<'tcx>],
    owner: &mut ProductionSemanticBodyRequestOwnerV1<'tcx>,
) -> Result<CallTablesV1<'a, 'tcx>, ProductionSemanticBodyErrorV1> {
    let mut direct_by_raw = try_filled_vec_v1(block_count, None, SemanticMirResourceV1::Blocks)?;
    let mut terminal_by_raw = try_filled_vec_v1(block_count, None, SemanticMirResourceV1::Blocks)?;
    for binding in direct_calls {
        owner.charge(SemanticMirResourceV1::ValidationWork, 1)?;
        if binding.caller != function {
            return Err(table("direct-call binding table"));
        }
        insert_sparse_binding_v1(
            &mut direct_by_raw,
            binding.rustc_block,
            binding,
            "direct-call binding table",
        )?;
    }
    for recipe in terminal_expansions {
        owner.charge(SemanticMirResourceV1::ValidationWork, 1)?;
        if recipe.caller != function || terminal_argument_count_v1(recipe.expansion).is_none() {
            return Err(table("terminal expansion table"));
        }
        insert_sparse_binding_v1(
            &mut terminal_by_raw,
            recipe.rustc_block,
            recipe,
            "terminal expansion table",
        )?;
    }
    if direct_by_raw
        .iter()
        .zip(&terminal_by_raw)
        .any(|(direct, terminal)| direct.is_some() && terminal.is_some())
    {
        return Err(table("call binding table"));
    }
    Ok((direct_by_raw, terminal_by_raw))
}

fn insert_dense_binding_v1<'a, T>(
    table: &mut [Option<&'a T>],
    index: u32,
    value: &'a T,
    table_name: &'static str,
) -> Result<(), ProductionSemanticBodyErrorV1> {
    insert_sparse_binding_v1(table, index, value, table_name)
}

fn insert_sparse_binding_v1<'a, T>(
    bindings: &mut [Option<&'a T>],
    index: u32,
    value: &'a T,
    table_name: &'static str,
) -> Result<(), ProductionSemanticBodyErrorV1> {
    let index = usize::try_from(index).map_err(|_| table(table_name))?;
    let slot = bindings.get_mut(index).ok_or_else(|| table(table_name))?;
    if slot.replace(value).is_some() {
        return Err(table(table_name));
    }
    Ok(())
}

fn collect_dense_bindings_v1<T>(
    bindings: Vec<Option<T>>,
    table_name: &'static str,
) -> Result<Vec<T>, ProductionSemanticBodyErrorV1> {
    bindings
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| table(table_name))
}

fn normalize_type_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    raw: Ty<'tcx>,
) -> Result<Ty<'tcx>, ()> {
    instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(raw),
        )
        .map_err(|_| ())
}

fn resolve_direct_call_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    body: &Body<'tcx>,
    function: &Operand<'tcx>,
) -> Result<Instance<'tcx>, &'static str> {
    let raw = function.ty(body, tcx);
    let callable = caller
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(raw),
        )
        .map_err(|_| "callable type that failed monomorphic normalization")?;
    let TyKind::FnDef(def_id, arguments) = callable.kind() else {
        return Err("indirect or non-function-definition call");
    };
    Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *def_id, arguments)
        .map_err(|_| "direct call whose concrete rustc instance failed resolution")?
        .ok_or("direct call without a concrete rustc instance")
}

fn validate_export_role_v1(
    role: SemanticFunctionRoleV1,
    export: &ProductionSemanticFunctionExportV1,
) -> Result<(), ProductionSemanticBodyErrorV1> {
    let valid = matches!(
        (role, export),
        (
            SemanticFunctionRoleV1::KernelRoot,
            ProductionSemanticFunctionExportV1::Kernel(_)
        ) | (
            SemanticFunctionRoleV1::DeviceFfiExport,
            ProductionSemanticFunctionExportV1::DeviceFfi(_)
        ) | (
            SemanticFunctionRoleV1::InternalHelper | SemanticFunctionRoleV1::DropGlue(_),
            ProductionSemanticFunctionExportV1::None
        )
    );
    if valid {
        Ok(())
    } else {
        Err(table("function role/export binding"))
    }
}

fn semantic_local_role_v1(
    raw_local: u32,
    argument_count: usize,
) -> Result<SemanticLocalRoleV1, ProductionSemanticBodyErrorV1> {
    if raw_local == u32::try_from(RETURN_PLACE.index()).unwrap_or(0) {
        return Ok(SemanticLocalRoleV1::Return);
    }
    let raw = usize::try_from(raw_local).map_err(|_| table("local role"))?;
    if raw <= argument_count {
        let argument = raw.checked_sub(1).ok_or_else(|| table("local role"))?;
        Ok(SemanticLocalRoleV1::Argument(
            u32::try_from(argument).map_err(|_| table("local role"))?,
        ))
    } else {
        Ok(SemanticLocalRoleV1::Temporary)
    }
}

fn semantic_borrow_kind_v1(
    borrow: BorrowKind,
    block: Option<u32>,
    statement: Option<u32>,
) -> Result<SemanticBorrowKindV1, ProductionSemanticBodyErrorV1> {
    match borrow {
        BorrowKind::Shared => Ok(SemanticBorrowKindV1::Shared),
        BorrowKind::Fake(_) => Err(unsupported("fake borrow rvalue", block, statement)),
        BorrowKind::Mut {
            kind: MutBorrowKind::Default | MutBorrowKind::TwoPhaseBorrow,
        } => Ok(SemanticBorrowKindV1::Mutable),
        BorrowKind::Mut {
            kind: MutBorrowKind::ClosureCapture,
        } => Err(unsupported(
            "closure-capture mutable borrow rvalue",
            block,
            statement,
        )),
    }
}

const fn terminal_argument_count_v1(expansion: ProductionTerminalExpansionV1) -> Option<usize> {
    match expansion {
        ProductionTerminalExpansionV1::ThreadIndex1d => Some(0),
        ProductionTerminalExpansionV1::ThreadIndexGet => Some(1),
        ProductionTerminalExpansionV1::DisjointSliceGetMut => Some(2),
        ProductionTerminalExpansionV1::ThreadIndexIntoDisjoint
        | ProductionTerminalExpansionV1::ThreadIndexCheckedShift
        | ProductionTerminalExpansionV1::DisjointIndexGet
        | ProductionTerminalExpansionV1::DisjointIndexCheckedShift => Some(1),
        ProductionTerminalExpansionV1::ThreadIndexCheckedBlock => Some(1),
        ProductionTerminalExpansionV1::DisjointSliceGetDisjointMut => Some(2),
        ProductionTerminalExpansionV1::GridLeaderCurrent => Some(0),
        ProductionTerminalExpansionV1::DisjointSliceGetMutExclusive => Some(3),
        ProductionTerminalExpansionV1::DisjointSliceGetBlockMut => Some(3),
    }
}

fn semantic_binary_operation(operation: BinOp) -> Option<SemanticBinaryOpV1> {
    match operation {
        BinOp::Add => Some(SemanticBinaryOpV1::Add),
        BinOp::Sub => Some(SemanticBinaryOpV1::Subtract),
        BinOp::Mul => Some(SemanticBinaryOpV1::Multiply),
        BinOp::Div => Some(SemanticBinaryOpV1::Divide),
        BinOp::Rem => Some(SemanticBinaryOpV1::Remainder),
        BinOp::BitXor => Some(SemanticBinaryOpV1::BitXor),
        BinOp::BitAnd => Some(SemanticBinaryOpV1::BitAnd),
        BinOp::BitOr => Some(SemanticBinaryOpV1::BitOr),
        BinOp::Shl => Some(SemanticBinaryOpV1::ShiftLeft),
        BinOp::Shr => Some(SemanticBinaryOpV1::ShiftRight),
        BinOp::Eq => Some(SemanticBinaryOpV1::Equal),
        BinOp::Lt => Some(SemanticBinaryOpV1::LessThan),
        BinOp::Le => Some(SemanticBinaryOpV1::LessOrEqual),
        BinOp::Ne => Some(SemanticBinaryOpV1::NotEqual),
        BinOp::Ge => Some(SemanticBinaryOpV1::GreaterOrEqual),
        BinOp::Gt => Some(SemanticBinaryOpV1::GreaterThan),
        BinOp::Offset => Some(SemanticBinaryOpV1::Offset),
        BinOp::Cmp
        | BinOp::AddWithOverflow
        | BinOp::SubWithOverflow
        | BinOp::MulWithOverflow
        | BinOp::AddUnchecked
        | BinOp::SubUnchecked
        | BinOp::MulUnchecked
        | BinOp::ShlUnchecked
        | BinOp::ShrUnchecked => None,
    }
}

fn try_vec_v1<T>(
    capacity: usize,
    resource: SemanticMirResourceV1,
) -> Result<Vec<T>, ProductionSemanticBodyErrorV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| allocation(resource))?;
    Ok(values)
}

fn try_filled_vec_v1<T: Clone>(
    length: usize,
    value: T,
    resource: SemanticMirResourceV1,
) -> Result<Vec<T>, ProductionSemanticBodyErrorV1> {
    let mut values = try_vec_v1(length, resource)?;
    values.resize(length, value);
    Ok(values)
}

fn table(table: &'static str) -> ProductionSemanticBodyErrorV1 {
    ProductionSemanticBodyErrorV1::IdentityTableMismatch { table }
}

fn allocation(resource: SemanticMirResourceV1) -> ProductionSemanticBodyErrorV1 {
    ProductionSemanticBodyErrorV1::Allocation { resource }
}

fn unsupported(
    construct: impl Into<String>,
    block: Option<u32>,
    statement: Option<u32>,
) -> ProductionSemanticBodyErrorV1 {
    ProductionSemanticBodyErrorV1::Unsupported {
        construct: construct
            .into()
            .chars()
            .take(MAX_ERROR_COMPONENT_CHARS_V1)
            .collect(),
        block,
        statement,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_middle::mir::FakeBorrowKind;

    #[test]
    fn terminal_expansion_arities_are_closed() {
        assert_eq!(
            terminal_argument_count_v1(ProductionTerminalExpansionV1::ThreadIndex1d),
            Some(0)
        );
        assert_eq!(
            terminal_argument_count_v1(ProductionTerminalExpansionV1::ThreadIndexGet),
            Some(1)
        );
        assert_eq!(
            terminal_argument_count_v1(ProductionTerminalExpansionV1::DisjointSliceGetMut),
            Some(2)
        );
    }

    #[test]
    fn local_roles_follow_rustc_body_numbering() {
        assert_eq!(
            semantic_local_role_v1(0, 2).unwrap(),
            SemanticLocalRoleV1::Return
        );
        assert_eq!(
            semantic_local_role_v1(1, 2).unwrap(),
            SemanticLocalRoleV1::Argument(0)
        );
        assert_eq!(
            semantic_local_role_v1(2, 2).unwrap(),
            SemanticLocalRoleV1::Argument(1)
        );
        assert_eq!(
            semantic_local_role_v1(3, 2).unwrap(),
            SemanticLocalRoleV1::Temporary
        );
    }

    #[test]
    fn diagnostic_components_are_bounded() {
        let error = unsupported(
            "x".repeat(MAX_ERROR_COMPONENT_CHARS_V1 + 32),
            Some(7),
            Some(3),
        );
        let ProductionSemanticBodyErrorV1::Unsupported { construct, .. } = error else {
            panic!("unexpected error kind");
        };
        assert_eq!(construct.len(), MAX_ERROR_COMPONENT_CHARS_V1);
    }

    #[test]
    fn request_owner_accounting_is_cumulative_across_bodies() {
        let limits = SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::Functions, 1)
            .unwrap()
            .with_limit(SemanticMirResourceV1::ConstantBytes, 3)
            .unwrap();
        let mut owner = ProductionSemanticBodyRequestOwnerV1 {
            limits,
            totals: ConstructionTotalsV1::default(),
            callables: HashMap::new(),
        };

        owner.charge(SemanticMirResourceV1::Functions, 1).unwrap();
        let error = owner
            .charge(SemanticMirResourceV1::Functions, 1)
            .unwrap_err();
        assert!(matches!(
            error,
            ProductionSemanticBodyErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::Functions,
                actual: 2,
                maximum: 1,
            }
        ));

        owner
            .charge(SemanticMirResourceV1::ConstantBytes, 2)
            .unwrap();
        let error = owner
            .charge(SemanticMirResourceV1::ConstantBytes, 2)
            .unwrap_err();
        assert!(matches!(
            error,
            ProductionSemanticBodyErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::ConstantBytes,
                actual: 4,
                maximum: 3,
            }
        ));
    }

    #[test]
    fn callable_owner_rejects_id_and_kind_substitution() {
        let callable = SemanticCallableIdV1::from_index(4);
        assert!(require_canonical_callable_id_v1(4, callable).is_ok());
        assert!(require_canonical_callable_id_v1(3, callable).is_err());

        let record = ProductionSemanticCallableOwnerRecordV1 {
            kind: ProductionSemanticCallableOwnerKindV1::Terminal(
                ProductionTerminalExpansionV1::ThreadIndex1d,
            ),
            semantic_callable: callable,
        };
        assert!(
            resolve_owned_callable_record_v1(
                record,
                ProductionSemanticCallableOwnerKindV1::Terminal(
                    ProductionTerminalExpansionV1::ThreadIndex1d,
                ),
            )
            .is_ok()
        );
        assert!(
            resolve_owned_callable_record_v1(
                record,
                ProductionSemanticCallableOwnerKindV1::Terminal(
                    ProductionTerminalExpansionV1::ThreadIndexGet,
                ),
            )
            .is_err()
        );
        assert!(
            resolve_owned_callable_record_v1(
                record,
                ProductionSemanticCallableOwnerKindV1::Defined,
            )
            .is_err()
        );
    }

    #[test]
    fn fake_borrows_fail_closed_before_schema_construction() {
        for kind in [FakeBorrowKind::Deep, FakeBorrowKind::Shallow] {
            let error = semantic_borrow_kind_v1(BorrowKind::Fake(kind), Some(9), Some(2))
                .expect_err("fake borrows must never enter semantic MIR");
            assert!(matches!(
                error,
                ProductionSemanticBodyErrorV1::Unsupported {
                    block: Some(9),
                    statement: Some(2),
                    ..
                }
            ));
        }
    }
}
