"""Opt-in kernel regression, only inside a reviewed read-only-host/private-/tmp namespace.

The BusyBox scripts are test programs, not admitted systemd or service evidence.
The actual static qualification image supplies all production helper transitions.
"""
import contextlib
import ctypes
import hashlib
import json
import os
from pathlib import Path
import select
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time

LIBC = ctypes.CDLL(None, use_errno=True)
LIBC.mount.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p,
                       ctypes.c_ulong, ctypes.c_char_p]
LIBC.mount.restype = ctypes.c_int
LIBC.umount2.argtypes = [ctypes.c_char_p, ctypes.c_int]
LIBC.umount2.restype = ctypes.c_int
LIBC.prctl.restype = ctypes.c_int
MAX_OUTPUT = 65536
PID1_COMMAND = '__systemd-preflight-pid1-v79'
PARENT_ENV = 'FE2O3_QUALIFICATION_SYSTEMD_PARENT_PID_V1'
CHECKS = b'''#!/bin/sh
set -eu
echo stderr-write-check >&2
test "$$" = 1
for item in /proc/[0-9]*; do test "$item" = /proc/1; done
count=0
while read -r id parent device root point flags rest; do
    if test "$point" = /proc; then
        count=$((count + 1))
        for flag in ro nosuid nodev noexec; do
            case ",$flags," in *,$flag,*) ;; *) exit 51 ;; esac
        done
    fi
done < /proc/1/mountinfo
test "$count" = 1
echo pid1=1
echo root=$(/bin/busybox stat -Lc '%d:%i' /)
echo namespace=$(/bin/busybox readlink /proc/1/ns/mnt)
'''


def syscall(result):
    if result != 0:
        error = ctypes.get_errno()
        raise OSError(error, os.strerror(error))


def identity(path):
    value = path.lstat()
    assert stat.S_ISREG(value.st_mode) and value.st_uid == 0 and value.st_nlink == 1
    assert value.st_size <= 8 * 1024**2
    before = (value.st_dev, value.st_ino, value.st_mode, value.st_size,
              value.st_mtime_ns, value.st_ctime_ns)
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    value = path.lstat()
    assert before == (value.st_dev, value.st_ino, value.st_mode, value.st_size,
                      value.st_mtime_ns, value.st_ctime_ns)
    return before, digest


@contextlib.contextmanager
def fixture(work, busybox, name, tail, proc_mutation=None):
    directory = work / name
    directory.mkdir(mode=0o700)
    for part in ['lower', 'upper', 'work', 'root']:
        (directory / part).mkdir(mode=0o755)
        (directory / part).chmod(0o755)
    lower, root = directory / 'lower', directory / 'root'
    for part in ['bin', 'usr', 'usr/bin', 'proc']:
        (lower / part).mkdir(mode=0o755)
        (lower / part).chmod(0o755)
    shutil.copyfile(busybox, lower / 'bin/busybox')
    (lower / 'bin/busybox').chmod(0o555)
    (lower / 'bin/sh').symlink_to('busybox')
    (lower / 'usr/bin/systemd-analyze').write_bytes(CHECKS + tail)
    (lower / 'usr/bin/systemd-analyze').chmod(0o555)
    if proc_mutation == 'mode':
        (lower / 'proc').chmod(0o700)
    elif proc_mutation == 'nonempty':
        (lower / 'proc/unexpected').write_bytes(b'not empty')
    elif proc_mutation in ['symlink', 'file']:
        (lower / 'proc').rmdir()
        if proc_mutation == 'symlink':
            (lower / 'proc').symlink_to('/')
        else:
            (lower / 'proc').write_bytes(b'not a directory')
    else:
        assert proc_mutation is None
    options = f'lowerdir={lower},upperdir={directory}/upper,workdir={directory}/work'.encode()
    syscall(LIBC.mount(b'overlay', os.fsencode(root), b'overlay', 2 | 4, options))
    try:
        yield root
    finally:
        # Success requires ordinary unmount; no lazy-unmount success fallback.
        syscall(LIBC.umount2(os.fsencode(root), 0))
        shutil.rmtree(directory)


def children(pid):
    path = Path(f'/proc/{pid}/task/{pid}/children')
    try:
        data = path.read_bytes()
    except FileNotFoundError:
        return []
    assert len(data) <= 4096
    values = [int(value) for value in data.split()]
    assert len(values) <= 16
    return values


def terminal(fd, timeout=5):
    poll = select.poll()
    poll.register(fd, select.POLLIN)
    events = poll.poll(int(timeout * 1000))
    assert events and events[0][1] & select.POLLIN, 'owned pidfd did not become terminal'


def reap_adopted():
    deadline = time.monotonic() + 5
    while children(os.getpid()):
        assert time.monotonic() < deadline, 'adopted descendants did not reap'
        for child in children(os.getpid()):
            try:
                os.waitpid(child, os.WNOHANG)
            except ChildProcessError:
                pass
        time.sleep(0.01)


