//! Root-qualified source/call joins for independently reconstructed native values.
use super::native_helper_value_template_v1::{Ledger, Meter, Template, sort_metered, vector};
use super::native_helper_value_templates_v1 as native;
use super::*;

#[cfg(test)]
#[path = "native_helper_value_context_v1_tests.rs"]
mod tests;
use fe2o3_kernel_ir::FunctionRole;
use fe2o3_mir_model::semantic_mir_v1::{SemanticCanonAbiV1, SemanticExternAbiV1};

type Error = &'static str;
type Key = (usize, BlockId, usize);

struct Call<'a> {
    key: Key,
    operation: &'a Operation,
    source: &'a SemanticDirectCallV1,
    target: usize,
    local: bool,
}

struct Anchor<'a> {
    key: Key,
    source: &'a SemanticDirectCallV1,
    local: bool,
    seen: bool,
}

pub(super) struct NativeHelperValues<'a> {
    module: &'a Module,
    semantic: &'a AdmittedInertSemanticMirV1,
    correspondence: &'a SemanticKirCorrespondenceV1,
    root: SemanticFunctionIdV1,
    entry: usize,
    source_functions: Vec<Option<usize>>,
    native_functions: Vec<Option<usize>>,
    calls: Vec<Call<'a>>,
    templates: Vec<Option<Template>>,
    storage: usize,
    ledger: Ledger,
    live_floor: usize,
}

fn lookup(length: usize, width: usize, meter: &mut dyn Meter) -> Result<(), Error> {
    let depth = usize::BITS as usize - length.leading_zeros() as usize;
    meter.work(
        depth
            .checked_add(1)
            .and_then(|n| n.checked_mul(width))
            .ok_or("native helper lookup work overflow")?,
    )
}

fn scalar_type(semantic: &AdmittedInertSemanticMirV1, ty: SemanticTypeIdV1) -> Result<Type, Error> {
    let shape = semantic
        .types()
        .get(ty.index() as usize)
        .ok_or("native helper source type outside owner")?
        .shape();
    let scalar = match shape {
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool) => ScalarType::Bool,
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }) => ScalarType::F32,
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 64 }) => ScalarType::F64,
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }) => {
            match (*signed, *bits) {
                (true, 8) => ScalarType::I8,
                (true, 16) => ScalarType::I16,
                (true, 32) => ScalarType::I32,
                (true, 64) => ScalarType::I64,
                (false, 8) => ScalarType::U8,
                (false, 16) => ScalarType::U16,
                (false, 32) => ScalarType::U32,
                (false, 64) => ScalarType::U64,
                _ => return Err("native helper source integer outside exact scalar widths"),
            }
        }
        _ => return Err("native helper source ABI has non-scalar layout"),
    };
    Ok(Type::Scalar(scalar))
}

fn signature(
    semantic: &AdmittedInertSemanticMirV1,
    source: usize,
    function: &Function,
    meter: &mut dyn Meter,
) -> Result<(), Error> {
    meter.work(16)?;
    let source = semantic
        .functions()
        .get(source)
        .ok_or("native helper source function outside owner")?;
    let abi = source.abi();
    if source.role() != SemanticFunctionRoleV1::InternalHelper
        || source.export().is_some()
        || function.role != FunctionRole::InternalHelper
        || function.body.is_none()
        || abi.canon_abi() != SemanticCanonAbiV1::Rust
        || abi.extern_abi() != SemanticExternAbiV1::Rust
        || abi.can_unwind()
        || abi.c_variadic()
        || !abi.hidden_arguments().is_empty()
        || abi.fixed_count() as usize != abi.arguments().len()
        || abi.arguments().len() != abi.source_input_types().len()
        || abi.arguments().len() != abi.source_argument_ownership().len()
        || abi.arguments().len() != function.signature.parameters.len()
        || function.signature.results.len() != 1
    {
        return Err("native helper source/native role or direct Rust ABI mismatch");
    }
    for (((argument, ty), ownership), actual) in abi
        .arguments()
        .iter()
        .zip(abi.source_input_types())
        .zip(abi.source_argument_ownership())
        .zip(&function.signature.parameters)
    {
        meter.work(9)?;
        if !argument.is_source()
            || argument.ty() != *ty
            || argument.value().adjusted().is_some()
            || argument.value().pointee_override().is_some()
            || !matches!(argument.mode(), SemanticAbiPassModeV1::Direct(_))
            || *ownership != SemanticSourceArgumentOwnershipV1::ByValue
            || scalar_type(semantic, *ty)? != *actual
        {
            return Err("native helper exact scalar parameter ABI mismatch");
        }
    }
    let result = abi.return_value();
    if result.source_ty() != abi.source_output_type()
        || result.adjusted().is_some()
        || result.pointee_override().is_some()
        || !matches!(result.mode(), SemanticAbiPassModeV1::Direct(_))
        || scalar_type(semantic, abi.source_output_type())? != function.signature.results[0]
    {
        return Err("native helper exact scalar return ABI mismatch");
    }
    Ok(())
}

