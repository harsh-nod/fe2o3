use crate::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, BlockSize, Capability, CodeObjectFormat,
    CodeObjectIdentity, CompilerIdentity, DecodeError, DigestBytes, Dimensions, Endianness,
    IdentityText, KernelEntry, LaunchContract, MANIFEST_MAGIC, MANIFEST_VERSION, MAX_ABI_FIELDS,
    MAX_CODE_OBJECTS, MAX_IDENTITY_TEXT_BYTES, MAX_KERNELS, MAX_MANIFEST_BYTES, MAX_NAME_BYTES,
    ManifestV1, Mutability, Name, PointerWidth, ScalarType, TargetIdentity, ToolIdentity,
    ValidationError,
};

const CAPABILITY_COUNT: usize = 11;

impl ManifestV1 {
    /// Decodes bytes and returns a manifest only after all wire and semantic checks pass.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(DecodeError::TooLarge {
                max: MAX_MANIFEST_BYTES,
            });
        }
        let mut reader = Reader::new(bytes);
        if reader.array::<8>()? != MANIFEST_MAGIC {
            return Err(DecodeError::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != MANIFEST_VERSION {
            return Err(DecodeError::UnknownVersion(version));
        }
        let flags = reader.u16()?;
        if flags != 0 {
            return Err(DecodeError::UnsupportedFlags(flags));
        }

        let compiler = CompilerIdentity::new(reader.identity_text()?, reader.identity_text()?);
        let producer = ToolIdentity::new(reader.identity_text()?, reader.identity_text()?);
        let target = TargetIdentity::new(
            reader.identity_text()?,
            reader.identity_text()?,
            reader.pointer_width()?,
            reader.endianness()?,
            reader.capabilities("target capabilities")?,
        )?;

        let code_object_count = reader.count_u32("code objects", 1, MAX_CODE_OBJECTS)?;
        let mut code_objects = Vec::with_capacity(code_object_count);
        for _ in 0..code_object_count {
            code_objects.push(CodeObjectIdentity::new(
                reader.digest()?,
                reader.code_object_format()?,
                reader.u64()?,
            )?);
        }
        ensure_code_object_order(&code_objects)?;

        let kernel_count = reader.count_u32("kernels", 1, MAX_KERNELS)?;
        let mut kernels = Vec::with_capacity(kernel_count);
        for _ in 0..kernel_count {
            let kernel_id = reader.digest()?;
            let name = reader.name()?;
            let symbol = reader.name()?;
            let source_digest = reader.digest()?;
            let executable_digest = reader.digest()?;
            let code_object_digest = reader.digest()?;
            let capabilities = reader.capabilities("kernel capabilities")?;
            let launch = reader.launch()?;
            let abi = reader.abi(target.pointer_width())?;
            kernels.push(KernelEntry::new(
                kernel_id,
                name,
                symbol,
                source_digest,
                executable_digest,
                code_object_digest,
                capabilities,
                launch,
                abi,
            )?);
        }
        ensure_kernel_order(&kernels)?;

        if !reader.is_empty() {
            return Err(DecodeError::TrailingBytes);
        }
        Ok(Self::new(
            compiler,
            producer,
            target,
            code_objects,
            kernels,
        )?)
    }
}

