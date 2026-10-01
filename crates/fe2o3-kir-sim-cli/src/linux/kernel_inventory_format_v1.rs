//! Borrowed inventory projection; no executable support or source ABI inference.
use super::*;
use fe2o3_kernel_ir::{
    AddressSpace, ExecutionRoleV15, LaunchDomain, LaunchExtent, SynchronizationScope,
    TargetCapability, VectorLayoutV12,
};
use serde::ser::SerializeSeq;
use std::collections::BTreeSet;

#[derive(Serialize)]
pub(super) struct Report<'a> {
    schema: &'static str,
    status: &'static str,
    authority: &'static str,
    simulated: bool,
    simulator_admission: &'static str,
    source_authentication: bool,
    proof_authority: bool,
    compiler_execution_authority: bool,
    native_abi: &'static str,
    launch_authority: bool,
    hardware_observed: bool,
    performance_prediction: bool,
    target_profile: &'static str,
    additional_launch_requirements: &'static str,
    module: &'a str,
    kir: Identity,
    storage_layouts: usize,
    declared_capabilities: Capabilities<'a>,
    kernels: Kernels<'a>,
}
impl<'a> Report<'a> {
    pub(super) fn new(module: &'a Module, identity: [u8; 32], length: u64, raw: [u8; 32]) -> Self {
        Self {
            schema: "fe2o3-kernel-inventory-v1",
            status: "ok",
            authority: "observation_only",
            simulated: false,
            simulator_admission: "not_checked",
            source_authentication: false,
            proof_authority: false,
            compiler_execution_authority: false,
            native_abi: "unavailable",
            launch_authority: false,
            hardware_observed: false,
            performance_prediction: false,
            target_profile: "not_encoded",
            additional_launch_requirements: "unavailable_from_kernel_metadata",
            module: module.id.as_str(),
            kir: Identity {
                wire_version: 18,
                identity_sha256: Hex(identity),
                raw_sha256: Hex(raw),
                canonical_bytes: length,
            },
            storage_layouts: module.storage_layouts.len(),
            declared_capabilities: Capabilities(&module.required_capabilities),
            kernels: Kernels(module),
        }
    }
}
#[derive(Serialize)]
struct Identity {
    wire_version: u16,
    identity_sha256: Hex,
    raw_sha256: Hex,
    canonical_bytes: u64,
}
struct Hex([u8; 32]);
impl Serialize for Hex {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut bytes = [0; 64];
        for (index, value) in self.0.iter().copied().enumerate() {
            bytes[index * 2] = DIGITS[usize::from(value >> 4)];
            bytes[index * 2 + 1] = DIGITS[usize::from(value & 15)];
        }
        serializer.serialize_str(std::str::from_utf8(&bytes).expect("hex is ASCII"))
    }
}

struct Kernels<'a>(&'a Module);
impl Serialize for Kernels<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.kernels.len()))?;
        for kernel in &self.0.kernels {
            let entry = self
                .0
                .function(&kernel.entry)
                .ok_or_else(|| serde::ser::Error::custom("missing verified entry"))?;
            sequence.serialize_element(&Kernel {
                id: kernel.id.as_str(),
                entry: entry.id.as_str(),
                request_abi: "entry_parameter_order",
                source_parameter_names: "unavailable",
                domain: Domain(&kernel.domain),
                workgroup_size: kernel.workgroup_size.map(|size| [size.x, size.y, size.z]),
                declared_capabilities: Capabilities(&kernel.required_capabilities),
                entry_declared_capabilities: Capabilities(&entry.required_capabilities),
                parameters: Parameters(&entry.signature.parameters),
                results: Types(&entry.signature.results),
            })?;
        }
        sequence.end()
    }
}
#[derive(Serialize)]
struct Kernel<'a> {
    id: &'a str,
    entry: &'a str,
    request_abi: &'static str,
    source_parameter_names: &'static str,
    domain: Domain<'a>,
    workgroup_size: Option<[u32; 3]>,
    declared_capabilities: Capabilities<'a>,
    entry_declared_capabilities: Capabilities<'a>,
    parameters: Parameters<'a>,
    results: Types<'a>,
}
struct Domain<'a>(&'a LaunchDomain);
impl Serialize for Domain<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(usize::from(self.0.rank())))?;
        for extent in self.0.extents() {
            sequence.serialize_element(&match extent {
                LaunchExtent::Dynamic => None,
                LaunchExtent::Static(value) => Some(value),
            })?;
        }
        sequence.end()
    }
}
struct Parameters<'a>(&'a [Type]);
impl Serialize for Parameters<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Parameter<'a> {
            index: usize,
            r#type: TypeView<'a>,
            request_encoding: Encoding,
        }
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for (index, ty) in self.0.iter().enumerate() {
            sequence.serialize_element(&Parameter {
                index,
                r#type: TypeView(ty),
                request_encoding: encoding(ty),
            })?;
        }
        sequence.end()
    }
}
struct Types<'a>(&'a [Type]);
impl Serialize for Types<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for ty in self.0 {
            sequence.serialize_element(&TypeView(ty))?;
        }
        sequence.end()
    }
}
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Encoding {
    Scalar {
        r#type: &'static str,
    },
    BufferOrBufferView {
        element: &'static str,
        required_access: &'static str,
        alignment: &'static str,
    },
    Unavailable {
        reason: &'static str,
    },
}
fn encoding(ty: &Type) -> Encoding {
    let memory = match ty {
        Type::Scalar(scalar) => {
            return Encoding::Scalar {
                r#type: scalar_type_name(*scalar),
            };
        }
        Type::Slice(slice) => Some((&*slice.element, slice.address_space, slice.access)),
        Type::Pointer(pointer) => Some((&*pointer.pointee, pointer.address_space, pointer.access)),
        _ => None,
    };
    if let Some((Type::Scalar(element), AddressSpace::Global, access)) = memory {
        return Encoding::BufferOrBufferView {
            element: scalar_type_name(*element),
            required_access: access_name(access),
            alignment: "caller_selected_and_checked_at_preflight",
        };
    }
    Encoding::Unavailable {
        reason: "not_representable_by_current_request_arguments",
    }
}

