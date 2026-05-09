#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dim {
    X,
    Y,
    Z,
}

impl Dim {
    pub fn suffix(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
            Self::Z => "z",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmdgcnIntrinsic {
    WorkItemId(Dim),
    WorkGroupId(Dim),
    DispatchPtr,
    SBarrier,
}

impl AmdgcnIntrinsic {
    pub fn llvm_name(self) -> &'static str {
        match self {
            Self::WorkItemId(Dim::X) => "llvm.amdgcn.workitem.id.x",
            Self::WorkItemId(Dim::Y) => "llvm.amdgcn.workitem.id.y",
            Self::WorkItemId(Dim::Z) => "llvm.amdgcn.workitem.id.z",
            Self::WorkGroupId(Dim::X) => "llvm.amdgcn.workgroup.id.x",
            Self::WorkGroupId(Dim::Y) => "llvm.amdgcn.workgroup.id.y",
            Self::WorkGroupId(Dim::Z) => "llvm.amdgcn.workgroup.id.z",
            Self::DispatchPtr => "llvm.amdgcn.dispatch.ptr",
            Self::SBarrier => "llvm.amdgcn.s.barrier",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressSpace {
    Generic,
    Global,
    Region,
    Local,
    Constant,
    Private,
    Constant32Bit,
    BufferFatPointer,
}

impl AddressSpace {
    pub fn llvm_id(self) -> u32 {
        match self {
            Self::Generic => 0,
            Self::Global => 1,
            Self::Region => 2,
            Self::Local => 3,
            Self::Constant => 4,
            Self::Private => 5,
            Self::Constant32Bit => 6,
            Self::BufferFatPointer => 7,
        }
    }
}

pub const AMDGPU_TRIPLE: &str = "amdgcn-amd-amdhsa";
