use crate::ValidationError;

pub const MAX_UNIT_BYTES_V1: usize = 4 * 1024 * 1024;
pub const MAX_FUNCTIONS_V1: usize = 4096;
pub const MAX_FUNCTION_NAME_BYTES_V1: usize = 512;
pub const MAX_PARAMETERS_PER_FUNCTION_V1: usize = 128;
pub const MAX_BLOCKS_PER_FUNCTION_V1: usize = 65_535;
pub const MAX_TOTAL_BLOCKS_V1: usize = 131_072;
pub const MAX_SUCCESSORS_PER_BLOCK_V1: usize = 256;

macro_rules! opaque_identity {
    ($name:ident, $field:literal) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            pub fn new(bytes: [u8; 32]) -> Result<Self, ValidationError> {
                if bytes == [0; 32] {
                    return Err(ValidationError::ZeroIdentity { field: $field });
                }
                Ok(Self(bytes))
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

opaque_identity!(FunctionIdentityV1, "function identity");
opaque_identity!(StableTypeIdentityV1, "stable type identity");
opaque_identity!(SourceFileIdentityV1, "source file identity");

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct BlockIdV1(u32);

impl BlockIdV1 {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FunctionRoleV1 {
    Kernel,
    Helper,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceLocationV1 {
    file: SourceFileIdentityV1,
    line: u32,
    column: u32,
}

impl SourceLocationV1 {
    pub fn new(
        file: SourceFileIdentityV1,
        line: u32,
        column: u32,
    ) -> Result<Self, ValidationError> {
        if line == 0 || column == 0 {
            return Err(ValidationError::InvalidSourceLocation);
        }
        Ok(Self { file, line, column })
    }

    pub const fn file(self) -> SourceFileIdentityV1 {
        self.file
    }

    pub const fn line(self) -> u32 {
        self.line
    }

    pub const fn column(self) -> u32 {
        self.column
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedSignatureV1 {
    parameters: Vec<StableTypeIdentityV1>,
    return_type: StableTypeIdentityV1,
}

impl TypedSignatureV1 {
    pub fn new(
        parameters: Vec<StableTypeIdentityV1>,
        return_type: StableTypeIdentityV1,
    ) -> Result<Self, ValidationError> {
        if parameters.len() > MAX_PARAMETERS_PER_FUNCTION_V1 {
            return Err(ValidationError::TooMany {
                field: "function parameters",
                max: MAX_PARAMETERS_PER_FUNCTION_V1,
            });
        }
        Ok(Self {
            parameters,
            return_type,
        })
    }

    pub fn parameters(&self) -> &[StableTypeIdentityV1] {
        &self.parameters
    }

    pub const fn return_type(&self) -> StableTypeIdentityV1 {
        self.return_type
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BasicBlockV1 {
    id: BlockIdV1,
    location: SourceLocationV1,
    successors: Vec<BlockIdV1>,
}

impl BasicBlockV1 {
    pub fn new(
        id: BlockIdV1,
        location: SourceLocationV1,
        mut successors: Vec<BlockIdV1>,
    ) -> Result<Self, ValidationError> {
        if successors.len() > MAX_SUCCESSORS_PER_BLOCK_V1 {
            return Err(ValidationError::TooMany {
                field: "CFG block successors",
                max: MAX_SUCCESSORS_PER_BLOCK_V1,
            });
        }
        successors.sort_unstable();
        if successors.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ValidationError::Duplicate {
                field: "CFG block successors",
            });
        }
        Ok(Self {
            id,
            location,
            successors,
        })
    }

    pub const fn id(&self) -> BlockIdV1 {
        self.id
    }

    pub const fn location(&self) -> SourceLocationV1 {
        self.location
    }

    pub fn successors(&self) -> &[BlockIdV1] {
        &self.successors
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MonomorphizedFunctionV1 {
    identity: FunctionIdentityV1,
    role: FunctionRoleV1,
    diagnostic_name: String,
    location: SourceLocationV1,
    signature: TypedSignatureV1,
    entry_block: BlockIdV1,
    blocks: Vec<BasicBlockV1>,
}

impl MonomorphizedFunctionV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identity: FunctionIdentityV1,
        role: FunctionRoleV1,
        diagnostic_name: impl Into<String>,
        location: SourceLocationV1,
        signature: TypedSignatureV1,
        entry_block: BlockIdV1,
        mut blocks: Vec<BasicBlockV1>,
    ) -> Result<Self, ValidationError> {
        let diagnostic_name = diagnostic_name.into();
        validate_diagnostic_name(&diagnostic_name)?;
        if blocks.is_empty() {
            return Err(ValidationError::Empty {
                field: "function CFG blocks",
            });
        }
        if blocks.len() > MAX_BLOCKS_PER_FUNCTION_V1 {
            return Err(ValidationError::TooMany {
                field: "function CFG blocks",
                max: MAX_BLOCKS_PER_FUNCTION_V1,
            });
        }
        blocks.sort_unstable_by_key(BasicBlockV1::id);
        for (expected, block) in blocks.iter().enumerate() {
            let expected = u32::try_from(expected).map_err(|_| ValidationError::Overflow {
                field: "CFG block identity",
            })?;
            if block.id().get() != expected {
                return Err(ValidationError::NonDenseBlockId {
                    expected,
                    actual: block.id().get(),
                });
            }
        }
        let block_count = u32::try_from(blocks.len()).map_err(|_| ValidationError::Overflow {
            field: "CFG block count",
        })?;
        if entry_block.get() >= block_count {
            return Err(ValidationError::InvalidEntryBlock {
                block: entry_block.get(),
            });
        }
        for block in &blocks {
            for successor in block.successors() {
                if successor.get() >= block_count {
                    return Err(ValidationError::InvalidSuccessor {
                        block: block.id().get(),
                        successor: successor.get(),
                    });
                }
            }
        }
        Ok(Self {
            identity,
            role,
            diagnostic_name,
            location,
            signature,
            entry_block,
            blocks,
        })
    }

    pub const fn identity(&self) -> FunctionIdentityV1 {
        self.identity
    }

    pub const fn role(&self) -> FunctionRoleV1 {
        self.role
    }

    pub fn diagnostic_name(&self) -> &str {
        &self.diagnostic_name
    }

    pub const fn location(&self) -> SourceLocationV1 {
        self.location
    }

    pub const fn signature(&self) -> &TypedSignatureV1 {
        &self.signature
    }

    pub const fn entry_block(&self) -> BlockIdV1 {
        self.entry_block
    }

    pub fn blocks(&self) -> &[BasicBlockV1] {
        &self.blocks
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendUnitV1 {
    functions: Vec<MonomorphizedFunctionV1>,
}

impl FrontendUnitV1 {
    pub fn new(mut functions: Vec<MonomorphizedFunctionV1>) -> Result<Self, ValidationError> {
        if functions.is_empty() {
            return Err(ValidationError::Empty {
                field: "frontend functions",
            });
        }
        if functions.len() > MAX_FUNCTIONS_V1 {
            return Err(ValidationError::TooMany {
                field: "frontend functions",
                max: MAX_FUNCTIONS_V1,
            });
        }
        functions.sort_unstable_by_key(MonomorphizedFunctionV1::identity);
        if functions
            .windows(2)
            .any(|pair| pair[0].identity() == pair[1].identity())
        {
            return Err(ValidationError::Duplicate {
                field: "function identities",
            });
        }
        if !functions
            .iter()
            .any(|function| function.role() == FunctionRoleV1::Kernel)
        {
            return Err(ValidationError::MissingKernel);
        }
        let total_blocks = functions.iter().try_fold(0_usize, |total, function| {
            total.checked_add(function.blocks().len())
        });
        if total_blocks.is_none_or(|count| count > MAX_TOTAL_BLOCKS_V1) {
            return Err(ValidationError::TooMany {
                field: "frontend CFG blocks",
                max: MAX_TOTAL_BLOCKS_V1,
            });
        }
        let unit = Self { functions };
        crate::encode::validate_encoded_size(&unit)?;
        Ok(unit)
    }

    pub fn functions(&self) -> &[MonomorphizedFunctionV1] {
        &self.functions
    }
}

fn validate_diagnostic_name(value: &str) -> Result<(), ValidationError> {
    if value.is_empty() {
        return Err(ValidationError::Empty {
            field: "function diagnostic name",
        });
    }
    if value.len() > MAX_FUNCTION_NAME_BYTES_V1 {
        return Err(ValidationError::TextTooLong {
            field: "function diagnostic name",
            max: MAX_FUNCTION_NAME_BYTES_V1,
        });
    }
    if value.chars().any(char::is_control) {
        return Err(ValidationError::InvalidText {
            field: "function diagnostic name",
        });
    }
    Ok(())
}
