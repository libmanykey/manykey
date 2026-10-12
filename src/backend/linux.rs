//! Linux backend: one reader thread per evdev keyboard, plus a hot-plug scanner.
//!
//! Needs read access to `/dev/input/event*` (usually the `input` group).
//! Keyboards are discovered from `/proc/bus/input/devices`, using udev's test
//! for a full keyboard (Esc, digits and Q to D), which separates real keyboards
//! from power buttons and the like. The interfaces of one physical keyboard are
//! grouped and reported as one keyboard; see `parse_keyboards`.

use crate::{
    from_evdev, Backend, Event, EventOrigin, EventSink, KeyEvent, KeyState, KeyboardId, KeyboardInfo, Options, Scope,
};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const EV_KEY: u16 = 1;

pub struct LinuxBackend {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Backend for LinuxBackend {}

impl LinuxBackend {
    pub fn start(tx: EventSink, options: &Options) -> io::Result<Self> {
        // evdev has no notion of window focus. Refuse rather than silently
        // reporting everything to a caller that asked for less.
        if options.scope != Scope::Global {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Scope::OwnProcessFocused is not supported on Linux (evdev cannot tell which window has focus)",
            ));
        }
        let release_on_unplug = options.release_on_unplug;
        // Fail early if /proc is unusable rather than silently reporting nothing.
        let keyboards = list_keyboards()?;
        // Opening a device is where missing permission shows up. If keyboards
        // exist but none can be opened, say so, instead of reporting an empty
        // keyboard list that looks like "no keyboards".
        if !keyboards.is_empty() {
            let mut first_error = None;
            let mut opened = false;
            for kb in &keyboards {
                match File::open(&kb.path) {
                    Ok(_) => {
                        opened = true;
                        break;
                    }
                    Err(e) => {
                        first_error.get_or_insert((kb.path.clone(), e));
                    }
                }
            }
            if let (false, Some((path, err))) = (opened, first_error) {
                return Err(open_failure(&path, &err));
            }
        }
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let thread =
            thread::Builder::new().name("manykey-scan".into()).spawn(move || scan_loop(tx, s, release_on_unplug))?;
        Ok(Self { stop, thread: Some(thread) })
    }
}

impl Drop for LinuxBackend {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// An error for a device we couldn't open, with what to do about it.
fn open_failure(path: &str, err: &io::Error) -> io::Error {
    let hint = if err.kind() == io::ErrorKind::PermissionDenied {
        " Reading input devices normally needs membership of the `input` group (log out and back in after \
         adding yourself). That permission lets any program you run read every keystroke; see \
         https://github.com/libmanykey/manykey#linux-permissions"
    } else {
        ""
    };
    io::Error::new(err.kind(), format!("cannot open {path}: {err}.{hint}"))
}

/// One evdev interface that belongs to a keyboard.
struct Found {
    name: String,
    path: String,
    vendor_id: Option<u16>,
    product_id: Option<u16>,
    phys: Option<String>,
    /// Which physical keyboard this interface belongs to. A composite USB
    /// keyboard has several interfaces (main keys, an NKRO report, System
    /// Control, Consumer Control); they share a group and so one KeyboardId.
    group: String,
}

fn list_keyboards() -> io::Result<Vec<Found>> {
    Ok(parse_keyboards(&std::fs::read_to_string("/proc/bus/input/devices")?))
}

/// The keyboard interfaces in `/proc/bus/input/devices` text.
///
/// An interface counts if it is a full keyboard (see
/// [`DeviceBlock::is_full_keyboard`]) or belongs to the same physical device as
/// one (its media and system-key interfaces). Full keyboards come first, so a
/// keyboard is announced with its main interface's name and path.
fn parse_keyboards(text: &str) -> Vec<Found> {
    let devices: Vec<DeviceBlock> = text
        .split("\n\n")
        .map(DeviceBlock::parse)
        .filter(|d| d.has_kbd_handler() && d.event_node().is_some())
        .collect();
    let keyboards: HashSet<String> = devices.iter().filter(|d| d.is_full_keyboard()).map(DeviceBlock::group).collect();
    let mut found: Vec<&DeviceBlock> = devices.iter().filter(|d| keyboards.contains(&d.group())).collect();
    found.sort_by_key(|d| !d.is_full_keyboard()); // stable, so /proc order is kept otherwise
    found.into_iter().map(DeviceBlock::to_found).collect()
}

/// One device's block of `/proc/bus/input/devices`, e.g.
///
/// ```text
/// I: Bus=0003 Vendor=04d9 Product=1603 Version=0110
/// N: Name="  USB Keyboard"
/// P: Phys=usb-0000:09:00.3-1/input0
/// U: Uniq=
/// H: Handlers=sysrq kbd leds event2
/// B: KEY=1000000000007 ff800000000007ff febeffdff3cfffff fffffffffffffffe
/// ```
///
/// The format is what the kernel's `input_devices_seq_show` prints; it isn't
/// formally documented.
#[derive(Default)]
struct DeviceBlock<'a> {
    name: &'a str,
    phys: &'a str,
    uniq: &'a str,
    vendor_id: Option<u16>,
    product_id: Option<u16>,
    handlers: &'a str,
    /// The `KEY` capability bitmap: hex words, most significant first.
    key_bits: &'a str,
}

