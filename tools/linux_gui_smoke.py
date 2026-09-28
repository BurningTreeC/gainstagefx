#!/usr/bin/env python3
"""Exercise the bundled CLAP editor through the real Linux/X11 host ABI.

Requires an X server (a desktop or Xvfb) and libX11. No audio device or Python
packages are needed. The plugin is never installed into the user's DAW paths.
"""
import argparse
import ctypes as C
import os
from pathlib import Path
import threading
import time

P = C.c_void_p
U = C.c_uint32
B = C.c_bool
S = C.c_char_p
F = C.CFUNCTYPE


class Version(C.Structure):
    _fields_ = [(n, U) for n in ("major", "minor", "revision")]


class Entry(C.Structure):
    _fields_ = [("version", Version)] + [(n, P) for n in ("init", "deinit", "get_factory")]


class Factory(C.Structure):
    _fields_ = [(n, P) for n in ("count", "descriptor", "create")]


class Plugin(C.Structure):
    _fields_ = [(n, P) for n in ("desc", "data", "init", "destroy", "activate", "deactivate", "start", "stop", "reset", "process", "extension", "main")]


class Host(C.Structure):
    _fields_ = [("version", Version), ("data", P)] + [(n, S) for n in ("name", "vendor", "url", "host_version")] + [(n, P) for n in ("extension", "restart", "process", "callback")]


class Gui(C.Structure):
    _fields_ = [(n, P) for n in ("supported", "preferred", "create", "destroy", "scale", "size", "can_resize", "resize_hints", "adjust_size", "set_size", "parent", "transient", "title", "show", "hide")]


class HostGui(C.Structure):
    _fields_ = [(n, P) for n in ("hints", "resize", "show", "hide", "closed")]


class Window(C.Structure):
    _fields_ = [("api", S), ("x11", C.c_ulong)]


class WindowAttributes(C.Structure):
    _fields_ = [("background_pixmap", C.c_ulong), ("background_pixel", C.c_ulong),
                ("border_pixmap", C.c_ulong), ("border_pixel", C.c_ulong),
                ("bit_gravity", C.c_int), ("win_gravity", C.c_int), ("backing_store", C.c_int),
                ("backing_planes", C.c_ulong), ("backing_pixel", C.c_ulong),
                ("save_under", C.c_int), ("event_mask", C.c_long), ("do_not_propagate_mask", C.c_long),
                ("override_redirect", C.c_int), ("colormap", C.c_ulong), ("cursor", C.c_ulong)]


class PointerEvent(C.Structure):
    # XButtonEvent and XMotionEvent share this layout (button/is_hint at detail).
    _fields_ = [("type", C.c_int), ("serial", C.c_ulong), ("send_event", C.c_int),
                ("display", P), ("window", C.c_ulong), ("root", C.c_ulong),
                ("subwindow", C.c_ulong), ("time", C.c_ulong),
                ("x", C.c_int), ("y", C.c_int), ("x_root", C.c_int), ("y_root", C.c_int),
                ("state", U), ("detail", U), ("same_screen", C.c_int)]


