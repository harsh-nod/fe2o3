"""Small exact V1 request subset. User coordinates are requests, not ownership."""
import re

U64 = (1 << 64) - 1
HELP = """CPU replay console; logical observations, not hardware or source authority.
help | state | step [1..64] | reverse [1..64] | continue [1..65536]
break add FUNCTION BLOCK OPERATION [before|after]
break list | break remove ID
watch add ALLOCATION GENERATION OFFSET LENGTH [read|write|atomic|any]
watch list | watch remove ID
source FUNCTION BLOCK OPERATION
stack | memory ALLOCATION GENERATION OFFSET LENGTH | quit
Numbers are decimal. BLOCK is protocol roster ordinal, not raw KIR BlockId.
Watch/memory length <=4096 bytes; generation is explicit, never synthesized.
Lists/stack show at most 16 rows; a returned next_cursor means more is omitted.
Unavailable/redacted/truncated facts remain as returned. No over/out, eval,
source editing, private runtime origins, lifetime generations or hardware mode.
"""

class CommandError(ValueError):
    pass

def number(text, minimum=0, maximum=U64):
    if not re.fullmatch(r"0|[1-9][0-9]{0,19}", text):
        raise CommandError("canonical unsigned decimal required")
    value = int(text)
    if not minimum <= value <= maximum:
        raise CommandError("number outside command bound")
    return value

def site(words):
    if len(words) != 3:
        raise CommandError("FUNCTION BLOCK OPERATION required")
    function, block, operation = (number(word) for word in words)
    return {"function_ordinal": function, "block_ordinal": block,
            "point": {"kind": "operation", "operation_ordinal": operation}}

def memory_range(words):
    if len(words) != 4:
        raise CommandError("ALLOCATION GENERATION OFFSET LENGTH required")
    allocation, generation, offset, length = (
        number(words[0], 1), number(words[1]), number(words[2]), number(words[3], 1, 4096))
    if offset + length > U64:
        raise CommandError("byte range overflow")
    return {"allocation": {"ordinal": allocation, "generation": generation},
            "byte_offset": offset, "byte_len": length}

def parse_command(line):
    if not isinstance(line, str) or len(line.encode("utf-8")) > 1024:
        raise CommandError("command exceeds 1024 UTF-8 bytes")
    if any(ord(c) < 32 or ord(c) == 127 for c in line):
        raise CommandError("control characters are not command syntax")
    words = line.split()
    if not words:
        return None
    name, args = words[0], words[1:]
    if name == "help" and not args:
        return "help"
    if name in ("state", "stack", "quit") and not args:
        return {"state": {"operation": "get_state"},
                "stack": {"operation": "inspect_stack", "scope": {"level": "dispatch"},
                          "page": {"limit": 16}},
                "quit": {"operation": "terminate"}}[name]
    if name in ("step", "reverse", "continue") and len(args) <= 1:
        if name == "continue":
            return {"operation": "continue", "max_events": number(args[0], 1, 65536) if args else 1024}
        return {"operation": "step", "direction": "forward" if name == "step" else "reverse",
                "granularity": "operation", "count": number(args[0], 1, 64) if args else 1}
    if name == "source":
        return {"operation": "resolve_source", "site": site(args)}
    if name == "memory":
        return {"operation": "read_memory", **memory_range(args)}
    if name in ("break", "watch") and args:
        plural = "breakpoints" if name == "break" else "watchpoints"
        if args == ["list"]:
            return {"operation": "list_" + plural, "page": {"limit": 16}}
        if len(args) == 2 and args[0] == "remove":
            id_field = "breakpoint_ids" if name == "break" else "watchpoint_ids"
            return {"operation": "remove_" + plural, id_field: [number(args[1], 1)]}
        if args[0] == "add":
            if name == "break" and len(args) in (4, 5):
                phase = args[4] if len(args) == 5 else "before"
                if phase not in ("before", "after"):
                    raise CommandError("break phase must be before or after")
                spec = {"enabled": True, "kind": {"kind": "site", "site": site(args[1:4]),
                        "phase": phase + "_operation"}}
            elif name == "watch" and len(args) in (5, 6):
                access = args[5] if len(args) == 6 else "write"
                if access not in ("read", "write", "atomic", "any"):
                    raise CommandError("unknown watch access")
                spec = {"enabled": True, **memory_range(args[1:5]),
                        "access": access, "timing": "after_commit"}
            else:
                raise CommandError("invalid add arguments")
            return {"operation": "set_" + plural, plural: [spec]}
    raise CommandError("unsupported command; use help")
