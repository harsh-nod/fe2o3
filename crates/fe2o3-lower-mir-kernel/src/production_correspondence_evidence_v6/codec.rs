use super::*;

const MAGIC: &[u8; 8] = b"F2M2K6\0\0";
const MAX_NAME: usize = 4096;

pub(super) fn encode(e: &InertCanonicalMirToKirCorrespondenceEvidenceV6) -> Result<Vec<u8>> {
    validate(e)?;
    let mut w = Writer(Vec::new());
    w.bytes(MAGIC)?;
    w.bytes(&MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_VERSION_V6.to_le_bytes())?;
    w.bytes(&MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_POLICY_V6.to_le_bytes())?;
    w.words([0, 0])?;
    w.bytes(&e.semantic_ssa_identity)?;
    w.word(match e.canonical_kernel_ir.version() {
        ProductionCanonicalKernelIrVersionV1::V8 => 8,
        ProductionCanonicalKernelIrVersionV1::V9 => 9,
        ProductionCanonicalKernelIrVersionV1::V11 => 11,
        ProductionCanonicalKernelIrVersionV1::V12 => 12,
        ProductionCanonicalKernelIrVersionV1::V13 => 13,
    })?;
    w.bytes(&e.canonical_kernel_ir.canonical_length().to_le_bytes())?;
    w.bytes(e.canonical_kernel_ir.digest())?;
    for limit in [
        e.lowering_limits.max_functions,
        e.lowering_limits.max_blocks,
        e.lowering_limits.max_statements,
        e.lowering_limits.max_operations,
    ] {
        w.bytes(&(limit as u64).to_le_bytes())?;
    }
    w.blob(e.expansion.canonical_bytes())?;
    w.rows(&e.roots, |w, root| {
        w.words([root.root.index(), root.source_body.index()])?;
        w.bytes(&root.execution_view_identity)?;
        match &root.induction {
            MirToKirInductionEvidenceV6::Original(induction) => {
                w.word(1)?;
                w.blob(induction.canonical_bytes())
            }
            MirToKirInductionEvidenceV6::Expanded(induction) => {
                w.word(2)?;
                w.blob(induction.canonical_bytes())
            }
        }
    })?;
    w.rows(&e.functions, |w, f| {
        w.words([
            f.root().index(),
            f.source_body().index(),
            f.kernel_ir_function_ordinal,
            match f.role() {
                SemanticKirFunctionRoleV1::KernelEntry => 1,
                SemanticKirFunctionRoleV1::InternalHelper => 2,
            },
        ])?;
        w.blob(f.kernel_ir_function().as_str().as_bytes())
    })?;
    let c = &e.correspondence;
    w.count(c.function_count)?;
    // The source hash and expansion identity are uniquely derived from the nested
    // expansion. All remaining live correspondence fields are encoded below.
    w.rows(&c.blocks, |w, b| {
        w.words([
            b.correspondence_owner.index(),
            b.semantic_function.index(),
            b.semantic_block.index(),
            b.kernel_ir_block.0,
            b.source_statement_count,
        ])
    })?;
    w.rows(&c.statement_operation_spans, |w, s| {
        w.words([
            s.correspondence_owner.index(),
            s.semantic_function.index(),
            s.semantic_block.index(),
            s.statement_ordinal,
            s.kernel_ir_block.0,
            s.first_operation_ordinal,
            s.operation_count,
        ])
    })?;
    w.rows(&c.terminator_operation_spans, |w, s| {
        w.words([
            s.correspondence_owner.index(),
            s.semantic_function.index(),
            s.semantic_block.index(),
            s.kernel_ir_block.0,
            s.first_operation_ordinal,
            s.operation_count,
        ])
    })?;
    w.rows(&c.generated_terminator_values, |w, s| {
        w.words([
            s.correspondence_owner.index(),
            s.semantic_function.index(),
            s.semantic_block.index(),
            s.destination_local.index(),
            s.input.0,
            s.output.0,
        ])
    })?;
    w.rows(&c.synthetic_operation_spans, |w, s| {
        w.words([
            s.correspondence_owner.index(),
            s.semantic_function.index(),
            match s.rule {
                SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage => 1,
                SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage => 2,
                SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap => 3,
            },
            s.kernel_ir_block.0,
            s.first_operation_ordinal,
            s.operation_count,
        ])
    })?;
    w.rows(&c.parameter_bindings, |w, p| {
        w.words([
            p.correspondence_owner.index(),
            p.semantic_function.index(),
            p.semantic_local.index(),
            p.kernel_ir_value.0,
        ])
    })?;
    w.rows(&c.parameter_component_bindings, |w, p| {
        w.words([
            p.correspondence_owner.index(),
            p.semantic_function.index(),
            p.semantic_local.index(),
            p.semantic_component_type.index(),
            p.kernel_ir_value.0,
        ])?;
        w.rows(&p.projection, |w, projection| match projection {
            SemanticKirParameterProjectionV1::Field(index) => w.words([1, *index]),
            SemanticKirParameterProjectionV1::ArrayIndex(index) => w.words([2, *index]),
        })
    })?;
    w.rows(&c.ignored_parameter_bindings, |w, p| {
        w.words([
            p.correspondence_owner.index(),
            p.semantic_function.index(),
            p.semantic_local.index(),
            p.semantic_type.index(),
        ])
    })?;
    let len = u32::try_from(w.0.len()).map_err(|_| E::LimitExceeded)?;
    w.0[16..20].copy_from_slice(&len.to_le_bytes());
    Ok(w.0)
}

