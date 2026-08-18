use dialect_gpu::{AddressSpaceAttr, MemorySpaceOp};
use dialect_kernel::{AlgorithmOp, IterationDomainAttr};
use fe2o3_lower_kernel_gpu::{
    ConfigError, KernelGpuLoweringPass, LoweringConfig, LoweringError, MAX_LOGICAL_REGIONS,
    MAX_REWRITES, MAX_WORKGROUP_AXIS, PASS_REGISTRATION_MARKER_KEY, PassRegistrationError,
    PostconditionError, SynchronizationMode, WorkgroupShape, register_pass,
};
use pliron::{context::Context, identifier::Identifier, op::Op};

fn config(
    rank: &[u32],
    regions: u16,
    spaces: &[AddressSpaceAttr],
    synchronization: SynchronizationMode,
    rewrites: u16,
) -> Result<LoweringConfig, ConfigError> {
    LoweringConfig::new(
        WorkgroupShape::new(rank)?,
        regions,
        spaces,
        synchronization,
        rewrites,
    )
}

#[test]
fn rejects_unbounded_and_incoherent_configuration() {
    assert_eq!(
        WorkgroupShape::new(&[]),
        Err(ConfigError::RankOutOfBounds(0))
    );
    assert_eq!(
        WorkgroupShape::new(&[1, 1, 1, 1]),
        Err(ConfigError::RankOutOfBounds(4))
    );
    assert_eq!(
        WorkgroupShape::new(&[0]),
        Err(ConfigError::WorkgroupAxisOutOfBounds { axis: 0, extent: 0 })
    );
    assert_eq!(
        WorkgroupShape::new(&[MAX_WORKGROUP_AXIS + 1]),
        Err(ConfigError::WorkgroupAxisOutOfBounds {
            axis: 0,
            extent: MAX_WORKGROUP_AXIS + 1,
        })
    );
    assert_eq!(
        WorkgroupShape::new(&[33, 33]),
        Err(ConfigError::WorkgroupTooLarge(1_089))
    );

    let shape = WorkgroupShape::new(&[64]).expect("valid shape");
    assert_eq!(
        LoweringConfig::new(
            shape,
            0,
            &[AddressSpaceAttr::Global],
            SynchronizationMode::None,
            8,
        ),
        Err(ConfigError::RegionCountOutOfBounds(0))
    );
    assert_eq!(
        LoweringConfig::new(
            shape,
            MAX_LOGICAL_REGIONS + 1,
            &[AddressSpaceAttr::Global],
            SynchronizationMode::None,
            8,
        ),
        Err(ConfigError::RegionCountOutOfBounds(MAX_LOGICAL_REGIONS + 1))
    );
    assert_eq!(
        LoweringConfig::new(shape, 1, &[], SynchronizationMode::None, 8),
        Err(ConfigError::MemorySpaceCountOutOfBounds(0))
    );
    assert_eq!(
        LoweringConfig::new(
            shape,
            1,
            &[AddressSpaceAttr::Global, AddressSpaceAttr::Global],
            SynchronizationMode::None,
            8,
        ),
        Err(ConfigError::DuplicateMemorySpace(AddressSpaceAttr::Global))
    );
    assert_eq!(
        LoweringConfig::new(
            shape,
            1,
            &[AddressSpaceAttr::Global],
            SynchronizationMode::WorkgroupBarrier,
            8,
        ),
        Err(ConfigError::WorkgroupBarrierWithoutWorkgroupMemory)
    );
    assert_eq!(
        LoweringConfig::new(
            shape,
            1,
            &[AddressSpaceAttr::Global],
            SynchronizationMode::None,
            0,
        ),
        Err(ConfigError::RewriteLimitOutOfBounds(0))
    );
    assert_eq!(
        LoweringConfig::new(
            shape,
            1,
            &[AddressSpaceAttr::Global],
            SynchronizationMode::None,
            MAX_REWRITES + 1,
        ),
        Err(ConfigError::RewriteLimitOutOfBounds(MAX_REWRITES + 1))
    );
}

#[test]
fn rejects_missing_or_poisoned_registration() {
    let bounded = config(
        &[64],
        1,
        &[AddressSpaceAttr::Global],
        SynchronizationMode::None,
        8,
    )
    .expect("valid config");
    let mut context = Context::new();
    let source = AlgorithmOp::new(&mut context, 1).expect("valid source");
    let mut pass = KernelGpuLoweringPass::new(bounded.clone());
    assert_eq!(
        pass.run_checked(source.get_operation(), &mut context),
        Err(LoweringError::PassNotRegistered)
    );

    let mut poisoned = Context::new();
    let marker_key: Identifier = PASS_REGISTRATION_MARKER_KEY
        .try_into()
        .expect("valid marker key");
    let marker = poisoned.aux_data.insert(Box::new(7_u32));
    poisoned.aux_data_map.insert(marker_key, marker);
    assert_eq!(
        register_pass(&mut poisoned),
        Err(PassRegistrationError::MarkerCollision)
    );

    let poisoned_source = AlgorithmOp::new(&mut poisoned, 1).expect("valid source");
    let mut poisoned_pass = KernelGpuLoweringPass::new(bounded);
    assert_eq!(
        poisoned_pass.run_checked(poisoned_source.get_operation(), &mut poisoned),
        Err(LoweringError::RegistrationCorrupt)
    );
}

