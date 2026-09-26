// Verifier indexes use logical cells, not Rust byte layouts. These are atomic
// debit transcripts. The CFG summary below intentionally has no interior trace.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Cells {
    work: usize,
    retained: usize,
    peak: usize,
}

fn bounded_sort(count: usize, width: usize, trace: &mut Trace) {
    if count >= 2 {
        let height = usize::BITS as usize - (count - 1).leading_zeros() as usize;
        trace.work(product(product(product(4, count), height), width.max(1)));
    }
}

fn upper_bound<T: Ord>(rows: &[T], key: &T, width: usize, trace: &mut Trace) {
    trace.work(1);
    let (mut left, mut right) = (0, rows.len());
    while left < right {
        let middle = left + (right - left) / 2;
        trace.work(width.max(1));
        if rows[middle] > *key {
            right = middle;
        } else {
            left = middle + 1;
        }
    }
    if left != 0 {
        trace.work(width.max(1));
    }
}

fn ordered_radix(keys: &mut [u32], cells_per_row: usize, trace: &mut Trace) {
    if keys.len() < 2 {
        return;
    }
    trace.work(1);
    let mut previous = keys[0];
    let mut ordered = true;
    for &key in &keys[1..] {
        trace.work(2);
        if key < previous {
            ordered = false;
            break;
        }
        previous = key;
    }
    if !ordered {
        trace.work(sum(&[
            product(4, sum(&[product(3, keys.len()), 512])),
            keys.len(),
        ]));
        let scratch = product(keys.len(), cells_per_row);
        trace.reserve(scratch);
        keys.sort();
        trace.release(scratch);
    }
}

struct Numeric {
    keys: Vec<u32>,
    dense: Option<(u32, usize)>,
    retained: usize,
}

impl Numeric {
    fn derive(mut keys: Vec<u32>, cells_per_row: usize, trace: &mut Trace) -> Self {
        let count = keys.len();
        let rows = product(count, cells_per_row);
        trace.work(count);
        trace.reserve(rows);
        let mut dense = None;
        let mut retained = rows;
        if count >= 2 {
            trace.work(product(3, count).checked_sub(2).unwrap());
            let minimum = *keys.iter().min().unwrap();
            let maximum = *keys.iter().max().unwrap();
            trace.work(5);
            let span = usize::try_from(u64::from(maximum) - u64::from(minimum) + 1).unwrap();
            if span <= product(2, count) {
                trace.work(sum(&[product(4, count), product(2, span)]));
                let owner = sum(&[span, 5]);
                trace.reserve(sum(&[owner, rows]));
                keys.sort();
                trace.release(rows);
                dense = Some((minimum, span));
                retained = sum(&[rows, owner]);
            } else {
                ordered_radix(&mut keys, cells_per_row, trace);
            }
        }
        Self {
            keys,
            dense,
            retained,
        }
    }

    fn query(&self, key: u32, trace: &mut Trace) {
        if let Some((minimum, span)) = self.dense {
            trace.work(1);
            if let Some(offset) = key.checked_sub(minimum).map(|x| x as usize) {
                if offset < span {
                    trace.work(2);
                    if self.keys.binary_search(&key).is_ok() {
                        trace.work(1);
                    }
                }
            }
        } else {
            upper_bound(&self.keys, &key, 1, trace);
        }
    }
}

// Exact aggregate for an ascending, dense-ID, entry-reachable straight chain.
// It must not be turned into one atomic work event when predicting inner cuts.
fn chain(blocks: usize) -> Cells {
    assert_ne!(blocks, 0);
    let index = product(36, blocks).checked_sub(7).unwrap();
    let ordered_probe = if blocks == 1 {
        0
    } else {
        product(2, blocks) - 1
    };
    let reachable = sum(&[product(6, blocks), 1]);
    let postorder = sum(&[product(13, blocks), 1]);
    let dominators = if blocks == 1 {
        9
    } else {
        product(17, blocks) - 6
    };
    let intervals = sum(&[product(18, blocks), 6]);
    let reducibility = product(24, blocks) - 4;
    Cells {
        work: sum(&[
            index,
            ordered_probe,
            reachable,
            postorder,
            dominators,
            intervals,
            reducibility,
        ]),
        retained: product(18, blocks) - 7,
        peak: product(24, blocks) - 8,
    }
}