pub(super) fn decode(bytes: &[u8]) -> Result<InertCanonicalMirToKirCorrespondenceEvidenceV6> {
    if bytes.len() > MAX_MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_BYTES_V6 {
        return Err(E::LimitExceeded);
    }
    let mut r = Reader {
        bytes,
        position: 0,
        rows: 0,
    };
    if r.fixed::<8>()? != *MAGIC
        || u16::from_le_bytes(r.fixed()?) != MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_VERSION_V6
        || u16::from_le_bytes(r.fixed()?) != MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_POLICY_V6
        || r.word()? != 0
    {
        return Err(E::InvalidHeader);
    }
    if r.count()? != bytes.len() {
        return Err(E::InvalidLength);
    }
    let semantic_ssa_identity = r.fixed()?;
    let version = match r.word()? {
        8 => ProductionCanonicalKernelIrVersionV1::V8,
        9 => ProductionCanonicalKernelIrVersionV1::V9,
        11 => ProductionCanonicalKernelIrVersionV1::V11,
        12 => ProductionCanonicalKernelIrVersionV1::V12,
        13 => ProductionCanonicalKernelIrVersionV1::V13,
        _ => return Err(E::InvalidHeader),
    };
    let length = u64::from_le_bytes(r.fixed()?);
    let digest = r.fixed()?;
    let canonical_kernel_ir =
        ProductionCanonicalKernelIrIdentityV1::from_canonical_parts(version, digest, length);
    let mut limits = [0; 4];
    for limit in &mut limits {
        *limit = usize::try_from(u64::from_le_bytes(r.fixed()?)).map_err(|_| E::LimitExceeded)?;
    }
    let lowering_limits = ProductionSemanticKirLimitsV1::new_with_max_operations(
        limits[0], limits[1], limits[2], limits[3],
    );
    let expansion =
        InertCanonicalSemanticCallExpansionEvidenceV1::decode(r.blob()?).map_err(E::Expansion)?;
    let roots = r.rows(48, |r| {
        let [root, body] = r.words()?;
        let execution_view_identity = r.fixed()?;
        let induction = match r.word()? {
            1 => MirToKirInductionEvidenceV6::Original(
                InertCanonicalSemanticU32InductionEvidenceV1::decode(r.blob()?)
                    .map_err(E::InductionV1)?,
            ),
            2 => MirToKirInductionEvidenceV6::Expanded(
                InertCanonicalSemanticU32InductionEvidenceV2::decode(r.blob()?)
                    .map_err(E::InductionV2)?,
            ),
            _ => return Err(E::InvalidHeader),
        };
        Ok(MirToKirRootCorrespondenceEvidenceV6 {
            root: fid(root),
            source_body: fid(body),
            execution_view_identity,
            induction,
        })
    })?;
    let functions = r.rows(20, |r| {
        let [root, body, kernel_ir_function_ordinal, role] = r.words()?;
        let role = match role {
            1 => SemanticKirFunctionRoleV1::KernelEntry,
            2 => SemanticKirFunctionRoleV1::InternalHelper,
            _ => return Err(E::InvalidHeader),
        };
        let name = r.blob()?;
        if name.is_empty() || name.len() > MAX_NAME {
            return Err(E::LimitExceeded);
        }
        let name = std::str::from_utf8(name).map_err(|_| E::InvalidHeader)?;
        Ok(MirToKirFunctionCorrespondenceEvidenceV6 {
            record: SemanticKirFunctionCorrespondenceV1 {
                correspondence_owner: fid(root),
                semantic_function: fid(body),
                kernel_ir_function: FunctionId::new(name),
                role,
            },
            kernel_ir_function_ordinal,
        })
    })?;
    let function_count = r.count()?;
    let blocks = r.rows(20, |r| {
        let [root, body, block, kir, count] = r.words()?;
        Ok(SemanticKirBlockCorrespondenceV1 {
            correspondence_owner: fid(root),
            semantic_function: fid(body),
            semantic_block: bid(block),
            kernel_ir_block: BlockId(kir),
            source_statement_count: count,
        })
    })?;
    let statement_operation_spans = r.rows(28, |r| {
        let [root, body, block, statement, kir, first, count] = r.words()?;
        Ok(SemanticKirStatementOperationSpanV1 {
            correspondence_owner: fid(root),
            semantic_function: fid(body),
            semantic_block: bid(block),
            statement_ordinal: statement,
            kernel_ir_block: BlockId(kir),
            first_operation_ordinal: first,
            operation_count: count,
        })
    })?;
    let terminator_operation_spans = r.rows(24, |r| {
        let [root, body, block, kir, first, count] = r.words()?;
        Ok(SemanticKirTerminatorOperationSpanV1 {
            correspondence_owner: fid(root),
            semantic_function: fid(body),
            semantic_block: bid(block),
            kernel_ir_block: BlockId(kir),
            first_operation_ordinal: first,
            operation_count: count,
        })
    })?;
    let generated_terminator_values = r.rows(24, |r| {
        let [root, body, block, local, input, output] = r.words()?;
        Ok(SemanticKirGeneratedTerminatorValuesV1 {
            correspondence_owner: fid(root),
            semantic_function: fid(body),
            semantic_block: bid(block),
            destination_local: lid(local),
            input: ValueId(input),
            output: ValueId(output),
        })
    })?;
    let synthetic_operation_spans = r.rows(24, |r| {
        let [root, body, rule, kir, first, count] = r.words()?;
        let rule = match rule {
            1 => SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage,
            2 => SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage,
            3 => SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap,
            _ => return Err(E::InvalidHeader),
        };
        Ok(SemanticKirSyntheticOperationSpanV1 {
            correspondence_owner: fid(root),
            semantic_function: fid(body),
            rule,
            kernel_ir_block: BlockId(kir),
            first_operation_ordinal: first,
            operation_count: count,
        })
    })?;
    let parameter_bindings = r.rows(16, |r| {
        let [root, body, local, value] = r.words()?;
        Ok(SemanticKirParameterBindingV1 {
            correspondence_owner: fid(root),
            semantic_function: fid(body),
            semantic_local: lid(local),
            kernel_ir_value: ValueId(value),
        })
    })?;
    let parameter_component_bindings = r.rows(24, |r| {
        let [root, body, local, ty, value] = r.words()?;
        let projection = r.rows(8, |r| match r.words()? {
            [1, index] => Ok(SemanticKirParameterProjectionV1::Field(index)),
            [2, index] => Ok(SemanticKirParameterProjectionV1::ArrayIndex(index)),
            _ => Err(E::InvalidHeader),
        })?;
        Ok(SemanticKirParameterComponentBindingV1 {
            correspondence_owner: fid(root),
            semantic_function: fid(body),
            semantic_local: lid(local),
            semantic_component_type: SemanticTypeIdV1::from_index(ty),
            kernel_ir_value: ValueId(value),
            projection,
        })
    })?;
    let ignored_parameter_bindings = r.rows(16, |r| {
        let [root, body, local, ty] = r.words()?;
        Ok(SemanticKirIgnoredParameterBindingV1 {
            correspondence_owner: fid(root),
            semantic_function: fid(body),
            semantic_local: lid(local),
            semantic_type: SemanticTypeIdV1::from_index(ty),
        })
    })?;
    if r.position != bytes.len() {
        return Err(E::InvalidLength);
    }
    let lowered_functions = functions.iter().map(|f| f.record.clone()).collect();
    let correspondence = SemanticKirCorrespondenceV1 {
        semantic_sha256: *expansion.source_semantic_sha256(),
        execution_expansion_identity: expansion
            .roots()
            .iter()
            .any(|r| r.has_expanded_calls())
            .then(|| *expansion.expansion_identity()),
        function_count,
        lowered_functions,
        blocks,
        statement_operation_spans,
        terminator_operation_spans,
        generated_terminator_values,
        synthetic_operation_spans,
        parameter_bindings,
        parameter_component_bindings,
        ignored_parameter_bindings,
    };
    let mut evidence = InertCanonicalMirToKirCorrespondenceEvidenceV6 {
        canonical_bytes: Box::new([]),
        identity: [0; 32],
        semantic_ssa_identity,
        canonical_kernel_ir,
        lowering_limits,
        expansion,
        roots,
        functions,
        correspondence,
    };
    let canonical = encode(&evidence)?;
    if canonical != bytes {
        return Err(E::NonCanonical);
    }
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update((canonical.len() as u64).to_le_bytes());
    hash.update(&canonical);
    evidence.identity = hash.finalize().into();
    evidence.canonical_bytes = canonical.into_boxed_slice();
    Ok(evidence)
}