impl<'a> DeviceBlock<'a> {
    fn parse(block: &'a str) -> Self {
        let mut d = Self::default();
        for line in block.lines() {
            let Some((tag, rest)) = line.split_once(": ") else { continue };
            let Some((key, value)) = rest.split_once('=') else { continue };
            match (tag, key) {
                ("I", _) => {
                    // Several fields on one line: Bus=0003 Vendor=04d9 Product=1603 Version=0110
                    for (k, v) in rest.split_whitespace().filter_map(|kv| kv.split_once('=')) {
                        match k {
                            "Vendor" => d.vendor_id = u16::from_str_radix(v, 16).ok(),
                            "Product" => d.product_id = u16::from_str_radix(v, 16).ok(),
                            _ => {}
                        }
                    }
                }
                // Some keyboards pad their names ("  USB Keyboard").
                ("N", "Name") => d.name = value.trim_matches('"').trim(),
                ("P", "Phys") => d.phys = value.trim(),
                ("U", "Uniq") => d.uniq = value.trim(),
                ("H", "Handlers") => d.handlers = value,
                ("B", "KEY") => d.key_bits = value.trim(),
                _ => {}
            }
        }
        d
    }

    fn has_kbd_handler(&self) -> bool {
        self.handlers.split_whitespace().any(|h| h == "kbd")
    }