impl NativeHelperValues<'_> {
    fn check_live(&self, meter: &mut dyn Meter) -> Result<(), Error> {
        if self.ledger != meter.identity()? {
            return Err("native helper foreign ledger");
        }
        if meter.storage()? < self.live_floor {
            return Err("native helper live storage floor lost");
        }
        Ok(())
    }

    pub(super) fn root_call<'s>(
        &'s self,
        function: &Function,
        location: FunctionOperationLocation,
        operation: &Operation,
        meter: &mut dyn Meter,
    ) -> Result<&'s Template, Error> {
        self.check_live(meter)?;
        self.call(
            self.module,
            self.semantic,
            self.correspondence,
            self.root,
            function,
            location,
            operation,
            meter,
        )
    }

    fn row(&self, key: Key, meter: &mut dyn Meter) -> Result<&Call<'_>, Error> {
        self.check_live(meter)?;
        lookup(self.calls.len(), 3, meter)?;
        let index = self
            .calls
            .binary_search_by_key(&key, |row| row.key)
            .map_err(|_| "native call has no exact source anchor")?;
        Ok(&self.calls[index])
    }

    fn target(
        &self,
        key: Key,
        operation: &Operation,
        meter: &mut dyn Meter,
    ) -> Result<usize, Error> {
        self.check_live(meter)?;
        meter.work(5)?;
        let row = self.row(key, meter)?;
        if !std::ptr::eq(row.operation, operation) || !row.local {
            return Err("native call occurrence or result transport substituted");
        }
        let OperationKind::Call { callee, arguments } = &operation.kind else {
            return Err("native value is not a call");
        };
        let function = &self.module.functions[row.target];
        meter.work(
            callee
                .as_str()
                .len()
                .checked_add(1)
                .ok_or("native helper name work overflow")?,
        )?;
        if callee != &function.id
            || arguments.len() != row.source.arguments().len()
            || operation.results.len() != 1
            || function.signature.results.len() != 1
            || function.signature.results.first() != Some(&operation.results[0].ty)
        {
            return Err("native call callee, arity or result changed");
        }
        let source = self.native_functions[row.target]
            .and_then(|index| self.semantic.functions().get(index))
            .ok_or("native helper exact callee source missing")?;
        let destination = row
            .source
            .destination()
            .ok_or("native helper source result missing")?;
        if source.abi().source_input_types().len() != row.source.arguments().len()
            || destination.place().ty() != source.abi().source_output_type()
            || !destination.place().projections().is_empty()
            || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
        {
            return Err("native helper source call transport mismatch");
        }
        for (argument, expected) in row
            .source
            .arguments()
            .iter()
            .zip(source.abi().source_input_types())
        {
            meter.work(1)?;
            if argument.ty() != *expected {
                return Err("native helper source call operand type mismatch");
            }
        }
        Ok(row.target)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn call<'s>(
        &'s self,
        module: &Module,
        semantic: &AdmittedInertSemanticMirV1,
        correspondence: &SemanticKirCorrespondenceV1,
        root: SemanticFunctionIdV1,
        function: &Function,
        location: FunctionOperationLocation,
        operation: &Operation,
        meter: &mut dyn Meter,
    ) -> Result<&'s Template, Error> {
        self.check_live(meter)?;
        meter.work(6)?;
        if !std::ptr::eq(self.module, module)
            || !std::ptr::eq(self.semantic, semantic)
            || !std::ptr::eq(self.correspondence, correspondence)
            || root != self.root
            || !std::ptr::eq(&self.module.functions[self.entry], function)
        {
            return Err("native helper module, source owner, root or caller substitution");
        }
        let target = self.target(
            (self.entry, location.block, location.operation_index),
            operation,
            meter,
        )?;
        self.templates[target]
            .as_ref()
            .ok_or("unresolved actual native helper return")
    }

    fn destroy(self, meter: &mut dyn Meter) -> Result<(), Error> {
        self.check_live(meter)?;
        let Self {
            templates,
            source_functions,
            native_functions,
            calls,
            storage,
            ..
        } = self;
        for template in templates.into_iter().flatten() {
            template.destroy(meter)?;
        }
        drop(source_functions);
        drop(native_functions);
        drop(calls);
        meter.release(storage)
    }
}

