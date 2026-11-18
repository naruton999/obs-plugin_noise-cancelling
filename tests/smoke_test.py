#!/usr/bin/env python3
"""Smoke test for the df_ffi shared library.

Usage:
    python3 tests/smoke_test.py <path-to-df_ffi-library>
"""

import ctypes
import math
import sys


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: smoke_test.py <path-to-df_ffi-library>", file=sys.stderr)
        return 2

    lib = ctypes.CDLL(sys.argv[1])

    lib.dfx_create.argtypes = [ctypes.c_float, ctypes.c_float]
    lib.dfx_create.restype = ctypes.c_void_p
    lib.dfx_hop_size.argtypes = [ctypes.c_void_p]
    lib.dfx_hop_size.restype = ctypes.c_size_t
    lib.dfx_sample_rate.argtypes = [ctypes.c_void_p]
    lib.dfx_sample_rate.restype = ctypes.c_size_t
    lib.dfx_process.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(ctypes.c_float),
        ctypes.POINTER(ctypes.c_float),
    ]
    lib.dfx_process.restype = ctypes.c_int
    lib.dfx_set_atten_lim.argtypes = [ctypes.c_void_p, ctypes.c_float]
    lib.dfx_set_post_filter_beta.argtypes = [ctypes.c_void_p, ctypes.c_float]
    lib.dfx_destroy.argtypes = [ctypes.c_void_p]

    handle = lib.dfx_create(100.0, 0.0)
    if not handle:
        print("dfx_create failed", file=sys.stderr)
        return 1

    try:
        hop = lib.dfx_hop_size(handle)
        sample_rate = lib.dfx_sample_rate(handle)
        if hop == 0 or sample_rate != 48000:
            print(f"unexpected model parameters: hop={hop} sr={sample_rate}", file=sys.stderr)
            return 1

        input_buffer = (ctypes.c_float * hop)()
        output_buffer = (ctypes.c_float * hop)()

        for i in range(hop):
            input_buffer[i] = 0.1 * math.sin(2.0 * math.pi * 440.0 * i / sample_rate)

        for _ in range(20):
            result = lib.dfx_process(handle, input_buffer, output_buffer)
            if result != 0:
                print(f"dfx_process failed: {result}", file=sys.stderr)
                return 1
            if not all(math.isfinite(output_buffer[i]) for i in range(hop)):
                print("non-finite sample in output", file=sys.stderr)
                return 1

        lib.dfx_set_atten_lim(handle, 50.0)
        lib.dfx_set_post_filter_beta(handle, 0.02)

        if lib.dfx_process(handle, input_buffer, output_buffer) != 0:
            print("dfx_process failed after parameter update", file=sys.stderr)
            return 1

        print("df_ffi smoke test passed")
        return 0
    finally:
        lib.dfx_destroy(handle)


if __name__ == "__main__":
    raise SystemExit(main())
