const CHECKED_DOMAIN_NODES_V1: usize = 64;
const CHECKED_DOMAIN_FORMULAS_V1: usize = 32;
const CHECKED_DOMAIN_DAG_ITEMS_V1: usize =
    CHECKED_DOMAIN_NODES_V1 * 8 + CHECKED_DOMAIN_FORMULAS_V1 * 12 + 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CheckedDomainGeometryV1 {
    RowStriped([u64; 2]),
    Tiled([u64; 4]),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CheckedDomainNodeKindV1 {
    Leaf(u8),
    Constant(u64),
    Binary {
        kind: IndexBinaryKindAttr,
        lhs: u8,
        rhs: u8,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CheckedDomainNodeV1 {
    kind: CheckedDomainNodeKindV1,
    requirements: u32,
    depth: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CheckedDomainAtomV1 {
    lhs: u8,
    rhs: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CheckedDomainFormulaV1 {
    atoms: [CheckedDomainAtomV1; 3],
    positive: [u8; 2],
    negative: [u8; 2],
    clauses: u8,
    depth: u16,
}

impl CheckedDomainFormulaV1 {
    const EMPTY: Self = Self {
        atoms: [CheckedDomainAtomV1 { lhs: 0, rhs: 0 }; 3],
        positive: [0; 2],
        negative: [0; 2],
        clauses: 0,
        depth: 0,
    };

    fn level(self) -> u8 {
        let used = self.positive[0] | self.positive[1] | self.negative[0] | self.negative[1];
        self.atoms
            .iter()
            .enumerate()
            .filter(|(i, _)| used & (1 << i) != 0)
            .map(|(_, atom)| atom.lhs.max(atom.rhs))
            .max()
            .unwrap_or(0)
    }
}

struct CheckedDomainDagV1 {
    nodes: [CheckedDomainNodeV1; CHECKED_DOMAIN_NODES_V1],
    formulas: [CheckedDomainFormulaV1; CHECKED_DOMAIN_FORMULAS_V1],
    node_count: usize,
    formula_count: usize,
    required: u32,
}

impl CheckedDomainDagV1 {
    fn node(
        &mut self,
        kind: CheckedDomainNodeKindV1,
        requirements: u32,
    ) -> Result<u8, RankedBoundsFindingV1> {
        if requirements.count_ones() > 4 {
            return Err(RankedBoundsFindingV1::StructuralVerificationFailed);
        }
        if self.node_count == CHECKED_DOMAIN_NODES_V1 {
            return Err(bounds_transport_failure_v1(
                "checked expression",
                CHECKED_DOMAIN_NODES_V1,
                CHECKED_DOMAIN_NODES_V1 + 1,
            ));
        }
        let mut depth = match kind {
            CheckedDomainNodeKindV1::Binary { lhs, rhs, .. } => self.nodes[usize::from(lhs)]
                .depth
                .max(self.nodes[usize::from(rhs)].depth),
            _ => 0,
        };
        for (index, formula) in self.formulas[..self.formula_count].iter().enumerate() {
            if requirements & (1 << index) != 0 && usize::from(formula.level()) >= self.node_count {
                return Err(RankedBoundsFindingV1::StructuralVerificationFailed);
            }
            if requirements & (1 << index) != 0 {
                depth = depth.max(formula.depth);
            }
        }
        let id = self.node_count as u8;
        self.nodes[self.node_count] = CheckedDomainNodeV1 {
            kind,
            requirements,
            depth: depth + 1,
        };
        self.node_count += 1;
        Ok(id)
    }

    fn constant(&mut self, value: u64) -> Result<u8, RankedBoundsFindingV1> {
        self.node(CheckedDomainNodeKindV1::Constant(value), 0)
    }

    fn binary(
        &mut self,
        kind: IndexBinaryKindAttr,
        lhs: u8,
        rhs: u8,
        requirements: u32,
    ) -> Result<u8, RankedBoundsFindingV1> {
        if usize::from(lhs.max(rhs)) >= self.node_count {
            return Err(RankedBoundsFindingV1::StructuralVerificationFailed);
        }
        self.node(
            CheckedDomainNodeKindV1::Binary { kind, lhs, rhs },
            requirements,
        )
    }

    fn formula(
        &mut self,
        mut formula: CheckedDomainFormulaV1,
    ) -> Result<u8, RankedBoundsFindingV1> {
        if self.formula_count == CHECKED_DOMAIN_FORMULAS_V1 {
            return Err(bounds_transport_failure_v1(
                "checked predicate",
                CHECKED_DOMAIN_FORMULAS_V1,
                CHECKED_DOMAIN_FORMULAS_V1 + 1,
            ));
        }
        if usize::from(formula.level()) >= self.node_count {
            return Err(RankedBoundsFindingV1::StructuralVerificationFailed);
        }
        let used =
            formula.positive[0] | formula.positive[1] | formula.negative[0] | formula.negative[1];
        formula.depth = formula
            .atoms
            .iter()
            .enumerate()
            .filter(|(i, _)| used & (1 << i) != 0)
            .map(|(_, atom)| {
                self.nodes[usize::from(atom.lhs)]
                    .depth
                    .max(self.nodes[usize::from(atom.rhs)].depth)
            })
            .max()
            .unwrap_or(0)
            + 1;
        let id = self.formula_count as u8;
        self.formulas[self.formula_count] = formula;
        self.formula_count += 1;
        self.required |= 1 << id;
        Ok(id)
    }

    fn less(&mut self, lhs: u8, rhs: u8, required: bool) -> Result<u8, RankedBoundsFindingV1> {
        self.formula(CheckedDomainFormulaV1 {
            atoms: [CheckedDomainAtomV1 { lhs, rhs }; 3],
            positive: [u8::from(required), 0],
            negative: [u8::from(!required), 0],
            clauses: 1,
            depth: 0,
        })
    }

    // b>0 and c<b are separately required. The second clause preserves the
    // quotient-equality/tail case instead of discarding the final partial row.
    fn multiply_add(
        &mut self,
        maximum: u8,
        a: u8,
        b: u8,
        c: u8,
        requirements: u32,
        divisor_requirements: u32,
    ) -> Result<u8, RankedBoundsFindingV1> {
        let quotient = self.binary(
            IndexBinaryKindAttr::Divide,
            maximum,
            b,
            divisor_requirements,
        )?;
        let tail = self.binary(
            IndexBinaryKindAttr::Remainder,
            maximum,
            b,
            divisor_requirements,
        )?;
        let carry = self.formula(CheckedDomainFormulaV1 {
            atoms: [
                CheckedDomainAtomV1 {
                    lhs: a,
                    rhs: quotient,
                },
                CheckedDomainAtomV1 {
                    lhs: quotient,
                    rhs: a,
                },
                CheckedDomainAtomV1 { lhs: tail, rhs: c },
            ],
            positive: [1, 0],
            negative: [0, 7],
            clauses: 3,
            depth: 0,
        })?;
        let requirements = requirements | (1 << carry);
        let product = self.binary(IndexBinaryKindAttr::Multiply, a, b, requirements)?;
        self.binary(IndexBinaryKindAttr::Add, product, c, requirements)
    }

    fn build(
        geometry: CheckedDomainGeometryV1,
        budget: &mut RankedBoundsBudget,
    ) -> Result<Option<Self>, RankedBoundsFindingV1> {
        budget.work(20)?;
        let valid = match geometry {
            CheckedDomainGeometryV1::RowStriped([lanes, elements]) => {
                lanes != 0
                    && elements != 0
                    && (elements - 1)
                        .checked_mul(lanes)
                        .and_then(|v| v.checked_add(lanes - 1))
                        .is_some()
            }
            CheckedDomainGeometryV1::Tiled([lanes, rows, columns, elements]) => {
                lanes != 0
                    && rows != 0
                    && columns != 0
                    && elements != 0
                    && lanes.is_multiple_of(columns)
                    && lanes.checked_mul(elements) == rows.checked_mul(columns)
                    && (lanes / columns).checked_mul(elements) == Some(rows)
            }
        };
        if !valid {
            return Ok(None);
        }
        budget.storage(CHECKED_DOMAIN_DAG_ITEMS_V1)?;
        budget.work(CHECKED_DOMAIN_DAG_ITEMS_V1 * 32)?;
        let mut dag = Self {
            nodes: [CheckedDomainNodeV1 {
                kind: CheckedDomainNodeKindV1::Constant(0),
                requirements: 0,
                depth: 0,
            }; CHECKED_DOMAIN_NODES_V1],
            formulas: [CheckedDomainFormulaV1::EMPTY; CHECKED_DOMAIN_FORMULAS_V1],
            node_count: 0,
            formula_count: 0,
            required: 0,
        };
        // Exact runtime tuple and physical extent; never ranked Argument IDs.
        for role in 0..6 {
            dag.node(CheckedDomainNodeKindV1::Leaf(role), 0)?;
        }
        let zero = dag.constant(0)?;
        let maximum = dag.constant(u64::MAX)?;
        let (lanes, elements) = match geometry {
            CheckedDomainGeometryV1::RowStriped([lanes, elements]) => (lanes, elements),
            CheckedDomainGeometryV1::Tiled([lanes, _, _, elements]) => (lanes, elements),
        };
        let lanes = dag.constant(lanes)?;
        let elements = dag.constant(elements)?;
        let component = dag.less(1, elements, true)?;
        let component_required = 1 << component;
        let lane = dag.binary(IndexBinaryKindAttr::Remainder, 0, lanes, 0)?;
        let (row, column) = match geometry {
            CheckedDomainGeometryV1::RowStriped(_) => {
                let row = dag.binary(IndexBinaryKindAttr::Divide, 0, lanes, 0)?;
                let base =
                    dag.binary(IndexBinaryKindAttr::Multiply, 1, lanes, component_required)?;
                let column =
                    dag.binary(IndexBinaryKindAttr::Add, base, lane, component_required)?;
                (row, column)
            }
            CheckedDomainGeometryV1::Tiled([_, tile_rows, tile_columns, _]) => {
                let height = dag.constant(tile_rows)?;
                let width = dag.constant(tile_columns)?;
                let padding = dag.constant(tile_columns - 1)?;
                let columns_max = dag.constant(u64::MAX - (tile_columns - 1))?;
                let rounded_ok = dag.less(columns_max, 3, false)?;
                let rounded = dag.binary(IndexBinaryKindAttr::Add, 3, padding, 1 << rounded_ok)?;
                let tiles = dag.binary(IndexBinaryKindAttr::Divide, rounded, width, 0)?;
                let tiles_positive = dag.less(zero, tiles, true)?;
                let tile = dag.binary(IndexBinaryKindAttr::Divide, 0, lanes, 0)?;
                let tile_row = dag.binary(
                    IndexBinaryKindAttr::Divide,
                    tile,
                    tiles,
                    1 << tiles_positive,
                )?;
                let tile_column = dag.binary(
                    IndexBinaryKindAttr::Remainder,
                    tile,
                    tiles,
                    1 << tiles_positive,
                )?;
                let lane_row = dag.binary(IndexBinaryKindAttr::Divide, lane, width, 0)?;
                let local_base =
                    dag.binary(IndexBinaryKindAttr::Multiply, lane_row, elements, 0)?;
                let local_row =
                    dag.binary(IndexBinaryKindAttr::Add, local_base, 1, component_required)?;
                let local_column = dag.binary(IndexBinaryKindAttr::Remainder, lane, width, 0)?;
                let row =
                    dag.multiply_add(maximum, tile_row, height, local_row, component_required, 0)?;
                let column = dag.multiply_add(maximum, tile_column, width, local_column, 0, 0)?;
                (row, column)
            }
        };
        dag.less(row, 2, true)?;
        let column_in_view = dag.less(column, 3, true)?;
        let stride_contains_columns = dag.less(4, 3, false)?;
        let stride_positive = dag.less(zero, 4, true)?;
        let requirements =
            (1 << column_in_view) | (1 << stride_contains_columns) | (1 << stride_positive);
        let index =
            dag.multiply_add(maximum, row, 4, column, requirements, 1 << stride_positive)?;
        dag.less(index, 5, true)?;
        Ok(Some(dag))
    }
}
