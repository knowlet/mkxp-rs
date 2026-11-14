/// RGSS button codes. The numeric values match `Input::ButtonCode` in mkxp
/// so a script port can keep using `Input::C` and friends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Button {
    None = 0,
    Down = 2,
    Left = 4,
    Right = 6,
    Up = 8,
    A = 11,
    B = 12,
    C = 13,
    X = 14,
    Y = 15,
    Z = 16,
    L = 17,
    R = 18,
    Shift = 21,
    Ctrl = 22,
    Alt = 23,
    F5 = 25,
    F6 = 26,
    F7 = 27,
    F8 = 28,
    F9 = 29,
    MouseLeft = 38,
    MouseMiddle = 39,
    MouseRight = 40,
    MouseX1 = 41,
    MouseX2 = 42,
}

impl Button {
    pub fn from_code(code: i32) -> Option<Self> {
        Some(match code {
            0 => Self::None,
            2 => Self::Down,
            4 => Self::Left,
            6 => Self::Right,
            8 => Self::Up,
            11 => Self::A,
            12 => Self::B,
            13 => Self::C,
            14 => Self::X,
            15 => Self::Y,
            16 => Self::Z,
            17 => Self::L,
            18 => Self::R,
            21 => Self::Shift,
            22 => Self::Ctrl,
            23 => Self::Alt,
            25 => Self::F5,
            26 => Self::F6,
            27 => Self::F7,
            28 => Self::F8,
            29 => Self::F9,
            38 => Self::MouseLeft,
            39 => Self::MouseMiddle,
            40 => Self::MouseRight,
            41 => Self::MouseX1,
            42 => Self::MouseX2,
            _ => return None,
        })
    }

    fn index(self) -> usize {
        self as u8 as usize
    }
}

const STATE_LEN: usize = 43;

#[derive(Clone, Copy, Debug, Default)]
struct ButtonState {
    pressed: bool,
    triggered: bool,
    repeated: bool,
    released: bool,
}

#[derive(Clone, Copy)]
struct Binding {
    key: &'static str,
    target: Button,
    repeatable: bool,
}

/// Keyboard and mouse state machine. The host reports physical keys with
/// `set_key` / `set_mouse`; scripts call `update` once per frame, then query
/// `is_pressed`, `is_triggered`, `is_repeated`, `dir4`, and `dir8`.
pub struct Input {
    keys: Vec<String>,
    mouse: [bool; 5],
    states: [ButtonState; STATE_LEN],
    states_old: [ButtonState; STATE_LEN],
    bindings: Vec<Binding>,
    repeating: Button,
    repeat_count: u32,
    repeat_start: u32,
    repeat_delay: u32,
    dir4: i32,
    dir4_previous: Button,
    dir8: i32,
    mouse_x: i32,
    mouse_y: i32,
}

