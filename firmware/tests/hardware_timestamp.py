#!/usr/bin/env python3
"""Verify that a gs_usb CAN device supplies receive hardware timestamps.

The two CAN channels must be connected to the same CAN bus.  Configure them
first, for example:

    sudo ip link set can0 up type can bitrate 500000
    sudo ip link set can1 up type can bitrate 500000
    sudo ./hardware_timestamp.py

This test relies only on Python's standard library and Linux SocketCAN.
"""

import argparse
import errno
import socket
import struct
import sys
import time
from typing import NoReturn

# Linux UAPI values.  Python does not expose these on every distribution.
SOL_SOCKET = socket.SOL_SOCKET
SO_TIMESTAMPING = 37
SCM_TIMESTAMPING = 37
SOF_TIMESTAMPING_RX_HARDWARE = 1 << 2
SOF_TIMESTAMPING_RAW_HARDWARE = 1 << 6

CAN_EFF_FLAG = 0x80000000
CAN_SFF_MASK = 0x7FF
CAN_FRAME = struct.Struct("=IB3x8s")


def timestamping_flags() -> int:
    return SOF_TIMESTAMPING_RX_HARDWARE | SOF_TIMESTAMPING_RAW_HARDWARE


def timespec_size() -> int:
    # struct timespec uses time_t and long.  Linux hosts are normally 64-bit,
    # but accepting 32-bit long makes the test useful on 32-bit hosts too.
    return struct.calcsize("@ll")


def decode_hardware_timestamp(
    control_messages: list[tuple[int, int, bytes]],
) -> int | None:
    """Return the raw hardware timestamp from SCM_TIMESTAMPING, in ns."""
    for level, ctype, data in control_messages:
        if level != SOL_SOCKET or ctype != SCM_TIMESTAMPING:
            continue

        item_size = timespec_size()
        if len(data) < 3 * item_size:
            continue
        fmt = "@ll" if item_size == 8 else "@qq"
        # SCM_TIMESTAMPING contains software, system-hardware, and
        # raw-hardware timespecs, in that order.  We explicitly require the
        # third slot; a software timestamp is not sufficient for this test.
        sec, nsec = struct.unpack_from(fmt, data, 2 * item_size)
        if sec == 0 and nsec == 0:
            return None
        return sec * 1_000_000_000 + nsec
    return None


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--tx", default="can1", help="CAN interface used to transmit (default: can1)"
    )
    parser.add_argument(
        "--rx", default="can0", help="CAN interface used to receive (default: can0)"
    )
    parser.add_argument(
        "--count", type=int, default=20, help="number of frames to verify (default: 20)"
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=3.0,
        help="receive timeout in seconds (default: 3)",
    )
    return parser.parse_args()


def fail(message: str) -> NoReturn:
    print(f"FAIL: {message}", file=sys.stderr)
    raise SystemExit(1)


def main() -> int:
    args = parse_args()
    if args.count < 2:
        fail("--count must be at least 2")
    if sys.platform != "linux":
        fail("this test requires Linux SocketCAN")

    tx = rx = None
    try:
        tx = socket.socket(socket.PF_CAN, socket.SOCK_RAW, socket.CAN_RAW)
        rx = socket.socket(socket.PF_CAN, socket.SOCK_RAW, socket.CAN_RAW)
        rx.settimeout(args.timeout)
        try:
            rx.setsockopt(SOL_SOCKET, SO_TIMESTAMPING, timestamping_flags())
        except OSError as exc:
            if exc.errno in (errno.ENOPROTOOPT, errno.EOPNOTSUPP, errno.EINVAL):
                fail(
                    "Linux or the gs_usb driver does not support hardware timestamping"
                )
            raise

        try:
            tx.bind((args.tx,))
            rx.bind((args.rx,))
        except OSError as exc:
            fail(f"could not bind {args.tx!r}/{args.rx!r}: {exc}")

        can_id = 0x600
        sent = {}
        for sequence in range(args.count):
            # Include a sequence marker in every frame so stale bus traffic
            # cannot make the test pass.
            payload = struct.pack("<I", 0x54535000 | sequence) + b"TSOK"
            tx.send(CAN_FRAME.pack(can_id, len(payload), payload))
            sent[sequence] = payload
            time.sleep(0.005)

        received = []
        deadline = time.monotonic() + args.timeout
        while len(received) < args.count and time.monotonic() < deadline:
            try:
                frame, ancillary, _, _ = rx.recvmsg(CAN_FRAME.size, 256)
            except socket.timeout:
                break
            if len(frame) < CAN_FRAME.size:
                continue
            frame_id, length, payload = CAN_FRAME.unpack(frame[: CAN_FRAME.size])
            if frame_id & CAN_EFF_FLAG or (frame_id & CAN_SFF_MASK) != can_id:
                continue
            sequence = struct.unpack_from("<I", payload, 0)[0] & 0xFF
            if sequence not in sent or payload[:length] != sent[sequence]:
                continue
            hardware_ns = decode_hardware_timestamp(ancillary)
            if hardware_ns is None:
                fail("received a matching frame without a raw hardware timestamp")
            if any(previous_sequence == sequence for previous_sequence, _ in received):
                continue
            received.append((sequence, hardware_ns))

        if len(received) != args.count:
            fail(f"received {len(received)} of {args.count} expected frames")

        timestamps = [timestamp for _, timestamp in received]
        now_ns = time.time_ns()
        if any(abs(timestamp - now_ns) > 30_000_000_000 for timestamp in timestamps):
            fail("hardware timestamp is not a current Unix timestamp")
        if len(set(timestamps)) < 2:
            fail("hardware timestamps did not advance")
        if any(later < earlier for earlier, later in zip(timestamps, timestamps[1:])):
            fail("hardware timestamps are not monotonic")

        print(
            f"PASS: {len(received)} frames carried raw hardware timestamps "
            f"({(timestamps[-1] - timestamps[0]) / 1e6:.3f} ms span)"
        )
        return 0
    finally:
        if tx is not None:
            tx.close()
        if rx is not None:
            rx.close()


if __name__ == "__main__":
    main()