pub(super) struct TypeView<'a>(pub(super) &'a Type);
impl Serialize for TypeView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum View<'a> {
            Unit,
            Scalar {
                r#type: &'static str,
                bits: Option<u16>,
            },
            StorageObject {
                layout: u32,
            },
            Execution {
                role: Role,
            },
            Vector {
                element: &'static str,
                lanes: u16,
                layout: Layout,
            },
            Pointer {
                address_space: &'static str,
                access: &'static str,
                pointee: TypeView<'a>,
            },
            Slice {
                address_space: &'static str,
                access: &'static str,
                element: TypeView<'a>,
            },
        }
        let value = match self.0 {
            Type::Unit => View::Unit,
            Type::Scalar(value) => View::Scalar {
                r#type: scalar_type_name(*value),
                bits: value.bit_width(),
            },
            Type::StorageObject(id) => View::StorageObject { layout: id.0 },
            Type::Execution(role) => View::Execution {
                role: match role {
                    ExecutionRoleV15::Context => Role::Context,
                    ExecutionRoleV15::Workgroup => Role::Workgroup,
                    ExecutionRoleV15::MaskedTileU32 { lanes, elements } => Role::MaskedTileU32 {
                        lanes: *lanes,
                        elements: *elements,
                    },
                    ExecutionRoleV15::LaneFragmentU32 { lanes, elements } => {
                        Role::LaneFragmentU32 {
                            lanes: *lanes,
                            elements: *elements,
                        }
                    }
                },
            },
            Type::Vector(vector) => View::Vector {
                element: scalar_type_name(vector.element),
                lanes: vector.lanes,
                layout: match vector.layout {
                    VectorLayoutV12::Contiguous => Layout::Contiguous,
                    VectorLayoutV12::Interleaved { factor } => Layout::Interleaved { factor },
                },
            },
            Type::Pointer(pointer) => View::Pointer {
                address_space: address(pointer.address_space),
                access: access_name(pointer.access),
                pointee: TypeView(&pointer.pointee),
            },
            Type::Slice(slice) => View::Slice {
                address_space: address(slice.address_space),
                access: access_name(slice.access),
                element: TypeView(&slice.element),
            },
        };
        value.serialize(serializer)
    }
}
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Role {
    Context,
    Workgroup,
    MaskedTileU32 { lanes: u16, elements: u16 },
    LaneFragmentU32 { lanes: u16, elements: u16 },
}
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Layout {
    Contiguous,
    Interleaved { factor: u16 },
}
fn address(space: AddressSpace) -> &'static str {
    match space {
        AddressSpace::Private => "private",
        AddressSpace::Workgroup => "workgroup",
        AddressSpace::Global => "global",
        AddressSpace::Constant => "constant",
        AddressSpace::Generic => "generic",
    }
}
struct Capabilities<'a>(&'a BTreeSet<TargetCapability>);
impl Serialize for Capabilities<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum Capability<'a> {
            Float16,
            BFloat16,
            Float64,
            Int64,
            Subgroups,
            SubgroupSize {
                lanes: u32,
            },
            WorkgroupMemory,
            WorkgroupBarrier,
            DynamicWorkgroupMemory,
            WaveWidth {
                lanes: u32,
            },
            Atomic {
                width_bits: u16,
                address_space: &'static str,
                max_scope: &'static str,
            },
            Extension {
                namespace: &'a str,
                name: &'a str,
            },
        }
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for capability in self.0 {
            let view = match capability {
                TargetCapability::Float16 => Capability::Float16,
                TargetCapability::BFloat16 => Capability::BFloat16,
                TargetCapability::Float64 => Capability::Float64,
                TargetCapability::Int64 => Capability::Int64,
                TargetCapability::Subgroups => Capability::Subgroups,
                TargetCapability::SubgroupSize(lanes) => Capability::SubgroupSize { lanes: *lanes },
                TargetCapability::WorkgroupMemory => Capability::WorkgroupMemory,
                TargetCapability::WorkgroupBarrier => Capability::WorkgroupBarrier,
                TargetCapability::DynamicWorkgroupMemory => Capability::DynamicWorkgroupMemory,
                TargetCapability::WaveWidth(width) => Capability::WaveWidth {
                    lanes: width.lanes(),
                },
                TargetCapability::Extension { namespace, name } => {
                    Capability::Extension { namespace, name }
                }
                TargetCapability::Atomic {
                    width_bits,
                    address_space,
                    max_scope,
                } => Capability::Atomic {
                    width_bits: *width_bits,
                    address_space: address(*address_space),
                    max_scope: match max_scope {
                        SynchronizationScope::Invocation => "invocation",
                        SynchronizationScope::Subgroup => "subgroup",
                        SynchronizationScope::Workgroup => "workgroup",
                        SynchronizationScope::Device => "device",
                        SynchronizationScope::System => "system",
                    },
                },
            };
            sequence.serialize_element(&view)?;
        }
        sequence.end()
    }
}
