#!/usr/bin/env python3
"""Opt-in loopback-only CPU debugger bridge; never a public or GPU service."""
import argparse
import os
import signal
import socket
import sys
import time

from bridge_http import origin, read_request, response_bytes, write_response
from bridge_inputs import CustodyError, InputPins, MIB, TokenFile
from bridge_session import BridgeError, BridgeSession
from fe2o3_debug_console import KINDS, launch_arguments

SERVICE_SECONDS = 1800
MAX_ACCEPTED_SOCKETS = 1024


def serve_peer(peer, controller, token, host, allowed_origin):
    """One request per socket; authentication failures do not touch the child."""
    controller.last_dispatched = False
    try:
        token.check()
    except (CustodyError, OSError):
        controller.custody_failed = True
        controller.poison()
        return False  # Stop the service, not merely this unauthenticated socket.
    try:
        request = read_request(peer, host, allowed_origin, token.value)
        if request is None:
            wire = response_bytes(None, allowed_origin, preflight=True)
        else:
            result = controller.handle_request(request, peer)
            wire = response_bytes(result, allowed_origin)
    except BridgeError as error:
        error.closed = controller.closed
        wire = response_bytes(error.envelope(), allowed_origin)
    except (CustodyError, OSError):
        # An unauthenticated peer reset must not kill another authenticated session.
        if controller.last_dispatched:
            controller.poison()
        outcome = "unknown" if controller.last_dispatched else "not_sent"
        wire = response_bytes(BridgeError("backend_failed", outcome, controller.closed).envelope(),
                              allowed_origin)
    try:
        write_response(peer, wire)
    except (OSError, BridgeError):
        if controller.last_dispatched:
            controller.poison()
        # Generic transport failure: no request, response, token or stderr logging.
    return True


def run_service(controller, token, port, allowed_origin):
    started = time.monotonic()
    accepted = 0
    listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        # No address/port reuse and no wildcard/hostname bind.
        listener.bind(("127.0.0.1", port))
        listener.listen(1)
        listener.settimeout(0.2)
        while time.monotonic() - started < SERVICE_SECONDS and accepted < MAX_ACCEPTED_SOCKETS:
            controller.expire()  # Runs even if the browser sends no further requests.
            try:
                peer, address = listener.accept()
            except socket.timeout:
                continue
            accepted += 1
            with peer:
                peer.settimeout(0.2)
                if address[0] != "127.0.0.1":
                    continue
                if not serve_peer(peer, controller, token, "127.0.0.1:" + str(port), allowed_origin):
                    break
    finally:
        listener.close()
        reaped = controller.close()
    return 0 if reaped and not controller.custody_failed else 1


def observed_launch_arguments(args):
    """Only the local owner selects the additive CLI profile; no browser argv."""
    command = launch_arguments(args)
    profile = getattr(args, "runtime_observations", None)
    if profile is not None:
        if profile != "v1":
            raise ValueError("closed runtime observation profile")
        command.extend(("--runtime-observations", "v1"))
    return command


def arguments(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--binary-bytes", required=True, type=int)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--kind", choices=KINDS, required=True)
    parser.add_argument("--input", required=True)
    parser.add_argument("--input-bytes", required=True, type=int)
    parser.add_argument("--input-sha256", required=True)
    parser.add_argument("--request", required=True)
    parser.add_argument("--request-bytes", required=True, type=int)
    parser.add_argument("--request-sha256", required=True)
    parser.add_argument("--wave-width", choices=(32, 64), type=int, required=True)
    parser.add_argument("--runtime-observations", choices=("v1",),
                        help="owner-selected CPU runtime/storage observations; default unchanged")
    parser.add_argument("--token-file", required=True, help="owner-only 0600 file, 64 random lowercase hex")
    parser.add_argument("--port", required=True, type=int)
    parser.add_argument("--origin", required=True, help="exact http://127.0.0.1:PORT frontend origin")
    result = parser.parse_args(argv)
    if not 1 <= result.port <= 65535:
        parser.error("bounded loopback port required")
    try:
        result.origin = origin(result.origin)
    except ValueError:
        parser.error("explicit loopback origin required")
    return result


def main(argv=None):
    args = arguments(argv)
    token = inputs = controller = None
    previous_term = signal.getsignal(signal.SIGTERM)

    def interrupted(_signum, _frame):
        raise KeyboardInterrupt

    try:
        if os.name != "posix" or not sys.platform.startswith("linux"):
            raise ValueError("Linux local CPU profile required")
        command = observed_launch_arguments(args)  # Existing closed CLI/path owner plus explicit opt-in.
        token = TokenFile(args.token_file)
        inputs = InputPins(((args.binary, args.binary_bytes, args.binary_sha256, 512 * MIB, True),
                            (args.input, args.input_bytes, args.input_sha256, 64 * MIB, False),
                            (args.request, args.request_bytes, args.request_sha256, MIB, False)))
        controller = BridgeSession(command, inputs, runtime_observations=args.runtime_observations)
        signal.signal(signal.SIGTERM, interrupted)
        return run_service(controller, token, args.port, args.origin)
    except KeyboardInterrupt:
        return 130
    except (OSError, ValueError, BridgeError):
        # No exception interpolation: paths, tokens and backend bytes are not logged.
        print("CPU bridge stopped or refused; no session completion is implied.", file=sys.stderr)
        return 1
    finally:
        if controller is not None:
            controller.close()
        if inputs is not None:
            inputs.close()
        if token is not None:
            token.close()
        signal.signal(signal.SIGTERM, previous_term)


if __name__ == "__main__":
    sys.exit(main())
