//! Keyboard layout and modifier state for native UserSession input.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KeyboardState {
    shift: bool,
    caps_lock: bool,
}

impl KeyboardState {
    pub fn update_modifiers(&mut self, pressed: bool, code: u16) -> bool {
        match code {
            42 | 54 => {
                self.shift = pressed;
                true
            }
            58 if pressed => {
                self.caps_lock = !self.caps_lock;
                true
            }
            _ => false,
        }
    }

    pub fn character<L: KeyLayout>(&self, layout: &L, code: u16) -> Option<char> {
        layout.character(code, self.shift, self.caps_lock)
    }
}

pub trait KeyLayout {
    fn character(&self, code: u16, shift: bool, caps_lock: bool) -> Option<char>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct UsKeyLayout;

impl KeyLayout for UsKeyLayout {
    fn character(&self, code: u16, shift: bool, caps_lock: bool) -> Option<char> {
        let mapped = if (16..=25).contains(&code) {
            char::from(
                *b"qwertyuiop"
                    .get((code - 16) as usize)
                    .expect("validated keyboard code"),
            )
        } else if (30..=38).contains(&code) {
            char::from(
                *b"asdfghjkl"
                    .get((code - 30) as usize)
                    .expect("validated keyboard code"),
            )
        } else if (44..=50).contains(&code) {
            char::from(
                *b"zxcvbnm"
                    .get((code - 44) as usize)
                    .expect("validated keyboard code"),
            )
        } else {
            return punctuation(code, shift);
        };

        let uppercase = shift ^ caps_lock;
        Some(if uppercase {
            mapped.to_ascii_uppercase()
        } else {
            mapped
        })
    }
}

fn punctuation(code: u16, shift: bool) -> Option<char> {
    if (2..=11).contains(&code) {
        let chars = if shift { b"!@#$%^&*()" } else { b"1234567890" };
        return Some(char::from(chars[(code - 2) as usize]));
    }

    Some(match code {
        12 => {
            if shift {
                '_'
            } else {
                '-'
            }
        }
        13 => {
            if shift {
                '+'
            } else {
                '='
            }
        }
        26 => {
            if shift {
                '{'
            } else {
                '['
            }
        }
        27 => {
            if shift {
                '}'
            } else {
                ']'
            }
        }
        39 => {
            if shift {
                ':'
            } else {
                ';'
            }
        }
        40 => {
            if shift {
                '"'
            } else {
                '\''
            }
        }
        43 => {
            if shift {
                '|'
            } else {
                '\\'
            }
        }
        51 => {
            if shift {
                '<'
            } else {
                ','
            }
        }
        52 => {
            if shift {
                '>'
            } else {
                '.'
            }
        }
        53 => {
            if shift {
                '?'
            } else {
                '/'
            }
        }
        57 => ' ',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{KeyLayout, KeyboardState, UsKeyLayout};

    #[test]
    fn us_layout_maps_shift_and_caps_independently() {
        let layout = UsKeyLayout;
        assert_eq!(layout.character(30, false, false), Some('a'));
        assert_eq!(layout.character(30, true, false), Some('A'));
        assert_eq!(layout.character(30, false, true), Some('A'));
        assert_eq!(layout.character(30, true, true), Some('a'));
    }

    #[test]
    fn keyboard_state_toggles_caps_only_on_press() {
        let mut state = KeyboardState::default();
        let layout = UsKeyLayout;
        assert_eq!(state.character(&layout, 30), Some('a'));
        assert!(state.update_modifiers(true, 58));
        assert_eq!(state.character(&layout, 30), Some('A'));
        assert!(!state.update_modifiers(false, 58));
        assert!(state.update_modifiers(true, 58));
        assert_eq!(state.character(&layout, 30), Some('a'));
    }

    #[test]
    fn keyboard_state_tracks_shift_press_and_release() {
        let mut state = KeyboardState::default();
        assert!(state.update_modifiers(true, 42));
        let layout = UsKeyLayout;
        assert_eq!(state.character(&layout, 30), Some('A'));
        assert!(state.update_modifiers(false, 42));
        assert_eq!(state.character(&layout, 30), Some('a'));
    }
}
