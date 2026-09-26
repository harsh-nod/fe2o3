use super::*;
use attachments::AttachmentTarget;
use fe2o3_kernel_ir::CanonicalKirOperationOriginV1 as Origin;

/// An interval of actual output gaps, not an arbitrarily selected position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionOptimizedSourceGapIntervalV18 {
    /// Actual successor block containing every possible placement.
    pub block: Block,
    /// First possible gap, inclusive.
    pub first: u32,
    /// Last possible gap, inclusive.
    pub last: u32,
}

pub(super) struct SourceIndex {
    pub(super) operations: Vec<Option<OpCoordinate>>,
    pub(super) gap_starts: Vec<usize>,
    pub(super) gaps: Vec<Option<ProductionOptimizedSourceGapIntervalV18>>,
    pub(super) attachments: Vec<AttachmentTarget>,
    sites: Vec<SourceSite>,
}

#[derive(Clone, Copy)]
struct SourceSite {
    key: [usize; 5],
    first: usize,
    end: usize,
}

fn site_key(
    root: usize,
    instance: usize,
    block: SemanticBlockIdV1,
    statement: Option<u32>,
) -> [usize; 5] {
    [
        root,
        instance,
        block.index() as usize,
        usize::from(statement.is_some()),
        statement.unwrap_or(0) as usize,
    ]
}

