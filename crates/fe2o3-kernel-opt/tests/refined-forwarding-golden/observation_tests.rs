use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BinaryOp, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKirOperationCoordinateV1 as Site, CheckedBinaryOperator, ComparePredicate, Constant,
    FunctionRole, Module, Operation, OperationKind as Kind, ScalarType, Terminator, Type, ValueDef,
    ValueId, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::fmt::Write;

pub const WORK: usize = 500_000_000;
pub const STORAGE: usize = 128 << 20;

#[derive(Clone, Copy)]
pub struct Case {
    pub name: &'static str,
    pub wire: &'static str,
    pub golden: &'static str,
    pub splits: usize,
    pub forwards: usize,
}
macro_rules! case {
    ($name:literal, $splits:literal, $forwards:literal) => {
        Case {
            name: $name,
            wire: include_str!(concat!($name, ".hex")),
            golden: include_str!(concat!($name, ".golden")),
            splits: $splits,
            forwards: $forwards,
        }
    };
}
pub const CASES: [Case; 13] = [
    case!("dynamic-loop", 1, 1),
    case!("diamond", 1, 1),
    case!("duplicate-edge", 1, 1),
    case!("two-functions", 2, 2),
    case!("ungrounded-phi", 0, 0),
    case!("step-two", 0, 1),
    case!("unchecked-add", 0, 1),
    case!("global-clobber", 1, 0),
    case!("trap-cut", 1, 0),
    case!("volatile", 1, 0),
    case!("alignment", 1, 0),
    case!("noop", 0, 0),
    case!("different-stores", 1, 0),
];

// The existing frozen-wire convention: whitespace plus lowercase hex only.
// This decodes transport, not IR. The production bounded decoder owns admission.
pub fn hex(text: &str) -> Vec<u8> {
    assert!(text.len() <= 64 * 1024);
    let digits: Vec<_> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    assert_eq!(digits.len() % 2, 0);
    let nibble = |b| match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        _ => panic!("noncanonical hex transport"),
    };
    digits
        .chunks_exact(2)
        .map(|pair| nibble(pair[0]) * 16 + nibble(pair[1]))
        .collect()
}

pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> (Owner, usize) {
    let floor = budget.storage();
    let (owner, receipt) =
        Owner::from_canonical_bytes_with_verification_budget_v12(bytes, budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(owner.canonical().canonical_bytes(), bytes);
    (owner, receipt.retained_storage())
}

fn ty(value: &Type) -> &'static str {
    match value {
        Type::Scalar(ScalarType::Bool) => "Bool",
        Type::Scalar(ScalarType::U32) => "U32",
        Type::Scalar(ScalarType::U64) => "U64",
        Type::Pointer(pointer) => {
            assert_eq!(pointer.access, AccessMode::ReadWrite);
            assert_eq!(*pointer.pointee, Type::Scalar(ScalarType::U32));
            match pointer.address_space {
                AddressSpace::Private => "PtrPrivate",
                AddressSpace::Global => "PtrGlobal",
                _ => panic!("outside closed pointer grammar"),
            }
        }
        _ => panic!("outside closed type grammar: {value:?}"),
    }
}
fn values(values: &[ValueId]) -> String {
    values
        .iter()
        .map(|id| format!("v{}", id.0))
        .collect::<Vec<_>>()
        .join(",")
}
fn definitions(values: &[ValueDef]) -> String {
    values
        .iter()
        .map(|v| format!("v{}:{}", v.id.0, ty(&v.ty)))
        .collect::<Vec<_>>()
        .join(",")
}
pub fn site(at: Site) -> String {
    format!(
        "f{}:b{}:o{}",
        at.block.function.0, at.block.block, at.operation
    )
}
pub fn operation(module: &Module, at: Site) -> &Operation {
    &module.functions[at.block.function.0 as usize]
        .body
        .as_ref()
        .unwrap()
        .blocks[at.block.block as usize]
        .operations[at.operation as usize]
}

