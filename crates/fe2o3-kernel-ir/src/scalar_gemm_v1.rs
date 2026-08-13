//! Canonical scalar `f32` GEMM checkpoint for the exact gfx942 profile.
//!
//! This module deliberately describes one algorithm, not a general GEMM
//! dialect. Its exact graph is the boundary that later Rust-source and Verus
//! evidence must reproduce before executable authority can be considered.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{
    AccessMode, AddressSpace, Axis, BasicBlock, BinaryOp, BlockId, CastKind, ComparePredicate,
    Constant, Function, IntrinsicKind, IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent,
    MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature, TargetCapability,
    Terminator, Type, ValueDef, ValueId, VerificationErrors, WaveWidth,
    gfx942_xnack_minus_target_capability, verify_module,
};

pub const SCALAR_GEMM_V1_MODULE_ID: &str = "fe2o3::scalar_gemm_v1";
pub const SCALAR_GEMM_V1_FUNCTION_ID: &str = "__fe2o3_scalar_gemm_v1_impl";
pub const SCALAR_GEMM_V1_KERNEL_ID: &str = "scalar_gemm_v1";
pub const SCALAR_GEMM_V1_WORKGROUP_SIZE: u32 = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarGemmArchitectureV1 {
    Gfx942,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarGemmXnackV1 {
    Disabled,
    Enabled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarGemmCodeObjectV1 {
    V5,
    V6,
}

/// Closed target requirements accepted by scalar GEMM V1.
///
/// These fields are requirements on a later inspected artifact. They are not
/// evidence that textual LLVM was compiled to COV6 or that a code object has
/// the requested target metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarGemmTargetRequirementsV1 {
    pub architecture: ScalarGemmArchitectureV1,
    pub xnack: ScalarGemmXnackV1,
    pub code_object: ScalarGemmCodeObjectV1,
}

impl ScalarGemmTargetRequirementsV1 {
    pub const fn gfx942_xnack_minus_cov6() -> Self {
        Self {
            architecture: ScalarGemmArchitectureV1::Gfx942,
            xnack: ScalarGemmXnackV1::Disabled,
            code_object: ScalarGemmCodeObjectV1::V6,
        }
    }

    pub const fn is_exact(self) -> bool {
        matches!(
            self,
            Self {
                architecture: ScalarGemmArchitectureV1::Gfx942,
                xnack: ScalarGemmXnackV1::Disabled,
                code_object: ScalarGemmCodeObjectV1::V6,
            }
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScalarGemmV1Error {
    UnsupportedTargetRequirements,
    InvalidKernelIr(VerificationErrors),
    NonCanonicalKernelIr,
}

impl fmt::Display for ScalarGemmV1Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedTargetRequirements => formatter.write_str(
                "scalar GEMM V1 requires gfx942:xnack- and requires downstream COV6 inspection",
            ),
            Self::InvalidKernelIr(error) => error.fmt(formatter),
            Self::NonCanonicalKernelIr => formatter.write_str(
                "kernel IR does not match the canonical scalar GEMM V1 cyclic-SSA graph",
            ),
        }
    }
}

impl Error for ScalarGemmV1Error {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidKernelIr(error) => Some(error),
            _ => None,
        }
    }
}

