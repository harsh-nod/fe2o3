/// Inert declaring-frame provenance for one exact original canonical Alloca.
/// This does not identify a dynamic invocation or prove lifetime or byte values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSourceAllocationFrameV32 {
    root: usize,
    instance: usize,
    function: SemanticFunctionIdV1,
    local: u32,
    source_generation: Option<u32>,
    semantic_type: SemanticTypeIdV1,
    allocation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    bytes: u64,
    alignment: u32,
    layout: Option<fe2o3_kernel_ir::StorageLayoutIdV1>,
}

impl ProductionSourceAllocationFrameV32 {
    /// Exact original root ordinal in the source owner.
    pub const fn root(self) -> usize {
        self.root
    }
    /// Static source-call instance within `root`, never a dynamic invocation ID.
    pub const fn instance(self) -> usize {
        self.instance
    }
    /// Original MIR declaration owning the retained local.
    pub const fn function(self) -> SemanticFunctionIdV1 {
        self.function
    }
    /// Original declaration-local ordinal, qualified by root and instance.
    pub const fn local(self) -> u32 {
        self.local
    }
    /// Static object-cell generation. Dynamic allocation/frame generations
    /// remain a separate relation; this value must never substitute for them.
    pub const fn source_generation(self) -> Option<u32> {
        self.source_generation
    }
    /// Original semantic type of the whole retained local.
    pub const fn semantic_type(self) -> SemanticTypeIdV1 {
        self.semantic_type
    }
    /// Exact original canonical Alloca after source attachment relocation.
    pub const fn allocation(self) -> fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
        self.allocation
    }
    /// Checked physical extent of this retained allocation.
    pub const fn bytes(self) -> u64 {
        self.bytes
    }
    /// Checked physical base alignment of this retained allocation.
    pub const fn alignment(self) -> u32 {
        self.alignment
    }
    /// Original owned layout, when the retained source representation has one.
    /// Absence does not authorize reconstructing a layout from byte width.
    pub const fn layout(self) -> Option<fe2o3_kernel_ir::StorageLayoutIdV1> {
        self.layout
    }
}

fn source_allocation_frame_headers_v32<F>() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a, 's, 'w> = (
        &'a ProductionSourceCorrespondenceV18<'s>,
        &'a mut ArgumentBudgetV1<'w>,
        &'a ScopedModuleRootV29,
        SourceOwnedResultV18<&'a ScopedModuleRootV29>,
        &'a ScopedSourceSlotV29,
        &'a ScopedSourceSlotInstanceV29,
        Option<&'a ScopedSourceSlotInstanceV29>,
        &'a AdmittedInertSemanticMirV1,
        SourceOwnedResultV18<&'a AdmittedInertSemanticMirV1>,
        &'a SemanticFunctionDeclV1,
        Option<&'a SemanticFunctionDeclV1>,
        &'a fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1,
        Option<&'a fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>,
        SourcePhysicalBackingV18<'a>,
        SourceOwnedResultV18<SourcePhysicalBackingV18<'a>>,
        ProductionSourceAllocationFrameV32,
        SourceOwnedResultV18<ProductionSourceAllocationFrameV32>,
        fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        SourceOwnedResultV18<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>,
        (SemanticFunctionIdV1, Option<(usize, SemanticBlockIdV1)>),
        SourceOwnedResultV18<(SemanticFunctionIdV1, Option<(usize, SemanticBlockIdV1)>)>,
        Option<usize>,
        SourceOwnedResultV18<Option<usize>>,
        Option<u32>,
        Option<fe2o3_kernel_ir::StorageLayoutIdV1>,
        [SourceOwnedResultV18<()>; 3],
        Result<(), ArgumentResourceV1>,
        [usize; 8],
        [u32; 3],
        u64,
        std::ops::Range<usize>,
    );
    argument_sum_v1(&[
        size_of::<Frame<'_, '_, '_>>(),
        std::mem::align_of::<Frame<'_, '_, '_>>(),
        size_of::<SourceCallbackCustodyV29<F>>(),
        std::mem::align_of::<SourceCallbackCustodyV29<F>>(),
        size_of::<SourceOwnedResultV18<((usize, usize), SourceCallbackCustodyV29<F>)>>(),
        size_of::<std::thread::Result<SourceOwnedResultV18<()>>>(),
        size_of::<std::panic::AssertUnwindSafe<&mut SourceCallbackCustodyV29<F>>>(),
        6 * size_of::<&()>(),
        4 * size_of::<usize>(),
    ])
}