struct Reader<'a> {
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    const fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], DecodeError> {
        if self.remaining.len() < count {
            return Err(DecodeError::Truncated);
        }
        let (value, remaining) = self.remaining.split_at(count);
        self.remaining = remaining;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let mut value = [0; N];
        value.copy_from_slice(self.take(N)?);
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn count_u16(
        &mut self,
        field: &'static str,
        min: usize,
        max: usize,
    ) -> Result<usize, DecodeError> {
        let count = usize::from(self.u16()?);
        validate_count(field, count as u64, min, max)?;
        Ok(count)
    }

    fn count_u32(
        &mut self,
        field: &'static str,
        min: usize,
        max: usize,
    ) -> Result<usize, DecodeError> {
        let count = u64::from(self.u32()?);
        validate_count(field, count, min, max)?;
        Ok(count as usize)
    }

    fn text(&mut self, field: &'static str, max: usize) -> Result<&'a str, DecodeError> {
        let count = self.count_u16(field, 1, max)?;
        let bytes = self.take(count)?;
        std::str::from_utf8(bytes).map_err(|_| ValidationError::InvalidText { field }.into())
    }

    fn name(&mut self) -> Result<Name, DecodeError> {
        Ok(Name::new(self.text("name", MAX_NAME_BYTES)?)?)
    }

    fn identity_text(&mut self) -> Result<IdentityText, DecodeError> {
        Ok(IdentityText::new(
            self.text("identity text", MAX_IDENTITY_TEXT_BYTES)?,
        )?)
    }

    fn digest(&mut self) -> Result<DigestBytes, DecodeError> {
        Ok(DigestBytes::from_bytes(self.array()?))
    }

    fn pointer_width(&mut self) -> Result<PointerWidth, DecodeError> {
        match self.u8()? {
            0 => Ok(PointerWidth::Bits32),
            1 => Ok(PointerWidth::Bits64),
            tag => Err(DecodeError::UnknownTag {
                kind: "pointer width",
                tag,
            }),
        }
    }

    fn endianness(&mut self) -> Result<Endianness, DecodeError> {
        match self.u8()? {
            0 => Ok(Endianness::Little),
            1 => Ok(Endianness::Big),
            tag => Err(DecodeError::UnknownTag {
                kind: "endianness",
                tag,
            }),
        }
    }

    fn capabilities(&mut self, field: &'static str) -> Result<Vec<Capability>, DecodeError> {
        let count = self.count_u16(field, 0, CAPABILITY_COUNT)?;
        let mut capabilities = Vec::with_capacity(count);
        for _ in 0..count {
            capabilities.push(capability_from_tag(self.u16()?)?);
        }
        ensure_capability_order(&capabilities, field)?;
        Ok(capabilities)
    }

    fn code_object_format(&mut self) -> Result<CodeObjectFormat, DecodeError> {
        match self.u8()? {
            0 => Ok(CodeObjectFormat::NativeExecutable),
            1 => Ok(CodeObjectFormat::RelocatableObject),
            2 => Ok(CodeObjectFormat::LlvmBitcode),
            3 => Ok(CodeObjectFormat::SpirV),
            tag => Err(DecodeError::UnknownTag {
                kind: "code object format",
                tag,
            }),
        }
    }

    fn dimensions(&mut self) -> Result<Dimensions, DecodeError> {
        Ok(Dimensions::new(self.u32()?, self.u32()?, self.u32()?)?)
    }

    fn launch(&mut self) -> Result<LaunchContract, DecodeError> {
        let rank = self.u8()?;
        let block_size = match self.u8()? {
            0 => BlockSize::Any,
            1 => BlockSize::Exact(self.dimensions()?),
            2 => BlockSize::AtMost(self.dimensions()?),
            tag => {
                return Err(DecodeError::UnknownTag {
                    kind: "block size",
                    tag,
                });
            }
        };
        Ok(LaunchContract::new(
            rank,
            block_size,
            self.dimensions()?,
            self.u32()?,
            self.u32()?,
        )?)
    }

    fn abi(&mut self, pointer_width: PointerWidth) -> Result<AbiLayout, DecodeError> {
        let size = self.u64()?;
        let alignment = self.u32()?;
        let field_count = self.count_u16("ABI fields", 0, MAX_ABI_FIELDS)?;
        let mut fields = Vec::with_capacity(field_count);
        for _ in 0..field_count {
            let name = self.name()?;
            let offset = self.u64()?;
            let field_size = self.u64()?;
            let field_alignment = self.u32()?;
            let kind = match self.u8()? {
                0 => AbiKind::Scalar(self.scalar_type()?),
                1 => AbiKind::Pointer {
                    pointee_size: self.u64()?,
                    pointee_alignment: self.u32()?,
                },
                2 => AbiKind::Slice {
                    element_size: self.u64()?,
                    element_alignment: self.u32()?,
                },
                tag => {
                    return Err(DecodeError::UnknownTag {
                        kind: "ABI kind",
                        tag,
                    });
                }
            };
            fields.push(AbiField::new(
                name,
                offset,
                field_size,
                field_alignment,
                kind,
                self.mutability()?,
                self.access()?,
                self.address_space()?,
            )?);
        }
        Ok(AbiLayout::new(size, alignment, pointer_width, fields)?)
    }

    fn scalar_type(&mut self) -> Result<ScalarType, DecodeError> {
        match self.u8()? {
            0 => Ok(ScalarType::I8),
            1 => Ok(ScalarType::U8),
            2 => Ok(ScalarType::I16),
            3 => Ok(ScalarType::U16),
            4 => Ok(ScalarType::I32),
            5 => Ok(ScalarType::U32),
            6 => Ok(ScalarType::I64),
            7 => Ok(ScalarType::U64),
            8 => Ok(ScalarType::F16),
            9 => Ok(ScalarType::F32),
            10 => Ok(ScalarType::F64),
            tag => Err(DecodeError::UnknownTag {
                kind: "scalar type",
                tag,
            }),
        }
    }

    fn mutability(&mut self) -> Result<Mutability, DecodeError> {
        match self.u8()? {
            0 => Ok(Mutability::Immutable),
            1 => Ok(Mutability::Mutable),
            tag => Err(DecodeError::UnknownTag {
                kind: "mutability",
                tag,
            }),
        }
    }

    fn access(&mut self) -> Result<Access, DecodeError> {
        match self.u8()? {
            0 => Ok(Access::ByValue),
            1 => Ok(Access::ReadOnly),
            2 => Ok(Access::WriteOnly),
            3 => Ok(Access::ReadWrite),
            tag => Err(DecodeError::UnknownTag {
                kind: "access",
                tag,
            }),
        }
    }

    fn address_space(&mut self) -> Result<AddressSpace, DecodeError> {
        match self.u8()? {
            0 => Ok(AddressSpace::Value),
            1 => Ok(AddressSpace::Global),
            2 => Ok(AddressSpace::Constant),
            3 => Ok(AddressSpace::Workgroup),
            4 => Ok(AddressSpace::Private),
            5 => Ok(AddressSpace::Generic),
            tag => Err(DecodeError::UnknownTag {
                kind: "address space",
                tag,
            }),
        }
    }
}

fn validate_count(
    field: &'static str,
    count: u64,
    min: usize,
    max: usize,
) -> Result<(), DecodeError> {
    if count < min as u64 || count > max as u64 {
        Err(DecodeError::CountOutOfRange {
            field,
            count,
            min,
            max,
        })
    } else {
        Ok(())
    }
}

fn ensure_capability_order(
    capabilities: &[Capability],
    field: &'static str,
) -> Result<(), DecodeError> {
    for pair in capabilities.windows(2) {
        if pair[0] == pair[1] {
            return Err(ValidationError::Duplicate { field }.into());
        }
        if pair[0] > pair[1] {
            return Err(DecodeError::NonCanonicalOrder { field });
        }
    }
    Ok(())
}

fn ensure_code_object_order(code_objects: &[CodeObjectIdentity]) -> Result<(), DecodeError> {
    for pair in code_objects.windows(2) {
        if pair[0].digest() == pair[1].digest() {
            return Err(ValidationError::Duplicate {
                field: "code object digest",
            }
            .into());
        }
        if pair[0].digest() > pair[1].digest() {
            return Err(DecodeError::NonCanonicalOrder {
                field: "code objects",
            });
        }
    }
    Ok(())
}

fn ensure_kernel_order(kernels: &[KernelEntry]) -> Result<(), DecodeError> {
    for pair in kernels.windows(2) {
        if pair[0].kernel_id() == pair[1].kernel_id() {
            return Err(ValidationError::Duplicate { field: "kernel ID" }.into());
        }
        if pair[0].kernel_id() > pair[1].kernel_id() {
            return Err(DecodeError::NonCanonicalOrder { field: "kernels" });
        }
    }
    Ok(())
}

fn capability_from_tag(tag: u16) -> Result<Capability, DecodeError> {
    match tag {
        0 => Ok(Capability::Subgroup),
        1 => Ok(Capability::Ballot),
        2 => Ok(Capability::Shuffle),
        3 => Ok(Capability::WorkgroupMemory),
        4 => Ok(Capability::MatrixMultiply),
        5 => Ok(Capability::AsyncCopy),
        6 => Ok(Capability::Atomics),
        7 => Ok(Capability::AmdWave),
        8 => Ok(Capability::AmdMfma),
        9 => Ok(Capability::AmdWmma),
        10 => Ok(Capability::AmdDsPermute),
        _ => Err(DecodeError::UnknownCapability(tag)),
    }
}
