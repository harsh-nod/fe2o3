// Frozen Graph103/104 geometry checker; only the method name changes.
impl CapabilitySsaGraphV1<'_> {
    fn loan_region_before_endpoint_scc(
        &mut self,
        from: u32,
        to: u32,
    ) -> Result<Arc<CapabilityLoanRegionV1>, ProductionSemanticKirErrorV1> {
        // Keys are meaningful only within this graph's immutable body/SSA pair.
        // The extra unit covers the Arc clone, before any retained row is used.
        self.charge(lookup_work(self.reuse.loan_regions.len()).saturating_add(1))?;
        if let Some(region) = self.reuse.loan_regions.get(&(from, to)) {
            return Ok(Arc::clone(region));
        }
        let blocks = self.path_region(from, to)?;
        let acyclic = self.region_is_acyclic_reusing_scratch(&blocks)?;
        // path_region already charged its vector allocation; moving it retains
        // that exact buffer. Charge the new Arc allocation and B-tree row.
        let allocation_words = std::mem::size_of::<CapabilityLoanRegionV1>()
            .div_ceil(std::mem::size_of::<usize>())
            .saturating_add(2);
        self.charge(
            insertion_work::<(u32, u32), Arc<CapabilityLoanRegionV1>>(
                self.reuse.loan_regions.len(),
            )
            .saturating_add(allocation_words),
        )?;
        let region = Arc::new(CapabilityLoanRegionV1 { blocks, acyclic });
        self.reuse
            .loan_regions
            .insert((from, to), Arc::clone(&region));
        Ok(region)
    }
}

// Fixture-only transitive closure and closed-form work. Neither reads the
// production SCC index, planner traversal, or production debit helpers.
fn endpoint_closure(edges: &[Vec<u32>]) -> Vec<Vec<bool>> {
    let n = edges.len();
    let mut paths = vec![vec![false; n]; n];
    for i in 0..n {
        paths[i][i] = true;
        for &j in &edges[i] {
            paths[i][j as usize] = true;
        }
    }
    for k in 0..n {
        for i in 0..n {
            for j in 0..n {
                let through = paths[i][k] && paths[k][j];
                paths[i][j] |= through;
            }
        }
    }
    paths
}

fn endpoint_fixture_scc_cost(edges: &[Vec<u32>]) -> usize {
    let paths = endpoint_closure(edges);
    let live = &paths[0];
    let vertices = live.iter().filter(|&&x| x).count();
    let incoming: usize = edges
        .iter()
        .enumerate()
        .filter(|(i, _)| live[*i])
        .map(|(_, row)| row.len())
        .sum();
    let components = (0..edges.len())
        .filter(|&i| live[i] && !(0..i).any(|j| live[j] && paths[i][j] && paths[j][i]))
        .count();
    let vector_header = std::mem::size_of::<Vec<u32>>().div_ceil(std::mem::size_of::<usize>());
    // Index: two owner references + vector; scratch: one vector. Includes
    // validation, owner initialization, and the final successful publication.
    (2 + 2 * vector_header) + 4 * edges.len() + 6 * vertices + 3 * incoming + 2 * components + 4
}

fn endpoint_fixture_lookup(rows: usize) -> usize {
    let mut height = 0;
    let mut n = rows;
    while n != 0 {
        height += 1;
        n /= 2;
    }
    1 + 12 * height
}

fn endpoint_fixture_publication(rows: usize) -> usize {
    let word = std::mem::size_of::<usize>();
    let row_words = (2 * std::mem::size_of::<u32>() + word).div_ceil(word) + 1;
    let region_words = std::mem::size_of::<CapabilityLoanRegionV1>().div_ceil(word) + 2;
    endpoint_fixture_lookup(rows) * (row_words + 1) + row_words + region_words
}

struct EndpointCostOracle {
    capacities: Vec<usize>,
    completed_from: Option<usize>,
    index_cold: bool,
    rows: std::collections::BTreeMap<(usize, usize), (Vec<bool>, bool)>,
}

impl EndpointCostOracle {
    fn new(count: usize) -> Self {
        Self {
            capacities: vec![0; count],
            completed_from: None,
            index_cold: true,
            rows: std::collections::BTreeMap::new(),
        }
    }

    fn query(&mut self, edges: &[Vec<u32>], from: usize, to: usize) -> (usize, Vec<bool>, bool) {
        let mut cost = endpoint_fixture_lookup(self.rows.len()) + 2 + 5;
        if let Some((region, acyclic)) = self.rows.get(&(from, to)) {
            return (cost, region.clone(), *acyclic);
        }
        let paths = endpoint_closure(edges);
        let region: Vec<_> = (0..edges.len())
            .map(|v| paths[0][from] && paths[from][v] && paths[v][to])
            .collect();
        let acyclic =
            !(0..edges.len()).any(|v| region[v] && edges[v].iter().any(|&w| paths[w as usize][v]));
        let (path_cost, path_region) = path_scratch_fixture_work(
            edges,
            from,
            to,
            &mut self.capacities,
            &mut self.completed_from,
        );
        assert_eq!(region, path_region);
        cost += path_cost
            + 1
            + edges.len()
            + region.iter().filter(|&&x| x).count()
            + endpoint_fixture_publication(self.rows.len());
        if self.index_cold {
            cost += endpoint_fixture_scc_cost(edges);
            self.index_cold = false;
        }
        self.rows.insert((from, to), (region.clone(), acyclic));
        (cost, region, acyclic)
    }
}
