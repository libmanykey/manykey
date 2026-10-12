# manykey

[![CI](https://github.com/libmanykey/manykey/actions/workflows/ci.yml/badge.svg)](https://github.com/libmanykey/manykey/actions/workflows/ci.yml)

**Keyboard input that knows which keyboard it came from.**

Operating systems merge every keyboard into one stream: press `W` on keyboard A or keyboard B and your program sees the same `W`. manykey gives you `(keyboard, key)` instead, so a local multiplayer game can give each player their own keyboard, a macro tool can give a spare keyboard its own meaning, and a stream overlay can show which keyboard is which.

It is a small library, not a framework. It reports *which physical key on which physical keyboard*, and nothing else: no text input, no layouts, no action mapping, no windowing. Use your game engine or toolkit for all of that.

![`manykey-view` screenshot with three connected keyboards](https://raw.githubusercontent.com/libmanykey/manykey/HEAD/docs/images/viewer-screenshot.png)

*Each tile is a different physical keyboard. Keys light up independently as they're pressed, and each keyboard has its own typing box.*

![Three keyboards connected to one laptop, with the viewer on its screen](https://raw.githubusercontent.com/libmanykey/manykey/HEAD/docs/images/desk-setup.jpg)

*The setup behind the screenshot: the laptop's built-in keyboard, a USB keyboard and a ZSA Moonlander, all connected to one Windows laptop.*

```text
$ manykey watch
+ keyboard 1 ----:---- Standard PS/2 Keyboard
keyboard 1  LeftShift    down   (native 0x002a)
keyboard 1  H            down   (native 0x0023)
keyboard 1  H            up     (native 0x0023)
keyboard 1  LeftShift    up     (native 0x002a)
keyboard 1  E            down   (native 0x0012)
keyboard 1  E            up     (native 0x0012)
keyboard 1  L            down   (native 0x0026)
keyboard 1  L            up     (native 0x0026)
...
```

With two keyboards attached, each keystroke carries its own `keyboard` number; that number is the whole point of the library.

**Where it's useful:** local multiplayer games where each player has their own keyboard; macro and control surfaces (a spare keyboard or keypad that means something different from your main one); installations and kiosks; input diagnostics; anything that cares which physical device produced an input.

> **Early prototype.** On Windows, run on one machine with a built-in keyboard, a USB keyboard and a ZSA Moonlander typing at the same time, and in CI with injected key presses. On Linux, run on one Debian machine with a USB keyboard and the Moonlander, and in CI with virtual keyboards. Hot-plugging has been tested with the Moonlander on both. Two identical keyboards, media keys on Linux, and Linux key repeat on physical keyboards are untested. The API will change before 1.0. There are no Python or other-language packages yet. Per-backend details are under [Backends](#backends).
>
> Throughout this document, **Tested** means it was run on real hardware (see the limits above), **Tested (virtual)** means it was run in CI against virtual keyboards but not physical ones, **Builds** means it compiles but hasn't been run, and **Planned** or **Idea** means it doesn't exist yet.

**AI disclosure:** This library was developed with very significant help from several AI tools. I have reviewed all of the code myself, but I did not step through it in a debugger. I've done Win32 and low-level Linux device programming before and the relevant sections look sensible to me. I tested `manykey list`, `manykey watch` and `manykey-view` on Windows and Linux, found real problems, and had them fixed. Expect it to differ from hand-written code, and note that statements about how operating systems behave were written with AI assistance and are marked untested unless run on hardware. Treat it as an early prototype. How contributions are handled is described [below](#ai-use-in-development).

## Contents

- [Why this exists](#why-this-exists)
- [Quick start](#quick-start): Rust, C, command line
- [Backends](#backends): one per way of reading keyboards, and what each costs you
- [How it works](#how-it-works): keyboards, keys, events
- [Options](#options)
- [What manykey does to your program](#what-manykey-does-to-your-program) (please read)
- [Linux permissions](#linux-permissions)
- [Limitations and FAQ](#limitations-and-faq)
- [The viewer app](#the-viewer-app)
- [Building and testing](#building-and-testing)
- [Alternatives and prior art](#alternatives-and-prior-art)
- [Roadmap](#roadmap)
- [AI use in development](#ai-use-in-development)
- [Contributing](#contributing)

## Why this exists

I originally wanted this years ago so I could turn an entire second keyboard into a macro keyboard for programming. At the time I settled for a LuaMacros plus AutoHotkey workaround, which worked but was a hacky mess. More recently I had ideas for couch co-op games, where several people share one PC with a keyboard each, and those would be much easier to build if a program could simply tell the keyboards apart. manykey is my attempt at a clean answer to both: small, embeddable in other programs, and honest about what it can and can't see.

## Quick start

### Rust

```toml
# Cargo.toml
[dependencies]
manykey = "0.1"
```

```rust,no_run
use manykey::{Event, KeyState, Listener};

fn main() -> std::io::Result<()> {
    let listener = Listener::new()?;
    for event in listener.iter() {
        match event {
            Event::KeyboardAdded(k) => println!("+ {} (id {})", k.name, k.id.0),
            Event::KeyboardRemoved(id) => println!("- keyboard {}", id.0),
            Event::Key(k) if k.state == KeyState::Pressed => {
                println!("keyboard {} pressed {}", k.keyboard.0, k.key);
            }
            _ => {}
        }
    }
    Ok(())
}
```

Waiting forever isn't always what you want. Inside a game loop, drain what's ready:

```rust,no_run
# let listener = manykey::Listener::new().unwrap();
while let Some(event) = listener.try_recv() {
    // handle event
    # let _ = event;
}
```

Or block with a timeout (`recv_timeout`), or hand `listener.wait_handle()` to your own event loop: it's an OS handle (a file descriptor on Linux, an event `HANDLE` on Windows) that is signalled while events are waiting.

`Listener::new()` picks the backend for your operating system automatically. See [Backends](#backends).

### C

`include/manykey.h` is generated from the Rust source. Build the library with `cargo build --release` and link `manykey` (a `.dll`/`.so`/static library, all produced).

```c
#include <stdio.h>
#include "manykey.h"

int main(void) {
    manykey_context *ctx;
    if (manykey_open(&ctx) != 0) {
        fprintf(stderr, "manykey: %s\n", manykey_last_error());
        return 1;
    }
    manykey_event ev = { .size = sizeof ev };   /* always set size */
    while (manykey_wait(ctx, &ev, -1) == 1) {
        if (ev.type == MANYKEY_EVENT_KEY && ev.state == MANYKEY_KEY_PRESSED) {
            char name[32];
            manykey_key_name(ev.key, name, sizeof name);
            printf("keyboard %llu pressed %s\n", (unsigned long long)ev.keyboard_id, name);
        }
    }
    manykey_close(ctx);
    return 0;
}
```

**Check the version.** Compare `manykey_version()` (the library you loaded) with the header's `MANYKEY_VERSION` at startup; before 1.0 they must agree on major and minor. [`examples/watch.c`](https://github.com/libmanykey/manykey/blob/HEAD/examples/watch.c) shows the check.

To change settings, use `manykey_options_default` and `manykey_open_with`; see [Options](#options). A complete example is in [`examples/watch.c`](https://github.com/libmanykey/manykey/blob/HEAD/examples/watch.c). A context is not thread-safe: use it from one thread, or lock around it yourself.

### Command line

```text
manykey --version                   print the version (please include it in bug reports)
manykey list                        show connected keyboards
manykey watch                       print keyboards and key events as they happen
manykey watch --include-injected    ...and software-injected input (Windows)
manykey watch --backend NAME        auto (default), raw-input (Windows) or evdev (Linux)
```

## Backends

manykey is organized around **backends**. A backend is the piece that talks to one operating-system mechanism for reading keyboards. Everything above it (the key model, the events, the options, the Rust and C APIs) is the same whichever backend is underneath. What differs is what each backend can see, what it needs from the system, and what it costs you.

`Listener::new()` picks the best backend for your operating system. You can name one explicitly with the `backend` [option](#options) (`manykey watch --backend evdev` on the command line); asking for a backend that isn't available on the current platform is an error, not a silent fallback. Today each OS has exactly one real backend, so this matters more once there are several on one OS (there are at least three plausible ones on Linux; the sections below say why you'd want to choose).

| Backend | Platform | Status | Needs | Sees unfocused input | `OwnProcessFocused` scope | Device name / USB ids / port |
|---|---|---|---|---|---|---|
| [Raw Input](#windows-raw-input) | Windows | **Tested** (three real keyboards; CI with injected input) | nothing | yes | yes | name, USB ids / no port |
| [evdev](#linux-evdev) | Linux | **Tested** | read access to `/dev/input` | yes | no (returns an error) | name, USB ids, port |
| [XInput2](#linux-other-backends) | Linux, X11 only | Planned | an X11 session | expected yes | not yet decided | expected |
| [libinput](#linux-other-backends) | Linux | Idea | same device access as evdev | yes | no | expected |
| [IOHIDManager](#macos) | macOS | Not started | Input Monitoring permission | | | |
| [Fake](#fake-backend-for-tests) | any | **Tested** | nothing | n/a | n/a | scripted |

### Windows: Raw Input

*Status: tested, with limits.* Run on one machine with a built-in keyboard, a generic USB keyboard and a ZSA Moonlander typing simultaneously (each was reported as its own keyboard, with its product name and USB ids), with software-injected input, and in CI on GitHub's Windows runner, where injected key presses were reported correctly. Unplugging was observed to be detected. Two identical keyboards are untested. The Windows backend uses the Raw Input API. It creates a hidden message-only window on its own thread and registers for keyboard input with `RIDEV_INPUTSINK`, so it receives every keyboard's keys even while your window isn't focused.

- **Needs:** nothing. No driver, no administrator rights, no hooks.
- **Keyboards already connected** are announced at startup. Unplugging is reported through the OS's device-change notification (observed working). A keyboard plugged in *later* is announced when Windows reports its arrival, not on its first keypress (an earlier version waited for the first keypress, which testing showed was a bug). **Tested with the ZSA Moonlander:** it was announced on plug-in, both to the viewer and to `manykey watch`, as a single keyboard. A keyboard that comes back is meant to get its earlier `KeyboardId` again, since interfaces are matched by their physical device (container id) or, for built-in keyboards, by device path; that part has not been checked.
- **One keyboard, several interfaces.** A composite USB keyboard can show up to Windows as more than one keyboard interface (the ZSA Moonlander exposes two, of which only one carries keystrokes). manykey groups the interfaces of one physical device, using Windows' *container id*, and reports a single keyboard (confirmed with the Moonlander, which was previously reported twice): announced when its first interface appears and removed when its last one goes. Built-in keyboards all share a generic container, so they are told apart by device path instead. A receiver that serves several keyboards under one container would be merged into one; that's a limitation of what Windows exposes. The `path` you see is that of the first interface seen.
- **Names and ids.** The name comes from the HID product string, which the backend reads by briefly opening the device handle with zero access rights; if that fails, it falls back to the Device Manager name from the registry. The HID product string is what the USB keyboards we tried use ("USB Keyboard", "Moonlander Mark I"); the registry fallback is what the built-in keyboard uses. USB vendor and product ids come from the device path, so built-in keyboards (which often have none) show `----:----`. The port isn't reported.
- **Key repeat** arrives as repeated `Pressed` events. There is no separate `Repeat`.
- **Registration is per process.** Windows lets a process register for raw keyboard input once, and a later registration replaces an earlier one. If another component in your process (winit, SDL, an engine) also registers for raw keyboard input, one of you silently stops receiving. See `reclaim_registration` under [Options](#options).
- **Focused scope** (`OwnProcessFocused`) is implemented with a foreground-window change notification, and drops events while none of your windows is in front.
- **Software-injected input** (anything sent with `SendInput`, such as macro tools) arrives with no device. It is dropped by default, and with `include_injected` it shows up as one extra pseudo-keyboard.

### Linux: evdev

*Status: tested on one Debian machine (X11) with a USB keyboard and a ZSA Moonlander, and in CI with virtual keyboards.* On that machine, `manykey watch` and the viewer reported both keyboards, overlapping key presses, and unplugging and replugging the Moonlander, which came back with the same `KeyboardId`. A key held while unplugging was released; a key already held while plugging in is not reported until it's released (the kernel only reports changes). Media keys and key repeat haven't been tried on physical keyboards. In GitHub's Linux CI the backend found a keyboard with its kernel name and ids, reported press, repeat and release, announced a second keyboard plugged in while running, and reported both removals. The evdev backend reads the kernel's `/dev/input/event*` devices directly, on one thread per keyboard, and rescans `/proc/bus/input/devices` twice a second to notice keyboards being plugged and unplugged.

- **Needs read access to the input devices**, normally membership of the `input` group. See [Linux permissions](#linux-permissions); this is a real cost.
- **Works under X11, Wayland and the text console**, because it doesn't talk to the display server at all. That also means it has **no idea which window has focus**, so `OwnProcessFocused` returns an error instead of silently reporting everything.
- **What it reports:** the kernel's device name, USB ids, and the physical port. A keyboard replugged into the same port gets the same `KeyboardId` back (tested with the Moonlander); in a different port it gets a new one (by design; not tested).
- **Key repeat** is reported as `Repeat`.
- **Which devices count as keyboards** uses the same test as udev: the device has the first 31 key codes (Esc through S). That leaves out power buttons and laptop hotkey devices. Virtual keyboards created by other software, such as remappers, can show up as keyboards too, and so can a mouse's keyboard interface (some gaming mice have one for macro buttons, and claim every key).
- **A keyboard with several interfaces is one keyboard,** as on Windows. Many USB keyboards expose their media and system keys, and sometimes a second key report, as extra input devices. manykey groups the interfaces of one device (same port and USB ids, plus the serial or Bluetooth address when there is one) under one `KeyboardId`, and also reads the media and system-key interfaces of those keyboards.

### Linux: other backends

These don't exist yet. They're listed because the reason for having several backends is that no single Linux mechanism is best on every axis:

- **XInput2 (X11).** *Planned.* Talking to the X server should need no special file permissions, which is the main appeal. It would only work in real X11 sessions; programs running through XWayland probably can't see input meant for native Wayland windows. We haven't built or verified this.
- **libinput.** *Idea.* A better parser for the same devices evdev reads, with hotplug handled for you. It does not remove the permission requirement, because libinput opens the same device files.
- **Wayland.** As far as we know, Wayland's core protocol merges all keyboards into one, so applications cannot tell which physical keyboard a key came from, and there's no sanctioned global alternative. If that's right, a Wayland user needs evdev (or libinput) and its permissions. We haven't verified this across compositors, so treat it as our current understanding, not a guarantee.

### macOS

*Not started.* The likely route is IOHIDManager, which also needs the user to grant Input Monitoring permission.

### Fake backend (for tests)

*Tested.* `Listener::fake(capacity)` returns a listener plus a handle for adding keyboards and injecting key events, with no hardware. manykey's own tests use it, and you can use it to test code that consumes manykey.

### WSL2

WSL2 can't see the host machine's keyboards, so none of the real backends work inside it. Run the Windows build instead.

## How it works

### Keyboards

Each keyboard gets a `KeyboardId`: a number that identifies it for as long as it stays connected. It is **not** persisted across runs. Whether the *same* physical keyboard gets the *same* id back after being unplugged and replugged depends on the backend and on how much identity it can see. On Linux it does when the keyboard goes back into the same port (tested with one keyboard); on Windows we haven't tested it. Don't rely on it yet. `Event::KeyboardAdded` carries a `KeyboardInfo` with what the backend knows about the device:

| Field | Meaning |
|---|---|
| `name` | Product name where the backend has one. |
| `vendor_id`, `product_id` | USB ids when known. Built-in keyboards often have none. |
| `location` | Where it's attached, when the backend reports it (evdev: the physical port). |
| `path` | The OS device path. |

If you want to remember "this is player 2's keyboard" across runs, you can try matching on these. It's a heuristic: two identical keyboards share name and ids, and moving one to another port can change its path or location. Don't treat it as a guaranteed physical-device identity.

### Keys

A `Key` is a **physical key**, named by its USB HID usage, so it means the same thing on every backend and every layout. `Key::A` is the key that's labelled `A` on a US keyboard, wherever it sits on a French one. Media and brightness keys (HID Consumer page) are encoded as `(page << 16) | usage`. A key manykey doesn't know is `Key::UNKNOWN`; every event also carries the raw `native_code` so nothing is lost.

manykey reports positions, not characters. Turning a key into text depends on the layout, dead keys and input methods, which is the OS's job. Use your toolkit's text input for typing, and manykey for *who pressed what*.

### Events

| Event | Meaning |
|---|---|
| `KeyboardAdded(info)` | A keyboard is present (see the backend notes for when). |
| `KeyboardRemoved(id)` | It was unplugged. |
| `Key(KeyEvent)` | `keyboard`, `key`, `native_code`, `state` (`Pressed`, `Released` or `Repeat`), and `origin` (`Native` or `Synthesized`; see below). |
| `Overflow { dropped }` | Your program fell behind and events were lost. **Treat held-key state as invalid and rebuild it.** |

`Event`, `KeyState`, `EventOrigin` and `Scope` are `#[non_exhaustive]`: new variants may be added in future versions, so always include a catch-all `_` arm when you `match` on them.

**Some events are generated by manykey, not by the keyboard.** Nearly everything you receive is what the OS reported. The exceptions:

- If a keyboard is **unplugged while holding keys**, manykey can emit a `Released` for each held key just before `KeyboardRemoved`. Switch: `release_on_unplug`. (Raw Input backend; untested with a real unplug. On Linux the kernel itself releases held keys when a device disappears, so you get a `Native` release there, as CI confirmed with a virtual keyboard, and manykey's own release is only a fallback.)
- Under `OwnProcessFocused`, when your process **loses focus**, manykey emits a `Released` for every held key, then goes quiet until focus returns. Keys still physically held when focus returns are *not* re-announced; they look "up" until their next press. Switch: `release_on_focus_loss`. (Raw Input only, since that's the only backend with focused scope.)
- `Overflow` is also manykey's own marker. It's a notification, not a key event, and has no switch.

**Both release behaviours are on by default and can be turned off**, because only your application knows whether it wants them: they keep a held-key table correct for apps that keep one, and they are unwanted noise for apps that record exactly what the OS sent. If you switch them off, the corresponding releases are simply never produced.

Every `KeyEvent` carries an **`origin`**: `Native` if the OS reported it, `Synthesized` if manykey generated it. Use it to tell them apart: for diagnostics, for recording what the OS actually did, or to ignore the synthesized releases when you'd rather not have them (or switch them off entirely with the options below). In C it's `manykey_event.origin` (`MANYKEY_ORIGIN_NATIVE` / `MANYKEY_ORIGIN_SYNTHESIZED`), and `manykey watch` marks synthesized events with `[synthesized]`. Synthesized events never leave your `Listener`: they're added to *your* event stream only, and nothing is sent to the OS or to other programs.

**Overflow.** Events wait in a bounded queue (4096 by default). If you stop draining it, the oldest events are discarded: repeats first, then presses, then releases, and device add/remove last. An `Overflow` event is delivered ahead of whatever survived, so you always find out. A dropped *release* would otherwise leave a key stuck down in your state.

## Options

`Listener::new()` uses the defaults. For anything else:

```rust,no_run
use manykey::{Listener, Options, Scope};

let listener = Listener::with_options(Options {
    scope: Scope::OwnProcessFocused,
    ..Options::default()
})?;
# Ok::<(), std::io::Error>(())
```

| Option | Default | Backends | What it does |
|---|---|---|---|
| `backend` | `Auto` | all | Which backend reads the keyboards: `Auto`, `RawInput` (Windows) or `Evdev` (Linux). Naming one that isn't available on this platform returns an error. |
| `scope` | `Global` | `OwnProcessFocused`: Raw Input only | `Global` reports every keystroke on every keyboard, whoever has focus (what a background tool needs). `OwnProcessFocused` reports only while a window of **your** process is the foreground window, and releases any held keys when focus is lost. See the note below. |
| `reclaim_registration` | `false` | Raw Input | If another part of your process (a windowing library, an engine) takes over raw keyboard input registration, take it back. Checked twice a second. Off by default because it overrides the host. Turn it on if you get **no events inside a program that has its own window**. |
| `include_injected` | `false` | Raw Input | Also report input injected by software (`SendInput`, AutoHotkey and similar), which has no physical device and appears as one extra pseudo-keyboard named "Injected input (no physical device)". We haven't tested what other software, such as remote-desktop or accessibility tools, does here. |
| `release_on_unplug` | `true` | all | Emit a `Released` for each held key when a keyboard is unplugged, before `KeyboardRemoved`. See [Events](#events). |
| `release_on_focus_loss` | `true` | Raw Input | With `OwnProcessFocused`: emit a `Released` for each held key when your process loses focus. If off, those keys never get a release from manykey. |
| `capacity` | `4096` | all | How many events the queue holds before discarding (see [Events](#events)). |

Options that don't apply to the backend in use are ignored, except `backend` and `scope`, which return an error if they can't be honoured, so that you never silently get something other than what you asked for.

In C: `manykey_options_default(&opts)`, set `opts.backend`, `opts.scope`, `opts.capacity` and `opts.flags` (`MANYKEY_OPTION_RECLAIM_REGISTRATION`, `MANYKEY_OPTION_INCLUDE_INJECTED`, `MANYKEY_OPTION_RELEASE_ON_UNPLUG`, `MANYKEY_OPTION_RELEASE_ON_FOCUS_LOSS`), then `manykey_open_with(&opts, &ctx)`. Always start from `manykey_options_default`: the two release flags are *on* in the defaults, so a zeroed struct switches them off.

**`OwnProcessFocused` is a courtesy, not a security boundary.** The operating system still lets your process observe input while it isn't focused; the library simply doesn't pass those events on. It exists so a game or app that only cares about its own input can say so, and so that your users and reviewers can see that's what you asked for.

## What manykey does to your program

A library that reads raw keyboard input should say exactly what it does. This section is about the library as a whole; each backend's own behaviour is described under [Backends](#backends).

**What it reads.** Every keystroke on every keyboard, including while your window isn't focused (unless you choose `OwnProcessFocused`). This is the same capability a keylogger would use, so treat it as one: don't ship it in software whose users wouldn't expect it, and don't send what it reports anywhere they wouldn't expect.

**What it doesn't do.**
- It **observes only**. It never blocks, changes, delays or injects keystrokes *into the operating system or other programs*. Other programs, including your own, still get their keys as usual. (It can add a few synthesized events to *your own* event stream; they're on by default, switchable, and listed under [Events](#events).)
- No driver, no kernel component, no hooks (`SetWindowsHookEx`), and on Windows no administrator rights.
- It does not poll or ask the OS for the current state of keys. It only reports the events it receives.
- It does not log, store or transmit anything. The library makes no network connections and writes no files. (The [viewer app](#the-viewer-app) saves a small preferences file; the library does not.)

**What it changes in your process.**
- It starts background threads, and the Windows and Linux backends do a little extra housekeeping, listed per backend above (opening device handles to read names; rescanning for hotplug; the optional timers).
- On Windows, **raw input registration is per process**, so manykey and another raw-input user in the same process can take it from each other (see the Raw Input section). Two `Listener`s in one process currently have this problem with each other too. Open one and share it.

## Linux permissions

Reading `/dev/input/event*` normally needs membership of the `input` group (or an ACL granted by a udev rule). **Be aware what that means:** any program running as that user can then read every keystroke, including passwords typed into other windows, regardless of focus. That's a real cost, and one reason a lower-privilege backend is on the roadmap.

- If keyboards exist but manykey can't open any of them, `Listener::new()` fails with a permission error that explains the `input` group (CI checks this as an unprivileged user). A device that can't be opened while others can is skipped rather than reported, so a keyboard missing from the list can mean a permissions problem.
- Adding yourself to the `input` group (`sudo usermod -aG input $USER`, then log in again) is the simple route for your own machine.
- Shipping software that requires this to ordinary users is a decision to make deliberately.

## Limitations and FAQ

**Why doesn't it show my Fn key?** On nearly every keyboard, Fn is handled inside the keyboard's firmware and never sent to the computer. Software, including manykey, can only see its effects (for example, F5 versus a brightness key).

**Why don't my media or brightness keys show up?** Some media keys arrive as ordinary keyboard keys and are reported (play/pause, next/previous, stop, mute, volume). Other keys, brightness among them on the keyboard we tried, didn't show up at all. Our current guess is that they're sent as a different kind of HID report that the Windows backend doesn't read yet, but we haven't confirmed that. On Linux, evdev defines brightness key codes that the evdev backend maps, though we haven't tested them.

**Why is there an extra "keyboard" that only produces Unknown keys?** Some devices that aren't keyboards you type on still identify themselves as keyboards: laptop hotkey and Fn controllers, for example, or other HID devices such as HID-over-I2C ones. On Windows we saw one called "HIDI2C Device". Its keys aren't ones manykey knows, so they show as `Unknown` with the raw scancode (the viewer prints it as `?0x....`). We haven't confirmed what that device was; it's a good candidate for the brightness and Fn keys that otherwise never showed up. On Linux, a gaming mouse's keyboard interface (for its macro buttons) shows up the same way.

**Can it tell two identical keyboards apart?** While they're plugged in, that's the intent: each gets its own id. We haven't been able to test with two identical keyboards. Recognising which is which across runs is best-effort; see [Keyboards](#keyboards).

**Does it work in remote-desktop sessions, virtual machines or with virtual keyboards?** **Untested.** We don't know how remote-desktop clients, virtual machines, KVM switches or software keyboards appear to each backend. Expect surprises, and please report what you see.

**What happens if a key release is lost?** On Windows we have seen keys left "down" after a release that manykey never received. We suspect input going elsewhere (the lock screen, a UAC prompt, an elevated window) but haven't confirmed the cause. manykey deliberately doesn't poll the OS to repair this behind your back; if it matters for your app, offer a "reset held keys" action. The [viewer](#the-viewer-app) has a button that asks Windows which lit keys are really down.

**Can I use it to block or remap keys?** No. manykey only observes. Blocking requires a driver or hooks, which it deliberately avoids.

**Is `KeyboardId` the same on every run?** No. Use `KeyboardInfo` to recognise keyboards across runs.

## The viewer app

`apps/keyboard-view` is a small egui app that doubles as the best way to see what manykey does: one tile per connected keyboard, each drawing a keyboard whose keys light up as they're pressed, with a typing box per keyboard.

```text
cargo run -p manykey-view
cargo run -p manykey-view -- --focused    # only react while the viewer has focus
cargo run -p manykey-view -- --log        # also print every event to stdout
```

On Linux the viewer needs the same device access as the library (see [Linux permissions](#linux-permissions)), and `--focused` isn't available because the evdev backend can't tell which window has focus. If you run it with `sudo`, its preferences are saved in root's home directory rather than yours.

- Pick each keyboard's form factor from its dropdown (all keys, standard, no numpad, 75%, compact, keypad-only). The choice is remembered per keyboard in `%APPDATA%\manykey\viewer.conf` (Windows) or `~/.config/manykey/viewer.conf`.
- A key that isn't part of a keyboard's chosen form factor turns the tile border orange and is listed ("Not in this layout"): either the layout needs changing, or something unexpected is sending keys.
- "Show advanced keyboard information" reveals each keyboard's location and device path. "Release stuck keys" asks Windows which lit keys are really down and un-lights the rest, but only when you click it. That check uses a Windows API from the viewer itself; it is not part of the library, which doesn't query key state.
- Layouts live one per file in `apps/keyboard-view/src/layouts/`. Adding one is a short file plus a line in `mod.rs`.

## Building and testing

You need a recent stable Rust toolchain.

```text
cargo build --release          # library, C libraries, and the manykey command line tool
cargo test --workspace         # unit tests (no keyboard hardware needed)
cargo run -p manykey-view      # the viewer (builds egui; the first build is slow)
```

To regenerate the C header after changing `src/capi.rs`:

```text
cbindgen --config cbindgen.toml --crate manykey --output include/manykey.h
```

Layout of the repository:

```text
src/                  the library
src/backend/          one file per backend (windows.rs, linux.rs)
include/manykey.h     generated C header
examples/             C example
apps/keyboard-view/   the viewer app (separate crate; not built by default)
```

## Alternatives and prior art

manykey isn't the first attempt at this problem. These are the related projects we know of; we haven't done a thorough survey, and corrections are welcome.

- **ManyMouse** is a C library with the same goal for mice, and one we looked at while designing the API.
- **Interception and AutoHotInterception (AHI)** use a kernel driver so AutoHotkey scripts can act on a specific keyboard. They can intercept input before applications see it, at the cost of installing a driver. manykey needs no driver, and in exchange can only observe.
- **LuaMacros** is a Lua scripting tool for dedicating a keyboard to macros.
- **SDL3** lets an application list keyboards and see which one produced an event, within SDL's windowing model. We haven't compared how it behaves for input outside your window. manykey is for programs that don't use SDL, or that need input outside a window.
- **Raw Input (Windows), evdev and libinput (Linux)** are the operating-system interfaces manykey is built on or may be built on.

## Roadmap

Roughly in order, and subject to change:

- Validate on real hardware: identical keyboards, more makes of USB keyboard, and more Linux machines (media keys, Bluetooth keyboards, Wayland sessions). Then fix what that finds, and add a `manykey doctor` command.
- Build the XInput2 backend for lower-privilege Linux use (the `backend` option already exists to select it).
- A shared Windows backend so several `Listener`s in one process don't compete.
- `keyboards()` for synchronous inventory, and `held_keys()` snapshots for recovering from overflow.
- Windows support for HID Consumer-page keys (brightness and friends).
- Bindings and packaging: Python, a Godot extension, .NET, Debian packages.

## AI use in development

How this project was built is described in the AI disclosure near the top. This section is about contributions.

AI-assisted contributions are welcome, and the project itself is built that way. Unreviewed ones are not. "Slop" is what you get when nobody takes responsibility for reviewing what an AI wrote and for making sure it adds up to a coherent design, so every contribution must come from an accountable human who has personally reviewed every line and can answer questions about its design and how it was verified. Multi-keyboard input is fiddly platform plumbing that I never found rewarding enough to build by hand, and AI assistance is what made it worth attempting. Details are in [CONTRIBUTING.md](https://github.com/libmanykey/manykey/blob/HEAD/CONTRIBUTING.md).

## Contributing

Issues and pull requests are welcome at <https://github.com/libmanykey/manykey>. The most useful contribution right now is **testing on real hardware**; [CONTRIBUTING.md](https://github.com/libmanykey/manykey/blob/HEAD/CONTRIBUTING.md) lists what we most need checked and how to report it. If you report a bug, include your OS, what keyboards were attached, and the output of `manykey list` / `manykey watch`. For suspected security problems, please report privately; see [SECURITY.md](https://github.com/libmanykey/manykey/blob/HEAD/SECURITY.md).

## License

MIT. See [LICENSE](https://github.com/libmanykey/manykey/blob/HEAD/LICENSE).
