#!/usr/bin/env python3
"""Stream a 3x4 fire effect to the Nightstand /ws WebSocket endpoint.

Requires the `websocket-client` package (`pip install websocket-client`).
"""

from __future__ import annotations

import argparse
import json
import random
import time
import websocket
from queue import Empty, Full, Queue
from threading import Event, Thread
from typing import List, Optional, Sequence, Tuple


WIDTH = 4
HEIGHT = 3


PALETTE: Tuple[Tuple[int, int, int], ...] = (
    (0, 0, 0),
    (7, 0, 0),
    (15, 0, 0),
    (31, 0, 0),
    (47, 7, 0),
    (71, 15, 0),
    (95, 23, 0),
    (119, 31, 0),
    (143, 47, 0),
    (159, 63, 0),
    (175, 79, 0),
    (191, 95, 0),
    (207, 111, 0),
    (223, 127, 0),
    (239, 143, 0),
    (255, 159, 0),
    (255, 175, 0),
    (255, 191, 0),
    (255, 207, 0),
    (255, 215, 31),
    (255, 223, 63),
    (255, 231, 95),
    (255, 239, 127),
    (255, 247, 159),
)


class FireEffect:
    def __init__(self, width: int, height: int) -> None:
        self.width = width
        self.height = height
        self.heat = [0 for _ in range(width * height)]

    def _idx(self, x: int, y: int) -> int:
        return y * self.width + x

    def step(self) -> List[Tuple[int, int, int]]:
        bottom = self.height - 1
        for x in range(self.width):
            self.heat[self._idx(x, bottom)] = random.randint(160, 255)

        for y in range(bottom - 1, -1, -1):
            for x in range(self.width):
                sources: List[int] = [self.heat[self._idx(x, y + 1)]]
                if x > 0:
                    sources.append(self.heat[self._idx(x - 1, y + 1)])
                if x < self.width - 1:
                    sources.append(self.heat[self._idx(x + 1, y + 1)])
                avg = sum(sources) // len(sources)
                decay = random.randint(10, 35)
                self.heat[self._idx(x, y)] = max(avg - decay, 0)

        return [self._heat_to_rgb(value) for value in self.heat]

    @staticmethod
    def _heat_to_rgb(value: int) -> Tuple[int, int, int]:
        value = max(0, min(255, value))
        idx = int((value / 255) * (len(PALETTE) - 1))
        return PALETTE[idx]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("host", help="Hostname or IP of the ESP32 board")
    parser.add_argument("--port", type=int, default=80, help="WebSocket port (default: 80)")
    parser.add_argument(
        "--interval",
        type=float,
        default=0.05,
        help="Seconds between frames (default: 0.05)",
    )
    parser.add_argument(
        "--path",
        default="/ws",
        help="WebSocket path on the device (default: /ws)",
    )
    return parser.parse_args()


def build_uri(host: str, port: int, path: str) -> str:
    prefix = f"ws://{host}"
    if port not in (80, 443):
        prefix = f"{prefix}:{port}"
    return f"{prefix}{path}"


def connect(uri: str) -> websocket.WebSocket:
    ws = websocket.create_connection(uri, timeout=5)
    ws.settimeout(5)
    try:
        msg = ws.recv()
        if msg:
            print(f"WebSocket server says: {msg}")
    except websocket.WebSocketTimeoutException:
        pass
    return ws


def send_pixels(ws: websocket.WebSocket, pixels: Sequence[Tuple[int, int, int]]) -> None:
    payload = json.dumps([{"r": r, "g": g, "b": b} for r, g, b in pixels])
    ws.send(payload)
    try:
        resp = ws.recv()
        if resp:
            try:
                parsed = json.loads(resp)
            except json.JSONDecodeError:
                print(f"Non-JSON response: {resp}")
            else:
                if parsed.get("error"):
                    print(f"Device reported error: {parsed}")
    except websocket.WebSocketTimeoutException:
        pass


def websocket_sender(
    uri: str,
    queue: "Queue[Optional[List[Tuple[int, int, int]]]]",
    stop_event: Event,
) -> int:
    ws: Optional[websocket.WebSocket] = None
    while not stop_event.is_set():
        if ws is None:
            try:
                ws = connect(uri)
                print(f"Connected to {uri}")
            except websocket.WebSocketException as err:
                print(f"Failed to connect: {err}. Retrying...")
                time.sleep(1)
                continue

        try:
            pixels = queue.get(timeout=0.2)
        except Empty:
            continue

        if pixels is None:
            break

        try:
            send_pixels(ws, pixels)
        except (websocket.WebSocketException, OSError) as err:
            print(f"WebSocket error: {err}. Reconnecting...")
            try:
                queue.put_nowait(pixels)
            except Full:
                pass
            try:
                ws.close()
            except Exception:
                pass
            ws = None

    if ws is not None:
        try:
            send_pixels(ws, [])
        except Exception:
            pass
        try:
            ws.close()
        except Exception:
            pass

    return 0


def main() -> int:
    args = parse_args()
    uri = build_uri(args.host, args.port, args.path)
    fire = FireEffect(WIDTH, HEIGHT)
    queue: "Queue[Optional[List[Tuple[int, int, int]]]]" = Queue(maxsize=16)
    stop_event = Event()

    worker = Thread(target=websocket_sender, args=(uri, queue, stop_event), daemon=True)
    worker.start()

    try:
        while not stop_event.is_set():
            pixels = fire.step()
            try:
                queue.put(pixels, timeout=0.1)
            except Full:
                continue
            time.sleep(args.interval)
    except KeyboardInterrupt:
        pass
    finally:
        stop_event.set()
        queue.put(None)
        worker.join()

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
