mod semantic_constant;
mod semantic_memory;
mod semantic_type;

pub use semantic_constant::{
    MAX_CONSTANT_ALLOCATION_BYTES, MAX_CONSTANT_ALLOCATIONS, MAX_CONSTANT_GRAPH_DEPTH,
    MAX_CONSTANT_IDENTITY_BYTES, MAX_CONSTANT_RELOCATIONS, MAX_CONSTANT_TOTAL_BYTES,
    MAX_CONSTANT_WIRE_BYTES, MirAlignment, MirAllocationId, MirAllocationOrigin, MirByteOffset,
    MirConstantAllocation, MirConstantDecodeError, MirConstantIdentity, MirConstantRepresentation,
    MirConstantValidationError, MirInitializedMask, MirMemoryIdentity, MirPointerProvenance,
    MirPointerRelocation, MirPointerWidth, MirPromotedIdentity, MirSemanticConstantPool,
    MirStaticIdentity, MirSymbolIdentity,
};
pub use semantic_memory::{
    MAX_MEMORY_OPERATION_WIRE_BYTES, MirCopyNonOverlappingContract, MirElementCount,
    MirMemoryAccessContract, MirMemoryContractDecodeError, MirMemoryContractValidationError,
    MirMemoryPermission, MirOperationProvenance, MirOverlapContract, MirPointerDistanceContract,
    MirPointerDistanceResult, MirPointerDistanceUnit, MirPointerOperandContract,
    MirProvenanceRegion, MirSemanticMemoryOperation, MirVolatileAccessContract,
};
pub use semantic_type::{
    MirAddressSpace, MirAggregateLayout, MirEnumEncoding, MirEnumType, MirField, MirLayout,
    MirMutability, MirPadding, MirScalarType, MirSemanticType, MirStructType, MirTypeKind,
    MirTypeValidationError, MirVariant,
};

pub const DIALECT: &str = "mir";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOpRecord {
    pub op: MirOp,
    pub attrs: Vec<MirAttr>,
}

impl MirOpRecord {
    pub fn new(op: MirOp) -> Self {
        Self {
            op,
            attrs: Vec::new(),
        }
    }

    pub fn with_attr(mut self, attr: MirAttr) -> Self {
        self.attrs.push(attr);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirAttr {
    pub name: &'static str,
    pub value: MirAttrValue,
}

impl MirAttr {
    pub fn string(name: &'static str, value: impl Into<String>) -> Self {
        Self {
            name,
            value: MirAttrValue::String(value.into()),
        }
    }

    pub fn usize(name: &'static str, value: usize) -> Self {
        Self {
            name,
            value: MirAttrValue::Usize(value),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirAttrValue {
    String(String),
    Usize(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirOp {
    Module,
    Func,
    Block,
    Statement,
    Arg,
    Local,
    Const,
    Assign,
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Lt,
    Le,
    Ne,
    Ge,
    Gt,
    Cmp,
    Cast,
    Load,
    Store,
    Drop,
    Branch,
    CondBranch,
    Switch,
    Call,
    Return,
    Assert,
    Unreachable,
    ThreadIndex1d,
    SliceLen,
    SlicePtr,
    Gep,
    PointerDistance,
    VolatileLoad,
    VolatileStore,
    CopyNonOverlapping,
    Other,
}

impl MirOp {
    pub fn name(self) -> &'static str {
        match self {
            Self::Module => "mir.module",
            Self::Func => "mir.func",
            Self::Block => "mir.block",
            Self::Statement => "mir.statement",
            Self::Arg => "mir.arg",
            Self::Local => "mir.local",
            Self::Const => "mir.const",
            Self::Assign => "mir.assign",
            Self::Add => "mir.add",
            Self::Sub => "mir.sub",
            Self::Mul => "mir.mul",
            Self::Div => "mir.div",
            Self::Eq => "mir.eq",
            Self::Lt => "mir.lt",
            Self::Le => "mir.le",
            Self::Ne => "mir.ne",
            Self::Ge => "mir.ge",
            Self::Gt => "mir.gt",
            Self::Cmp => "mir.cmp",
            Self::Cast => "mir.cast",
            Self::Load => "mir.load",
            Self::Store => "mir.store",
            Self::Drop => "mir.drop",
            Self::Branch => "mir.br",
            Self::CondBranch => "mir.cond_br",
            Self::Switch => "mir.switch",
            Self::Call => "mir.call",
            Self::Return => "mir.return",
            Self::Assert => "mir.assert",
            Self::Unreachable => "mir.unreachable",
            Self::ThreadIndex1d => "mir.thread_index_1d",
            Self::SliceLen => "mir.slice_len",
            Self::SlicePtr => "mir.slice_ptr",
            Self::Gep => "mir.gep",
            Self::PointerDistance => "mir.pointer_distance",
            Self::VolatileLoad => "mir.volatile_load",
            Self::VolatileStore => "mir.volatile_store",
            Self::CopyNonOverlapping => "mir.copy_nonoverlapping",
            Self::Other => "mir.other",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirType {
    I1,
    I32,
    I64,
    USize,
    F32,
    F64,
    Ptr,
    Slice,
    DisjointSlice,
    Unit,
    Unknown,
}

impl MirType {
    pub fn name(self) -> &'static str {
        match self {
            Self::I1 => "mir.i1",
            Self::I32 => "mir.i32",
            Self::I64 => "mir.i64",
            Self::USize => "mir.usize",
            Self::F32 => "mir.f32",
            Self::F64 => "mir.f64",
            Self::Ptr => "mir.ptr",
            Self::Slice => "mir.slice",
            Self::DisjointSlice => "mir.disjoint_slice",
            Self::Unit => "mir.unit",
            Self::Unknown => "mir.unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_names_are_dialect_qualified() {
        assert_eq!(MirOp::Func.name(), "mir.func");
        assert_eq!(MirOp::Assign.name(), "mir.assign");
        assert_eq!(MirOp::Lt.name(), "mir.lt");
        assert_eq!(MirOp::ThreadIndex1d.name(), "mir.thread_index_1d");
        assert_eq!(MirOp::PointerDistance.name(), "mir.pointer_distance");
        assert_eq!(MirOp::VolatileLoad.name(), "mir.volatile_load");
        assert_eq!(MirOp::VolatileStore.name(), "mir.volatile_store");
        assert_eq!(MirOp::CopyNonOverlapping.name(), "mir.copy_nonoverlapping");
    }

    #[test]
    fn type_names_are_dialect_qualified() {
        assert_eq!(MirType::USize.name(), "mir.usize");
        assert_eq!(MirType::DisjointSlice.name(), "mir.disjoint_slice");
    }

    #[test]
    fn op_records_carry_attributes() {
        let record = MirOpRecord::new(MirOp::Func)
            .with_attr(MirAttr::string("symbol", "vecadd"))
            .with_attr(MirAttr::usize("args", 3));

        assert_eq!(record.op, MirOp::Func);
        assert_eq!(record.attrs.len(), 2);
    }
}