fn index_functions(
    context: &mut NativeHelperValues<'_>,
    meter: &mut dyn Meter,
) -> Result<(), Error> {
    let (mut names, storage) = vector(context.module.functions.len(), meter)?;
    let result = (|| {
        let mut width = 1;
        for (index, function) in context.module.functions.iter().enumerate() {
            meter.work(
                function
                    .id
                    .as_str()
                    .len()
                    .checked_add(1)
                    .ok_or("native helper identity work overflow")?,
            )?;
            width = width.max(
                function
                    .id
                    .as_str()
                    .len()
                    .checked_add(1)
                    .ok_or("native helper identity width overflow")?,
            );
            names.push((function.id.as_str(), index));
        }
        sort_metered(&mut names, width, meter, |a, b| a.0.cmp(b.0))?;
        for pair in names.windows(2) {
            meter.work(width)?;
            if pair[0].0 == pair[1].0 {
                return Err("duplicate native function identity");
            }
        }
        let mut entry = None;
        for row in context.correspondence.lowered_functions() {
            meter.work(3)?;
            if row.correspondence_owner() != context.root {
                continue;
            }
            lookup(names.len(), width, meter)?;
            let index = names
                .binary_search_by_key(&row.kernel_ir_function().as_str(), |r| r.0)
                .map_err(|_| "source helper roster missing native function")?;
            let index = names[index].1;
            let source = row.semantic_function().index() as usize;
            let target = context
                .source_functions
                .get_mut(source)
                .ok_or("source helper roster outside owner")?;
            if target.replace(index).is_some()
                || context.native_functions[index].replace(source).is_some()
            {
                return Err("duplicate source/native helper association");
            }
            match row.role() {
                SemanticKirFunctionRoleV1::KernelEntry => {
                    if context.module.functions[index].role != FunctionRole::KernelEntry
                        || entry.replace(index).is_some()
                    {
                        return Err("native helper root entry mismatch");
                    }
                }
                SemanticKirFunctionRoleV1::InternalHelper => {
                    if context.module.functions[index].role != FunctionRole::InternalHelper {
                        return Err("native helper roster role mismatch");
                    }
                }
            }
        }
        if entry != Some(context.entry) {
            return Err("native helper correspondence selects another root entry");
        }
        Ok(())
    })();
    drop(names);
    meter.release(storage)?;
    result
}

