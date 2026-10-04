"""Launcher-only V20 selection. No browser-controlled paths or legacy fallback."""
from bridge_session import BridgeSession
from bridge_physical_v20_protocol import PhysicalQuerySessionV20
from bridge_physical_v20_values import SCHEMA
from fe2o3_debug_console import regular_path

KIND = "diagnostic-kir-v20"
CANONICAL_BYTES = 128 * 1024
REQUEST_BYTES = 16 * 1024


def launch_arguments_v20(args):
    if args.kind != KIND or args.wave_width != 64 or getattr(args, "runtime_observations", None) is not None:
        raise ValueError("closed V20 physical CPU profile required")
    binary = regular_path(args.binary, 512 * 1024 * 1024, executable=True)
    canonical = regular_path(args.input, CANONICAL_BYTES)
    request = regular_path(args.request, REQUEST_BYTES)
    return [binary, "sim", "--diagnostic-kir-v20", canonical, "--request", request,
            "--wave-width", "64", "--protocol", "jsonl"]


class PhysicalBridgeSessionV20(BridgeSession):
    request_schema = SCHEMA
    response_schema = SCHEMA

    def __init__(self, *args, runtime_observations=None, **kwargs):
        if runtime_observations is not None:
            raise ValueError("V20 runtime-observation extension not admitted")
        super().__init__(*args, runtime_observations=None, **kwargs)

    def new_protocol(self):
        return PhysicalQuerySessionV20()