impl Input {
    pub fn new(rgss_version: i32, fixed_framerate: i32) -> Self {
        // Fixed bindings from `staticKbBindings` are polled before the
        // rebindable defaults, matching `applyBindings`. Left Shift still
        // maps to A through the default table below.
        let mut bindings = vec![
            Binding {
                key: "lshift",
                target: Button::Shift,
                repeatable: true,
            },
            Binding {
                key: "rshift",
                target: Button::Shift,
                repeatable: true,
            },
            Binding {
                key: "lctrl",
                target: Button::Ctrl,
                repeatable: true,
            },
            Binding {
                key: "rctrl",
                target: Button::Ctrl,
                repeatable: true,
            },
            Binding {
                key: "lalt",
                target: Button::Alt,
                repeatable: true,
            },
            Binding {
                key: "ralt",
                target: Button::Alt,
                repeatable: true,
            },
            Binding {
                key: "f5",
                target: Button::F5,
                repeatable: true,
            },
            Binding {
                key: "f6",
                target: Button::F6,
                repeatable: true,
            },
            Binding {
                key: "f7",
                target: Button::F7,
                repeatable: true,
            },
            Binding {
                key: "f8",
                target: Button::F8,
                repeatable: true,
            },
            Binding {
                key: "f9",
                target: Button::F9,
                repeatable: true,
            },
            Binding {
                key: "left",
                target: Button::Left,
                repeatable: true,
            },
            Binding {
                key: "right",
                target: Button::Right,
                repeatable: true,
            },
            Binding {
                key: "up",
                target: Button::Up,
                repeatable: true,
            },
            Binding {
                key: "down",
                target: Button::Down,
                repeatable: true,
            },
            Binding {
                key: "space",
                target: Button::C,
                repeatable: true,
            },
            Binding {
                key: "return",
                target: Button::C,
                repeatable: true,
            },
            Binding {
                key: "escape",
                target: Button::B,
                repeatable: true,
            },
            Binding {
                key: "lshift",
                target: Button::A,
                repeatable: true,
            },
            Binding {
                key: "x",
                target: Button::B,
                repeatable: true,
            },
            Binding {
                key: "d",
                target: Button::Z,
                repeatable: true,
            },
            Binding {
                key: "q",
                target: Button::L,
                repeatable: true,
            },
            Binding {
                key: "w",
                target: Button::R,
                repeatable: true,
            },
            Binding {
                key: "a",
                target: Button::X,
                repeatable: true,
            },
            Binding {
                key: "s",
                target: Button::Y,
                repeatable: true,
            },
            Binding {
                key: "lctrl",
                target: Button::Ctrl,
                repeatable: true,
            },
            Binding {
                key: "lalt",
                target: Button::Alt,
                repeatable: true,
            },
            Binding {
                key: "mouse_left",
                target: Button::MouseLeft,
                repeatable: false,
            },
            Binding {
                key: "mouse_middle",
                target: Button::MouseMiddle,
                repeatable: false,
            },
            Binding {
                key: "mouse_right",
                target: Button::MouseRight,
                repeatable: false,
            },
            Binding {
                key: "mouse_x1",
                target: Button::MouseX1,
                repeatable: false,
            },
            Binding {
                key: "mouse_x2",
                target: Button::MouseX2,
                repeatable: false,
            },
        ];
        if rgss_version <= 1 {
            bindings.push(Binding {
                key: "z",
                target: Button::A,
                repeatable: true,
            });
            bindings.push(Binding {
                key: "c",
                target: Button::C,
                repeatable: true,
            });
        } else {
            bindings.push(Binding {
                key: "z",
                target: Button::C,
                repeatable: true,
            });
        }
        for (index, name) in ["f5", "f6", "f7", "f8", "f9"].iter().enumerate() {
            let target = Button::from_code(25 + index as i32).unwrap();
            bindings.push(Binding {
                key: name,
                target,
                repeatable: true,
            });
        }
        let fps = if fixed_framerate > 0 {
            fixed_framerate as u32
        } else if rgss_version >= 2 {
            60
        } else {
            40
        };
        let (repeat_start, repeat_delay) = repeat_timing(fps, rgss_version);
        Self {
            keys: Vec::new(),
            mouse: [false; 5],
            states: [ButtonState::default(); STATE_LEN],
            states_old: [ButtonState::default(); STATE_LEN],
            bindings,
            repeating: Button::None,
            repeat_count: 0,
            repeat_start,
            repeat_delay,
            dir4: 0,
            dir4_previous: Button::None,
            dir8: 0,
            mouse_x: 0,
            mouse_y: 0,
        }
    }

    pub fn set_key(&mut self, name: &str, down: bool) {
        let name = name.to_ascii_lowercase();
        if down {
            if !self.keys.iter().any(|key| key == &name) {
                self.keys.push(name);
            }
        } else {
            self.keys.retain(|key| key != &name);
        }
    }

    pub fn set_mouse_button(&mut self, button: i32, down: bool) {
        let index = match button {
            1 => 0,
            2 => 1,
            3 => 2,
            4 => 3,
            5 => 4,
            _ => return,
        };
        self.mouse[index] = down;
        let name = [
            "mouse_left",
            "mouse_middle",
            "mouse_right",
            "mouse_x1",
            "mouse_x2",
        ][index];
        self.set_key(name, down);
    }

    pub fn set_mouse_pos(&mut self, x: i32, y: i32) {
        self.mouse_x = x;
        self.mouse_y = y;
    }

