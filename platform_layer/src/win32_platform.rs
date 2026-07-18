use game::GameUpdateAndRenderFn;
use game::game_api::{
    ButtonState, ControllerButton, ControllerInput, GAME_DLL, GameMemory, Input, MouseButton,
    OffscreenBuffer,
};
use handmade_macros::{gigabytes, megabytes};
use logger::{log_error, log_info, log_warning};
use std::os::raw::c_void;
use std::ptr;
use windows_sys::Win32::Foundation::{
    ERROR_SUCCESS, FALSE, FILETIME, GetLastError, HMODULE, HWND, MAX_PATH, POINT, RECT,
};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLACKNESS, BeginPaint, DIB_RGB_COLORS, EndPaint, GetDC,
    GetDeviceCaps, GetMonitorInfoW, HDC, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromWindow,
    PAINTSTRUCT, PatBlt, ReleaseDC, SRCCOPY, ScreenToClient, StretchDIBits, VREFRESH,
};
use windows_sys::Win32::Media::{TIMERR_NOERROR, timeBeginPeriod};
use windows_sys::Win32::Storage::FileSystem::{
    CopyFileW, GetFileAttributesExW, GetFileExInfoStandard, WIN32_FILE_ATTRIBUTE_DATA,
};
use windows_sys::Win32::System::Environment::GetCurrentDirectoryW;
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleFileNameW, GetModuleHandleW, GetProcAddress, LoadLibraryW,
};
use windows_sys::Win32::System::Memory::{MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE, VirtualAlloc};
use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows_sys::Win32::System::Threading::Sleep;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_A, VK_D, VK_DOWN, VK_F4, VK_F11, VK_LBUTTON, VK_LEFT, VK_MBUTTON, VK_RBUTTON,
    VK_RETURN, VK_RIGHT, VK_S, VK_SHIFT, VK_SPACE, VK_UP, VK_W,
};
use windows_sys::Win32::UI::Input::XboxController::{
    XINPUT_GAMEPAD_A, XINPUT_GAMEPAD_B, XINPUT_GAMEPAD_DPAD_DOWN, XINPUT_GAMEPAD_DPAD_LEFT,
    XINPUT_GAMEPAD_DPAD_RIGHT, XINPUT_GAMEPAD_DPAD_UP, XINPUT_GAMEPAD_LEFT_SHOULDER,
    XINPUT_GAMEPAD_LEFT_THUMB_DEADZONE, XINPUT_GAMEPAD_RIGHT_SHOULDER,
    XINPUT_GAMEPAD_RIGHT_THUMB_DEADZONE, XINPUT_GAMEPAD_START, XINPUT_GAMEPAD_X, XINPUT_GAMEPAD_Y,
    XINPUT_STATE, XInputGetState, XUSER_MAX_COUNT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW,
    DispatchMessageW, GWL_STYLE, GWLP_USERDATA, GetClientRect, GetCursorPos, GetWindowLongPtrW,
    GetWindowLongW, GetWindowPlacement, HWND_TOP, IDC_ARROW, LoadCursorW, MSG, PM_REMOVE,
    PeekMessageW, RegisterClassW, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE,
    SWP_NOZORDER, SetWindowLongPtrW, SetWindowPlacement, SetWindowPos, TranslateMessage,
    WINDOWPLACEMENT, WM_CLOSE, WM_KEYDOWN, WM_KEYUP, WM_NCCREATE, WM_PAINT, WM_QUIT, WM_SETCURSOR,
    WM_SIZE, WM_SYSKEYDOWN, WM_SYSKEYUP, WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};
use windows_sys::{s, w};

use crate::EXECUTABLE_NAME;

#[derive(Default)]
struct Win32OffscreenBuffer {
    info: BITMAPINFO,
    width: u32,
    height: u32,
    pitch: u32,
    pixels: Vec<u32>,
}

