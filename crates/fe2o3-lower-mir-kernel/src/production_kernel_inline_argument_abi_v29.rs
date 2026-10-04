use fe2o3_mir_model::semantic_mir_v1::SemanticAbiArgumentV1;

/// Original source-owned by-value ABI proposal, not a native or memory permit.
pub enum ProductionKernelByValueAbiV29<'a> {
    /// Original Ignore mode has no physical parameter or byte interval.
    Ignored(ProductionKernelIgnoredArgumentAbiV29<'a>),
    /// Captured inline region, whose final native/runtime mapping is still required.
    Inline(ProductionKernelInlineArgumentAbiV29<'a>),
}

struct OriginalByValueAbiV29<'a> {
    profile: &'a CapturedKernelArgumentAbiV18,
    root: &'a Root,
    root_ordinal: usize,
    argument: &'a Argument,
    argument_ordinal: usize,
    declaration: &'a SemanticTypeDeclV1,
    abi: &'a SemanticAbiArgumentV1,
}

/// Borrowed original Ignore argument; no physical offset is exposed.
pub struct ProductionKernelIgnoredArgumentAbiV29<'a> {
    original: OriginalByValueAbiV29<'a>,
}

/// Borrowed inline source interval. It makes no initialization/provenance claim.
pub struct ProductionKernelInlineArgumentAbiV29<'a> {
    original: OriginalByValueAbiV29<'a>,
    offset: u32,
    size: u64,
    alignment: u64,
}

impl ProductionKernelByValueAbiV29<'_> {
    fn original(&self) -> &OriginalByValueAbiV29<'_> {
        match self {
            Self::Ignored(value) => &value.original,
            Self::Inline(value) => &value.original,
        }
    }

    /// Original root ordinal, not a final or optimized function number.
    pub fn root(&self) -> usize {
        self.original().root_ordinal
    }
    /// Original semantic kernel function.
    pub fn function(&self) -> SemanticFunctionIdV1 {
        self.original().root.function
    }
    /// Original source argument ordinal.
    pub fn argument(&self) -> usize {
        self.original().argument_ordinal
    }
    /// Original source type, not the selected physical storage schema.
    pub fn source_type(&self) -> SemanticTypeIdV1 {
        self.original().argument.ty
    }
    /// Original source type identity.
    pub fn source_type_identity(&self) -> SemanticTypeIdentityV1 {
        self.original().argument.identity
    }
    /// Complete admitted original argument ABI, including its mode.
    pub fn source_abi(&self) -> &SemanticAbiArgumentV1 {
        self.original().abi
    }
    /// Original source layout; Ignore may retain alignment absent from physical ABI.
    pub fn source_declaration(&self) -> &SemanticTypeDeclV1 {
        self.original().declaration
    }
    /// Original source owner identity retained by the same captured profile.
    pub fn source_sha256(&self) -> &[u8; 32] {
        &self.original().profile.source
    }
    /// Original root binding, not a caller-selected export name.
    pub fn kernel_binding(&self) -> &[u8; 32] {
        &self.original().root.binding
    }
    /// Captured proposed explicit extent, not final code-object metadata.
    pub fn explicit_argument_bytes(&self) -> u32 {
        self.original().root.explicit_argument_bytes
    }
    /// Captured proposed physical root alignment.
    pub fn kernarg_alignment_bytes(&self) -> u32 {
        self.original().root.kernarg_alignment_bytes
    }
}

impl ProductionKernelInlineArgumentAbiV29<'_> {
    /// Byte offset in the captured explicit kernarg proposal, not a pointer offset.
    pub fn offset(&self) -> u32 {
        self.offset
    }
    /// Original object extent, including uninterpreted padding/inactive payload.
    pub fn size_bytes(&self) -> u64 {
        self.size
    }
    /// Original object's required source alignment.
    pub fn alignment_bytes(&self) -> u64 {
        self.alignment
    }
}

fn by_value_source_abi_v29<'a>(
    semantic: &'a fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    function: &'a SemanticFunctionDeclV1,
    ordinal: usize,
    ty: SemanticTypeIdV1,
) -> Result<(&'a SemanticTypeDeclV1, &'a SemanticAbiArgumentV1), ProductionSemanticKirErrorV1> {
    let declaration = semantic
        .types()
        .get(ty.index() as usize)
        .ok_or_else(mismatch)?;
    let abi = function
        .abi()
        .adjusted_arguments()
        .get(ordinal)
        .ok_or_else(mismatch)?;
    if function.role() != SemanticFunctionRoleV1::KernelRoot
        || function.kernel_entry().is_none()
        || function.abi().canon_abi() != SemanticCanonAbiV1::GpuKernel
        || function.abi().extern_abi()
            != fe2o3_mir_model::semantic_mir_v1::SemanticExternAbiV1::GpuKernel
        || function.abi().source_input_types().get(ordinal) != Some(&ty)
        || function.abi().source_argument_ownership().get(ordinal)
            != Some(&SemanticSourceArgumentOwnershipV1::ByValue)
        || abi.role() != SemanticAbiArgumentRoleV1::Source
        || abi.ty() != ty
        || abi.value().adjusted().is_some()
        || abi.value().pointee_override().is_some()
        || declaration.layout().is_uninhabited()
    {
        return Err(mismatch());
    }
    let size = declaration.layout().size_bytes().ok_or_else(mismatch)?;
    let ignored = match abi.mode() {
        SemanticAbiPassModeV1::Ignore => true,
        SemanticAbiPassModeV1::Direct(_)
        | SemanticAbiPassModeV1::Pair { .. }
        | SemanticAbiPassModeV1::Cast { .. }
        | SemanticAbiPassModeV1::Indirect { .. } => false,
    };
    if ignored != (size == 0) {
        return Err(mismatch());
    }
    Ok((declaration, abi))
}