fn index_calls<'a>(
    context: &mut NativeHelperValues<'a>,
    meter: &mut dyn Meter,
) -> Result<(), Error> {
    let (mut blocks, block_storage) = vector(context.correspondence.blocks.len(), meter)?;
    let (mut anchors, anchor_storage) =
        match vector(context.correspondence.call_returns.len(), meter) {
            Ok(value) => value,
            Err(error) => {
                drop(blocks);
                meter.release(block_storage)?;
                return Err(error);
            }
        };
    let result = (|| {
        for row in &context.correspondence.blocks {
            meter.work(3)?;
            if row.correspondence_owner() == context.root {
                blocks.push((
                    (
                        row.semantic_function().index(),
                        row.semantic_block().index(),
                    ),
                    row.kernel_ir_block(),
                ));
            }
        }
        sort_metered(&mut blocks, 2, meter, |a, b| a.0.cmp(&b.0))?;
        for pair in blocks.windows(2) {
            meter.work(2)?;
            if pair[0].0 == pair[1].0 {
                return Err("duplicate source/native helper block");
            }
        }
        for row in &context.correspondence.call_returns {
            meter.work(6)?;
            if row.correspondence_owner != context.root {
                continue;
            }
            let SemanticKirCallReturnKindV1::Call {
                arguments_first,
                call_operation,
                destination_end,
                destination,
                ..
            } = row.kind
            else {
                continue;
            };
            let source_index = row.semantic_function.index() as usize;
            let source = context
                .semantic
                .functions()
                .get(source_index)
                .ok_or("helper call anchor source outside owner")?;
            let Some(SemanticTerminatorKindV1::Call(call)) = source
                .blocks()
                .get(row.semantic_block.index() as usize)
                .map(|b| b.terminator().kind())
            else {
                return Err("helper call anchor has no source Call");
            };
            if !matches!(
                context
                    .semantic
                    .callables()
                    .get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::Defined { .. })
            ) {
                continue;
            }
            if arguments_first > call_operation || destination_end <= call_operation {
                return Err("helper call anchor has an invalid operation span");
            }
            let native = context
                .source_functions
                .get(source_index)
                .copied()
                .flatten()
                .ok_or("helper call anchor lacks root-qualified function")?;
            let key = (row.semantic_function.index(), row.semantic_block.index());
            lookup(blocks.len(), 2, meter)?;
            let block = blocks
                .binary_search_by_key(&key, |row| row.0)
                .map_err(|_| "helper call anchor lacks source/native block")?;
            anchors.push(Anchor {
                key: (native, blocks[block].1, call_operation as usize),
                source: call,
                local: matches!(destination, SemanticKirCallDestinationV1::Local),
                seen: false,
            });
        }
        sort_metered(&mut anchors, 3, meter, |a, b| a.key.cmp(&b.key))?;
        for pair in anchors.windows(2) {
            meter.work(3)?;
            if pair[0].key == pair[1].key {
                return Err("duplicate helper call anchor");
            }
        }
        for (function_index, function) in context.module.functions.iter().enumerate() {
            meter.work(2)?;
            if context.native_functions[function_index].is_none() {
                continue;
            }
            let body = function
                .body
                .as_ref()
                .ok_or("root-qualified native function is external")?;
            for block in &body.blocks {
                meter.work(1)?;
                for (operation_index, operation) in block.operations.iter().enumerate() {
                    meter.work(2)?;
                    let OperationKind::Call { callee, arguments } = &operation.kind else {
                        continue;
                    };
                    let key = (function_index, block.id, operation_index);
                    lookup(anchors.len(), 3, meter)?;
                    let Ok(anchor) = anchors.binary_search_by_key(&key, |row| row.key) else {
                        continue;
                    };
                    let anchor = &mut anchors[anchor];
                    if std::mem::replace(&mut anchor.seen, true) {
                        return Err("reused helper call anchor");
                    }
                    let SemanticCallableDeclV1::Defined {
                        function: source_target,
                    } = context.semantic.callables()[anchor.source.callee().index() as usize]
                    else {
                        return Err("helper source Call target changed");
                    };
                    let target = context
                        .source_functions
                        .get(source_target.index() as usize)
                        .copied()
                        .flatten()
                        .ok_or("helper callee lacks root-qualified instance")?;
                    let actual = &context.module.functions[target];
                    meter.work(
                        callee
                            .as_str()
                            .len()
                            .checked_add(1)
                            .ok_or("native helper callee work overflow")?,
                    )?;
                    if callee != &actual.id || arguments.len() != anchor.source.arguments().len() {
                        return Err("native/source helper call callee or arity mismatch");
                    }
                    context.calls.push(Call {
                        key,
                        operation,
                        source: anchor.source,
                        target,
                        local: anchor.local,
                    });
                }
            }
        }
        meter.work(anchors.len())?;
        if anchors.iter().any(|anchor| !anchor.seen) {
            return Err("helper source call anchor has no actual native Call");
        }
        sort_metered(&mut context.calls, 3, meter, |a, b| a.key.cmp(&b.key))?;
        Ok(())
    })();
    drop(anchors);
    drop(blocks);
    meter.release(anchor_storage)?;
    meter.release(block_storage)?;
    result
}

