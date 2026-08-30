#![forbid(unsafe_code)]

use std::env;
use std::io::{self, BufReader, Write};
use std::process::ExitCode;

use fe2o3_semantic_query::{
    AgentProfilerErrorCodeV1, AgentProfilerServiceErrorV1, AgentProfilerServiceLimitsV1,
    AgentProfilerServiceV1, decode_agent_profiler_request_line_v1,
    read_agent_profiler_request_line_v1, run_agent_profiler_distributed_overlap_jsonl_v1,
};

fn main() -> ExitCode {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    match arguments.as_slice() {
        [mode] if mode == "jsonl" => match run_jsonl() {
            Ok(()) => ExitCode::SUCCESS,
            Err(()) => ExitCode::from(1),
        },
        [mode] if mode == "distributed-overlap-jsonl" => {
            let stdin = io::stdin();
            let mut input = BufReader::new(stdin.lock());
            let stdout = io::stdout();
            let mut output = stdout.lock();
            match run_agent_profiler_distributed_overlap_jsonl_v1(&mut input, &mut output) {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => ExitCode::from(1),
            }
        }
        _ => ExitCode::from(2),
    }
}

fn run_jsonl() -> Result<(), ()> {
    let stdin = io::stdin();
    let mut input = BufReader::new(stdin.lock());
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let mut service =
        AgentProfilerServiceV1::new(AgentProfilerServiceLimitsV1::default()).map_err(|_| ())?;

    loop {
        let line = match read_agent_profiler_request_line_v1(&mut input) {
            Ok(Some(line)) => line,
            Ok(None) => return Ok(()),
            Err(AgentProfilerServiceErrorV1::RequestTooLarge) => {
                write_terminal(
                    &mut output,
                    &mut service,
                    AgentProfilerErrorCodeV1::RequestTooLarge,
                )?;
                return Err(());
            }
            Err(_) => {
                write_terminal(
                    &mut output,
                    &mut service,
                    AgentProfilerErrorCodeV1::InvalidRequest,
                )?;
                return Err(());
            }
        };
        let request = match decode_agent_profiler_request_line_v1(&line) {
            Ok(request) => request,
            Err(_) => {
                write_terminal(
                    &mut output,
                    &mut service,
                    AgentProfilerErrorCodeV1::InvalidRequest,
                )?;
                return Err(());
            }
        };
        let response = service.handle(request);
        let terminal = response.is_terminal();
        let encoded = match service.encode_response(&response) {
            Ok(encoded) => encoded,
            Err(AgentProfilerServiceErrorV1::ResponseTooLarge) => {
                write_terminal(
                    &mut output,
                    &mut service,
                    AgentProfilerErrorCodeV1::ResponseTooLarge,
                )?;
                return Err(());
            }
            Err(_) => {
                write_terminal(
                    &mut output,
                    &mut service,
                    AgentProfilerErrorCodeV1::InternalEvidenceMismatch,
                )?;
                return Err(());
            }
        };
        output.write_all(&encoded).map_err(|_| ())?;
        output.flush().map_err(|_| ())?;
        if terminal {
            return Err(());
        }
    }
}

fn write_terminal<W: Write>(
    output: &mut W,
    service: &mut AgentProfilerServiceV1,
    code: AgentProfilerErrorCodeV1,
) -> Result<(), ()> {
    let response = service.terminal_protocol_error(code).map_err(|_| ())?;
    let encoded = service.encode_response(&response).map_err(|_| ())?;
    output.write_all(&encoded).map_err(|_| ())?;
    output.flush().map_err(|_| ())
}