fn type_nodes(ty: &fe2o3_kernel_ir::Type) -> usize {
    use fe2o3_kernel_ir::Type;
    match ty {
        Type::Unit | Type::Scalar(_) => 1,
        Type::Pointer(pointer) => sum(&[1, type_nodes(&pointer.pointee)]),
        _ => panic!("whole-entry fixture type is outside the independently derived profile"),
    }
}

fn ordinary_symbol(name: &str) {
    for prefix in [
        "__fe2o3_ir_amdgpu_diagnostics_gfx942_v1_",
        "__fe2o3_ir_float_v1_",
    ] {
        assert!(
            !name.starts_with(prefix),
            "reserved descriptor path needs a separate derivation"
        );
    }
}

fn function_lookup(names: &[&str], name: &str, trace: &mut Trace) {
    let width = names
        .iter()
        .map(|x| x.len())
        .max()
        .unwrap_or(0)
        .max(name.len())
        + 1;
    upper_bound(names, &name, width, trace);
}

fn operands(operation: &fe2o3_kernel_ir::Operation) -> Vec<u32> {
    use fe2o3_kernel_ir::OperationKind;
    match &operation.kind {
        OperationKind::Constant(_) | OperationKind::Alloca { count: None, .. } => vec![],
        OperationKind::Store { pointer, value, .. } => vec![pointer.0, value.0],
        OperationKind::Load { pointer, .. } => vec![pointer.0],
        OperationKind::Call { arguments, .. } => arguments.iter().map(|x| x.0).collect(),
        _ => panic!("whole-entry fixture operation requires a new source-derived term"),
    }
}

