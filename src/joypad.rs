use std::collections::HashMap;

/// Game Boy joypad state
pub struct Joypad {
    /// Current button states (true = pressed)
    buttons: HashMap<JoypadButton, bool>,

    /// Joypad register (P1/FF00)
    /// Bit 5: Select Button Keys (0=Select)
    /// Bit 4: Select Direction Keys (0=Select)
    /// Bit 3: Down or Start
    /// Bit 2: Up or Select
    /// Bit 1: Left or B
    /// Bit 0: Right or A
    pub register: u8,

    /// Interrupt flag - set when button pressed
    pub interrupt_requested: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JoypadButton {
    Right,
    Left,
    Up,
    Down,
    A,
    B,
    Select,
    Start,
}

impl Default for Joypad {
    fn default() -> Self {
        Self::new()
    }
}

impl Joypad {
    pub fn new() -> Self {
        let mut buttons = HashMap::new();
        buttons.insert(JoypadButton::Right, false);
        buttons.insert(JoypadButton::Left, false);
        buttons.insert(JoypadButton::Up, false);
        buttons.insert(JoypadButton::Down, false);
        buttons.insert(JoypadButton::A, false);
        buttons.insert(JoypadButton::B, false);
        buttons.insert(JoypadButton::Select, false);
        buttons.insert(JoypadButton::Start, false);

        Self {
            buttons,
            register: 0xCF, // All buttons released, both groups selected
            interrupt_requested: false,
        }
    }

    /// Press a button
    pub fn press(&mut self, button: JoypadButton) {
        if let Some(state) = self.buttons.get_mut(&button) {
            if !*state {
                *state = true;
                // Only trigger interrupt if the button's group is currently selected
                let is_direction = matches!(
                    button,
                    JoypadButton::Up
                        | JoypadButton::Down
                        | JoypadButton::Left
                        | JoypadButton::Right
                );
                let is_button = !is_direction;
                let select_directions = (self.register & 0x10) == 0;
                let select_buttons = (self.register & 0x20) == 0;

                if (is_direction && select_directions) || (is_button && select_buttons) {
                    self.interrupt_requested = true;
                }
            }
        }
        self.update_register();
    }

    /// Release a button
    pub fn release(&mut self, button: JoypadButton) {
        if let Some(state) = self.buttons.get_mut(&button) {
            *state = false;
        }
        self.update_register();
    }

    /// Release a button
    pub fn release_button(&mut self, button: JoypadButton) {
        if let Some(state) = self.buttons.get_mut(&button) {
            *state = false;
        }
        self.update_register();
    }

    /// Alias for press method
    pub fn press_button(&mut self, button: JoypadButton) {
        self.press(button);
    }

    /// Check if a button is pressed
    pub fn is_pressed(&self, button: JoypadButton) -> bool {
        *self.buttons.get(&button).unwrap_or(&false)
    }

    /// Update the joypad register based on current button states
    fn update_register(&mut self) {
        let select_buttons = (self.register & 0x20) == 0;
        let select_directions = (self.register & 0x10) == 0;

        // Start with upper bits always set (bits 6-7 are always 1 on DMG)
        let mut value = (self.register & 0xF0) | 0xC0;

        // If neither group is selected, all bits are 1
        if !select_buttons && !select_directions {
            value |= 0x0F;
        } else {
            // Start with all bits set (buttons not pressed)
            let mut lower = 0x0F;

            if select_buttons {
                // Button keys: Start, Select, B, A
                if self.is_pressed(JoypadButton::Start) {
                    lower &= !0x08;
                }
                if self.is_pressed(JoypadButton::Select) {
                    lower &= !0x04;
                }
                if self.is_pressed(JoypadButton::B) {
                    lower &= !0x02;
                }
                if self.is_pressed(JoypadButton::A) {
                    lower &= !0x01;
                }
            }

            if select_directions {
                // Direction keys: Down, Up, Left, Right
                if self.is_pressed(JoypadButton::Down) {
                    lower &= !0x08;
                }
                if self.is_pressed(JoypadButton::Up) {
                    lower &= !0x04;
                }
                if self.is_pressed(JoypadButton::Left) {
                    lower &= !0x02;
                }
                if self.is_pressed(JoypadButton::Right) {
                    lower &= !0x01;
                }
            }

            value |= lower;
        }

        // Ensure bits 6-7 are always 1 (DMG hardware behavior)
        self.register = value | 0xC0;
    }

