//! Gamepad input handling using SDL2
//!
//! Provides gamepad state that can be polled each frame.
//! Supports hot-plug detection for connecting/disconnecting controllers.

use sdl2::controller::{GameController, Axis, Button};
use sdl2::GameControllerSubsystem;

/// Normalized gamepad state (values in -1.0 to 1.0 range for axes, bool for buttons)
#[derive(Debug, Clone, Default)]
pub struct GamepadState {
    pub connected: bool,

    // Sticks (normalized -1.0 to 1.0)
    pub left_stick_x: f32,
    pub left_stick_y: f32,
    pub right_stick_x: f32,
    pub right_stick_y: f32,

    // Triggers (normalized 0.0 to 1.0)
    pub left_trigger: f32,
    pub right_trigger: f32,

    // Face buttons
    pub button_a: bool,
    pub button_b: bool,
    pub button_x: bool,
    pub button_y: bool,

    // Shoulder buttons
    pub left_shoulder: bool,
    pub right_shoulder: bool,

    // Stick buttons
    pub left_stick_button: bool,
    pub right_stick_button: bool,

    // D-pad
    pub dpad_up: bool,
    pub dpad_down: bool,
    pub dpad_left: bool,
    pub dpad_right: bool,

    // Menu buttons
    pub start: bool,
    pub back: bool,
    pub guide: bool,
}

pub struct GamepadManager {
    controller_subsystem: GameControllerSubsystem,
    controller: Option<GameController>,
    state: GamepadState,
}

impl GamepadManager {
    pub fn new(sdl_context: &sdl2::Sdl) -> Result<Self, String> {
        let controller_subsystem = sdl_context.game_controller()?;

        // Try to open the first available controller
        let controller = Self::find_controller(&controller_subsystem);

        if let Some(ref c) = controller {
            log::info!("Gamepad connected: {}", c.name());
        } else {
            log::info!("No gamepad connected");
        }

        Ok(Self {
            controller_subsystem,
            controller,
            state: GamepadState::default(),
        })
    }

    fn find_controller(subsystem: &GameControllerSubsystem) -> Option<GameController> {
        let num_joysticks = subsystem.num_joysticks().ok()?;

        for i in 0..num_joysticks {
            if subsystem.is_game_controller(i) {
                if let Ok(controller) = subsystem.open(i) {
                    return Some(controller);
                }
            }
        }
        None
    }

    /// Try to connect a controller if none is currently connected
    /// Call this periodically (e.g., every frame) for hot-plug support
    pub fn try_connect(&mut self) {
        if self.controller.is_none() {
            if let Some(controller) = Self::find_controller(&self.controller_subsystem) {
                log::info!("Gamepad connected: {}", controller.name());
                self.controller = Some(controller);
            }
        }
    }

    /// Update gamepad state - call this each frame after pumping SDL events
    pub fn update(&mut self) {
        // Hot-plug: try to find a controller if we don't have one
        self.try_connect();

        if let Some(ref controller) = self.controller {
            if !controller.attached() {
                log::info!("Gamepad disconnected");
                self.controller = None;
                self.state = GamepadState::default();
                return;
            }

            self.state.connected = true;

            // Axes (SDL returns i16, normalize to -1.0..1.0)
            self.state.left_stick_x = normalize_axis(controller.axis(Axis::LeftX));
            self.state.left_stick_y = normalize_axis(controller.axis(Axis::LeftY));
            self.state.right_stick_x = normalize_axis(controller.axis(Axis::RightX));
            self.state.right_stick_y = normalize_axis(controller.axis(Axis::RightY));

            // Triggers (SDL returns 0..32767, normalize to 0.0..1.0)
            self.state.left_trigger = normalize_trigger(controller.axis(Axis::TriggerLeft));
            self.state.right_trigger = normalize_trigger(controller.axis(Axis::TriggerRight));

            // Face buttons
            self.state.button_a = controller.button(Button::A);
            self.state.button_b = controller.button(Button::B);
            self.state.button_x = controller.button(Button::X);
            self.state.button_y = controller.button(Button::Y);

            // Shoulder buttons
            self.state.left_shoulder = controller.button(Button::LeftShoulder);
            self.state.right_shoulder = controller.button(Button::RightShoulder);

            // Stick buttons
            self.state.left_stick_button = controller.button(Button::LeftStick);
            self.state.right_stick_button = controller.button(Button::RightStick);

            // D-pad
            self.state.dpad_up = controller.button(Button::DPadUp);
            self.state.dpad_down = controller.button(Button::DPadDown);
            self.state.dpad_left = controller.button(Button::DPadLeft);
            self.state.dpad_right = controller.button(Button::DPadRight);

            // Menu buttons
            self.state.start = controller.button(Button::Start);
            self.state.back = controller.button(Button::Back);
            self.state.guide = controller.button(Button::Guide);
        } else {
            self.state.connected = false;
        }
    }

    /// Get current gamepad state
    pub fn state(&self) -> &GamepadState {
        &self.state
    }

    /// Check if any input is active (for logging/debugging)
    pub fn has_input(&self) -> bool {
        if !self.state.connected {
            return false;
        }

        // Check if any stick is moved significantly
        let deadzone = 0.15;
        if self.state.left_stick_x.abs() > deadzone
            || self.state.left_stick_y.abs() > deadzone
            || self.state.right_stick_x.abs() > deadzone
            || self.state.right_stick_y.abs() > deadzone
        {
            return true;
        }

        // Check triggers
        if self.state.left_trigger > 0.1 || self.state.right_trigger > 0.1 {
            return true;
        }

        // Check any button
        self.state.button_a || self.state.button_b || self.state.button_x || self.state.button_y
            || self.state.left_shoulder || self.state.right_shoulder
            || self.state.dpad_up || self.state.dpad_down || self.state.dpad_left || self.state.dpad_right
            || self.state.start || self.state.back
    }
}

/// Normalize axis value from i16 (-32768..32767) to f32 (-1.0..1.0)
fn normalize_axis(value: i16) -> f32 {
    if value >= 0 {
        value as f32 / 32767.0
    } else {
        value as f32 / 32768.0
    }
}

/// Normalize trigger value from i16 (0..32767) to f32 (0.0..1.0)
fn normalize_trigger(value: i16) -> f32 {
    (value.max(0) as f32) / 32767.0
}