#[derive(Default)]
#[repr(C)]
struct Win32GameCode {
    game_code_dll: HMODULE,
    dll_last_write_time: FILETIME,

    game_update_and_render: Option<GameUpdateAndRenderFn>,

    is_valid: bool,
}

pub struct Win32Platform {
    window_position: WINDOWPLACEMENT,
    perf_count_frequency: i64,
    game_memory_block_total_size: u64,
    game_memory_block: *mut c_void,
    is_running: bool,
    offscreen_buffer: Win32OffscreenBuffer,
    exe_filename: [u16; MAX_PATH as usize],
    one_past_last_exe_file_name_slash: usize,
}

impl Default for Win32Platform {
    fn default() -> Self {
        Self {
            window_position: Default::default(),
            perf_count_frequency: Default::default(),
            game_memory_block_total_size: Default::default(),
            game_memory_block: Default::default(),
            is_running: Default::default(),
            offscreen_buffer: Default::default(),
            one_past_last_exe_file_name_slash: Default::default(),
            exe_filename: [0; MAX_PATH as usize],
        }
    }
}

impl Win32Platform {
    fn new() -> Self {
        Win32Platform::default()
    }

    fn get_wall_clock(&self) -> i64 {
        let mut result = 0;
        unsafe { QueryPerformanceCounter(&mut result) };
        result
    }

    fn get_seconds_elapsed(&self, start: i64, end: i64) -> f32 {
        return (end - start) as f32 / self.perf_count_frequency as f32;
    }

    fn get_platform_ptr(window: &HWND) -> Option<&mut Win32Platform> {
        unsafe {
            let platform_ptr = GetWindowLongPtrW(*window, GWLP_USERDATA) as *mut Win32Platform;
            if platform_ptr.is_null() {
                return None;
            }

            Some(&mut *platform_ptr)
        }
    }

