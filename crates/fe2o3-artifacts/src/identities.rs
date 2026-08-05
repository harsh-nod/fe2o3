use crate::ValidationError;

pub const MAX_NAME_BYTES: usize = 128;
pub const MAX_IDENTITY_TEXT_BYTES: usize = 256;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Name(String);

impl Name {
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        validate_name(&value, "name")?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct IdentityText(String);

impl IdentityText {
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        validate_identity_text(&value, "identity text")?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct DigestBytes([u8; 32]);

impl DigestBytes {
    /// Wraps opaque identity bytes. This does not calculate or verify a digest.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerIdentity {
    name: IdentityText,
    version: IdentityText,
}

impl CompilerIdentity {
    pub const fn new(name: IdentityText, version: IdentityText) -> Self {
        Self { name, version }
    }

    pub const fn name(&self) -> &IdentityText {
        &self.name
    }

    pub const fn version(&self) -> &IdentityText {
        &self.version
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolIdentity {
    name: IdentityText,
    version: IdentityText,
}

impl ToolIdentity {
    pub const fn new(name: IdentityText, version: IdentityText) -> Self {
        Self { name, version }
    }

    pub const fn name(&self) -> &IdentityText {
        &self.name
    }

    pub const fn version(&self) -> &IdentityText {
        &self.version
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PointerWidth {
    Bits32,
    Bits64,
}

impl PointerWidth {
    pub const fn bytes(self) -> u64 {
        match self {
            Self::Bits32 => 4,
            Self::Bits64 => 8,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Endianness {
    Little,
    Big,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Capability {
    Subgroup,
    Ballot,
    Shuffle,
    WorkgroupMemory,
    MatrixMultiply,
    AsyncCopy,
    Atomics,
    AmdWave,
    AmdMfma,
    AmdWmma,
    AmdDsPermute,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetIdentity {
    triple: IdentityText,
    architecture: IdentityText,
    pointer_width: PointerWidth,
    endianness: Endianness,
    capabilities: Vec<Capability>,
}

impl TargetIdentity {
    pub fn new(
        triple: IdentityText,
        architecture: IdentityText,
        pointer_width: PointerWidth,
        endianness: Endianness,
        mut capabilities: Vec<Capability>,
    ) -> Result<Self, ValidationError> {
        sort_unique(&mut capabilities, "target capability")?;
        Ok(Self {
            triple,
            architecture,
            pointer_width,
            endianness,
            capabilities,
        })
    }

    pub const fn triple(&self) -> &IdentityText {
        &self.triple
    }

    pub const fn architecture(&self) -> &IdentityText {
        &self.architecture
    }

    pub const fn pointer_width(&self) -> PointerWidth {
        self.pointer_width
    }

    pub const fn endianness(&self) -> Endianness {
        self.endianness
    }

    pub fn capabilities(&self) -> &[Capability] {
        &self.capabilities
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CodeObjectFormat {
    NativeExecutable,
    RelocatableObject,
    LlvmBitcode,
    SpirV,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeObjectIdentity {
    digest: DigestBytes,
    format: CodeObjectFormat,
    byte_len: u64,
}

impl CodeObjectIdentity {
    pub fn new(
        digest: DigestBytes,
        format: CodeObjectFormat,
        byte_len: u64,
    ) -> Result<Self, ValidationError> {
        if byte_len == 0 {
            return Err(ValidationError::InvalidLayout(
                "code object byte length must be nonzero",
            ));
        }
        Ok(Self {
            digest,
            format,
            byte_len,
        })
    }

    pub const fn digest(&self) -> DigestBytes {
        self.digest
    }

    pub const fn format(&self) -> CodeObjectFormat {
        self.format
    }

    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Dimensions {
    x: u32,
    y: u32,
    z: u32,
}

impl Dimensions {
    pub fn new(x: u32, y: u32, z: u32) -> Result<Self, ValidationError> {
        if x == 0 || y == 0 || z == 0 {
            return Err(ValidationError::InvalidDimension { field: "launch" });
        }
        Ok(Self { x, y, z })
    }

    pub const fn x(self) -> u32 {
        self.x
    }

    pub const fn y(self) -> u32 {
        self.y
    }

    pub const fn z(self) -> u32 {
        self.z
    }

    fn validate_rank(self, rank: u8, field: &'static str) -> Result<(), ValidationError> {
        if (rank < 2 && self.y != 1) || (rank < 3 && self.z != 1) {
            return Err(ValidationError::InvalidDimension { field });
        }
        u64::from(self.x)
            .checked_mul(u64::from(self.y))
            .and_then(|xy| xy.checked_mul(u64::from(self.z)))
            .ok_or(ValidationError::Overflow(field))?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockSize {
    Any,
    Exact(Dimensions),
    AtMost(Dimensions),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchContract {
    rank: u8,
    block_size: BlockSize,
    max_grid: Dimensions,
    static_shared_memory_bytes: u32,
    max_dynamic_shared_memory_bytes: u32,
}

impl LaunchContract {
    pub fn new(
        rank: u8,
        block_size: BlockSize,
        max_grid: Dimensions,
        static_shared_memory_bytes: u32,
        max_dynamic_shared_memory_bytes: u32,
    ) -> Result<Self, ValidationError> {
        if !(1..=3).contains(&rank) {
            return Err(ValidationError::InvalidRank(rank));
        }
        max_grid.validate_rank(rank, "grid")?;
        match block_size {
            BlockSize::Any => {}
            BlockSize::Exact(dimensions) | BlockSize::AtMost(dimensions) => {
                dimensions.validate_rank(rank, "block")?;
            }
        }
        static_shared_memory_bytes
            .checked_add(max_dynamic_shared_memory_bytes)
            .ok_or(ValidationError::Overflow("shared memory"))?;
        Ok(Self {
            rank,
            block_size,
            max_grid,
            static_shared_memory_bytes,
            max_dynamic_shared_memory_bytes,
        })
    }

    pub const fn rank(&self) -> u8 {
        self.rank
    }

    pub const fn block_size(&self) -> BlockSize {
        self.block_size
    }

    pub const fn max_grid(&self) -> Dimensions {
        self.max_grid
    }

    pub const fn static_shared_memory_bytes(&self) -> u32 {
        self.static_shared_memory_bytes
    }

    pub const fn max_dynamic_shared_memory_bytes(&self) -> u32 {
        self.max_dynamic_shared_memory_bytes
    }
}

fn validate_name(value: &str, field: &'static str) -> Result<(), ValidationError> {
    if value.is_empty() {
        return Err(ValidationError::Empty { field });
    }
    if value.len() > MAX_NAME_BYTES {
        return Err(ValidationError::TooLong {
            field,
            max: MAX_NAME_BYTES,
        });
    }
    let mut bytes = value.bytes();
    let first = bytes.next().ok_or(ValidationError::Empty { field })?;
    if !(first.is_ascii_alphabetic() || first == b'_')
        || !bytes.all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'$')
        })
    {
        return Err(ValidationError::InvalidText { field });
    }
    Ok(())
}

fn validate_identity_text(value: &str, field: &'static str) -> Result<(), ValidationError> {
    if value.is_empty() {
        return Err(ValidationError::Empty { field });
    }
    if value.len() > MAX_IDENTITY_TEXT_BYTES {
        return Err(ValidationError::TooLong {
            field,
            max: MAX_IDENTITY_TEXT_BYTES,
        });
    }
    if value.trim() != value || !value.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
        return Err(ValidationError::InvalidText { field });
    }
    Ok(())
}

fn sort_unique<T: Ord>(values: &mut [T], field: &'static str) -> Result<(), ValidationError> {
    values.sort_unstable();
    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ValidationError::Duplicate { field });
    }
    Ok(())
}
