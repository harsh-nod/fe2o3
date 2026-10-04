use crate::{ProductionConditionalOwnershipSiteV1, ProductionRankedValueV1};
use dialect_gpu::{ExecutionDomainAttr, ExecutionLayoutOp};
use dialect_kernel::{
    DYNAMIC_EXTENT, IndexType, MemorySpaceAttr, OwnershipCoverageAttr, OwnershipPartitionAttr,
};
use fe2o3_kernel_ir::ConditionalTotalViewAddressDomainV1 as Domain;
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, types::FunctionType},
    dialect::DialectName,
    r#type::TypeHandle,
};

struct Fixture {
    context: Context,
    function: FuncOp,
    reads: Vec<LiveReadBoundV1>,
    occurrences: [OccurrenceV1; 1],
    census: Census,
    epoch: u64,
}

fn hard() -> Limits {
    Limits::production_hard_ceiling()
}

fn fixture(global: bool, count: usize) -> Fixture {
    fixture_with_extent(global, count, false)
}

fn fixture_with_extent(global: bool, count: usize, shared_extent: bool) -> Fixture {
    let mut context = Context::new();
    dialect_kernel::register_dialect(
        &mut context,
        &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
    )
    .unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    let index: TypeHandle = IndexType::get(&context).into();
    let signature = FunctionType::get(&context, vec![index; count + 1], vec![]);
    let function = FuncOp::new(
        &mut context,
        "conditional_bounds".try_into().unwrap(),
        signature,
    );
    let entry = function.get_entry_block(&context);
    let body = BasicBlock::new(&mut context, Some("body".try_into().unwrap()), vec![]);
    let exit = BasicBlock::new(&mut context, Some("exit".try_into().unwrap()), vec![]);
    body.insert_at_back(function.get_region(&context), &context);
    exit.insert_at_back(function.get_region(&context), &context);
    ExecutionLayoutOp::new_with_domain(
        &mut context,
        17,
        [0, 1, 1],
        [2, 1, 1],
        2,
        ExecutionDomainAttr::FullPhysicalWorkgroups,
    )
    .get_operation()
    .insert_at_back(entry, &context);
    let invocation = InvocationIndexOp::new(&mut context, 0, 0);
    invocation.get_operation().insert_at_back(entry, &context);
    let point = invocation.result(&context);
    let mut views = Vec::new();
    for argument in 0..=count {
        let extent = entry
            .deref(&context)
            .get_argument(if shared_extent { 0 } else { argument });
        let ty = RankedViewType::new(&context, 32, argument == 0, vec![DYNAMIC_EXTENT]).unwrap();
        let view = RankedViewOp::new_in_space_with_allocation_contract(
            &mut context,
            ty,
            vec![extent],
            MemorySpaceAttr::Global,
            argument as u64 + 1,
            argument as u64 + 1,
        )
        .unwrap();
        view.get_operation().insert_at_back(entry, &context);
        views.push(view.result(&context));
    }
    let ownership = OwnershipContractOp::new(
        &mut context,
        views[0],
        OwnershipCoverageAttr::TotalView,
        OwnershipPartitionAttr::ExactSets,
    )
    .unwrap();
    ownership.get_operation().insert_at_back(entry, &context);
    let mut reads = Vec::new();
    for argument in 1..=count {
        let read = RankedAccessOp::new(
            &mut context,
            AccessKindAttr::Read,
            views[argument],
            vec![point],
        )
        .unwrap();
        read.get_operation()
            .insert_at_back(if global { entry } else { body }, &context);
        reads.push(LiveReadBoundV1 {
            operation: read.get_operation(),
            view: views[argument],
            index: point,
            extent: entry
                .deref(&context)
                .get_argument(if shared_extent { 0 } else { argument }),
            domain: if global {
                Domain::GlobalLaunch
            } else {
                Domain::GuardedOutput
            },
        });
    }
    let output_extent = entry.deref(&context).get_argument(0);
    IndexLessThanBranchOp::new(&mut context, point, output_extent, body, exit)
        .get_operation()
        .insert_at_back(entry, &context);
    RankedAccessOp::new(&mut context, AccessKindAttr::Write, views[0], vec![point])
        .unwrap()
        .get_operation()
        .insert_at_back(body, &context);
    BranchOp::new(&mut context, exit)
        .get_operation()
        .insert_at_back(body, &context);
    ReturnOp::new(&mut context)
        .get_operation()
        .insert_at_back(exit, &context);
    verify_operation(function.get_operation(), &context).unwrap();
    let occurrence = (
        ProductionConditionalOwnershipSiteV1 {
            block: 0,
            operation: (count + 3) as u32,
            view: ProductionRankedValueV1::Local(crate::ProductionRankedValueIdV1::new(1)),
        },
        ownership.get_operation(),
        views[0],
    );
    let mut fixture = Fixture {
        context,
        function,
        reads,
        occurrences: [occurrence],
        census: Census::default(),
        epoch: 0,
    };
    fixture.refresh();
    fixture
}

