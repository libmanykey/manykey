//! The `ManyKeyboard` node.

use crate::keymap::to_godot;
use godot::classes::node::ProcessMode;
use godot::classes::{INode, Node};
use godot::obj::EngineEnum;
use godot::prelude::*;
use manykey::{Event, EventOrigin, Key, KeyState, KeyboardInfo, Listener, Options, Scope};
use std::collections::{BTreeMap, BTreeSet};

struct Known {
    info: KeyboardInfo,
    connected: bool,
}

/// Reports which physical keyboard each key press came from.
///
/// Add one to your scene (or create it in code), connect to its signals, and
/// you get `(keyboard_id, key)` instead of just a key. Keys are reported by
/// physical position (Godot's `physical_keycode`), and each keyboard has an id
/// that stays the same while it's plugged in.
///
/// By default it only reports keys while your game's window has focus. Turn on
/// `global_capture` to receive keys even when it doesn't (for example for a
/// background tool). Settings are read when the node starts.
#[derive(GodotClass)]
#[class(base=Node)]
pub struct ManyKeyboard {
    base: Base<Node>,
    listener: Option<Listener>,
    keyboards: BTreeMap<u64, Known>,
    /// keyboard id -> keys currently held. Tracked as manykey keys, not Godot
    /// keycodes: left and right Shift are one Godot key, so releasing one must
    /// not make the other look released.
    pressed: BTreeMap<u64, BTreeSet<Key>>,
    last_error: GString,

    /// Start listening automatically when the node enters the scene tree.
    #[export]
    auto_start: bool,
    /// Receive keys even when this game's window isn't focused. Off by default.
    /// Required on Linux, where focus-only capture isn't available.
    #[export]
    global_capture: bool,
    /// Also report keys injected by software (macros, remote tools) as an extra keyboard (Windows).
    #[export]
    include_injected: bool,
    /// If another part of the process takes over raw keyboard input, take it back (Windows). Rarely needed.
    #[export]
    reclaim_registration: bool,
}

#[godot_api]
impl INode for ManyKeyboard {
    fn init(base: Base<Node>) -> Self {
        // Keep draining manykey's queue while the game is paused, so events
        // don't pile up and overflow (and a pause menu can still use them).
        // A default only: the inspector or a script can change it.
        base.to_init_gd().set_process_mode(ProcessMode::ALWAYS);
        Self {
            base,
            listener: None,
            keyboards: BTreeMap::new(),
            pressed: BTreeMap::new(),
            last_error: GString::new(),
            auto_start: true,
            global_capture: false,
            include_injected: false,
            reclaim_registration: false,
        }
    }

    fn ready(&mut self) {
        if self.auto_start {
            self.start();
        }
    }

    fn process(&mut self, _delta: f64) {
        self.pump();
    }

    fn exit_tree(&mut self) {
        self.stop();
    }
}

#[godot_api]
impl ManyKeyboard {
    /// A keyboard is present. Sent for keyboards already connected at start-up and for ones plugged in later.
    /// `vendor_id` and `product_id` are -1 when unknown; `location` is empty when unknown.
    #[signal]
    fn keyboard_added(keyboard_id: i64, name: GString, vendor_id: i64, product_id: i64, location: GString);

    /// A keyboard was unplugged.
    #[signal]
    fn keyboard_removed(keyboard_id: i64);

    /// A key changed state on one keyboard. `physical_keycode` is a Godot `Key` value (`KEY_UNKNOWN` if Godot
    /// has no code for it). `pressed` is false for a release; `echo` is true for auto-repeat. `hid_usage` is
    /// manykey's own key code, which tells left and right modifiers apart. `synthesized` is true when manykey
    /// generated the event (for example a release for a key still held when focus was lost) instead of the OS.
    #[signal]
    fn key_event(
        keyboard_id: i64,
        physical_keycode: i64,
        pressed: bool,
        echo: bool,
        hid_usage: i64,
        synthesized: bool,
        native_code: i64,
    );

    /// Events were lost because the game fell behind. Held-key state was reset; rebuild any you keep.
    #[signal]
    fn overflow(dropped: i64);

    /// Start listening. Returns false (and reports the reason) if it can't; see `get_last_error()`.
    #[func]
    fn start(&mut self) -> bool {
        if self.listener.is_some() {
            return true;
        }
        let options = Options {
            scope: if self.global_capture { Scope::Global } else { Scope::OwnProcessFocused },
            include_injected: self.include_injected,
            reclaim_registration: self.reclaim_registration,
            ..Options::default()
        };
        match Listener::with_options(options) {
            Ok(listener) => {
                self.listener = Some(listener);
                self.last_error = GString::new();
                true
            }
            Err(e) => {
                let message = format!("ManyKeyboard could not start: {e}");
                godot_error!("{message}");
                self.last_error = GString::from(&message);
                false
            }
        }
    }