fn function_pass(
    module: &fe2o3_kernel_ir::Module,
    function: &fe2o3_kernel_ir::Function,
    names: &[&str],
) -> Cells {
    use fe2o3_kernel_ir::{OperationKind, Terminator};
    let body = function.body.as_ref().unwrap();
    let blocks = body.blocks.len();
    // This profile has exactly the single-block or forward straight-chain CFG.
    // Its aggregate is kept outside the atomic index transcript.
    for (ordinal, block) in body.blocks.iter().enumerate() {
        assert_eq!(block.id.0 as usize, ordinal);
        if ordinal + 1 < blocks {
            assert!(
                matches!(&block.terminator, Some(Terminator::Branch { target, .. })
                if target.0 as usize == ordinal + 1)
            );
        } else {
            assert!(matches!(block.terminator, Some(Terminator::Return { .. })));
        }
    }
    let cfg = chain(blocks);
    let operations: usize = body.blocks.iter().map(|b| b.operations.len()).sum();
    let mut state = Trace::new(0);
    let block_index = Numeric::derive(body.blocks.iter().map(|b| b.id.0).collect(), 3, &mut state);
    state.work(blocks);
    state.work(blocks);
    state.work(operations);
    state.work(blocks);
    state.work(operations);
    // None identifies a function parameter, so cross-block uses of parameters
    // do not pay a CFG dominance query. This is descriptive fixture data only.
    let mut definitions = Vec::new();
    for (id, ty) in body.parameters.iter().zip(&function.signature.parameters) {
        definitions.push((id.0, ty, None));
    }
    for block in &body.blocks {
        for parameter in &block.parameters {
            definitions.push((parameter.id.0, &parameter.ty, Some(block.id.0)));
        }
        for operation in &block.operations {
            for result in &operation.results {
                definitions.push((result.id.0, &result.ty, Some(block.id.0)));
            }
        }
    }
    let definition_index =
        Numeric::derive(definitions.iter().map(|x| x.0).collect(), 6, &mut state);
    state.work(1);
    let index_cost = state.success();
    let mut pass = Trace::new(0);
    pass.work(5);
    pass.work(blocks);
    units(&mut pass, blocks.saturating_sub(1));
    pass.work(definitions.len());
    for &(_, ty, site) in &definitions {
        if site.is_some() {
            pass.work(5);
            units(&mut pass, type_nodes(ty));
        }
    }
    pass.work(definitions.len());
    units(&mut pass, definitions.len().saturating_sub(1));
    pass.work(blocks);
    pass.work(blocks);
    let definition = |id: u32| definitions.iter().find(|row| row.0 == id).unwrap();
    let use_value = |id: u32, block: u32, pass: &mut Trace| {
        definition_index.query(id, pass);
        if definition(id).2.is_some_and(|site| site != block) {
            // Both block IDs satisfy the actual direct indexed lookup.
            for debit in [1, 1, 1, 1, 4] {
                pass.work(debit);
            }
        }
    };
    for block in &body.blocks {
        pass.work(block.operations.len());
        for operation in &block.operations {
            pass.work(5);
            if let OperationKind::Call { callee, .. } = &operation.kind {
                ordinary_symbol(callee.as_str());
                pass.work(product(2, callee.as_str().len() + 1));
            } else {
                pass.work(1);
            }
            pass.work(1);
            for id in operands(operation) {
                pass.work(1);
                use_value(id, block.id.0, &mut pass);
            }
            pass.work(1); // Registered-kind dispatch, with supported targets None.
            pass.work(1); // Legacy-kind dispatch.
            match &operation.kind {
                OperationKind::Constant(_) => {
                    pass.work(5);
                    pass.work(1);
                    units(&mut pass, type_nodes(&operation.results[0].ty));
                }
                OperationKind::Alloca {
                    element,
                    count: None,
                    ..
                } => {
                    assert!(matches!(element, fe2o3_kernel_ir::Type::Scalar(_)));
                    pass.work(1); // Alignment; scalar storability has no extra node.
                    units(&mut pass, type_nodes(element));
                }
                OperationKind::Load { pointer, .. } => {
                    pass.work(1);
                    definition_index.query(pointer.0, &mut pass);
                    assert!(
                        matches!(definition(pointer.0).1, fe2o3_kernel_ir::Type::Pointer(p)
                        if matches!(*p.pointee, fe2o3_kernel_ir::Type::Scalar(_)))
                    );
                    pass.work(5);
                    pass.work(1);
                    units(&mut pass, type_nodes(&operation.results[0].ty));
                }
                OperationKind::Store { pointer, value, .. } => {
                    pass.work(1);
                    pass.work(1);
                    definition_index.query(pointer.0, &mut pass);
                    assert!(
                        matches!(definition(pointer.0).1, fe2o3_kernel_ir::Type::Pointer(p)
                        if matches!(*p.pointee, fe2o3_kernel_ir::Type::Scalar(_)))
                    );
                    definition_index.query(value.0, &mut pass);
                    units(&mut pass, type_nodes(definition(value.0).1));
                }
                OperationKind::Call { callee, arguments } => {
                    pass.work(product(2, callee.as_str().len() + 1));
                    function_lookup(names, callee.as_str(), &mut pass);
                    let target = module.functions.iter().find(|f| f.id == *callee).unwrap();
                    assert_eq!(arguments.len(), target.signature.parameters.len());
                    pass.work(arguments.len());
                    for (argument, ty) in arguments.iter().zip(&target.signature.parameters) {
                        definition_index.query(argument.0, &mut pass);
                        units(&mut pass, type_nodes(ty));
                    }
                    assert_eq!(operation.results.len(), target.signature.results.len());
                    pass.work(operation.results.len());
                    for ty in &target.signature.results {
                        units(&mut pass, type_nodes(ty));
                    }
                }
                _ => unreachable!(),
            }
        }
        pass.work(5);
        pass.work(1);
        let arguments = match block.terminator.as_ref().unwrap() {
            Terminator::Branch { arguments, .. } => arguments,
            Terminator::Return { values } => values,
            _ => unreachable!(),
        };
        pass.work(arguments.len());
        for argument in arguments {
            use_value(argument.0, block.id.0, &mut pass);
        }
        pass.work(1);
        match block.terminator.as_ref().unwrap() {
            Terminator::Branch { target, arguments } => {
                block_index.query(target.0, &mut pass);
                let target = &body.blocks[target.0 as usize];
                assert_eq!(arguments.len(), target.parameters.len());
                pass.work(arguments.len());
                for (argument, parameter) in arguments.iter().zip(&target.parameters) {
                    definition_index.query(argument.0, &mut pass);
                    units(&mut pass, type_nodes(&parameter.ty));
                }
            }
            Terminator::Return { values } => {
                assert_eq!(values.len(), function.signature.results.len());
                pass.work(values.len());
                for (value, ty) in values.iter().zip(&function.signature.results) {
                    definition_index.query(value.0, &mut pass);
                    units(&mut pass, type_nodes(ty));
                }
            }
            _ => unreachable!(),
        }
    }
    Cells {
        work: sum(&[cfg.work, index_cost.work, pass.success().work]),
        retained: 0,
        peak: cfg.peak.max(sum(&[cfg.retained, index_cost.peak])),
    }
}