pub(super) fn attachment_interval(
    original: &ProductionSourceCorrespondenceV18<'_>,
    mut key: TileAttachmentKeyV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<std::ops::Range<usize>> {
    key.part = 0;
    let first = private_array_partition_v1(
        original.attachments,
        |row| source_attachment_key_v18(row.key),
        source_attachment_key_v18(key),
        false,
        &mut SourceCorrespondenceWorkV18(budget),
    )?;
    key.part = usize::MAX;
    let end = private_array_partition_v1(
        original.attachments,
        |row| source_attachment_key_v18(row.key),
        source_attachment_key_v18(key),
        true,
        &mut SourceCorrespondenceWorkV18(budget),
    )?;
    if first >= end || end > original.attachments.len() {
        return resources::binding("optimized source attachment interval");
    }
    Ok(first..end)
}

impl SourceIndex {
    pub(super) fn build(
        original: &ProductionSourceCorrespondenceV18<'_>,
        checked: &Transition<'_, '_, '_, '_>,
        control: &Control<'_, '_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Self> {
        budget.charge_work(3)?;
        if !std::ptr::eq(original.inventory, checked.input())
            || !std::ptr::eq(control.input(), checked.input())
            || !std::ptr::eq(control.output(), checked.output())
        {
            return resources::binding("optimized source transport endpoints");
        }
        let input = checked.input();
        let gap_count = input
            .operations()
            .len()
            .checked_add(input.blocks().len())
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let mut result = Self {
            operations: resources::vector(input.operations().len(), budget)?,
            gap_starts: resources::vector(input.blocks().len(), budget)?,
            gaps: resources::vector(gap_count, budget)?,
            attachments: resources::vector(original.attachments.len(), budget)?,
            sites: Vec::new(),
        };
        budget.charge_work(input.operations().len())?;
        result.operations.resize(input.operations().len(), None);
        budget.charge_work(gap_count)?;
        result.gaps.resize(gap_count, None);
        let mut next = 0usize;
        for block in input.blocks() {
            budget.charge_work(1)?;
            result.gap_starts.push(next);
            next = argument_sum_v1(&[next, block.operations.len(), 1])?;
        }
        if next != gap_count {
            return resources::binding("optimized source gap census");
        }
        for row in checked.rows().operations {
            budget.charge_work(1)?;
            if let Origin::Retained(source) = row.origin {
                let index = resources::operation_index(input, source, budget)?;
                if result.operations[index].replace(row.output).is_some() {
                    return resources::binding("optimized source repeated operation placement");
                }
            }
        }
        result.fill_gaps(checked, budget)?;
        for row in original.attachments {
            budget.charge_work(1)?;
            let target = attachments::project(row.location, checked, control, &result, budget)?;
            result.attachments.push(target);
        }
        result.build_sites(original, budget)?;
        Ok(result)
    }

    fn build_sites(
        &mut self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        let roots = original.source.root_count(budget)?;
        let mut count = 0usize;
        for root in 0..roots {
            budget.charge_work(1)?;
            count = count
                .checked_add(original.source.root_row(root)?.coordinates.spans.rows.len())
                .ok_or(ArgumentResourceV1::Arithmetic)?;
        }
        self.sites = resources::vector(count, budget)?;
        for root in 0..roots {
            let root_row = original.source.root_row(root)?;
            for (ordinal, row) in root_row.coordinates.spans.rows.iter().enumerate() {
                budget.charge_work(3)?;
                let instance = row.instance.index();
                let function = original.source.instance(root, instance, budget)?.0;
                let (owner, actual_function, _) = row.source.coordinates();
                if owner != root_row.coordinates.root || function != actual_function {
                    return resources::binding("optimized source span owner or instance");
                }
                let (block, statement) = match row.source {
                    InstanceSpanSourceV1::Statement(site) => {
                        (site.semantic_block, Some(site.statement_ordinal))
                    }
                    InstanceSpanSourceV1::Terminator(site) => (site.semantic_block, None),
                    InstanceSpanSourceV1::Synthetic(_)
                    | InstanceSpanSourceV1::InvocationEntry(_) => continue,
                };
                let range = attachment_interval(
                    original,
                    TileAttachmentKeyV29 {
                        root,
                        family: TileAttachmentFamilyV29::InstanceSpans,
                        instance,
                        row: ordinal,
                        field: TileAttachmentFieldV29::Span,
                        component: 0,
                        part: 0,
                    },
                    budget,
                )?;
                self.sites.push(SourceSite {
                    key: site_key(root, instance, block, statement),
                    first: range.start,
                    end: range.end,
                });
            }
        }
        private_array_heapsort_v1(
            &mut self.sites,
            |row| row.key,
            &mut SourceCorrespondenceWorkV18(budget),
            || ArgumentResourceV1::Arithmetic.into(),
        )?;
        for pair in self.sites.windows(2) {
            budget.charge_work(11)?;
            if pair[0].key >= pair[1].key {
                return resources::binding("ambiguous optimized source site span");
            }
        }
        Ok(())
    }

    pub(super) fn site(
        &self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&[AttachmentTarget]> {
        self.optional_site(root, instance, block, statement, budget)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized source site span",
            ))
    }

    // Absence is only an index result. The guarded source adapter must establish
    // original identity and complete block control before assigning a meaning.
    pub(super) fn optional_site(
        &self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<&[AttachmentTarget]>> {
        let key = site_key(root, instance, block, statement);
        let ordinal = private_array_partition_v1(
            &self.sites,
            |row| row.key,
            key,
            false,
            &mut SourceCorrespondenceWorkV18(budget),
        )?;
        budget.charge_work(7)?;
        let Some(site) = self.sites.get(ordinal).filter(|row| row.key == key) else {
            return Ok(None);
        };
        self.attachments.get(site.first..site.end).map(Some).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("optimized source span attachments"),
        )
    }

    pub(super) fn visit_block_sites(
        &self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
        mut visit: impl FnMut(
            Option<u32>,
            &[AttachmentTarget],
            &mut ArgumentBudgetV1<'_>,
        ) -> SourceOwnedResultV18<()>,
    ) -> SourceOwnedResultV18<usize> {
        let lower = site_key(root, instance, block, None);
        let upper = [root, instance, block.index() as usize, 1, usize::MAX];
        let first = private_array_partition_v1(
            &self.sites,
            |row| row.key,
            lower,
            false,
            &mut SourceCorrespondenceWorkV18(budget),
        )?;
        let end = private_array_partition_v1(
            &self.sites,
            |row| row.key,
            upper,
            true,
            &mut SourceCorrespondenceWorkV18(budget),
        )?;
        budget.charge_work(4)?;
        if self
            .sites
            .get(end)
            .is_some_and(|row| row.key[..3] == lower[..3])
        {
            return resources::binding("optimized source block site discriminator");
        }
        let rows = self
            .sites
            .get(first..end)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized source block site range",
            ))?;
        for row in rows {
            budget.charge_work(9)?;
            if row.key[..3] != lower[..3] {
                return resources::binding("optimized source block site owner");
            }
            let statement = match (row.key[3], row.key[4]) {
                (0, 0) => None,
                (1, ordinal) => Some(u32::try_from(ordinal).map_err(|_| {
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "optimized source block statement ordinal",
                    )
                })?),
                _ => return resources::binding("optimized source block site discriminator"),
            };
            let targets = self.attachments.get(row.first..row.end).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("optimized source span attachments"),
            )?;
            visit(statement, targets, budget)?;
        }
        Ok(rows.len())
    }

    // Two linear passes over each checked merge chain retain both sides of a
    // source gap. Inserted constants do not acquire source/currentness authority.
    fn fill_gaps(
        &mut self,
        checked: &Transition<'_, '_, '_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        for block in checked.rows().blocks {
            budget.charge_work(1)?;
            let first = block.segments.start as usize;
            let end = first
                .checked_add(block.segments.len as usize)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            let segments = checked.rows().segments.get(first..end).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("optimized source segment interval"),
            )?;
            let output_index = resources::block_index(checked.output(), block.output, budget)?;
            let output_count =
                u32::try_from(checked.output().blocks()[output_index].operations.len())
                    .map_err(|_| ArgumentResourceV1::Arithmetic)?;
            let mut lower = 0u32;
            for segment in segments {
                let index = resources::block_index(checked.input(), segment.input, budget)?;
                let range = checked.input().blocks()[index].operations.clone();
                let gap_start = self.gap_starts[index];
                for offset in 0..=range.len() {
                    budget.charge_work(1)?;
                    self.gaps[gap_start + offset] = Some(ProductionOptimizedSourceGapIntervalV18 {
                        block: block.output,
                        first: lower,
                        last: output_count,
                    });
                    if offset < range.len() {
                        if let Some(actual) = self.operations[range.start + offset] {
                            if actual.block != block.output
                                || actual.operation < lower
                                || actual.operation >= output_count
                            {
                                return resources::binding(
                                    "optimized source retained anchors changed order",
                                );
                            }
                            lower = actual
                                .operation
                                .checked_add(1)
                                .ok_or(ArgumentResourceV1::Arithmetic)?;
                        }
                    }
                }
            }
            let mut upper = output_count;
            for segment in segments.iter().rev() {
                let index = resources::block_index(checked.input(), segment.input, budget)?;
                let range = checked.input().blocks()[index].operations.clone();
                let gap_start = self.gap_starts[index];
                for offset in (0..=range.len()).rev() {
                    budget.charge_work(1)?;
                    if offset < range.len() {
                        if let Some(actual) = self.operations[range.start + offset] {
                            if actual.operation >= upper {
                                return resources::binding("optimized source gap order");
                            }
                            upper = actual.operation;
                        }
                    }
                    let gap = self.gaps[gap_start + offset].as_mut().ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding("optimized source gap coverage"),
                    )?;
                    if gap.first > upper {
                        return resources::binding("optimized source empty gap interval");
                    }
                    gap.last = upper;
                }
            }
        }
        Ok(())
    }

    pub(super) fn operation(
        &self,
        checked: &Transition<'_, '_, '_, '_>,
        input: OpCoordinate,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<OpCoordinate>> {
        let index = resources::operation_index(checked.input(), input, budget)?;
        budget.charge_work(1)?;
        self.operations
            .get(index)
            .copied()
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized source operation placement census",
            ))
    }

    pub(super) fn gap(
        &self,
        checked: &Transition<'_, '_, '_, '_>,
        block: Block,
        operation: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionOptimizedSourceGapIntervalV18>> {
        let index = resources::block_index(checked.input(), block, budget)?;
        budget.charge_work(2)?;
        if operation > checked.input().blocks()[index].operations.len() {
            return resources::binding("optimized source input gap outside its block");
        }
        let index = self.gap_starts[index]
            .checked_add(operation)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        self.gaps
            .get(index)
            .copied()
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized source gap roster",
            ))
    }
}