struct Pending {
    function: usize,
    block: usize,
    operation: usize,
    invalid: bool,
}

fn build(context: &mut NativeHelperValues<'_>, meter: &mut dyn Meter) -> Result<(), Error> {
    let count = context.module.functions.len();
    let (mut states, state_storage) = vector(count, meter)?;
    states.resize(count, 0u8);
    let (mut pending, pending_storage) = match vector::<Pending>(count, meter) {
        Ok(value) => value,
        Err(error) => {
            drop(states);
            meter.release(state_storage)?;
            return Err(error);
        }
    };
    let result = (|| {
        for start_index in 0..context.calls.len() {
            meter.work(2)?;
            let start = context.calls[start_index].target;
            if context.calls[start_index].key.0 != context.entry || states[start] != 0 {
                continue;
            }
            states[start] = 1;
            pending.push(Pending {
                function: start,
                block: 0,
                operation: 0,
                invalid: false,
            });
            while let Some(frame) = pending.last_mut() {
                meter.work(5)?;
                let function = &context.module.functions[frame.function];
                let body = function
                    .body
                    .as_ref()
                    .ok_or("native helper has no actual body")?;
                if let Some(block) = body.blocks.get(frame.block) {
                    if let Some(operation) = block.operations.get(frame.operation) {
                        let location = (frame.function, block.id, frame.operation);
                        frame.operation += 1;
                        if !matches!(operation.kind, OperationKind::Call { .. }) {
                            continue;
                        }
                        let target = match context.target(location, operation, meter) {
                            Ok(target) => target,
                            Err(error) if meter.exhausted() => return Err(error),
                            Err(_) => {
                                frame.invalid = true;
                                continue;
                            }
                        };
                        match states[target] {
                            0 => {
                                states[target] = 1;
                                pending.push(Pending {
                                    function: target,
                                    block: 0,
                                    operation: 0,
                                    invalid: false,
                                });
                            }
                            1 | 3 => frame.invalid = true,
                            2 => {}
                            _ => return Err("invalid native helper traversal state"),
                        }
                        continue;
                    }
                    frame.block += 1;
                    frame.operation = 0;
                    continue;
                }
                let finished = pending.pop().ok_or("native helper stack underflow")?;
                let template = if finished.invalid {
                    None
                } else {
                    let source = context.native_functions[finished.function]
                        .ok_or("native helper source association missing")?;
                    let result =
                        signature(context.semantic, source, function, meter).and_then(|()| {
                            native::derive(
                                function,
                                |location, operation, meter| {
                                    let target = context.target(
                                        (
                                            finished.function,
                                            location.block,
                                            location.operation_index,
                                        ),
                                        operation,
                                        meter,
                                    )?;
                                    context.templates[target]
                                        .as_ref()
                                        .ok_or("unresolved nested native helper")
                                },
                                meter,
                            )
                        });
                    match result {
                        Ok(template) => Some(template),
                        Err(error) if meter.exhausted() => return Err(error),
                        Err(_) => None,
                    }
                };
                let state = if template.is_some() { 2 } else { 3 };
                if let Some(template) = &template {
                    // DFS and derive scratch are temporary, not retained rows.
                    context.live_floor = context
                        .live_floor
                        .checked_add(template.retained_storage())
                        .ok_or("native helper retained floor overflow")?;
                }
                states[finished.function] = state;
                context.templates[finished.function] = template;
                if state == 3
                    && let Some(parent) = pending.last_mut()
                {
                    parent.invalid = true;
                }
            }
        }
        Ok(())
    })();
    drop(pending);
    drop(states);
    meter.release(pending_storage)?;
    meter.release(state_storage)?;
    result
}