// Exact successful shared-verifier component for this closed scalar/private/
// typed-call profile. Canonical encode/decode and borrowed depth preflight are
// separate components; neither is silently folded into this sum.
fn verifier(module: &fe2o3_kernel_ir::Module) -> Cells {
    use fe2o3_kernel_ir::{FunctionRole, OperationKind};
    assert!(!module.id.as_str().is_empty());
    assert!(module.required_capabilities.is_empty());
    let (functions, kernels) = (module.functions.len(), module.kernels.len());
    assert_ne!(functions, 0);
    let mut names: Vec<_> = module.functions.iter().map(|f| f.id.as_str()).collect();
    names.sort();
    let mut kernel_names: Vec<_> = module.kernels.iter().map(|k| k.id.as_str()).collect();
    kernel_names.sort();
    let mut entries: Vec<_> = module.kernels.iter().map(|k| k.entry.as_str()).collect();
    entries.sort();
    let mut trace = Trace::new(0);
    trace.work(functions);
    trace.work(functions);
    trace.reserve(product(2, functions));
    bounded_sort(
        functions,
        names.iter().map(|x| x.len()).max().unwrap() + 2,
        &mut trace,
    );
    trace.work(kernels);
    trace.work(kernels);
    trace.reserve(product(3, kernels));
    bounded_sort(
        kernels,
        kernel_names.iter().map(|x| x.len()).max().unwrap_or(0) + 2,
        &mut trace,
    );
    bounded_sort(
        kernels,
        entries.iter().map(|x| x.len()).max().unwrap_or(0) + 1,
        &mut trace,
    );
    let module_live = sum(&[product(2, functions), product(3, kernels)]);
    let mut peak = module_live;
    trace.work(5);
    trace.work(0);
    trace.work(0);
    trace.work(functions);
    for pair in names.windows(2) {
        trace.work(pair[0].len().max(pair[1].len()) + 1);
        assert_ne!(pair[0], pair[1]);
    }
    trace.work(functions);
    let mut body_work = 0;
    for function in &module.functions {
        ordinary_symbol(function.id.as_str());
        assert!(function.required_capabilities.is_empty());
        assert!(function.body.is_some());
        trace.work(5);
        trace.work(5);
        trace.work(0);
        trace.work(0);
        trace.work(function.signature.parameters.len() + function.signature.results.len());
        for ty in function
            .signature
            .parameters
            .iter()
            .chain(&function.signature.results)
        {
            units(&mut trace, type_nodes(ty));
        }
        trace.work(product(2, function.id.as_str().len() + 1));
        trace.work(1);
        trace.work(1);
        let body = function_pass(module, function, &names);
        body_work = sum(&[body_work, body.work]);
        peak = peak.max(sum(&[module_live, body.peak]));
    }
    trace.work(kernels);
    for pair in kernel_names.windows(2) {
        trace.work(pair[0].len().max(pair[1].len()) + 1);
        assert_ne!(pair[0], pair[1]);
    }
    trace.work(kernels);
    for (ordinal, kernel) in module.kernels.iter().enumerate() {
        assert!(kernel.required_capabilities.is_empty());
        assert_eq!(kernel.domain.rank(), 1);
        trace.work(5);
        trace.work(5);
        trace.work(0);
        trace.work(0);
        trace.work(1);
        trace.work(1);
        function_lookup(&names, kernel.entry.as_str(), &mut trace);
        let root = module
            .functions
            .iter()
            .position(|f| f.id == kernel.entry)
            .unwrap();
        assert_eq!(module.functions[root].role, FunctionRole::KernelEntry);
        assert!(module.functions[root].signature.results.is_empty());
        trace.work(if ordinal == 0 { functions + 3 } else { 3 });
        if ordinal == 0 {
            trace.reserve(product(2, functions));
        }
        let mut pending = vec![root];
        let mut visited = vec![false; functions];
        visited[root] = true;
        while let Some(index) = pending.pop() {
            trace.work(1);
            let body = module.functions[index].body.as_ref().unwrap();
            trace.work(body.blocks.len());
            for block in &body.blocks {
                trace.work(block.operations.len());
                for operation in &block.operations {
                    if let OperationKind::Call { callee, .. } = &operation.kind {
                        function_lookup(&names, callee.as_str(), &mut trace);
                        let target = module
                            .functions
                            .iter()
                            .position(|f| f.id == *callee)
                            .unwrap();
                        if !visited[target] {
                            trace.work(2);
                            visited[target] = true;
                            pending.push(target);
                        }
                    }
                }
            }
        }
    }
    if kernels != 0 {
        trace.release(product(2, functions));
    }
    trace.work(functions);
    for function in &module.functions {
        if function.role == FunctionRole::KernelEntry {
            let width = entries
                .iter()
                .map(|x| x.len())
                .max()
                .unwrap_or(0)
                .max(function.id.as_str().len())
                + 1;
            upper_bound(&entries, &function.id.as_str(), width, &mut trace);
        }
    }
    trace.release(module_live);
    Cells {
        work: sum(&[body_work, trace.success().work]),
        retained: 0,
        peak: peak.max(trace.success().peak),
    }
}