impl ProductionSourceCorrespondenceV18<'_> {
    fn allocation_frame_v32(
        &self,
        root: usize,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceAllocationFrameV32> {
        self.retain_query((|| {
            self.query(budget)?;
            budget.charge_work(9)?;
            let operation =
                scoped_raw_admission_v29::source_slot_input_v18(self, root, ordinal, budget)?;
            let backing =
                self.retained_allocation_for_slot_v18(root, ordinal, operation, budget)?;
            let owner = self.source.root_row(root)?;
            let active = self
                .source
                .active_ordinal(root, backing.instance, budget)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "allocation frame inactive source instance",
                ))?;
            let frame = owner.source_slots.instances.get(active).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("allocation frame instance census"),
            )?;
            let (function, incoming) = self.source.instance(root, backing.instance, budget)?;
            if frame.instance != backing.slot.instance
                || frame.function != function
                || frame.incoming.map(|call| (call.caller.index(), call.block)) != incoming
                || !frame.slots.contains(&ordinal)
                || backing.row != ordinal
            {
                return self
                    .source
                    .missing("allocation frame declaration or instance differs");
            }
            let local = backing.slot.origin.identity.original_local().ok_or(
                ProductionSourceOwnedViewErrorV18::Binding(
                    "allocation frame original local absent",
                ),
            )?;
            let semantic = self.source.source_semantic(budget)?;
            let declaration = semantic.functions().get(function.index() as usize).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("allocation frame source function"),
            )?;
            let original = declaration.locals().get(local as usize).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("allocation frame source local"),
            )?;
            if original.ty() != backing.slot.origin.semantic_type {
                return self
                    .source
                    .missing("allocation frame original type differs");
            }
            let source_generation = match backing.slot.origin.identity {
                ScopedAllocationIdentityV29::LegacyLocal(_) => None,
                ScopedAllocationIdentityV29::OriginalObject { generation, .. } => Some(generation),
                _ => {
                    return self
                        .source
                        .missing("allocation frame unsupported source identity");
                }
            };
            let layout = match backing.slot.origin.source {
                ScopedAllocationSourceV29::Legacy => None,
                ScopedAllocationSourceV29::OriginalArray { schema }
                | ScopedAllocationSourceV29::OriginalObject { schema, .. } => Some(schema),
            };
            let (bytes, alignment) = match backing.slot.representation {
                ScopedSlotRepresentationV29::ScalarArray(value) => {
                    (value.bytes, value.element.alignment)
                }
                ScopedSlotRepresentationV29::Object {
                    bytes, alignment, ..
                } => (bytes, alignment),
            };
            Ok(ProductionSourceAllocationFrameV32 {
                root,
                instance: backing.instance,
                function,
                local,
                source_generation,
                semantic_type: original.ty(),
                allocation: operation,
                bytes,
                alignment,
                layout,
            })
        })())
    }

    /// Visits every retained original allocation of this exact root once.
    /// Consumers must index this complete census once and separately join every
    /// actual memory event. No dynamic lifetime or memory authority is issued.
    /// A callback may retain its own paid storage. Only this visitor's known
    /// fixed header is refunded; the containing source scope owns other credit.
    /// Physical allocation hoisting does not establish logical frame activation.
    pub fn visit_allocation_frames_v32<F>(
        &self,
        root: usize,
        budget: &mut ArgumentBudgetV1<'_>,
        visit: F,
    ) -> SourceOwnedResultV18<()>
    where
        F: FnMut(
            ProductionSourceAllocationFrameV32,
            &mut ArgumentBudgetV1<'_>,
        ) -> SourceOwnedResultV18<()>,
    {
        let visit = SourceCallbackCustodyV29::new(visit);
        self.query(budget)?;
        let floor = budget.storage();
        let ((count, retained), mut visit) =
            scoped_source_attempt_v29(self.source.cleanup, budget, floor, |budget| {
                visit.prepare(|| {
                    self.retain_query((|| {
                        let retained = argument_sum_v1(&[
                            source_allocation_frame_headers_v32::<F>()?,
                            source_callback_custody_finish_preflight_v29::<
                                (),
                                ProductionSourceOwnedViewErrorV18,
                            >(budget)?,
                            source_owned_finish_preflight_v26::<
                                (),
                                ProductionSourceOwnedViewErrorV18,
                            >(budget)?,
                        ])?;
                        budget.reserve_storage(retained)?;
                        budget.charge_work(2)?;
                        let count = self.source.root_row(root)?.source_slots.slots.len();
                        Ok((count, retained))
                    })())
                })
            })?;
        let retained_floor = budget.storage();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for ordinal in 0..count {
                budget.charge_work(2)?;
                let frame = self.allocation_frame_v32(root, ordinal, budget)?;
                let callback_floor = budget.storage();
                let result = visit
                    .as_mut()
                    .expect("allocation visitor remains in custody")(
                    frame, budget
                );
                let custody = self.observe_custody(budget);
                if budget.storage() < callback_floor || budget.storage() < retained_floor {
                    self.source.cleanup.deny_refund();
                    return self.retain_query(Err(ArgumentResourceV1::Accounting.into()));
                }
                custody?;
                self.retain_query(result)?;
                self.query(budget)?;
            }
            Ok(())
        }));
        let caught = visit.finish(caught);
        let prior = self.source.guard.first.get();
        let postflight = if budget.storage() < retained_floor {
            self.source.cleanup.deny_refund();
            Err(ArgumentResourceV1::Accounting.into())
        } else {
            self.observe_custody(budget)
        };
        source_owned_finish_callback_v18(
            caught,
            prior,
            postflight,
            self.source.cleanup,
            budget,
            retained,
        )
    }
}