use ProductionCorrespondenceEvidenceErrorV6 as E;
fn fid(id: u32) -> SemanticFunctionIdV1 {
    SemanticFunctionIdV1::from_index(id)
}
fn bid(id: u32) -> SemanticBlockIdV1 {
    SemanticBlockIdV1::from_index(id)
}
fn lid(id: u32) -> SemanticLocalIdV1 {
    SemanticLocalIdV1::from_index(id)
}

pub(super) fn validate_record_counts(c: &SemanticKirCorrespondenceV1) -> Result<()> {
    let mut count = 0usize;
    for n in [
        c.lowered_functions.len(),
        c.blocks.len(),
        c.statement_operation_spans.len(),
        c.terminator_operation_spans.len(),
        c.generated_terminator_values.len(),
        c.synthetic_operation_spans.len(),
        c.parameter_bindings.len(),
        c.parameter_component_bindings.len(),
        c.ignored_parameter_bindings.len(),
    ]
    .into_iter()
    .chain(
        c.parameter_component_bindings
            .iter()
            .map(|p| p.projection.len()),
    ) {
        count = count.checked_add(n).ok_or(E::LimitExceeded)?;
        if count > MAX_MIR_TO_KIR_CORRESPONDENCE_RECORDS_V6 {
            return Err(E::LimitExceeded);
        }
    }
    Ok(())
}