    /// Read the joypad register
    pub fn read(&self) -> u8 {
        self.register
    }

    /// Write to the joypad register (select which button group to read)
    pub fn write(&mut self, value: u8) {
        // Only bits 4 and 5 are writable from the value
        // Preserve bits 6-7 (typically 1 on DMG), bits 0-3 will be computed by update_register
        self.register = (self.register & 0xC0) | (value & 0x30);
        self.update_register();
    }

    /// Set the raw joypad register (used during post-boot init to apply IO_RESET)
    pub fn set_register_raw(&mut self, value: u8) {
        self.register = value;
        self.update_register();
    }

    /// Clear the interrupt flag
    pub fn clear_interrupt(&mut self) {
        self.interrupt_requested = false;
    }

    /// Get all currently pressed buttons
    pub fn get_pressed_buttons(&self) -> Vec<JoypadButton> {
        self.buttons
            .iter()
            .filter(|(_, &pressed)| pressed)
            .map(|(&button, _)| button)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_joypad_initial_state() {
        let joypad = Joypad::new();
        assert_eq!(joypad.read(), 0xCF);
        assert!(!joypad.interrupt_requested);
    }

    #[test]
    fn test_press_release_button() {
        let mut joypad = Joypad::new();

        assert!(!joypad.is_pressed(JoypadButton::A));
        joypad.press(JoypadButton::A);
        assert!(joypad.is_pressed(JoypadButton::A));
        assert!(joypad.interrupt_requested);

        joypad.release(JoypadButton::A);
        assert!(!joypad.is_pressed(JoypadButton::A));
    }

    #[test]
    fn test_direction_keys() {
        let mut joypad = Joypad::new();

        // Select direction keys
        joypad.write(0x10);
        assert_eq!(joypad.read(), 0x1F); // All released

        joypad.press(JoypadButton::Right);
        assert_eq!(joypad.read(), 0x1E); // Right pressed (bit 0 = 0)

        joypad.press(JoypadButton::Left);
        assert_eq!(joypad.read(), 0x1C); // Right + Left pressed

        joypad.press(JoypadButton::Up);
        assert_eq!(joypad.read(), 0x18); // Right + Left + Up pressed

        joypad.press(JoypadButton::Down);
        assert_eq!(joypad.read(), 0x10); // All directions pressed
    }

    #[test]
    fn test_button_keys() {
        let mut joypad = Joypad::new();

        // Select button keys
        joypad.write(0x20);
        assert_eq!(joypad.read(), 0x2F); // All released

        joypad.press(JoypadButton::A);
        assert_eq!(joypad.read(), 0x2E); // A pressed (bit 0 = 0)

        joypad.press(JoypadButton::B);
        assert_eq!(joypad.read(), 0x2C); // A + B pressed

        joypad.press(JoypadButton::Select);
        assert_eq!(joypad.read(), 0x28); // A + B + Select pressed

        joypad.press(JoypadButton::Start);
        assert_eq!(joypad.read(), 0x20); // All buttons pressed
    }

    #[test]
    fn test_get_pressed_buttons() {
        let mut joypad = Joypad::new();

        assert_eq!(joypad.get_pressed_buttons().len(), 0);

        joypad.press(JoypadButton::A);
        joypad.press(JoypadButton::Start);

        let pressed = joypad.get_pressed_buttons();
        assert_eq!(pressed.len(), 2);
        assert!(pressed.contains(&JoypadButton::A));
        assert!(pressed.contains(&JoypadButton::Start));
    }
}