def invoke(tool, root, work, name, parent_death=False):
    output, error = work / f'{name}.stdout', work / f'{name}.stderr'
    with contextlib.ExitStack() as files:
        root_fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
        files.callback(os.close, root_fd)
        stdout = files.enter_context(output.open('xb'))
        stderr = files.enter_context(error.open('xb'))
        child = subprocess.Popen([str(tool), '__systemd-preflight-tool-v1', 'systemd-version'],
                                 stdin=root_fd, stdout=stdout, stderr=stderr,
                                 env={PARENT_ENV: str(os.getpid())})
        descriptors = []
        try:
            if parent_death:
                deadline = time.monotonic() + 10
                while b'descendant-ready\n' not in output.read_bytes():
                    assert child.poll() is None, error.read_bytes()
                    assert output.stat().st_size <= MAX_OUTPUT
                    assert time.monotonic() < deadline, 'PID1 readiness deadline'
                    time.sleep(0.01)
                pid1 = children(child.pid)
                assert len(pid1) == 1, (child.pid, child.poll(), pid1,
                                       output.read_text(), error.read_text())
                descendants = children(pid1[0])
                assert len(descendants) == 1
                pids = [child.pid, pid1[0], descendants[0]]
                for pid in pids:
                    assert os.getpgid(pid) == os.getpgrp(), 'inherited group changed'
                    descriptors.append(os.pidfd_open(pid))
                child.kill()  # Only the owning helper, never PID1 or its descendant.
                assert child.wait(timeout=5) == -signal.SIGKILL
                for fd in descriptors:
                    terminal(fd)
                reap_adopted()
                status = -signal.SIGKILL
            else:
                status = child.wait(timeout=10)
        finally:
            if child.poll() is None:
                child.kill()
                child.wait(timeout=5)
            for fd in descriptors:
                os.close(fd)
    assert output.stat().st_size <= MAX_OUTPUT and error.stat().st_size <= MAX_OUTPUT
    return status, output.read_text(), error.read_text()


def bad_handle_child(tool, root, kind):
    """PID1 entry negatives run in a fresh child namespace, not an admitted tool."""
    with contextlib.ExitStack() as files:
        if kind in ['regular', 'closed']:
            descriptor = os.open('/dev/null', os.O_RDONLY | os.O_CLOEXEC)
        elif kind == 'sibling':
            descriptor = os.pidfd_open(os.getppid())
        elif kind == 'dead':
            donor = subprocess.Popen(['/bin/true'])
            descriptor = os.pidfd_open(donor.pid)
            assert donor.wait(timeout=5) == 0
        else:
            raise AssertionError(kind)
        files.callback(os.close, descriptor)
        os.unshare(os.CLONE_NEWNS | os.CLONE_NEWPID)
        syscall(LIBC.mount(None, b'/', None, (1 << 18) | (1 << 14), None))
        root_fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
        files.callback(os.close, root_fd)
        status = subprocess.run([str(tool), PID1_COMMAND, 'systemd-version', str(os.getpid())],
                                stdin=root_fd, stdout=subprocess.DEVNULL, stderr=descriptor,
                                env={}, timeout=10,
                                preexec_fn=(lambda: os.close(2)) if kind == 'closed' else None).returncode
        assert status == 1, (kind, status)
    print(f'bad-handle-{kind}=refused', flush=True)


def early_death_child(tool, root, gate):
    """Hold PID1 before exec so its genuine creating parent can die before binding."""
    assert stat.S_ISFIFO(os.fstat(gate).st_mode)
    descriptor = os.pidfd_open(os.getpid())
    root_fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
    try:
        os.unshare(os.CLONE_NEWNS | os.CLONE_NEWPID)
        syscall(LIBC.mount(None, b'/', None, (1 << 18) | (1 << 14), None))

        def await_release():
            assert os.read(gate, 1) == b'!'
            os.close(gate)

        child = subprocess.Popen([str(tool), PID1_COMMAND, 'systemd-version', str(os.getpid())],
                                 stdin=root_fd, stderr=descriptor, env={},
                                 pass_fds=(gate,), preexec_fn=await_release)
        # The driver kills this wrapper while Popen is still awaiting exec.
        child.wait(timeout=10)
        raise AssertionError('creating parent unexpectedly survived gate')
    finally:
        os.close(root_fd)
        os.close(descriptor)


