//! Real canonical/optimizer components; no manufactured source request or proof.
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, ComparePredicate, Constant, Function,
    IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation,
    OperationKind as Kind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
    WorkgroupSize,
};
pub(super) const WORK: usize = 1_000_000_000;
pub(super) const STORAGE: usize = 256 * 1024 * 1024;
pub(super) const FLOOR: usize = 71;

fn op(id: u32, ty: Type, kind: Kind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}

pub(super) fn module(global_input_address: bool) -> Module {
    module_with_input_arguments(global_input_address, [1, 2])
}

pub(super) fn module_with_input_arguments(
    global_input_address: bool,
    input_arguments: [u32; 2],
) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = |access| Type::pointer(scalar.clone(), AddressSpace::Global, access);
    let slice = |access| Type::slice(scalar.clone(), AddressSpace::Global, access);
    let mut entry = BasicBlock::new(BlockId(17));
    entry.operations = vec![
        op(
            10,
            Type::INDEX,
            Kind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        op(11, Type::INDEX, Kind::SliceLength { slice: ValueId(0) }),
        op(
            12,
            Type::BOOL,
            Kind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(10),
                rhs: ValueId(11),
            },
        ),
        op(13, scalar.clone(), Kind::Constant(Constant::U32(0))),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(12),
        then_target: BlockId(31),
        then_arguments: vec![],
        else_target: BlockId(59),
        else_arguments: vec![],
    });
    let mut yes = BasicBlock::new(BlockId(31));
    yes.operations = vec![
        op(
            20,
            pointer(AccessMode::ReadWrite),
            Kind::SliceData { slice: ValueId(0) },
        ),
        op(
            21,
            pointer(AccessMode::ReadWrite),
            Kind::GetElementPointer {
                base: ValueId(20),
                offset: ValueId(10),
            },
        ),
    ];
    for (argument, base, at, value) in [
        (input_arguments[0], 30, 31, 32),
        (input_arguments[1], 40, 41, 42),
    ] {
        let addresses = [
            op(
                base,
                pointer(AccessMode::ReadOnly),
                Kind::SliceData {
                    slice: ValueId(argument),
                },
            ),
            op(
                at,
                pointer(AccessMode::ReadOnly),
                Kind::GetElementPointer {
                    base: ValueId(base),
                    offset: ValueId(10),
                },
            ),
        ];
        if global_input_address {
            entry.operations.extend(addresses);
        } else {
            yes.operations.extend(addresses);
        }
        yes.operations.push(op(
            value,
            scalar.clone(),
            Kind::Load {
                pointer: ValueId(at),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
    }
    yes.operations.extend([
        op(
            50,
            scalar.clone(),
            Kind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(32),
                rhs: ValueId(42),
            },
        ),
        op(
            51,
            scalar.clone(),
            Kind::Binary {
                op: BinaryOp::BitOr,
                lhs: ValueId(50),
                rhs: ValueId(13),
            },
        ),
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(21),
                value: ValueId(51),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]);
    yes.terminator = Some(Terminator::Return { values: vec![] });
    let mut no = BasicBlock::new(BlockId(59));
    no.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("conditional-prefix-components");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                slice(AccessMode::ReadWrite),
                slice(AccessMode::ReadOnly),
                slice(AccessMode::ReadOnly),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry, yes, no],
    ));
    let mut kernel = Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    module.kernels.push(kernel);
    module
}

pub(super) fn graph(module: &Module, budget: &mut Budget<'_>) -> Graph {
    let (graph, storage) =
        Graph::from_module_ref_with_verification_budget_v12(module, budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    graph
}

pub(super) fn prefix(bound: &Graph, budget: &mut Budget<'_>) -> Prefix {
    let p5 =
        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy5_v1(bound, budget).unwrap();
    budget.reserve_storage(p5.retained_storage()).unwrap();
    let p6 = fe2o3_kernel_opt::continue_checked_canonical_kernel_ir_policy6_v1(bound, p5, budget)
        .unwrap();
    budget.reserve_storage(p6.retained_storage()).unwrap();
    p6
}

pub(super) fn with_prefix(
    profile: Profile,
    run: impl FnOnce(&Graph, &Graph, &Prefix, &mut Budget<'_>),
) {
    with_module_prefix(profile, &module(false), run)
}

pub(super) fn with_module_prefix(
    profile: Profile,
    module: &Module,
    run: impl FnOnce(&Graph, &Graph, &Prefix, &mut Budget<'_>),
) {
    let mut work = Work::new(WORK);
    let mut target = Budget::new(&mut work, STORAGE);
    target.reserve_storage(FLOOR).unwrap();
    let n = graph(module, &mut target);
    let binding = dialect_amdgcn::bind_production_target_v1(n.module(), profile).unwrap();
    let b = graph(binding.module(), &mut target);
    drop(binding);
    let p6 = prefix(&b, &mut target);
    let floor = target.storage();
    let account = target.work_ledger_identity_v1();
    run(&n, &b, &p6, &mut target);
    assert_eq!(target.storage(), floor);
    assert!(target.work_ledger_identity_v1() == account);
}
