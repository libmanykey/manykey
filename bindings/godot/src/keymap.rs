//! manykey's physical keys (USB HID usages) to Godot's `Key` enum.
//!
//! manykey reports key *positions*, so this maps to what Godot calls the
//! physical keycode: `Key::A` is the key that is "A" on a US layout, wherever
//! it is on the user's keyboard. Godot has one code each for Shift, Ctrl, Alt
//! and Meta (no left/right), so those collapse; the signal also carries the
//! HID usage so scripts can still tell them apart.

use godot::global::Key as G;
use manykey::Key as K;

/// The Godot key for a manykey key, or `Key::UNKNOWN` if Godot has no equivalent.
pub fn to_godot(key: K) -> G {
    match key {
        K::A => G::A,
        K::B => G::B,
        K::C => G::C,
        K::D => G::D,
        K::E => G::E,
        K::F => G::F,
        K::G => G::G,
        K::H => G::H,
        K::I => G::I,
        K::J => G::J,
        K::K => G::K,
        K::L => G::L,
        K::M => G::M,
        K::N => G::N,
        K::O => G::O,
        K::P => G::P,
        K::Q => G::Q,
        K::R => G::R,
        K::S => G::S,
        K::T => G::T,
        K::U => G::U,
        K::V => G::V,
        K::W => G::W,
        K::X => G::X,
        K::Y => G::Y,
        K::Z => G::Z,

        K::Digit1 => G::KEY_1,
        K::Digit2 => G::KEY_2,
        K::Digit3 => G::KEY_3,
        K::Digit4 => G::KEY_4,
        K::Digit5 => G::KEY_5,
        K::Digit6 => G::KEY_6,
        K::Digit7 => G::KEY_7,
        K::Digit8 => G::KEY_8,
        K::Digit9 => G::KEY_9,
        K::Digit0 => G::KEY_0,

        K::Enter => G::ENTER,
        K::Escape => G::ESCAPE,
        K::Backspace => G::BACKSPACE,
        K::Tab => G::TAB,
        K::Space => G::SPACE,
        K::Minus => G::MINUS,
        K::Equal => G::EQUAL,
        K::LeftBracket => G::BRACKETLEFT,
        K::RightBracket => G::BRACKETRIGHT,
        K::Backslash => G::BACKSLASH,
        K::Semicolon => G::SEMICOLON,
        K::Quote => G::APOSTROPHE,
        K::Grave => G::QUOTELEFT,
        K::Comma => G::COMMA,
        K::Period => G::PERIOD,
        K::Slash => G::SLASH,
        K::CapsLock => G::CAPSLOCK,

        K::F1 => G::F1,
        K::F2 => G::F2,
        K::F3 => G::F3,
        K::F4 => G::F4,
        K::F5 => G::F5,
        K::F6 => G::F6,
        K::F7 => G::F7,
        K::F8 => G::F8,
        K::F9 => G::F9,
        K::F10 => G::F10,
        K::F11 => G::F11,
        K::F12 => G::F12,
        K::F13 => G::F13,
        K::F14 => G::F14,
        K::F15 => G::F15,
        K::F16 => G::F16,
        K::F17 => G::F17,
        K::F18 => G::F18,
        K::F19 => G::F19,
        K::F20 => G::F20,
        K::F21 => G::F21,
        K::F22 => G::F22,
        K::F23 => G::F23,
        K::F24 => G::F24,

        K::PrintScreen => G::PRINT,
        K::ScrollLock => G::SCROLLLOCK,
        K::Pause => G::PAUSE,
        K::Insert => G::INSERT,
        K::Home => G::HOME,
        K::PageUp => G::PAGEUP,
        K::Delete => G::DELETE,
        K::End => G::END,
        K::PageDown => G::PAGEDOWN,
        K::Right => G::RIGHT,
        K::Left => G::LEFT,
        K::Down => G::DOWN,
        K::Up => G::UP,

        K::NumLock => G::NUMLOCK,
        K::KpDivide => G::KP_DIVIDE,
        K::KpMultiply => G::KP_MULTIPLY,
        K::KpMinus => G::KP_SUBTRACT,
        K::KpPlus => G::KP_ADD,
        K::KpEnter => G::KP_ENTER,
        K::Kp1 => G::KP_1,
        K::Kp2 => G::KP_2,
        K::Kp3 => G::KP_3,
        K::Kp4 => G::KP_4,
        K::Kp5 => G::KP_5,
        K::Kp6 => G::KP_6,
        K::Kp7 => G::KP_7,
        K::Kp8 => G::KP_8,
        K::Kp9 => G::KP_9,
        K::Kp0 => G::KP_0,
        K::KpPeriod => G::KP_PERIOD,

        K::Menu => G::MENU,
        K::Mute => G::VOLUMEMUTE,
        K::VolumeUp => G::VOLUMEUP,
        K::VolumeDown => G::VOLUMEDOWN,
        K::MediaPlayPause => G::MEDIAPLAY,
        K::MediaStop => G::MEDIASTOP,
        K::MediaNext => G::MEDIANEXT,
        K::MediaPrev => G::MEDIAPREVIOUS,

        K::LeftCtrl | K::RightCtrl => G::CTRL,
        K::LeftShift | K::RightShift => G::SHIFT,
        K::LeftAlt | K::RightAlt => G::ALT,
        K::LeftGui | K::RightGui => G::META,

        // Includes the ISO extra key and brightness keys, which Godot has no code for.
        _ => G::UNKNOWN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use godot::obj::EngineEnum;

    #[test]
    fn letters_and_digits_use_godots_ascii_values() {
        // Godot's letter and digit keycodes are their ASCII codes.
        assert_eq!(to_godot(K::A).ord(), 'A' as i32);
        assert_eq!(to_godot(K::W).ord(), 'W' as i32);
        assert_eq!(to_godot(K::Z).ord(), 'Z' as i32);
        assert_eq!(to_godot(K::Digit0).ord(), '0' as i32);
        assert_eq!(to_godot(K::Digit9).ord(), '9' as i32);
    }

    #[test]
    fn left_and_right_modifiers_collapse_to_one_godot_key() {
        assert_eq!(to_godot(K::LeftShift), to_godot(K::RightShift));
        assert_eq!(to_godot(K::LeftCtrl), to_godot(K::RightCtrl));
        assert_ne!(to_godot(K::LeftShift), to_godot(K::LeftCtrl));
    }

    #[test]
    fn keys_godot_lacks_are_unknown_not_wrong() {
        assert_eq!(to_godot(K::UNKNOWN), G::UNKNOWN);
        assert_eq!(to_godot(K::BrightnessUp), G::UNKNOWN);
        assert_eq!(to_godot(K::NonUsBackslash), G::UNKNOWN);
    }

    #[test]
    fn every_key_on_a_standard_keyboard_maps_to_something() {
        let standard = [
            K::Escape,
            K::F1,
            K::F12,
            K::PrintScreen,
            K::Grave,
            K::Digit1,
            K::Minus,
            K::Equal,
            K::Backspace,
            K::Tab,
            K::LeftBracket,
            K::RightBracket,
            K::Backslash,
            K::CapsLock,
            K::Semicolon,
            K::Quote,
            K::Enter,
            K::LeftShift,
            K::Comma,
            K::Period,
            K::Slash,
            K::RightShift,
            K::LeftCtrl,
            K::LeftGui,
            K::LeftAlt,
            K::Space,
            K::RightAlt,
            K::RightGui,
            K::Menu,
            K::RightCtrl,
            K::Insert,
            K::Delete,
            K::Home,
            K::End,
            K::PageUp,
            K::PageDown,
            K::Up,
            K::Down,
            K::Left,
            K::Right,
            K::NumLock,
            K::KpDivide,
            K::KpMultiply,
            K::KpMinus,
            K::KpPlus,
            K::KpEnter,
            K::Kp0,
            K::Kp1,
            K::Kp9,
            K::KpPeriod,
            K::ScrollLock,
            K::Pause,
        ];
        for k in standard {
            assert_ne!(to_godot(k), G::UNKNOWN, "{k:?} has no Godot mapping");
        }
    }
}
