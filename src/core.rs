use std::collections::HashSet;
use crate::config::{ActivationMode, Config, DragMode, Key};

pub trait PointerOutput {
    fn move_by(&mut self, x: i32, y: i32);
    fn button(&mut self, index: usize, down: bool);
    fn scroll(&mut self, notches: i32);
}

/// Platform listeners translate keys and obey the returned suppression decision.
pub trait InputListener {
    fn key(&mut self, key: Key, down: bool) -> bool;
}

pub struct Engine<P: PointerOutput> {
    pub config: Config,
    pub pointer: P,
    pub active: bool,
    pub enabled: bool,
    physical: HashSet<Key>,
    captured: HashSet<Key>,
    pressed: HashSet<Key>,
    buttons: [bool; 2],
    drag_locked: bool,
    speed: f64,
    idle_seconds: f64,
    remainder: [f64; 2],
    scroll_remainder: f64,
    direction: (i32, i32),
    scroll_direction: i32,
    debug: bool,
    pub logs: Vec<String>,
}

impl<P: PointerOutput> Engine<P> {
    pub fn new(config: Config, pointer: P, debug: bool) -> Self {
        Self { config, pointer, active: false, enabled: true, physical: HashSet::new(),
            captured: HashSet::new(), pressed: HashSet::new(), buttons: [false; 2], drag_locked: false,
            speed: 0.0, idle_seconds: 0.0, remainder: [0.0; 2], scroll_remainder: 0.0,
            direction: (0, 0), scroll_direction: 0, debug, logs: Vec::new() }
    }

    fn log(&mut self, message: String) {
        if self.debug { self.logs.push(message); }
    }

    pub fn seed_held(&mut self, key: Key) { self.physical.insert(key); }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.reset();
        self.enabled = enabled;
    }

    pub fn configure(&mut self, config: Config) -> Result<(), String> {
        config.validate()?;
        self.reset();
        self.config = config;
        Ok(())
    }

    pub fn reset(&mut self) {
        if self.active { self.log("mouse layer deactivated".into()); }
        self.active = false;
        self.drag_locked = false;
        self.pressed.clear();
        self.update();
        self.speed = 0.0;
        self.idle_seconds = 0.0;
        self.remainder = [0.0; 2];
        self.scroll_remainder = 0.0;
        // Keep captured keys until release: never leak a partial key sequence.
    }

    fn update(&mut self) {
        for i in 0..2 {
            let down = self.active && if i == 0 && self.config.drag_mode == DragMode::Toggle {
                self.drag_locked
            } else { self.pressed.contains(&self.config.clicks[i]) };
            if down != self.buttons[i] {
                self.pointer.button(i, down);
                self.buttons[i] = down;
                self.log(format!("button {i} {}", if down { "down" } else { "up" }));
            }
        }
        let held = |key| i32::from(self.active && self.pressed.contains(&key));
        let m = self.config.movement;
        let direction = (held(m[3]) - held(m[1]), held(m[2]) - held(m[0]));
        let scroll = held(self.config.scroll[0]) - held(self.config.scroll[1]);
        if direction != self.direction {
            self.direction = direction;
            if direction != (0, 0) {
                if self.speed == 0.0 || self.idle_seconds > self.config.direction_grace_ms / 1000.0 {
                    self.speed = self.config.base_speed;
                }
                self.idle_seconds = 0.0;
            }
            self.remainder = [0.0; 2];
            self.log(format!("movement direction {direction:?}"));
        }
        if scroll != self.scroll_direction {
            self.scroll_direction = scroll;
            self.scroll_remainder = 0.0;
            self.log(format!("scroll direction {scroll}"));
        }
    }

    pub fn tick(&mut self, seconds: f64) {
        if !self.active { return; }
        // A stalled loop must never replay a large movement after resuming.
        if !seconds.is_finite() || seconds > 0.25 { self.reset(); return; }
        let dt = seconds.clamp(0.0, 0.05);
        let (x, y) = self.direction;
        if x != 0 || y != 0 {
            let next = (self.speed + self.config.acceleration * dt).min(self.config.max_speed);
            let precision = self.config.precision.iter().any(|k| self.physical.contains(k));
            let distance = (self.speed + next) * 0.5 * dt
                * if precision { self.config.precision_multiplier } else { 1.0 };
            self.speed = next;
            let length = f64::from(x * x + y * y).sqrt();
            self.remainder[0] += f64::from(x) / length * distance;
            self.remainder[1] += f64::from(y) / length * distance;
            let dx = self.remainder[0].trunc() as i32;
            let dy = self.remainder[1].trunc() as i32;
            self.remainder[0] -= f64::from(dx);
            self.remainder[1] -= f64::from(dy);
            if dx != 0 || dy != 0 { self.pointer.move_by(dx, dy); }
        } else { self.idle_seconds += seconds.max(0.0); }
        self.tick_scroll(dt);
    }

    fn tick_scroll(&mut self, dt: f64) {
        self.scroll_remainder += f64::from(self.scroll_direction) * self.config.scroll_notches_per_second * dt;
        let notches = self.scroll_remainder.trunc() as i32;
        if notches != 0 {
            self.scroll_remainder -= f64::from(notches);
            self.pointer.scroll(notches);
            self.log(format!("scroll {notches}"));
        }
    }
}

