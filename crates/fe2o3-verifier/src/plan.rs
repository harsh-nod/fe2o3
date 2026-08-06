use std::fmt;

use crate::{
    AxiomPolicy, Configuration, ExecutionTools, ModelError, ProofRequestV1,
    VerificationModelIdentity,
};

pub const MAX_PATH_BYTES: usize = 4096;
pub const MAX_TIMEOUT_SECONDS: u32 = 86_400;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvocationPaths {
    verifier_program: String,
    solver_program: String,
    recorder_program: String,
    request_file: String,
    result_file: String,
}

impl InvocationPaths {
    pub fn new(
        verifier_program: impl Into<String>,
        solver_program: impl Into<String>,
        recorder_program: impl Into<String>,
        request_file: impl Into<String>,
        result_file: impl Into<String>,
    ) -> Result<Self, PlanError> {
        Ok(Self {
            verifier_program: checked_path("verifier program", verifier_program.into())?,
            solver_program: checked_path("solver program", solver_program.into())?,
            recorder_program: checked_path("recorder program", recorder_program.into())?,
            request_file: checked_path("request file", request_file.into())?,
            result_file: checked_path("result file", result_file.into())?,
        })
    }

    pub fn verifier_program(&self) -> &str {
        &self.verifier_program
    }

    pub fn solver_program(&self) -> &str {
        &self.solver_program
    }

    pub fn recorder_program(&self) -> &str {
        &self.recorder_program
    }

    pub fn request_file(&self) -> &str {
        &self.request_file
    }

