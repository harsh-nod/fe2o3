#!/usr/bin/env bash
set -euo pipefail

die() {
  printf 'fe2o3-static-host-lld-build-guard-test: %s\n' "$*" >&2
  exit 1
}

sha256_file() {
  /usr/bin/sha256sum -- "$1" | /usr/bin/cut -d ' ' -f 1
}

canonical_file() {
  /usr/bin/readlink -f -- "$1"
}

write_root_manifest() {
  local label=$1 root=$2 output=$3

  walk_directory() {
    local directory=$1 relative=$2 path name child target
    printf 'D\t%s\t%s\n' "$relative" "$(/usr/bin/stat -Lc '%a' -- "$directory")"
    while IFS= read -r -d '' path; do
      name=${path##*/}
      if [[ "$relative" == . ]]; then
        child=$name
      else
        child=$relative/$name
      fi
      if [[ -L "$path" ]]; then
        target=$(/usr/bin/readlink -- "$path")
        printf 'L\t%s\t%s\n' "$child" "$target"
      elif [[ -d "$path" ]]; then
        walk_directory "$path" "$child"
      elif [[ -f "$path" ]]; then
        printf 'F\t%s\t%s\t%s\t%s\n' "$child" \
          "$(/usr/bin/stat -Lc '%a' -- "$path")" \
          "$(/usr/bin/stat -Lc '%s' -- "$path")" "$(sha256_file "$path")"
      else
        die "cannot manifest special entry $path"
      fi
    done < <(/usr/bin/find "$directory" -mindepth 1 -maxdepth 1 -print0 |
      /usr/bin/sort -z)
  }

  {
    printf 'fe2o3-static-host-lld-build-guard-root-v1\n'
    printf 'label=%s\n' "$label"
    walk_directory "$root" .
  } >"$output"
}

expect_failure() {
  local label=$1
  shift
  local stdout="$work/$label.stdout" stderr="$work/$label.stderr" status
  set +e
  "$@" >"$stdout" 2>"$stderr"
  status=$?
  set -e
  [[ $status -eq 70 ]] || die "$label returned $status instead of 70"
  /usr/bin/grep -Fq 'fe2o3-static-host-lld-build-guard:' "$stderr" ||
    die "$label omitted the deterministic guard diagnostic"
}

script_dir=$(/usr/bin/readlink -f -- "$(/usr/bin/dirname -- "$0")")
readonly script_dir
readonly source="$script_dir/fe2o3-static-host-lld-build-guard.cpp"
readonly bootstrap_helper="$script_dir/fe2o3-static-host-lld-build-bootstrap.sh"
readonly trace_source="$script_dir/fe2o3-static-host-lld-build-trace-check.cpp"
readonly tmp_redirect_source="$script_dir/fe2o3-static-host-lld-tmp-redirect.cpp"
work=$(/usr/bin/mktemp -d)
readonly work
cleanup() {
  /usr/bin/rm -rf -- "$work" "${absence_fixture:-}" \
    "${inherited_fd_outside:-}"
}
trap cleanup EXIT
readonly guard="$work/guard"
readonly trace_checker="$work/trace-checker"
readonly tmp_redirect="$work/tmp-redirect.so"
readonly realpath_probe="$work/realpath-probe"
readonly inherited_fd_probe="$work/inherited-fd-probe"
readonly ipc_probe="$work/ipc-probe"
readonly ipc_launcher="$work/ipc-launcher"
readonly tmp_bypass_probe="$work/tmp-bypass-probe"
readonly private_tmp="$work/private-tmp"
cxx=$(/usr/bin/readlink -f -- "${CXX:-/usr/bin/c++}")
readonly cxx

# shellcheck disable=SC1090,SC1091
source "$bootstrap_helper"
readonly bootstrap_source="$work/bootstrap-guard.cpp"
/usr/bin/install --mode=0644 -- "$source" "$bootstrap_source"
original_length=$(/usr/bin/stat -Lc '%s' -- "$bootstrap_source")
original_sha=$(sha256_file "$bootstrap_source")
readonly original_length original_sha
fe2o3_bootstrap_retain_file guard-source "$bootstrap_source" \
  "$original_sha" "$original_length" 644 || die "$FE2O3_BOOTSTRAP_ERROR"
"$cxx" -std=c++20 -O2 -Wall -Wextra -Wpedantic -Werror -Wconversion \
  -Wsign-conversion -fno-rtti -fno-ident -o "$guard" "$bootstrap_source" &
compile_pid=$!
/usr/bin/sleep 1
printf '\n' >>"$bootstrap_source"
/usr/bin/truncate -s "$original_length" -- "$bootstrap_source"
wait "$compile_pid" || die 'guard compilation failed during bootstrap attack test'
[[ $(sha256_file "$bootstrap_source") == "$original_sha" ]] ||
  die 'bootstrap attack did not restore the source bytes'
if fe2o3_bootstrap_verify_all guard-compilation-mutation-test; then
  die 'bootstrap retention accepted mutation and restore during guard compilation'
fi
"$cxx" -std=c++20 -O2 -Wall -Wextra -Wpedantic -Werror -Wconversion \
  -Wsign-conversion -fno-rtti -fno-ident -o "$trace_checker" "$trace_source"
"$cxx" -std=c++20 -O2 -Wall -Wextra -Wpedantic -Werror -Wconversion \
  -Wsign-conversion -fPIC -fvisibility=hidden -fno-ident \
  -fno-stack-protector -shared -nostdlib -nodefaultlibs -Wl,--build-id=none \
  -Wl,-z,noexecstack -Wl,-z,separate-code -o "$tmp_redirect" \
  "$tmp_redirect_source"
/usr/bin/chmod 0444 "$tmp_redirect"
/usr/bin/mkdir -- "$private_tmp"
"$cxx" -x c++ -std=c++20 -O2 -Wall -Wextra -Wpedantic -Werror \
  -Wconversion -Wsign-conversion -o "$realpath_probe" - <<'EOF'
#include <climits>
#include <cerrno>
#include <cstdlib>
#include <cstring>

int main(int argc, char **argv) {
  char resolved[PATH_MAX];
  if (argc != 2)
    return 2;
  if (::realpath("/tmp", resolved) == nullptr)
    return errno > 0 && errno < 126 ? errno : 125;
  return ::strcmp(resolved, argv[1]) == 0 ? 0 : 3;
}
EOF
"$cxx" -x c++ -std=c++20 -O2 -Wall -Wextra -Wpedantic -Werror \
  -Wconversion -Wsign-conversion -o "$inherited_fd_probe" - <<'EOF'
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/syscall.h>

#include <cerrno>
#include <cstdlib>
#include <cstring>
#include <fcntl.h>
#include <unistd.h>

extern char **environ;

int main(int argc, char **argv) {
  if (argc == 2 && std::strcmp(argv[1], "--exec-leaked") == 0)
    return 99;
  if (argc != 4)
    return 2;
  const int executable = std::atoi(argv[1]);
  const int readable = std::atoi(argv[2]);
  const int writable = std::atoi(argv[3]);
  struct stat input{};
  struct stat output{};
  struct stat error{};
  if (::fstat(STDIN_FILENO, &input) != 0 ||
      ::fstat(STDOUT_FILENO, &output) != 0 ||
      ::fstat(STDERR_FILENO, &error) != 0 || !S_ISCHR(input.st_mode) ||
      !S_ISCHR(output.st_mode) || !S_ISCHR(error.st_mode))
    return 3;
  char byte = 0;
  errno = 0;
  if (::read(readable, &byte, 1) != -1 || errno != EBADF)
    return 4;
  errno = 0;
  if (::write(writable, "x", 1) != -1 || errno != EBADF)
    return 5;
  errno = 0;
  if (::ftruncate(writable, 0) != -1 || errno != EBADF)
    return 6;
  errno = 0;
  void *mapping = ::mmap(nullptr, 4096, PROT_READ, MAP_PRIVATE, readable, 0);
  if (mapping != MAP_FAILED || errno != EBADF)
    return 7;
  char *const leakedArguments[] = {argv[0],
                                   const_cast<char *>("--exec-leaked"),
                                   nullptr};
  errno = 0;
  if (::syscall(SYS_execveat, executable, "", leakedArguments, environ,
                AT_EMPTY_PATH) != -1 ||
      errno != EBADF)
    return 8;
  return 0;
}
EOF
"$cxx" -x c++ -std=c++20 -O2 -Wall -Wextra -Wpedantic -Werror \
  -Wconversion -Wsign-conversion -o "$tmp_bypass_probe" - <<'EOF'
#include <linux/stat.h>
#include <sys/stat.h>
#include <sys/syscall.h>

#include <cerrno>
#include <fcntl.h>
#include <unistd.h>

int main(int argc, char **argv) {
  if (argc != 2)
    return 2;
  struct statx global{};
  struct statx privateDirectory{};
  if (::syscall(SYS_statx, AT_FDCWD, "/tmp", AT_SYMLINK_NOFOLLOW,
                STATX_TYPE | STATX_INO, &global) != 0 ||
      ::syscall(SYS_statx, AT_FDCWD, argv[1], AT_SYMLINK_NOFOLLOW,
                STATX_TYPE | STATX_INO, &privateDirectory) != 0 ||
      global.stx_ino == privateDirectory.stx_ino)
    return 3;
  struct stat metadata{};
  if (::syscall(SYS_newfstatat, AT_FDCWD, "/tmp", &metadata,
                AT_SYMLINK_NOFOLLOW) != 0 ||
      ::syscall(SYS_faccessat, AT_FDCWD, "/tmp", F_OK) != 0)
    return 4;
  char target[16]{};
  errno = 0;
  if (::syscall(SYS_readlinkat, AT_FDCWD, "/tmp", target, sizeof(target)) !=
          -1 ||
      errno != EINVAL)
    return 5;
  errno = 0;
  if (::syscall(SYS_openat, AT_FDCWD, "/tmp", O_RDONLY | O_DIRECTORY) != -1 ||
      errno != EACCES)
    return 6;
  errno = 0;
  if (::syscall(SYS_open, "/tmp", O_RDONLY | O_DIRECTORY) != -1 ||
      errno != EACCES)
    return 7;
  const int privateFd = static_cast<int>(
      ::syscall(SYS_openat, AT_FDCWD, argv[1], O_RDONLY | O_DIRECTORY));
  if (privateFd < 0)
    return 8;
  return ::close(privateFd) == 0 ? 0 : 9;
}
EOF
"$cxx" -x c++ -std=c++20 -O2 -Wall -Wextra -Wpedantic -Werror \
  -Wconversion -Wsign-conversion -o "$ipc_probe" - <<'EOF'
#include <linux/io_uring.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <sys/types.h>
#include <sys/uio.h>
#include <sys/un.h>

#include <cerrno>
#include <cstddef>
#include <cstring>
#include <fcntl.h>
#include <unistd.h>

template <typename Callable> bool denied(Callable operation) {
  errno = 0;
  return operation() == -1 && errno == EPERM;
}

int main() {
  errno = 0;
  if (::fcntl(42, F_GETFD) != -1 || errno != EBADF)
    return 2;
  int pair[2] = {-1, -1};
  sockaddr_un filesystem{};
  filesystem.sun_family = AF_UNIX;
  std::strcpy(filesystem.sun_path, "/tmp/fe2o3-denied-ipc.sock");
  sockaddr_un abstract{};
  abstract.sun_family = AF_UNIX;
  constexpr char abstractName[] = "fe2o3-denied-ipc";
  std::memcpy(abstract.sun_path + 1, abstractName, sizeof(abstractName) - 1);
  msghdr message{};
  mmsghdr messages[1]{};
  iovec local{};
  iovec remote{};
  socklen_t length = sizeof(filesystem);
  int option = 0;
  socklen_t optionLength = sizeof(option);
  io_uring_params ring{};
  constexpr long x32Tag = 0x40000000L;
  const bool passed =
      denied([&] {
        return static_cast<int>(
            ::syscall(static_cast<long>(SYS_socket) | x32Tag, AF_UNIX,
                      SOCK_STREAM, 0));
      }) &&
      denied([&] {
        return static_cast<int>(
            ::syscall(static_cast<long>(SYS_socketpair) | x32Tag, AF_UNIX,
                      SOCK_STREAM, 0, pair));
      }) &&
      denied([&] {
        return static_cast<int>(
            ::syscall(static_cast<long>(SYS_pidfd_getfd) | x32Tag, -1, 0, 0));
      }) &&
      denied([] { return ::socket(AF_UNIX, SOCK_STREAM, 0); }) &&
      denied([&] { return ::socketpair(AF_UNIX, SOCK_STREAM, 0, pair); }) &&
      denied([&] {
        return ::connect(-1, reinterpret_cast<sockaddr *>(&filesystem),
                         sizeof(filesystem));
      }) &&
      denied([&] {
        return ::connect(-1, reinterpret_cast<sockaddr *>(&abstract),
                         static_cast<socklen_t>(offsetof(sockaddr_un, sun_path) +
                                                sizeof(abstractName)));
      }) &&
      denied([&] {
        return ::bind(-1, reinterpret_cast<sockaddr *>(&filesystem),
                      sizeof(filesystem));
      }) &&
      denied([] { return ::listen(-1, 1); }) &&
      denied([] { return ::accept(-1, nullptr, nullptr); }) &&
      denied([] { return ::accept4(-1, nullptr, nullptr, SOCK_CLOEXEC); }) &&
      denied([&] {
        return ::getsockname(-1, reinterpret_cast<sockaddr *>(&filesystem),
                             &length);
      }) &&
      denied([&] {
        return ::getpeername(-1, reinterpret_cast<sockaddr *>(&filesystem),
                             &length);
      }) &&
      denied([&] { return ::sendmsg(-1, &message, 0); }) &&
      denied([&] { return ::recvmsg(-1, &message, 0); }) &&
      denied([&] { return ::sendmmsg(-1, messages, 1, 0); }) &&
      denied([&] { return ::recvmmsg(-1, messages, 1, 0, nullptr); }) &&
      denied([&] {
        return ::sendto(-1, "x", 1, 0,
                        reinterpret_cast<sockaddr *>(&filesystem),
                        sizeof(filesystem));
      }) &&
      denied([&] {
        return ::recvfrom(-1, nullptr, 0, 0,
                          reinterpret_cast<sockaddr *>(&filesystem), &length);
      }) &&
      denied([&] {
        return ::setsockopt(-1, SOL_SOCKET, SO_REUSEADDR, &option,
                            sizeof(option));
      }) &&
      denied([&] {
        return ::getsockopt(-1, SOL_SOCKET, SO_REUSEADDR, &option,
                            &optionLength);
      }) &&
      denied([] { return ::shutdown(-1, SHUT_RDWR); }) &&
      denied([] { return static_cast<int>(::syscall(SYS_pidfd_open, getpid(), 0)); }) &&
      denied([] { return static_cast<int>(::syscall(SYS_pidfd_getfd, -1, 0, 0)); }) &&
      denied([&] {
        return static_cast<int>(
            ::syscall(SYS_process_vm_readv, getpid(), &local, 1, &remote, 1, 0));
      }) &&
      denied([&] {
        return static_cast<int>(::syscall(SYS_process_vm_writev, getpid(),
                                          &local, 1, &remote, 1, 0));
      }) &&
      denied([&] {
        return static_cast<int>(::syscall(SYS_io_uring_setup, 1, &ring));
      }) &&
      denied([] { return static_cast<int>(::syscall(SYS_io_uring_enter, -1, 0, 0, 0, nullptr, 0)); }) &&
      denied([] { return static_cast<int>(::syscall(SYS_io_uring_register, -1, 0, nullptr, 0)); });
  return passed ? 0 : 3;
}
EOF
"$cxx" -x c++ -std=c++20 -O2 -Wall -Wextra -Wpedantic -Werror \
  -Wconversion -Wsign-conversion -o "$ipc_launcher" - <<'EOF'
#include <sys/socket.h>

#include <cerrno>
#include <cstring>
#include <fcntl.h>
#include <unistd.h>

int main(int argc, char **argv) {
  if (argc < 2)
    return 2;
  int sockets[2] = {-1, -1};
  if (::socketpair(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0, sockets) != 0)
    return 3;
  const int secret = ::open("/etc/passwd", O_RDONLY | O_CLOEXEC);
  if (secret < 0)
    return 4;
  char byte = 'x';
  iovec vector{&byte, 1};
  alignas(cmsghdr) char control[CMSG_SPACE(sizeof(int))]{};
  msghdr message{};
  message.msg_iov = &vector;
  message.msg_iovlen = 1;
  message.msg_control = control;
  message.msg_controllen = sizeof(control);
  cmsghdr *header = CMSG_FIRSTHDR(&message);
  if (header == nullptr)
    return 5;
  header->cmsg_level = SOL_SOCKET;
  header->cmsg_type = SCM_RIGHTS;
  header->cmsg_len = CMSG_LEN(sizeof(int));
  std::memcpy(CMSG_DATA(header), &secret, sizeof(secret));
  if (::sendmsg(sockets[0], &message, 0) != 1)
    return 6;
  if (::dup2(sockets[1], 42) != 42 ||
      ::fcntl(42, F_SETFD, 0) != 0)
    return 7;
  ::close(secret);
  ::close(sockets[0]);
  if (sockets[1] != 42)
    ::close(sockets[1]);
  ::execv(argv[1], argv + 1);
  return errno == 0 ? 8 : errno;
}
EOF

command=$(/usr/bin/readlink -f -- /usr/bin/bash)
readonly command
command_sha=$(sha256_file "$command")
readonly command_sha
command_length=$(/usr/bin/stat -Lc '%s' -- "$command")
readonly command_length
command_mode=$(/usr/bin/stat -Lc '%a' -- "$command")
readonly command_mode
landlock_args=(
  --landlock-writable-root test-work "$work"
  --landlock-read-write-file dev-null /dev/null
  --landlock-read-only system-library-root /usr/lib
  --landlock-read-only dynamic-loader-cache /etc/ld.so.cache
  --tmp-redirect "$tmp_redirect" "$private_tmp"
)
utility_file_args=()
utility_file_args+=(
  --file tmp-redirect "$tmp_redirect" "$(sha256_file "$tmp_redirect")" \
  "$(/usr/bin/stat -Lc '%s' -- "$tmp_redirect")" 444
  --file realpath-probe "$realpath_probe" "$(sha256_file "$realpath_probe")" \
  "$(/usr/bin/stat -Lc '%s' -- "$realpath_probe")" \
  "$(/usr/bin/stat -Lc '%a' -- "$realpath_probe")"
)
for probe in "$inherited_fd_probe" "$tmp_bypass_probe" "$ipc_probe"; do
  probe_name=${probe##*/}
  utility_file_args+=(
    --file "$probe_name" "$probe" "$(sha256_file "$probe")" \
    "$(/usr/bin/stat -Lc '%s' -- "$probe")" \
    "$(/usr/bin/stat -Lc '%a' -- "$probe")"
  )
done
dynamic_loader=$(/usr/bin/readlink -f -- /lib64/ld-linux-x86-64.so.2)
utility_file_args+=(
  --file dynamic-loader "$dynamic_loader" "$(sha256_file "$dynamic_loader")" \
  "$(/usr/bin/stat -Lc '%s' -- "$dynamic_loader")" \
  "$(/usr/bin/stat -Lc '%a' -- "$dynamic_loader")"
)
for utility_name in cp ln mkdir mv rm; do
  utility_path=$(/usr/bin/readlink -f -- "/usr/bin/$utility_name")
  utility_file_args+=(
    --file "utility-$utility_name" "$utility_path" \
    "$(sha256_file "$utility_path")" \
    "$(/usr/bin/stat -Lc '%s' -- "$utility_path")" \
    "$(/usr/bin/stat -Lc '%a' -- "$utility_path")"
  )
done
readonly dynamic_loader landlock_args utility_file_args

make_fixture() {
  local root=$1
  /usr/bin/mkdir -p -- "$root/a/nested"
  printf 'alpha\n' >"$root/a/one"
  printf 'beta\n' >"$root/two"
  /usr/bin/ln -s one "$root/a/one-link"
}

readonly fixture="$work/fixture"
make_fixture "$fixture"
readonly watched="$work/watched"
/usr/bin/mkdir -- "$watched"
readonly manifest="$work/fixture.manifest"
write_root_manifest fixture "$fixture" "$manifest"
root_sha=$(sha256_file "$manifest")
readonly root_sha
root_length=$(/usr/bin/stat -Lc '%s' -- "$manifest")
readonly root_length

guard_prefix=(
  "$guard"
  --max-entries 1000
  --max-depth 16
  --max-manifest-bytes 1048576
  --root fixture "$fixture" "$root_sha" "$root_length"
  --directory scripts-directory "$watched"
  --ancestor test-work-ancestor "$work"
  --file command "$command" "$command_sha" "$command_length" "$command_mode"
  "${utility_file_args[@]}"
  "${landlock_args[@]}"
)

readonly benign_status="$work/benign.status"
# shellcheck disable=SC2016
"${guard_prefix[@]}" --status "$benign_status" \
  --command 6 "$command" -c \
  '[[ /tmp -ef "$1" ]]; printf "ephemeral\n" >"$2"; rm "$2"' \
  guard-test "$private_tmp" "$work/ancestor-ephemeral" \
  --command 2 "$realpath_probe" "$private_tmp"
/usr/bin/grep -Fq 'STATUS=passed' "$benign_status" ||
  die 'benign guard run did not pass'
/usr/bin/grep -Fq \
  'SCOPE=measured-build-closure-integrity-with-landlock-filesystem-enforcement-and-observational-input-admission' \
  "$benign_status" || die 'guard status overstates its scope'
/usr/bin/grep -Fxq 'LANDLOCK_FILESYSTEM_ENFORCEMENT=passed' "$benign_status" ||
  die 'guard status omitted Landlock enforcement'
/usr/bin/grep -Fxq 'LANDLOCK_HANDLED_FS_RIGHTS=0x7fff' "$benign_status" ||
  die 'guard status omitted the complete ABI-4 filesystem rights mask'
/usr/bin/grep -Fxq \
  'INHERITED_AMBIENT_DESCRIPTORS=closed-before-child-exec' "$benign_status" ||
  die 'guard status omitted the closed child descriptor table'
/usr/bin/grep -Fxq \
  'NETWORK_IPC_ISOLATION=provided-by-seccomp-deny-policy-v1' \
  "$benign_status" || die 'guard status omitted seccomp network IPC isolation'
/usr/bin/grep -Fxq \
  'SECCOMP_X32_TAGGED_SYSCALLS=denied-with-EPERM-before-table-v1' \
  "$benign_status" || die 'guard status omitted x32-tagged syscall rejection'
/usr/bin/grep -Fxq \
  'PROCESS_CREATION=allowed-required-subprocesses-inherit-policy' \
  "$benign_status" || die 'guard status overstated process isolation'
/usr/bin/grep -Fxq \
  'STATUS_TERMINAL=fe2o3-static-host-lld-build-guard-status-v1-end' \
  "$benign_status" || die 'guard status omitted its terminal marker'
/usr/bin/grep -Fxq \
  'AMBIENT_TMP_ACCESS=landlock-open-denied-with-partial-libc-metadata-redirect' \
  "$benign_status" || die 'guard status overstates temporary-path redirection'
/usr/bin/grep -Fxq 'DIRECT_TMP_SYSCALL_REDIRECTION=not_provided' \
  "$benign_status" || die 'guard status omits direct temporary syscall limits'
/usr/bin/grep -Fxq \
  'STATUS_PUBLICATION=descriptor-bound-unprotected-measurement' \
  "$benign_status" || die 'guard status omitted publication scope'
/usr/bin/grep -Fxq 'PROTECTED_PUBLICATION=absent' "$benign_status" ||
  die 'guard status overstated publication protection'
observed_root_sha=$(
  /usr/bin/sed -n 's/^ROOT_fixture_MANIFEST_SHA256=//p' "$benign_status"
)
observed_root_length=$(
  /usr/bin/sed -n 's/^ROOT_fixture_MANIFEST_LENGTH=//p' "$benign_status"
)
[[ "$observed_root_sha" == "$root_sha" ]] ||
  die 'guard SHA-256 differs from the external manifest'
[[ "$observed_root_length" == "$root_length" ]] ||
  die 'guard manifest length differs from the external manifest'

readonly inherited_fd_outside="$work/../fe2o3-inherited-fd-outside-$$"
printf 'outside descriptor must stay unchanged\n' >"$inherited_fd_outside"
inherited_fd_outside_sha=$(sha256_file "$inherited_fd_outside")
readonly inherited_fd_outside_sha
exec 7<"$inherited_fd_probe"
exec 8</etc/passwd
exec 9<>"$inherited_fd_outside"
readonly inherited_fd_status="$work/inherited-fd.status"
"${guard_prefix[@]}" --status "$inherited_fd_status" --command 4 \
  "$inherited_fd_probe" 7 8 9
exec 7<&-
exec 8<&-
exec 9<&-
/usr/bin/grep -Fxq 'STATUS=passed' "$inherited_fd_status" ||
  die 'ambient descriptor closure attack did not pass safely'
[[ $(sha256_file "$inherited_fd_outside") == "$inherited_fd_outside_sha" ]] ||
  die 'child wrote or truncated through an inherited ambient descriptor'
/usr/bin/rm -- "$inherited_fd_outside"

readonly ipc_status="$work/ipc.status"
"$ipc_launcher" "${guard_prefix[@]}" --status "$ipc_status" --command 1 \
  "$ipc_probe"
/usr/bin/grep -Fxq 'STATUS=passed' "$ipc_status" ||
  die 'queued SCM_RIGHTS and denied IPC syscall test did not pass safely'

readonly tmp_bypass_status="$work/tmp-bypass.status"
"${guard_prefix[@]}" --status "$tmp_bypass_status" --command 2 \
  "$tmp_bypass_probe" "$private_tmp"
/usr/bin/grep -Fxq 'STATUS=passed' "$tmp_bypass_status" ||
  die 'direct temporary-path syscall boundary test did not pass'

absence_fixture="$work-absence"
readonly absence_fixture
/usr/bin/mkdir -- "$absence_fixture"
printf 'must not be readable\n' >"$absence_fixture/secret"
readonly absence_manifest="$work/absence.manifest"
write_root_manifest absence "$absence_fixture" "$absence_manifest"
absence_sha=$(sha256_file "$absence_manifest")
absence_length=$(/usr/bin/stat -Lc '%s' -- "$absence_manifest")
readonly absence_sha absence_length
# shellcheck disable=SC2016
expect_failure landlock-absence-root-read "$guard" --max-entries 1000 \
  --max-depth 16 --max-manifest-bytes 1048576 \
  --absence-root absence "$absence_fixture" "$absence_sha" "$absence_length" \
  --file command "$command" "$command_sha" "$command_length" "$command_mode" \
  "${utility_file_args[@]}" "${landlock_args[@]}" \
  --status "$work/landlock-absence-root-read.status" --command 5 \
  "$command" -c 'exec 3<"$1"; read -r _ <&3' guard-test \
  "$absence_fixture/secret"

# shellcheck disable=SC2016
expect_failure mutation-restore "${guard_prefix[@]}" \
  --status "$work/mutation-restore.status" --command 5 "$command" -c \
  'cp "$1/a/one" "$1/a/backup"; printf "changed\n" >"$1/a/one"; mv "$1/a/backup" "$1/a/one"' \
  guard-test "$fixture"

# shellcheck disable=SC2016
expect_failure directory-mutation-restore "${guard_prefix[@]}" \
  --status "$work/directory-mutation-restore.status" --command 5 "$command" -c \
  'printf "transient\n" >"$1/entry"; /usr/bin/rm -- "$1/entry"' \
  guard-test "$watched"

readonly status_race="$work/status-publication-race.status"
# shellcheck disable=SC2016
expect_failure status-publication-mutation "${guard_prefix[@]}" \
  --status "$status_race" --command 6 "$command" -c \
  '(for ((i = 0; i < 10000000; ++i)); do if [[ -e "$1" ]]; then original=$(<"$2"); printf "changed\n" >"$2"; printf "%s\n" "$original" >"$2"; exit; fi; done) & disown' \
  guard-test "$status_race" "$fixture/a/one"
[[ ! -e "$status_race" && ! -L "$status_race" ]] ||
  die 'rejected status-publication race retained a passing status'

readonly status_content_race="$work/status-content-race.status"
# shellcheck disable=SC2016
expect_failure status-publication-same-length-mutation "${guard_prefix[@]}" \
  --status "$status_content_race" --command 5 "$command" -c \
  '(for ((i = 0; i < 10000000; ++i)); do if [[ -e "$1" ]]; then exec 3<>"$1"; printf X >&3; exit; fi; done) & disown' \
  guard-test "$status_content_race"
[[ ! -e "$status_content_race" && ! -L "$status_content_race" ]] ||
  die 'same-length status mutation retained a passing status'

readonly ancestor_target="$work/ancestor-target"
/usr/bin/mkdir -- "$ancestor_target"
# shellcheck disable=SC2016
expect_failure ancestor-replacement-restore "${guard_prefix[@]}" \
  --ancestor replaceable-ancestor "$ancestor_target" \
  --status "$work/ancestor-replacement-restore.status" --command 5 \
  "$command" -c 'mv "$1" "$1.saved"; mv "$1.saved" "$1"' guard-test \
  "$ancestor_target"

# shellcheck disable=SC2016
expect_failure absent-path-creation "${guard_prefix[@]}" \
  --status "$work/absent-path-creation.status" --command 5 "$command" -c \
  'printf "created\n" >"$1/previously-absent"' guard-test "$fixture"

readonly replacement="$work/replacement"
make_fixture "$replacement"
readonly replacement_manifest="$work/replacement.manifest"
write_root_manifest replacement "$replacement" "$replacement_manifest"
replacement_sha=$(sha256_file "$replacement_manifest")
readonly replacement_sha
replacement_length=$(/usr/bin/stat -Lc '%s' -- "$replacement_manifest")
readonly replacement_length
# shellcheck disable=SC2016
expect_failure root-replacement "$guard" --max-entries 1000 --max-depth 16 \
  --max-manifest-bytes 1048576 \
  --root replacement "$replacement" "$replacement_sha" "$replacement_length" \
  --file command "$command" "$command_sha" "$command_length" "$command_mode" \
  "${utility_file_args[@]}" "${landlock_args[@]}" \
  --status "$work/root-replacement.status" --command 5 "$command" -c \
  'mv "$1" "$1.original"; mkdir "$1"; mv "$1.original" "$1.retained"' \
  guard-test "$replacement"

readonly symlink_target="$work/symlink-target"
make_fixture "$symlink_target"
/usr/bin/ln -s symlink-target "$work/symlink-root"
expect_failure symlink-root "$guard" --root symlink-root "$work/symlink-root" \
  "$root_sha" "$root_length" \
  --file command "$command" "$command_sha" "$command_length" "$command_mode" \
  "${utility_file_args[@]}" "${landlock_args[@]}" \
  --status "$work/symlink-root.status" --command 3 "$command" -c ':'

readonly escaping="$work/escaping"
make_fixture "$escaping"
/usr/bin/ln -s ../outside "$escaping/escape"
expect_failure escaping-symlink "$guard" --root escaping "$escaping" \
  "$root_sha" "$root_length" \
  --file command "$command" "$command_sha" "$command_length" "$command_mode" \
  "${utility_file_args[@]}" "${landlock_args[@]}" \
  --status "$work/escaping.status" --command 3 "$command" -c ':'

readonly special="$work/special"
make_fixture "$special"
/usr/bin/mkfifo "$special/fifo"
expect_failure special-file "$guard" --root special "$special" \
  "$root_sha" "$root_length" \
  --file command "$command" "$command_sha" "$command_length" "$command_mode" \
  "${utility_file_args[@]}" "${landlock_args[@]}" \
  --status "$work/special.status" --command 3 "$command" -c ':'

expect_failure entry-bound "$guard" --max-entries 1 --root fixture "$fixture" \
  "$root_sha" "$root_length" \
  --file command "$command" "$command_sha" "$command_length" "$command_mode" \
  "${utility_file_args[@]}" "${landlock_args[@]}" \
  --status "$work/entry-bound.status" --command 3 "$command" -c ':'
expect_failure depth-bound "$guard" --max-depth 1 --root fixture "$fixture" \
  "$root_sha" "$root_length" \
  --file command "$command" "$command_sha" "$command_length" "$command_mode" \
  "${utility_file_args[@]}" "${landlock_args[@]}" \
  --status "$work/depth-bound.status" --command 3 "$command" -c ':'

readonly landlock_outside="$work/../fe2o3-landlock-outside-$$"
readonly landlock_source="$work/landlock-source"
printf 'outside\n' >"$landlock_source"
/usr/bin/ln -s /etc/passwd "$work/landlock-symlink-escape"
# shellcheck disable=SC2016
expect_failure landlock-read-escape "${guard_prefix[@]}" \
  --status "$work/landlock-read-escape.status" --command 5 "$command" -c \
  'exec 3<"$1"; read -r _ <&3' guard-test /etc/passwd
# shellcheck disable=SC2016
expect_failure landlock-write-escape "${guard_prefix[@]}" \
  --status "$work/landlock-write-escape.status" --command 5 "$command" -c \
  'printf "escape\n" >"$1"' guard-test "$landlock_outside"
expect_failure landlock-exec-escape "${guard_prefix[@]}" \
  --status "$work/landlock-exec-escape.status" --command 3 \
  "$command" -c 'exec /usr/bin/true'
# shellcheck disable=SC2016
expect_failure landlock-symlink-read-escape "${guard_prefix[@]}" \
  --status "$work/landlock-symlink-read-escape.status" --command 5 \
  "$command" -c 'exec 3<"$1"; read -r _ <&3' guard-test \
  "$work/landlock-symlink-escape"
expect_failure landlock-symlink-create-denied "${guard_prefix[@]}" \
  --status "$work/landlock-symlink-create-denied.status" --command 4 \
  /usr/bin/ln -s /etc/passwd "$work/child-created-symlink"
expect_failure landlock-rename-escape "${guard_prefix[@]}" \
  --status "$work/landlock-rename-escape.status" --command 3 \
  /usr/bin/mv "$landlock_source" "$landlock_outside"
expect_failure landlock-link-escape "${guard_prefix[@]}" \
  --status "$work/landlock-link-escape.status" --command 3 \
  /usr/bin/ln "$landlock_source" "$landlock_outside"
[[ ! -e "$landlock_outside" && ! -L "$landlock_outside" ]] ||
  die 'Landlock escape test created an outside path'
[[ ! -e "$work/child-created-symlink" &&
  ! -L "$work/child-created-symlink" ]] ||
  die 'Landlock writable root allowed child symlink creation'

trace_exact="$work/trace-exact"
trace_root="$work/trace-root"
trace_build="$work/trace-build"
trace_artifact="$work/trace-artifact"
trace_absence="$work/trace-absence"
/usr/bin/mkdir -- "$trace_exact" "$trace_build" "$trace_artifact" \
  "$trace_absence"
readonly generated_exact="$trace_build/generated-exact"
printf 'generated exact input\n' >"$generated_exact"
for index in 0 1 2 3 4; do
  /usr/bin/mkdir -- "$trace_root-$index"
done
for ((index = 0; index < 80; index++)); do
  printf 'pinned %s\n' "$index" >"$trace_exact/file-$index"
done
trace_allowlist="$work/trace.allowlist"
{
  printf 'FORMAT=fe2o3-static-host-lld-trace-allowlist-v1\n'
  for ((index = 0; index < 80; index++)); do
    printf 'F\texact-%s\t%s/file-%s\n' "$index" "$trace_exact" "$index"
  done
  printf 'F\tcommand\t%s\n' "$command"
  printf 'F\tgenerated-exact\t%s\n' "$generated_exact"
  printf 'K\tproc-self-cgroup-denied\t/proc/self/cgroup\n'
  printf 'K\tdev-urandom\t/dev/urandom\n'
  printf 'K\tuser-home\t/home/harsh\n'
  printf 'K\tsource-parent\t/home/harsh\n'
  printf 'K\tsystem-root\t/usr\n'
  printf 'K\tsource-parent\t/usr\n'
  for index in 0 1 2 3 4; do
    printf 'R\troot-%s\t%s-%s\n' "$index" "$trace_root" "$index"
  done
  printf 'N\tabsence-root\t%s\n' "$trace_absence"
  printf 'O\tBUILD\t%s\n' "$trace_build"
  printf 'O\tARTIFACT\t%s\n' "$trace_artifact"
} >"$trace_allowlist"
unlisted="$work/unlisted-input"
printf 'unlisted\n' >"$unlisted"
/usr/bin/ln -s "$unlisted" "$trace_build/symlink-escape"
unlisted_directory="$work/unlisted-directory"
/usr/bin/mkdir -- "$unlisted_directory"
printf 'nested unlisted\n' >"$unlisted_directory/input"
/usr/bin/ln -s /etc/passwd "$unlisted_directory/passwd-link"
/usr/bin/ln -s "$unlisted_directory" "$trace_build/nested-escape"

write_trace() {
  local prefix=$1 syscall=$2
  printf '%s\n' \
    'execve("/usr/bin/bash", ["/usr/bin/bash"], 0x0) = 0' \
    "$syscall" >"$prefix.70001"
}

write_trace_records() {
  local prefix=$1
  shift
  printf '%s\n' 'execve("/usr/bin/bash", ["/usr/bin/bash"], 0x0) = 0' \
    "$@" >"$prefix.70001"
}

expect_trace_failure() {
  local label=$1 syscall=$2
  local prefix="$work/$label.raw"
  local canonical="$work/$label.canonical" inputs="$work/$label.inputs"
  local checked="$work/$label.checked" status
  write_trace "$prefix" "$syscall"
  set +e
  "$trace_checker" --check "$prefix" "$canonical" "$inputs" \
    "$trace_allowlist" "$trace_build" "$checked" >"$work/$label.stdout" \
    2>"$work/$label.stderr"
  status=$?
  set -e
  [[ $status -eq 70 ]] || die "$label trace check returned $status instead of 70"
  /usr/bin/grep -Fq 'fe2o3-static-host-lld-build-trace-check:' \
    "$work/$label.stderr" || die "$label omitted the trace-check diagnostic"
}

expect_trace_records_failure() {
  local label=$1
  shift
  local prefix="$work/$label.raw"
  local canonical="$work/$label.canonical" inputs="$work/$label.inputs"
  local checked="$work/$label.checked" status
  write_trace_records "$prefix" "$@"
  set +e
  "$trace_checker" --check "$prefix" "$canonical" "$inputs" \
    "$trace_allowlist" "$trace_build" "$checked" >"$work/$label.stdout" \
    2>"$work/$label.stderr"
  status=$?
  set -e
  [[ $status -eq 70 ]] || die "$label trace check returned $status instead of 70"
  /usr/bin/grep -Fq 'fe2o3-static-host-lld-build-trace-check:' \
    "$work/$label.stderr" || die "$label omitted the trace-check diagnostic"
}

expect_ephemeral_trace_success() {
  local label=$1 syscall=$2
  local prefix="$work/$label.raw"
  local canonical="$work/$label.canonical" inputs="$work/$label.inputs"
  local checked="$work/$label.checked"
  write_trace "$prefix" "$syscall"
  "$trace_checker" --check "$prefix" "$canonical" "$inputs" \
    "$trace_allowlist" "$trace_build" "$checked"
  /usr/bin/grep -Fq $'E\t' "$canonical" ||
    die "$label omitted its ephemeral output observation"
  ! /usr/bin/grep -Fq "$trace_build/" "$inputs" ||
    die "$label misclassified an output observation as an admitted input"
}

printf -v hostile_trace_byte '\377'
readonly hostile_trace_byte
nonascii_filename_prefix="$work/nonascii-filename.raw"
write_trace "$nonascii_filename_prefix" \
  "openat(AT_FDCWD<$trace_build>, \"$trace_exact/file-0\", O_RDONLY) = 3<$trace_exact/file-0>"
printf '%s\n' \
  'execve("/usr/bin/bash", ["/usr/bin/bash"], 0x0) = 0' \
  >"$nonascii_filename_prefix.$hostile_trace_byte"
set +e
"$trace_checker" --check "$nonascii_filename_prefix" \
  "$work/nonascii-filename.canonical" "$work/nonascii-filename.inputs" \
  "$trace_allowlist" "$trace_build" "$work/nonascii-filename.checked" \
  >"$work/nonascii-filename.stdout" 2>"$work/nonascii-filename.stderr"
nonascii_filename_status=$?
set -e
[[ $nonascii_filename_status -eq 70 ]] ||
  die 'non-ASCII per-PID trace filename was not rejected deterministically'

nonascii_prefix="$work/nonascii-$hostile_trace_byte.raw"
write_trace "$nonascii_prefix" \
  "openat(AT_FDCWD<$trace_build>, \"$trace_exact/file-0\", O_RDONLY) = 3<$trace_exact/file-0>"
set +e
"$trace_checker" --check "$nonascii_prefix" \
  "$work/nonascii-prefix.canonical" "$work/nonascii-prefix.inputs" \
  "$trace_allowlist" "$trace_build" "$work/nonascii-prefix.checked" \
  >"$work/nonascii-prefix.stdout" 2>"$work/nonascii-prefix.stderr"
nonascii_prefix_status=$?
set -e
[[ $nonascii_prefix_status -eq 70 ]] ||
  die 'non-ASCII trace prefix was not rejected deterministically'

valid_trace="$work/valid.raw"
write_trace "$valid_trace" \
  "openat(AT_FDCWD<$trace_build>, \"$trace_exact/file-0\", O_RDONLY) = 3<$trace_exact/file-0>"
"$trace_checker" --check "$valid_trace" "$work/valid.canonical" \
  "$work/valid.inputs" "$trace_allowlist" "$trace_build" \
  "$work/valid.checked"
/usr/bin/grep -Fq 'STATUS=measured-observational-admission' \
  "$work/valid.canonical" || die 'valid trace omitted measured admission status'
/usr/bin/grep -Fxq 'PER_FILE_BYTE_BOUND=67108864' "$work/valid.checked" ||
  die 'checked raw record omitted its per-file byte bound'
/usr/bin/grep -Fxq 'AGGREGATE_BYTE_BOUND=268435456' "$work/valid.checked" ||
  die 'checked raw record omitted its aggregate byte bound'
/usr/bin/grep -Fxq \
  'TERMINAL=fe2o3-static-host-lld-checked-raw-traces-v1-end' \
  "$work/valid.checked" || die 'checked raw record omitted its terminal marker'
readonly valid_retained="$work/valid-retained.raw"
"$trace_checker" --retain "$work/valid.checked" "$valid_trace" \
  "$valid_retained" "$work/valid.canonical" "$work/valid.inputs" \
  "$work/valid-retention.ledger" valid 65536 268435456
"$trace_checker" --check "$valid_retained" "$work/valid-replay.canonical" \
  "$work/valid-replay.inputs" "$trace_allowlist" "$trace_build" \
  "$work/valid-replay.checked"
/usr/bin/cmp --silent "$work/valid.canonical" "$work/valid-replay.canonical" ||
  die 'retained raw trace replay changed canonical evidence'
/usr/bin/cmp --silent "$work/valid.inputs" "$work/valid-replay.inputs" ||
  die 'retained raw trace replay changed admitted inputs'

mutation_trace="$work/checked-raw-mutation.raw"
write_trace "$mutation_trace" \
  "openat(AT_FDCWD<$trace_build>, \"$trace_exact/file-0\", O_RDONLY) = 3<$trace_exact/file-0>"
"$trace_checker" --check "$mutation_trace" \
  "$work/checked-raw-mutation.canonical" \
  "$work/checked-raw-mutation.inputs" "$trace_allowlist" "$trace_build" \
  "$work/checked-raw-mutation.checked"
exec 3<>"$mutation_trace.70001"
printf X >&3
exec 3>&-
set +e
"$trace_checker" --retain "$work/checked-raw-mutation.checked" \
  "$mutation_trace" "$work/checked-raw-mutation-retained.raw" \
  "$work/checked-raw-mutation.canonical" \
  "$work/checked-raw-mutation.inputs" \
  "$work/checked-raw-mutation.ledger" raw-mutation 65536 268435456 \
  >"$work/checked-raw-mutation.stdout" \
  2>"$work/checked-raw-mutation.stderr"
mutation_status=$?
set -e
[[ $mutation_status -eq 70 ]] ||
  die 'raw trace mutation after checking was accepted during retention'

canonical_mutation_trace="$work/checked-canonical-mutation.raw"
write_trace "$canonical_mutation_trace" \
  "openat(AT_FDCWD<$trace_build>, \"$trace_exact/file-0\", O_RDONLY) = 3<$trace_exact/file-0>"
"$trace_checker" --check "$canonical_mutation_trace" \
  "$work/checked-canonical-mutation.canonical" \
  "$work/checked-canonical-mutation.inputs" "$trace_allowlist" \
  "$trace_build" "$work/checked-canonical-mutation.checked"
/usr/bin/chmod 0644 "$work/checked-canonical-mutation.canonical"
exec 3<>"$work/checked-canonical-mutation.canonical"
printf X >&3
exec 3>&-
set +e
"$trace_checker" --retain "$work/checked-canonical-mutation.checked" \
  "$canonical_mutation_trace" \
  "$work/checked-canonical-mutation-retained.raw" \
  "$work/checked-canonical-mutation.canonical" \
  "$work/checked-canonical-mutation.inputs" \
  "$work/checked-canonical-mutation.ledger" canonical-mutation 65536 \
  268435456 \
  >"$work/checked-canonical-mutation.stdout" \
  2>"$work/checked-canonical-mutation.stderr"
canonical_mutation_status=$?
set -e
[[ $canonical_mutation_status -eq 70 ]] ||
  die 'canonical trace mutation after checking was accepted during retention'

global_retention_bytes=0
for phase_number in 1 2 3 4 5 6; do
  phase_prefix="$work/global-retention-$phase_number.raw"
  write_trace "$phase_prefix" \
    "openat(AT_FDCWD<$trace_build>, \"$trace_exact/file-0\", O_RDONLY) = 3<$trace_exact/file-0>"
  "$trace_checker" --check "$phase_prefix" \
    "$work/global-retention-$phase_number.canonical" \
    "$work/global-retention-$phase_number.inputs" "$trace_allowlist" \
    "$trace_build" "$work/global-retention-$phase_number.checked"
  if ((phase_number <= 5)); then
    ((global_retention_bytes += $(/usr/bin/stat -Lc '%s' -- \
      "$phase_prefix.70001")))
  fi
done
readonly global_retention_ledger="$work/global-retention.ledger"
for phase_number in 1 2 3 4 5; do
  "$trace_checker" --retain \
    "$work/global-retention-$phase_number.checked" \
    "$work/global-retention-$phase_number.raw" \
    "$work/global-retained-$phase_number.raw" \
    "$work/global-retention-$phase_number.canonical" \
    "$work/global-retention-$phase_number.inputs" "$global_retention_ledger" \
    "phase-$phase_number" 6 "$global_retention_bytes"
done
global_retention_ledger_sha=$(sha256_file "$global_retention_ledger")
set +e
"$trace_checker" --retain "$work/global-retention-6.checked" \
  "$work/global-retention-6.raw" "$work/global-retained-6.raw" \
  "$work/global-retention-6.canonical" "$work/global-retention-6.inputs" \
  "$global_retention_ledger" phase-6 6 "$global_retention_bytes" \
  >"$work/global-retention-overflow.stdout" \
  2>"$work/global-retention-overflow.stderr"
global_retention_status=$?
set -e
[[ $global_retention_status -eq 70 ]] ||
  die 'multiphase global raw trace byte overflow was accepted'
[[ ! -e "$work/global-retained-6.raw.70001" ]] ||
  die 'global raw trace overflow copied a destination before rejection'
[[ $(sha256_file "$global_retention_ledger") == \
  "$global_retention_ledger_sha" ]] ||
  die 'global raw trace overflow changed its cumulative ledger'
/usr/bin/grep -Fxq 'FILES=5' "$global_retention_ledger" ||
  die 'global raw trace ledger has the wrong file total'
/usr/bin/grep -Fxq "TOTAL_BYTES=$global_retention_bytes" \
  "$global_retention_ledger" ||
  die 'global raw trace ledger has the wrong byte total'
/usr/bin/grep -Fxq \
  'TERMINAL=fe2o3-static-host-lld-global-retention-ledger-v1-end' \
  "$global_retention_ledger" ||
  die 'global raw trace ledger omitted its terminal record'

oversized_trace="$work/oversized.raw"
/usr/bin/truncate -s 67108865 "$oversized_trace.70001"
set +e
"$trace_checker" --check "$oversized_trace" "$work/oversized.canonical" \
  "$work/oversized.inputs" "$trace_allowlist" "$trace_build" \
  "$work/oversized.checked" >"$work/oversized.stdout" \
  2>"$work/oversized.stderr"
oversized_status=$?
set -e
[[ $oversized_status -eq 70 ]] ||
  die 'per-PID raw trace byte bound was not enforced'
generated_exact_trace="$work/generated-exact.raw"
write_trace "$generated_exact_trace" \
  "openat(AT_FDCWD<$trace_build>, \"$generated_exact\", O_RDONLY) = 3<$generated_exact>"
"$trace_checker" --check "$generated_exact_trace" \
  "$work/generated-exact.canonical" "$work/generated-exact.inputs" \
  "$trace_allowlist" "$trace_build" "$work/generated-exact.checked"
/usr/bin/grep -Fxq $'F\tgenerated-exact\t$BUILD/generated-exact' \
  "$work/generated-exact.inputs" ||
  die 'generated exact tool was misclassified as an ephemeral output'
device_trace="$work/device.raw"
write_trace "$device_trace" \
  'openat(AT_FDCWD, "/dev/urandom", O_RDONLY) = 3</dev/urandom<char 1:9>>'
"$trace_checker" --check "$device_trace" "$work/device.canonical" \
  "$work/device.inputs" "$trace_allowlist" "$trace_build" \
  "$work/device.checked"
/usr/bin/grep -Fq $'K\topenat-result\tdev-urandom\t/dev/urandom' \
  "$work/device.canonical" ||
  die 'nested character-device descriptor annotation was not admitted'

topology_trace="$work/topology.raw"
write_trace_records "$topology_trace" \
  'stat("/home/harsh", {st_mode=S_IFDIR|0755}) = 0' \
  'stat("/usr", {st_mode=S_IFDIR|0755}) = 0'
"$trace_checker" --check "$topology_trace" "$work/topology.canonical" \
  "$work/topology.inputs" "$trace_allowlist" "$trace_build" \
  "$work/topology.checked"
/usr/bin/grep -Fxq $'K\tsource-parent+user-home\t/home/harsh' \
  "$work/topology.inputs" ||
  die 'source directly under user home did not merge duplicate topology rows'
/usr/bin/grep -Fxq $'K\tsource-parent+system-root\t/usr' \
  "$work/topology.inputs" ||
  die 'non-home source parent did not merge duplicate topology rows'

expect_trace_failure unlisted-openat \
  "openat(AT_FDCWD<$trace_build>, \"$unlisted\", O_RDONLY) = 3<$unlisted>"
expect_trace_failure output-symlink-openat \
  "openat(AT_FDCWD<$trace_build>, \"$trace_build/symlink-escape\", O_RDONLY) = 3<$unlisted>"
expect_trace_failure output-symlink-stat \
  "stat(\"$trace_build/symlink-escape\", {st_mode=S_IFREG}) = 0"
expect_trace_failure output-symlink-access \
  "access(\"$trace_build/symlink-escape\", R_OK) = 0"
expect_trace_failure output-symlink-statx \
  "statx(AT_FDCWD<$trace_build>, \"$trace_build/symlink-escape\", 0, STATX_ALL, {stx_mode=S_IFREG}) = 0"
expect_trace_failure output-symlink-faccessat \
  "faccessat(AT_FDCWD<$trace_build>, \"$trace_build/symlink-escape\", R_OK) = 0"
expect_trace_failure output-symlink-faccessat2 \
  "faccessat2(AT_FDCWD<$trace_build>, \"$trace_build/symlink-escape\", R_OK, 0) = 0"
expect_ephemeral_trace_success output-symlink-lstat \
  "lstat(\"$trace_build/symlink-escape\", {st_mode=S_IFLNK}) = 0"
expect_ephemeral_trace_success output-symlink-readlink \
  "readlink(\"$trace_build/symlink-escape\", \"$unlisted\", 4096) = ${#unlisted}"
expect_trace_failure output-nested-symlink-stat \
  "stat(\"$trace_build/nested-escape/input\", {st_mode=S_IFREG}) = 0"
expect_trace_failure output-nested-symlink-access \
  "access(\"$trace_build/nested-escape/input\", R_OK) = 0"
expect_trace_failure output-nested-symlink-open \
  "openat(AT_FDCWD<$trace_build>, \"$trace_build/nested-escape/input\", O_RDONLY) = 3<$unlisted_directory/input>"
expect_trace_failure output-nested-symlink-readlink \
  "readlink(\"$trace_build/nested-escape/passwd-link\", \"/etc/passwd\", 4096) = 11"
expect_trace_failure unlisted-relative-dirfd \
  "openat(3<$work>, \"unlisted-input\", O_RDONLY) = 4<$unlisted>"
expect_trace_failure unlisted-newfstatat \
  "newfstatat(AT_FDCWD<$trace_build>, \"$unlisted\", {st_mode=S_IFREG}, 0) = 0"
expect_trace_failure unlisted-access \
  "access(\"$unlisted\", R_OK) = 0"
expect_trace_failure unlisted-readlink \
  "readlink(\"$unlisted\", \"target\", 4096) = 6"
expect_trace_failure output-unconfirmed-stat-after-delete \
  "newfstatat(AT_FDCWD<$trace_build>, \"$trace_build/deleted-unconfirmed\", {st_mode=S_IFREG}, 0) = 0"
expect_trace_failure output-deleted-symlink-stat-race \
  "stat(\"$trace_build/deleted-stat-symlink\", {st_mode=S_IFREG}) = 0"
expect_trace_failure output-deleted-symlink-access-race \
  "access(\"$trace_build/deleted-access-symlink\", R_OK) = 0"
expect_trace_failure output-deleted-symlink-statx-race \
  "statx(AT_FDCWD<$trace_build>, \"$trace_build/deleted-statx-symlink\", 0, STATX_ALL, {stx_mode=S_IFREG}) = 0"
expect_trace_failure output-deleted-symlink-faccessat-race \
  "faccessat(AT_FDCWD<$trace_build>, \"$trace_build/deleted-faccessat-symlink\", R_OK) = 0"
expect_trace_records_failure output-racing-symlink-create-use-delete \
  "symlink(\"$unlisted\", \"$trace_build/racing-symlink\") = 0" \
  "newfstatat(AT_FDCWD<$trace_build>, \"$trace_build/racing-symlink\", {st_mode=S_IFREG}, 0) = 0" \
  "unlink(\"$trace_build/racing-symlink\") = 0"
create_stat_delete_trace="$work/output-create-stat-delete.raw"
write_trace_records "$create_stat_delete_trace" \
  "openat(AT_FDCWD<$trace_build>, \"$trace_build/deleted-before-check\", O_RDWR|O_CREAT, 0600) = 3<$trace_build/deleted-before-check>" \
  "newfstatat(AT_FDCWD<$trace_build>, \"$trace_build/deleted-before-check\", {st_mode=S_IFREG}, 0) = 0" \
  "unlink(\"$trace_build/deleted-before-check\") = 0"
"$trace_checker" --check "$create_stat_delete_trace" \
  "$work/output-create-stat-delete.canonical" \
  "$work/output-create-stat-delete.inputs" "$trace_allowlist" "$trace_build" \
  "$work/output-create-stat-delete.checked"
/usr/bin/grep -Fq 'prior-fd-resolved-landlock-output-ephemeral' \
  "$work/output-create-stat-delete.canonical" ||
  die 'descriptor-confirmed create-stat-delete output was not retained'
expect_ephemeral_trace_success output-create-open-delete \
  "openat(AT_FDCWD<$trace_build>, \"$trace_build/deleted-open\", O_RDONLY) = 3<$trace_build/deleted-open (deleted)>"
expect_trace_failure unlisted-mmap \
  "mmap(NULL, 4096, PROT_READ, MAP_PRIVATE, 3<$unlisted>, 0) = 0x1000"
expect_trace_failure unlisted-read-fd \
  "read(3<$unlisted>, \"x\", 1) = 1"
expect_trace_failure unlisted-rename-second \
  "rename(\"$trace_exact/file-0\", \"$unlisted\") = 0"
expect_trace_failure unlisted-link-second \
  "link(\"$trace_exact/file-0\", \"$unlisted\") = 0"
expect_trace_failure unlisted-missing-probe \
  "newfstatat(AT_FDCWD<$trace_build>, \"$work/unlisted-missing\", 0x0, 0) = -1 ENOENT (No such file or directory)"
proc_denial_trace="$work/proc-denial.raw"
write_trace "$proc_denial_trace" \
  'openat(AT_FDCWD, "/proc/self/cgroup", O_RDONLY) = -1 EACCES (Permission denied)'
"$trace_checker" --check "$proc_denial_trace" \
  "$work/proc-denial.canonical" "$work/proc-denial.inputs" \
  "$trace_allowlist" "$trace_build" "$work/proc-denial.checked"
/usr/bin/grep -Fq $'N\topenat\tproc-self-cgroup-denied\t/proc/self/cgroup\tEACCES' \
  "$work/proc-denial.canonical" ||
  die 'exact proc denial was not retained without PID canonicalization'
absence_probe_trace="$work/absence-probe.raw"
write_trace "$absence_probe_trace" \
  "newfstatat(AT_FDCWD<$trace_build>, \"$trace_absence/missing\", 0x0, 0) = -1 ENOENT (No such file or directory)"
"$trace_checker" --check "$absence_probe_trace" \
  "$work/absence-probe.canonical" "$work/absence-probe.inputs" \
  "$trace_allowlist" "$trace_build" "$work/absence-probe.checked"
/usr/bin/grep -Fq $'N\tnewfstatat\tabsence-root' \
  "$work/absence-probe.canonical" ||
  die 'absence-only root did not retain its failed probe'
empty_probe_trace="$work/empty-probe.raw"
write_trace "$empty_probe_trace" \
  'mkdir("", 0777) = -1 ENOENT (No such file or directory)'
"$trace_checker" --check "$empty_probe_trace" \
  "$work/empty-probe.canonical" "$work/empty-probe.inputs" \
  "$trace_allowlist" "$trace_build" "$work/empty-probe.checked"
/usr/bin/grep -Fq $'N\tmkdir\tkernel-invalid-empty-path\t<empty>\tENOENT' \
  "$work/empty-probe.canonical" ||
  die 'kernel-invalid empty pathname was not retained as absence evidence'
expect_trace_failure empty-path-success 'mkdir("", 0777) = 0'
expect_trace_failure empty-path-wrong-errno \
  'mkdir("", 0777) = -1 EACCES (Permission denied)'
printf 'existing\n' >"$trace_absence/existing"
expect_trace_failure absence-root-successful-read \
  "openat(AT_FDCWD<$trace_build>, \"$trace_absence/existing\", O_RDONLY) = 3<$trace_absence/existing>"
expect_trace_failure unfinished-record \
  "openat(AT_FDCWD<$trace_build>, \"$unlisted\", O_RDONLY <unfinished ...>"
expect_trace_failure resumed-record \
  "<... openat resumed>) = 3<$unlisted>"
expect_trace_failure truncated-record \
  "openat(AT_FDCWD<$trace_build>, \"$unlisted, O_RDONLY) = 3<$unlisted>"
expect_trace_failure unknown-path-syscall \
  "open_tree(AT_FDCWD<$trace_build>, \"$unlisted\", 0) = 3<$unlisted>"

printf 'fe2o3 static host LLD build guard tests passed\n'
