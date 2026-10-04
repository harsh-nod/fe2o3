//! Exact target reconstruction shared by closed descriptor and contract families.

macro_rules! mixed_target_selection_family {
    ($max_bytes:ident, $scratch:ident, $codec_error:ident, $input:ident, $row:ident, $encode:ident, $length:ident, $descriptor_scratch:ident, $descriptor_error:ident, $descriptor:ident, $decode:ident, $error:ident, $subject:ident, $contract_module:ident, $contract_error:ident, $contract:ident, $with_selection:ident, $check_selection:ident) => {
        use fe2o3_amd_target::ProductionAmdTargetProfileV1;
        use fe2o3_compiler_lineage::{
            TargetLineageIdentityV3, $codec_error as CodecError, $encode, $input as Input, $length,
            $max_bytes, $row as Row, $scratch as SCRATCH,
        };
        use fe2o3_kernel_descriptor::{
            MAX_KERNELS, $decode, $descriptor, $descriptor_error, $descriptor_scratch,
        };
        use fe2o3_kernel_ir::{
            CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
            CanonicalKernelIrVerificationResourceErrorV1 as Resource, FunctionRole,
            VerifiedCanonicalKernelIrModuleV18 as Owner,
        };
        use sha2::{Digest, Sha256};
        use std::{
            mem::{align_of, size_of},
            panic::AssertUnwindSafe,
        };

        /// Content reconstruction failure, never a proof or authorization decision.
        #[derive(Debug)]
        pub enum $error {
            /// Original sticky ledger refusal.
            Resource(Resource),
            /// A complete typed subject, descriptor, target or canonical wire differs.
            Binding(&'static str),
        }
        type Error = $error;
        impl From<Resource> for Error {
            fn from(value: Resource) -> Self {
                Self::Resource(value)
            }
        }
        impl std::fmt::Display for Error {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "mixed target selection: {self:?}")
            }
        }
        impl std::error::Error for Error {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                match self {
                    Self::Resource(error) => Some(error),
                    Self::Binding(_) => None,
                }
            }
        }

        /// Caller-retained exact subjects. This struct authenticates none of them.
        pub struct $subject<'a> {
            /// Actual admitted final V18 owner; target selection does not mutate it.
            pub owner: &'a Owner,
            /// Independently derived original invocation coordinates.
            pub invocation: TargetLineageIdentityV3,
            /// Exact semantic-MIR receipt, used both for domain-separated outer identity
            /// and raw canonical-MIR SHA256 in V26 subjects; not a semantic theorem.
            pub semantic_mir: &'a fe2o3_compiler_lineage::InertCanonicalSemanticMirReceiptV3,
            /// Complete family-specific descriptor, including every mandatory contract.
            pub descriptor: &'a [u8],
            /// Actual independently authenticated target profile.
            pub profile: ProductionAmdTargetProfileV1,
        }
        fn codec(e: CodecError<Resource>) -> Error {
            match e {
                CodecError::Charge(e) => e.into(),
                _ => Error::Binding("strict target selection schema"),
            }
        }
        fn descriptor(e: $descriptor_error<Resource>) -> Error {
            match e {
                $descriptor_error::Nominal(
                    fe2o3_kernel_descriptor::DescriptorWireErrorV3::Work(e),
                )
                | $descriptor_error::Contract(
                    fe2o3_kernel_descriptor::$contract_module::$contract_error::Resource(e),
                ) => e.into(),
                _ => Error::Binding("complete mixed descriptor"),
            }
        }
        fn frames<R, F>() -> Result<usize, Error> {
            [
                SCRATCH,
                $descriptor_scratch,
                size_of::<$descriptor<'_>>(),
                size_of::<[Row<'_>; MAX_KERNELS]>(),
                size_of::<[bool; MAX_KERNELS]>(),
                size_of::<Input<'_>>(),
                size_of::<Vec<u8>>(),
                size_of::<Sha256>(),
                2 * size_of::<[u8; 32]>(),
                2 * size_of::<[u32; 3]>(),
                size_of::<[u64; 3]>(),
                size_of::<fe2o3_kernel_ir::WorkgroupSize>(),
                size_of::<Option<fe2o3_kernel_ir::WorkgroupSize>>(),
                size_of::<fe2o3_kernel_descriptor::DimensionsV1>(),
                size_of::<fe2o3_kernel_descriptor::BlockSizeV1>(),
                size_of::<Option<u64>>(),
                size_of::<Result<$descriptor<'_>, $descriptor_error<Resource>>>(),
                size_of::<Result<usize, CodecError<Resource>>>(),
                size_of::<Result<(), CodecError<Resource>>>(),
                size_of::<
                    Result<
                        TargetLineageIdentityV3,
                        fe2o3_compiler_lineage::ProductionTargetLineageErrorV3,
                    >,
                >(),
                size_of::<Option<usize>>(),
                size_of::<fe2o3_kernel_descriptor::$contract_module::$contract<'_>>(),
                size_of::<
                    Result<
                        fe2o3_kernel_descriptor::$contract_module::$contract<'_>,
                        $descriptor_error<Resource>,
                    >,
                >(),
                size_of::<fe2o3_kernel_descriptor::KernelDescriptorRefV3<'_, '_>>(),
                size_of::<
                    Result<
                        fe2o3_kernel_descriptor::KernelDescriptorRefV3<'_, '_>,
                        $descriptor_error<Resource>,
                    >,
                >(),
                size_of::<(&$subject<'_>, F)>(),
                size_of::<AssertUnwindSafe<(&$subject<'_>, F)>>(),
                size_of::<F>(),
                align_of::<F>(),
                size_of::<R>(),
                size_of::<Result<R, Error>>(),
                size_of::<std::thread::Result<Result<R, Error>>>(),
                16 * size_of::<usize>(),
            ]
            .into_iter()
            .try_fold(0usize, |a, b| {
                a.checked_add(b).ok_or(Resource::Arithmetic.into())
            })
        }

        /// Reconstructs selection from the actual graph and complete descriptor, then
        /// lends canonical bytes only within the caller's original ledger scope.
        /// Caller prepays immutable graph/descriptor backing. The borrowed semantic
        /// preimage lives in upstream custody, so this helper explicitly reserves its
        /// complete extent for this scope. Generic lineage codecs invoked by `visit`
        /// retain their own documented bounded allocation domain.
        /// This is not LLVM replay, semantic refinement, compiler authentication,
        /// final-artifact currentness or publication/load/launch authorization.
        pub fn $with_selection<R, F>(
            subject: &$subject<'_>,
            budget: &mut Budget<'_>,
            visit: F,
        ) -> Result<R, Error>
        where
            F: FnOnce(&[u8], &mut Budget<'_>) -> Result<R, Error>,
        {
            budget.check_prior_denials_v1()?;
            let floor = subject
                .owner
                .canonical_bytes()
                .len()
                .checked_add(subject.descriptor.len())
                .ok_or(Resource::Arithmetic)?;
            let scratch = frames::<R, F>()?
                .checked_add(subject.semantic_mir.canonical_preimage().len())
                .ok_or(Resource::Arithmetic)?;
            let capture = (subject, visit);
            let operation = move |budget: &mut Budget<'_>| {
                let (subject, visit) = std::convert::identity(capture);
                let table = $decode(subject.descriptor, &mut |n| budget.charge_work(n))
                    .map_err(descriptor)?;
                let owner = subject.owner;
                let kernels = &owner.module().kernels;
                budget.charge_work(
                    subject
                        .semantic_mir
                        .canonical_preimage()
                        .len()
                        .checked_add(64)
                        .ok_or(Resource::Arithmetic)?,
                )?;
                let source_sha256: [u8; 32] =
                    Sha256::digest(subject.semantic_mir.canonical_preimage()).into();
                budget.charge_work(64 + MAX_KERNELS)?;
                let target = table.device_target().as_amd_target_id();
                if target.processor() != subject.profile.cpu()
                    || target.xnack() != Some(fe2o3_amd_target::FeatureState::Disabled)
                    || target.sramecc().is_some()
                    || table.kernel_count() != kernels.len()
                    || kernels.len() > MAX_KERNELS
                {
                    return Err(Error::Binding(
                        "exact graph/descriptor target and root count",
                    ));
                }
                let mut seen = [false; MAX_KERNELS];
                for i in 0..table.kernel_count() {
                    let contract = table
                        .contract(i, &mut |n| budget.charge_work(n))
                        .map_err(descriptor)?;
                    let nominal = table
                        .kernel(i, &mut |n| budget.charge_work(n))
                        .map_err(descriptor)?;
                    let s = contract.subjects();
                    budget.charge_work(65)?;
                    if &s.output_graph_identity != owner.identity().digest()
                        || s.source_semantic_identity != source_sha256
                    {
                        return Err(Error::Binding(
                            "descriptor original source and final V18 graph",
                        ));
                    }
                    let function = owner
                        .module()
                        .functions
                        .get(s.output_function as usize)
                        .ok_or(Error::Binding("descriptor output function"))?;
                    budget.charge_work(1)?;
                    if function.role != FunctionRole::KernelEntry {
                        return Err(Error::Binding("descriptor exact entry function"));
                    }
                    // The schema ceiling is 128 roots. Every comparison is charged;
                    // no graph-name scan or unchecked allocation escapes the ledger.
                    let mut found = None;
                    for (index, kernel) in kernels.iter().enumerate() {
                        budget.charge_work(
                            kernel.entry.as_str().len() + function.id.as_str().len() + 1,
                        )?;
                        if kernel.entry == function.id {
                            found = Some(index);
                            break;
                        }
                    }
                    let index =
                        found.ok_or(Error::Binding("descriptor function is a current root"))?;
                    budget.charge_work(
                        kernels[index].id.as_str().len() + nominal.entry_name().len(),
                    )?;
                    if kernels[index].id.as_str() != nominal.entry_name() {
                        return Err(Error::Binding("descriptor exact exported root symbol"));
                    }
                    let kernel = &kernels[index];
                    let group = kernel
                        .workgroup_size
                        .ok_or(Error::Binding("exact current root workgroup"))?;
                    let dimensions = [group.x, group.y, group.z];
                    budget.charge_work(32)?;
                    let flat = u64::from(group.x)
                        .checked_mul(u64::from(group.y))
                        .and_then(|n| n.checked_mul(u64::from(group.z)))
                        .ok_or(Error::Binding("workgroup extent overflow"))?;
                    let launch = nominal.launch();
                    let max_grid = launch.max_grid();
                    let block_matches = match launch.block_size() {
                        fe2o3_kernel_descriptor::BlockSizeV1::Any => true,
                        fe2o3_kernel_descriptor::BlockSizeV1::Exact(d) => {
                            dimensions == [d.x(), d.y(), d.z()]
                        }
                        fe2o3_kernel_descriptor::BlockSizeV1::AtMost(d) => dimensions
                            .into_iter()
                            .zip([d.x(), d.y(), d.z()])
                            .all(|(n, max)| n <= max),
                    };
                    if !block_matches
                        || dimensions.contains(&0)
                        || flat > u64::from(launch.max_flat_workgroup_size())
                        || s.source_rank != kernel.domain.rank()
                        || dimensions[usize::from(s.source_rank)..]
                            .iter()
                            .any(|n| *n != 1)
                    {
                        return Err(Error::Binding(
                            "current workgroup and descriptor launch constraints",
                        ));
                    }
                    // Nominal grid axes count workgroups. V26 retains the complete
                    // physical item envelope, never a clamp to source-static KIR.
                    for (axis, blocks) in [max_grid.x(), max_grid.y(), max_grid.z()]
                        .into_iter()
                        .enumerate()
                    {
                        let physical = u64::from(dimensions[axis]) * u64::from(blocks);
                        if s.exact_grid[axis] != physical {
                            return Err(Error::Binding(
                                "complete V26 physical invocation envelope",
                            ));
                        }
                    }
                    for (axis, extent) in kernel.domain.extents().enumerate() {
                        if let fe2o3_kernel_ir::LaunchExtent::Static(n) = extent {
                            if u64::from(n) > s.exact_grid[axis] {
                                return Err(Error::Binding(
                                    "static graph exceeds physical invocation envelope",
                                ));
                            }
                        }
                    }
                    if seen[index] {
                        return Err(Error::Binding("duplicate descriptor root"));
                    }
                    seen[index] = true;
                }
                budget.charge_work(MAX_KERNELS)?;
                let mut rows = [Row {
                    kernel: "",
                    workgroup: [0; 3],
                }; MAX_KERNELS];
                for (row, kernel) in rows.iter_mut().zip(kernels) {
                    budget.charge_work(4)?;
                    let group = kernel
                        .workgroup_size
                        .ok_or(Error::Binding("exact current root workgroup"))?;
                    *row = Row {
                        kernel: kernel.id.as_str(),
                        workgroup: [group.x, group.y, group.z],
                    };
                }
                budget.charge_work(subject.descriptor.len() + 64)?;
                let input = Input {
                    invocation: subject.invocation,
                    semantic_mir: TargetLineageIdentityV3::new(
                        *subject.semantic_mir.identity().sha256(),
                        subject.semantic_mir.identity().byte_len(),
                    )
                    .map_err(|_| Error::Binding("exact original semantic receipt"))?,
                    kernel_ir: TargetLineageIdentityV3::new(
                        *owner.identity().digest(),
                        owner.identity().canonical_length(),
                    )
                    .map_err(|_| Error::Binding("actual V18 identity"))?,
                    descriptor_sha256: Sha256::digest(subject.descriptor).into(),
                    profile: subject.profile,
                    workgroups: &rows[..kernels.len()],
                };
                let length = $length(input, SCRATCH, |n| budget.charge_work(n)).map_err(codec)?;
                budget.reserve_storage(length)?;
                let mut bytes = Vec::new();
                bytes
                    .try_reserve_exact(length)
                    .map_err(|_| Resource::Allocation)?;
                budget.reserve_storage(
                    bytes
                        .capacity()
                        .checked_sub(length)
                        .ok_or(Resource::Accounting)?,
                )?;
                budget.charge_work(length)?;
                bytes.resize(length, 0);
                $encode(input, &mut bytes, SCRATCH, |n| budget.charge_work(n)).map_err(codec)?;
                let result = visit(&bytes, budget);
                budget.check_prior_denials_v1()?;
                result
            };
            #[cfg(test)]
            {
                assert_eq!(
                    std::mem::size_of_val(&operation),
                    size_of::<(&$subject<'_>, F)>()
                );
                assert_eq!(
                    std::mem::align_of_val(&operation),
                    align_of::<(&$subject<'_>, F)>()
                );
            }
            budget.with_prepaid_scope(floor, 0, 0, scratch, operation)
        }

        /// Receiver-side exact reconstruction using the same admitted graph and
        /// descriptor recipe as publication. No schema or identity-only fallback.
        /// Caller additionally prepays the complete received immutable wire backing.
        pub fn $check_selection(
            subject: &$subject<'_>,
            bytes: &[u8],
            budget: &mut Budget<'_>,
        ) -> Result<(), Error> {
            budget.check_prior_denials_v1()?;
            if bytes.len() > $max_bytes {
                return Err(Error::Binding("target selection wire bound"));
            }
            let required = subject
                .owner
                .canonical_bytes()
                .len()
                .checked_add(subject.descriptor.len())
                .and_then(|n| n.checked_add(bytes.len()))
                .ok_or(Resource::Arithmetic)?;
            if budget.storage() < required {
                return Err(Resource::Accounting.into());
            }
            $with_selection(subject, budget, |expected, budget| {
                budget.charge_work(expected.len().max(bytes.len()))?;
                if bytes != expected {
                    return Err(Error::Binding("exact target selection bytes"));
                }
                Ok(())
            })
        }
    };
}
pub(crate) use mixed_target_selection_family;
