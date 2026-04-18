#!/usr/bin/env python3
import json
import sys

PROTOCOL = 1
PLUGIN = "clankers-stdio-echo"
TOOL = "stdio_echo_fixture"


def read_frame():
    length_bytes = sys.stdin.buffer.read(4)
    if not length_bytes:
        return None
    if len(length_bytes) != 4:
        raise EOFError("short frame length")
    length = int.from_bytes(length_bytes, "big")
    payload = sys.stdin.buffer.read(length)
    if len(payload) != length:
        raise EOFError("short frame payload")
    return json.loads(payload)


def write_frame(frame):
    payload = json.dumps(frame, separators=(",", ":")).encode("utf-8")
    sys.stdout.buffer.write(len(payload).to_bytes(4, "big"))
    sys.stdout.buffer.write(payload)
    sys.stdout.buffer.flush()


def require_host_hello():
    frame = read_frame()
    if frame is None:
        raise SystemExit("missing host hello")
    if frame.get("type") != "hello":
        raise SystemExit(f"expected hello, got {frame}")
    if frame.get("plugin_protocol") != PROTOCOL:
        raise SystemExit(f"unexpected protocol: {frame}")


def send_startup():
    write_frame(
        {
            "type": "hello",
            "plugin_protocol": PROTOCOL,
            "plugin": PLUGIN,
            "version": "0.1.0",
        }
    )
    write_frame({"type": "ready", "plugin_protocol": PROTOCOL})
    write_frame(
        {
            "type": "register_tools",
            "plugin_protocol": PROTOCOL,
            "tools": [
                {
                    "name": TOOL,
                    "description": "Echo input or wait for cancellation.",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "message": {"type": "string"},
                            "mode": {"type": "string", "enum": ["echo", "wait_for_cancel"]},
                        },
                        "required": ["mode"],
                        "additionalProperties": False,
                    },
                }
            ],
        }
    )


def main():
    require_host_hello()
    send_startup()

    waiting_for_cancel = set()
    while True:
        frame = read_frame()
        if frame is None:
            return
        frame_type = frame.get("type")
        if frame_type == "tool_invoke":
            call_id = frame["call_id"]
            args = frame.get("args", {})
            mode = args.get("mode", "echo")
            if mode == "wait_for_cancel":
                waiting_for_cancel.add(call_id)
                write_frame(
                    {
                        "type": "tool_progress",
                        "plugin_protocol": PROTOCOL,
                        "call_id": call_id,
                        "message": "waiting for cancel",
                    }
                )
                continue
            if mode != "echo":
                write_frame(
                    {
                        "type": "tool_error",
                        "plugin_protocol": PROTOCOL,
                        "call_id": call_id,
                        "message": f"unsupported mode: {mode}",
                    }
                )
                continue
            write_frame(
                {
                    "type": "tool_result",
                    "plugin_protocol": PROTOCOL,
                    "call_id": call_id,
                    "content": f"fixture:{args.get('message', '')}",
                }
            )
        elif frame_type == "tool_cancel":
            call_id = frame["call_id"]
            if call_id in waiting_for_cancel:
                waiting_for_cancel.remove(call_id)
                write_frame(
                    {
                        "type": "tool_cancelled",
                        "plugin_protocol": PROTOCOL,
                        "call_id": call_id,
                    }
                )
        elif frame_type == "shutdown":
            return


if __name__ == "__main__":
    main()