pub(super) fn borrowed_verifier(module: &fe2o3_kernel_ir::Module) -> (usize, usize, usize) {
    use fe2o3_kernel_ir::OperationKind;
    let core = verifier(module);
    let mut visits = 1;
    for function in &module.functions {
        visits = sum(&[visits, 1]);
        for ty in function
            .signature
            .parameters
            .iter()
            .chain(&function.signature.results)
        {
            visits = sum(&[visits, 1, type_nodes(ty)]);
        }
        for block in &function.body.as_ref().unwrap().blocks {
            visits = sum(&[visits, 1]);
            for parameter in &block.parameters {
                visits = sum(&[visits, 1, type_nodes(&parameter.ty)]);
            }
            for operation in &block.operations {
                visits = sum(&[visits, 2]); // Operation visit and kind dispatch.
                for result in &operation.results {
                    visits = sum(&[visits, 1, type_nodes(&result.ty)]);
                }
                if let OperationKind::Alloca { element, .. } = &operation.kind {
                    visits = sum(&[visits, type_nodes(element)]);
                }
            }
        }
    }
    // The final successful root-reference query ends in an equality comparison
    // with this width. Only this actual terminal debit is used for a work cut.
    let last = module
        .functions
        .iter()
        .rev()
        .find(|f| f.role == fe2o3_kernel_ir::FunctionRole::KernelEntry)
        .expect("fixture has a genuine kernel entry");
    let terminal = module
        .kernels
        .iter()
        .map(|k| k.entry.as_str().len())
        .max()
        .unwrap()
        .max(last.id.as_str().len())
        + 1;
    (sum(&[visits, core.work]), core.peak, terminal)
}

#[derive(Default)]
struct Wire {
    bytes: usize,
    tokens: usize,
    text_bytes: usize,
    decoded_payload: usize,
}