fn validate(e: &InertCanonicalMirToKirCorrespondenceEvidenceV6) -> Result<()> {
    let c = &e.correspondence;
    validate_record_counts(c)?;
    if e.semantic_ssa_identity == [0; 32]
        || e.canonical_kernel_ir.digest() == &[0; 32]
        || e.canonical_kernel_ir.canonical_length() == 0
        || c.semantic_sha256 != *e.expansion.source_semantic_sha256()
        || c.execution_expansion_identity
            != e.expansion
                .roots()
                .iter()
                .any(|r| r.has_expanded_calls())
                .then(|| *e.expansion.expansion_identity())
        || c.function_count == 0
    {
        return Err(E::Correspondence);
    }
    if e.roots.is_empty()
        || e.roots.len() != e.expansion.roots().len()
        || e.functions.len() != e.roots.len()
        || c.lowered_functions.len() != e.functions.len()
    {
        return Err(E::RootRoster);
    }
    for (ordinal, ((root, expanded), function)) in e
        .roots
        .iter()
        .zip(e.expansion.roots())
        .zip(&e.functions)
        .enumerate()
    {
        if root.root != expanded.root()
            || root.source_body != expanded.source_body()
            || root.execution_view_identity != *expanded.identity()
            || function.root() != root.root
            || function.source_body() != root.source_body
            || function.role() != SemanticKirFunctionRoleV1::KernelEntry
            || function.record != c.lowered_functions[ordinal]
            || function.kernel_ir_function().as_str().is_empty()
            || function.kernel_ir_function().as_str().len() > MAX_NAME
        {
            return Err(E::RootRoster);
        }
        match &root.induction {
            MirToKirInductionEvidenceV6::Original(induction) => {
                if expanded.has_expanded_calls() {
                    return Err(E::InductionCoordinateSpace);
                }
                if induction.semantic_mir_sha256() != e.semantic_sha256()
                    || induction.function() != root.source_body.index()
                    || induction.function_identity()
                        != expanded.expanded_function_identity().as_bytes()
                {
                    return Err(E::ReportMismatch);
                }
            }
            MirToKirInductionEvidenceV6::Expanded(induction) => {
                if !expanded.has_expanded_calls() {
                    return Err(E::InductionCoordinateSpace);
                }
                if induction.root() != root.root
                    || induction.function() != root.source_body.index()
                    || induction.semantic_mir_sha256() != e.semantic_sha256()
                    || induction.expansion_evidence_identity() != e.expansion.identity()
                    || induction.expansion_identity() != e.expansion.expansion_identity()
                    || induction.execution_view_identity() != &root.execution_view_identity
                    || induction.function_identity()
                        != expanded.expanded_function_identity().as_bytes()
                {
                    return Err(E::ReportMismatch);
                }
            }
        }
    }
    let ordinals = e
        .functions
        .iter()
        .map(|f| f.kernel_ir_function_ordinal)
        .collect::<BTreeSet<_>>();
    let names = e
        .functions
        .iter()
        .map(|f| f.kernel_ir_function())
        .collect::<BTreeSet<_>>();
    if ordinals.len() != e.functions.len() || names.len() != e.functions.len() {
        return Err(E::RootRoster);
    }
    let view = |root, body| {
        e.expansion
            .root(root)
            .filter(|v| v.source_body() == body)
            .ok_or(E::Correspondence)
    };
    let mut blocks = BTreeMap::new();
    let mut kir_blocks = BTreeSet::new();
    for b in &c.blocks {
        let source = view(b.correspondence_owner, b.semantic_function)?;
        let origin = source
            .block_origins()
            .get(b.semantic_block.index() as usize)
            .ok_or(E::Correspondence)?;
        if origin.statements().len() != b.source_statement_count as usize
            || blocks
                .insert((b.correspondence_owner, b.semantic_block), b)
                .is_some()
            || !kir_blocks.insert((b.correspondence_owner, b.kernel_ir_block))
        {
            return Err(E::Correspondence);
        }
    }
    let block = |root, body, id, kir| {
        let b = blocks.get(&(root, id)).ok_or(E::Correspondence)?;
        if b.semantic_function != body || b.kernel_ir_block != kir {
            return Err(E::Correspondence);
        }
        Ok(*b)
    };
    let mut statements = BTreeSet::new();
    for s in &c.statement_operation_spans {
        let b = block(
            s.correspondence_owner,
            s.semantic_function,
            s.semantic_block,
            s.kernel_ir_block,
        )?;
        if s.statement_ordinal >= b.source_statement_count
            || !statements.insert((
                s.correspondence_owner,
                s.semantic_block,
                s.statement_ordinal,
            ))
            || s.first_operation_ordinal
                .checked_add(s.operation_count)
                .is_none()
        {
            return Err(E::Correspondence);
        }
    }
    let expected_statements = c
        .blocks
        .iter()
        .try_fold(0usize, |n, b| {
            n.checked_add(b.source_statement_count as usize)
        })
        .ok_or(E::LimitExceeded)?;
    if statements.len() != expected_statements {
        return Err(E::Correspondence);
    }
    let mut terminators = BTreeSet::new();
    for s in &c.terminator_operation_spans {
        block(
            s.correspondence_owner,
            s.semantic_function,
            s.semantic_block,
            s.kernel_ir_block,
        )?;
        if !terminators.insert((s.correspondence_owner, s.semantic_block))
            || s.first_operation_ordinal
                .checked_add(s.operation_count)
                .is_none()
        {
            return Err(E::Correspondence);
        }
    }
    if terminators.len() != blocks.len() {
        return Err(E::Correspondence);
    }
    for s in &c.synthetic_operation_spans {
        view(s.correspondence_owner, s.semantic_function)?;
        if s.operation_count == 0
            || s.first_operation_ordinal
                .checked_add(s.operation_count)
                .is_none()
        {
            return Err(E::Correspondence);
        }
    }
    for p in &c.parameter_bindings {
        check_local(
            &view,
            p.correspondence_owner,
            p.semantic_function,
            p.semantic_local,
        )?;
    }
    for p in &c.parameter_component_bindings {
        check_local(
            &view,
            p.correspondence_owner,
            p.semantic_function,
            p.semantic_local,
        )?;
    }
    for p in &c.ignored_parameter_bindings {
        check_local(
            &view,
            p.correspondence_owner,
            p.semantic_function,
            p.semantic_local,
        )?;
    }
    for g in &c.generated_terminator_values {
        check_local(
            &view,
            g.correspondence_owner,
            g.semantic_function,
            g.destination_local,
        )?;
        if !blocks.contains_key(&(g.correspondence_owner, g.semantic_block)) {
            return Err(E::Correspondence);
        }
    }
    Ok(())
}

