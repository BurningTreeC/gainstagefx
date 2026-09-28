#!/usr/bin/env python3
"""Measure factory preset levels through an actual CLAP binary, without a DAW.

Export the parameter manifest with `cargo run --example preset_manifest`.
Run separately for old/new binaries (loading two Rust plugin runtimes into the
same process is unnecessary). Output is JSON; plugins are never installed.
"""
import argparse
import ctypes as C
import json
import math
import os
import sys
from pathlib import Path
from linux_gui_smoke import P, U, B, S, F, Version, Entry, Factory, Plugin, Host, fn

D = C.c_double

class Header(C.Structure):
    _fields_ = [("size", U), ("time", U), ("space", C.c_uint16),
                ("type", C.c_uint16), ("flags", U)]

class Value(C.Structure):
    _fields_ = [("header", Header), ("id", U), ("cookie", P),
                ("note", C.c_int32), ("port", C.c_int16),
                ("channel", C.c_int16), ("key", C.c_int16), ("value", D)]

class InEvents(C.Structure):
    _fields_ = [("ctx", P), ("size", P), ("get", P)]

class OutEvents(C.Structure):
    _fields_ = [("ctx", P), ("push", P)]

class ParamInfo(C.Structure):
    _fields_ = [("id", U), ("flags", U), ("cookie", P),
                ("name", C.c_char * 256), ("module", C.c_char * 1024),
                ("minimum", D), ("maximum", D), ("default", D)]

class Params(C.Structure):
    _fields_ = [(n, P) for n in ("count", "info", "value", "text", "parse", "flush")]

class AudioBuffer(C.Structure):
    _fields_ = [("data32", C.POINTER(C.POINTER(C.c_float))), ("data64", P),
                ("channels", U), ("latency", U), ("constant", C.c_uint64)]

class Process(C.Structure):
    _fields_ = [("time", C.c_int64), ("frames", U), ("transport", P),
                ("input", C.POINTER(AudioBuffer)), ("output", C.POINTER(AudioBuffer)),
                ("inputs", U), ("outputs", U),
                ("in_events", C.POINTER(InEvents)), ("out_events", C.POINTER(OutEvents))]


def param_hash(text):
    value = 0
    for byte in text.encode():
        value = (31 * value + byte) & 0xffffffff
    return value & 0x7fffffff


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("plugin", type=Path)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--only", default="")
    args = parser.parse_args()
    path = args.plugin.resolve()
    manifest = json.loads(args.manifest.read_text())
    callbacks = []
    pending = [False]
    events = []
    def cb(result, types, body):
        callback = F(result, *types)(body)
        callbacks.append(callback)
        return C.cast(callback, P).value
    host = Host(Version(1, 2, 0), None, b"GainStageFx audio measurement", b"BurningTreeC", b"", b"1",
                cb(P, [P, S], lambda *_: None), cb(None, [P], lambda _: None),
                cb(None, [P], lambda _: None), cb(None, [P], lambda _: pending.__setitem__(0, True)))
    incoming = InEvents(None, cb(U, [P], lambda _: len(events)),
                        cb(P, [P, U], lambda _, i: C.addressof(events[i])))
    outgoing = OutEvents(None, cb(B, [P, P], lambda *_: True))
    library = C.CDLL(os.fspath(path))
    entry = Entry.in_dll(library, "clap_entry")
    assert fn(entry.init, B, S)(os.fsencode(path))
    factory_ptr = fn(entry.get_factory, P, S)(b"clap.plugin-factory")
    factory = C.cast(factory_ptr, C.POINTER(Factory)).contents
    rate, block = 96000, 512
    input_arrays = [(C.c_float * block)() for _ in range(2)]
    output_arrays = [(C.c_float * block)() for _ in range(2)]
    in_ptrs = (C.POINTER(C.c_float) * 2)(*input_arrays)
    out_ptrs = (C.POINTER(C.c_float) * 2)(*output_arrays)
    input_buffer = AudioBuffer(in_ptrs, None, 2, 0, 0)
    output_buffer = AudioBuffer(out_ptrs, None, 2, 0, 0)
    signal = [10**(-18/20) * sum(a*math.sin(math.tau*f*k/rate)
              for f, a in [(82.4, .5), (123.5, .3), (246.9, .2)]) for k in range(rate)]
    levels = {}
    for name, values in manifest:
        if args.only not in name:
            continue
        plugin_ptr = fn(factory.create, P, P, P, S)(factory_ptr, C.byref(host), b"com.burningtreec.gainstagefx")
        assert plugin_ptr
        plugin = C.cast(plugin_ptr, C.POINTER(Plugin)).contents
        assert fn(plugin.init, B, P)(plugin_ptr)
        params_ptr = fn(plugin.extension, P, P, S)(plugin_ptr, b"clap.params")
        params = C.cast(params_ptr, C.POINTER(Params)).contents
        infos = {}
        for i in range(fn(params.count, U, P)(plugin_ptr)):
            info = ParamInfo()
            assert fn(params.info, B, P, U, P)(plugin_ptr, i, C.byref(info))
            infos[info.id] = info
        events.clear()
        for key, normalized in values:
            info = infos[param_hash(key)]
            value = info.minimum + normalized*(info.maximum-info.minimum)
            events.append(Value(Header(C.sizeof(Value), 0, 0, 5, 0), info.id, None, -1, -1, -1, -1, value))
        fn(params.flush, None, P, P, P)(plugin_ptr, C.byref(incoming), C.byref(outgoing))
        for event in events:
            actual = D()
            assert fn(params.value, B, P, U, P)(plugin_ptr, event.id, C.byref(actual))
            assert abs(actual.value-event.value) < 1e-4, (name, event.id, actual.value, event.value)
        events.clear()
        assert fn(plugin.activate, B, P, D, U, U)(plugin_ptr, rate, 1, block)
        fn(plugin.reset, None, P)(plugin_ptr)
        assert fn(plugin.start, B, P)(plugin_ptr)
        energy, samples = 0.0, 0
        for offset in range(0, rate, block):
            count = min(block, rate-offset)
            for ch in input_arrays:
                ch[:count] = signal[offset:offset+count]
            process = Process(offset, count, None, C.pointer(input_buffer), C.pointer(output_buffer),
                              1, 1, C.pointer(incoming), C.pointer(outgoing))
            assert fn(plugin.process, C.c_int32, P, P)(plugin_ptr, C.byref(process)) != 0
            for i in range(count):
                y = output_arrays[0][i]
                assert math.isfinite(y), (name, offset+i, y)
                if offset+i >= rate//2:
                    energy += y*y
                    samples += 1
            if pending[0]:
                pending[0] = False
                fn(plugin.main, None, P)(plugin_ptr)
        levels[name] = 10*math.log10(max(energy/samples, 1e-24))+18
        print(f"{name}: {levels[name]:+.3f} dB", file=sys.stderr, flush=True)
        fn(plugin.stop, None, P)(plugin_ptr)
        fn(plugin.deactivate, None, P)(plugin_ptr)
        fn(plugin.destroy, None, P)(plugin_ptr)
    fn(entry.deinit, None)()
    print(json.dumps(levels, indent=2))

if __name__ == "__main__":
    main()
