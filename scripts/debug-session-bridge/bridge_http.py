"""Small closed HTTP framing profile; no generic web framework or request logging."""
import hmac
import json
import re
import socket
import time

from bridge_session import BridgeError, RESPONSE_SCHEMA

BODY_CAP = 4096
HEADER_CAP = 8192
RESPONSE_CAP = 1024 * 1024
HTTP_SECONDS = 5
PATHS = {"/v1/connect": "connect", "/v1/command": "command", "/v1/disconnect": "disconnect"}


def origin(value):
    match = re.fullmatch(r"http://127\.0\.0\.1:([1-9][0-9]{0,4})", value or "")
    if not match or not 1 <= int(match.group(1)) <= 65535:
        raise ValueError("explicit loopback origin required")
    return value


def parse_headers(raw):
    if type(raw) is not bytes or len(raw) > HEADER_CAP or not raw.endswith(b"\r\n\r\n"):
        raise BridgeError("invalid_request")
    try:
        lines = raw[:-4].decode("ascii", "strict").split("\r\n")
    except UnicodeError:
        raise BridgeError("invalid_request") from None
    if not lines or len(lines) > 33 or len(lines[0]) > 2048:
        raise BridgeError("invalid_request")
    parts = lines[0].split(" ")
    if len(parts) != 3 or parts[0] not in ("POST", "OPTIONS") or parts[2] != "HTTP/1.1":
        raise BridgeError("invalid_request")
    method, path = parts[:2]
    if path not in PATHS:
        raise BridgeError("invalid_request")
    headers = {}
    for line in lines[1:]:
        if len(line) > 1024 or ":" not in line:
            raise BridgeError("invalid_request")
        name, value = line.split(":", 1)
        if not re.fullmatch(r"[A-Za-z0-9-]+", name):
            raise BridgeError("invalid_request")
        if any(ord(c) < 32 or ord(c) == 127 for c in value):
            raise BridgeError("invalid_request")
        name, value = name.lower(), value.strip(" ")
        if name in headers:
            raise BridgeError("invalid_request")
        headers[name] = value
    return method, path, headers


def validate_headers(method, headers, host, allowed_origin, token):
    if headers.get("host") != host or headers.get("origin") != allowed_origin:
        raise BridgeError("invalid_request")
    if any(key in headers for key in ("transfer-encoding", "expect", "content-encoding",
                                      "cookie", "authorization",
                                      "access-control-request-private-network")):
        raise BridgeError("invalid_request")
    if method == "OPTIONS":
        if headers.get("access-control-request-method") != "POST":
            raise BridgeError("invalid_request")
        requested = headers.get("access-control-request-headers", "").lower().split(",")
        if sorted(item.strip(" ") for item in requested) != ["content-type", "x-fe2o3-bridge-token"]:
            raise BridgeError("invalid_request")
        if headers.get("content-length", "0") != "0" or "x-fe2o3-bridge-token" in headers:
            raise BridgeError("invalid_request")
        return 0
    candidate = headers.get("x-fe2o3-bridge-token", "")
    if not re.fullmatch(r"[0-9a-f]{64}", candidate) or not hmac.compare_digest(candidate, token):
        raise BridgeError("authentication_failed")
    if headers.get("content-type") != "application/json":
        raise BridgeError("invalid_request")
    length = headers.get("content-length", "")
    if not re.fullmatch(r"[1-9][0-9]{0,3}", length) or not 2 <= int(length) <= BODY_CAP:
        raise BridgeError("invalid_request")
    if any(key.startswith("access-control-request-") for key in headers):
        raise BridgeError("invalid_request")
    return int(length)


def decode_body(raw, expected_action):
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise BridgeError("invalid_request")
            result[key] = value
        return result

    def numeric(_text):
        # Every request value is a string, never a lossy browser number.
        raise BridgeError("invalid_request")

    if not 2 <= len(raw) <= BODY_CAP:
        raise BridgeError("invalid_request")
    try:
        value = json.loads(raw.decode("utf-8", "strict"), object_pairs_hook=pairs,
                           parse_int=numeric, parse_float=numeric, parse_constant=numeric)
    except (UnicodeError, ValueError, RecursionError):
        raise BridgeError("invalid_request") from None
    if type(value) is not dict or value.get("action") != expected_action:
        raise BridgeError("invalid_request")
    if len(value) > 6 or any(type(item) is not str or not item.isascii() or
            any(ord(char) < 32 or ord(char) == 127 for char in item)
            for item in value.values()):
        raise BridgeError("invalid_request")
    return value


def read_request(peer, host, allowed_origin, token):
    deadline = time.monotonic() + HTTP_SECONDS
    data = bytearray()
    while b"\r\n\r\n" not in data:
        if time.monotonic() >= deadline:
            raise BridgeError("invalid_request")
        try:
            chunk = peer.recv(min(4096, HEADER_CAP + 1 - len(data)))
        except socket.timeout:
            continue
        if not chunk:
            raise BridgeError("invalid_request")
        data.extend(chunk)
        if len(data) > HEADER_CAP and b"\r\n\r\n" not in data:
            raise BridgeError("invalid_request")
    end = data.index(b"\r\n\r\n") + 4
    method, path, headers = parse_headers(bytes(data[:end]))
    length = validate_headers(method, headers, host, allowed_origin, token)
    body = bytearray(data[end:])
    if len(body) > length:
        raise BridgeError("invalid_request")
    while len(body) < length:
        if time.monotonic() >= deadline:
            raise BridgeError("invalid_request")
        try:
            chunk = peer.recv(min(4096, length + 1 - len(body)))
        except socket.timeout:
            continue
        if not chunk:
            raise BridgeError("invalid_request")
        body.extend(chunk)
        if len(body) > length:
            raise BridgeError("invalid_request")
    if method == "OPTIONS":
        return None
    return decode_body(bytes(body), PATHS[path])


def response_bytes(value, allowed_origin, preflight=False):
    if preflight:
        status, body = "204 No Content", b""
    else:
        body = json.dumps(value, ensure_ascii=True, allow_nan=False,
                          separators=(",", ":")).encode("ascii")
        status = "200 OK" if value["status"] in ("ok", "disconnected") else "400 Bad Request"
        if value.get("code") == "authentication_failed":
            status = "403 Forbidden"
        elif value.get("code") in ("session_exists", "stale_session", "stale_sequence", "stale_revision"):
            status = "409 Conflict"
    if len(body) > RESPONSE_CAP:
        raise BridgeError("resource_limit")
    # The only CORS recipient is a launcher-selected loopback origin, never reflected input.
    headers = ["HTTP/1.1 " + status, "Content-Type: application/json",
               "Content-Length: " + str(len(body)), "Connection: close",
               "Cache-Control: no-store", "Pragma: no-cache", "X-Content-Type-Options: nosniff",
               "Access-Control-Allow-Origin: " + allowed_origin, "Vary: Origin"]
    if preflight:
        headers.extend(("Access-Control-Allow-Methods: POST",
                        "Access-Control-Allow-Headers: Content-Type, X-Fe2o3-Bridge-Token",
                        "Access-Control-Max-Age: 0"))
    return ("\r\n".join(headers) + "\r\n\r\n").encode("ascii") + body


def write_response(peer, wire):
    deadline = time.monotonic() + HTTP_SECONDS
    offset = 0
    while offset < len(wire):
        if time.monotonic() >= deadline:
            raise OSError("response transport deadline")
        try:
            count = peer.send(wire[offset:offset + 65536])
        except socket.timeout:
            continue
        if count <= 0:
            raise OSError("response transport closed")
        offset += count
