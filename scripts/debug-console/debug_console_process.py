"""One owned CPU-debugger child, selector-driven console and bounded pipes."""
import collections
import os
import selectors
import signal
import subprocess
import time

from debug_console_commands import CommandError, HELP, parse_command
from debug_console_protocol import LineFramer, ProtocolError, Session, encode, require

REQUEST_SECONDS = 30
SESSION_SECONDS = 900
DRAIN_SECONDS = 2
OUTPUT_QUEUE = 2 * 1024 * 1024
OUTPUT_TOTAL = 8 * 1024 * 1024

class ConsoleInput:
    def __init__(self):
        self.partial = bytearray()
        self.lines = collections.deque()
        self.total = 0
        self.commands = 0
        self.ended = False

    def feed(self, chunk):
        require(type(chunk) is bytes and len(chunk) <= 4096, "console chunk cap")
        require(self.total + len(chunk) <= 65536, "console cumulative input cap")
        self.total += len(chunk)
        parts = chunk.split(b"\n")
        for index, part in enumerate(parts):
            require(len(self.partial) + len(part) <= 1025, "console line cap")
            self.partial.extend(part)
            if index < len(parts) - 1:
                raw = bytes(self.partial)
                if raw.endswith(b"\r"):
                    raw = raw[:-1]  # Terminal CRLF only, not embedded controls.
                require(len(raw) <= 1024, "console line cap")
                require(len(self.lines) < 8 and self.commands < 256, "queued/total command cap")
                self.lines.append(raw.decode("utf-8", "strict"))
                self.commands += 1
                self.partial.clear()

    def eof(self):
        require(not self.partial, "console final command requires LF")
        self.ended = True

class Output:
    def __init__(self):
        self.pending = bytearray()
        self.total = 0
        self.since = None

    def append(self, data):
        require(len(self.pending) + len(data) <= OUTPUT_QUEUE, "console pending output cap")
        require(self.total + len(data) <= OUTPUT_TOTAL, "console cumulative output cap")
        self.total += len(data)
        if not self.pending:
            self.since = time.monotonic()
        self.pending.extend(data)

    def write(self, fd):
        try:
            count = os.write(fd, self.pending)
        except BlockingIOError:
            return
        require(count > 0, "console output closed")
        del self.pending[:count]
        if not self.pending:
            self.since = None

def _kill_owned(child, sig):
    # start_new_session=True owns this child's process group. Do not signal a
    # potentially recycled PID/group after the leader has already been reaped.
    if child.poll() is None:
        try:
            os.killpg(child.pid, sig)
        except ProcessLookupError:
            pass

def cleanup(child):
    """Bounded waits; failure to reap is reported, never claimed complete."""
    try:
        _kill_owned(child, signal.SIGTERM)
        try:
            child.wait(timeout=1)
        except subprocess.TimeoutExpired:
            _kill_owned(child, signal.SIGKILL)
            child.wait(timeout=1)
    except (OSError, subprocess.TimeoutExpired):
        return False
    return True

def check_deadlines(now, started, request_started, output_since, closing):
    require(now - started < SESSION_SECONDS, "console 900-second lifetime cap")
    if request_started is not None:
        require(now - request_started < REQUEST_SECONDS, "request timeout; outcome may be unknown")
    if output_since is not None:
        require(now - output_since < 5, "console output drain timeout")
    if closing is not None:
        require(now - closing < DRAIN_SECONDS, "debugger close/drain timeout")

