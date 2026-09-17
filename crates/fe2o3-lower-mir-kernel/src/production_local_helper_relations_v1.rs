// Source/N relations are sealed only after exact final-subject validation.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UnitLocalAssociationKeyV1 {
    root: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    physical: usize,
}

impl UnitLocalAssociationKeyV1 {
    fn key(self) -> [usize; 3] {
        [
            self.root.index() as usize,
            self.function.index() as usize,
            self.physical,
        ]
    }
}

#[derive(Clone, Copy)]
struct UnitLocalPhysicalRowsV1<'a> {
    functions: &'a [RetainedHelperKindV1],
    allocations: &'a [RetainedLocalAllocationV1],
    accesses: &'a [RetainedLocalAccessV1],
    control: &'a [RetainedLocalControlV1],
    edge_bindings: &'a [RetainedLocalEdgeBindingV1],
}

#[derive(Clone, Copy, Debug)]
struct UnitLocalBodyRowV1 {
    physical: usize,
    allocations: (usize, usize),
    accesses: (usize, usize),
    control: (usize, usize),
    edge_bindings: (usize, usize),
}

#[derive(Clone, Copy, Debug)]
#[allow(
    dead_code,
    reason = "Retained source evidence; downstream consumers remain gated."
)]
struct UnitLocalAssociationRowV1 {
    key: UnitLocalAssociationKeyV1,
    source_identity: fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdentityV1,
    plan_identity: fe2o3_mir_model::SsaPlanIdentityV1,
    body: usize,
    values: (usize, usize),
    memory: (usize, usize),
    control: (usize, usize),
    unit_return_local: SemanticLocalIdV1,
    unit_type: SemanticTypeIdV1,
    return_control: usize,
    call_count: usize,
}

#[derive(Clone, Copy, Debug)]
#[allow(
    dead_code,
    reason = "Retained source evidence; downstream consumers remain gated."
)]
enum UnitLocalValueOriginV1 {
    Event {
        occurrence: usize,
    },
    Constant {
        occurrence: usize,
    },
    Assignment {
        block: SemanticBlockIdV1,
        statement: u32,
        local: SemanticLocalIdV1,
    },
    Edge {
        edge: fe2o3_mir_model::SsaEdgeIdV1,
        variable: fe2o3_mir_model::SsaVariableIdV1,
    },
    UnitReturn {
        block: SemanticBlockIdV1,
        local: SemanticLocalIdV1,
    },
}

#[derive(Clone, Copy, Debug)]
enum UnitLocalNativeValueV1 {
    Scalar {
        value: ValueId,
        scalar: ScalarType,
        definition: Option<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>,
    },
    IgnoredUnit,
    // Real source Assert diagnostics need not have emitted N definitions.
    DiagnosticOnly,
}

#[derive(Clone, Copy, Debug)]
#[allow(
    dead_code,
    reason = "Retained source evidence; downstream consumers remain gated."
)]
enum UnitLocalValueRecipeV1 {
    Unit,
    Literal {
        bits: u64,
    },
    Copy {
        input: usize,
    },
    Cast {
        input: usize,
        kind: SemanticCastKindV1,
    },
    Compare {
        left: usize,
        right: usize,
        predicate: fe2o3_kernel_ir::ComparePredicate,
    },
    ArrayLength {
        local: SemanticLocalIdV1,
        allocation: usize,
    },
    Load {
        access: usize,
    },
    Edge {
        input: usize,
        physical_binding: Option<usize>,
    },
}

#[derive(Clone, Copy, Debug)]
#[allow(
    dead_code,
    reason = "Retained source evidence; downstream consumers remain gated."
)]
struct UnitLocalValueRowV1 {
    // Explicit caller key also permits ignored Unit rows for kernel-entry calls.
    key: UnitLocalAssociationKeyV1,
    origin: UnitLocalValueOriginV1,
    role: Option<fe2o3_pliron::ProductionSemanticSsaOperandRoleV1>,
    source_type: SemanticTypeIdV1,
    source_ssa: Option<SsaValueV1>,
    native: UnitLocalNativeValueV1,
    recipe: UnitLocalValueRecipeV1,
    // Exact source bits, not a substitute for source identity or core knownness.
    known_bits: Option<u64>,
}

#[derive(Clone, Copy, Debug)]
enum UnitLocalInvalidateV1 {
    Move,
    StorageLive,
    StorageDead,
    Deinitialize,
}

