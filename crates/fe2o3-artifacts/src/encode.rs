use crate::{
    AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize, Capability,
    CodeObjectFormat, DigestBytes, Dimensions, Endianness, IdentityText, LaunchContract,
    ManifestV1, Mutability, Name, PointerWidth, ScalarType,
};

pub const MANIFEST_MAGIC: [u8; 8] = *b"FE2O3AM\0";
pub const MANIFEST_VERSION: u16 = 1;
pub const MAX_MANIFEST_BYTES: usize = 4 * 1024 * 1024;

impl ManifestV1 {
    /// Encodes the validated manifest using the canonical v1 binary format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.bytes(&MANIFEST_MAGIC);
        writer.u16(MANIFEST_VERSION);
        writer.u16(0);
        writer.identity_text(self.compiler().name());
        writer.identity_text(self.compiler().version());
        writer.identity_text(self.producer().name());
        writer.identity_text(self.producer().version());
        writer.identity_text(self.target().triple());
        writer.identity_text(self.target().architecture());
        writer.u8(pointer_width_tag(self.target().pointer_width()));
        writer.u8(endianness_tag(self.target().endianness()));
        writer.capabilities(self.target().capabilities());

        writer.u32(self.code_objects().len() as u32);
        for code_object in self.code_objects() {
            writer.digest(code_object.digest());
            writer.u8(code_object_format_tag(code_object.format()));
            writer.u64(code_object.byte_len());
        }

        writer.u32(self.kernels().len() as u32);
        for kernel in self.kernels() {
            writer.digest(kernel.kernel_id());
            writer.name(kernel.name());
            writer.name(kernel.symbol());
            writer.digest(kernel.source_digest());
            writer.digest(kernel.executable_digest());
            writer.digest(kernel.code_object_digest());
            writer.capabilities(kernel.required_capabilities());
            writer.launch(kernel.launch());
            writer.abi(kernel.abi());
        }
        debug_assert!(writer.bytes.len() <= MAX_MANIFEST_BYTES);
        writer.bytes
    }
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self {
            bytes: Vec::with_capacity(512),
        }
    }

    fn bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes(value.to_le_bytes().as_slice());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(value.to_le_bytes().as_slice());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(value.to_le_bytes().as_slice());
    }

    fn text(&mut self, value: &str) {
        self.u16(value.len() as u16);
        self.bytes(value.as_bytes());
    }

    fn name(&mut self, value: &Name) {
        self.text(value.as_str());
    }

    fn identity_text(&mut self, value: &IdentityText) {
        self.text(value.as_str());
    }

    fn digest(&mut self, value: DigestBytes) {
        self.bytes(value.as_bytes());
    }

    fn capabilities(&mut self, capabilities: &[Capability]) {
        self.u16(capabilities.len() as u16);
        for capability in capabilities {
            self.u16(capability_tag(*capability));
        }
    }

    fn dimensions(&mut self, dimensions: Dimensions) {
        self.u32(dimensions.x());
        self.u32(dimensions.y());
        self.u32(dimensions.z());
    }

    fn launch(&mut self, launch: &LaunchContract) {
        self.u8(launch.rank());
        match launch.block_size() {
            BlockSize::Any => self.u8(0),
            BlockSize::Exact(dimensions) => {
                self.u8(1);
                self.dimensions(dimensions);
            }
            BlockSize::AtMost(dimensions) => {
                self.u8(2);
                self.dimensions(dimensions);
            }
        }
        self.dimensions(launch.max_grid());
        self.u32(launch.static_shared_memory_bytes());
        self.u32(launch.max_dynamic_shared_memory_bytes());
    }

    fn abi(&mut self, abi: &AbiLayout) {
        self.u64(abi.size());
        self.u32(abi.alignment());
        self.u16(abi.fields().len() as u16);
        for field in abi.fields() {
            self.name(field.name());
            self.u64(field.offset());
            self.u64(field.size());
            self.u32(field.alignment());
            match field.kind() {
                AbiKind::Scalar(scalar) => {
                    self.u8(0);
                    self.u8(scalar_tag(scalar));
                }
                AbiKind::Pointer {
                    pointee_size,
                    pointee_alignment,
                } => {
                    self.u8(1);
                    self.u64(pointee_size);
                    self.u32(pointee_alignment);
                }
                AbiKind::Slice {
                    element_size,
                    element_alignment,
                } => {
                    self.u8(2);
                    self.u64(element_size);
                    self.u32(element_alignment);
                }
            }
            self.u8(mutability_tag(field.mutability()));
            self.u8(access_tag(field.access()));
            self.u8(address_space_tag(field.address_space()));
            self.digest(field.type_identity().rust_type());
            self.digest(field.type_identity().layout());
            self.u8(ownership_tag(field.ownership()));
            self.u8(alias_class_tag(field.alias_class()));
        }
    }
}

