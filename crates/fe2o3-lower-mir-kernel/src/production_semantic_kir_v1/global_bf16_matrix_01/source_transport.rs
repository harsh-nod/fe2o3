include!("guarded_read_source.rs");

/// Original operands captured for every reachable BF16 load by one full Session.
/// This is source transport, not constructor, bounds, or observed-read evidence.
pub struct ProductionGlobalBf16SourceBatchV1<'a> {
    owner: &'a ProductionSemanticSsaOwnerV1,
    view: &'a SemanticExpandedRootV1,
    inputs: GlobalBf16SourceBatchV1<'a>,
}

/// Borrowed source coordinates of one captured load and its original wrapper.
/// Fields have no public constructors and do not grant memory authority.
#[derive(Clone, Copy)]
pub struct ProductionGlobalBf16SourceRowV1<'row, 'a> {
    row: &'row GlobalBf16SourceBatchRowV1<'a>,
}

/// Pointer-identical source operand and its exact retained statement/terminator.
/// A constant is still a constant, not a manufactured promoted SSA use.
#[derive(Clone, Copy)]
pub struct ProductionGlobalBf16SourceOperandV1<'a> {
    inner: GlobalBf16SourceOperandV1<'a>,
}

impl<'a> ProductionGlobalBf16SourceOperandV1<'a> {
    /// Original operand node owned by the retained expansion.
    pub fn operand(self) -> &'a SemanticOperandV1 {
        self.inner.operand
    }

    /// Exact source query coordinates, including ParameterTransfer statements.
    pub fn site(self) -> fe2o3_pliron::ProductionSemanticSsaSourceSiteV1 {
        fe2o3_pliron::ProductionSemanticSsaSourceSiteV1::new(
            SemanticBlockIdV1::from_index(self.inner.site.block),
            self.inner.site.statement,
        )
    }

    /// Destination local of the containing assignment, not the operand's issuer.
    pub fn destination_local(self) -> u32 {
        self.inner.site.local
    }
}

impl<'a> ProductionGlobalBf16SourceBatchV1<'a> {
    /// Owner whose replay supplied this complete source census.
    pub fn owner(&self) -> &'a ProductionSemanticSsaOwnerV1 {
        self.owner
    }

    /// Exact retained expansion; this is not a copied or independently built view.
    pub fn view(&self) -> &'a SemanticExpandedRootV1 {
        self.view
    }

    /// Ordered source loads. No allocation or caller-selected roster is made here.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = ProductionGlobalBf16SourceRowV1<'_, 'a>> {
        self.inputs
            .rows()
            .iter()
            .map(|row| ProductionGlobalBf16SourceRowV1 { row })
    }

    /// Indexed access to the unchanged source-order census; no rescan occurs.
    pub fn row(&self, index: usize) -> Option<ProductionGlobalBf16SourceRowV1<'_, 'a>> {
        self.inputs
            .rows()
            .get(index)
            .map(|row| ProductionGlobalBf16SourceRowV1 { row })
    }
}

