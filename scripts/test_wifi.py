#!/usr/bin/env python3
"""Simple HTTP probe for the ESP-IDF Wi-Fi control server.

The script checks both endpoints implemented in ``src/wifi.rs``:
- ``/`` expects ``Nightstand online``
- ``/params`` accepts a POST payload (defaults to an example JSON)
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from http.client import HTTPConnection, HTTPResponse
from typing import Optional

DEFAULT_PAYLOAD = {"brightness": 0.5, "mode": "sunrise"}


def request(
    host: str,
    port: int,
    method: str,
    path: str,
    body: Optional[bytes] = None,
    headers: Optional[dict[str, str]] = None,
    timeout: float = 5.0,
) -> tuple[int, str, bytes]:
    conn = HTTPConnection(host, port, timeout=timeout)
    try:
        conn.request(method, path, body=body, headers=headers or {})
        resp: HTTPResponse = conn.getresponse()
        return resp.status, resp.reason, resp.read()
    finally:
        conn.close()


def verify_root(host: str, port: int) -> None:
    status, _, body = request(host, port, "GET", "/")
    decoded = body.decode("utf-8", errors="replace")
    if status != 200 or decoded.strip() != "Nightstand online":
        raise RuntimeError(
            f"GET / failed (status={status}, body={decoded!r}); check Wi-Fi server"
        )
    print("[ok] GET / -> Nightstand online")


def verify_params(host: str, port: int, payload: str) -> None:
    headers = {"Content-Type": "application/json", "Content-Length": str(len(payload))}
    status, _, body = request(host, port, "POST", "/params", payload.encode("utf-8"), headers)
    decoded = body.decode("utf-8", errors="replace")
    if status != 200 or decoded.strip() != '{"status":"ok"}':
        raise RuntimeError(
            f"POST /params failed (status={status}, body={decoded!r}); check Wi-Fi server"
        )
    print("[ok] POST /params -> {\"status\":\"ok\"}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Probe the Nightstand Wi-Fi HTTP server")
    parser.add_argument("host", help="Hostname or IP of the ESP32 board (e.g. 192.168.1.50)")
    parser.add_argument("--port", type=int, default=80, help="HTTP port, defaults to 80")
    parser.add_argument(
        "--payload",
        help="JSON payload to send to /params; defaults to a demo payload",
    )
    parser.add_argument(
        "--retries",
        type=int,
        default=1,
        help="How many times to retry the full check before failing",
    )
    parser.add_argument(
        "--wait",
        type=float,
        default=2.0,
        help="Seconds to wait between retries",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    payload = args.payload or json.dumps(DEFAULT_PAYLOAD)

    for attempt in range(1, args.retries + 1):
        try:
            verify_root(args.host, args.port)
            verify_params(args.host, args.port, payload)
            print("Wi-Fi control server looks healthy ✨")
            return 0
        except Exception as exc:  # noqa: BLE001 - we want to show the exact error
            if attempt == args.retries:
                print(f"[error] {exc}", file=sys.stderr)
                return 1
            print(f"[warn] {exc}; retrying in {args.wait}s", file=sys.stderr)
            time.sleep(args.wait)

    return 1


if __name__ == "__main__":
    raise SystemExit(main())
