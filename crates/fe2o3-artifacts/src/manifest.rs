use crate::{
    AbiLayout, AliasClass, Capability, CodeObjectIdentity, CompilerIdentity, DigestBytes,
    LaunchContract, Name, TargetIdentity, ToolIdentity, ValidationError,
};

pub const MAX_CODE_OBJECTS: usize = 128;
pub const MAX_KERNELS: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelEntry {
    kernel_id: DigestBytes,
    name: Name,
    symbol: Name,
    source_digest: DigestBytes,
    executable_digest: DigestBytes,
    code_object_digest: DigestBytes,
    required_capabilities: Vec<Capability>,
    launch: LaunchContract,
    abi: AbiLayout,
}

impl KernelEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kernel_id: DigestBytes,
        name: Name,
        symbol: Name,
        source_digest: DigestBytes,
        executable_digest: DigestBytes,
        code_object_digest: DigestBytes,
        mut required_capabilities: Vec<Capability>,
        launch: LaunchContract,
        abi: AbiLayout,
    ) -> Result<Self, ValidationError> {
        sort_unique(&mut required_capabilities, "kernel capability")?;
        Ok(Self {
            kernel_id,
            name,
            symbol,
            source_digest,
            executable_digest,
            code_object_digest,
            required_capabilities,
            launch,
            abi,
        })
    }

    pub const fn kernel_id(&self) -> DigestBytes {
        self.kernel_id
    }

    pub const fn name(&self) -> &Name {
        &self.name
    }

    pub const fn symbol(&self) -> &Name {
        &self.symbol
    }

    pub const fn source_digest(&self) -> DigestBytes {
        self.source_digest
    }

    pub const fn executable_digest(&self) -> DigestBytes {
        self.executable_digest
    }

    pub const fn code_object_digest(&self) -> DigestBytes {
        self.code_object_digest
    }

    pub fn required_capabilities(&self) -> &[Capability] {
        &self.required_capabilities
    }

    pub const fn launch(&self) -> &LaunchContract {
        &self.launch
    }

    pub const fn abi(&self) -> &AbiLayout {
        &self.abi
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestV1 {
    compiler: CompilerIdentity,
    producer: ToolIdentity,
    target: TargetIdentity,
    code_objects: Vec<CodeObjectIdentity>,
    kernels: Vec<KernelEntry>,
}

impl ManifestV1 {
    pub fn new(
        compiler: CompilerIdentity,
        producer: ToolIdentity,
        target: TargetIdentity,
        mut code_objects: Vec<CodeObjectIdentity>,
        mut kernels: Vec<KernelEntry>,
    ) -> Result<Self, ValidationError> {
        require_count(code_objects.len(), "code objects", MAX_CODE_OBJECTS)?;
        require_count(kernels.len(), "kernels", MAX_KERNELS)?;

        code_objects.sort_unstable_by_key(CodeObjectIdentity::digest);
        if code_objects
            .windows(2)
            .any(|pair| pair[0].digest() == pair[1].digest())
        {
            return Err(ValidationError::Duplicate {
                field: "code object digest",
            });
        }
        kernels.sort_unstable_by_key(KernelEntry::kernel_id);
        if kernels
            .windows(2)
            .any(|pair| pair[0].kernel_id() == pair[1].kernel_id())
        {
            return Err(ValidationError::Duplicate { field: "kernel ID" });
        }

        reject_duplicate_kernel_names(&kernels, false)?;
        reject_duplicate_kernel_names(&kernels, true)?;
        for kernel in &kernels {
            if kernel.abi().pointer_width() != target.pointer_width() {
                return Err(ValidationError::PointerWidthMismatch);
            }
            if code_objects
                .binary_search_by_key(&kernel.code_object_digest(), CodeObjectIdentity::digest)
                .is_err()
            {
                return Err(ValidationError::MissingCodeObject);
            }
            for capability in kernel.required_capabilities() {
                if target.capabilities().binary_search(capability).is_err() {
                    return Err(ValidationError::MissingCapability(capability.name()));
                }
            }
            if kernel
                .abi()
                .fields()
                .iter()
                .any(|field| field.alias_class() == AliasClass::SharedAtomic)
                && kernel
                    .required_capabilities()
                    .binary_search(&Capability::Atomics)
                    .is_err()
            {
                return Err(ValidationError::MissingCapability("atomics"));
            }
        }

        Ok(Self {
            compiler,
            producer,
            target,
            code_objects,
            kernels,
        })
    }

    pub const fn compiler(&self) -> &CompilerIdentity {
        &self.compiler
    }

    pub const fn producer(&self) -> &ToolIdentity {
        &self.producer
    }

    pub const fn target(&self) -> &TargetIdentity {
        &self.target
    }

    pub fn code_objects(&self) -> &[CodeObjectIdentity] {
        &self.code_objects
    }

    pub fn kernels(&self) -> &[KernelEntry] {
        &self.kernels
    }
}

fn sort_unique<T: Ord>(values: &mut [T], field: &'static str) -> Result<(), ValidationError> {
    values.sort_unstable();
    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ValidationError::Duplicate { field });
    }
    Ok(())
}

fn require_count(count: usize, field: &'static str, max: usize) -> Result<(), ValidationError> {
    if count == 0 {
        Err(ValidationError::EmptyCollection { field })
    } else if count > max {
        Err(ValidationError::TooMany { field, max })
    } else {
        Ok(())
    }
}

fn reject_duplicate_kernel_names(
    kernels: &[KernelEntry],
    symbols: bool,
) -> Result<(), ValidationError> {
    let mut names: Vec<&Name> = kernels
        .iter()
        .map(|kernel| {
            if symbols {
                kernel.symbol()
            } else {
                kernel.name()
            }
        })
        .collect();
    names.sort_unstable();
    if names.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ValidationError::Duplicate {
            field: if symbols {
                "kernel symbol"
            } else {
                "kernel name"
            },
        });
    }
    Ok(())
}
