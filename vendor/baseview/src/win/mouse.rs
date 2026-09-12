//! Button identity tracking for native capture cancellation. A counter cannot
//! recover from a missed release or a duplicate down notification.
use crate::MouseButton;

pub(super) const BUTTONS: [MouseButton; 5] = [
    MouseButton::Left, MouseButton::Middle, MouseButton::Right,
    MouseButton::Back, MouseButton::Forward,
];

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(super) struct Buttons(u8);

impl Buttons {
    fn bit(button: MouseButton) -> u8 {
        match button {
            MouseButton::Left => 1,
            MouseButton::Middle => 2,
            MouseButton::Right => 4,
            MouseButton::Back => 8,
            MouseButton::Forward => 16,
            _ => 0,
        }
    }

    pub fn contains(self, button: MouseButton) -> bool {
        self.0 & Self::bit(button) != 0
    }

    pub fn insert(&mut self, button: MouseButton) {
        self.0 |= Self::bit(button);
    }

    pub fn remove(&mut self, button: MouseButton) -> bool {
        let held = self.contains(button);
        self.0 &= !Self::bit(button);
        held
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn merge(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_down_does_not_leave_capture_stuck() {
        let mut buttons = Buttons::default();
        buttons.insert(MouseButton::Left);
        buttons.insert(MouseButton::Left);
        assert!(buttons.remove(MouseButton::Left));
        assert!(buttons.is_empty());
        assert!(!buttons.remove(MouseButton::Left));
    }

    #[test]
    fn releasing_one_button_preserves_the_other_drag() {
        let mut buttons = Buttons::default();
        for button in BUTTONS { buttons.insert(button); }
        assert!(buttons.remove(MouseButton::Right));
        assert!(buttons.contains(MouseButton::Left));
        assert!(!buttons.contains(MouseButton::Right));
        for button in BUTTONS { buttons.remove(button); }
        assert!(buttons.is_empty());
    }
}
