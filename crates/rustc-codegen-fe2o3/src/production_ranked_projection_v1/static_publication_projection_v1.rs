include!("static_publication_control_v1.rs");

#[derive(Clone, Debug)]
struct ProjectedStaticPublicationV1 {
    source: StaticPublicationSourceV1,
    blocks: Vec<Option<ProjectedStaticPublicationEffectV1>>,
    controls: Vec<Option<ProjectedCfgTerminatorV1>>,
}

fn projected_item_operation_count_v1(item: &ProjectedBlockItemV1) -> usize {
    match item {
        ProjectedBlockItemV1::StaticPublication(effect) => effect.operation_count(),
        _ => 1,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectedStaticPublicationEffectV1 {
    role: StaticPublicationRoleV1,
    payload: ProductionRankedValueIdV1,
    flags: ProductionRankedValueIdV1,
    index: ProductionRankedValueV1,
    extent: ProductionRankedValueV1,
    read_results: Option<[ProductionRankedValueIdV1; 3]>,
}

impl ProjectedStaticPublicationEffectV1 {
    const fn operation_count(&self) -> usize {
        match self.role {
            StaticPublicationRoleV1::Producer => 2,
            StaticPublicationRoleV1::Consumer => 4,
        }
    }

    fn prepare_values(&mut self, next: &mut u32) -> Result<(), ProductionRankedProjectionErrorV1> {
        if self.read_results.is_some() {
            return Err(static_publication_reject_v1());
        }
        if self.role == StaticPublicationRoleV1::Consumer {
            self.read_results = Some([
                next_value_id(next)?,
                next_value_id(next)?,
                next_value_id(next)?,
            ]);
        }
        Ok(())
    }

    fn materialize(
        self,
    ) -> Result<Vec<ProductionRankedOperationV1>, ProductionRankedProjectionErrorV1> {
        let payload = ProductionRankedValueV1::Local(self.payload);
        let flags = ProductionRankedValueV1::Local(self.flags);
        let index = self.index;
        match self.role {
            StaticPublicationRoleV1::Producer if self.read_results.is_none() => Ok(vec![
                ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Write,
                    view: payload,
                    indices: vec![index],
                },
                ProductionRankedOperationV1::PublicationAtomicStoreU32 {
                    view: flags,
                    index,
                    value: 2,
                },
            ]),
            StaticPublicationRoleV1::Consumer => {
                let [acquired, result, success] =
                    self.read_results.ok_or_else(static_publication_reject_v1)?;
                Ok(vec![
                    ProductionRankedOperationV1::PublicationAtomicStoreU32 {
                        view: flags,
                        index,
                        value: 1,
                    },
                    ProductionRankedOperationV1::PublicationAtomicLoadU32 {
                        result: acquired,
                        view: flags,
                        index,
                    },
                    ProductionRankedOperationV1::PublicationReadGuard {
                        result,
                        success,
                        index,
                        physical_extent: self.extent,
                        acquired: ProductionRankedValueV1::Local(acquired),
                    },
                    ProductionRankedOperationV1::PredicatedAccess {
                        kind: AccessKindAttr::Read,
                        view: payload,
                        index: ProductionRankedValueV1::Local(result),
                        success: ProductionRankedValueV1::Local(success),
                    },
                ])
            }
            StaticPublicationRoleV1::Producer => Err(static_publication_reject_v1()),
        }
    }
}

struct StaticPublicationProjectorV1<'a> {
    types: &'a [SemanticTypeDeclV1],
    callables: &'a [SemanticCallableDeclV1],
    function: &'a SemanticFunctionDeclV1,
    index_values: &'a [Option<ProjectedDisjointIndexV1>],
    extent_arguments: &'a mut [Option<u32>],
    next_argument: &'a mut usize,
    entry_operations: &'a mut Vec<ProductionRankedOperationV1>,
    next_value: &'a mut u32,
}

impl StaticPublicationProjectorV1<'_> {
    fn view(
        &mut self,
        root: StaticPublicationRootV1,
    ) -> Result<
        (ProductionRankedValueIdV1, ProductionRankedValueV1),
        ProductionRankedProjectionErrorV1,
    > {
        let extent = project_consumed_read_only_extent_v1(
            root.argument,
            self.extent_arguments,
            self.next_argument,
        )?;
        let result = next_value_id(self.next_value)?;
        reserve_operation(self.entry_operations)?;
        self.entry_operations
            .push(ProductionRankedOperationV1::ViewInSpace {
                result,
                element_width: 32,
                writable: true,
                shape: vec![DYNAMIC_EXTENT],
                dynamic_extents: vec![extent],
                memory_space: MemorySpaceAttr::Global,
                allocation_origin: root.allocation.allocation_origin,
                noalias_class: root.allocation.noalias_class,
            });
        Ok((result, extent))
    }

    fn project(
        &mut self,
        source: StaticPublicationSourceV1,
    ) -> Result<ProjectedStaticPublicationV1, ProductionRankedProjectionErrorV1> {
        let (payload, extent) = self.view(source.payload)?;
        let (flags, flag_extent) = self.view(source.flags)?;
        let (index, controls) =
            project_static_publication_control_v1(self, &source, [extent, flag_extent])?;
        let mut blocks = vec![None; self.function.blocks().len()];
        for site in [source.producer, source.consumer] {
            // Acquire and its dependent guard stay in the consumer block. Their
            // IDs are assigned after entry hoisting, in final CFG block order.
            blocks[site.block] = Some(ProjectedStaticPublicationEffectV1 {
                role: site.role,
                payload,
                flags,
                index,
                extent,
                read_results: None,
            });
        }
        Ok(ProjectedStaticPublicationV1 {
            source,
            blocks,
            controls,
        })
    }
}
