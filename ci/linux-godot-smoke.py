#!/usr/bin/env python3
"""Smoke test for the Godot extension on Linux, using virtual keyboards.

Runs bindings/godot/demo/test_headless.gd in a headless Godot with the
extension built and copied into the demo (bindings/godot/build.sh), types on
virtual keyboards made with uinput (needs root), and checks what the
ManyKeyboard node reported: keyboards, keys, held keys with both Shift keys,
a second keyboard plugged in and its removal.

Usage: sudo python3 ci/linux-godot-smoke.py GODOT_BINARY bindings/godot/demo

On a desktop, the virtual keyboards' key presses (A, B and Shift) also reach
whichever window has focus.
"""
import importlib.util
import os
import re
import subprocess
import sys
import threading
import time

# Reuse the uinput keyboard from the evdev smoke test.
_spec = importlib.util.spec_from_file_location(
    "uinput_smoke", os.path.join(os.path.dirname(os.path.abspath(__file__)), "linux-uinput-smoke.py"))
_uinput = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_uinput)
VirtualKeyboard = _uinput.VirtualKeyboard

KEY_A, KEY_B, KEY_LEFTSHIFT, KEY_RIGHTSHIFT = 30, 48, 42, 54
GODOT_KEY_A, GODOT_KEY_B, GODOT_KEY_SHIFT = 65, 66, 4194325
HID_LEFTSHIFT, HID_RIGHTSHIFT = 225, 229


def wait_for(lines, pattern, timeout):
    """Wait until some output line matches `pattern`; returns the match or None."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        for line in list(lines):
            m = re.search(pattern, line)
            if m:
                return m
        time.sleep(0.1)
    return None


def evaluate(out):
    """Check test_headless.gd's output; returns a list of failures."""
    failures = []

    added = re.findall(r"^ADDED id=(\d+) name=(.*) vid=(-?\d+) pid=(-?\d+) known=(\d+)$", out, re.M)
    # A handler calling get_keyboards() must already see the keyboard it's told about.
    for n, (_, name, _, _, known) in enumerate(added, 1):
        if int(known) != n:
            failures.append(f"keyboard_added for {name!r}: get_keyboards() had {known} keyboards, expected {n}")

    ids = {name: kid for kid, name, vid, pid, _ in added if (vid, pid) in [("4660", "22136"), ("4660", "39321")]}
    one, two = ids.get("manykey-ci-kbd"), ids.get("manykey-ci-kbd-two")
    if not one:
        failures.append("virtual keyboard 1 was not announced with its name and USB ids")
    if not two:
        failures.append("hot-plugged keyboard 2 was not announced")
    if one and two and one == two:
        failures.append("the two keyboards got the same id")

    def key_line(kid, key, pressed, hid=None):
        hid_part = rf"hid={hid}" if hid else r"hid=\d+"
        return re.search(rf"^KEY id={kid} key={key} pressed={pressed} echo=false {hid_part} .* held=(\[.*\])$", out, re.M)

    if one:
        if not key_line(one, GODOT_KEY_A, "true", 4):
            failures.append("A press not reported for keyboard 1 (or wrong Godot key / HID usage)")
        if not key_line(one, GODOT_KEY_A, "false", 4):
            failures.append("A release not reported for keyboard 1")
        # Right Shift released while left Shift is still held: Shift is still held.
        m = key_line(one, GODOT_KEY_SHIFT, "false", HID_RIGHTSHIFT)
        if not m:
            failures.append("right Shift release not reported")
        elif m.group(1) != f"[{GODOT_KEY_SHIFT}]":
            failures.append(f"after releasing right Shift with left Shift held, held={m.group(1)}, expected [{GODOT_KEY_SHIFT}]")
        m = key_line(one, GODOT_KEY_SHIFT, "false", HID_LEFTSHIFT)
        if not m:
            failures.append("left Shift release not reported")
        elif m.group(1) != "[]":
            failures.append(f"after releasing both Shift keys, held={m.group(1)}, expected []")
    if two:
        if not key_line(two, GODOT_KEY_B, "true"):
            failures.append("B press not reported for keyboard 2")
        if not re.search(rf"^REMOVED id={two}$", out, re.M):
            failures.append("keyboard 2 removal was not reported")
    return failures


def main(godot, demo):
    # Godot learns about the extension's classes when it imports the project.
    subprocess.run([godot, "--headless", "--path", demo, "--import"],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=300)

    kb = VirtualKeyboard("manykey-ci-kbd", 0x1234, 0x5678)
    time.sleep(1.0)  # let udev/the kernel publish the device

    proc = subprocess.Popen([godot, "--headless", "--path", demo, "--script", "res://test_headless.gd",
                             "--", "--global"], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    lines = []
    reader = threading.Thread(target=lambda: [lines.append(l.rstrip("\n")) for l in proc.stdout], daemon=True)
    reader.start()

    failures = []
    running = wait_for(lines, r"^RUNNING=(\w+)", 60)
    if not running or running.group(1) != "true":
        failures.append("the ManyKeyboard node did not start")
    else:
        time.sleep(1.0)  # startup enumeration
        kb.key(KEY_A, True)
        kb.key(KEY_A, False)
        kb.key(KEY_LEFTSHIFT, True)
        kb.key(KEY_RIGHTSHIFT, True)
        kb.key(KEY_RIGHTSHIFT, False)       # left Shift is still held here
        kb.key(KEY_LEFTSHIFT, False)
        kb2 = VirtualKeyboard("manykey-ci-kbd-two", 0x1234, 0x9999)   # hot-plug a second one
        time.sleep(1.5)
        kb2.key(KEY_B, True)
        kb2.key(KEY_B, False)
        time.sleep(0.5)
        kb2.destroy()
        time.sleep(1.5)
    if not wait_for(lines, r"^DONE$", 60):
        failures.append("test_headless.gd did not finish")
    proc.kill()
    reader.join(timeout=3)
    kb.destroy()

    out = "\n".join(lines)
    print("== Godot output\n" + out)
    failures += evaluate(out)
    if failures:
        print("\nFAILED:\n  - " + "\n  - ".join(failures))
        return 1
    print("\nOK: the extension reported keyboards, keys, held Shift keys, hot-plug and removal")
    return 0


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    sys.exit(main(sys.argv[1], sys.argv[2]))