#[test]
fn unsupported_inputs_fail_terminally_without_a_result() {
    let mut context = Context::new();
    register_pass(&mut context).expect("registration succeeds");

    let rank_one = config(
        &[64],
        1,
        &[AddressSpaceAttr::Global],
        SynchronizationMode::None,
        8,
    )
    .expect("valid config");
    let foreign = MemorySpaceOp::new(&mut context, AddressSpaceAttr::Global);
    let mut pass = KernelGpuLoweringPass::new(rank_one.clone());
    assert_eq!(
        pass.run_checked(foreign.get_operation(), &mut context),
        Err(LoweringError::UnsupportedSourceOperation)
    );
    assert!(pass.last_result().is_none());

    let rank_four = AlgorithmOp::new(&mut context, 4).expect("kernel rank is valid");
    assert_eq!(
        pass.run_checked(rank_four.get_operation(), &mut context),
        Err(LoweringError::UnsupportedSourceRank(4))
    );
    assert!(pass.last_result().is_none());

    let rank_two = AlgorithmOp::new(&mut context, 2).expect("valid source");
    assert_eq!(
        pass.run_checked(rank_two.get_operation(), &mut context),
        Err(LoweringError::RankMismatch {
            source: 2,
            workgroup: 1,
        })
    );
    assert!(pass.last_result().is_none());

    let regions = config(
        &[64],
        2,
        &[AddressSpaceAttr::Global],
        SynchronizationMode::None,
        8,
    )
    .expect("bounded but unsupported config");
    let rank_one_source = AlgorithmOp::new(&mut context, 1).expect("valid source");
    let mut pass = KernelGpuLoweringPass::new(regions);
    assert_eq!(
        pass.run_checked(rank_one_source.get_operation(), &mut context),
        Err(LoweringError::UnsupportedRegionCount(2))
    );
    assert!(pass.last_result().is_none());
}

#[test]
fn rejects_rewrite_overflow_and_malformed_source() {
    let mut context = Context::new();
    register_pass(&mut context).expect("registration succeeds");
    let source = AlgorithmOp::new(&mut context, 1).expect("valid source");

    let too_small = config(
        &[64],
        1,
        &[AddressSpaceAttr::Workgroup],
        SynchronizationMode::WorkgroupBarrier,
        4,
    )
    .expect("valid bounded config");
    let mut pass = KernelGpuLoweringPass::new(too_small);
    assert_eq!(
        pass.run_checked(source.get_operation(), &mut context),
        Err(LoweringError::RewriteLimitExceeded {
            required: 5,
            limit: 4,
        })
    );

    source.set_iteration_domain(&context, IterationDomainAttr::new(2).expect("valid domain"));
    assert_eq!(
        pass.run_checked(source.get_operation(), &mut context),
        Err(LoweringError::SourceVerificationFailed)
    );
    assert!(pass.last_result().is_none());
}

#[test]
fn postcondition_validator_detects_mutated_gpu_ir() {
    let mut context = Context::new();
    register_pass(&mut context).expect("registration succeeds");
    let source = AlgorithmOp::new(&mut context, 1).expect("valid source");
    let bounded = config(
        &[64],
        1,
        &[AddressSpaceAttr::Global],
        SynchronizationMode::None,
        8,
    )
    .expect("valid config");
    let mut pass = KernelGpuLoweringPass::new(bounded);
    pass.run_checked(source.get_operation(), &mut context)
        .expect("lowering succeeds");
    let result = pass.take_result().expect("result exists");

    result.operations()[0]
        .deref_mut(&context)
        .attributes
        .0
        .clear();
    assert_eq!(
        result.validate(&context),
        Err(PostconditionError::InvalidGpuOperation { index: 0 })
    );
}

#[test]
fn postcondition_validator_rejects_a_foreign_context_before_dereferencing() {
    let mut owner = Context::new();
    register_pass(&mut owner).expect("owner registration succeeds");
    let source = AlgorithmOp::new(&mut owner, 1).expect("valid source");
    let bounded = config(
        &[64],
        1,
        &[AddressSpaceAttr::Global],
        SynchronizationMode::None,
        8,
    )
    .expect("valid config");
    let mut pass = KernelGpuLoweringPass::new(bounded.clone());
    pass.run_checked(source.get_operation(), &mut owner)
        .expect("lowering succeeds");
    let result = pass.take_result().expect("result exists");

    let mut foreign = Context::new();
    register_pass(&mut foreign).expect("foreign registration succeeds");
    let foreign_source = AlgorithmOp::new(&mut foreign, 1).expect("valid foreign source");
    KernelGpuLoweringPass::new(bounded)
        .run_checked(foreign_source.get_operation(), &mut foreign)
        .expect("foreign lowering populates comparable arena slots");

    assert_eq!(
        result.validate(&foreign),
        Err(PostconditionError::ContextMismatch)
    );
}
