// Shared inert test inputs. Nothing here is compiler-origin or native authority.
use fe2o3_kernel_descriptor::*;
use fe2o3_kernel_ir::{
    AccessMode as KirAccess, AddressSpace, AmdGpuDiagnosticOperation, BasicBlock, BlockId,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, F32MathFunction,
    F32MathImplementation, FloatOperation, Function, Kernel, LaunchDomain, LaunchExtent, Module,
    ScalarType as KirScalar, Signature, Terminator, Type, ValueId, WorkgroupSize,
};
use std::fmt::Write;

pub const GRAPH_ORDER: [&str; 3] = ["zebra", "amber", "maple"];
pub const DESCRIPTOR_ORDER: [&str; 3] = ["maple", "zebra", "amber"];
pub const LEXICAL_PAIRS: [(&str, usize, usize); 3] =
    [("amber", 1, 2), ("maple", 2, 0), ("zebra", 0, 1)];

#[derive(Clone, Copy)]
pub enum Shape {
    U8,
    Usize,
    I16,
    Isize,
    SharedU16,
    U32,
    DisjointU64,
    U64,
    I64,
}
impl Shape {
    fn source(self) -> SourceTypeDescriptorV3 {
        use ScalarTypeV1 as S;
        use SourceTypeDescriptorV3 as T;
        match self {
            Self::U8 => T::Scalar(S::U8),
            Self::Usize => T::Usize,
            Self::I16 => T::Scalar(S::I16),
            Self::Isize => T::Isize,
            Self::SharedU16 => T::SharedSlice(S::U16),
            Self::U32 => T::Scalar(S::U32),
            Self::DisjointU64 => T::DisjointSlice(S::U64),
            Self::U64 => T::Scalar(S::U64),
            Self::I64 => T::Scalar(S::I64),
        }
    }
    fn layout(self) -> DeviceLayoutDescriptorV1 {
        use ScalarTypeV1 as S;
        match self {
            Self::U8 => DeviceLayoutDescriptorV1::scalar(S::U8),
            Self::Usize | Self::U64 => DeviceLayoutDescriptorV1::scalar(S::U64),
            Self::I16 => DeviceLayoutDescriptorV1::scalar(S::I16),
            Self::Isize | Self::I64 => DeviceLayoutDescriptorV1::scalar(S::I64),
            Self::SharedU16 => DeviceLayoutDescriptorV1::shared_slice(S::U16),
            Self::U32 => DeviceLayoutDescriptorV1::scalar(S::U32),
            Self::DisjointU64 => DeviceLayoutDescriptorV1::disjoint_slice(S::U64),
        }
    }
    fn kir(self) -> Type {
        match self {
            Self::U8 => Type::Scalar(KirScalar::U8),
            Self::Usize | Self::U64 => Type::Scalar(KirScalar::U64),
            Self::I16 => Type::Scalar(KirScalar::I16),
            Self::Isize | Self::I64 => Type::Scalar(KirScalar::I64),
            Self::U32 => Type::Scalar(KirScalar::U32),
            Self::SharedU16 => Type::slice(
                Type::Scalar(KirScalar::U16),
                AddressSpace::Global,
                KirAccess::ReadOnly,
            ),
            Self::DisjointU64 => Type::slice(
                Type::Scalar(KirScalar::U64),
                AddressSpace::Global,
                KirAccess::ReadWrite,
            ),
        }
    }
    fn llvm(self) -> &'static [&'static str] {
        match self {
            Self::U8 => &["i8"],
            Self::I16 => &["i16"],
            Self::U32 => &["i32"],
            Self::Usize | Self::Isize | Self::U64 | Self::I64 => &["i64"],
            Self::SharedU16 | Self::DisjointU64 => &["ptr addrspace(1)", "i64"],
        }
    }
    fn ownership(self) -> OwnershipSemantics {
        match self {
            Self::SharedU16 => OwnershipSemantics::SharedBorrow,
            Self::DisjointU64 => OwnershipSemantics::UniqueBorrow,
            _ => OwnershipSemantics::ByValue,
        }
    }
}

