//! Exact static call edges from one immutable, verified Bundle V6 owner.
//! This additive query does not change the operation-page grammar or materialize calls.
use super::*;
use fe2o3_kernel_ir::{FunctionId, FunctionRole};

/// Separate query bound; the existing operation/region limits are unchanged.
pub const MAX_AUTHORING_CALL_VALUES_V1: usize = 64;
pub const MAX_AUTHORING_CALL_KERNELS_V1: usize = 64;

/// One exact Kernel record whose entry is the caller's retained FunctionId.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthoringCallKernelRegistrationV1 {
    pub kernel: u32,
    pub kernel_id: String,
    pub entry_function_id: String,
}

/// Canonical caller role and registration, not authenticated Rust source identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthoringCallerV1 {
    pub function: u32,
    pub function_id: String,
    pub role: &'static str,
    /// Complete same-caller matches within this query's bounds; no first-match choice.
    pub kernel_registrations: Vec<AuthoringCallKernelRegistrationV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthoringCalleeV1 {
    /// Ordinal in this exact bundle, not an ordinal in another compilation.
    pub function: u32,
    /// The retained canonical FunctionId, treated as opaque rather than parsed.
    pub function_id: String,
    pub role: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthoringCallArgumentV1 {
    pub position: u32,
    /// SSA value in selector.operations[0].function.
    pub operand: AuthoringValueV1,
    /// SSA parameter in callee.function; equal numeric labels do not join scopes.
    pub formal: AuthoringValueV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthoringCallResultV1 {
    pub position: u32,
    /// SSA result in the caller. No callee Return operand is invented.
    pub result: AuthoringValueV1,
    pub signature_type: String,
}

/// A bounded, inert static call-site projection. It is not a runtime stack frame,
/// physical helper ABI, transitive helper-closure certificate, or executable graph.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthoringCallTargetV1 {
    pub schema: &'static str,
    pub authority: AuthoringAuthorityV1,
    pub selector: AuthoringRegionSelectorV1,
    pub call: AuthoringOperationV1,
    pub caller: AuthoringCallerV1,
    pub callee: AuthoringCalleeV1,
    pub arguments: Vec<AuthoringCallArgumentV1>,
    pub results: Vec<AuthoringCallResultV1>,
    pub correspondence: &'static str,
    pub transitive_helper_closure: &'static str,
    pub dynamic_invocation: &'static str,
    pub physical_abi: &'static str,
}

/// Additive error type: existing authoring error variants and diagnostics are unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoringCallTargetErrorV1 {
    Snapshot(AuthoringErrorV1),
    ExpectedSingleOperation,
    NotCall,
    MissingCallee,
    AmbiguousCallee,
    UnsupportedCallee,
    InvalidCallBinding,
    UnsupportedCaller,
    MissingKernelRegistration,
    InvalidCallerRegistration,
}

impl From<AuthoringErrorV1> for AuthoringCallTargetErrorV1 {
    fn from(error: AuthoringErrorV1) -> Self {
        Self::Snapshot(error)
    }
}

impl fmt::Display for AuthoringCallTargetErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Snapshot(error) => return fmt::Display::fmt(error, formatter),
            Self::ExpectedSingleOperation => {
                "call-target selection must name exactly one operation"
            }
            Self::NotCall => "selected canonical operation is not a direct Call",
            Self::MissingCallee => "retained Call target is missing from the same canonical module",
            Self::AmbiguousCallee => {
                "retained Call target is ambiguous in the same canonical module"
            }
            Self::UnsupportedCallee => "call-target query requires a defined internal helper",
            Self::InvalidCallBinding => {
                "retained call arguments or results disagree with the typed helper signature"
            }
            Self::UnsupportedCaller => "call-target query requires a defined canonical caller",
            Self::MissingKernelRegistration => {
                "KernelEntry caller has no same-owner kernel registration"
            }
            Self::InvalidCallerRegistration => {
                "caller role or same-owner kernel registrations are inconsistent"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for AuthoringCallTargetErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Snapshot(error) => Some(error),
            _ => None,
        }
    }
}

type CallResult<T> = std::result::Result<T, AuthoringCallTargetErrorV1>;