    pub fn mouse_x(&self) -> i32 {
        self.mouse_x
    }
    pub fn mouse_y(&self) -> i32 {
        self.mouse_y
    }

    pub fn update(&mut self) {
        self.states_old = self.states;
        self.states = [ButtonState::default(); STATE_LEN];
        let mut repeat_cand = Button::None;
        let bindings = self.bindings.clone();
        for binding in &bindings {
            self.poll(binding, &mut repeat_cand);
        }
        let bindings = self.binding_targets();
        for target in bindings {
            let index = target.index();
            if !self.states[index].pressed && self.states_old[index].pressed {
                self.states[index].released = true;
            }
        }
        self.update_dir4();
        self.update_dir8();

        if repeat_cand != Button::None && repeat_cand != self.repeating {
            self.repeating = repeat_cand;
            self.repeat_count = 0;
            self.states[repeat_cand.index()].repeated = true;
            return;
        }
        if self.states[self.repeating.index()].pressed {
            self.repeat_count += 1;
            let repeated = self.repeat_count >= self.repeat_start
                && self.repeat_delay > 0
                && (self.repeat_count + 1) % self.repeat_delay == 0;
            if repeated {
                self.states[self.repeating.index()].repeated = true;
            }
            return;
        }
        self.repeating = Button::None;
    }

    pub fn is_pressed(&self, button: Button) -> bool {
        self.states[button.index()].pressed
    }
    pub fn is_triggered(&self, button: Button) -> bool {
        self.states[button.index()].triggered
    }
    pub fn is_repeated(&self, button: Button) -> bool {
        self.states[button.index()].repeated
    }
    pub fn is_released(&self, button: Button) -> bool {
        self.states[button.index()].released
    }

    pub fn count(&self, button: Button) -> u32 {
        if button == self.repeating {
            self.repeat_count
        } else {
            0
        }
    }

    pub fn dir4(&self) -> i32 {
        self.dir4
    }
    pub fn dir8(&self) -> i32 {
        self.dir8
    }

    fn key_down(&self, name: &str) -> bool {
        self.keys.iter().any(|key| key == name)
    }

    fn poll(&mut self, binding: &Binding, repeat_cand: &mut Button) {
        if !self.key_down(binding.key) || binding.target == Button::None {
            return;
        }
        let index = binding.target.index();
        let old = self.states_old[index];
        self.states[index].pressed = true;
        if !old.pressed {
            self.states[index].triggered = true;
        }
        if *repeat_cand != Button::None {
            return;
        }
        if self.repeating != binding.target && !old.pressed {
            if binding.repeatable {
                *repeat_cand = binding.target;
            } else {
                self.repeating = Button::None;
            }
        }
    }

    fn binding_targets(&self) -> Vec<Button> {
        let mut targets = Vec::new();
        for binding in &self.bindings {
            if !targets.contains(&binding.target) {
                targets.push(binding.target);
            }
        }
        targets
    }

    fn pressed(&self, button: Button) -> bool {
        self.states[button.index()].pressed
    }

    fn update_dir4(&mut self) {
        const DIRS: [Button; 4] = [Button::Down, Button::Left, Button::Right, Button::Up];
        const OTHER: [[Button; 3]; 4] = [
            [Button::Left, Button::Right, Button::Up],
            [Button::Down, Button::Up, Button::Right],
            [Button::Down, Button::Up, Button::Left],
            [Button::Left, Button::Right, Button::Down],
        ];
        let mut flag = 0i32;
        for (i, dir) in DIRS.iter().enumerate() {
            if self.pressed(*dir) {
                flag |= 1 << (*dir as i32);
            }
            let _ = i;
        }
        let dead_ud = (1 << Button::Down as i32) | (1 << Button::Up as i32);
        let dead_lr = (1 << Button::Left as i32) | (1 << Button::Right as i32);
        if flag == dead_ud || flag == dead_lr {
            self.dir4 = 0;
            return;
        }
        let previous = self.dir4_previous;
        if previous != Button::None && self.pressed(previous) {
            let row = (previous as i32 / 2 - 1) as usize;
            for other in OTHER[row] {
                if self.pressed(other) {
                    self.dir4 = other as i32;
                    return;
                }
            }
        }
        for dir in DIRS {
            if self.pressed(dir) {
                self.dir4 = dir as i32;
                self.dir4_previous = dir;
                return;
            }
        }
        self.dir4 = 0;
        self.dir4_previous = Button::None;
    }