#[derive(Clone)]
pub struct Argument {
    pub shape: Shape,
    pub offset: u32,
}
impl Argument {
    // Literal ABI facts, independent of the production physical-layout checker.
    pub fn components(&self) -> Vec<PhysicalComponentV3> {
        use PhysicalAbiComponentKind as K;
        use ScalarTypeV1 as S;
        let (kind, size, alignment, access, alias) = match self.shape {
            Shape::U8 => (
                K::ScalarByValue(S::U8),
                1,
                1,
                AccessMode::ByValue,
                AliasSemantics::Value,
            ),
            Shape::I16 => (
                K::ScalarByValue(S::I16),
                2,
                2,
                AccessMode::ByValue,
                AliasSemantics::Value,
            ),
            Shape::U32 => (
                K::ScalarByValue(S::U32),
                4,
                4,
                AccessMode::ByValue,
                AliasSemantics::Value,
            ),
            Shape::Usize | Shape::U64 => (
                K::ScalarByValue(S::U64),
                8,
                8,
                AccessMode::ByValue,
                AliasSemantics::Value,
            ),
            Shape::Isize | Shape::I64 => (
                K::ScalarByValue(S::I64),
                8,
                8,
                AccessMode::ByValue,
                AliasSemantics::Value,
            ),
            Shape::SharedU16 => (
                K::GlobalPointer,
                8,
                8,
                AccessMode::ReadOnly,
                AliasSemantics::SharedReadOnly,
            ),
            Shape::DisjointU64 => (
                K::GlobalPointer,
                8,
                8,
                AccessMode::ReadWrite,
                AliasSemantics::Exclusive,
            ),
        };
        let mut result = vec![PhysicalComponentV3 {
            kind,
            offset: self.offset,
            size,
            alignment,
            access,
            alias,
        }];
        if matches!(self.shape, Shape::SharedU16 | Shape::DisjointU64) {
            result.push(PhysicalComponentV3 {
                kind: K::SliceLengthU64,
                offset: self.offset + 8,
                size: 8,
                alignment: 8,
                access: AccessMode::ByValue,
                alias: AliasSemantics::Value,
            });
        }
        result
    }
}

