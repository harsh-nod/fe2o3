use super::*;
use std::fmt;

const SUFFIX: &str = "\n[source-partial-move diagnostic truncated]\n";

pub(super) struct Text {
    text: String,
    truncated: bool,
}

impl fmt::Write for Text {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.truncated
            || value.len() > (MAX_BYTES - SUFFIX.len()).saturating_sub(self.text.len())
        {
            self.truncated = true;
            return Err(fmt::Error);
        }
        self.text.push_str(value);
        Ok(())
    }
}

impl Text {
    pub(super) fn new() -> Self {
        Self {
            text: String::with_capacity(MAX_BYTES),
            truncated: false,
        }
    }
    pub(super) fn full(&self) -> bool {
        self.truncated
    }
    pub(super) fn finish(mut self) -> String {
        if self.truncated {
            self.text.push_str(SUFFIX);
        }
        self.text
    }
    pub(super) fn hash(&mut self, bytes: &[u8; 32]) {
        for byte in bytes {
            let _ = write!(self, "{byte:02x}");
        }
    }
    pub(super) fn source(&mut self, source: SemanticSourceProvenanceV1) {
        for (kind, origin) in [
            ("expansion", source.expansion()),
            ("call_site", source.call_site()),
        ] {
            let _ = write!(self, "{kind}=");
            if let Some(origin) = origin {
                self.hash(origin.file().as_bytes());
                let _ = write!(
                    self,
                    ":{:?}-{:?}/bytes{:?} ",
                    origin.start_coordinate(),
                    origin.end_coordinate(),
                    origin.byte_range()
                );
            } else {
                let _ = write!(self, "unavailable ");
            }
        }
    }
    pub(super) fn ty(&mut self, types: &[SemanticTypeDeclV1], id: SemanticTypeIdV1) {
        let Some(ty) = types.get(id.index() as usize) else {
            let _ = writeln!(self, "type{} unavailable", id.index());
            return;
        };
        let _ = write!(self, "type{} identity=", id.index());
        self.hash(ty.identity().as_bytes());
        let _ = write!(
            self,
            " size={:?} align={} uninhabited={} shape=",
            ty.layout().size_bytes(),
            ty.layout().alignment_bytes(),
            ty.layout().is_uninhabited()
        );
        match ty.shape() {
            SemanticTypeShapeV1::Aggregate(fields)
            | SemanticTypeShapeV1::Tuple(fields)
            | SemanticTypeShapeV1::Union(fields) => {
                let kind = match ty.shape() {
                    SemanticTypeShapeV1::Aggregate(_) => "Aggregate",
                    SemanticTypeShapeV1::Tuple(_) => "Tuple",
                    _ => "Union",
                };
                let prefix = &fields.fields()[..fields.fields().len().min(6)];
                let _ = write!(
                    self,
                    "{kind}(count={},type_prefix={prefix:?},truncated={})",
                    fields.fields().len(),
                    fields.fields().len() > 6
                );
            }
            SemanticTypeShapeV1::Enum {
                discriminant,
                variants,
            } => {
                let _ = write!(
                    self,
                    "Enum(discriminant={},variants={})",
                    discriminant.index(),
                    variants.len()
                );
            }
            SemanticTypeShapeV1::Array { element, length } => {
                let _ = write!(self, "Array(element={},length={length})", element.index());
            }
            SemanticTypeShapeV1::Slice { element } => {
                let _ = write!(self, "Slice(element={})", element.index());
            }
            SemanticTypeShapeV1::Pointer(pointer) => {
                let _ = write!(
                    self,
                    "Pointer(kind={:?},pointee={},metadata={:?})",
                    pointer.kind(),
                    pointer.pointee().index(),
                    pointer.metadata()
                );
            }
            SemanticTypeShapeV1::Scalar(scalar) => {
                let _ = write!(self, "Scalar({scalar:?})");
            }
            SemanticTypeShapeV1::ValidityScalar(_) => {
                let _ = write!(self, "ValidityScalar");
            }
            SemanticTypeShapeV1::FunctionPointer {
                arguments,
                return_type,
                ..
            } => {
                let _ = write!(
                    self,
                    "FunctionPointer(arguments={},return={})",
                    arguments.fields().len(),
                    return_type.index()
                );
            }
            SemanticTypeShapeV1::Unit => {
                let _ = write!(self, "Unit");
            }
            SemanticTypeShapeV1::Never => {
                let _ = write!(self, "Never");
            }
            SemanticTypeShapeV1::Opaque => {
                let _ = write!(self, "Opaque");
            }
        }
        let _ = writeln!(self);
    }
    fn place(&mut self, body: &SemanticFunctionDeclV1, place: &SemanticPlaceV1) {
        let _ = write!(self, "local{}(", place.local().index());
        if let Some(local) = body.locals().get(place.local().index() as usize) {
            let _ = write!(
                self,
                "base_type{},role={:?}",
                local.ty().index(),
                local.role()
            );
        } else {
            let _ = write!(self, "base_unavailable");
        }
        let _ = write!(self, ")");
        for projection in place.projections().iter().take(MAX_PROJECTIONS) {
            let _ = write!(
                self,
                "->{:?}:type{}",
                projection.kind(),
                projection.result_type().index()
            );
        }
        if place.projections().len() > MAX_PROJECTIONS {
            let _ = write!(
                self,
                "->[projections truncated,total={} ]",
                place.projections().len()
            );
        }
        let _ = write!(self, ":result_type{}", place.ty().index());
    }
    fn operand(&mut self, body: &SemanticFunctionDeclV1, operand: &SemanticOperandV1) {
        match operand {
            SemanticOperandV1::Copy(place) => {
                let _ = write!(self, "Copy(");
                self.place(body, place);
                let _ = write!(self, ")");
            }
            SemanticOperandV1::Move(place) => {
                let _ = write!(self, "Move(");
                self.place(body, place);
                let _ = write!(self, ")");
            }
            SemanticOperandV1::Constant(constant) => {
                let _ = write!(self, "Constant(type{},", constant.ty().index());
                match constant.value() {
                    SemanticConstantValueV1::ZeroSized => {
                        let _ = write!(self, "ZeroSized");
                    }
                    SemanticConstantValueV1::Scalar(value) => {
                        let _ = write!(self, "Scalar({value:?})");
                    }
                    SemanticConstantValueV1::Bytes(_) => {
                        let _ = write!(self, "Bytes[payload omitted]");
                    }
                    SemanticConstantValueV1::Pointer(value) => {
                        let _ = write!(self, "Pointer({value:?})");
                    }
                    SemanticConstantValueV1::Callable(id) => {
                        let _ = write!(self, "Callable{}", id.index());
                    }
                }
                let _ = write!(self, ")");
            }
        }
    }
    fn rvalue(&mut self, body: &SemanticFunctionDeclV1, value: &SemanticRvalueV1) {
        let _ = write!(self, "type{}:", value.result_type().index());
        match value.kind() {
            SemanticRvalueKindV1::Use(_) => {
                let _ = write!(self, "Use");
            }
            SemanticRvalueKindV1::Unary { operation, .. } => {
                let _ = write!(self, "Unary({operation:?})");
            }
            SemanticRvalueKindV1::Binary { operation, .. } => {
                let _ = write!(self, "Binary({operation:?})");
            }
            SemanticRvalueKindV1::CheckedBinary(value) => {
                let _ = write!(self, "CheckedBinary({:?})", value.operation());
            }
            SemanticRvalueKindV1::UncheckedBinary(value) => {
                let _ = write!(self, "UncheckedBinary({:?})", value.operation());
            }
            SemanticRvalueKindV1::Cast { kind, .. } => {
                let _ = write!(self, "Cast({kind:?})");
            }
            SemanticRvalueKindV1::Aggregate(value) => {
                let _ = write!(
                    self,
                    "Aggregate({:?},operands={})",
                    value.kind(),
                    value.operands().len()
                );
            }
            SemanticRvalueKindV1::Borrow { kind, place } => {
                let _ = write!(self, "Borrow({kind:?},");
                self.place(body, place);
                let _ = write!(self, ")");
            }
            SemanticRvalueKindV1::AddressOf { mutability, place } => {
                let _ = write!(self, "AddressOf({mutability:?},");
                self.place(body, place);
                let _ = write!(self, ")");
            }
            SemanticRvalueKindV1::Length(place) => {
                let _ = write!(self, "Length(");
                self.place(body, place);
                let _ = write!(self, ")");
            }
            SemanticRvalueKindV1::Discriminant(place) => {
                let _ = write!(self, "Discriminant(");
                self.place(body, place);
                let _ = write!(self, ")");
            }
            SemanticRvalueKindV1::Load(load) => {
                let _ = write!(self, "Load(");
                self.place(body, load.source());
                let _ = write!(self, ")");
            }
        }
        let _ = write!(self, " operands=[");
        let mut count = 0;
        let complete = value
            .kind()
            .try_visit_operands(|operand| {
                if count == MAX_OPERANDS || self.full() {
                    return Err(());
                }
                if count != 0 {
                    let _ = write!(self, ", ");
                }
                count += 1;
                self.operand(body, operand);
                Ok(())
            })
            .is_ok();
        if !complete {
            let _ = write!(self, " [operands truncated]");
        }
        let _ = write!(self, "]");
    }
    pub(super) fn statement(
        &mut self,
        body: &SemanticFunctionDeclV1,
        statement: &SemanticStatementKindV1,
    ) {
        match statement {
            SemanticStatementKindV1::Assign(value) => {
                let _ = write!(self, "Assign(");
                self.place(body, value.destination());
                let _ = write!(self, ") = ");
                self.rvalue(body, value.value());
            }
            SemanticStatementKindV1::Store(value) => {
                let _ = write!(self, "Store(");
                self.place(body, value.destination());
                let _ = write!(self, ") = ");
                self.operand(body, value.value());
            }
            SemanticStatementKindV1::AtomicRmw(value) => {
                let _ = write!(self, "AtomicRmw(address=");
                self.place(body, value.address());
                let _ = write!(self, ",value=");
                self.operand(body, value.value());
                let _ = write!(self, ",destination=");
                self.place(body, value.destination());
                let _ = write!(self, ")");
            }
            SemanticStatementKindV1::AtomicCompareExchange(value) => {
                let _ = write!(self, "AtomicCompareExchange(address=");
                self.place(body, value.address());
                let _ = write!(self, ",expected=");
                self.operand(body, value.expected());
                let _ = write!(self, ",replacement=");
                self.operand(body, value.replacement());
                let _ = write!(self, ",destination=");
                self.place(body, value.destination());
                let _ = write!(self, ")");
            }
            SemanticStatementKindV1::SetDiscriminant {
                place,
                variant_index,
            } => {
                let _ = write!(self, "SetDiscriminant(variant={variant_index},");
                self.place(body, place);
                let _ = write!(self, ")");
            }
            SemanticStatementKindV1::Deinitialize(place) => {
                let _ = write!(self, "Deinitialize(");
                self.place(body, place);
                let _ = write!(self, ")");
            }
            SemanticStatementKindV1::StorageLive(local) => {
                let _ = write!(self, "StorageLive(local{})", local.index());
            }
            SemanticStatementKindV1::StorageDead(local) => {
                let _ = write!(self, "StorageDead(local{})", local.index());
            }
            SemanticStatementKindV1::Assume(value) => {
                let _ = write!(self, "Assume(");
                self.operand(body, value);
                let _ = write!(self, ")");
            }
            SemanticStatementKindV1::Nop => {
                let _ = write!(self, "Nop");
            }
        }
    }
    fn callee(&mut self, callables: &[SemanticCallableDeclV1], id: SemanticCallableIdV1) {
        let _ = write!(self, "callable{}", id.index());
        match callables.get(id.index() as usize) {
            Some(SemanticCallableDeclV1::Defined { function }) => {
                let _ = write!(self, "/Defined(function{})", function.index());
            }
            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation_identity, ..
            }) => {
                let _ = write!(self, "/CompilerIntrinsic(identity=");
                self.hash(operation_identity.as_bytes());
                let _ = write!(self, ")");
            }
            Some(SemanticCallableDeclV1::DeviceFfiImport { .. }) => {
                let _ = write!(self, "/DeviceFfiImport");
            }
            None => {
                let _ = write!(self, "/unavailable");
            }
        }
    }
    fn arguments(&mut self, body: &SemanticFunctionDeclV1, arguments: &[SemanticOperandV1]) {
        let _ = write!(self, " arguments(count={})=[", arguments.len());
        for (index, argument) in arguments.iter().enumerate().take(MAX_OPERANDS) {
            if index != 0 {
                let _ = write!(self, ", ");
            }
            self.operand(body, argument);
        }
        if arguments.len() > MAX_OPERANDS {
            let _ = write!(self, " [arguments truncated]");
        }
        let _ = write!(self, "]");
    }
    pub(super) fn terminator(
        &mut self,
        body: &SemanticFunctionDeclV1,
        callables: &[SemanticCallableDeclV1],
        value: &SemanticTerminatorKindV1,
    ) {
        match value {
            SemanticTerminatorKindV1::Call(call) => {
                let _ = write!(self, "Call ");
                self.callee(callables, call.callee());
                self.arguments(body, call.arguments());
                if let Some(destination) = call.destination() {
                    let _ = write!(self, " destination=");
                    self.place(body, destination.place());
                    let _ = write!(self, " normal={:?}", destination.edge());
                }
                let _ = write!(self, " unwind={:?}", call.unwind());
            }
            SemanticTerminatorKindV1::TailCall(call) => {
                let _ = write!(self, "TailCall ");
                self.callee(callables, call.callee());
                self.arguments(body, call.arguments());
                let _ = write!(self, " unwind={:?}", call.unwind());
            }
            SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } => {
                let _ = write!(self, "SwitchInt(");
                self.operand(body, discriminant);
                let _ = write!(self, ") targets_count={}", targets.values().len());
                for target in targets.values().iter().take(MAX_OPERANDS) {
                    let _ = write!(self, " {:?}", target);
                }
                if targets.values().len() > MAX_OPERANDS {
                    let _ = write!(self, " [switch targets truncated]");
                }
                let _ = write!(self, " otherwise={:?}", targets.otherwise());
            }
            SemanticTerminatorKindV1::Drop {
                place,
                drop_glue,
                target,
                unwind,
            } => {
                let _ = write!(self, "Drop(glue{},", drop_glue.index());
                self.place(body, place);
                let _ = write!(self, ",target={target:?},unwind={unwind:?})");
            }
            SemanticTerminatorKindV1::Assert {
                condition,
                expected,
                message,
                target,
                unwind,
            } => {
                let _ = write!(self, "Assert(expected={expected},condition=");
                self.operand(body, condition);
                let _ = write!(self, ",message=");
                self.assert_message(body, message);
                let _ = write!(self, ",target={target:?},unwind={unwind:?})");
            }
            SemanticTerminatorKindV1::Goto(edge) => {
                let _ = write!(self, "Goto({edge:?})");
            }
            SemanticTerminatorKindV1::FalseEdge {
                real_target,
                imaginary_target,
            } => {
                let _ = write!(
                    self,
                    "FalseEdge(real={real_target:?},imaginary={imaginary_target:?})"
                );
            }
            SemanticTerminatorKindV1::Return => {
                let _ = write!(self, "Return");
            }
            SemanticTerminatorKindV1::Unreachable => {
                let _ = write!(self, "Unreachable");
            }
            SemanticTerminatorKindV1::UnwindResume => {
                let _ = write!(self, "UnwindResume");
            }
            SemanticTerminatorKindV1::UnwindTerminate => {
                let _ = write!(self, "UnwindTerminate");
            }
            SemanticTerminatorKindV1::Abort => {
                let _ = write!(self, "Abort");
            }
        }
    }
    fn assert_message(&mut self, body: &SemanticFunctionDeclV1, value: &SemanticAssertMessageV1) {
        match value {
            SemanticAssertMessageV1::BoundsCheck { length, index } => {
                let _ = write!(self, "BoundsCheck(");
                self.operand(body, length);
                let _ = write!(self, ",");
                self.operand(body, index);
                let _ = write!(self, ")");
            }
            SemanticAssertMessageV1::Overflow {
                operation,
                left,
                right,
            } => {
                let _ = write!(self, "Overflow({operation:?},");
                self.operand(body, left);
                let _ = write!(self, ",");
                self.operand(body, right);
                let _ = write!(self, ")");
            }
            SemanticAssertMessageV1::DivisionByZero(value) => {
                let _ = write!(self, "DivisionByZero(");
                self.operand(body, value);
                let _ = write!(self, ")");
            }
            SemanticAssertMessageV1::RemainderByZero(value) => {
                let _ = write!(self, "RemainderByZero(");
                self.operand(body, value);
                let _ = write!(self, ")");
            }
            SemanticAssertMessageV1::MisalignedPointerDereference {
                required_alignment,
                found_alignment,
            } => {
                let _ = write!(self, "MisalignedPointerDereference(");
                self.operand(body, required_alignment);
                let _ = write!(self, ",");
                self.operand(body, found_alignment);
                let _ = write!(self, ")");
            }
            SemanticAssertMessageV1::NullPointerDereference => {
                let _ = write!(self, "NullPointerDereference");
            }
            SemanticAssertMessageV1::ResumedAfterReturn => {
                let _ = write!(self, "ResumedAfterReturn");
            }
            SemanticAssertMessageV1::ResumedAfterPanic => {
                let _ = write!(self, "ResumedAfterPanic");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_partial_move_diagnostic_byte_cap_and_utf8_are_exact() {
        let mut out = Text::new();
        let _ = write!(out, "{}", "x".repeat(MAX_BYTES - SUFFIX.len() - 1));
        let _ = write!(out, "{}", char::from_u32(0x3bb).unwrap());
        let _ = write!(out, "ignored");
        let text = out.finish();
        assert!(text.len() <= MAX_BYTES);
        assert!(text.ends_with(SUFFIX));
        assert!(!text.contains("ignored"));
    }
}
