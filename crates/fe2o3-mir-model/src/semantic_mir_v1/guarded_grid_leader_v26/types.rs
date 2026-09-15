#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticGuardedGridLeaderTypesV1 {
    pub grid_reference: SemanticTypeIdV1,
    pub grid: SemanticTypeIdV1,
    pub leader: SemanticTypeIdV1,
    pub leader_option: SemanticTypeIdV1,
    pub rank: SemanticTypeIdV1,
    pub boolean: SemanticTypeIdV1,
    pub grid_option: SemanticTypeIdV1,
    pub context_reference: SemanticTypeIdV1,
}

impl SemanticGuardedGridLeaderTypesV1 {
    pub const fn new(ids: [SemanticTypeIdV1; 8]) -> Self {
        let [
            grid_reference,
            grid,
            leader,
            leader_option,
            rank,
            boolean,
            grid_option,
            context_reference,
        ] = ids;
        Self {
            grid_reference,
            grid,
            leader,
            leader_option,
            rank,
            boolean,
            grid_option,
            context_reference,
        }
    }
    pub const fn all(self) -> [SemanticTypeIdV1; 8] {
        [
            self.grid_reference,
            self.grid,
            self.leader,
            self.leader_option,
            self.rank,
            self.boolean,
            self.grid_option,
            self.context_reference,
        ]
    }
}

/// Complete five-block roles, derived from edges rather than raw ordinals.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticGuardedGridLeaderBodyV1 {
    pub guard: SemanticBlockIdV1,
    pub issuer: SemanticBlockIdV1,
    pub some: SemanticBlockIdV1,
    pub none: SemanticBlockIdV1,
    pub exit: SemanticBlockIdV1,
    pub receiver: SemanticLocalIdV1,
    pub result: SemanticLocalIdV1,
    pub issued: SemanticLocalIdV1,
    pub issuer_callable: SemanticCallableIdV1,
}

/// One original, independently source-authenticated receiver construction.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticGuardedGridLeaderSourceV1 {
    pub caller: SemanticFunctionIdV1,
    pub call_block: SemanticBlockIdV1,
    pub grid_getter: SemanticFunctionIdV1,
    pub grid_current: SemanticFunctionIdV1,
    pub grid_call_block: SemanticBlockIdV1,
}