#[derive(Clone, Copy, Debug)]
#[allow(
    dead_code,
    reason = "Retained source evidence; downstream consumers remain gated."
)]
enum UnitLocalMemoryRowV1 {
    Allocation {
        key: UnitLocalAssociationKeyV1,
        local: SemanticLocalIdV1,
        source_type: SemanticTypeIdV1,
        physical_allocation: usize,
        array_slot: Option<usize>,
        element: ScalarType,
        count: u64,
        element_bytes: u64,
        alignment: u32,
    },
    Access {
        // Existing full owner/body/block/statement/role-tag/role-ordinal/component key.
        source_key: [usize; 7],
        key: UnitLocalAssociationKeyV1,
        local: SemanticLocalIdV1,
        allocation: usize,
        cell: u64,
        physical_access: usize,
        value: usize,
        // Exact earlier source Access row, not another equal-valued Store.
        latest_source_store: Option<usize>,
    },
    Invalidate {
        key: UnitLocalAssociationKeyV1,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: Option<fe2o3_pliron::ProductionSemanticSsaOperandRoleV1>,
        local: SemanticLocalIdV1,
        reason: UnitLocalInvalidateV1,
    },
}

#[derive(Clone, Copy, Debug)]
#[allow(
    dead_code,
    reason = "Retained source evidence; downstream consumers remain gated."
)]
enum UnitLocalControlKindV1 {
    Goto {
        edge: fe2o3_mir_model::SsaEdgeIdV1,
        target: SemanticBlockIdV1,
        physical_target: BlockId,
        core_bindings: (usize, usize),
        staged_values: (usize, usize),
    },
    Assert {
        edge: fe2o3_mir_model::SsaEdgeIdV1,
        target: SemanticBlockIdV1,
        physical_target: BlockId,
        predicate: usize,
        length: usize,
        index: usize,
        expected: bool,
        selected_successor: u32,
        inactive_block: BlockId,
        core_bindings: (usize, usize),
        staged_values: (usize, usize),
    },
    Return {
        local: SemanticLocalIdV1,
        unit_type: SemanticTypeIdV1,
        unit_value: usize,
    },
    InactiveTrap {
        synthetic_span: usize,
    },
}

#[derive(Clone, Copy, Debug)]
#[allow(
    dead_code,
    reason = "Retained source evidence; downstream consumers remain gated."
)]
struct UnitLocalControlRowV1 {
    association: usize,
    source_block: Option<SemanticBlockIdV1>,
    physical_block: BlockId,
    physical_control: usize,
    kind: UnitLocalControlKindV1,
}

#[derive(Clone, Copy, Debug)]
#[allow(
    dead_code,
    reason = "Retained source evidence; downstream consumers remain gated."
)]
struct UnitLocalCallRowV1 {
    caller: UnitLocalAssociationKeyV1,
    source_block: SemanticBlockIdV1,
    callee_association: usize,
    call: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    destination_local: SemanticLocalIdV1,
    unit_type: SemanticTypeIdV1,
    source_edge: fe2o3_mir_model::SsaEdgeIdV1,
    continuation_source: SemanticBlockIdV1,
    continuation_physical: BlockId,
    ignored_values: (usize, usize),
    return_control: usize,
}

#[derive(Debug)]
struct SealedUnitLocalSourceV1 {
    bodies: Vec<UnitLocalBodyRowV1>,
    associations: Vec<UnitLocalAssociationRowV1>,
    values: Vec<UnitLocalValueRowV1>,
    memory: Vec<UnitLocalMemoryRowV1>,
    control: Vec<UnitLocalControlRowV1>,
    calls: Vec<UnitLocalCallRowV1>,
}

impl SealedUnitLocalSourceV1 {
    fn empty() -> Self {
        Self {
            bodies: Vec::new(),
            associations: Vec::new(),
            values: Vec::new(),
            memory: Vec::new(),
            control: Vec::new(),
            calls: Vec::new(),
        }
    }
    fn is_empty(&self) -> bool {
        self.associations.is_empty()
    }
}

struct UnitLocalAssociationInputV1<'a> {
    association: usize,
    key: UnitLocalAssociationKeyV1,
    subject: CanonicalCallSubjectV1<'a>,
    origins: &'a SealedAssertOriginsV1,
    source: &'a SemanticFunctionDeclV1,
    plan: &'a fe2o3_pliron::ProductionSemanticSsaFunctionPlanV1,
    occurrences: fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'a>,
    physical_function: &'a Function,
    physical: UnitLocalPhysicalRowsV1<'a>,
    body: UnitLocalBodyRowV1,
    unit_return_local: SemanticLocalIdV1,
    unit_type: SemanticTypeIdV1,
}

#[derive(Clone, Copy)]
enum UnitLocalOperandUseV1 {
    Native(ValueId),
    Next,
    Diagnostic,
}

enum UnitLocalControlStepV1 {
    Return {
        control: usize,
    },
    Continue {
        target: SemanticBlockIdV1,
        staged_values: (usize, usize),
    },
}