impl Wire {
    fn field(&mut self, bytes: usize) {
        self.bytes = sum(&[self.bytes, bytes]);
        self.tokens = sum(&[self.tokens, 1]);
    }
    fn fields(&mut self, widths: &[usize]) {
        for &width in widths {
            self.field(width);
        }
    }
    fn text(&mut self, text: &str) {
        self.fields(&[4, text.len()]);
        self.text_bytes = sum(&[self.text_bytes, text.len()]);
        let mut independent = String::new();
        independent.try_reserve_exact(text.len()).unwrap();
        assert_eq!(
            independent.capacity(),
            text.len(),
            "pinned String exact-capacity premise"
        );
        self.decoded_payload = sum(&[self.decoded_payload, independent.capacity()]);
    }
    fn vector<T>(&mut self, count: usize) {
        self.field(4);
        self.decoded_payload = sum(&[self.decoded_payload, exact_capacity::<T>(count)]);
    }
    fn ty(&mut self, ty: &fe2o3_kernel_ir::Type) {
        use fe2o3_kernel_ir::Type;
        match ty {
            Type::Unit => self.field(1),
            Type::Scalar(_) => self.fields(&[1, 1]),
            Type::Pointer(pointer) => {
                self.fields(&[1, 1, 1]);
                self.ty(&pointer.pointee);
                self.decoded_payload = sum(&[self.decoded_payload, size_of::<Type>()]);
            }
            _ => panic!("new type requires an independently derived wire term"),
        }
    }
    fn values(&mut self, values: &[fe2o3_kernel_ir::ValueId]) {
        self.vector::<fe2o3_kernel_ir::ValueId>(values.len());
        for _ in values {
            self.field(4);
        }
    }
    fn definition(&mut self, definition: &fe2o3_kernel_ir::ValueDef) {
        self.field(4);
        self.ty(&definition.ty);
    }
    fn module(module: &fe2o3_kernel_ir::Module) -> Self {
        use fe2o3_kernel_ir::{
            BasicBlock, Constant, Function, Kernel, LaunchExtent, Operation, OperationKind,
            Terminator, Type, ValueDef,
        };
        let mut out = Self::default();
        out.fields(&[8, 2, 2, 4, 4]);
        out.text(module.id.as_str());
        out.vector::<Function>(module.functions.len());
        out.vector::<Kernel>(module.kernels.len());
        assert!(module.required_capabilities.is_empty());
        out.field(4);
        for function in &module.functions {
            out.text(function.id.as_str());
            out.vector::<Type>(function.signature.parameters.len());
            for ty in &function.signature.parameters {
                out.ty(ty);
            }
            out.vector::<Type>(function.signature.results.len());
            for ty in &function.signature.results {
                out.ty(ty);
            }
            out.field(1);
            let body = function.body.as_ref().unwrap();
            out.values(&body.parameters);
            out.vector::<BasicBlock>(body.blocks.len());
            for block in &body.blocks {
                out.field(4);
                out.vector::<ValueDef>(block.parameters.len());
                for parameter in &block.parameters {
                    out.definition(parameter);
                }
                out.vector::<Operation>(block.operations.len());
                for operation in &block.operations {
                    out.vector::<ValueDef>(operation.results.len());
                    for result in &operation.results {
                        out.definition(result);
                    }
                    out.field(1);
                    match &operation.kind {
                        OperationKind::Constant(Constant::U64(_) | Constant::Index(_)) => {
                            out.fields(&[1, 8])
                        }
                        OperationKind::Alloca {
                            element,
                            count: None,
                            ..
                        } => {
                            out.ty(element);
                            out.fields(&[1, 1, 4]);
                        }
                        OperationKind::Load { .. } => out.fields(&[4, 1, 4, 1]),
                        OperationKind::Store { .. } => out.fields(&[4, 4, 1, 4, 1]),
                        OperationKind::Call { callee, arguments } => {
                            out.text(callee.as_str());
                            out.values(arguments);
                        }
                        _ => panic!("new operation requires an independently derived wire term"),
                    }
                }
                out.fields(&[1, 1]); // Present terminator and its kind.
                match block.terminator.as_ref().unwrap() {
                    Terminator::Branch { arguments, .. } => {
                        out.field(4);
                        out.values(arguments);
                    }
                    Terminator::Return { values } => out.values(values),
                    _ => panic!("new terminator requires an independently derived wire term"),
                }
            }
            assert!(function.required_capabilities.is_empty());
            out.field(4);
        }
        for kernel in &module.kernels {
            out.text(kernel.id.as_str());
            out.text(kernel.entry.as_str());
            assert_eq!(kernel.domain.rank(), 1);
            out.field(1);
            for extent in kernel.domain.extents() {
                out.field(1);
                if matches!(extent, LaunchExtent::Static(_)) {
                    out.field(4);
                }
            }
            out.field(1);
            if kernel.workgroup_size.is_some() {
                out.fields(&[4, 4, 4]);
            }
            assert!(kernel.required_capabilities.is_empty());
            out.field(4);
        }
        out
    }
}

pub(super) fn inverse(module: &fe2o3_kernel_ir::Module) -> (usize, usize, usize, usize, usize) {
    use fe2o3_kernel_ir::{
        FunctionId, VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_DOMAIN_V1,
        VerifiedCanonicalKernelIrModuleV12,
    };
    let wire = Wire::module(module);
    let semantic = verifier(module);
    let (functions, kernels) = (module.functions.len(), module.kernels.len());
    let key_width = module
        .functions
        .iter()
        .map(|f| f.id.as_str().len())
        .chain(module.kernels.iter().map(|k| k.entry.as_str().len()))
        .max()
        .unwrap_or(0)
        + 1;
    let role = sum(&[
        kernels,
        functions,
        product(
            sum(&[
                product(kernels, kernels + 1) / 2,
                product(functions, kernels + 1),
            ]),
            key_width,
        ),
        kernels,
    ]);
    // Count(false), count(true), materialize(false); decode, count(false),
    // compare(true); structural equality; identity hash. There are three role
    // checks, three counter walks and one extra comparison action per field.
    let hash = sum(&[
        wire.bytes,
        14,
        VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_DOMAIN_V1.len(),
    ]);
    let work = sum(&[
        product(4, wire.tokens),
        product(4, wire.bytes),
        9,
        product(2, wire.text_bytes),
        product(3, role),
        semantic.work,
        hash,
    ]);
    let retained = sum(&[
        size_of::<VerifiedCanonicalKernelIrModuleV12>(),
        wire.bytes,
        wire.decoded_payload,
    ]);
    // The source decoder admits its established conservative B-tree payload,
    // not an ABI-sized tree-node mirror. Empty capability trees add zero.
    let role_scratch = product(
        kernels,
        sum(&[
            product(
                3,
                sum(&[
                    product(11, size_of::<&FunctionId>()),
                    product(14, size_of::<usize>()),
                ]),
            ),
            product(4, size_of::<&FunctionId>()),
        ]),
    );
    let peak = sum(&[retained, role_scratch.max(semantic.peak)]);
    (work, retained, peak, hash, wire.bytes)
}