def run_console(argv, input_fd=0, output_fd=1):
    require(os.name == "posix" and input_fd != output_fd, "distinct POSIX console descriptors required")
    child = None
    selector = selectors.DefaultSelector()
    previous = {}
    transcript = LineFramer()
    console = ConsoleInput()
    output = Output()
    session = Session()
    stderr = bytearray()
    outbound = bytearray()
    write_registered = False
    output_registered = False
    stdout_open = stderr_open = True
    request_started = None
    closing = None
    terminated = False
    failure = None
    started = time.monotonic()

    def send(body):
        nonlocal request_started
        require(not outbound, "one pending request write")
        request = session.begin(body)
        outbound.extend(encode(request) + b"\n")
        request_started = time.monotonic()

    def receive(response):
        nonlocal request_started, closing, terminated
        session.accept(response)  # No display/state commit before validation.
        request_started = None
        output.append(encode(response) + b"\n")
        if response["operation"] == "terminate":
            require(response["status"] == "ok", "backend refused termination")
            terminated = True
            closing = time.monotonic()
            child.stdin.close()
        else:
            output.append(b"fe2o3> ")

    try:
        for fd in (input_fd, output_fd):
            previous[fd] = os.get_blocking(fd)
        # Distinct descriptors may alias one terminal open-file description.
        # Observe every original mode before changing any shared O_NONBLOCK bit.
        for fd in (input_fd, output_fd):
            os.set_blocking(fd, False)
        child = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, bufsize=0, start_new_session=True,
                                 close_fds=True)
        for pipe in (child.stdin, child.stdout, child.stderr):
            os.set_blocking(pipe.fileno(), False)
        selector.register(input_fd, selectors.EVENT_READ, "input")
        selector.register(child.stdout, selectors.EVENT_READ, "stdout")
        selector.register(child.stderr, selectors.EVENT_READ, "stderr")
        output.append(HELP.encode("ascii"))
        send({"operation": "discover_capabilities"})
        while True:
            now = time.monotonic()
            check_deadlines(now, started, request_started, output.since, closing)
            if child.poll() is not None and not stdout_open and not stderr_open and not output.pending:
                require(terminated and session.pending is None and child.returncode == 0,
                        "unexpected debugger exit")
                break
            if session.pending is None and not outbound and not output.pending and not terminated:
                if console.lines:
                    line = console.lines.popleft()
                    try:
                        body = parse_command(line)
                        if body == "help":
                            output.append(HELP.encode("ascii") + b"fe2o3> ")
                        elif body is None:
                            output.append(b"fe2o3> ")
                        else:
                            send(body)
                    except CommandError as error:
                        output.append(("command error: " + str(error) + "\nfe2o3> ").encode("ascii"))
                elif console.ended:
                    send({"operation": "terminate"})
            if outbound and not write_registered:
                selector.register(child.stdin, selectors.EVENT_WRITE, "request")
                write_registered = True
            if output.pending and not output_registered:
                selector.register(output_fd, selectors.EVENT_WRITE, "display")
                output_registered = True
            for key, _events in selector.select(0.1):
                if key.data == "request":
                    try:
                        count = os.write(child.stdin.fileno(), outbound)
                    except BlockingIOError:
                        continue
                    require(count > 0, "debugger stdin closed")
                    del outbound[:count]
                    if not outbound:
                        selector.unregister(child.stdin)
                        write_registered = False
                    continue
                if key.data == "display":
                    output.write(output_fd)
                    if not output.pending:
                        selector.unregister(output_fd)
                        output_registered = False
                    continue
                fd = key.fd
                try:
                    chunk = os.read(fd, 4096)
                except BlockingIOError:
                    continue
                if not chunk:
                    selector.unregister(key.fileobj)
                    if key.data == "input":
                        console.eof()
                    elif key.data == "stdout":
                        transcript.eof()
                        require(session.pending is None and terminated, "unexpected debugger stdout EOF")
                        stdout_open = False
                    else:
                        stderr_open = False
                    continue
                if key.data == "input":
                    console.feed(chunk)
                elif key.data == "stdout":
                    require(not outbound, "response arrived before request fully written")
                    transcript.feed(chunk, receive)
                else:
                    require(len(stderr) + len(chunk) <= 65536, "cumulative stderr cap")
                    stderr.extend(chunk)
    except BaseException as error:
        failure = error
    finally:
        selector.close()
        reaped = True
        if child is not None:
            reaped = cleanup(child)
            for pipe in (child.stdin, child.stdout, child.stderr):
                if pipe is not None and not pipe.closed:
                    try:
                        pipe.close()
                    except OSError as error:
                        failure = failure or error
        for fd, was_blocking in previous.items():
            try:
                os.set_blocking(fd, was_blocking)
            except OSError as error:
                failure = failure or error
        if not reaped:
            failure = ProtocolError("cleanup timeout: owned child not confirmed reaped")
    if failure is not None:
        detail = encode({"console_error": str(failure)[:512],
                         "stderr_tail": bytes(stderr[-4096:]).decode("utf-8", "replace")})
        # Best effort, bounded diagnostics; do not block on a stalled stderr pipe.
        previous_error_mode = os.get_blocking(2)
        try:
            os.set_blocking(2, False)
            try:
                os.write(2, detail + b"\n")
            except (BlockingIOError, BrokenPipeError):
                pass
        finally:
            os.set_blocking(2, previous_error_mode)
        return 130 if isinstance(failure, KeyboardInterrupt) else 1
    return 0