impl AuthoringSnapshotV1 {
    /// Inspect one exact static Call to a defined InternalHelper. The selected
    /// call's retained FunctionId is resolved by equality inside this owner only.
    /// All bindings use canonical Type equality before bounded text rendering.
    /// The query does not infer a target from source spans, names, constant bits,
    /// argument values, or another report, and does not traverse helper bodies.
    pub fn inspect_call_target_v1(
        &self,
        selector: &AuthoringRegionSelectorV1,
    ) -> CallResult<AuthoringCallTargetV1> {
        self.check_selector(selector)?;
        let [coordinate] = selector.operations.as_slice() else {
            return Err(AuthoringCallTargetErrorV1::ExpectedSingleOperation);
        };
        let operation = self.operation(*coordinate)?;
        let OperationKind::Call { callee, arguments } = &operation.kind else {
            return Err(AuthoringCallTargetErrorV1::NotCall);
        };
        let mut scan = 0;
        let caller = self.retained_caller_v1(coordinate.function, &mut scan)?;
        let (function, target) = self.retained_callee_v1(callee, &mut scan)?;
        if target.role != FunctionRole::InternalHelper {
            return Err(AuthoringCallTargetErrorV1::UnsupportedCallee);
        }
        let body = target
            .body
            .as_ref()
            .ok_or(AuthoringCallTargetErrorV1::UnsupportedCallee)?;
        for count in [
            arguments.len(),
            operation.results.len(),
            target.signature.parameters.len(),
            target.signature.results.len(),
            body.parameters.len(),
        ] {
            if count > MAX_AUTHORING_CALL_VALUES_V1 {
                return Err(AuthoringErrorV1::ResourceLimit.into());
            }
        }
        if arguments.len() != target.signature.parameters.len()
            || body.parameters.len() != target.signature.parameters.len()
            || operation.results.len() != target.signature.results.len()
        {
            return Err(AuthoringCallTargetErrorV1::InvalidCallBinding);
        }
        let arguments = self.call_arguments_v1(*coordinate, function, target, arguments)?;
        let results = self.call_results_v1(*coordinate, operation, target)?;
        Ok(bounded_report(AuthoringCallTargetV1 {
            schema: "fe2o3-authoring-call-target-v1",
            authority: AUTHORITY,
            selector: selector.clone(),
            call: self.operation_view(*coordinate)?,
            caller,
            callee: AuthoringCalleeV1 {
                function,
                function_id: target.id.as_str().to_owned(),
                role: "internal_helper",
            },
            arguments,
            results,
            correspondence: "exact_retained_call_operand_to_formal_position",
            transitive_helper_closure: "not_traversed",
            dynamic_invocation: "unavailable_static_call_site_only",
            physical_abi: "unavailable_logical_canonical_call",
        })?)
    }

    fn retained_caller_v1(&self, function: u32, scan: &mut usize) -> CallResult<AuthoringCallerV1> {
        let caller = self
            .module
            .functions
            .get(function as usize)
            .ok_or(AuthoringErrorV1::InvalidCoordinate)?;
        if caller.id.as_str().len() > MAX_TEXT_BYTES {
            return Err(AuthoringErrorV1::ResourceLimit.into());
        }
        if caller.id.as_str().is_empty() || caller.body.is_none() {
            return Err(AuthoringCallTargetErrorV1::UnsupportedCaller);
        }
        let role = match caller.role {
            FunctionRole::KernelEntry => "kernel_entry",
            FunctionRole::InternalHelper => "internal_helper",
            FunctionRole::DeviceFfiExport | FunctionRole::ExternalImport => {
                return Err(AuthoringCallTargetErrorV1::UnsupportedCaller);
            }
        };
        let mut registrations: Vec<AuthoringCallKernelRegistrationV1> = Vec::new();
        for (index, kernel) in self.module.kernels.iter().enumerate() {
            charge(
                scan,
                caller.id.as_str().len().min(kernel.entry.as_str().len()) + 1,
            )?;
            if kernel.entry != caller.id {
                continue;
            }
            if registrations.len() == MAX_AUTHORING_CALL_KERNELS_V1
                || kernel.id.as_str().len() > MAX_TEXT_BYTES
            {
                return Err(AuthoringErrorV1::ResourceLimit.into());
            }
            if kernel.id.as_str().is_empty() || caller.role != FunctionRole::KernelEntry {
                return Err(AuthoringCallTargetErrorV1::InvalidCallerRegistration);
            }
            for previous in &registrations {
                charge(
                    scan,
                    previous.kernel_id.len().min(kernel.id.as_str().len()) + 1,
                )?;
                if previous.kernel_id == kernel.id.as_str() {
                    return Err(AuthoringCallTargetErrorV1::InvalidCallerRegistration);
                }
            }
            registrations.push(AuthoringCallKernelRegistrationV1 {
                kernel: ordinal(index)?,
                kernel_id: kernel.id.as_str().to_owned(),
                entry_function_id: kernel.entry.as_str().to_owned(),
            });
        }
        if caller.role == FunctionRole::KernelEntry && registrations.is_empty() {
            return Err(AuthoringCallTargetErrorV1::MissingKernelRegistration);
        }
        Ok(AuthoringCallerV1 {
            function,
            function_id: caller.id.as_str().to_owned(),
            role,
            kernel_registrations: registrations,
        })
    }

