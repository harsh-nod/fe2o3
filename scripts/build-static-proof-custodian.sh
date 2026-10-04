#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly repo_root
readonly target_dir="${FE2O3_STATIC_PROOF_CUSTODIAN_TARGET_DIR:-${repo_root}/target/static-proof-custodian}"
readonly target="x86_64-unknown-linux-musl"
readonly image_dir="${target_dir}/${target}/release"

cd -- "${repo_root}"
for name in fe2o3-proof-manager fe2o3-application-proof-controller fe2o3-proof-custodian-provision; do
  CARGO_TARGET_DIR="${target_dir}" cargo rustc \
    --locked --release --target "${target}" \
    -p fe2o3-proof-custodian --bin "${name}" -- \
    -C target-feature=+crt-static -C relocation-model=static \
    -C link-arg=-static -C link-arg=-no-pie \
    -C link-arg=-Wl,-e,fe2o3_secure_start_v1

  executable="${image_dir}/${name}"
  report="${target_dir}/${name}.readelf.txt"
  /usr/bin/readelf -hW -lW -dW -sW -- "${executable}" >"${report}"
  /usr/bin/grep -Eq 'Class:[[:space:]]+ELF64' "${report}"
  /usr/bin/grep -Eq 'Type:[[:space:]]+EXEC' "${report}"
  entry_address="$(/usr/bin/awk '/Entry point address:/ { print $4 }' "${report}")"
  secure_start_address="$(/usr/bin/nm -n --defined-only -- "${executable}" \
    | /usr/bin/awk '$3 == "fe2o3_secure_start_v1" { print "0x" $1 }')"
  if [[ -z "${entry_address}" || -z "${secure_start_address}" \
    || $((entry_address)) -ne $((secure_start_address)) ]]; then
    printf '%s does not enter through its secure pre-runtime shim\n' "${name}" >&2
    exit 1
  fi
  if /usr/bin/grep -Eq 'INTERP|DYNAMIC|\(NEEDED\)|\(RPATH\)|\(RUNPATH\)' "${report}"; then
    printf '%s contains a dynamic-loader dependency\n' "${name}" >&2
    exit 1
  fi
  /usr/bin/grep -Eq 'GNU_STACK.*RW[[:space:]]' "${report}"
  if [[ -n "$(/usr/bin/nm -u -- "${executable}")" ]]; then
    printf '%s contains undefined symbols\n' "${name}" >&2
    exit 1
  fi

  # Invalid argv must stop even a root caller before any installed service work.
  set +e
  smoke_output="$({ /usr/bin/timeout --kill-after=1s 5s /usr/bin/env -i \
    "${executable}" --invalid-arguments 3<&- 4<&- 5<&- 6<&- 7<&- 8<&-; } 2>&1)"
  smoke_status=$?
  set -e
  if [[ ${smoke_status} -ne 98 ]]; then
    printf '%s did not reject invalid entry within its deadline\n' "${name}" >&2
    exit 1
  fi
  case "${name}" in
    fe2o3-proof-manager)
      [[ "${smoke_output}" == 'proof manager: proof manager does not accept arguments' ]] ;;
    fe2o3-application-proof-controller)
      [[ -z "${smoke_output}" ]] ;;
    fe2o3-proof-custodian-provision)
      [[ "${smoke_output}" == 'proof provisioning: usage: '* ]] ;;
  esac
done

FE2O3_STATIC_PROOF_CUSTODIAN_DIR="${image_dir}" CARGO_TARGET_DIR="${target_dir}" \
  cargo test --locked --release --target "${target}" -p fe2o3-proof-custodian \
    --lib provisioning::tests::release_images_satisfy_production_static_elf_profile -- --exact --ignored
printf '%s\n' "${image_dir}"
