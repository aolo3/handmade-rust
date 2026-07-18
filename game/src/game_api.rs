use std::os::raw::c_void;
pub const GAME_DLL: &str = env!("CARGO_PKG_NAME");

pub const BITMAP_BYTES_PER_PIXEL: u32 = 4;

#[derive(Default)]
pub struct GameMemory {
    pub permanent_storage_size: u64,
    pub transient_storage_size: u64,
    pub permanent_storage: *mut c_void,
    pub transient_storage: *mut c_void,
    pub is_initialized: bool,
}

#[derive(Copy, Clone, Default)]
pub struct ButtonState {
    pub is_down: bool,
    pub half_transition_count: u32,
}

pub enum ControllerButton {
    Up,
    Right,
    Down,
    Left,
    LeftShoulder,
    RightShoulder,
    South,
    West,
    North,
    East,
    Start,
    Count,
}

#[derive(Default)]
pub struct ControllerInput {
    pub buttons: [ButtonState; ControllerButton::Count as usize],
    pub left_stick_x: f32,
    pub left_stick_y: f32,
    pub right_stick_x: f32,
    pub right_stick_y: f32,

    pub is_analog: bool,
    pub is_connected: bool,
}

impl ControllerInput {
    pub fn get_button(&mut self, button_name: ControllerButton) -> &mut ButtonState {
        let idx = button_name as usize;
        assert!(idx < self.buttons.len());
        &mut self.buttons[idx]
    }
}

pub enum MouseButton {
    Left,
    Right,
    Middle,
    Count,
}

#[derive(Default)]
pub struct Input {
    pub mouse_buttons: [ButtonState; MouseButton::Count as usize],
    pub controllers: [ControllerInput; 5], // 4 controllers + 1 keyboard

    pub executable_reloaded: bool,
    pub delta_time: f32,

    pub mouse_x: i32,
    pub mouse_y: i32,
    pub mouse_wheel: i32,
}

impl Input {
    pub fn get_mouse_button(&mut self, button_name: MouseButton) -> &mut ButtonState {
        let idx = button_name as usize;
        assert!(idx < self.mouse_buttons.len());
        &mut self.mouse_buttons[idx]
    }

    pub fn get_controller_input(&mut self, controller_index: usize) -> &mut ControllerInput {
        assert!(controller_index < self.controllers.len());
        &mut self.controllers[controller_index]
    }
}

#[repr(C)]
pub struct OffscreenBuffer {
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub pixels: *mut u32,
}