impl<'row, 'a> ProductionGlobalBf16SourceRowV1<'row, 'a> {
    /// Exact source owner, independently retaining the canonical subject.
    pub fn owner(self) -> &'a ProductionSemanticSsaOwnerV1 {
        self.row.inputs().owner()
    }

    /// Expansion that owns all borrowed source operands in this row.
    pub fn view(self) -> &'a SemanticExpandedRootV1 {
        self.row.inputs().view()
    }

    /// Actual expanded load block, not an original-function block ordinal.
    pub fn load_block(self) -> u32 {
        self.row.inputs().inputs().consumer.block
    }

    /// Original load call node retained by the source capture.
    pub fn call(self) -> &'a SemanticDirectCallV1 {
        self.row.inputs().inputs().call
    }

    /// Original typed load contract; it does not prove its memory effects.
    pub fn contract(self) -> SemanticGlobalBf16MatrixLoadV1 {
        self.row.inputs().inputs().contract()
    }

    /// Actual A/B forwarding wrapper instance.
    pub fn wrapper_instance(self) -> SemanticCallInstanceIdV1 {
        self.row.frame().wrapper()
    }

    /// Actual checked-constructor instance containing the matrix aggregate.
    pub fn checked_instance(self) -> SemanticCallInstanceIdV1 {
        self.row.frame().checked()
    }

    /// Wrapper function, resolved from the retained instance rather than a label.
    pub fn wrapper_function(self) -> SemanticFunctionIdV1 {
        self.view().instances()[self.wrapper_instance().index() as usize].function()
    }

    /// Checked-helper function, independently retained from its caller wrapper.
    pub fn checked_function(self) -> SemanticFunctionIdV1 {
        self.view().instances()[self.checked_instance().index() as usize].function()
    }

    /// Original policy-pair receiver at the wrapper's ParameterTransfer.
    pub fn receiver(self) -> ProductionGlobalBf16SourceOperandV1<'a> {
        ProductionGlobalBf16SourceOperandV1 {
            inner: self.row.frame().receiver(),
        }
    }

    /// Physical slice input to the actual read-only Global binding.
    pub fn physical(self) -> ProductionGlobalBf16SourceOperandV1<'a> {
        ProductionGlobalBf16SourceOperandV1 {
            inner: self.row.inputs().inputs().physical(),
        }
    }

    /// Offset, rows, columns, stride at their original aggregate operand sites.
    pub fn geometry(self) -> [ProductionGlobalBf16SourceOperandV1<'a>; 4] {
        self.row
            .inputs()
            .inputs()
            .geometry()
            .map(|inner| ProductionGlobalBf16SourceOperandV1 { inner })
    }

    /// Actual lane-reference operand at the original load, not a numeric lane.
    pub fn lane(self) -> ProductionGlobalBf16SourceOperandV1<'a> {
        ProductionGlobalBf16SourceOperandV1 {
            inner: self.row.inputs().inputs().lane(),
        }
    }

    /// Role-ordered base coordinates at the original load.
    pub fn bases(self) -> [ProductionGlobalBf16SourceOperandV1<'a>; 2] {
        self.row
            .inputs()
            .inputs()
            .bases()
            .map(|inner| ProductionGlobalBf16SourceOperandV1 { inner })
    }

    /// Actual initialized matrix aggregate SSA definition, not a type-only value.
    pub fn matrix_value(self) -> SsaValueV1 {
        self.row.inputs().inputs().matrix()
    }

    /// Actual read-only Global issuer retained by the storage source query.
    pub fn global_value(self) -> SsaValueV1 {
        self.row.inputs().inputs().global()
    }

    /// Source site of the actual checked matrix aggregate.
    pub fn construction_site(self) -> fe2o3_pliron::ProductionSemanticSsaSourceSiteV1 {
        let site = self.row.inputs().inputs().construction();
        fe2o3_pliron::ProductionSemanticSsaSourceSiteV1::new(
            SemanticBlockIdV1::from_index(site.block),
            site.statement,
        )
    }
}

impl<'a> ProductionScopedMatrixSourceSessionV1<'a> {
    /// Captures the full source census and lends its existing work charger to
    /// the sibling provider/input consumer. No lane or read is marked consumed.
    /// Returning the batch does not close this Session or its pending roster.
    pub fn with_global_bf16_source_inputs<T>(
        &mut self,
        consume: impl FnOnce(
            ProductionGlobalBf16SourceBatchV1<'a>,
            &mut dyn FnMut(usize) -> Result<(), ProductionSemanticKirErrorV1>,
        ) -> Result<T, ProductionSemanticKirErrorV1>,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        let inputs = self.capture_global_bf16_inputs()?;
        self.with_bf16_source_graph(|owner, view, _, graph, _| {
            graph.charge(
                std::mem::size_of::<ProductionGlobalBf16SourceBatchV1<'_>>()
                    .div_ceil(std::mem::size_of::<usize>()),
            )?;
            consume(
                ProductionGlobalBf16SourceBatchV1 {
                    owner,
                    view,
                    inputs,
                },
                &mut |work| graph.charge(work),
            )
        })
    }
}
