mod win32_platform;
pub const EXECUTABLE_NAME: &str = env!("CARGO_PKG_NAME");

fn main() {
    if cfg!(target_os = "windows") {
        win32_platform::Win32Platform::platform_main();
    } else {
        unimplemented!("This OS is not supported");
    }
}
