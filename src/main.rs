#![cfg_attr(windows, windows_subsystem = "windows")]

mod config;
mod core;
mod platform;

fn main() {
    #[cfg(windows)]
    unsafe { windows_sys::Win32::System::Console::AttachConsole(windows_sys::Win32::System::Console::ATTACH_PARENT_PROCESS); }
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("keybmouse [--debug] [--smoke-test]\nOpens settings and runs in the notification area.\n--smoke-test installs the input hook for 1 second, then exits without opening settings.");
        return;
    }
    if args.iter().any(|a| a != "--debug" && a != "--smoke-test") {
        eprintln!("Unknown option. Use --help.");
        std::process::exit(2);
    }
    if let Err(error) = platform::run(args.iter().any(|a| a == "--debug"), args.iter().any(|a| a == "--smoke-test")) {
        eprintln!("keybmouse: {error}");
        #[cfg(windows)]
        if !args.iter().any(|a| a == "--smoke-test") {
            use windows_sys::Win32::UI::WindowsAndMessaging::*;
            let message: Vec<u16> = format!("{error}\0").encode_utf16().collect();
            let title: Vec<u16> = "Keybmouse\0".encode_utf16().collect();
            unsafe { MessageBoxW(std::ptr::null_mut(), message.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR); }
        }
        std::process::exit(1);
    }
}