/// Constructs the only Kernel IR graph admitted by scalar GEMM V1.
pub fn scalar_gemm_v1_module() -> Module {
    let read_slice = Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadOnly);
    let write_slice = Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);
    let read_pointer = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadOnly);
    let write_pointer = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);
    let access = MemoryAccess::new(AddressSpace::Global, 4);

    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        value_op(
            6,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: crate::IndexKind::Global,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        value_op(7, Type::INDEX, zext(3)),
        value_op(8, Type::INDEX, zext(4)),
        value_op(9, Type::INDEX, zext(5)),
        value_op(10, Type::INDEX, binary(BinaryOp::Multiply, 7, 8)),
        value_op(
            11,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(6),
                rhs: ValueId(10),
            },
        ),
        value_op(
            12,
            read_pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        value_op(
            13,
            read_pointer.clone(),
            OperationKind::SliceData { slice: ValueId(1) },
        ),
        value_op(
            14,
            write_pointer.clone(),
            OperationKind::SliceData { slice: ValueId(2) },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(11),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(5),
        else_arguments: vec![],
    });

    let mut coordinates = BasicBlock::new(BlockId(1));
    coordinates.operations = vec![
        value_op(15, Type::INDEX, binary(BinaryOp::Divide, 6, 8)),
        value_op(16, Type::INDEX, binary(BinaryOp::Remainder, 6, 8)),
        value_op(
            17,
            Type::Scalar(ScalarType::U32),
            OperationKind::Constant(Constant::U32(0)),
        ),
        value_op(18, Type::F32, OperationKind::Constant(Constant::F32Bits(0))),
    ];
    coordinates.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![ValueId(17), ValueId(18)],
    });

    let mut header = BasicBlock::new(BlockId(2));
    header.parameters = vec![
        ValueDef::new(ValueId(19), Type::Scalar(ScalarType::U32)),
        ValueDef::new(ValueId(20), Type::F32),
    ];
    header.operations = vec![value_op(
        21,
        Type::BOOL,
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(19),
            rhs: ValueId(5),
        },
    )];
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(21),
        then_target: BlockId(3),
        then_arguments: vec![],
        else_target: BlockId(4),
        else_arguments: vec![ValueId(20)],
    });

    let mut body = BasicBlock::new(BlockId(3));
    body.operations = vec![
        value_op(22, Type::INDEX, zext(19)),
        value_op(23, Type::INDEX, binary(BinaryOp::Multiply, 15, 9)),
        value_op(24, Type::INDEX, binary(BinaryOp::Add, 23, 22)),
        value_op(25, Type::INDEX, binary(BinaryOp::Multiply, 22, 8)),
        value_op(26, Type::INDEX, binary(BinaryOp::Add, 25, 16)),
        value_op(
            27,
            read_pointer.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(12),
                offset: ValueId(24),
            },
        ),
        value_op(
            28,
            Type::F32,
            OperationKind::Load {
                pointer: ValueId(27),
                access,
            },
        ),
        value_op(
            29,
            read_pointer,
            OperationKind::GetElementPointer {
                base: ValueId(13),
                offset: ValueId(26),
            },
        ),
        value_op(
            30,
            Type::F32,
            OperationKind::Load {
                pointer: ValueId(29),
                access,
            },
        ),
        value_op(31, Type::F32, binary(BinaryOp::Multiply, 28, 30)),
        value_op(32, Type::F32, binary(BinaryOp::Add, 20, 31)),
        value_op(
            33,
            Type::Scalar(ScalarType::U32),
            OperationKind::Constant(Constant::U32(1)),
        ),
        value_op(
            34,
            Type::Scalar(ScalarType::U32),
            binary(BinaryOp::Add, 19, 33),
        ),
    ];
    body.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![ValueId(34), ValueId(32)],
    });

    let mut store = BasicBlock::new(BlockId(4));
    store.parameters = vec![ValueDef::new(ValueId(35), Type::F32)];
    store.operations = vec![
        value_op(
            36,
            write_pointer,
            OperationKind::GetElementPointer {
                base: ValueId(14),
                offset: ValueId(6),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(36),
                value: ValueId(35),
                access,
            },
        ),
    ];
    store.terminator = Some(Terminator::Return { values: vec![] });

    let mut inactive = BasicBlock::new(BlockId(5));
    inactive.terminator = Some(Terminator::Return { values: vec![] });

    let mut function = Function::kernel_entry(
        SCALAR_GEMM_V1_FUNCTION_ID,
        Signature::new(
            vec![
                read_slice.clone(),
                read_slice,
                write_slice,
                Type::Scalar(ScalarType::U32),
                Type::Scalar(ScalarType::U32),
                Type::Scalar(ScalarType::U32),
            ],
            vec![],
        ),
        (0..6).map(ValueId).collect(),
        vec![entry, coordinates, header, body, store, inactive],
    );
    function.required_capabilities = exact_capabilities();

    let mut kernel = Kernel::new(
        SCALAR_GEMM_V1_KERNEL_ID,
        SCALAR_GEMM_V1_FUNCTION_ID,
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(crate::WorkgroupSize::new(
        SCALAR_GEMM_V1_WORKGROUP_SIZE,
        1,
        1,
    ));
    kernel.required_capabilities = exact_capabilities();

    let mut module = Module::new(SCALAR_GEMM_V1_MODULE_ID);
    module.required_capabilities = exact_capabilities();
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

/// Verifies ordinary Kernel IR invariants and the exact GEMM graph. Exact
/// equality intentionally rejects helper calls, extra roots, alternative
/// arithmetic association, index truncation, and non-injective C addressing.
pub fn verify_scalar_gemm_v1_module(
    module: &Module,
    requirements: ScalarGemmTargetRequirementsV1,
) -> Result<(), ScalarGemmV1Error> {
    if !requirements.is_exact() {
        return Err(ScalarGemmV1Error::UnsupportedTargetRequirements);
    }
    // Run the bounded structural verifier before comparing an untrusted graph
    // with the canonical graph. This keeps malformed and oversized inputs on
    // the verifier's resource-limited path.
    verify_module(module).map_err(ScalarGemmV1Error::InvalidKernelIr)?;
    if module != &scalar_gemm_v1_module() {
        return Err(ScalarGemmV1Error::NonCanonicalKernelIr);
    }
    Ok(())
}

fn exact_capabilities() -> BTreeSet<TargetCapability> {
    BTreeSet::from([
        gfx942_xnack_minus_target_capability(),
        TargetCapability::WaveWidth(WaveWidth::Wave64),
    ])
}

fn value_op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}

fn zext(value: u32) -> OperationKind {
    OperationKind::Cast {
        kind: CastKind::ZeroExtend,
        value: ValueId(value),
        to: Type::INDEX,
    }
}

fn binary(op: BinaryOp, lhs: u32, rhs: u32) -> OperationKind {
    OperationKind::Binary {
        op,
        lhs: ValueId(lhs),
        rhs: ValueId(rhs),
    }
}
