use fe2o3_host_link_closure::{
    HOST_LINK_ADMISSION_MAX_BYTES_PER_POLL_V1, HOST_LINK_ADMISSION_MAX_MILLIS_PER_POLL_V1,
    HOST_LINK_ADMISSION_MAX_OPERATIONS_PER_POLL_V1, HOST_LINK_REJECTION_CODES_V1,
    HOST_LINK_RESULT_COPY_POLICY_V1, HOST_LLD_FIRST_INPUT_CHILD_FD_V1,
    HOST_LLD_INPUT_ARGUMENT_PREFIX_V1, HOST_LLD_PROTOCOL_ARGUMENT_V1,
    HOST_LLD_REQUEST_ARGUMENT_PREFIX_V1, HOST_LLD_RESULT_SOCKET_ARGUMENT_PREFIX_V1,
    HOST_LLD_RESULT_SOCKET_CHILD_FD_V1, MAX_AUTHENTICATED_HOST_LINK_EXECUTIONS_V1,
    MAX_HOST_LINK_ARCHIVE_MEMBERS_V1, MAX_HOST_LINK_ARGUMENTS_V1,
    MAX_HOST_LINK_ELF_PROGRAM_HEADERS_V1, MAX_HOST_LINK_ELF_SECTIONS_V1,
    MAX_HOST_LINK_ELF_TABLE_ENTRIES_V1, MAX_HOST_LINK_INPUT_BYTES_V1,
    MAX_HOST_LINK_OUTPUT_BYTES_V1, MAX_HOST_LINK_PRODUCERS_V1,
    MAX_HOST_LINK_RESULT_RECORD_BYTES_V1, MAX_HOST_LINK_RETAINED_BYTES_V1,
    MAX_HOST_LINK_UNIQUE_INPUTS_V1,
};

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("rejection-codes-v1") => {
            for code in HOST_LINK_REJECTION_CODES_V1 {
                println!("{code}");
            }
        }
        Some("static-tool-contract-v1") => {
            println!("schema=fe2o3-host-link-static-tool-contract-v1");
            println!("tool-approval=external-unsafe-authority,move-only,exact-plan-and-tool");
            println!(
                "launcher=clone3-clone-pidfd-clear-sighand,execveat-at-empty-path,exec-status-pipe"
            );
            println!("canonical-child-fds=result:91,inputs:100+");
            println!("process-witness=atomic-pidfd,waitid-p-pidfd,scm-credentials");
            println!("process-reap=bounded-api-return,single-event-loop,eventual-waitid");
            println!("worker-process-creation=seccomp-deny-clone-clone3-fork-vfork");
            println!("worker-signal-state=default-dispositions,empty-mask");
            println!(
                "root-journal-procfs=retained-PROC_SUPER_MAGIC,mount-namespace-and-path-identities"
            );
            println!("execution-wall-timeout-seconds=30");
            println!("admission-max-bytes-per-poll={HOST_LINK_ADMISSION_MAX_BYTES_PER_POLL_V1}");
            println!(
                "admission-max-operations-per-poll={HOST_LINK_ADMISSION_MAX_OPERATIONS_PER_POLL_V1}"
            );
            println!(
                "admission-cooperative-check-target-ms={HOST_LINK_ADMISSION_MAX_MILLIS_PER_POLL_V1}"
            );
            println!("max-authenticated-executions={MAX_AUTHENTICATED_HOST_LINK_EXECUTIONS_V1}");
            println!("max-plan-arguments={MAX_HOST_LINK_ARGUMENTS_V1}");
            println!("max-producers={MAX_HOST_LINK_PRODUCERS_V1}");
            println!("max-unique-inputs={MAX_HOST_LINK_UNIQUE_INPUTS_V1}");
            println!("max-input-bytes={MAX_HOST_LINK_INPUT_BYTES_V1}");
            println!("max-retained-bytes={MAX_HOST_LINK_RETAINED_BYTES_V1}");
            println!("max-output-bytes={MAX_HOST_LINK_OUTPUT_BYTES_V1}");
            println!("max-cumulative-archive-members={MAX_HOST_LINK_ARCHIVE_MEMBERS_V1}");
            println!("max-elf-program-headers={MAX_HOST_LINK_ELF_PROGRAM_HEADERS_V1}");
            println!("max-elf-sections={MAX_HOST_LINK_ELF_SECTIONS_V1}");
            println!("max-elf-table-entries={MAX_HOST_LINK_ELF_TABLE_ENTRIES_V1}");
            println!("result-socket-child-fd={HOST_LLD_RESULT_SOCKET_CHILD_FD_V1}");
            println!("first-input-child-fd={HOST_LLD_FIRST_INPUT_CHILD_FD_V1}");
            println!("argv[0]=fe2o3-host-lld");
            println!("argv[1]={HOST_LLD_PROTOCOL_ARGUMENT_V1}");
            println!(
                "argv[2]={HOST_LLD_RESULT_SOCKET_ARGUMENT_PREFIX_V1}91:<dev_decimal>:<ino_decimal>"
            );
            println!(
                "argv[3]={HOST_LLD_REQUEST_ARGUMENT_PREFIX_V1}<plan_sha256>:<closure_sha256>:<nonce_sha256>"
            );
            println!(
                "argv[semantic]={HOST_LLD_INPUT_ARGUMENT_PREFIX_V1}<fd>:<kind>:<sha256hex>:<size_decimal>:<mode_octal>"
            );
            println!("input-kinds=elf-rel,archive,rlib");
            println!(
                "input-elf=x86_64-elf64-little-et-rel,bounded-subset,no-compressed-crel-bitcode-deplibs"
            );
            println!("archive-index=gnu32-and-long-names-validated,bsd-and-sym64-rejected");
            println!(
                "output-elf=x86_64-elf64-little-et-exec,bounded-static-subset,no-interp-dynamic-needed-wx-execstack"
            );
            println!("result-record-max-bytes={MAX_HOST_LINK_RESULT_RECORD_BYTES_V1}");
            println!(
                "result-record=fe2o3-host-lld-result-v1\\tplan=<hex>\\tclosure=<hex>\\tnonce=<hex>\\tsha256=<hex>\\tlength=<decimal>\\tcopy={HOST_LINK_RESULT_COPY_POLICY_V1}\\n"
            );
            println!("result-copy-policy={HOST_LINK_RESULT_COPY_POLICY_V1}");
            println!("result-rights-count=1");
            println!("result-sender-identity=regular,nlink:0,current-euid,tmpfs");
            println!("result-seals=WRITE|GROW|SHRINK|SEAL");
            println!("stable-closure-digest-excludes=result-socket-dev-inode,request-control");
        }
        _ => {
            eprintln!(
                "usage: host-link-closure-contract \
                 <rejection-codes-v1|static-tool-contract-v1>"
            );
            std::process::exit(64);
        }
    }
}