impl Fixture {
    fn refresh(&mut self) {
        use crate::production_analysis::{
            LivePlironStructuralIdentityProviderV1, PlironStructuralIdentityProviderV1,
        };
        self.census = LivePlironStructuralIdentityProviderV1::new(&self.context, &self.function)
            .capture_with_resource_limits_v1(hard())
            .ok()
            .unwrap()
            .input_census;
        self.epoch = current_epoch(&self.context).unwrap();
    }

    fn manager(&self, limits: Limits) -> Manager {
        let mut manager = Manager::new_with_resource_contract(
            &self.function,
            self.census,
            Bound::default(),
            0,
            limits,
        )
        .unwrap();
        manager.prepare_function_inventory(&self.context, &self.function);
        manager
    }

    fn input<'a>(&'a self, reads: Option<&'a [LiveReadBoundV1]>) -> InputV1<'a> {
        InputV1 {
            context: &self.context,
            function: &self.function,
            census: self.census,
            epoch: self.epoch,
            reads,
            occurrences: &self.occurrences,
        }
    }

    fn run(&self, reads: Option<&[LiveReadBoundV1]>) -> Result<ReportV1, ErrorV1> {
        let mut manager = self.manager(hard());
        let bound = preflight_v1(
            self.census,
            reads.map_or(0, <[LiveReadBoundV1]>::len),
            hard(),
        )
        .unwrap();
        manager
            .admit_retained_resource_upper_bound(Phase::MemoryBounds, bound)
            .unwrap();
        run_observed(self.input(reads), &mut manager, (None, None))
    }

    fn ordinary(&self) -> RankedBoundsReportV1 {
        run_pliron_ranked_bounds_check_v1(&self.context, &self.function)
    }

    fn insert_terminal_read(&mut self, overflow: bool) {
        let entry = self.function.get_entry_block(&self.context);
        let before = entry
            .deref(&self.context)
            .get_terminator(&self.context)
            .unwrap();
        let ty = RankedViewType::new(&self.context, 32, false, vec![4]).unwrap();
        let view = RankedViewOp::new(&mut self.context, ty, vec![]).unwrap();
        view.get_operation().insert_before(&self.context, before);
        let literal = IndexConstantOp::new(&mut self.context, if overflow { u64::MAX } else { 4 });
        literal.get_operation().insert_before(&self.context, before);
        let mut index = literal.result(&self.context);
        if overflow {
            let one = IndexConstantOp::new(&mut self.context, 1);
            one.get_operation().insert_before(&self.context, before);
            let one = one.result(&self.context);
            let add = IndexBinaryOp::new(&mut self.context, IndexBinaryKindAttr::Add, index, one);
            add.get_operation().insert_before(&self.context, before);
            index = add.result(&self.context);
        }
        let view = view.result(&self.context);
        RankedAccessOp::new(&mut self.context, AccessKindAttr::Read, view, vec![index])
            .unwrap()
            .get_operation()
            .insert_before(&self.context, before);
        self.refresh();
    }
}