#[cfg(test)]
mod block_site_tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    fn fixture() -> SourceIndex {
        let keys = [
            [0, 0, 1, 0, 0],
            [0, 0, 2, 0, 0],
            [0, 0, 2, 1, 0],
            [0, 0, 2, 1, 3],
            [0, 0, 3, 0, 0],
        ];
        SourceIndex {
            operations: vec![],
            gap_starts: vec![],
            gaps: vec![],
            attachments: vec![AttachmentTarget::OriginalNoOutput; keys.len()],
            sites: keys
                .into_iter()
                .enumerate()
                .map(|(index, key)| SourceSite {
                    key,
                    first: index,
                    end: index + 1,
                })
                .collect(),
        }
    }

    // Independent logical comparisons, including partition's terminal probe.
    fn partition_work(keys: &[[usize; 5]], target: [usize; 5], upper: bool) -> usize {
        let (mut low, mut high, mut work) = (0, keys.len(), 0);
        while low != high {
            let middle = (low + high) / 2;
            let compared = keys[middle]
                .iter()
                .zip(target)
                .position(|(a, b)| *a != b)
                .map_or(5, |index| index + 1);
            let left = keys[middle] < target || upper && keys[middle] == target;
            work += 6 + compared + usize::from(left);
            if left {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        work + 1
    }

    #[test]
    fn required_and_optional_present_site_keep_identical_exact_work() {
        let index = fixture();
        let keys = index.sites.iter().map(|site| site.key).collect::<Vec<_>>();
        let block = SemanticBlockIdV1::from_index(2);
        for statement in [None, Some(0), Some(3)] {
            let expected = partition_work(&keys, site_key(0, 0, block, statement), false) + 7;
            for optional in [false, true] {
                for short in [false, true] {
                    let mut work = Work::new(expected - usize::from(short));
                    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
                    let result = if optional {
                        index
                            .optional_site(0, 0, block, statement, &mut budget)
                            .map(|rows| rows.unwrap().len())
                    } else {
                        index
                            .site(0, 0, block, statement, &mut budget)
                            .map(<[_]>::len)
                    };
                    if short {
                        assert!(
                            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(
                            ArgumentResourceV1::Work(error))) if error.actual() == expected)
                        );
                        assert_eq!(budget.work(), expected - 7);
                    } else {
                        assert_eq!(result.unwrap(), 1);
                        assert_eq!(budget.work(), expected);
                    }
                    assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
                }
            }
        }
    }

    #[test]
    fn optional_absence_is_not_a_successful_required_site() {
        let index = fixture();
        for optional in [false, true] {
            let mut work = Work::new(10_000);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            let block = SemanticBlockIdV1::from_index(2);
            if optional {
                assert!(
                    index
                        .optional_site(0, 0, block, Some(1), &mut budget)
                        .unwrap()
                        .is_none()
                );
            } else {
                assert!(matches!(
                    index.site(0, 0, block, Some(1), &mut budget),
                    Err(ProductionSourceOwnedViewErrorV18::Binding(
                        "optimized source site span"
                    ))
                ));
            }
        }
    }

    #[test]
    fn complete_block_visitor_has_independent_exact_and_one_short_work() {
        let index = fixture();
        let keys = index.sites.iter().map(|site| site.key).collect::<Vec<_>>();
        let expected = partition_work(&keys, [0, 0, 2, 0, 0], false)
            + partition_work(&keys, [0, 0, 2, 1, usize::MAX], true)
            + 4
            + 3 * 9;
        for short in [false, true] {
            let mut work = Work::new(expected - usize::from(short));
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            let mut visited = Vec::new();
            let result = index.visit_block_sites(
                0,
                0,
                SemanticBlockIdV1::from_index(2),
                &mut budget,
                |statement, rows, _| {
                    assert_eq!(rows.len(), 1);
                    assert!(matches!(rows[0], AttachmentTarget::OriginalNoOutput));
                    visited.push(statement);
                    Ok(())
                },
            );
            if short {
                assert!(
                    matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Work(error))) if error.actual() == expected)
                );
                assert_eq!(visited, [None, Some(0)]);
                assert_eq!(budget.work(), expected - 9);
            } else {
                assert_eq!(result.unwrap(), 3);
                assert_eq!(visited, [None, Some(0), Some(3)]);
                assert_eq!(budget.work(), expected);
            }
            assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
        }
    }

    #[test]
    fn complete_block_visitor_refuses_corrupt_grammar_and_intervals() {
        for mode in 0..4 {
            let mut index = fixture();
            match mode {
                0 => index.sites[1].key[4] = 1,
                1 => index.sites[3].key[3] = 2,
                2 => index.sites[2].end = index.attachments.len() + 1,
                3 => index.sites[2].first = index.sites[2].end + 1,
                _ => unreachable!(),
            }
            let mut work = Work::new(10_000);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            assert!(matches!(
                index.visit_block_sites(
                    0,
                    0,
                    SemanticBlockIdV1::from_index(2),
                    &mut budget,
                    |_, _, _| Ok(())
                ),
                Err(ProductionSourceOwnedViewErrorV18::Binding(_))
            ));
        }
    }
}
