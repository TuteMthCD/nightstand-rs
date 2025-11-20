#!/usr/bin/env python3
"""Push a solid red frame to the Nightstand /params endpoint."""

from __future__ import annotations

import argparse
import json
from http.client import HTTPConnection, HTTPResponse
from typing import List


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("host", help="Hostname or IP of the ESP32 board")
    parser.add_argument("--port", type=int, default=80, help="HTTP port (default: 80)")
    parser.add_argument(
        "--count",
        type=int,
        default=12,
        help="How many pixels the strip has (default: 12)",
    )
    return parser.parse_args()


def make_payload(count: int) -> str:
    pixels: List[dict[str, int]] = [{"r": 255, "g": 0, "b": 0} for _ in range(count)]
    return json.dumps(pixels)


def post_pixels(host: str, port: int, payload: str) -> tuple[int, str, bytes]:
    conn = HTTPConnection(host, port, timeout=5)
    try:
        headers = {
            "Content-Type": "application/json",
            "Content-Length": str(len(payload)),
        }
        conn.request("POST", "/params", body=payload, headers=headers)
        resp: HTTPResponse = conn.getresponse()
        return resp.status, resp.reason, resp.read()
    finally:
        conn.close()


def main() -> int:
    args = parse_args()
    payload = make_payload(args.count)
    status, reason, body = post_pixels(args.host, args.port, payload)
    print(f"POST /params -> {status} {reason}: {body.decode('utf-8', errors='replace')}")
    return 0 if status == 200 else 1


if __name__ == "__main__":
    raise SystemExit(main())
