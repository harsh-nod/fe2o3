#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

contract_bin=(cargo run --quiet -p fe2o3-host-link-closure --bin host-link-closure-contract --)

run_combined_identity_hook() {
    local tool="${FE2O3_HOST_LLD_COMBINED_TOOL_V1:-}"
    if [[ -z "${tool}" ]]; then
        return
    fi
    if [[ "${tool}" != /* || ! -f "${tool}" || ! -x "${tool}" ]]; then
        echo "combined static-tool hook requires an absolute executable regular file" >&2
        exit 1
    fi
    local expected_sha256="${FE2O3_HOST_LLD_EXPECTED_SHA256_V1:-}"
    local expected_llvm="${FE2O3_HOST_LLD_EXPECTED_LLVM_BUILD_IDENTITY_V1:-}"
    if [[ ! "${expected_sha256}" =~ ^[0-9a-f]{64}$ || -z "${expected_llvm}" ]]; then
        echo "combined static-tool hook requires exact SHA-256 and LLVM build identity evidence" >&2
        exit 1
    fi
    local actual_sha256
    actual_sha256="$(sha256sum -- "${tool}")"
    actual_sha256="${actual_sha256%% *}"
    if [[ "${actual_sha256}" != "${expected_sha256}" ]]; then
        echo "combined static-tool executable digest does not match evidence" >&2
        exit 1
    fi
    local identity
    identity="$(env -i LC_ALL=C LANG=C TZ=UTC SOURCE_DATE_EPOCH=0 \
        "${tool}" --fe2o3-identity-v1)"
    local required
    for required in \
        'format=fe2o3-host-lld-identity-v1' \
        'authority=none' \
        'protocol=fe2o3-host-lld-elf-v2' \
        'input_protocol=fe2o3-input-v1' \
        'result_protocol=fe2o3-host-lld-result-v1' \
        'result_socket_fd=91' \
        'output_staging=tool-owned-sealed-memfd-v1' \
        'result_copy=receiver-owned-memfd-v1' \
        'max_argument_count=4096' \
        'max_input_count=2048' \
        'max_input_bytes=268435456' \
        'max_total_input_bytes=2147483648' \
        'max_output_bytes=536870912' \
        'max_archive_members=262144' \
        'dependent_libraries=forbidden' \
        'elf_class=ELF64' \
        'elf_machine=Advanced Micro Devices X86-64' \
        "llvm_build_identity=${expected_llvm}"; do
        if ! grep -Fqx -- "${required}" <<<"${identity}"; then
            printf 'combined static-tool identity missing exact field: %s\n' "${required}" >&2
            exit 1
        fi
    done
    echo "host-link closure/static-tool combined identity hook: PASS"
}

expected_contract='schema=fe2o3-host-link-static-tool-contract-v1
tool-approval=external-unsafe-authority,move-only,exact-plan-and-tool
launcher=clone3-clone-pidfd-clear-sighand,execveat-at-empty-path,exec-status-pipe
canonical-child-fds=result:91,inputs:100+
process-witness=atomic-pidfd,waitid-p-pidfd,scm-credentials
process-reap=bounded-api-return,single-event-loop,eventual-waitid
worker-process-creation=seccomp-deny-clone-clone3-fork-vfork
worker-signal-state=default-dispositions,empty-mask
root-journal-procfs=retained-PROC_SUPER_MAGIC,mount-namespace-and-path-identities
execution-wall-timeout-seconds=30
admission-max-bytes-per-poll=262144
admission-max-operations-per-poll=64
admission-cooperative-check-target-ms=10
max-authenticated-executions=64
max-plan-arguments=4092
max-producers=2048
max-unique-inputs=2048
max-input-bytes=268435456
max-retained-bytes=2147483648
max-output-bytes=536870912
max-cumulative-archive-members=262144
max-elf-program-headers=1024
max-elf-sections=8192
max-elf-table-entries=1048576
result-socket-child-fd=91
first-input-child-fd=100
argv[0]=fe2o3-host-lld
argv[1]=--fe2o3-host-lld-elf-v2
argv[2]=--fe2o3-result-socket-v1=91:<dev_decimal>:<ino_decimal>
argv[3]=--fe2o3-request-v1=<plan_sha256>:<closure_sha256>:<nonce_sha256>
argv[semantic]=--fe2o3-input-v1=<fd>:<kind>:<sha256hex>:<size_decimal>:<mode_octal>
input-kinds=elf-rel,archive,rlib
input-elf=x86_64-elf64-little-et-rel,bounded-subset,no-compressed-crel-bitcode-deplibs
archive-index=gnu32-and-long-names-validated,bsd-and-sym64-rejected
output-elf=x86_64-elf64-little-et-exec,bounded-static-subset,no-interp-dynamic-needed-wx-execstack
result-record-max-bytes=512
result-record=fe2o3-host-lld-result-v1\tplan=<hex>\tclosure=<hex>\tnonce=<hex>\tsha256=<hex>\tlength=<decimal>\tcopy=receiver-owned-memfd-v1\n
result-copy-policy=receiver-owned-memfd-v1
result-rights-count=1
result-sender-identity=regular,nlink:0,current-euid,tmpfs
result-seals=WRITE|GROW|SHRINK|SEAL
stable-closure-digest-excludes=result-socket-dev-inode,request-control'

actual_contract="$("${contract_bin[@]}" static-tool-contract-v1)"
if [[ "${actual_contract}" != "${expected_contract}" ]]; then
    diff -u <(printf '%s\n' "${expected_contract}") <(printf '%s\n' "${actual_contract}")
    echo "host-link static-tool contract drifted" >&2
    exit 1
fi

expected_rejections='unsupported-platform
io
invalid-version
invalid-wire
noncanonical-wire
plan-too-large
field-too-large
invalid-text
unsupported-argument
invalid-path
invalid-nonce
duplicate-record
noncanonical-order
digest-mismatch
replay-mismatch
wrong-target
wrong-nonce
not-regular
descriptor-changed
descriptor-unsealed
artifact-too-large
artifact-kind
thin-archive
symlink
root-changed
root-mutation
unresolved-search
unresolved-library
response-file
nested-response-file
linker-script
script-search-dir
script-include
absolute-nested-path
plugin
lto
unpublished-build-script
elf-policy
output-changed
output-empty
output-truncated
result-pending
worker-launch
worker-identity
worker-exit
worker-timeout
worker-capacity
tool-approval
runtime-dso-closure
invalid-state'

actual_rejections="$("${contract_bin[@]}" rejection-codes-v1)"
if [[ "${actual_rejections}" != "${expected_rejections}" ]]; then
    diff -u <(printf '%s\n' "${expected_rejections}") <(printf '%s\n' "${actual_rejections}")
    echo "host-link rejection table drifted" >&2
    exit 1
fi

if rg -n 'PublishedHostOutputDirectory|OutputDirectoryIdentity|output-dir-identity|/proc/self/fd/90|--fe2o3-host-lld-elf-v1' \
    crates/fe2o3-host-link-closure toolchains/rustc-host-link-handoff; then
    echo "rejected output-path or old protocol surface reappeared" >&2
    exit 1
fi

if rg -ni --glob '*.rs' --glob 'Cargo.toml' 'comgr' crates/fe2o3-host-link-closure; then
    echo "COMGR must not participate in host linking" >&2
    exit 1
fi

if rg -n '/proc/(self/fdinfo|[^ ]*/stat)' crates/fe2o3-host-link-closure/src/process.rs; then
    echo "unauthenticated procfs process identity reappeared" >&2
    exit 1
fi

if rg -n 'pidfd_open|Command::spawn|\.spawn\(\)' crates/fe2o3-host-link-closure/src/process.rs; then
    echo "post-spawn pidfd acquisition reappeared" >&2
    exit 1
fi

if rg -n 'pub fn (finish_child_handoff|admit_output|inherited_descriptors)\b' \
    crates/fe2o3-host-link-closure/src; then
    echo "unauthenticated result-channel or child-descriptor authority reappeared" >&2
    exit 1
fi

if rg -n 'copy_received_sealed_file|inspect_static_output_elf\(&captured\.bytes\)' \
    crates/fe2o3-host-link-closure/src/closure.rs; then
    echo "synchronous whole-output admission reappeared" >&2
    exit 1
fi

cargo test --locked -p fe2o3-host-link-closure --all-targets
run_combined_identity_hook
echo "host-link-closure hostile contract suite: PASS"
