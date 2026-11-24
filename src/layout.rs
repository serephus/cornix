use crate::config::KEY_COUNT;
use crate::keymap::KEYMAP;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Trans,
    Key(u8),
    Mod(u8),
    Momentary(u8),
    Boot,
    Reboot,
    Clear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: u8,
    pub pressed: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HidReport {
    pub modifiers: u8,
    pub keys: [u8; 6],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PressResult {
    None,
    Report,
    Boot,
    Reboot,
    Clear,
}

pub struct LayerState {
    active: u8,
    down: [bool; KEY_COUNT],
    held: [Action; KEY_COUNT],
    report: HidReport,
}

impl Default for LayerState {
    fn default() -> Self {
        Self::new()
    }
}

impl LayerState {
    pub fn new() -> Self {
        Self {
            active: 1,
            down: [false; KEY_COUNT],
            held: [Action::None; KEY_COUNT],
            report: HidReport::default(),
        }
    }

    pub fn report(&self) -> HidReport {
        self.report
    }

    fn resolve(&self, key: usize) -> Action {
        let mut layer = crate::config::LAYERS;
        while layer > 0 {
            layer -= 1;
            if self.active & (1 << layer) != 0 {
                let action = KEYMAP[layer][key];
                if action != Action::Trans {
                    return action;
                }
            }
        }
        Action::None
    }

    fn add_key(&mut self, key: u8) {
        if self.report.keys.contains(&key) {
            return;
        }
        if let Some(slot) = self.report.keys.iter_mut().find(|slot| **slot == 0) {
            *slot = key;
        }
    }

    fn remove_key(&mut self, key: u8) {
        if let Some(slot) = self.report.keys.iter_mut().find(|slot| **slot == key) {
            *slot = 0;
        }
    }

    pub fn press(&mut self, key: usize) -> PressResult {
        if key >= KEY_COUNT || self.down[key] {
            return PressResult::None;
        }
        self.down[key] = true;

        let action = self.resolve(key);
        self.held[key] = action;

        match action {
            Action::None | Action::Trans => PressResult::None,
            Action::Momentary(layer) => {
                if layer < 8 {
                    self.active |= 1 << layer;
                }
                PressResult::None
            }
            Action::Key(code) => {
                self.add_key(code);
                PressResult::Report
            }
            Action::Mod(bit) => {
                self.report.modifiers |= bit;
                PressResult::Report
            }
            Action::Boot => PressResult::Boot,
            Action::Reboot => PressResult::Reboot,
            Action::Clear => PressResult::Clear,
        }
    }

    pub fn release(&mut self, key: usize) -> bool {
        if key >= KEY_COUNT || !self.down[key] {
            return false;
        }
        self.down[key] = false;

        let action = core::mem::replace(&mut self.held[key], Action::None);
        match action {
            Action::Momentary(layer) => {
                if layer != 0 && layer < 8 {
                    self.active &= !(1 << layer);
                }
                false
            }
            Action::Key(code) => {
                self.remove_key(code);
                true
            }
            Action::Mod(bit) => {
                self.report.modifiers &= !bit;
                true
            }
            _ => false,
        }
    }
}
