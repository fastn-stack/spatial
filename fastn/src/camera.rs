//! Camera Controller
//!
//! Default camera controller that handles keyboard, mouse, and gamepad input
//! to move the camera around in the scene.

use crate::protocol::*;
use std::collections::HashSet;

/// Default camera settings
const DEFAULT_CAMERA_POSITION: [f32; 3] = [0.0, 1.6, 3.0];
const DEFAULT_CAMERA_YAW: f32 = -std::f32::consts::FRAC_PI_2; // Facing -Z (towards origin)
const DEFAULT_CAMERA_PITCH: f32 = -0.5; // Looking slightly down

/// Camera movement speeds
const MOVE_SPEED: f32 = 2.0;        // Units per second
const MOVE_SPEED_SLOW: f32 = 0.2;   // Units per second (with shift)
const ROTATE_SPEED: f32 = 0.15;     // Radians per second (fine-grained for keyboard)

/// Camera controller that processes input events and produces camera commands
pub struct CameraController {
    /// Camera position in world space
    pub position: [f32; 3],
    /// Yaw angle (rotation around Y axis)
    pub yaw: f32,
    /// Pitch angle (rotation around X axis, clamped)
    pub pitch: f32,
    /// Currently pressed keys (by key code string)
    pressed_keys: HashSet<String>,
    /// Whether camera state has changed and needs to emit a command
    dirty: bool,
}

impl Default for CameraController {
    fn default() -> Self {
        Self::new()
    }
}

impl CameraController {
    pub fn new() -> Self {
        Self {
            position: DEFAULT_CAMERA_POSITION,
            yaw: DEFAULT_CAMERA_YAW,
            pitch: DEFAULT_CAMERA_PITCH,
            pressed_keys: HashSet::new(),
            dirty: true, // Emit initial camera state
        }
    }

    /// Process an input event and return any resulting commands
    pub fn handle_event(&mut self, event: &Event) -> Vec<Command> {
        match event {
            Event::Input(input_event) => self.handle_input(input_event),
            Event::Lifecycle(LifecycleEvent::Frame(frame)) => self.handle_frame(frame.dt),
            _ => vec![],
        }
    }

    fn handle_input(&mut self, event: &InputEvent) -> Vec<Command> {
        match event {
            InputEvent::Keyboard(kb_event) => self.handle_keyboard(kb_event),
            InputEvent::Mouse(mouse_event) => self.handle_mouse(mouse_event),
            InputEvent::Gamepad(gamepad_event) => self.handle_gamepad(gamepad_event),
            _ => vec![],
        }
    }

    fn handle_keyboard(&mut self, event: &KeyboardEvent) -> Vec<Command> {
        match event {
            KeyboardEvent::KeyDown(data) => {
                self.pressed_keys.insert(data.code.clone());

                // Handle reset on '0' key
                if data.code == "Digit0" {
                    self.reset();
                    self.dirty = true;
                }
            }
            KeyboardEvent::KeyUp(data) => {
                self.pressed_keys.remove(&data.code);
            }
            _ => {}
        }
        vec![]
    }

    fn handle_mouse(&mut self, _event: &MouseEvent) -> Vec<Command> {
        // TODO: Implement mouse look when dragging
        vec![]
    }

    fn handle_gamepad(&mut self, _event: &GamepadEvent) -> Vec<Command> {
        // TODO: Implement gamepad camera control
        vec![]
    }