    fn event_node(&self) -> Option<&'a str> {
        self.handlers.split_whitespace().find(|h| h.starts_with("event"))
    }

    /// The test udev uses for `ID_INPUT_KEYBOARD`: the device has Esc, the
    /// number row and Q through D, which are key codes 1 to 31, the low bits of
    /// the bitmap's last word. That rules out power buttons, laptop hotkey
    /// devices and media-key interfaces, which have only a few keys.
    fn is_full_keyboard(&self) -> bool {
        const ESC_TO_D: u64 = 0xffff_fffe;
        let low_word = self.key_bits.split_whitespace().next_back().and_then(|w| u64::from_str_radix(w, 16).ok());
        low_word.is_some_and(|w| w & ESC_TO_D == ESC_TO_D)
    }

    /// What makes interfaces the same physical keyboard: where it's attached
    /// (`phys` without its `/inputN` interface suffix), its USB ids, and `uniq`.
    /// For USB the attachment point is the port; for Bluetooth `phys` is the
    /// adapter's address, shared by every device on it, and `uniq` is the
    /// device's own address. Without `phys` (virtual devices) interfaces are
    /// never merged.
    fn group(&self) -> String {
        let ids = format!("{:?}|{:?}|{}", self.vendor_id, self.product_id, self.uniq);
        if self.phys.is_empty() {
            return format!("name:{}|{ids}", self.name);
        }
        let device = match self.phys.rsplit_once("/input") {
            Some((base, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => base,
            _ => self.phys,
        };
        format!("phys:{device}|{ids}")
    }

    fn to_found(&self) -> Found {
        Found {
            name: self.name.to_string(),
            path: format!("/dev/input/{}", self.event_node().unwrap_or_default()),
            vendor_id: self.vendor_id,
            product_id: self.product_id,
            phys: Some(self.phys.to_string()).filter(|p| !p.is_empty()),
            group: self.group(),
        }
    }
}

fn scan_loop(tx: EventSink, stop: Arc<AtomicBool>, release_on_unplug: bool) {
    // Interface paths that have a reader thread.
    let active: Arc<Mutex<HashSet<String>>> = Arc::default();
    // How many interfaces of each keyboard are open. A keyboard is announced
    // when its first interface opens and removed when its last one closes.
    let open: Arc<Mutex<HashMap<KeyboardId, usize>>> = Arc::default();
    // Identity is the keyboard's group (port + USB ids + uniq), so replugging
    // into the same port yields the same KeyboardId even though the eventN
    // numbers change.
    let mut ids: HashMap<String, KeyboardId> = HashMap::new();
    let mut readers: Vec<JoinHandle<()>> = Vec::new();

    while !stop.load(Ordering::Relaxed) {
        for kb in list_keyboards().unwrap_or_default() {
            if active.lock().unwrap().contains(&kb.path) {
                continue;
            }
            let Ok(file) = File::open(&kb.path) else { continue };
            let next = ids.len() as u64 + 1;
            let id = *ids.entry(kb.group).or_insert(KeyboardId(next));
            active.lock().unwrap().insert(kb.path.clone());
            {
                let mut open = open.lock().unwrap();
                let count = open.entry(id).or_insert(0);
                *count += 1;
                if *count == 1 {
                    tx.send(Event::KeyboardAdded(KeyboardInfo {
                        id,
                        name: kb.name,
                        path: kb.path.clone(),
                        vendor_id: kb.vendor_id,
                        product_id: kb.product_id,
                        location: kb.phys,
                    }));
                }
            }
            let (tx, stop, active, open, path) = (tx.clone(), stop.clone(), active.clone(), open.clone(), kb.path);
            readers.push(thread::spawn(move || {
                read_device(file, id, &tx, &stop, release_on_unplug);
                active.lock().unwrap().remove(&path);
                let mut open = open.lock().unwrap();
                if let Some(count) = open.get_mut(&id) {
                    *count -= 1;
                    if *count == 0 {
                        open.remove(&id);
                        tx.send(Event::KeyboardRemoved(id));
                    }
                }
            }));
        }
        thread::sleep(Duration::from_millis(500));
    }
    for r in readers {
        let _ = r.join();
    }
}

/// Events that release every key in `held`, in a stable order.
fn release_events(id: KeyboardId, held: &HashSet<u32>) -> Vec<KeyEvent> {
    let mut codes: Vec<u32> = held.iter().copied().collect();
    codes.sort_unstable();
    codes
        .into_iter()
        .map(|code| KeyEvent {
            keyboard: id,
            key: from_evdev(code),
            native_code: code,
            state: KeyState::Released,
            origin: EventOrigin::Synthesized,
        })
        .collect()
}

fn read_device(mut file: File, id: KeyboardId, tx: &EventSink, stop: &AtomicBool, release_on_unplug: bool) {
    const SZ: usize = std::mem::size_of::<libc::input_event>();
    let mut buf = [0u8; SZ];
    let mut held: HashSet<u32> = HashSet::new();
    // Called when the device goes away (not when we're shutting down).
    let finish = |held: &HashSet<u32>| {
        if release_on_unplug {
            for ev in release_events(id, held) {
                tx.send(Event::Key(ev));
            }
        }
    };
    while !stop.load(Ordering::Relaxed) {
        // Poll with a timeout so we notice `stop` without needing a wakeup fd.
        let mut pfd = libc::pollfd { fd: file.as_raw_fd(), events: libc::POLLIN, revents: 0 };
        let n = unsafe { libc::poll(&mut pfd, 1, 200) };
        if n == 0 {
            continue;
        }
        if n < 0 || pfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            return finish(&held);
        }
        if file.read_exact(&mut buf).is_err() {
            return finish(&held); // ENODEV on unplug
        }
        let ev: libc::input_event = unsafe { std::ptr::read_unaligned(buf.as_ptr().cast()) };
        if ev.type_ != EV_KEY {
            continue;
        }
        let code = ev.code as u32;
        let state = match ev.value {
            0 => {
                held.remove(&code);
                KeyState::Released
            }
            1 => {
                held.insert(code);
                KeyState::Pressed
            }
            2 => KeyState::Repeat,
            _ => continue,
        };
        tx.send(Event::Key(KeyEvent {
            keyboard: id,
            key: from_evdev(code),
            native_code: code,
            state,
            origin: EventOrigin::Native,
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_failures_explain_the_input_group() {
        let e = open_failure("/dev/input/event3", &io::Error::from(io::ErrorKind::PermissionDenied));
        assert_eq!(e.kind(), io::ErrorKind::PermissionDenied);
        let msg = e.to_string();
        assert!(msg.contains("/dev/input/event3") && msg.contains("`input` group"), "{msg}");
    }

    #[test]
    fn other_open_failures_do_not_blame_permissions() {
        let e = open_failure("/dev/input/event3", &io::Error::from(io::ErrorKind::NotFound));
        assert!(!e.to_string().contains("input` group"));
    }

    /// Trimmed from a real machine (serial number replaced): a power button and a
    /// laptop hotkey device (neither is a full keyboard), a mouse with a keyboard interface, a plain USB
    /// keyboard with System/Consumer Control interfaces, and a ZSA Moonlander,
    /// which has two keyboard interfaces plus a mouse and its own controls.
    const DESKTOP: &str = r#"I: Bus=0019 Vendor=0000 Product=0001 Version=0000
N: Name="Power Button"
P: Phys=LNXPWRBN/button/input0
U: Uniq=
H: Handlers=kbd event1
B: KEY=8000 10000000000000 0

I: Bus=0019 Vendor=0000 Product=0000 Version=0000
N: Name="Eee PC WMI hotkeys"
P: Phys=eeepc-wmi/input0
U: Uniq=
H: Handlers=kbd event8 rfkill
B: KEY=7e40000 0 800000000000 0 0 1400b00100000 300180001100800 e000000000000 2

I: Bus=0003 Vendor=3367 Product=1968 Version=0111
N: Name="Endgame Gear XM2w 4k Wireless Gaming Mouse"
P: Phys=usb-0000:02:00.0-10/input0
U: Uniq=
H: Handlers=mouse0 event5
B: KEY=1f0000 0 0 0 0

I: Bus=0003 Vendor=3367 Product=1968 Version=0111
N: Name="Endgame Gear XM2w 4k Wireless Gaming Mouse Keyboard"
P: Phys=usb-0000:02:00.0-10/input1
U: Uniq=
H: Handlers=sysrq kbd event6
B: KEY=33eff 0 0 483ffff17aff32d bfd4444600000000 1 130c730b17c000 267bfad9415fed e09effdf01cfffff fffffffffffffffe

I: Bus=0003 Vendor=04d9 Product=1603 Version=0110
N: Name="  USB Keyboard"
P: Phys=usb-0000:09:00.3-1/input0
U: Uniq=
H: Handlers=sysrq kbd leds event2
B: KEY=1000000000007 ff800000000007ff febeffdff3cfffff fffffffffffffffe

I: Bus=0003 Vendor=04d9 Product=1603 Version=0110
N: Name="  USB Keyboard System Control"
P: Phys=usb-0000:09:00.3-1/input1
U: Uniq=
H: Handlers=kbd event3
B: KEY=c000 10000000000000 0

I: Bus=0003 Vendor=04d9 Product=1603 Version=0110
N: Name="  USB Keyboard Consumer Control"
P: Phys=usb-0000:09:00.3-1/input1
U: Uniq=
H: Handlers=kbd event4
B: KEY=1000002000000 39fad9411001 e000000000000 0

I: Bus=0003 Vendor=3297 Product=1972 Version=0111
N: Name="ZSA Technology Labs Moonlander Mark I"
P: Phys=usb-0000:02:00.0-3/input0
U: Uniq=SERIAL123
H: Handlers=sysrq kbd leds event21
B: KEY=1000000000007 ff9f207ac14057ff febeffdfffefffff fffffffffffffffe

I: Bus=0003 Vendor=3297 Product=1972 Version=0111
N: Name="ZSA Technology Labs Moonlander Mark I"
P: Phys=usb-0000:02:00.0-3/input2
U: Uniq=SERIAL123
H: Handlers=mouse1 event22
B: KEY=ff0000 0 0 0 0

I: Bus=0003 Vendor=3297 Product=1972 Version=0111
N: Name="ZSA Technology Labs Moonlander Mark I System Control"
P: Phys=usb-0000:02:00.0-3/input3
U: Uniq=SERIAL123
H: Handlers=kbd event23
B: KEY=c000 0 0 40000001000000 1200000000 0 100000800000000 40000010cc00 10168000000000 0

I: Bus=0003 Vendor=3297 Product=1972 Version=0111
N: Name="ZSA Technology Labs Moonlander Mark I Consumer Control"
P: Phys=usb-0000:02:00.0-3/input3
U: Uniq=SERIAL123
H: Handlers=kbd event24
B: KEY=33fff 0 0 483ffff17aff32d bfd4444600000000 1 130ff38b17c000 677bfad9415fed 19ed68000004400 10000002

I: Bus=0003 Vendor=3297 Product=1972 Version=0111
N: Name="ZSA Technology Labs Moonlander Mark I Keyboard"
P: Phys=usb-0000:02:00.0-3/input3
U: Uniq=SERIAL123
H: Handlers=sysrq kbd leds event25
B: KEY=1000000000007 ff98007a000007ff febeffdfffefffff fffffffffffffffe
"#;

    fn paths(found: &[Found]) -> Vec<&str> {
        found.iter().map(|f| f.path.as_str()).collect()
    }

    #[test]
    fn keyboards_and_their_extra_interfaces_are_found() {
        let found = parse_keyboards(DESKTOP);
        assert_eq!(
            paths(&found),
            [
                // full keyboards, in order (including the mouse's keyboard
                // interface, which claims every key)...
                "/dev/input/event6",
                "/dev/input/event2",
                "/dev/input/event21",
                "/dev/input/event25",
                // ...then the media/system interfaces of those keyboards.
                "/dev/input/event3",
                "/dev/input/event4",
                "/dev/input/event23",
                "/dev/input/event24",
            ]
        );
        assert_eq!(found[1].name, "USB Keyboard");
    }

    #[test]
    fn interfaces_of_one_device_share_a_group() {
        let found = parse_keyboards(DESKTOP);
        let group = |path: &str| found.iter().find(|f| f.path == path).unwrap().group.clone();
        let moonlander: HashSet<String> =
            ["event21", "event23", "event24", "event25"].iter().map(|e| group(&format!("/dev/input/{e}"))).collect();
        assert_eq!(moonlander.len(), 1);
        assert_eq!(group("/dev/input/event2"), group("/dev/input/event4"));
        let all: HashSet<String> = found.iter().map(|f| f.group.clone()).collect();
        assert_eq!(all.len(), 3); // mouse, USB keyboard, Moonlander
    }

    #[test]
    fn identical_bluetooth_keyboards_are_distinct() {
        // Bluetooth phys is the adapter address, shared by every device on it.
        let block = |uniq: &str, ev: u32| {
            format!(
                "I: Bus=0005 Vendor=05ac Product=0267 Version=0001\nN: Name=\"Magic Keyboard\"\n\
                 P: Phys=a4:c3:f0:00:00:01\nU: Uniq={uniq}\nH: Handlers=sysrq kbd leds event{ev}\n\
                 B: KEY=1000000000007 ff800000000007ff febeffdff3cfffff fffffffffffffffe\n"
            )
        };
        let text = format!("{}\n{}", block("aa:aa:aa:aa:aa:01", 30), block("aa:aa:aa:aa:aa:02", 31));
        let found = parse_keyboards(&text);
        assert_eq!(found.len(), 2);
        assert_ne!(found[0].group, found[1].group);
    }

    #[test]
    fn devices_without_phys_are_never_merged() {
        // uinput devices (as in CI) have an empty phys.
        let block = |name: &str, ev: u32| {
            format!(
                "I: Bus=0003 Vendor=1234 Product=5678 Version=0001\nN: Name=\"{name}\"\nP: Phys=\nU: Uniq=\n\
                 H: Handlers=sysrq kbd event{ev}\nB: KEY=ff ffffffffffffffff fffffffffffffffe\n"
            )
        };
        let found = parse_keyboards(&format!("{}\n{}", block("one", 30), block("two", 31)));
        assert_eq!(found.len(), 2);
        assert_ne!(found[0].group, found[1].group);
    }

    #[test]
    fn release_events_cover_every_held_key_in_order() {
        let held: HashSet<u32> = [30, 42].into_iter().collect(); // A, LeftShift
        let evs = release_events(KeyboardId(3), &held);
        assert_eq!(evs.len(), 2);
        assert!(evs.iter().all(|e| e.state == KeyState::Released && e.keyboard == KeyboardId(3)));
        assert!(evs.iter().all(|e| e.origin == EventOrigin::Synthesized));
        assert_eq!(evs[0].native_code, 30);
        assert_eq!(evs[1].native_code, 42);
    }
}