#[derive(Clone)]
pub struct Root {
    pub entry: &'static str,
    pub implementation: &'static str,
    pub arguments: Vec<Argument>,
    pub explicit: u32,
    pub alignment: u32,
    pub components: usize,
}
#[derive(Clone)]
pub struct Contract {
    pub roots: Vec<Root>,
}
impl Contract {
    pub fn standard() -> Self {
        assert_eq!(
            MAX_ARGUMENTS_PER_KERNEL, 64,
            "review the actual public boundary if changed"
        );
        let pattern = [
            Shape::U8,
            Shape::Usize,
            Shape::I16,
            Shape::Isize,
            Shape::SharedU16,
            Shape::U32,
            Shape::DisjointU64,
            Shape::U64,
        ];
        let offsets = [0, 8, 16, 24, 32, 48, 56, 72];
        let maple = (0..64)
            .map(|i| Argument {
                shape: pattern[i % 8],
                offset: 80 * (i / 8) as u32 + offsets[i % 8],
            })
            .collect();
        let zebra = [
            Shape::Usize,
            Shape::Isize,
            Shape::U64,
            Shape::I64,
            Shape::DisjointU64,
        ]
        .into_iter()
        .enumerate()
        .map(|(i, shape)| Argument {
            shape,
            offset: 8 * i as u32,
        })
        .collect();
        Self {
            roots: vec![
                Root {
                    entry: "maple",
                    implementation: "maple_impl",
                    arguments: maple,
                    explicit: 640,
                    alignment: 8,
                    components: 80,
                },
                Root {
                    entry: "zebra",
                    implementation: "zebra_impl",
                    arguments: zebra,
                    explicit: 48,
                    alignment: 8,
                    components: 6,
                },
                Root {
                    entry: "amber",
                    implementation: "amber_impl",
                    arguments: vec![],
                    explicit: 0,
                    alignment: 1,
                    components: 0,
                },
            ],
        }
    }
    pub fn module(&self, extra_roles: bool) -> Module {
        let entry = |index: usize| {
            let root = &self.roots[index];
            Function::kernel_entry(
                root.implementation,
                Signature::new(
                    root.arguments.iter().map(|a| a.shape.kir()).collect(),
                    vec![],
                ),
                (0..root.arguments.len() as u32).map(ValueId).collect(),
                vec![block()],
            )
        };
        let mut module = Module::new("independent-nonlexical-nominal-v3-scale");
        module.functions = if extra_roles {
            vec![
                entry(0),
                Function::external_import("zz_import", Signature::new(vec![], vec![])),
                Function::internal_helper(
                    "aa_helper",
                    Signature::new(vec![], vec![]),
                    vec![],
                    vec![block()],
                ),
                Function::internal_helper(
                    "zz_aux",
                    Signature::new(vec![], vec![]),
                    vec![],
                    vec![block()],
                ),
                entry(2),
                AmdGpuDiagnosticOperation::Trap.declaration(),
                Function::internal_helper(
                    "zz_helper",
                    Signature::new(vec![], vec![]),
                    vec![],
                    vec![block()],
                ),
                Function::external_import("aa_import", Signature::new(vec![], vec![])),
                Function::internal_helper(
                    "aa_aux",
                    Signature::new(vec![], vec![]),
                    vec![],
                    vec![block()],
                ),
                entry(1),
                FloatOperation::F32Math {
                    function: F32MathFunction::Sin,
                    implementation: F32MathImplementation::OcmlAbiV1,
                    arguments: vec![ValueId(0)],
                }
                .declaration(),
            ]
        } else {
            vec![entry(0), entry(2), entry(1)]
        };
        for index in [1, 2, 0] {
            let root = &self.roots[index];
            let mut kernel = Kernel::new(
                root.entry,
                root.implementation,
                LaunchDomain::D1 {
                    x: LaunchExtent::Dynamic,
                },
            );
            kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
            module.kernels.push(kernel);
        }
        assert_eq!(
            module
                .kernels
                .iter()
                .map(|k| k.id.as_str())
                .collect::<Vec<_>>(),
            GRAPH_ORDER
        );
        module
    }
    pub fn wire(&self, target: &str) -> Result<Vec<u8>, DescriptorWireErrorV3<Resource>> {
        let sources: Vec<Vec<_>> = self
            .roots
            .iter()
            .map(|root| {
                root.arguments
                    .iter()
                    .map(|arg| SourceTypeRecordV3::new(arg.shape.source(), &mut free).unwrap())
                    .collect()
            })
            .collect();
        let layouts: Vec<Vec<_>> = self
            .roots
            .iter()
            .map(|root| {
                root.arguments
                    .iter()
                    .map(|arg| device_layout_record_v3(arg.shape.layout(), &mut free).unwrap())
                    .collect()
            })
            .collect();
        let parts: Vec<Vec<_>> = self
            .roots
            .iter()
            .map(|root| root.arguments.iter().map(Argument::components).collect())
            .collect();
        let names: Vec<Vec<_>> = self
            .roots
            .iter()
            .map(|root| {
                (0..root.arguments.len())
                    .map(|i| format!("argument_{i:02}"))
                    .collect()
            })
            .collect();
        let mut type_records: Vec<_> = sources.iter().flatten().copied().collect();
        type_records.sort_by_key(|r| r.identity());
        type_records.dedup();
        let mut layout_records: Vec<_> = layouts.iter().flatten().cloned().collect();
        layout_records.sort_by_key(|r| r.identity());
        layout_records.dedup();
        let args: Vec<Vec<_>> = self
            .roots
            .iter()
            .enumerate()
            .map(|(r, root)| {
                root.arguments
                    .iter()
                    .enumerate()
                    .map(|(a, arg)| LogicalArgumentInputV3 {
                        source_index: a as u16,
                        name: &names[r][a],
                        source_type: sources[r][a].identity(),
                        device_layout: layouts[r][a].identity(),
                        ownership: arg.shape.ownership(),
                        access: parts[r][a][0].access,
                        alias: parts[r][a][0].alias,
                        components: &parts[r][a],
                    })
                    .collect()
            })
            .collect();
        let symbols: Vec<_> = self
            .roots
            .iter()
            .map(|r| format!("{}.kd", r.entry))
            .collect();
        let launch = LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
            DimensionsV1::new(1024, 1, 1).unwrap(),
            64,
            0,
            0,
        )
        .unwrap();
        let evidence = BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes([11; 32]),
            EvidenceDigest::from_sha256_bytes([12; 32]),
        );
        let kernels: Vec<_> = self
            .roots
            .iter()
            .enumerate()
            .map(|(r, root)| KernelDescriptorInputV3 {
                kernel_id: KernelId::from_bytes([r as u8 + 1; 32]),
                logical_name: root.entry,
                entry_name: root.entry,
                descriptor_symbol: &symbols[r],
                source_evidence: evidence,
                executable_ir_evidence: evidence,
                capabilities: &[CapabilityV1::AmdWave],
                abi_layout: KernelAbiLayoutV1::new(
                    root.explicit,
                    root.explicit + 256,
                    root.alignment,
                )
                .unwrap(),
                launch: &launch,
                arguments: &args[r],
            })
            .collect();
        let requirements: Vec<_> = (0..self.roots.len())
            .map(|r| {
                KernelTargetRequirementsV2::new(
                    KernelId::from_bytes([r as u8 + 1; 32]),
                    LdsRequirementsV2::new(0, 0).unwrap(),
                    RequiredWavefrontWidthV2::Wave64,
                    false,
                    SynchronizationRequirementsV2::empty(),
                    AtomicRequirementsV2::empty(),
                )
            })
            .collect();
        let compiler = CompilerIdentityV1::new(
            Text::new("rustc").unwrap(),
            Text::new("inert-scale-test").unwrap(),
            [7; 20],
        );
        let producer = ProducerIdentityV1::new(
            Text::new("fe2o3").unwrap(),
            Text::new("inert-scale-test").unwrap(),
        );
        let input = DeviceDescriptorTableInputV3 {
            canonical_code_object_digest: CanonicalCodeObjectDigest::from_bytes([0; 32]),
            code_object_version: CodeObjectVersion::V6,
            compiler: &compiler,
            producer: &producer,
            device_target: DeviceTargetV1::parse(target).unwrap(),
            type_records: &type_records,
            layout_records: &layout_records,
            kernels: &kernels,
            requirements: &requirements,
        };
        let mut bytes = vec![0; encoded_device_descriptor_table_v3_len(&input, &mut free)?];
        encode_device_descriptor_table_v3(&input, &mut bytes, &mut free)?;
        Ok(bytes)
    }
    pub fn check_table(&self, table: &DeviceDescriptorTableV3<'_>, b: &mut Budget<'_>) {
        assert_eq!(table.kernel_count(), 3);
        b.reserve_storage(4 * DESCRIPTOR_QUERY_STORAGE_V3).unwrap();
        for (r, expected) in self.roots.iter().enumerate() {
            let row = table.kernel(r, &mut |w| b.charge_work(w)).unwrap();
            assert_eq!(row.entry_name(), DESCRIPTOR_ORDER[r]);
            assert_eq!(row.argument_count(), expected.arguments.len());
            assert_eq!(row.component_count(), expected.components);
            assert_eq!(
                (
                    row.abi_layout().explicit_argument_size(),
                    row.abi_layout().kernarg_segment_size(),
                    row.abi_layout().kernarg_segment_alignment()
                ),
                (
                    expected.explicit,
                    expected.explicit + 256,
                    expected.alignment
                )
            );
            let mut cursor = row.arguments();
            for (i, arg) in expected.arguments.iter().enumerate() {
                let actual = cursor.next(&mut |w| b.charge_work(w)).unwrap().unwrap();
                assert_eq!(actual.source_index(), i as u16);
                assert_eq!(actual.name(), format!("argument_{i:02}"));
                assert_eq!(
                    table
                        .source_type(actual.source_type(), &mut |w| b.charge_work(w))
                        .unwrap()
                        .descriptor(),
                    arg.shape.source()
                );
                assert_eq!(
                    table
                        .device_layout(actual.device_layout(), &mut |w| b.charge_work(w))
                        .unwrap()
                        .descriptor(),
                    &arg.shape.layout()
                );
                let components = arg.components();
                assert_eq!(actual.ownership(), arg.shape.ownership());
                assert_eq!(
                    (actual.access(), actual.alias()),
                    (components[0].access, components[0].alias)
                );
                assert_eq!(actual.component_count(), components.len());
                for (part, expected) in components.iter().enumerate() {
                    assert_eq!(
                        &actual.component(part, &mut |w| b.charge_work(w)).unwrap(),
                        expected
                    );
                }
            }
            assert!(cursor.next(&mut |w| b.charge_work(w)).unwrap().is_none());
        }
        b.release_storage(4 * DESCRIPTOR_QUERY_STORAGE_V3).unwrap();
    }
    pub fn check_native_signatures(&self, prefix: &str) {
        let definitions: Vec<_> = prefix
            .lines()
            .filter_map(|line| line.strip_prefix("define amdgpu_kernel void @"))
            .collect();
        assert_eq!(definitions.len(), 3);
        for root in &self.roots {
            let marker = format!("{}(", root.entry);
            let matched: Vec<_> = definitions
                .iter()
                .filter_map(|line| line.strip_prefix(&marker))
                .collect();
            assert_eq!(matched.len(), 1, "exactly one native entry {}", root.entry);
            let parameters = matched[0].split_once(") #").unwrap().0;
            let actual: Vec<_> = if parameters.is_empty() {
                vec![]
            } else {
                parameters.split(", ").collect()
            };
            let expected: Vec<_> = root
                .arguments
                .iter()
                .enumerate()
                .flat_map(|(i, arg)| {
                    let types = arg.shape.llvm();
                    types.iter().enumerate().map(move |(part, ty)| {
                        if types.len() == 1 {
                            format!("{ty} %arg{i}")
                        } else {
                            format!("{ty} %arg{i}.{}", if part == 0 { "data" } else { "len" })
                        }
                    })
                })
                .collect();
            assert_eq!(actual, expected);
        }
    }
}
fn block() -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    block
}
pub fn free(_: usize) -> Result<(), Resource> {
    Ok(())
}

