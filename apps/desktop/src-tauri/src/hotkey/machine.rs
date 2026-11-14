//! The shortcut state machine. Pure logic, no Windows calls, so it is fully unit-tested.
//!
//! Defaults:
//! - hold Ctrl+Win            push-to-talk
//! - double-tap Ctrl+Win      hands-free on
//! - Ctrl+Win+H               hands-free on / off (also locks a push-to-talk into hands-free)
//! - Esc while recording      cancel
//! - Alt+Shift+Z / Alt+Shift+X  paste / copy the last transcript
//! - Alt+Shift+C              Command Mode on (edit the selection by voice); again = done
//! - Alt+Shift+N              open the Scratchpad

pub const VK_ESCAPE: u32 = 0x1B;
pub const VK_H: u32 = 0x48;
pub const VK_X: u32 = 0x58;
pub const VK_Z: u32 = 0x5A;
pub const VK_F: u32 = 0x46;
pub const VK_C: u32 = 0x43;
pub const VK_N: u32 = 0x4E;

/// A press shorter than this is a "tap", not a dictation.
pub const TAP_MS: u32 = 250;
/// Two taps within this time switch hands-free on.
pub const DOUBLE_TAP_MS: u32 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    PttStart,
    PttStop,
    HandsFreeStart,
    /// A push-to-talk in progress becomes hands-free (keep recording).
    HandsFreeLock,
    HandsFreeStop,
    Cancel,
    PasteLast,
    CopyLast,
    FixLast,
    /// Alt+Shift+H: English + Hinglish (auto) -> English only -> Hinglish only.
    CycleLanguage,
    /// Alt+Shift+C: copy the selection and listen for an instruction.
    CommandStart,
    /// Alt+Shift+C again: apply the spoken instruction to the selection.
    CommandStop,
    /// Alt+Shift+N: open the Scratchpad.
    OpenScratchpad,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Decision {
    pub action: Option<Action>,
    /// Do not pass this key on to the focused app.
    pub swallow: bool,
    /// Send a harmless dummy key so releasing Win does not open the Start menu.
    pub mask_start_menu: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Idle,
    Ptt,
    HandsFree,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modifier {
    Ctrl,
    Win,
    Alt,
    Shift,
}

fn modifier(vk: u32) -> Option<Modifier> {
    match vk {
        0x11 | 0xA2 | 0xA3 => Some(Modifier::Ctrl),
        0x5B | 0x5C => Some(Modifier::Win),
        0x12 | 0xA4 | 0xA5 => Some(Modifier::Alt),
        0x10 | 0xA0 | 0xA1 => Some(Modifier::Shift),
        _ => None,
    }
}

#[derive(Debug)]
pub struct Machine {
    ctrl: bool,
    win: bool,
    alt: bool,
    shift: bool,
    mode: Mode,
    ptt_started: u32,
    last_tap: Option<u32>,
    /// After a cancel, ignore the combo until every modifier is released.
    blocked: bool,
    /// Non-modifier key whose key-down we swallowed; its key-up is swallowed too.
    swallowed: Option<u32>,
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}

impl Machine {
    pub fn new() -> Self {
        Self {
            ctrl: false,
            win: false,
            alt: false,
            shift: false,
            mode: Mode::Idle,
            ptt_started: 0,
            last_tap: None,
            blocked: false,
            swallowed: None,
        }
    }

    #[cfg(test)]
    pub fn is_recording(&self) -> bool {
        self.mode != Mode::Idle
    }

    /// Called by the controller when a recording ends for a reason the keyboard did not
    /// cause (the 20-minute cap, an error).
    pub fn force_idle(&mut self) {
        self.mode = Mode::Idle;
    }

    fn combo(&self) -> bool {
        self.ctrl && self.win && !self.alt && !self.shift
    }

    fn act(action: Action, swallow: bool, mask: bool) -> Decision {
        Decision {
            action: Some(action),
            swallow,
            mask_start_menu: mask,
        }
    }

    /// Feed one physical key event. `time_ms` is the event time (wraps like GetTickCount).
    pub fn on_key(&mut self, vk: u32, down: bool, time_ms: u32) -> Decision {
        if let Some(m) = modifier(vk) {
            let was_combo = self.combo();
            match m {
                Modifier::Ctrl => self.ctrl = down,
                Modifier::Win => self.win = down,
                Modifier::Alt => self.alt = down,
                Modifier::Shift => self.shift = down,
            }
            let combo = self.combo();
            let mut decision = Decision::default();

            match self.mode {
                Mode::Idle => {
                    if combo && !was_combo && down && !self.blocked {
                        let double = self
                            .last_tap
                            .is_some_and(|t| time_ms.wrapping_sub(t) <= DOUBLE_TAP_MS);
                        self.last_tap = None;
                        if double {
                            self.mode = Mode::HandsFree;
                            decision = Self::act(Action::HandsFreeStart, false, true);
                        } else {
                            self.mode = Mode::Ptt;
                            self.ptt_started = time_ms;
                            decision = Self::act(Action::PttStart, false, true);
                        }
                    }
                }
                Mode::Ptt => {
                    if !combo {
                        self.mode = Mode::Idle;
                        let held = time_ms.wrapping_sub(self.ptt_started);
                        if self.alt || self.shift {
                            // Another shortcut (for example Ctrl+Win+Shift+...) took over.
                            self.blocked = true;
                            decision = Self::act(Action::Cancel, false, false);
                        } else if held < TAP_MS {
                            self.last_tap = Some(time_ms);
                            decision = Self::act(Action::Cancel, false, false);
                        } else {
                            decision = Self::act(Action::PttStop, false, false);
                        }
                    }
                }
                Mode::Command => {}
                Mode::HandsFree => {
                    if combo && !was_combo && down {
                        self.mode = Mode::Idle;
                        self.blocked = true;
                        decision = Self::act(Action::HandsFreeStop, false, true);
                    }
                }
            }
            if !self.ctrl && !self.win && !self.alt && !self.shift {
                self.blocked = false;
            }
            return decision;
        }

        // Non-modifier key.
        if !down {
            if self.swallowed == Some(vk) {
                self.swallowed = None;
                return Decision {
                    swallow: true,
                    ..Decision::default()
                };
            }
            return Decision::default();
        }
        let swallow = |this: &mut Self, action: Action| {
            this.swallowed = Some(vk);
            Self::act(action, true, false)
        };

        match self.mode {
            Mode::Ptt => {
                if vk == VK_H && self.combo() {
                    self.mode = Mode::HandsFree;
                    swallow(self, Action::HandsFreeLock)
                } else if vk == VK_ESCAPE {
                    self.mode = Mode::Idle;
                    self.blocked = true;
                    swallow(self, Action::Cancel)
                } else {
                    // Ctrl+Win+<key> is some other shortcut; step aside and let it through.
                    self.mode = Mode::Idle;
                    self.blocked = true;
                    Self::act(Action::Cancel, false, false)
                }
            }
            Mode::Command => {
                if vk == VK_ESCAPE {
                    self.mode = Mode::Idle;
                    swallow(self, Action::Cancel)
                } else if self.alt && self.shift && !self.ctrl && !self.win && vk == VK_C {
                    self.mode = Mode::Idle;
                    swallow(self, Action::CommandStop)
                } else {
                    Decision::default()
                }
            }
            Mode::HandsFree => {
                if vk == VK_ESCAPE {
                    self.mode = Mode::Idle;
                    swallow(self, Action::Cancel)
                } else if vk == VK_H && self.combo() {
                    self.mode = Mode::Idle;
                    self.blocked = true;
                    swallow(self, Action::HandsFreeStop)
                } else {
                    Decision::default()
                }
            }
            Mode::Idle => {
                if vk == VK_H && self.combo() && !self.blocked {
                    self.mode = Mode::HandsFree;
                    self.blocked = true;
                    swallow(self, Action::HandsFreeStart)
                } else if self.alt && self.shift && !self.ctrl && !self.win && vk == VK_Z {
                    swallow(self, Action::PasteLast)
                } else if self.alt && self.shift && !self.ctrl && !self.win && vk == VK_X {
                    swallow(self, Action::CopyLast)
                } else if self.alt && self.shift && !self.ctrl && !self.win && vk == VK_F {
                    swallow(self, Action::FixLast)
                } else if self.alt && self.shift && !self.ctrl && !self.win && vk == VK_H {
                    swallow(self, Action::CycleLanguage)
                } else if self.alt && self.shift && !self.ctrl && !self.win && vk == VK_C {
                    self.mode = Mode::Command;
                    swallow(self, Action::CommandStart)
                } else if self.alt && self.shift && !self.ctrl && !self.win && vk == VK_N {
                    swallow(self, Action::OpenScratchpad)
                } else {
                    Decision::default()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL: u32 = 0xA2;
    const WIN: u32 = 0x5B;
    const ALT: u32 = 0xA4;
    const SHIFT: u32 = 0xA0;

    fn actions(m: &mut Machine, events: &[(u32, bool, u32)]) -> Vec<Action> {
        events
            .iter()
            .filter_map(|&(vk, down, t)| m.on_key(vk, down, t).action)
            .collect()
    }

    #[test]
    fn hold_and_release_is_push_to_talk() {
        let mut m = Machine::new();
        let d = m.on_key(CTRL, true, 0);
        assert_eq!(d.action, None);
        let d = m.on_key(WIN, true, 10);
        assert_eq!(d.action, Some(Action::PttStart));
        assert!(d.mask_start_menu, "must stop the Start menu from opening");
        assert!(!d.swallow);
        // Key repeat while held changes nothing.
        assert_eq!(m.on_key(WIN, true, 500).action, None);
        assert_eq!(m.on_key(CTRL, true, 600).action, None);
        // A long pause while holding does not end the dictation; only release does.
        assert_eq!(m.on_key(WIN, false, 60_000).action, Some(Action::PttStop));
        assert_eq!(m.on_key(CTRL, false, 60_010).action, None);
        assert!(!m.is_recording());
    }

    #[test]
    fn win_then_ctrl_also_works() {
        let mut m = Machine::new();
        assert_eq!(
            actions(
                &mut m,
                &[(WIN, true, 0), (CTRL, true, 5), (CTRL, false, 900)]
            ),
            [Action::PttStart, Action::PttStop]
        );
    }

    #[test]
    fn double_tap_starts_hands_free_and_combo_stops_it() {
        let mut m = Machine::new();
        let got = actions(
            &mut m,
            &[
                (CTRL, true, 0),
                (WIN, true, 5),
                (WIN, false, 100),
                (CTRL, false, 105), // tap
                (CTRL, true, 300),
                (WIN, true, 305),
                (WIN, false, 400),
                (CTRL, false, 405), // second tap
            ],
        );
        assert_eq!(
            got,
            [Action::PttStart, Action::Cancel, Action::HandsFreeStart]
        );
        assert!(m.is_recording());
        // Typing normal keys while hands-free is not interfered with.
        assert_eq!(m.on_key(0x41, true, 1000), Decision::default());
        // Pressing the combo again finishes.
        let got = actions(
            &mut m,
            &[
                (CTRL, true, 5000),
                (WIN, true, 5005),
                (WIN, false, 5100),
                (CTRL, false, 5105),
            ],
        );
        assert_eq!(got, [Action::HandsFreeStop]);
        assert!(!m.is_recording());
    }

    #[test]
    fn slow_second_tap_is_not_a_double_tap() {
        let mut m = Machine::new();
        let got = actions(
            &mut m,
            &[
                (CTRL, true, 0),
                (WIN, true, 5),
                (WIN, false, 100),
                (CTRL, false, 105),
                (CTRL, true, 2000),
                (WIN, true, 2005),
            ],
        );
        assert_eq!(got, [Action::PttStart, Action::Cancel, Action::PttStart]);
    }

    #[test]
    fn alt_shift_c_starts_and_finishes_command_mode() {
        let mut m = Machine::new();
        let got = actions(
            &mut m,
            &[
                (ALT, true, 0),
                (SHIFT, true, 10),
                (VK_C, true, 20),
                (VK_C, false, 30),
                (SHIFT, false, 40),
                (ALT, false, 50),
            ],
        );
        assert_eq!(got, vec![Action::CommandStart]);
        assert!(m.is_recording());
        // Ordinary typing and Ctrl+Win do not end it.
        assert!(actions(&mut m, &[(VK_H, true, 100), (VK_H, false, 110)]).is_empty());
        let got = actions(
            &mut m,
            &[(ALT, true, 200), (SHIFT, true, 210), (VK_C, true, 220)],
        );
        assert_eq!(got, vec![Action::CommandStop]);
        assert!(!m.is_recording());
        let mut m = Machine::new();
        actions(
            &mut m,
            &[
                (ALT, true, 0),
                (SHIFT, true, 10),
                (VK_C, true, 20),
                (VK_C, false, 30),
                (SHIFT, false, 40),
                (ALT, false, 50),
            ],
        );
        assert_eq!(
            actions(&mut m, &[(VK_ESCAPE, true, 90)]),
            vec![Action::Cancel]
        );
    }

    #[test]
    fn ctrl_win_h_toggles_hands_free() {
        let mut m = Machine::new();
        m.on_key(CTRL, true, 0);
        assert_eq!(m.on_key(WIN, true, 5).action, Some(Action::PttStart));
        let d = m.on_key(VK_H, true, 50);
        assert_eq!(d.action, Some(Action::HandsFreeLock));
        assert!(d.swallow, "the H must not be typed");
        assert!(m.on_key(VK_H, false, 80).swallow);
        // Releasing the modifiers keeps recording.
        assert_eq!(
            actions(&mut m, &[(WIN, false, 200), (CTRL, false, 210)]),
            []
        );
        assert!(m.is_recording());
        // Ctrl+Win+H again stops.
        let got = actions(&mut m, &[(CTRL, true, 9000), (WIN, true, 9005)]);
        assert_eq!(got, [Action::HandsFreeStop]);
    }

    #[test]
    fn escape_cancels_and_is_swallowed() {
        let mut m = Machine::new();
        m.on_key(CTRL, true, 0);
        m.on_key(WIN, true, 5);
        let d = m.on_key(VK_ESCAPE, true, 400);
        assert_eq!((d.action, d.swallow), (Some(Action::Cancel), true));
        // Still holding the keys must not restart recording.
        assert_eq!(m.on_key(WIN, true, 450).action, None);
        assert_eq!(
            actions(&mut m, &[(WIN, false, 500), (CTRL, false, 510)]),
            []
        );
        // After a full release it works again.
        assert_eq!(
            actions(&mut m, &[(CTRL, true, 900), (WIN, true, 905)]),
            [Action::PttStart]
        );
    }

    #[test]
    fn other_ctrl_win_shortcuts_pass_through() {
        let mut m = Machine::new();
        m.on_key(CTRL, true, 0);
        m.on_key(WIN, true, 5);
        // Ctrl+Win+Left (switch virtual desktop)
        let d = m.on_key(0x25, true, 60);
        assert_eq!((d.action, d.swallow), (Some(Action::Cancel), false));
        assert!(!m.is_recording());
    }

    #[test]
    fn ctrl_win_alt_is_reserved_and_does_not_dictate() {
        let mut m = Machine::new();
        assert_eq!(
            actions(&mut m, &[(CTRL, true, 0), (ALT, true, 2), (WIN, true, 5)]),
            []
        );
        let mut m = Machine::new();
        assert_eq!(
            actions(
                &mut m,
                &[(CTRL, true, 0), (WIN, true, 5), (SHIFT, true, 400)]
            ),
            [Action::PttStart, Action::Cancel]
        );
    }

    #[test]
    fn paste_last_and_copy_last() {
        let mut m = Machine::new();
        m.on_key(ALT, true, 0);
        m.on_key(SHIFT, true, 5);
        let d = m.on_key(VK_Z, true, 10);
        assert_eq!((d.action, d.swallow), (Some(Action::PasteLast), true));
        assert!(m.on_key(VK_Z, false, 20).swallow);
        assert_eq!(m.on_key(VK_X, true, 30).action, Some(Action::CopyLast));
        let d = m.on_key(VK_F, true, 40);
        assert_eq!((d.action, d.swallow), (Some(Action::FixLast), true));
        let d = m.on_key(VK_H, true, 50);
        assert_eq!((d.action, d.swallow), (Some(Action::CycleLanguage), true));
        // Plain Z is just a letter.
        let mut m = Machine::new();
        assert_eq!(m.on_key(VK_Z, true, 0), Decision::default());
    }
}