    fn handle_frame(&mut self, dt: f32) -> Vec<Command> {
        // Process held keys for movement
        let mut dx = 0.0f32;
        let mut dz = 0.0f32;
        let mut dy = 0.0f32;
        let mut dyaw = 0.0f32;
        let mut dpitch = 0.0f32;

        // Check for shift modifier (slow movement)
        let shift_held = self.pressed_keys.contains("ShiftLeft")
            || self.pressed_keys.contains("ShiftRight");

        // Movement: WASD + QE + Arrow keys
        // Arrow keys: Shift+Up/Down = fly up/down at normal speed
        let arrow_up = self.pressed_keys.contains("ArrowUp");
        let arrow_down = self.pressed_keys.contains("ArrowDown");

        if self.pressed_keys.contains("KeyW") || (arrow_up && !shift_held) {
            dz -= 1.0; // Forward
        }
        if self.pressed_keys.contains("KeyS") || (arrow_down && !shift_held) {
            dz += 1.0; // Backward
        }
        if self.pressed_keys.contains("KeyA") || self.pressed_keys.contains("ArrowLeft") {
            dx -= 1.0; // Left
        }
        if self.pressed_keys.contains("KeyD") || self.pressed_keys.contains("ArrowRight") {
            dx += 1.0; // Right
        }
        if self.pressed_keys.contains("KeyQ") || (arrow_down && shift_held) {
            dy -= 1.0; // Down
        }
        if self.pressed_keys.contains("KeyE") || (arrow_up && shift_held) {
            dy += 1.0; // Up
        }

        // Rotation: IJKL
        if self.pressed_keys.contains("KeyJ") {
            dyaw -= 1.0; // Turn left
        }
        if self.pressed_keys.contains("KeyL") {
            dyaw += 1.0; // Turn right
        }
        if self.pressed_keys.contains("KeyI") {
            dpitch += 1.0; // Look up
        }
        if self.pressed_keys.contains("KeyK") {
            dpitch -= 1.0; // Look down
        }

        // Select move speed: slow only for Shift+WASD, not for arrow keys
        let using_arrows = arrow_up || arrow_down
            || self.pressed_keys.contains("ArrowLeft")
            || self.pressed_keys.contains("ArrowRight");
        let move_speed = if shift_held && !using_arrows { MOVE_SPEED_SLOW } else { MOVE_SPEED };

        // Apply movement in camera's local space
        if dx != 0.0 || dz != 0.0 {
            // Forward direction (in XZ plane)
            let forward_x = self.yaw.cos();
            let forward_z = self.yaw.sin();
            // Right direction
            let right_x = -self.yaw.sin();
            let right_z = self.yaw.cos();

            let move_amount = move_speed * dt;
            self.position[0] += (forward_x * dz + right_x * dx) * move_amount;
            self.position[2] += (forward_z * dz + right_z * dx) * move_amount;
            self.dirty = true;
        }

        // Apply vertical movement
        if dy != 0.0 {
            self.position[1] += dy * move_speed * dt;
            self.dirty = true;
        }

        // Apply rotation
        if dyaw != 0.0 || dpitch != 0.0 {
            self.yaw += dyaw * ROTATE_SPEED * dt;
            self.pitch += dpitch * ROTATE_SPEED * dt;
            // Clamp pitch to avoid gimbal lock
            self.pitch = self.pitch.clamp(-1.4, 1.4);
            self.dirty = true;
        }

        // Emit camera command if changed
        if self.dirty {
            self.dirty = false;
            vec![self.make_camera_command()]
        } else {
            vec![]
        }
    }

    /// Reset camera to default position and orientation
    pub fn reset(&mut self) {
        self.position = DEFAULT_CAMERA_POSITION;
        self.yaw = DEFAULT_CAMERA_YAW;
        self.pitch = DEFAULT_CAMERA_PITCH;
    }

    /// Calculate camera target from position, yaw, and pitch
    fn calculate_target(&self) -> [f32; 3] {
        let direction = [
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        ];
        [
            self.position[0] + direction[0],
            self.position[1] + direction[1],
            self.position[2] + direction[2],
        ]
    }

    fn make_camera_command(&self) -> Command {
        Command::Environment(EnvironmentCommand::SetCamera(CameraData {
            position: self.position,
            target: self.calculate_target(),
            up: [0.0, 1.0, 0.0],
            fov_degrees: 45.0,
            near: 0.1,
            far: 100.0,
        }))
    }
}
