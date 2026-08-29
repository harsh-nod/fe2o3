#!/usr/bin/env python3
import re
import os
import subprocess
import sys


def emit(line):
    sys.stdout.write(line + "\n")
    sys.stdout.flush()


emit('=thread-group-added,id="i1"')
emit('(gdb)')
inferior = None
pid_file = os.environ.get("FE2O3_FAKE_ROCGDB_PID_FILE")
if pid_file:
    with open(pid_file, "w", encoding="ascii") as output:
        output.write(str(os.getpid()) + "\n")
for raw in sys.stdin:
    raw = raw.rstrip("\n")
    match = re.fullmatch(r"([0-9]+)(-.*)", raw)
    if match is None:
        emit('^error,msg="missing token"')
        continue
    token, command = match.groups()
    prefix = token + "^"
    if command == "-gdb-set mi-async on":
        emit(prefix + "done")
    elif command == "-list-features":
        emit(prefix + 'done,features=["thread-info","data-read-memory-bytes","pending-breakpoints"]')
    elif command.startswith("-info-gdb-mi-command "):
        emit(prefix + 'done,command={exists="true"}')
    elif command.startswith("-file-exec-and-symbols ") or command.startswith("-exec-arguments"):
        emit(prefix + "done")
    elif command == "-exec-run":
        inferior = subprocess.Popen(["/bin/sleep", "60"])
        if pid_file:
            with open(pid_file, "a", encoding="ascii") as output:
                output.write(str(inferior.pid) + "\n")
        emit('=thread-group-started,id="i1",pid="' + str(inferior.pid) + '"')
        emit('*running,thread-id="all"')
        if os.environ.get("FE2O3_FAKE_ROCGDB_FAIL_RUN") == "1":
            emit(prefix + 'error,msg="fixture launch rejection"')
        else:
            emit(prefix + "running")
    elif command.startswith("-target-attach "):
        attached_pid = command.removeprefix("-target-attach ")
        emit('=thread-group-started,id="i1",pid="' + attached_pid + '"')
        emit(prefix + "done")
    elif command == "-thread-info":
        emit('~"ROCgdb private console text\\n"')
        emit('@"INFERIOR_NATIVE_SECRET\\n"')
        emit(prefix + 'done,threads=[{id="9",target-id="Host"}]')
        emit('*stopped,reason="signal-received",thread-id="9",frame={addr="0x1028"}')
    elif command == '-data-list-register-names --thread "9"':
        emit(prefix + 'done,register-names=["exec","pc"]')
    elif command == '-data-list-register-values --thread "9" x':
        emit(prefix + 'done,register-values=[{number="0",value="0x5"},{number="1",value="0x1028"}]')
    elif command == '-stack-list-variables --thread "9" --simple-values':
        emit(prefix + 'done,variables=[{name="kept",value="0x2a"},{name="gone",value="<optimized out>"}]')
    elif command.startswith('-data-evaluate-expression --thread "9" '):
        value = "0x22" if "second" in command else "0x11"
        emit(prefix + 'done,value="' + value + '"')
    elif command.startswith("-data-read-memory-bytes 0x2004 2"):
        emit(prefix + 'done,memory=[{begin="0x2004",offset="0x0",end="0x2006",contents="a10f"}]')
    elif command.startswith("-break-insert "):
        emit(prefix + 'done,bkpt={number="4"}')
    elif command.startswith('-exec-continue --thread "9"'):
        emit('*running,thread-id="9"')
        emit(prefix + "running")
    elif command == "-exec-interrupt --all":
        emit(prefix + "done")
        emit('*stopped,reason="signal-received",thread-id="9",frame={addr="0x1028"}')
    elif command.startswith('-exec-step-instruction --thread "9"'):
        emit(prefix + "running")
        emit('*running,thread-id="9"')
        emit('*stopped,reason="end-stepping-range",thread-id="9",frame={addr="0x102c"}')
    elif command == "-gdb-exit":
        if inferior is not None:
            inferior.terminate()
            inferior.wait(timeout=5)
            inferior = None
        emit(prefix + "exit")
        break
    else:
        emit(prefix + 'error,msg="unexpected structured command"')

if inferior is not None:
    inferior.terminate()
    inferior.wait(timeout=5)