pub fn suffix(wire: &[u8]) -> String {
    let mut output = String::from(
        "\nmodule asm \".section .fe2o3.kd.v3,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n",
    );
    for chunk in wire.chunks(16) {
        let mut bytes = chunk.iter();
        write!(output, "module asm \".byte 0x{:02x}", bytes.next().unwrap()).unwrap();
        for byte in bytes {
            write!(output, ", 0x{byte:02x}").unwrap();
        }
        output.push_str("\"\n");
    }
    output
}
pub fn check_suffix(prefix: &str, wire: &[u8], text: &str) {
    assert_eq!(text, format!("{prefix}{}", suffix(wire)));
    assert_eq!(text.matches(".section .fe2o3.kd.v3,").count(), 1);
    assert!(!text.contains(".fe2o3.kd.v1"));
    let section = text.strip_prefix(prefix).unwrap();
    let mut lines = section.lines();
    assert_eq!(lines.next(), Some(""));
    assert_eq!(
        lines.next(),
        Some("module asm \".section .fe2o3.kd.v3,\\22\\22,@progbits\"")
    );
    assert_eq!(lines.next(), Some("module asm \".balign 8\""));
    let recovered: Vec<_> = lines
        .flat_map(|line| {
            line.strip_prefix("module asm \".byte ")
                .unwrap()
                .strip_suffix('"')
                .unwrap()
                .split(", ")
                .map(|byte| u8::from_str_radix(byte.strip_prefix("0x").unwrap(), 16).unwrap())
        })
        .collect();
    assert_eq!(recovered, wire);
}