def fn(address, result, *args):
    return F(result, *args)(address)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("plugin", type=Path)
    parser.add_argument("--snapshots", type=Path)
    args = parser.parse_args()
    path = args.plugin.resolve()
    x = C.CDLL("libX11.so.6")

    def xfn(name, result, *types):
        f = getattr(x, name)
        f.restype, f.argtypes = result, types
        return f

    xfn("XInitThreads", C.c_int)()
    display = xfn("XOpenDisplay", P, S)(None)
    assert display, "Cannot open DISPLAY; use a desktop session or xvfb-run"
    root = xfn("XDefaultRootWindow", C.c_ulong, P)(display)
    create = xfn("XCreateSimpleWindow", C.c_ulong, P, C.c_ulong, C.c_int, C.c_int, U, U, U, C.c_ulong, C.c_ulong)
    parent = create(display, root, 0, 0, 780, 968, 0, 0, 0)
    # This is a simulated host pane. Let the test host control its size, rather
    # than allowing the desktop's tiling/window policy to override resize requests.
    attributes = WindowAttributes(override_redirect=1)
    xfn("XChangeWindowAttributes", C.c_int, P, C.c_ulong, C.c_ulong, C.POINTER(WindowAttributes))(display, parent, 1 << 9, C.byref(attributes))
    xfn("XStoreName", C.c_int, P, C.c_ulong, S)(display, parent, b"GainStageFx CLAP editor smoke test")
    map_window = xfn("XMapWindow", C.c_int, P, C.c_ulong)
    resize_window = xfn("XResizeWindow", C.c_int, P, C.c_ulong, U, U)
    sync = xfn("XSync", C.c_int, P, C.c_int)
    map_window(display, parent)
    sync(display, 0)
    pending = threading.Event()
    unexpected_close = threading.Event()
    callbacks = []

    def cb(result, types, body):
        callback = F(result, *types)(body)
        callbacks.append(callback)
        return C.cast(callback, P).value

    def resize(_host, width, height):
        # REAPER accepts large editors but caps the containing pane at the
        # available screen height. The child must retain its requested scale.
        resize_window(display, parent, width, min(height, 2004))
        sync(display, 0)
        return True

    host_gui = HostGui(
        cb(None, [P], lambda _: None),
        cb(B, [P, U, U], resize),
        cb(B, [P], lambda _: True), cb(B, [P], lambda _: True),
        cb(None, [P, B], lambda _h, _d: unexpected_close.set()),
    )
    host = Host(Version(1, 2, 0), None, b"GainStageFx smoke test", b"GainStageFx", b"", b"1",
                cb(P, [P, S], lambda _, name: C.addressof(host_gui) if name == b"clap.gui" else None),
                cb(None, [P], lambda _: None), cb(None, [P], lambda _: None),
                cb(None, [P], lambda _: pending.set()))
    library = C.CDLL(os.fspath(path))
    entry = Entry.in_dll(library, "clap_entry")
    assert fn(entry.init, B, S)(os.fsencode(path)), "entry.init failed"
    factory_ptr = fn(entry.get_factory, P, S)(b"clap.plugin-factory")
    factory = C.cast(factory_ptr, C.POINTER(Factory)).contents
    plugin_ptr = fn(factory.create, P, P, P, S)(factory_ptr, C.byref(host), b"com.burningtreec.gainstagefx")
    assert plugin_ptr, "factory.create failed"
    plugin = C.cast(plugin_ptr, C.POINTER(Plugin)).contents
    assert fn(plugin.init, B, P)(plugin_ptr), "plugin.init failed"
    gui_ptr = fn(plugin.extension, P, P, S)(plugin_ptr, b"clap.gui")
    assert gui_ptr, "Plugin does not expose clap.gui"
    gui = C.cast(gui_ptr, C.POINTER(Gui)).contents

    def pump(seconds=0.7):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if pending.is_set():
                pending.clear()
                fn(plugin.main, None, P)(plugin_ptr)
            assert not unexpected_close.is_set(), "Editor closed unexpectedly (check native error log)"
            time.sleep(0.01)

    def child_window():
        query = xfn("XQueryTree", C.c_int, P, C.c_ulong, C.POINTER(C.c_ulong), C.POINTER(C.c_ulong), C.POINTER(C.POINTER(C.c_ulong)), C.POINTER(U))
        root_out, parent_out, count = C.c_ulong(), C.c_ulong(), U()
        children = C.POINTER(C.c_ulong)()
        assert query(display, parent, C.byref(root_out), C.byref(parent_out), C.byref(children), C.byref(count))
        assert count.value == 1, f"Expected one embedded child, got {count.value}"
        child = children[0]
        xfn("XFree", C.c_int, P)(children)
        return child

    def dimensions():
        width, height = U(), U()
        assert fn(gui.size, B, P, C.POINTER(U), C.POINTER(U))(plugin_ptr, C.byref(width), C.byref(height))
        return width.value, height.value

    def capture(name):
        child = child_window()
        actual = dimensions()
        # XGetImage requires the region to fit the screen. Large zoom levels may
        # extend below a laptop display, so inspect the top of the actual child.
        width, height = map(U, (min(actual[0], 512), min(actual[1], 512)))
        image = xfn("XGetImage", P, P, C.c_ulong, C.c_int, C.c_int, U, U, C.c_ulong, C.c_int)(display, child, 0, 0, width.value, height.value, C.c_ulong(-1), 2)
        assert image, "Child is not viewable"
        pixel = xfn("XGetPixel", C.c_ulong, P, C.c_int, C.c_int)
        colors = {pixel(image, px, py) & 0xffffff for py in range(0, height.value, 8) for px in range(0, width.value, 8)}
        try:
            if len(colors) <= 32:
                print(
                    f"WARNING: XGetImage saw only {len(colors)} sampled colors; "
                    "headless OpenGL contents may not be readable through XGetImage",
                    flush=True,
                )
            if args.snapshots:
                args.snapshots.mkdir(parents=True, exist_ok=True)
                data = bytearray()
                for py in range(height.value):
                    for px in range(width.value):
                        p = pixel(image, px, py)
                        data.extend(((p >> 16) & 255, (p >> 8) & 255, p & 255))
                (args.snapshots / f"{name}.ppm").write_bytes(f"P6\n{width.value} {height.value}\n255\n".encode() + data)
        finally:
            xfn("XDestroyImage", C.c_int, P)(image)
        print(f"{name}: {actual[0]}x{actual[1]}, {len(colors)} colors", flush=True)

    def click(px, py):
        child = child_window()
        send = xfn("XSendEvent", C.c_int, P, C.c_ulong, C.c_int, C.c_long, P)
        for kind, mask, detail in [(6, 64, 0), (4, 4, 1), (5, 8, 1)]:
            event = PointerEvent(kind, 0, 1, display, child, root, 0, 0,
                                 px, py, px, py, 0, detail, 1)
            assert send(display, child, 0, mask, C.byref(event))
            sync(display, 0)
            pump(0.08)

    def check_zoom():
        current = dimensions()[0] / 780
        for row, scale in [(0, 0.75), (3, 1.25), (4, 1.5), (5, 1.75), (6, 2.0), (2, 1.0)]:
            click(round(408 * current), round(16 * current))
            click(round(408 * current), round((45 + row * 22) * current))
            pump()
            expected = (int(780 * scale * 1.5 + 0.5), int(968 * scale * 1.5 + 0.5))
            assert dimensions() == expected, f"Zoom {scale}: {dimensions()} != {expected}"
            capture(f"zoom-{scale}")
            current = scale * 1.5

    reparent = xfn("XReparentWindow", C.c_int, P, C.c_ulong, C.c_ulong, C.c_int, C.c_int)
    hidden = create(display, root, 0, 0, 780, 968, 0, 0, 0)
    for cycle in range(2):
        if cycle == 1:
            # REAPER's FX chain reparents containers after the CLAP editor was
            # created/shown. Visibility then changes through ancestor discovery,
            # without the editor receiving its own new MapNotify transition.
            reparent(display, parent, hidden, 0, 0)
            sync(display, 0)
        print(f"Creating editor {cycle + 1}", flush=True)
        assert fn(gui.create, B, P, S, B)(plugin_ptr, b"x11", False), "gui.create failed"
        # Match REAPER's HiDPI suggestion, including repeated notifications.
        # The fixed DPI policy must keep 100% at 1.5 on each editor creation.
        for _ in range(2):
            assert fn(gui.scale, B, P, C.c_double)(plugin_ptr, 2.0)
        if cycle == 0:
            assert dimensions() == (1170, 1452), f"Default zoom: {dimensions()}"
        resize_window(display, parent, *dimensions())
        sync(display, 0)
        assert fn(gui.parent, B, P, C.POINTER(Window))(plugin_ptr, C.byref(Window(b"x11", parent))), "gui.set_parent failed"
        assert fn(gui.show, B, P)(plugin_ptr), "gui.show failed"
        if cycle == 1:
            pump(0.1)
            reparent(display, parent, root, 0, 0)
            sync(display, 0)
        pump()
        capture(f"open-{cycle}")
        if cycle == 0:
            check_zoom()
        assert fn(gui.hide, B, P)(plugin_ptr)
        pump(0.1)
        assert fn(gui.show, B, P)(plugin_ptr)
        pump()
        capture(f"reshown-{cycle}")
        fn(gui.destroy, None, P)(plugin_ptr)
    fn(plugin.destroy, None, P)(plugin_ptr)
    fn(entry.deinit, None)()
    xfn("XDestroyWindow", C.c_int, P, C.c_ulong)(display, parent)
    xfn("XDestroyWindow", C.c_int, P, C.c_ulong)(display, hidden)
    xfn("XCloseDisplay", C.c_int, P)(display)
    print("PASS: Linux CLAP editor creates, paints, zooms, hides, shows, and recreates")


if __name__ == "__main__":
    main()