fn check_local<'a>(
    view: &impl Fn(
        SemanticFunctionIdV1,
        SemanticFunctionIdV1,
    ) -> Result<&'a fe2o3_mir_model::SemanticExpandedRootEvidenceV1>,
    root: SemanticFunctionIdV1,
    body: SemanticFunctionIdV1,
    local: SemanticLocalIdV1,
) -> Result<()> {
    if view(root, body)?
        .local_origins()
        .get(local.index() as usize)
        .is_none()
    {
        return Err(E::Correspondence);
    }
    Ok(())
}

struct Writer(Vec<u8>);
impl Writer {
    fn bytes(&mut self, bytes: &[u8]) -> Result<()> {
        if self
            .0
            .len()
            .checked_add(bytes.len())
            .is_none_or(|n| n > MAX_MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_BYTES_V6)
        {
            return Err(E::LimitExceeded);
        }
        self.0
            .try_reserve(bytes.len())
            .map_err(|_| E::AllocationFailure)?;
        self.0.extend_from_slice(bytes);
        Ok(())
    }
    fn word(&mut self, n: u32) -> Result<()> {
        self.bytes(&n.to_le_bytes())
    }
    fn words<const N: usize>(&mut self, words: [u32; N]) -> Result<()> {
        for n in words {
            self.word(n)?;
        }
        Ok(())
    }
    fn count(&mut self, n: usize) -> Result<()> {
        self.word(u32::try_from(n).map_err(|_| E::LimitExceeded)?)
    }
    fn blob(&mut self, bytes: &[u8]) -> Result<()> {
        self.count(bytes.len())?;
        self.bytes(bytes)
    }
    fn rows<T>(
        &mut self,
        rows: &[T],
        mut write: impl FnMut(&mut Self, &T) -> Result<()>,
    ) -> Result<()> {
        self.count(rows.len())?;
        for row in rows {
            write(self, row)?;
        }
        Ok(())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
    rows: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.position.checked_add(len).ok_or(E::InvalidLength)?;
        let bytes = self.bytes.get(self.position..end).ok_or(E::InvalidLength)?;
        self.position = end;
        Ok(bytes)
    }
    fn fixed<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.take(N)?.try_into().map_err(|_| E::InvalidLength)
    }
    fn word(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }
    fn words<const N: usize>(&mut self) -> Result<[u32; N]> {
        let mut words = [0; N];
        for word in &mut words {
            *word = self.word()?;
        }
        Ok(words)
    }
    fn count(&mut self) -> Result<usize> {
        usize::try_from(self.word()?).map_err(|_| E::LimitExceeded)
    }
    fn blob(&mut self) -> Result<&'a [u8]> {
        let len = self.count()?;
        self.take(len)
    }
    fn rows<T>(
        &mut self,
        minimum: usize,
        mut read: impl FnMut(&mut Self) -> Result<T>,
    ) -> Result<Box<[T]>> {
        let count = self.count()?;
        self.rows = self.rows.checked_add(count).ok_or(E::LimitExceeded)?;
        if self.rows > MAX_MIR_TO_KIR_CORRESPONDENCE_RECORDS_V6 {
            return Err(E::LimitExceeded);
        }
        if count
            .checked_mul(minimum)
            .is_none_or(|n| n > self.bytes.len() - self.position)
        {
            return Err(E::InvalidLength);
        }
        let mut rows = reserve(count)?;
        for _ in 0..count {
            rows.push(read(self)?);
        }
        Ok(rows.into_boxed_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_reader_rows_charge_exact_limit_then_reject_before_allocation() {
        let bytes = [
            1u32.to_le_bytes(),
            7u32.to_le_bytes(),
            1u32.to_le_bytes(),
            8u32.to_le_bytes(),
        ]
        .concat();
        let mut reader = Reader {
            bytes: &bytes,
            position: 0,
            rows: MAX_MIR_TO_KIR_CORRESPONDENCE_RECORDS_V6 - 1,
        };
        assert_eq!(&*reader.rows(4, |r| r.word()).unwrap(), &[7]);
        assert!(matches!(
            reader.rows(4, |r| r.word()),
            Err(E::LimitExceeded)
        ));
    }

    #[test]
    fn writer_aggregate_byte_cap_is_exact_and_stops_before_growth() {
        let mut writer = Writer(Vec::new());
        writer
            .bytes(&vec![0; MAX_MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_BYTES_V6])
            .unwrap();
        assert!(matches!(writer.bytes(&[1]), Err(E::LimitExceeded)));
        assert_eq!(
            writer.0.len(),
            MAX_MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_BYTES_V6
        );
    }
}