    /// Stop listening and forget which keys were held.
    #[func]
    fn stop(&mut self) {
        self.listener = None;
        self.pressed.clear();
    }

    /// Is the listener running?
    #[func]
    fn is_running(&self) -> bool {
        self.listener.is_some()
    }

    /// Why the last `start()` failed, or an empty string.
    #[func]
    fn get_last_error(&self) -> GString {
        self.last_error.clone()
    }

    /// The keyboards seen so far, as an array of dictionaries with `id`, `name`, `vendor_id`, `product_id`,
    /// `location` and `connected`.
    #[func]
    fn get_keyboards(&self) -> VarArray {
        let mut out = VarArray::new();
        for (id, known) in &self.keyboards {
            let name = GString::from(&known.info.name);
            let location = GString::from(known.info.location.as_deref().unwrap_or(""));
            let d = vdict! {
                "id" => *id as i64,
                "name" => &name,
                "vendor_id" => known.info.vendor_id.map_or(-1, i64::from),
                "product_id" => known.info.product_id.map_or(-1, i64::from),
                "location" => &location,
                "connected" => known.connected,
            };
            out.push(&d.to_variant());
        }
        out
    }

    /// Is this physical key (a Godot `Key` value) currently held on this keyboard?
    #[func]
    fn is_key_pressed(&self, keyboard_id: i64, physical_keycode: i64) -> bool {
        self.held_godot_keys(keyboard_id).contains(&(physical_keycode as i32))
    }

    /// Every physical key (as Godot `Key` values) currently held on this keyboard.
    #[func]
    fn get_pressed_keys(&self, keyboard_id: i64) -> PackedInt32Array {
        let keys: Vec<i32> = self.held_godot_keys(keyboard_id).into_iter().collect();
        PackedInt32Array::from(keys.as_slice())
    }
}

impl ManyKeyboard {
    /// The Godot keycodes held on a keyboard (both Shifts held is one `KEY_SHIFT`).
    fn held_godot_keys(&self, keyboard_id: i64) -> BTreeSet<i32> {
        let held = self.pressed.get(&(keyboard_id as u64));
        held.into_iter().flatten().map(|&key| to_godot(key).ord()).collect()
    }

    /// Drain everything that's arrived and turn it into signals. Runs on the main thread, once per frame.
    fn pump(&mut self) {
        let Some(listener) = &self.listener else { return };
        let events: Vec<Event> = std::iter::from_fn(|| listener.try_recv()).collect();
        for event in events {
            self.handle(event);
        }
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::KeyboardAdded(info) => {
                let id = info.id.0;
                let (name, location) = (info.name.clone(), info.location.clone().unwrap_or_default());
                let (vendor_id, product_id) =
                    (info.vendor_id.map_or(-1, i64::from), info.product_id.map_or(-1, i64::from));
                // Record it first, so a handler that calls get_keyboards() sees it.
                self.keyboards.insert(id, Known { info, connected: true });
                self.signals().keyboard_added().emit(
                    id as i64,
                    name.as_str(),
                    vendor_id,
                    product_id,
                    location.as_str(),
                );
            }
            Event::KeyboardRemoved(id) => {
                if let Some(known) = self.keyboards.get_mut(&id.0) {
                    known.connected = false;
                }
                self.pressed.remove(&id.0);
                self.signals().keyboard_removed().emit(id.0 as i64);
            }
            Event::Key(k) => {
                let (pressed, echo) = match k.state {
                    KeyState::Pressed => (true, false),
                    KeyState::Repeat => (true, true),
                    KeyState::Released => (false, false),
                    _ => return,
                };
                let code = to_godot(k.key).ord();
                let held = self.pressed.entry(k.keyboard.0).or_default();
                if pressed {
                    held.insert(k.key);
                } else {
                    held.remove(&k.key);
                }
                self.signals().key_event().emit(
                    k.keyboard.0 as i64,
                    code as i64,
                    pressed,
                    echo,
                    k.key.0 as i64,
                    k.origin == EventOrigin::Synthesized,
                    k.native_code as i64,
                );
            }
            Event::Overflow { dropped } => {
                self.pressed.clear();
                self.signals().overflow().emit(dropped as i64);
            }
            _ => {} // an event kind added to manykey after this was written
        }
    }
}
