//! Synthetic V18 fixture: inert layouts do not establish source or hardware custody.
#![allow(dead_code)]
use fe2o3_kernel_ir::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub const BOUND: usize = 64 * 1024 * 1024;
pub const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 4096,
    edges: 32768,
    containment_depth: 64,
    object_bytes: 256 * 1024 * 1024,
};

pub fn module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), pointer.clone()),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(2),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), scalar.clone()),
            OperationKind::Constant(Constant::U32(11)),
        ),
        Operation::new(
            vec![
                ValueDef::new(ValueId(5), scalar.clone()),
                ValueDef::new(ValueId(6), Type::BOOL),
            ],
            OperationKind::Binary {
                op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                lhs: ValueId(1),
                rhs: ValueId(4),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(3),
                value: ValueId(5),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![pointer, scalar], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    let mut module = Module::new("synthetic-v18-diagnostic");
    module.storage_layouts = vec![
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
        },
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Record(
                vec![StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(0),
                }]
                .into_boxed_slice(),
            ),
        },
    ];
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "scalar",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    refresh_capabilities(&mut module);
    module
}

fn refresh_capabilities(module: &mut Module) {
    let capabilities = module.functions[0].derived_capabilities();
    module.functions[0].required_capabilities = capabilities.clone();
    module.kernels[0].required_capabilities = capabilities.clone();
    module.required_capabilities = capabilities;
}

pub fn scalar_storage_module() -> Module {
    let mut raw = module();
    let block = &mut raw.functions[0].body.as_mut().unwrap().blocks[0];
    let mut store = block.operations.pop().unwrap();
    let OperationKind::Store { value, .. } = &mut store.kind else {
        unreachable!()
    };
    *value = ValueId(8);
    block.operations.extend([
        Operation::effect_free(
            ValueDef::new(
                ValueId(7),
                Type::pointer(
                    Type::StorageObject(StorageLayoutIdV1(0)),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::Alloca {
                element: Type::StorageObject(StorageLayoutIdV1(0)),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Storage(StorageOperationV1::WriteValue {
                address: ValueId(7),
                value: ValueId(5),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(8), Type::Scalar(ScalarType::U32)),
            OperationKind::Storage(StorageOperationV1::ReadValue {
                address: ValueId(7),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
        store,
    ]);
    refresh_capabilities(&mut raw);
    raw
}

pub fn storage_module(with_operation: bool) -> Module {
    let mut raw = module();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(
            ValueId(0),
            Type::pointer(
                Type::StorageObject(StorageLayoutIdV1(1)),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        ),
        OperationKind::Alloca {
            element: Type::StorageObject(StorageLayoutIdV1(1)),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 4,
        },
    ));
    if with_operation {
        block.operations.extend([
            Operation::effect_free(
                ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(Constant::U32(37)),
            ),
            Operation::effect_free(
                ValueDef::new(
                    ValueId(2),
                    Type::pointer(
                        Type::StorageObject(StorageLayoutIdV1(0)),
                        AddressSpace::Private,
                        AccessMode::ReadWrite,
                    ),
                ),
                OperationKind::Storage(StorageOperationV1::Project {
                    base: ValueId(0),
                    step: StorageProjectionV1::Field(0),
                }),
            ),
        ]);
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    raw.functions[0] =
        Function::kernel_entry("entry", Signature::new(vec![], vec![]), vec![], vec![block]);
    refresh_capabilities(&mut raw);
    raw
}

pub fn bytes(module: &Module) -> Vec<u8> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(BOUND);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, BOUND);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            LAYOUTS,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let bytes = owner.canonical_bytes().to_vec();
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
    bytes
}

pub fn request(value: u32) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "schema": "fe2o3-simulation-request-v1", "kernel": "scalar",
        "grid": [64, 1, 1], "workgroup": [64, 1, 1],
        "arguments": [
            { "kind": "buffer", "element": "u32", "access": "read_write", "alignment": 4,
              "bytes": format!("0x{}", "5a".repeat(264)),
              "initialized": format!("0x{}", "00".repeat(33)) },
            { "kind": "scalar", "type": "u32", "bits": format!("0x{value:08x}") },
        ]
    }))
    .unwrap()
}
pub fn storage_request() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "schema": "fe2o3-simulation-request-v1", "kernel": "scalar",
        "grid": [1, 1, 1], "workgroup": [1, 1, 1], "arguments": []
    }))
    .unwrap()
}
pub fn output(value: u32) -> Vec<u8> {
    let mut bytes = value.wrapping_add(11).to_le_bytes().repeat(64);
    bytes.extend_from_slice(&[0x5a; 8]);
    bytes
}

static NEXT: AtomicU64 = AtomicU64::new(0);
pub struct Files {
    root: PathBuf,
    pub kir: PathBuf,
    pub request: PathBuf,
}
impl Files {
    pub fn new(kir: &[u8], request: &[u8]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "fe2o3-v18-input-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let value = Self {
            kir: root.join("kernel.kir"),
            request: root.join("request.json"),
            root,
        };
        std::fs::write(&value.kir, kir).unwrap();
        std::fs::write(&value.request, request).unwrap();
        value
    }
    pub fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