pub(super) fn with_native_helper_values<R>(
    semantic: &AdmittedInertSemanticMirV1,
    module: &Module,
    correspondence: &SemanticKirCorrespondenceV1,
    root: SemanticFunctionIdV1,
    entry: &Function,
    meter: &mut dyn Meter,
    action: impl for<'s> FnOnce(&'s NativeHelperValues<'_>, &mut dyn Meter) -> Result<R, Error>,
) -> Result<R, Error> {
    let ledger = meter.identity()?;
    let floor = meter.storage()?;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        meter.work(8)?;
        if correspondence.semantic_sha256() != semantic.semantic_sha256().as_bytes()
            || correspondence.function_count() != semantic.functions().len()
        {
            return Err("native helper source/correspondence owner mismatch");
        }
        let mut found = None;
        for (index, function) in module.functions.iter().enumerate() {
            meter.work(1)?;
            if std::ptr::eq(function, entry) {
                found = Some(index);
            }
        }
        let entry = found.ok_or("native helper entry is foreign to module")?;
        let header = std::mem::size_of::<NativeHelperValues<'_>>();
        meter.reserve(header)?;
        let (mut source_functions, a) = vector(semantic.functions().len(), meter)?;
        source_functions.resize(semantic.functions().len(), None);
        let (mut native_functions, b) = vector(module.functions.len(), meter)?;
        native_functions.resize(module.functions.len(), None);
        let (mut templates, c) = vector(module.functions.len(), meter)?;
        templates.resize_with(module.functions.len(), || None);
        let (calls, d) = vector(correspondence.call_returns.len(), meter)?;
        let storage = [header, a, b, c, d]
            .into_iter()
            .try_fold(0usize, |sum, n| {
                sum.checked_add(n)
                    .ok_or("native helper header storage overflow")
            })?;
        let mut context = NativeHelperValues {
            module,
            semantic,
            correspondence,
            root,
            entry,
            source_functions,
            native_functions,
            calls,
            templates,
            storage,
            ledger: meter.identity()?,
            live_floor: floor
                .checked_add(storage)
                .ok_or("native helper retained floor overflow")?,
        };
        let result = (|| {
            index_functions(&mut context, meter)?;
            index_calls(&mut context, meter)?;
            build(&mut context, meter)?;
            action(&context, meter)
        })();
        context.destroy(meter)?;
        result
    }));
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(payload) => {
            if meter.identity().ok() == Some(ledger)
                && let Ok(storage) = meter.storage()
                && let Some(extra) = storage.checked_sub(floor)
            {
                let _ = meter.release(extra);
            }
            std::panic::resume_unwind(payload)
        }
    };
    if meter.identity()? != ledger {
        return Err("native helper caller ledger replaced");
    }
    match outcome {
        Ok(result) => {
            if meter.storage()? < floor {
                return Err("native helper lost incoming floor");
            }
            Ok(result)
        }
        Err(error) => {
            let extra = meter
                .storage()?
                .checked_sub(floor)
                .ok_or("native helper lost incoming floor")?;
            meter.release(extra)?;
            Err(error)
        }
    }
}