    fn update_dir8(&mut self) {
        const DIRS: [Button; 4] = [Button::Down, Button::Left, Button::Right, Button::Up];
        const OTHER: [[Button; 3]; 4] = [
            [Button::Left, Button::Right, Button::Up],
            [Button::Down, Button::Up, Button::Right],
            [Button::Down, Button::Up, Button::Left],
            [Button::Left, Button::Right, Button::Down],
        ];
        const COMBOS: [[i32; 4]; 4] = [[2, 1, 3, 0], [1, 4, 0, 7], [3, 0, 6, 9], [0, 7, 9, 8]];
        self.dir8 = 0;
        for (i, one) in DIRS.iter().enumerate() {
            if !self.pressed(*one) {
                continue;
            }
            for other in OTHER[i] {
                if !self.pressed(other) {
                    continue;
                }
                let row = (*one as i32 / 2 - 1) as usize;
                let col = (other as i32 / 2 - 1) as usize;
                self.dir8 = COMBOS[row][col];
                return;
            }
            self.dir8 = *one as i32;
            return;
        }
    }
}

fn repeat_timing(fps: u32, rgss_version: i32) -> (u32, u32) {
    if fps == 0 {
        return (1, 1);
    }
    let frame_ms = 1.0 / fps as f64;
    let start = if rgss_version >= 2 { 0.375 } else { 0.400 };
    let delay = 0.100;
    let repeat_start = (start / frame_ms).ceil() as u32;
    let repeat_delay = (delay / frame_ms).ceil().max(1.0) as u32;
    (repeat_start.max(1), repeat_delay)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_repeat_and_directions_follow_mkxp() {
        let mut input = Input::new(1, 0);
        assert_eq!(input.repeat_start, 16);
        assert_eq!(input.repeat_delay, 4);

        input.set_key("down", true);
        input.update();
        assert!(input.is_pressed(Button::Down));
        assert!(input.is_triggered(Button::Down));
        assert!(input.is_repeated(Button::Down));
        assert_eq!(input.dir4(), 2);
        assert_eq!(input.dir8(), 2);

        input.set_key("up", true);
        input.update();
        assert_eq!(input.dir4(), 0);

        input.set_key("up", false);
        input.set_key("left", true);
        input.update();
        assert_eq!(input.dir4(), Button::Left as i32);
        assert_eq!(input.dir8(), 1);

        let mut vx = Input::new(3, 0);
        assert_eq!(vx.repeat_start, 23);
        assert_eq!(vx.repeat_delay, 6);
        vx.set_key("z", true);
        vx.update();
        assert!(vx.is_triggered(Button::C));
        assert!(!vx.is_triggered(Button::A));

        let mut modifiers = Input::new(1, 0);
        modifiers.set_key("lshift", true);
        modifiers.update();
        assert!(modifiers.is_pressed(Button::A));
        assert!(modifiers.is_pressed(Button::Shift));
        modifiers.set_key("lshift", false);
        modifiers.set_key("rshift", true);
        modifiers.set_key("rctrl", true);
        modifiers.set_key("ralt", true);
        modifiers.set_key("f5", true);
        modifiers.update();
        assert!(modifiers.is_pressed(Button::Shift));
        assert!(!modifiers.is_pressed(Button::A));
        assert!(modifiers.is_pressed(Button::Ctrl));
        assert!(modifiers.is_pressed(Button::Alt));
        assert!(modifiers.is_pressed(Button::F5));
        modifiers.set_mouse_button(4, true);
        modifiers.update();
        assert!(modifiers.is_pressed(Button::MouseX1));
    }

    #[test]
    fn repeat_fires_again_after_the_delay() {
        let mut input = Input::new(1, 40);
        input.set_key("c", true);
        input.update();
        assert!(input.is_repeated(Button::C));
        let mut hits = 0;
        for _ in 0..20 {
            input.update();
            if input.is_repeated(Button::C) {
                hits += 1;
            }
        }
        assert!(hits >= 1);
    }
}