impl<P: PointerOutput> InputListener for Engine<P> {
    fn key(&mut self, key: Key, down: bool) -> bool {
        let fresh = if down { self.physical.insert(key) } else { self.physical.remove(&key) };
        // Preserve ownership even if settings/enable state change while a key is held.
        if self.captured.contains(&key) && (!self.enabled || key != self.config.activation) {
            if !down { self.captured.remove(&key); self.pressed.remove(&key); }
            self.update();
            return true;
        }
        if key == self.config.activation && self.enabled {
            if down && fresh {
                self.captured.insert(key);
                match self.config.activation_mode {
                    ActivationMode::Hold => { self.active = true; self.log("mouse layer activated".into()); }
                    ActivationMode::Toggle => {
                        if self.active { self.reset(); self.log("mouse layer deactivated".into()); }
                        else { self.active = true; self.log("mouse layer activated".into()); }
                    }
                }
            } else if !down {
                self.captured.remove(&key);
                if self.config.activation_mode == ActivationMode::Hold { self.reset(); }
            }
            return true;
        }
        let mapped = self.config.movement.contains(&key) || self.config.clicks.contains(&key)
            || self.config.scroll.contains(&key) || self.config.precision.contains(&key);
        let mut suppress = self.captured.contains(&key);
        if down && fresh && self.active && mapped {
            self.captured.insert(key);
            if key == self.config.clicks[0] && self.config.drag_mode == DragMode::Toggle {
                self.drag_locked = !self.drag_locked;
                self.log(format!("drag {}", if self.drag_locked { "locked" } else { "unlocked" }));
            } else { self.pressed.insert(key); }
            suppress = true;
        } else if !down {
            self.captured.remove(&key);
            self.pressed.remove(&key);
        }
        self.update();
        suppress
    }
}