pub fn dump(text: &mut String, label: &str, module: &Module) {
    assert!(module.kernels.is_empty());
    assert!(module.required_capabilities.is_empty());
    writeln!(
        text,
        "{label}: module {} kernels=0 capabilities=[]",
        module.id.as_str()
    )
    .unwrap();
    for (fi, f) in module.functions.iter().enumerate() {
        assert_eq!(f.role, FunctionRole::InternalHelper);
        assert!(f.required_capabilities.is_empty());
        let body = f.body.as_ref().unwrap();
        assert_eq!(body.parameters.len(), f.signature.parameters.len());
        let parameters = body
            .parameters
            .iter()
            .zip(&f.signature.parameters)
            .map(|(id, t)| format!("v{}:{}", id.0, ty(t)))
            .collect::<Vec<_>>()
            .join(",");
        let results = f
            .signature
            .results
            .iter()
            .map(ty)
            .collect::<Vec<_>>()
            .join(",");
        writeln!(text, "{label}: f{fi} {} role=InternalHelper params=[{parameters}] results=[{results}] capabilities=[]", f.id.as_str()).unwrap();
        for (bi, b) in body.blocks.iter().enumerate() {
            writeln!(
                text,
                "{label}: f{fi}:b{bi} block-id={} params=[{}]",
                b.id.0,
                definitions(&b.parameters)
            )
            .unwrap();
            for (oi, op) in b.operations.iter().enumerate() {
                let kind = match &op.kind {
                    Kind::Constant(Constant::Bool(v)) => format!("Constant Bool({v})"),
                    Kind::Constant(Constant::U32(v)) => format!("Constant U32({v})"),
                    Kind::Constant(Constant::U64(v)) => format!("Constant U64({v})"),
                    Kind::Alloca {
                        element,
                        count,
                        address_space,
                        alignment,
                    } => {
                        assert_eq!(*element, Type::Scalar(ScalarType::U32));
                        assert!(count.is_none());
                        assert_eq!(*address_space, AddressSpace::Private);
                        format!("Alloca U32 private align={alignment} count=none")
                    }
                    Kind::Load { pointer, access } => format!(
                        "Load v{} {:?} align={} volatile={}",
                        pointer.0, access.address_space, access.alignment, access.volatile
                    ),
                    Kind::Store {
                        pointer,
                        value,
                        access,
                    } => format!(
                        "Store v{},v{} {:?} align={} volatile={}",
                        pointer.0, value.0, access.address_space, access.alignment, access.volatile
                    ),
                    Kind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs,
                        rhs,
                    } => format!("LessThan v{},v{}", lhs.0, rhs.0),
                    Kind::Binary {
                        op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                        lhs,
                        rhs,
                    } => format!("CheckedAdd v{},v{}", lhs.0, rhs.0),
                    Kind::Binary {
                        op: op @ (BinaryOp::Add | BinaryOp::Divide | BinaryOp::BitOr),
                        lhs,
                        rhs,
                    } => format!("{op:?} v{},v{}", lhs.0, rhs.0),
                    _ => panic!("outside closed operation grammar: {:?}", op.kind),
                };
                writeln!(
                    text,
                    "{label}: f{fi}:b{bi}:o{oi} [{}] {kind}",
                    definitions(&op.results)
                )
                .unwrap();
            }
            let term = match b.terminator.as_ref().unwrap() {
                Terminator::Branch { target, arguments } => {
                    format!("Branch bb{} [{}]", target.0, values(arguments))
                }
                Terminator::ConditionalBranch {
                    condition,
                    then_target,
                    then_arguments,
                    else_target,
                    else_arguments,
                } => format!(
                    "Conditional v{} bb{} [{}] bb{} [{}]",
                    condition.0,
                    then_target.0,
                    values(then_arguments),
                    else_target.0,
                    values(else_arguments)
                ),
                Terminator::Return { values: returned } => format!("Return [{}]", values(returned)),
                _ => panic!("outside closed terminator grammar"),
            };
            writeln!(text, "{label}: f{fi}:b{bi} {term}").unwrap();
        }
    }
}