#[test]
fn whole_admission_dense_definition_index_keeps_bucket_owner_and_scratch_distinct() {
    let mut trace = Trace::new(23);
    let index = Numeric::derive(vec![0, 1, 2], 6, &mut trace);
    // Rows18; census7; decision5; scatter12+6. Bucket owner3+5
    // overlaps the existing rows and the temporary second18-cell row array.
    assert_eq!(index.retained, 26);
    assert_eq!(trace.success().work, 3 + 7 + 5 + 18);
    assert_eq!(trace.success().storage, 23 + 26);
    assert_eq!(trace.success().peak, 23 + 18 + 8 + 18);
    let denied = trace.run(usize::MAX, 23 + 18 + 8 + 18 - 1);
    assert_eq!(denied.work, 33);
    assert_eq!(denied.first_storage, Some(67));
    assert_eq!(denied.storage, 23 + 18);
    index.query(1, &mut trace);
    assert_eq!(trace.success().work, 37);
    index.query(7, &mut trace);
    assert_eq!(trace.success().work, 38);
    trace.release(index.retained);
    assert_eq!(trace.success().storage, 23);
}

#[test]
fn whole_admission_singleton_and_sparse_indexes_preserve_their_actual_search_schedule() {
    let mut single = Trace::new(23);
    let index = Numeric::derive(vec![0], 3, &mut single);
    index.query(0, &mut single);
    assert_eq!(single.success().work, 1 + 1 + 1 + 1);
    assert_eq!(single.success().peak, 26);

    let mut sparse = Trace::new(23);
    let index = Numeric::derive(vec![3, 100], 6, &mut sparse);
    // Rows2, min/max4, decision5, already ordered radix probe3.
    assert_eq!(sparse.success().work, 14);
    assert_eq!(index.retained, 12);
    assert_eq!(sparse.success().peak, 35);
    index.query(100, &mut sparse);
    assert_eq!(sparse.success().work, 17);
    assert_eq!(sparse.run(16, usize::MAX).first_work, Some(17));
}

#[test]
fn whole_admission_chain_summary_is_not_a_synthetic_first_denial_oracle() {
    assert_eq!(
        chain(1),
        Cells {
            work: 103,
            retained: 11,
            peak: 16
        }
    );
    assert_eq!(
        chain(2),
        Cells {
            work: 222,
            retained: 29,
            peak: 40
        }
    );
    let mut trace = Trace::new(0);
    bounded_sort(3, 7, &mut trace);
    assert_eq!(trace.success().work, 4 * 3 * 2 * 7);
    assert_eq!(trace.run(167, 0).work, 0);
    assert_eq!(trace.run(167, 0).first_work, Some(168));
}

// Complete structural coverage, not source or policy authority. The operation
// profile has no capability requirements; ordinary calls still visit both exact
// descriptor rosters before determining that no capability is emitted.
pub(super) struct Ranked<'a> {
    module: &'a fe2o3_kernel_ir::Module,
    graph: Graph<'a>,
    facts: Vec<usize>,
    capability_visits: Vec<usize>,
}

