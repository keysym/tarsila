use crate::ui_state::{UiEvent, UiState};
use crate::wrapped_image::WrappedImage;
use crate::{Bitmap, Event};
use macroquad::prelude::*;
use std::collections::HashMap;
use std::time::SystemTime;

const KEYDOWN_INTERVAL: u128 = 100;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum Shortcut {
    KeyPress(KeyCode),
    KeyPressMod(Modifier, KeyCode),
    KeyDown(KeyCode),
    KeyDownMod(Modifier, KeyCode),
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum Modifier {
    Ctrl,
    Shift,
}

#[derive(Debug, Clone)]
pub enum Effect {
    Event(Event<WrappedImage>),
    UiEvent(UiEvent),
}

pub struct KeyboardManager {
    shortcuts: HashMap<Shortcut, Effect>,
    last_keydown: Option<SystemTime>,
}

impl KeyboardManager {
    pub fn new() -> Self {
        Self {
            shortcuts: HashMap::new(),
            last_keydown: None,
        }
    }

    pub fn register_keydown(&mut self) {
        self.last_keydown = Some(SystemTime::now());
    }

    pub fn allow_keydown(&self) -> bool {
        match self.last_keydown {
            Some(time) => time.elapsed().unwrap().as_millis() > KEYDOWN_INTERVAL,
            None => true,
        }
    }

    pub fn register(&mut self, shortcut: Shortcut, effect: Effect) {
        self.shortcuts.insert(shortcut, effect);
    }

    pub fn register_keypress_event(&mut self, key: KeyCode, event: Event<WrappedImage>) {
        self.shortcuts
            .insert(Shortcut::KeyPress(key), Effect::Event(event));
    }

    pub fn register_keydown_event(&mut self, key: KeyCode, event: Event<WrappedImage>) {
        self.shortcuts
            .insert(Shortcut::KeyDown(key), Effect::Event(event));
    }

    pub fn register_keypress_mod_event(
        &mut self,
        modifier: Modifier,
        key: KeyCode,
        event: Event<WrappedImage>,
    ) {
        self.shortcuts
            .insert(Shortcut::KeyPressMod(modifier, key), Effect::Event(event));
    }

    pub fn register_keydown_mod_event(
        &mut self,
        modifier: Modifier,
        key: KeyCode,
        event: Event<WrappedImage>,
    ) {
        self.shortcuts
            .insert(Shortcut::KeyDownMod(modifier, key), Effect::Event(event));
    }

    pub fn register_keypress_ui_event(&mut self, key: KeyCode, event: UiEvent) {
        self.shortcuts
            .insert(Shortcut::KeyPress(key), Effect::UiEvent(event));
    }

    pub fn register_keydown_ui_event(&mut self, key: KeyCode, event: UiEvent) {
        self.shortcuts
            .insert(Shortcut::KeyDown(key), Effect::UiEvent(event));
    }

    pub fn process(&mut self) -> Vec<Effect> {
        let mut fx = Vec::new();

        for (shortcut, effect) in &self.shortcuts {
            let execute = match shortcut {
                Shortcut::KeyPress(key) => is_key_pressed(*key),
                Shortcut::KeyDown(key) => {
                    is_key_down(*key) && self.allow_keydown()
                }
                Shortcut::KeyPressMod(modif, key) => match modif {
                    Modifier::Ctrl => {
                        (is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl))
                            && is_key_pressed(*key)
                    }
                    Modifier::Shift => {
                        (is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift))
                            && is_key_pressed(*key)
                    }
                },
                Shortcut::KeyDownMod(modif, key) => match modif {
                    Modifier::Ctrl => {
                        (is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl))
                            && is_key_down(*key)
                            && self.allow_keydown()
                    }
                    Modifier::Shift => {
                        (is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift))
                            && is_key_down(*key)
                            && self.allow_keydown()
                    }
                },
                _ => false,
            };

            if execute {
                fx.push(effect.clone());
                break;
            }
        }

        if !fx.is_empty() {
            self.register_keydown();
        }

        fx
    }
}