    pub fn result_file(&self) -> &str {
        &self.result_file
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSpec {
    program: String,
    arguments: Vec<String>,
}

impl CommandSpec {
    pub fn program(&self) -> &str {
        &self.program
    }

    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifierPolicy {
    expected_tools: ExecutionTools,
    expected_configuration: Configuration,
    expected_model: VerificationModelIdentity,
    axiom_policy: AxiomPolicy,
    max_timeout_seconds: u32,
}

impl VerifierPolicy {
    pub fn new(
        expected_tools: ExecutionTools,
        expected_configuration: Configuration,
        expected_model: VerificationModelIdentity,
        axiom_policy: AxiomPolicy,
        max_timeout_seconds: u32,
    ) -> Result<Self, PlanError> {
        if max_timeout_seconds == 0 || max_timeout_seconds > MAX_TIMEOUT_SECONDS {
            return Err(PlanError::TimeoutOutOfRange {
                max: MAX_TIMEOUT_SECONDS,
            });
        }
        Ok(Self {
            expected_tools,
            expected_configuration,
            expected_model,
            axiom_policy,
            max_timeout_seconds,
        })
    }

    pub const fn expected_tools(&self) -> &ExecutionTools {
        &self.expected_tools
    }

    pub const fn axiom_policy(&self) -> &AxiomPolicy {
        &self.axiom_policy
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvocationPlan {
    request: ProofRequestV1,
    tools: ExecutionTools,
    command: CommandSpec,
    verifier_program: String,
    solver_program: String,
    request_file: String,
    result_file: String,
    request_bytes: Vec<u8>,
    timeout_seconds: u32,
}

impl InvocationPlan {
    pub const fn request(&self) -> &ProofRequestV1 {
        &self.request
    }

    pub const fn tools(&self) -> &ExecutionTools {
        &self.tools
    }

    pub const fn command(&self) -> &CommandSpec {
        &self.command
    }

    pub fn request_file(&self) -> &str {
        &self.request_file
    }

    pub fn verifier_program(&self) -> &str {
        &self.verifier_program
    }

    pub fn solver_program(&self) -> &str {
        &self.solver_program
    }

    pub fn result_file(&self) -> &str {
        &self.result_file
    }

    pub fn request_bytes(&self) -> &[u8] {
        &self.request_bytes
    }

    pub const fn timeout_seconds(&self) -> u32 {
        self.timeout_seconds
    }

    /// Canonical bytes an integrator can hash for an artifact invocation ID.
    pub fn canonical_invocation_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.request_bytes.len() + 1024);
        put_bytes(&mut bytes, b"FE2O3VIP");
        put_u16(&mut bytes, 1);
        put_u32(&mut bytes, self.timeout_seconds);
        put_text(&mut bytes, &self.command.program);
        put_u16(&mut bytes, self.command.arguments.len() as u16);
        for argument in &self.command.arguments {
            put_text(&mut bytes, argument);
        }
        for tool in [
            self.tools.verifier(),
            self.tools.solver(),
            self.tools.evidence_recorder(),
        ] {
            put_text(&mut bytes, tool.name().as_str());
            put_text(&mut bytes, tool.version().as_str());
            put_bytes(&mut bytes, tool.executable_digest().as_bytes());
            put_bytes(&mut bytes, tool.configuration_digest().as_bytes());
        }
        put_u32(&mut bytes, self.request_bytes.len() as u32);
        put_bytes(&mut bytes, &self.request_bytes);
        bytes
    }
}

pub fn build_invocation_plan(
    request: ProofRequestV1,
    measured_tools: ExecutionTools,
    paths: InvocationPaths,
    timeout_seconds: u32,
    policy: &VerifierPolicy,
) -> Result<InvocationPlan, PlanError> {
    if measured_tools != policy.expected_tools {
        return Err(PlanError::ToolPolicyMismatch);
    }
    if request.configuration() != &policy.expected_configuration {
        return Err(PlanError::ConfigurationPolicyMismatch);
    }
    if request.model() != &policy.expected_model {
        return Err(PlanError::ModelPolicyMismatch);
    }
    policy.axiom_policy.validate(request.trusted_items())?;
    if timeout_seconds == 0 || timeout_seconds > policy.max_timeout_seconds {
        return Err(PlanError::TimeoutOutOfRange {
            max: policy.max_timeout_seconds,
        });
    }

    let command = CommandSpec {
        program: paths.recorder_program,
        arguments: vec![
            "--request".to_owned(),
            paths.request_file.clone(),
            "--result".to_owned(),
            paths.result_file.clone(),
            "--verifier".to_owned(),
            paths.verifier_program.clone(),
            "--solver".to_owned(),
            paths.solver_program.clone(),
            "--timeout-seconds".to_owned(),
            timeout_seconds.to_string(),
        ],
    };
    let request_bytes = request.to_canonical_bytes();
    Ok(InvocationPlan {
        request,
        tools: measured_tools,
        command,
        verifier_program: paths.verifier_program,
        solver_program: paths.solver_program,
        request_file: paths.request_file,
        result_file: paths.result_file,
        request_bytes,
        timeout_seconds,
    })
}

fn checked_path(field: &'static str, path: String) -> Result<String, PlanError> {
    if path.is_empty() || path.len() > MAX_PATH_BYTES || path.chars().any(char::is_control) {
        Err(PlanError::InvalidPath { field })
    } else {
        Ok(path)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PlanError {
    InvalidPath { field: &'static str },
    TimeoutOutOfRange { max: u32 },
    ToolPolicyMismatch,
    ConfigurationPolicyMismatch,
    ModelPolicyMismatch,
    Model(ModelError),
}

impl fmt::Display for PlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath { field } => write!(formatter, "{field} is not a bounded path"),
            Self::TimeoutOutOfRange { max } => {
                write!(formatter, "timeout must be in 1..={max} seconds")
            }
            Self::ToolPolicyMismatch => write!(formatter, "measured tools do not match policy"),
            Self::ConfigurationPolicyMismatch => {
                write!(formatter, "proof configuration does not match policy")
            }
            Self::ModelPolicyMismatch => {
                write!(formatter, "verification model does not match policy")
            }
            Self::Model(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PlanError {}

impl From<ModelError> for PlanError {
    fn from(value: ModelError) -> Self {
        Self::Model(value)
    }
}

fn put_bytes(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(value);
}

fn put_u16(output: &mut Vec<u8>, value: u16) {
    put_bytes(output, &value.to_le_bytes());
}

fn put_u32(output: &mut Vec<u8>, value: u32) {
    put_bytes(output, &value.to_le_bytes());
}

fn put_text(output: &mut Vec<u8>, value: &str) {
    put_u16(output, value.len() as u16);
    put_bytes(output, value.as_bytes());
}