def death_before_binding(tool, root, work):
    read_gate, write_gate = os.pipe2(os.O_CLOEXEC)
    child = None
    descriptors = []
    try:
        with (work / 'early.stdout').open('xb') as stdout, (work / 'early.stderr').open('xb') as stderr:
            child = subprocess.Popen([sys.executable, '-I', '-B', __file__, '--early-death',
                                      str(tool), str(root), str(read_gate)],
                                     stdout=stdout, stderr=stderr, env={}, pass_fds=(read_gate,))
            deadline = time.monotonic() + 5
            while not (pid1 := children(child.pid)):
                assert child.poll() is None and time.monotonic() < deadline
                time.sleep(0.01)
            assert len(pid1) == 1
            assert os.getpgid(pid1[0]) == os.getpgrp()
            descriptors = [os.pidfd_open(child.pid), os.pidfd_open(pid1[0])]
            child.kill()
            assert child.wait(timeout=5) == -signal.SIGKILL
            terminal(descriptors[0])
            assert os.write(write_gate, b'!') == 1
            terminal(descriptors[1])
            observed, status = os.waitpid(pid1[0], os.WNOHANG)
            assert observed == pid1[0] and os.waitstatus_to_exitcode(status) == 1
        assert (work / 'early.stdout').read_bytes() == b''
        assert list((root / 'proc').iterdir()) == []
    finally:
        for descriptor in descriptors:
            poll = select.poll()
            poll.register(descriptor, select.POLLIN)
            if not poll.poll(0):
                signal.pidfd_send_signal(descriptor, signal.SIGKILL)
                terminal(descriptor)
            os.close(descriptor)
        if child is not None and child.poll() is None:
            child.kill()
            child.wait(timeout=5)
        os.close(read_gate)
        os.close(write_gate)
        reap_adopted()


def main():
    if len(sys.argv) == 5 and sys.argv[1] == '--bad-handle':
        bad_handle_child(Path(sys.argv[2]), Path(sys.argv[3]), sys.argv[4])
        return
    if len(sys.argv) == 5 and sys.argv[1] == '--early-death':
        early_death_child(Path(sys.argv[2]), Path(sys.argv[3]), int(sys.argv[4]))
        return
    assert len(sys.argv) == 3 and os.geteuid() == 0
    tool, busybox = map(Path, sys.argv[1:])
    initial = {path: identity(path) for path in [tool, busybox]}
    mounts = Path('/proc/self/mountinfo').read_bytes()
    lines = mounts.decode().splitlines()
    assert any(line.split()[4] == '/' and 'ro' in line.split()[5].split(',') for line in lines)
    assert any(line.split()[4] == '/tmp' and ' - tmpfs ' in line for line in lines)
    assert not children(os.getpid())
    syscall(LIBC.prctl(36, 1, 0, 0, 0))  # Subreap only this dedicated test process.
    work = Path(tempfile.mkdtemp(prefix='preflight-pid1-', dir='/tmp'))
    outcomes = []
    try:
        for name, tail, expected in [('success', b'echo success\n', 0),
                                     ('failure', b'exit 7\n', 1),
                                     ('parent-death', b'/bin/busybox sh -c \'echo descendant-ready; exec /bin/busybox sleep 60\' <&0 &\nchild=$!\nwait "$child"\n', -9)]:
            with fixture(work, busybox, name, tail) as root:
                before = root.stat()
                status, stdout, stderr = invoke(tool, root, work, name, name == 'parent-death')
                assert status == expected, (name, status, stdout, stderr)
                assert 'pid1=1\n' in stdout
                assert f'root={before.st_dev}:{before.st_ino}\n' in stdout
                namespace = [line for line in stdout.splitlines() if line.startswith('namespace=')]
                assert len(namespace) == 1
                assert namespace[0] != 'namespace=' + os.readlink('/proc/self/ns/mnt')
                if name == 'failure':
                    assert 'exit_code=Some(7)' in stderr
                after = root.stat()
                assert (before.st_dev, before.st_ino, before.st_mode) == (after.st_dev, after.st_ino, after.st_mode)
                assert list((root / 'proc').iterdir()) == [], 'proc leaked into caller namespace'
                outcomes.append({'case': name, 'status': status, 'private_pid1_checks': True})
        with fixture(work, busybox, 'bad-handles', b'exit 99\n') as root:
            for kind in ['regular', 'sibling', 'dead', 'closed']:
                result = subprocess.run([sys.executable, '-I', '-B', __file__, '--bad-handle',
                                         str(tool), str(root), kind], timeout=15, check=True,
                                        capture_output=True, env={})
                assert result.stdout == f'bad-handle-{kind}=refused\n'.encode()
                outcomes.append({'case': kind, 'refused': True})
        with fixture(work, busybox, 'early-death', b'exit 99\n') as root:
            death_before_binding(tool, root, work)
            outcomes.append({'case': 'parent-death-before-binding', 'refused': True})
        for mutation in ['mode', 'nonempty', 'symlink', 'file']:
            name = f'proc-{mutation}'
            with fixture(work, busybox, name, b'exit 99\n', mutation) as root:
                status, stdout, stderr = invoke(tool, root, work, name)
                assert status == 1 and stdout == '', (name, status, stdout, stderr)
                outcomes.append({'case': name, 'refused': True})
    finally:
        reap_adopted()
        shutil.rmtree(work)
    assert not work.exists() and not children(os.getpid())
    assert Path('/proc/self/mountinfo').read_bytes() == mounts
    assert initial == {path: identity(path) for path in initial}
    print(json.dumps({'scope': 'Real kernel/helper lifecycle only, not systemd/service admission',
                      'outcomes': outcomes, 'owned_cleanup': True, 'parent_mounts_unchanged': True}))
    print('qualification_preflight_namespace_v79: PASS')


if __name__ == '__main__':
    main()