impl<P: PointerOutput> Drop for Engine<P> {
    fn drop(&mut self) { self.reset(); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Key::*;
    #[derive(Default)]
    struct Fake { x: i32, y: i32, buttons: Vec<(usize, bool)>, scroll: i32 }
    impl PointerOutput for Fake {
        fn move_by(&mut self, x: i32, y: i32) { self.x += x; self.y += y; }
        fn button(&mut self, index: usize, down: bool) { self.buttons.push((index, down)); }
        fn scroll(&mut self, notches: i32) { self.scroll += notches; }
    }
    fn engine() -> Engine<Fake> { Engine::new(Config::default(), Fake::default(), false) }
    fn advance(e: &mut Engine<Fake>, count: usize) { for _ in 0..count { e.tick(0.01); } }

    #[test]
    fn inactive_typing_passes_through() {
        let mut e = engine();
        for key in [Letter('W'), Enter, Space, LeftShift, Letter('Q')] {
            assert!(!e.key(key, true)); assert!(!e.key(key, false));
        }
        advance(&mut e, 100);
        assert_eq!((e.pointer.x, e.pointer.y, e.pointer.scroll), (0, 0, 0));
        assert!(e.pointer.buttons.is_empty());
    }

    #[test]
    fn activation_release_ends_drag_and_swallows_remaining_sequence() {
        let mut e = engine();
        e.key(CapsLock, true); e.key(Enter, true); e.key(Letter('D'), true);
        advance(&mut e, 10);
        assert!(e.pointer.x > 0);
        e.key(CapsLock, false);
        let x = e.pointer.x;
        assert!(e.key(Enter, true)); // auto-repeat after layer release
        assert!(e.key(Letter('D'), true));
        advance(&mut e, 100);
        assert_eq!(e.pointer.x, x);
        assert_eq!(e.pointer.buttons, [(0, true), (0, false)]);
        assert!(e.key(Enter, false)); assert!(e.key(Letter('D'), false));
        assert!(!e.key(Enter, true));
    }

    #[test]
    fn repeat_does_not_change_speed_or_click_count() {
        let mut a = engine(); let mut b = engine();
        for e in [&mut a, &mut b] { e.key(CapsLock, true); e.key(Letter('D'), true); e.key(Enter, true); }
        for _ in 0..100 {
            a.key(Letter('D'), true); a.key(Enter, true); a.key(CapsLock, true);
            a.tick(0.01); b.tick(0.01);
        }
        assert_eq!(a.pointer.x, b.pointer.x);
        assert_eq!(a.pointer.buttons, [(0, true)]);
    }

    #[test]
    fn diagonal_is_normalized_and_opposites_cancel() {
        let mut straight = engine(); let mut diagonal = engine();
        for e in [&mut straight, &mut diagonal] { e.key(CapsLock, true); e.key(Letter('D'), true); }
        diagonal.key(Letter('W'), true);
        advance(&mut straight, 100); advance(&mut diagonal, 100);
        let distance = f64::from(diagonal.pointer.x.pow(2) + diagonal.pointer.y.pow(2)).sqrt();
        assert!((distance - f64::from(straight.pointer.x)).abs() < 2.0);
        assert_eq!(diagonal.pointer.x, -diagonal.pointer.y);
        straight.key(Letter('A'), true);
        let x = straight.pointer.x; advance(&mut straight, 10);
        assert_eq!(straight.pointer.x, x);
    }

    #[test]
    fn acceleration_and_precision_are_time_based() {
        let mut e = engine(); e.key(CapsLock, true); e.key(Letter('D'), true);
        advance(&mut e, 10); let first = e.pointer.x;
        advance(&mut e, 10); assert!(e.pointer.x - first > first);
        let mut fast = engine(); let mut precise = engine();
        for e in [&mut fast, &mut precise] { e.key(CapsLock, true); e.key(Letter('D'), true); }
        precise.key(LeftShift, true); precise.key(RightShift, true); precise.key(LeftShift, false);
        advance(&mut fast, 100); advance(&mut precise, 100);
        assert!((f64::from(precise.pointer.x) / f64::from(fast.pointer.x) - 0.18).abs() < 0.01);
    }

    #[test]
    fn preheld_typing_and_modifier_release_are_preserved() {
        let mut e = engine();
        assert!(!e.key(Enter, true)); assert!(!e.key(LeftShift, true));
        e.key(CapsLock, true);
        assert!(!e.key(Enter, true)); assert!(!e.key(Enter, false));
        assert!(!e.key(LeftShift, false));
        assert!(e.pointer.buttons.is_empty());
        assert!(e.key(Enter, true));
        assert_eq!(e.pointer.buttons, [(0, true)]);
    }

    #[test]
    fn scrolling_cancels_and_stops_on_release() {
        let mut e = engine(); e.key(CapsLock, true); e.key(Letter('Q'), true);
        advance(&mut e, 100); assert!((5..=6).contains(&e.pointer.scroll));
        e.key(Letter('E'), true); let scroll = e.pointer.scroll;
        advance(&mut e, 100); assert_eq!(e.pointer.scroll, scroll);
        e.key(Letter('Q'), false); advance(&mut e, 100);
        assert!(e.pointer.scroll < scroll);
        e.key(CapsLock, false); let scroll = e.pointer.scroll;
        advance(&mut e, 100); assert_eq!(e.pointer.scroll, scroll);
    }

    #[test]
    fn stall_cancels_buttons_and_requires_fresh_activation() {
        let mut e = engine(); e.key(CapsLock, true); e.key(Enter, true); e.key(Space, true);
        e.tick(0.5); assert!(!e.active);
        assert_eq!(e.pointer.buttons, [(0, true), (1, true), (0, false), (1, false)]);
        e.key(CapsLock, true); assert!(!e.active);
        e.key(CapsLock, false); e.key(CapsLock, true); assert!(e.active);
    }

    #[test]
    fn drop_releases_synthetic_buttons() {
        struct Recorder(std::rc::Rc<std::cell::RefCell<Vec<bool>>>);
        impl PointerOutput for Recorder {
            fn move_by(&mut self, _: i32, _: i32) {}
            fn scroll(&mut self, _: i32) {}
            fn button(&mut self, _: usize, down: bool) { self.0.borrow_mut().push(down); }
        }
        let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        { let mut e = Engine::new(Config::default(), Recorder(events.clone()), false);
          e.key(CapsLock, true); e.key(Enter, true); }
        assert_eq!(*events.borrow(), [true, false]);
    }

    #[test]
    fn direction_changes_keep_speed_but_never_drift() {
        let mut e = engine(); e.key(CapsLock, true); e.key(Letter('D'), true);
        advance(&mut e, 100);
        let speed = e.speed;
        e.key(Letter('W'), true); assert_eq!(e.speed, speed);
        e.key(Letter('D'), false); assert_eq!(e.speed, speed);
        e.key(Letter('W'), false);
        let position = (e.pointer.x, e.pointer.y);
        advance(&mut e, 5);
        assert_eq!((e.pointer.x, e.pointer.y), position);
        e.key(Letter('A'), true); assert_eq!(e.speed, speed);
        e.key(Letter('A'), false); advance(&mut e, 12);
        e.key(Letter('D'), true); assert_eq!(e.speed, e.config.base_speed);
        e.key(CapsLock, false); e.key(CapsLock, true); e.key(Letter('W'), true);
        assert_eq!(e.speed, e.config.base_speed);
    }

    #[test]
    fn pause_and_remapping_release_buttons_and_preserve_key_ownership() {
        let mut e = engine(); e.key(CapsLock, true); e.key(Enter, true);
        e.set_enabled(false);
        assert_eq!(e.pointer.buttons, [(0, true), (0, false)]);
        assert!(e.key(CapsLock, false)); assert!(e.key(Enter, false));
        assert!(!e.key(CapsLock, true)); assert!(!e.key(CapsLock, false));
        e.set_enabled(true); e.key(CapsLock, true); e.key(Enter, true);
        let mut config = Config::default(); config.activation = Key::Function(8);
        e.configure(config).unwrap();
        assert!(e.key(CapsLock, true)); assert!(!e.active);
        assert!(e.key(CapsLock, false)); assert!(e.key(Enter, false));
        assert!(!e.key(CapsLock, true));
        assert!(e.key(Key::Function(8), true)); assert!(e.active);
    }

    #[test]
    fn toggle_activation_does_not_require_holding_the_activation_key() {
        let mut e = engine();
        e.config.activation_mode = ActivationMode::Toggle;
        e.key(CapsLock, true); e.key(CapsLock, false);
        assert!(e.active);
        e.key(Letter('D'), true); advance(&mut e, 20); e.key(Letter('D'), false);
        assert!(e.pointer.x > 0);
        e.key(CapsLock, true); e.key(CapsLock, false);
        assert!(!e.active);
        let x = e.pointer.x; advance(&mut e, 20); assert_eq!(e.pointer.x, x);
    }

    #[test]
    fn toggle_drag_keeps_left_button_down_until_second_press() {
        let mut e = engine();
        e.config.activation_mode = ActivationMode::Toggle;
        e.config.drag_mode = DragMode::Toggle;
        e.key(CapsLock, true); e.key(CapsLock, false);
        e.key(Enter, true); e.key(Enter, false);
        assert_eq!(e.pointer.buttons, [(0, true)]);
        e.key(Letter('D'), true); advance(&mut e, 20); e.key(Letter('D'), false);
        e.key(Enter, true); e.key(Enter, false);
        assert_eq!(e.pointer.buttons, [(0, true), (0, false)]);
    }
}
