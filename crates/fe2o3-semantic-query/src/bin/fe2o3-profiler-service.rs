#![forbid(unsafe_code)]

use std::env;
use std::io::{self, BufReader};
use std::process::ExitCode;

use fe2o3_semantic_query::{
    run_agent_decoded_att_jsonl_v1, run_agent_kfd_source_isa_jsonl_v1,
    run_agent_pc_source_isa_jsonl_v1, run_agent_profiler_distributed_overlap_jsonl_v1,
    run_agent_profiler_jsonl_v1, run_agent_profiler_variant_jsonl_v1,
};

fn main() -> ExitCode {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    match arguments.as_slice() {
        [mode] if mode == "jsonl" => {
            let stdin = io::stdin();
            let mut input = BufReader::new(stdin.lock());
            let stdout = io::stdout();
            let mut output = stdout.lock();
            match run_agent_profiler_jsonl_v1(&mut input, &mut output) {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => ExitCode::from(1),
            }
        }
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
        [mode] if mode == "variant-jsonl" => {
            let stdin = io::stdin();
            let mut input = BufReader::new(stdin.lock());
            let stdout = io::stdout();
            let mut output = stdout.lock();
            match run_agent_profiler_variant_jsonl_v1(&mut input, &mut output) {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => ExitCode::from(1),
            }
        }
        [mode] if mode == "kfd-source-isa-jsonl" => {
            let stdin = io::stdin();
            let mut input = BufReader::new(stdin.lock());
            let stdout = io::stdout();
            let mut output = stdout.lock();
            match run_agent_kfd_source_isa_jsonl_v1(&mut input, &mut output) {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => ExitCode::from(1),
            }
        }
        [mode] if mode == "pc-source-isa-jsonl" => {
            let stdin = io::stdin();
            let mut input = BufReader::new(stdin.lock());
            let stdout = io::stdout();
            let mut output = stdout.lock();
            match run_agent_pc_source_isa_jsonl_v1(&mut input, &mut output) {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => ExitCode::from(1),
            }
        }
        [mode] if mode == "decoded-att-jsonl" => {
            let stdin = io::stdin();
            let mut input = BufReader::new(stdin.lock());
            let stdout = io::stdout();
            let mut output = stdout.lock();
            match run_agent_decoded_att_jsonl_v1(&mut input, &mut output) {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => ExitCode::from(1),
            }
        }
        _ => ExitCode::from(2),
    }
}