    fn retained_callee_v1(
        &self,
        callee: &FunctionId,
        scan: &mut usize,
    ) -> CallResult<(u32, &Function)> {
        if callee.as_str().len() > MAX_TEXT_BYTES {
            return Err(AuthoringErrorV1::ResourceLimit.into());
        }
        let mut found = None;
        for (index, function) in self.module.functions.iter().enumerate() {
            // Charge a conservative byte-comparison bound, not only one unit per
            // function. No index, module clone, or body traversal is allocated.
            charge(
                scan,
                callee.as_str().len().min(function.id.as_str().len()) + 1,
            )?;
            if &function.id == callee {
                if found.is_some() {
                    return Err(AuthoringCallTargetErrorV1::AmbiguousCallee);
                }
                found = Some((ordinal(index)?, function));
            }
        }
        found.ok_or(AuthoringCallTargetErrorV1::MissingCallee)
    }

    fn call_arguments_v1(
        &self,
        coordinate: AuthoringOperationCoordinateV1,
        function: u32,
        target: &Function,
        arguments: &[ValueId],
    ) -> CallResult<Vec<AuthoringCallArgumentV1>> {
        let body = target
            .body
            .as_ref()
            .ok_or(AuthoringCallTargetErrorV1::UnsupportedCallee)?;
        arguments
            .iter()
            .zip(&body.parameters)
            .zip(&target.signature.parameters)
            .enumerate()
            .map(|(position, ((operand, formal), ty))| {
                let formal_definition = self
                    .definitions
                    .get(function as usize)
                    .and_then(|values| values.get(formal));
                if !matches!(formal_definition, Some(Definition::Parameter(index)) if *index == position)
                    || self.value_type(coordinate.function, *operand) != Some(ty)
                    || self.value_type(function, *formal) != Some(ty)
                {
                    return Err(AuthoringCallTargetErrorV1::InvalidCallBinding);
                }
                Ok(AuthoringCallArgumentV1 {
                    position: ordinal(position)?,
                    operand: self.value_view(coordinate.function, *operand)?,
                    formal: self.value_view(function, *formal)?,
                })
            })
            .collect()
    }

    fn call_results_v1(
        &self,
        coordinate: AuthoringOperationCoordinateV1,
        operation: &Operation,
        target: &Function,
    ) -> CallResult<Vec<AuthoringCallResultV1>> {
        operation
            .results
            .iter()
            .zip(&target.signature.results)
            .enumerate()
            .map(|(position, (result, ty))| {
                if &result.ty != ty || self.value_type(coordinate.function, result.id) != Some(ty) {
                    return Err(AuthoringCallTargetErrorV1::InvalidCallBinding);
                }
                Ok(AuthoringCallResultV1 {
                    position: ordinal(position)?,
                    result: self.value_view(coordinate.function, result.id)?,
                    signature_type: bounded_debug(ty)?,
                })
            })
            .collect()
    }
}
