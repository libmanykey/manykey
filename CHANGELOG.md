# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/). Before 1.0, minor versions may contain breaking changes.

## [Unreleased]

### Added

- **Godot extension (experimental):** a `ManyKeyboard` node that reports which keyboard each key came from, with signals and queries, in `bindings/godot`. Builds from source only; not yet released as an addon.

## [0.1.0] - 2026-10-10

First public release: an early prototype. Only the Windows backend has been run on real keyboards; see the README's status section for exactly what has and hasn't been tested.

### Added

- **Windows backend (Raw Input):** reports each physical keyboard separately, including while your window isn't focused. Announces keyboards at startup and when they are plugged in, reports unplugging, and treats a composite USB keyboard with several interfaces as one keyboard. Names come from the HID product string, falling back to the Device Manager name.
- **Linux backend (evdev):** reads `/dev/input` devices. Tested in CI against virtual keyboards created with uinput; not yet run on physical hardware. Fails with an explanatory error when no keyboard can be opened because of permissions.
- **Physical-key model:** keys are USB HID usages, identical across backends and layouts, with the raw native scancode kept on every event.
- **Events:** `KeyboardAdded`, `KeyboardRemoved`, `Key` (with press, release and repeat states) and `Overflow`. Each key event says whether the OS reported it or manykey generated it (`EventOrigin`).
- **Bounded event queue** with a defined overflow policy, and an OS wait handle for integrating with an event loop.
- **Options:** choice of backend, capture scope (global, or only while your own window has focus on Windows), reclaiming raw-input registration, including software-injected input, switches for the synthesized releases on unplug and on focus loss, and queue capacity.
- **C API** with a generated header (`include/manykey.h`).
- **`manykey` command line tool:** `list` and `watch`.
- **`Listener::fake`:** a scriptable backend for tests that need no hardware.
- **`manykey-view`:** a viewer app that tiles one on-screen keyboard per physical keyboard, in the repository but not published to crates.io.

### Known limitations

- Linux has not been run on physical keyboards, and macOS is not supported.
- Two identical keyboards, and getting the same keyboard id back after replugging, have not been tested.
- Brightness, Fn and some other special keys are not reported on Windows; they may be sent as a kind of HID report the Windows backend doesn't read yet.
- Two `Listener`s in one Windows process compete for raw-input registration.
- The C API and the Rust API may change before 1.0.