impl<'a> Ranked<'a> {
    pub(super) fn new(module: &'a fe2o3_kernel_ir::Module, facts: Vec<usize>) -> Self {
        use fe2o3_kernel_ir::OperationKind;
        assert!(module.required_capabilities.is_empty());
        assert!(
            module
                .kernels
                .iter()
                .all(|k| k.required_capabilities.is_empty())
        );
        let mut capability_visits = Vec::new();
        for function in &module.functions {
            assert!(function.required_capabilities.is_empty());
            for block in &function.body.as_ref().unwrap().blocks {
                for operation in &block.operations {
                    let work = match &operation.kind {
                        OperationKind::Constant(_)
                        | OperationKind::Load { .. }
                        | OperationKind::Store { .. } => 1,
                        OperationKind::Alloca { address_space, .. } => {
                            assert_eq!(*address_space, fe2o3_kernel_ir::AddressSpace::Private);
                            1
                        }
                        OperationKind::Call { callee, .. } => {
                            assert!(module.functions.iter().any(|f| f.id == *callee));
                            assert!(!callee.as_str().starts_with("__fe2o3_ir_"));
                            // Eight AMDGPU diagnostic plus 27 floating-point
                            // descriptor names. Neither roster is queried here.
                            sum(&[product(8 + 27, callee.as_str().len() + 2), 1])
                        }
                        _ => panic!("closed structural resource profile"),
                    };
                    capability_visits.push(work);
                }
            }
        }
        Self {
            module,
            graph: Graph::module(module),
            facts,
            capability_visits,
        }
    }

    fn count(&self) -> usize {
        let g = &self.graph;
        sum(&[
            1,
            g.entries.len(),
            g.functions.len(),
            g.blocks.len(),
            g.definitions.len(),
            g.operations,
            g.used_definitions.len(),
            g.successor_blocks.len(),
            g.edge_arguments,
            g.effects,
            g.callees.len(),
            self.facts.len(),
        ])
    }

    fn visit(&self, checked: bool, trace: &mut Trace) {
        let g = &self.graph;
        units(trace, 1 + g.entries.len());
        for _ in 0..g.functions.len() + g.blocks.len() {
            trace.work(1);
            trace.work(1);
        }
        units(trace, g.definitions.len());
        for _ in 0..g.operations {
            trace.work(1);
            trace.work(1);
        }
        units(trace, g.used_definitions.len());
        if checked {
            for function in &self.module.functions {
                for block in &function.body.as_ref().unwrap().blocks {
                    trace.work(1);
                    if let Some(fe2o3_kernel_ir::Terminator::Branch { arguments, .. }) =
                        &block.terminator
                    {
                        trace.work(sum(&[2, arguments.len()]));
                        trace.work(1);
                    }
                }
            }
        } else {
            for _ in &g.successor_blocks {
                trace.work(3);
                trace.work(1);
            }
        }
        for _ in 0..g.edge_arguments {
            if checked {
                trace.work(1);
            }
            trace.work(1);
        }
        for _ in 0..g.effects {
            trace.work(1);
            trace.work(1);
        }
        units(trace, g.callees.len());
        units(trace, g.entries.len() + g.functions.len());
        for work in &self.capability_visits {
            trace.work(*work);
            trace.work(1);
        }
        for facts in &self.facts {
            trace.work(if checked { 2 } else { 1 });
            if checked {
                units(trace, *facts);
            }
            trace.work(1);
        }
        if checked {
            trace.work(1);
        }
    }

    pub(super) fn candidate(&self, trace: &mut Trace) -> usize {
        use fe2o3_kernel_analysis::{CanonicalRankedCandidateV1, CanonicalRankedCoverageRowV1};
        let scope = trace.enter(&[]);
        trace.work(1);
        self.visit(false, trace);
        let header = size_of::<CanonicalRankedCandidateV1<'_, '_, '_>>();
        trace.reserve(header);
        let payload = exact_capacity::<CanonicalRankedCoverageRowV1>(self.count());
        trace.reserve(payload);
        trace.work(self.count());
        trace.reserve(0);
        self.visit(false, trace);
        trace.leave(scope);
        sum(&[header, payload])
    }

    pub(super) fn checked(&self, trace: &mut Trace, callback: impl FnOnce(&mut Trace)) {
        use fe2o3_kernel_analysis::{CanonicalRankedViewErrorV1, CheckedCanonicalRankedViewV1};
        use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1;
        trace.work(1);
        trace.work(1 + self.facts.len());
        // Host/compiler-qualified equivalence only. The actual private type is
        // asserted by the required canonical_ranked_view companion, not exposed.
        type Accounting = (
            usize,
            CanonicalKernelIrWorkLedgerIdentityV1,
            usize,
            usize,
            Option<CanonicalRankedViewErrorV1>,
            bool,
        );
        let scope = trace.enter(&[]);
        trace.reserve(sum(&[
            size_of::<Accounting>(),
            size_of::<CheckedCanonicalRankedViewV1<'_, '_, '_, '_>>(),
        ]));
        self.visit(true, trace);
        trace.mark("original structural view callback");
        callback(trace);
        trace.leave(scope);
    }
}