impl CapturedKernelArgumentAbiV18 {
    pub(super) fn by_value_argument_v29<'a>(
        &'a self,
        owner: &'a ProductionSemanticSsaOwnerV1,
        root_ordinal: usize,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<ProductionKernelByValueAbiV29<'a>>, ProductionSemanticKirErrorV1> {
        budget.charge_work(32)?;
        let semantic = owner.source_semantic();
        let root = self.roots.get(root_ordinal).ok_or_else(mismatch)?;
        if self.source != *owner.source_semantic_sha256()
            || self.ssa != owner.identity()
            || self.target != semantic.target()
            || self.layout != semantic.target_layout_identity()
            || self.roots.len() != semantic.roots().len()
            || semantic.roots().get(root_ordinal) != Some(&root.function)
        {
            return Err(mismatch());
        }
        let function = semantic
            .functions()
            .get(root.function.index() as usize)
            .ok_or_else(mismatch)?;
        let entry = function.kernel_entry().ok_or_else(mismatch)?;
        let count = root
            .arguments
            .end
            .checked_sub(root.arguments.start)
            .ok_or_else(mismatch)?;
        let index = root
            .arguments
            .start
            .checked_add(ordinal)
            .ok_or_else(mismatch)?;
        if root.binding != *entry.kernel_binding_identity().as_bytes()
            || count != function.abi().source_input_types().len()
            || ordinal >= count
            || root.arguments.end > self.arguments.len()
            || !root.kernarg_alignment_bytes.is_power_of_two()
        {
            return Err(mismatch());
        }
        let row = self.arguments.get(index).ok_or_else(mismatch)?;
        if function.abi().source_input_types().get(ordinal) != Some(&row.ty)
            || function.abi().source_argument_ownership().get(ordinal) != Some(&row.ownership)
            || semantic
                .types()
                .get(row.ty.index() as usize)
                .map(SemanticTypeDeclV1::identity)
                != Some(row.identity)
        {
            return Err(mismatch());
        }
        let Kind::CompilerLaidOutByValue { offset } = row.kind else {
            return Ok(None);
        };
        let (declaration, abi) = by_value_source_abi_v29(semantic, function, ordinal, row.ty)?;
        if row.access != DescriptorAccess::ByValue
            || row.alias != AliasSemantics::Value
            || row.components != [None, None]
        {
            return Err(mismatch());
        }
        let previous = if ordinal == 0 {
            0
        } else {
            let prior = self.arguments.get(index - 1).ok_or_else(mismatch)?;
            let offset = match prior.kind {
                Kind::CompilerLaidOutByValue { offset } => offset,
                Kind::Descriptor(_) => prior.components[0].ok_or_else(mismatch)?.offset,
            };
            let size = semantic
                .types()
                .get(prior.ty.index() as usize)
                .ok_or_else(mismatch)?
                .layout()
                .size_bytes()
                .ok_or_else(mismatch)?;
            u64::from(offset).checked_add(size).ok_or_else(mismatch)?
        };
        packed_argument_end(
            semantic,
            function,
            ordinal,
            row,
            previous,
            root.explicit_argument_bytes,
            root.kernarg_alignment_bytes,
            budget,
        )?;
        let original = OriginalByValueAbiV29 {
            profile: self,
            root,
            root_ordinal,
            argument: row,
            argument_ordinal: ordinal,
            declaration,
            abi,
        };
        if matches!(abi.mode(), SemanticAbiPassModeV1::Ignore) {
            Ok(Some(ProductionKernelByValueAbiV29::Ignored(
                ProductionKernelIgnoredArgumentAbiV29 { original },
            )))
        } else {
            Ok(Some(ProductionKernelByValueAbiV29::Inline(
                ProductionKernelInlineArgumentAbiV29 {
                    original,
                    offset,
                    size: declaration.layout().size_bytes().ok_or_else(mismatch)?,
                    alignment: declaration.layout().alignment_bytes(),
                },
            )))
        }
    }
}

impl<'a> SourceDescriptorRootAbiV29<'a> {
    pub(super) fn by_value_argument_v29(
        self,
        instances: &'a ExecutionInstancesV29<'_>,
        argument: u32,
        local: SemanticLocalIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<ProductionKernelByValueAbiV29<'a>>, ProductionSemanticKirErrorV1> {
        budget.charge_work(8)?;
        let root = self
            .profile
            .roots
            .get(self.original_root)
            .ok_or_else(mismatch)?;
        let instance = instances.instance(instances.root()).ok_or_else(mismatch)?;
        if instance.function() != root.function {
            return Err(mismatch());
        }
        let function = instances
            .owner()
            .source_semantic()
            .functions()
            .get(root.function.index() as usize)
            .ok_or_else(mismatch)?;
        let declaration = function
            .locals()
            .get(local.index() as usize)
            .ok_or_else(mismatch)?;
        let ordinal = usize::try_from(argument).map_err(|_| mismatch())?;
        if declaration.role() != SemanticLocalRoleV1::Argument(argument)
            || function.abi().source_input_types().get(ordinal) != Some(&declaration.ty())
        {
            return Err(mismatch());
        }
        self.profile
            .by_value_argument_v29(instances.owner(), self.original_root, ordinal, budget)
    }
}