const fn pointer_width_tag(value: PointerWidth) -> u8 {
    match value {
        PointerWidth::Bits32 => 0,
        PointerWidth::Bits64 => 1,
    }
}

const fn endianness_tag(value: Endianness) -> u8 {
    match value {
        Endianness::Little => 0,
        Endianness::Big => 1,
    }
}

const fn capability_tag(value: Capability) -> u16 {
    match value {
        Capability::Subgroup => 0,
        Capability::Ballot => 1,
        Capability::Shuffle => 2,
        Capability::WorkgroupMemory => 3,
        Capability::MatrixMultiply => 4,
        Capability::AsyncCopy => 5,
        Capability::Atomics => 6,
        Capability::AmdWave => 7,
        Capability::AmdMfma => 8,
        Capability::AmdWmma => 9,
        Capability::AmdDsPermute => 10,
    }
}

const fn code_object_format_tag(value: CodeObjectFormat) -> u8 {
    match value {
        CodeObjectFormat::NativeExecutable => 0,
        CodeObjectFormat::RelocatableObject => 1,
        CodeObjectFormat::LlvmBitcode => 2,
        CodeObjectFormat::SpirV => 3,
    }
}

const fn scalar_tag(value: ScalarType) -> u8 {
    match value {
        ScalarType::I8 => 0,
        ScalarType::U8 => 1,
        ScalarType::I16 => 2,
        ScalarType::U16 => 3,
        ScalarType::I32 => 4,
        ScalarType::U32 => 5,
        ScalarType::I64 => 6,
        ScalarType::U64 => 7,
        ScalarType::F16 => 8,
        ScalarType::F32 => 9,
        ScalarType::F64 => 10,
    }
}

const fn mutability_tag(value: Mutability) -> u8 {
    match value {
        Mutability::Immutable => 0,
        Mutability::Mutable => 1,
    }
}

const fn access_tag(value: Access) -> u8 {
    match value {
        Access::ByValue => 0,
        Access::ReadOnly => 1,
        Access::WriteOnly => 2,
        Access::ReadWrite => 3,
    }
}

const fn address_space_tag(value: AddressSpace) -> u8 {
    match value {
        AddressSpace::Value => 0,
        AddressSpace::Global => 1,
        AddressSpace::Constant => 2,
        AddressSpace::Workgroup => 3,
        AddressSpace::Private => 4,
        AddressSpace::Generic => 5,
    }
}

const fn ownership_tag(value: ArgumentOwnership) -> u8 {
    match value {
        ArgumentOwnership::ByValue => 0,
        ArgumentOwnership::SharedBorrow => 1,
        ArgumentOwnership::UniqueBorrow => 2,
        ArgumentOwnership::RawPointer => 3,
    }
}

const fn alias_class_tag(value: AliasClass) -> u8 {
    match value {
        AliasClass::Value => 0,
        AliasClass::SharedReadOnly => 1,
        AliasClass::Exclusive => 2,
        AliasClass::Unrestricted => 3,
        AliasClass::SharedAtomic => 4,
    }
}