    unsafe extern "system" fn main_window_callback(
        window: HWND,
        message: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize {
        unsafe {
            let mut result = 0;

            let Some(platform) = Win32Platform::get_platform_ptr(&window) else {
                return DefWindowProcW(window, message, wparam, lparam);
            };

            match message {
                WM_SIZE => {}
                WM_PAINT => {
                    let mut paint = PAINTSTRUCT::default();
                    let device_context = BeginPaint(window, &mut paint);
                    let (width, height) = platform.get_window_dimension(window);
                    // let x = paint.rcPaint.left;
                    // let y = paint.rcPaint.top;

                    platform.display_buffer_in_window(
                        device_context,
                        width,
                        height,
                        // x, y
                    );
                    EndPaint(window, &paint);
                }

                WM_CLOSE => {
                    platform.is_running = false;
                }
                WM_SETCURSOR => {
                    result = DefWindowProcW(window, message, wparam, lparam);
                    // setcursor(ptr::null_mut());
                }
                _ => {
                    result = DefWindowProcW(window, message, wparam, lparam);
                }
            };

            result
        }
    }

    fn process_pending_messages(&mut self, keyboard_controller: &mut ControllerInput) {
        unsafe {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                match message.message {
                    WM_QUIT => {
                        self.is_running = false;
                    }
                    WM_SYSKEYDOWN | WM_SYSKEYUP | WM_KEYUP | WM_KEYDOWN => {
                        self.handle_key_message(&message, keyboard_controller);
                        // HANDLE KEY
                    }
                    _ => {
                        TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
            }
        }
    }

    fn handle_key_message(&mut self, message: &MSG, keyboard_controller: &mut ControllerInput) {
        let vk_code: u16 = message.wParam as u16;
        let was_down: bool = message.lParam & (1 << 31) != 0;
        let is_down: bool = message.lParam & (1 << 30) == 0;
        let is_alt_key_down = message.lParam & (1 << 29) != 0;

        if was_down == is_down {
            return;
        }

        match vk_code {
            VK_W => {
                self.process_keyboard_message(
                    keyboard_controller.get_button(ControllerButton::Up),
                    is_down,
                );
            }
            VK_S => {
                self.process_keyboard_message(
                    keyboard_controller.get_button(ControllerButton::Down),
                    is_down,
                );
            }
            VK_A => {
                self.process_keyboard_message(
                    keyboard_controller.get_button(ControllerButton::Left),
                    is_down,
                );
            }
            VK_D => {
                self.process_keyboard_message(
                    keyboard_controller.get_button(ControllerButton::Right),
                    is_down,
                );
            }
            VK_DOWN => {
                self.process_keyboard_message(
                    keyboard_controller.get_button(ControllerButton::South),
                    is_down,
                );
            }
            VK_UP => {
                self.process_keyboard_message(
                    keyboard_controller.get_button(ControllerButton::North),
                    is_down,
                );
            }
            VK_LEFT => {
                self.process_keyboard_message(
                    keyboard_controller.get_button(ControllerButton::West),
                    is_down,
                );
            }
            VK_RIGHT => {
                self.process_keyboard_message(
                    keyboard_controller.get_button(ControllerButton::East),
                    is_down,
                );
            }
            VK_SHIFT => {
                self.process_keyboard_message(
                    keyboard_controller.get_button(ControllerButton::RightShoulder),
                    is_down,
                );
            }
            VK_RETURN => {}
            VK_SPACE => self.process_keyboard_message(
                keyboard_controller.get_button(ControllerButton::Start),
                is_down,
            ),
            VK_F11 => {
                if is_down {
                    self.toggle_fullscreen(message.hwnd);
                }
            }
            VK_F4 => {
                if is_down && is_alt_key_down {
                    self.is_running = false;
                }
            }
            _ => {}
        };
    }

    fn toggle_fullscreen(&mut self, window: HWND) {
        unsafe {
            let style = GetWindowLongPtrW(window, GWL_STYLE);
            if (style & WS_OVERLAPPEDWINDOW as isize) != 0 {
                let mut monitor_info = MONITORINFO {
                    cbSize: size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };

                let window_placement_res = GetWindowPlacement(window, &mut self.window_position);
                let monitor_info_res = GetMonitorInfoW(
                    MonitorFromWindow(window, MONITOR_DEFAULTTOPRIMARY),
                    &mut monitor_info,
                );

                if window_placement_res != 0 && monitor_info_res != 0 {
                    SetWindowLongPtrW(window, GWL_STYLE, style & !(WS_OVERLAPPEDWINDOW as isize));
                    SetWindowPos(
                        window,
                        HWND_TOP,
                        monitor_info.rcMonitor.left,
                        monitor_info.rcMonitor.top,
                        monitor_info.rcMonitor.right - monitor_info.rcMonitor.left,
                        monitor_info.rcMonitor.bottom - monitor_info.rcMonitor.top,
                        SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
                    );
                }
            } else {
                SetWindowLongPtrW(window, GWL_STYLE, style | (WS_OVERLAPPEDWINDOW as isize));
                SetWindowPlacement(window, &mut self.window_position);
                SetWindowPos(
                    window,
                    ptr::null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
                );
            }
        }
    }

    fn process_keyboard_message(&self, new_state: &mut ButtonState, is_down: bool) {
        if new_state.is_down == is_down {
            return;
        }
        new_state.is_down = is_down;
        new_state.half_transition_count += 1;
    }

    fn normalize_xinput_stick_value(&self, stick_axis_value: i16, dead_zone: u16) -> f32 {
        let dead_zone = dead_zone as i16;
        if stick_axis_value < -dead_zone {
            return stick_axis_value as f32 / 32768.0;
        } else if stick_axis_value > dead_zone {
            return stick_axis_value as f32 / 32767.0;
        }
        0.0
    }

    fn process_xinput_digital_button(
        &self,
        xinput_button_state: u16,
        old_state: &mut ButtonState,
        new_state: &mut ButtonState,
        button_bit: u16,
    ) {
        new_state.is_down = (xinput_button_state & button_bit) == button_bit;
        new_state.half_transition_count = (old_state.is_down != new_state.is_down) as u32;
    }

    fn resize_dib_section(&mut self, width: u32, height: u32) {
        let buffer = &mut self.offscreen_buffer;
        buffer.width = width;
        buffer.height = height;

        buffer.info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        buffer.info.bmiHeader.biWidth = buffer.width as i32;
        buffer.info.bmiHeader.biHeight = buffer.height as i32;
        buffer.info.bmiHeader.biPlanes = 1;
        buffer.info.bmiHeader.biBitCount = 32;
        buffer.info.bmiHeader.biCompression = BI_RGB;

        let pixel_count = (buffer.width * buffer.height) as usize;
        buffer.pixels.clear();
        buffer.pixels.resize(pixel_count, 0);
    }

    fn get_window_dimension(&self, window: HWND) -> (u32, u32) {
        let mut rect = RECT::default();
        unsafe { GetClientRect(window, &mut rect) };
        let width = (rect.right - rect.left) as u32;
        let height = (rect.bottom - rect.top) as u32;
        (width, height)
    }

    fn display_buffer_in_window(
        &mut self,
        device_context: HDC,
        window_width: u32,
        window_height: u32,
        // x: i32,
        // y: i32,
    ) {
        unsafe {
            let buffer = &mut self.offscreen_buffer;

            if window_width >= buffer.width * 2 && window_height >= buffer.height * 2 {
                StretchDIBits(
                    device_context,
                    0,
                    0,
                    window_width as i32,
                    window_height as i32,
                    0,
                    0,
                    buffer.width as i32,
                    buffer.height as i32,
                    buffer.pixels.as_mut_ptr() as *mut c_void,
                    &buffer.info,
                    DIB_RGB_COLORS,
                    SRCCOPY,
                );
            } else {
                let offset_x = 0;
                let offset_y = 0;

                PatBlt(
                    device_context,
                    0,
                    0,
                    window_width as i32,
                    offset_y,
                    BLACKNESS,
                );
                PatBlt(
                    device_context,
                    0,
                    offset_y + buffer.height as i32,
                    window_width as i32,
                    offset_y,
                    BLACKNESS,
                );
                PatBlt(
                    device_context,
                    0,
                    0,
                    offset_x,
                    window_height as i32,
                    BLACKNESS,
                );
                PatBlt(
                    device_context,
                    offset_x + buffer.width as i32,
                    0,
                    window_width as i32,
                    window_height as i32,
                    BLACKNESS,
                );

                StretchDIBits(
                    device_context,
                    offset_x,
                    offset_y,
                    buffer.width as i32,
                    buffer.height as i32,
                    0,
                    0,
                    buffer.width as i32,
                    buffer.height as i32,
                    buffer.pixels.as_mut_ptr() as *mut c_void,
                    &buffer.info,
                    DIB_RGB_COLORS,
                    SRCCOPY,
                );
            }
        }
    }

    fn get_exe_filename(&mut self) {
        unsafe {
            let file_name_size = GetModuleFileNameW(
                ptr::null_mut(),
                self.exe_filename.as_mut_ptr() as *mut u16,
                MAX_PATH,
            );
            assert!(file_name_size > 0); // Filename MUST be non empty

            self.one_past_last_exe_file_name_slash = 0;
            if let Some(last_index) = self.exe_filename.iter().rposition(|v| *v == '\\' as u16) {
                self.one_past_last_exe_file_name_slash = last_index + 1;
            }
        }
    }

    fn build_cpath_from_exe_dir_utf16(&self, filename: &str) -> Vec<u16> {
        let mut result: Vec<u16> =
            self.exe_filename[..self.one_past_last_exe_file_name_slash].to_vec();

        result.extend(filename.encode_utf16());
        result.push(0); // Null terminator
        result
    }

    fn get_last_write_time(&self, filename_utf16: &Vec<u16>) -> FILETIME {
        let mut find_data = WIN32_FILE_ATTRIBUTE_DATA::default();
        unsafe {
            GetFileAttributesExW(
                filename_utf16.as_ptr(),
                GetFileExInfoStandard,
                &mut find_data as *mut WIN32_FILE_ATTRIBUTE_DATA as *mut c_void,
            );
        }
        find_data.ftLastWriteTime
    }

    fn load_game_code(&self) -> Win32GameCode {
        unsafe {
            let dll = self.build_cpath_from_exe_dir_utf16(format!("{GAME_DLL}.dll").as_str());
            let temp_dll =
                self.build_cpath_from_exe_dir_utf16(format!("{GAME_DLL}_temp.dll").as_str());

            let mut result = Win32GameCode::default();
            result.dll_last_write_time = self.get_last_write_time(&dll);
            CopyFileW(dll.as_ptr(), temp_dll.as_ptr(), FALSE);

            result.game_code_dll = LoadLibraryW(temp_dll.as_ptr() as *const u16);

            if result.game_code_dll.is_null() {
                return result;
            }
            result.is_valid = true;
            if let Some(proc_address) =
                GetProcAddress(result.game_code_dll, s!("game_update_and_render"))
            {
                let func: GameUpdateAndRenderFn = std::mem::transmute(proc_address);
                result.game_update_and_render = Some(func);
            } else {
                result.is_valid = false;
            }

            if !result.is_valid {
                result.game_update_and_render = None;
            }
            result
        }
    }

    pub fn platform_main() {
        // SAFETY: This is platform-specific code, it has to be unsafe
        //         The programmer should be responsible for ensuring the correctness of the code
        unsafe {
            assert!(cfg!(target_os = "windows"));
            let mut win32_platform = Win32Platform::new();
            QueryPerformanceFrequency(&mut win32_platform.perf_count_frequency);

            win32_platform.get_exe_filename();

            let desired_scheduler_ms = 1;
            let sleep_is_granular = timeBeginPeriod(desired_scheduler_ms) == TIMERR_NOERROR;

            let mut last_counter = win32_platform.get_wall_clock();

            let hinstance = GetModuleHandleW(ptr::null());

            let mut window_class = WNDCLASSW::default();
            window_class.style = CS_HREDRAW | CS_VREDRAW;
            window_class.lpfnWndProc = Some(Win32Platform::main_window_callback);
            window_class.hInstance = hinstance;
            window_class.hCursor = LoadCursorW(ptr::null_mut(), IDC_ARROW);
            window_class.lpszClassName = w!("handmaderustwindowclass");

            if RegisterClassW(&window_class) == 0 {
                log_error!("Failed to register window class");
                return;
            }

            let window: HWND = CreateWindowExW(
                0,
                window_class.lpszClassName,
                w!("Handmade Rust"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                ptr::null_mut(),
                ptr::null_mut(),
                hinstance,
                ptr::null_mut(),
            );

            if window.is_null() {
                log_error!("Failed to create window");
                return;
            }

            let platform_ptr: *mut Win32Platform = &mut win32_platform;
            SetWindowLongPtrW(window, GWLP_USERDATA, platform_ptr as isize);

            let window_width = 960;
            let window_height = 540;

            SetWindowPos(
                window,
                ptr::null_mut(),
                0,
                0,
                window_width,
                window_height,
                0,
            );

            {
                let (window_width, window_height) = win32_platform.get_window_dimension(window);
                win32_platform.resize_dib_section(window_width, window_height);
            }

            let monitor_refresh_hz: f32 = {
                let mut value = 60.0;
                let device_context = GetDC(window);
                let win32_refresh_rate: i32 = GetDeviceCaps(device_context, VREFRESH as i32);
                if win32_refresh_rate > 1 {
                    value = win32_refresh_rate as f32;
                }
                ReleaseDC(window, device_context);
                value
            };
            let game_update_hz = monitor_refresh_hz / 2.0;
            let target_seconds_per_frame = 1.0 / game_update_hz;

            let mut game_memory: GameMemory = GameMemory::default();
            game_memory.permanent_storage_size = megabytes!(256);
            game_memory.transient_storage_size = gigabytes!(1);
            win32_platform.game_memory_block_total_size =
                game_memory.permanent_storage_size + game_memory.transient_storage_size;

            win32_platform.game_memory_block = VirtualAlloc(
                ptr::null_mut(),
                win32_platform.game_memory_block_total_size as usize,
                MEM_RESERVE | MEM_COMMIT,
                PAGE_READWRITE,
            );

            game_memory.permanent_storage = win32_platform.game_memory_block;
            game_memory.transient_storage = (game_memory.permanent_storage as *mut u8)
                .add(game_memory.permanent_storage_size as usize)
                as *mut c_void;

            if game_memory.permanent_storage.is_null() || game_memory.transient_storage.is_null() {
                log_error!("Failed to allocate game memory");
                return;
            }

            let mut old_input: Input = Input::default();
            let mut new_input: Input = Input::default();

            win32_platform.is_running = true;

            println!("{}", GAME_DLL);
            println!("{}", EXECUTABLE_NAME);

            let game_code = win32_platform.load_game_code();

            // main loop
            while win32_platform.is_running {
                new_input.delta_time = target_seconds_per_frame;
                new_input.executable_reloaded = false;

                let old_keyboard_controller: &mut ControllerInput =
                    old_input.get_controller_input(0);
                let new_keyboard_controller: &mut ControllerInput =
                    new_input.get_controller_input(0);

                *new_keyboard_controller = ControllerInput::default();
                new_keyboard_controller.is_connected = true;
                for i in 0..new_keyboard_controller.buttons.len() {
                    new_keyboard_controller.buttons[i].is_down =
                        old_keyboard_controller.buttons[i].is_down;
                }

                win32_platform.process_pending_messages(new_keyboard_controller);

                let mut mouse_pos: POINT = POINT::default();
                GetCursorPos(&mut mouse_pos);
                ScreenToClient(window, &mut mouse_pos);
                new_input.mouse_x = mouse_pos.x;
                new_input.mouse_y = mouse_pos.y;
                new_input.mouse_wheel = 0;

                win32_platform.process_keyboard_message(
                    &mut new_input.get_mouse_button(MouseButton::Left),
                    GetKeyState(VK_LBUTTON as i32) & (1 << 15) != 0,
                );

                win32_platform.process_keyboard_message(
                    &mut new_input.get_mouse_button(MouseButton::Right),
                    GetKeyState(VK_RBUTTON as i32) & (1 << 15) != 0,
                );

                win32_platform.process_keyboard_message(
                    &mut new_input.get_mouse_button(MouseButton::Middle),
                    GetKeyState(VK_MBUTTON as i32) & (1 << 15) != 0,
                );

                let xuser_max_count = (XUSER_MAX_COUNT + 1) as usize;
                let max_controller_count = if xuser_max_count < new_input.controllers.len() {
                    xuser_max_count
                } else {
                    new_input.controllers.len()
                };

                for controller_index in 1..max_controller_count {
                    let mut xinput_state = XINPUT_STATE::default();
                    let old_controller = old_input.get_controller_input(controller_index);
                    let new_controller = new_input.get_controller_input(controller_index);
                    if XInputGetState((controller_index - 1) as u32, &mut xinput_state)
                        != ERROR_SUCCESS
                    {
                        new_controller.is_connected = false;
                        continue;
                    }
                    new_controller.is_connected = true;
                    let gamepad = &xinput_state.Gamepad;
                    new_controller.is_analog = old_controller.is_analog;

                    new_controller.left_stick_x = win32_platform.normalize_xinput_stick_value(
                        gamepad.sThumbLX,
                        XINPUT_GAMEPAD_LEFT_THUMB_DEADZONE,
                    );
                    new_controller.left_stick_y = win32_platform.normalize_xinput_stick_value(
                        gamepad.sThumbLY,
                        XINPUT_GAMEPAD_LEFT_THUMB_DEADZONE,
                    );
                    new_controller.right_stick_x = win32_platform.normalize_xinput_stick_value(
                        gamepad.sThumbRX,
                        XINPUT_GAMEPAD_RIGHT_THUMB_DEADZONE,
                    );
                    new_controller.right_stick_y = win32_platform.normalize_xinput_stick_value(
                        gamepad.sThumbRY,
                        XINPUT_GAMEPAD_RIGHT_THUMB_DEADZONE,
                    );

                    let dpad_up = (gamepad.wButtons & XINPUT_GAMEPAD_DPAD_UP) != 0;
                    let dpad_down = (gamepad.wButtons & XINPUT_GAMEPAD_DPAD_DOWN) != 0;
                    let dpad_left = (gamepad.wButtons & XINPUT_GAMEPAD_DPAD_LEFT) != 0;
                    let dpad_right = (gamepad.wButtons & XINPUT_GAMEPAD_DPAD_RIGHT) != 0;

                    if new_controller.left_stick_x != 0.0
                        || new_controller.left_stick_y != 0.0
                        || new_controller.right_stick_x != 0.0
                        || new_controller.right_stick_y != 0.0
                    {
                        new_controller.is_analog = true;
                    }
                    if dpad_up {
                        new_controller.left_stick_y = 1.0;
                        new_controller.is_analog = false;
                    }
                    if dpad_down {
                        new_controller.left_stick_y = -1.0;
                        new_controller.is_analog = false;
                    }
                    if dpad_left {
                        new_controller.left_stick_x = -1.0;
                        new_controller.is_analog = false;
                    }
                    if dpad_right {
                        new_controller.left_stick_x = 1.0;
                        new_controller.is_analog = false;
                    }

                    let threshold = 0.5;
                    win32_platform.process_xinput_digital_button(
                        (new_controller.left_stick_x < -threshold) as u16,
                        old_controller.get_button(ControllerButton::Left),
                        new_controller.get_button(ControllerButton::Left),
                        1,
                    );
                    win32_platform.process_xinput_digital_button(
                        (new_controller.left_stick_x > threshold) as u16,
                        old_controller.get_button(ControllerButton::Right),
                        new_controller.get_button(ControllerButton::Right),
                        1,
                    );
                    win32_platform.process_xinput_digital_button(
                        (new_controller.left_stick_y < -threshold) as u16,
                        old_controller.get_button(ControllerButton::Down),
                        new_controller.get_button(ControllerButton::Down),
                        1,
                    );
                    win32_platform.process_xinput_digital_button(
                        (new_controller.left_stick_y > threshold) as u16,
                        old_controller.get_button(ControllerButton::Up),
                        new_controller.get_button(ControllerButton::Up),
                        1,
                    );

                    win32_platform.process_xinput_digital_button(
                        gamepad.wButtons,
                        old_controller.get_button(ControllerButton::Up),
                        new_controller.get_button(ControllerButton::Up),
                        XINPUT_GAMEPAD_DPAD_UP,
                    );
                    win32_platform.process_xinput_digital_button(
                        gamepad.wButtons,
                        old_controller.get_button(ControllerButton::Down),
                        new_controller.get_button(ControllerButton::Down),
                        XINPUT_GAMEPAD_DPAD_DOWN,
                    );
                    win32_platform.process_xinput_digital_button(
                        gamepad.wButtons,
                        old_controller.get_button(ControllerButton::Left),
                        new_controller.get_button(ControllerButton::Left),
                        XINPUT_GAMEPAD_DPAD_LEFT,
                    );
                    win32_platform.process_xinput_digital_button(
                        gamepad.wButtons,
                        old_controller.get_button(ControllerButton::Right),
                        new_controller.get_button(ControllerButton::Right),
                        XINPUT_GAMEPAD_DPAD_RIGHT,
                    );
                    win32_platform.process_xinput_digital_button(
                        gamepad.wButtons,
                        old_controller.get_button(ControllerButton::LeftShoulder),
                        new_controller.get_button(ControllerButton::LeftShoulder),
                        XINPUT_GAMEPAD_LEFT_SHOULDER,
                    );
                    win32_platform.process_xinput_digital_button(
                        gamepad.wButtons,
                        old_controller.get_button(ControllerButton::RightShoulder),
                        new_controller.get_button(ControllerButton::RightShoulder),
                        XINPUT_GAMEPAD_RIGHT_SHOULDER,
                    );
                    win32_platform.process_xinput_digital_button(
                        gamepad.wButtons,
                        old_controller.get_button(ControllerButton::Start),
                        new_controller.get_button(ControllerButton::Start),
                        XINPUT_GAMEPAD_START,
                    );
                    win32_platform.process_xinput_digital_button(
                        gamepad.wButtons,
                        old_controller.get_button(ControllerButton::South),
                        new_controller.get_button(ControllerButton::South),
                        XINPUT_GAMEPAD_A,
                    );
                    win32_platform.process_xinput_digital_button(
                        gamepad.wButtons,
                        old_controller.get_button(ControllerButton::West),
                        new_controller.get_button(ControllerButton::West),
                        XINPUT_GAMEPAD_B,
                    );
                    win32_platform.process_xinput_digital_button(
                        gamepad.wButtons,
                        old_controller.get_button(ControllerButton::North),
                        new_controller.get_button(ControllerButton::North),
                        XINPUT_GAMEPAD_Y,
                    );
                    win32_platform.process_xinput_digital_button(
                        gamepad.wButtons,
                        old_controller.get_button(ControllerButton::East),
                        new_controller.get_button(ControllerButton::East),
                        XINPUT_GAMEPAD_X,
                    );
                }

                if let Some(game_update_and_render) = game_code.game_update_and_render {
                    game_update_and_render(&mut game_memory, &new_input);
                }

                let mut game_buffer = OffscreenBuffer {
                    pixels: win32_platform.offscreen_buffer.pixels.as_mut_ptr(),
                    width: win32_platform.offscreen_buffer.width,
                    height: win32_platform.offscreen_buffer.height,
                    pitch: win32_platform.offscreen_buffer.pitch,
                };

                let work_counter = win32_platform.get_wall_clock();

                let work_seconds_elapsed =
                    win32_platform.get_seconds_elapsed(last_counter, work_counter);
                let mut seconds_elapsed_for_frame = work_seconds_elapsed;
                if seconds_elapsed_for_frame < target_seconds_per_frame {
                    if sleep_is_granular {
                        let sleep_ms: u32 =
                            1000 * (target_seconds_per_frame - seconds_elapsed_for_frame) as u32;
                        if sleep_ms > 0 {
                            Sleep(sleep_ms);
                        }
                    }
                    while seconds_elapsed_for_frame < target_seconds_per_frame {
                        seconds_elapsed_for_frame = win32_platform
                            .get_seconds_elapsed(last_counter, win32_platform.get_wall_clock());
                    }
                } else {
                    log_warning!("Missed a frame!");
                }

                let end_counter = win32_platform.get_wall_clock();
                let counter_elapsed = end_counter - last_counter;
                last_counter = end_counter;

                let (window_width, window_height) = win32_platform.get_window_dimension(window);
                {
                    let device_context = GetDC(window);
                    win32_platform.display_buffer_in_window(
                        device_context,
                        window_width,
                        window_height,
                        // 0, 0
                    );
                    ReleaseDC(window, device_context);
                }

                // let fps = platform.perf_count_frequency / counter_elapsed;
                // log_info!("fps: {}", fps);
            }
        }
    }
}
