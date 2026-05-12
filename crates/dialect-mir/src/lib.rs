pub const DIALECT: &str = "mir";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirOp {
    Module,
    Func,
    Block,
    Arg,
    Const,
    Add,
    Sub,
    Mul,
    Div,
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
}

impl MirOp {
    pub fn name(self) -> &'static str {
        match self {
            Self::Module => "mir.module",
            Self::Func => "mir.func",
            Self::Block => "mir.block",
            Self::Arg => "mir.arg",
            Self::Const => "mir.const",
            Self::Add => "mir.add",
            Self::Sub => "mir.sub",
            Self::Mul => "mir.mul",
            Self::Div => "mir.div",
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
        assert_eq!(MirOp::ThreadIndex1d.name(), "mir.thread_index_1d");
    }

    #[test]
    fn type_names_are_dialect_qualified() {
        assert_eq!(MirType::USize.name(), "mir.usize");
        assert_eq!(MirType::DisjointSlice.name(), "mir.disjoint_slice");
    }
}
