use std::{cell::RefCell, collections::VecDeque, error::Error, mem::size_of, ptr::null_mut,
    sync::{atomic::{AtomicBool, Ordering}, Mutex}, time::{Duration, Instant}};
use windows_sys::Win32::{Foundation::*, System::{Console::*, LibraryLoader::GetModuleHandleW, Threading::CreateMutexW, StationsAndDesktops::*},
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*}};
use crate::{config::{Config, Key}, core::{Engine, InputListener, PointerOutput}};

static STOP: AtomicBool = AtomicBool::new(false);
static OUTPUT_FAILED: AtomicBool = AtomicBool::new(false);
// Shared only with the console shutdown handler, which Windows calls on another thread.
static BUTTONS: Mutex<u8> = Mutex::new(0);
thread_local! { static ENGINE: RefCell<Option<Engine<PendingPointer>>> = const { RefCell::new(None) }; }

#[derive(Debug, PartialEq)]
enum PointerCommand {
    Move(i32, i32),
    Button(usize, bool),
    Scroll(i32),
}

#[derive(Default)]
struct PendingPointer(VecDeque<PointerCommand>);

impl PointerOutput for PendingPointer {
    fn move_by(&mut self, x: i32, y: i32) { self.0.push_back(PointerCommand::Move(x, y)); }
    fn button(&mut self, index: usize, down: bool) { self.0.push_back(PointerCommand::Button(index, down)); }
    fn scroll(&mut self, notches: i32) { self.0.push_back(PointerCommand::Scroll(notches)); }
}

fn flush_pending(mut emit: impl FnMut(PointerCommand)) {
    loop {
        let command = ENGINE.with(|slot| {
            slot.borrow_mut().as_mut().and_then(|engine| engine.pointer.0.pop_front())
        });
        let Some(command) = command else { break; };
        // SendInput can synchronously re-enter keyboard_hook. Release the engine
        // borrow first; newly queued events must retain their FIFO order.
        emit(command);
    }
}

fn flush_mouse() {
    let mut pointer = WindowsPointer;
    flush_pending(|command| match command {
        PointerCommand::Move(x, y) => pointer.move_by(x, y),
        PointerCommand::Button(index, down) => pointer.button(index, down),
        PointerCommand::Scroll(notches) => pointer.scroll(notches),
    });
}

pub struct WindowsPointer;

fn send_mouse(flags: u32, x: i32, y: i32, data: u32) -> bool {
    let input = INPUT { r#type: INPUT_MOUSE, Anonymous: INPUT_0 { mi: MOUSEINPUT {
        dx: x, dy: y, mouseData: data, dwFlags: flags, time: 0, dwExtraInfo: 0,
    } } };
    // SAFETY: input is initialized, correctly sized, and valid for this synchronous call.
    let ok = unsafe { SendInput(1, &input, size_of::<INPUT>() as i32) == 1 };
    if !ok { OUTPUT_FAILED.store(true, Ordering::SeqCst); STOP.store(true, Ordering::SeqCst); }
    ok
}

fn release_buttons() {
    let mut buttons = BUTTONS.lock().unwrap_or_else(|e| e.into_inner());
    for (index, flag) in [MOUSEEVENTF_LEFTUP, MOUSEEVENTF_RIGHTUP].into_iter().enumerate() {
        if *buttons & (1 << index) != 0 && send_mouse(flag, 0, 0, 0) { *buttons &= !(1 << index); }
    }
}

impl PointerOutput for WindowsPointer {
    fn move_by(&mut self, x: i32, y: i32) {
        if !STOP.load(Ordering::SeqCst) { send_mouse(MOUSEEVENTF_MOVE, x, y, 0); }
    }
    fn button(&mut self, index: usize, down: bool) {
        let mut buttons = BUTTONS.lock().unwrap_or_else(|e| e.into_inner());
        if down && STOP.load(Ordering::SeqCst) { return; }
        let flags = [[MOUSEEVENTF_LEFTUP, MOUSEEVENTF_LEFTDOWN], [MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_RIGHTDOWN]];
        if send_mouse(flags[index][usize::from(down)], 0, 0, 0) {
            if down { *buttons |= 1 << index; } else { *buttons &= !(1 << index); }
        }
    }
    fn scroll(&mut self, notches: i32) {
        if !STOP.load(Ordering::SeqCst) { send_mouse(MOUSEEVENTF_WHEEL, 0, 0, (notches * 120) as u32); }
    }
}

fn translate(vk: u32) -> Option<Key> {
    match vk {
        0x41..=0x5a => Some(Key::Letter(char::from_u32(vk)?)),
        0x14 => Some(Key::CapsLock), 0x0d => Some(Key::Enter), 0x20 => Some(Key::Space),
        0xa0 => Some(Key::LeftShift), 0xa1 => Some(Key::RightShift),
        0x30..=0x39 => Some(Key::Digit((vk - 0x30) as u8)),
        0x70..=0x7b => Some(Key::Function((vk - 0x70 + 1) as u8)),
        0x09 => Some(Key::Tab), 0x08 => Some(Key::Backspace), 0x1b => Some(Key::Escape),
        0x25 => Some(Key::Left), 0x26 => Some(Key::Up), 0x27 => Some(Key::Right), 0x28 => Some(Key::Down),
        _ => None,
    }
}

unsafe extern "system" fn keyboard_hook(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: Windows supplies a KBDLLHOOKSTRUCT for HC_ACTION.
        let event = unsafe { &*(data as *const KBDLLHOOKSTRUCT) };
        if event.flags & LLKHF_INJECTED == 0 {
            if let Some(key) = translate(event.vkCode) {
                let down = message as u32 == WM_KEYDOWN || message as u32 == WM_SYSKEYDOWN;
                let suppress = ENGINE.with(|slot| {
                    slot.borrow_mut().as_mut().is_some_and(|engine| engine.key(key, down))
                });
                if suppress { return 1; }
            }
        }
    }
    // SAFETY: forwarding the original callback arguments as required by Windows.
    unsafe { CallNextHookEx(null_mut(), code, message, data) }
}

unsafe extern "system" fn console_handler(event: u32) -> i32 {
    if matches!(event, CTRL_C_EVENT | CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT | CTRL_LOGOFF_EVENT | CTRL_SHUTDOWN_EVENT) {
        STOP.store(true, Ordering::SeqCst);
        release_buttons();
        1
    } else { 0 }
}

struct Hook(HHOOK);
impl Drop for Hook {
    fn drop(&mut self) {
        // SAFETY: this guard owns the installed hook and handler registration.
        unsafe { UnhookWindowsHookEx(self.0); SetConsoleCtrlHandler(Some(console_handler), 0); }
        ENGINE.with(|slot| {
            if let Some(engine) = slot.borrow_mut().as_mut() { engine.reset(); }
        });
        flush_mouse();
        let engine = ENGINE.with(|slot| slot.borrow_mut().take());
        drop(engine);
        release_buttons();
    }
}

struct Instance(HANDLE);
impl Drop for Instance {
    fn drop(&mut self) { unsafe { CloseHandle(self.0); } }
}

pub enum Control { Configure(Config), Enabled(bool), Quit }

pub fn run(debug: bool, smoke_test: bool) -> Result<(), Box<dyn Error>> {
    if !smoke_test { return super::windows_ui::run(debug); }
    let (_sender, receiver) = std::sync::mpsc::channel();
    run_input(debug, true, Config::default(), receiver, None)
}

pub fn run_input(debug: bool, smoke_test: bool, config: Config,
    controls: std::sync::mpsc::Receiver<Control>, ready: Option<std::sync::mpsc::Sender<()>>,
) -> Result<(), Box<dyn Error>> {
    let name: Vec<u16> = "Local\\KeybmousePrototype\0".encode_utf16().collect();
    // Keep one hook per session; duplicate instances would inject duplicate clicks.
    let handle = unsafe { CreateMutexW(null_mut(), 0, name.as_ptr()) };
    if handle.is_null() { return Err(std::io::Error::last_os_error().into()); }
    let already_running = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    let _instance = Instance(handle);
    if already_running { return Err("Another keybmouse instance is already running.".into()); }
    STOP.store(false, Ordering::SeqCst);
    OUTPUT_FAILED.store(false, Ordering::SeqCst);
    let mut engine = Engine::new(config, PendingPointer::default(), debug);
    // Keys already held at launch must finish their original typing sequence.
    for vk in 0..=255 {
        if let Some(key) = translate(vk) {
            // SAFETY: querying a valid virtual-key code.
            if unsafe { GetAsyncKeyState(vk as i32) } < 0 { engine.seed_held(key); }
        }
    }
    ENGINE.with(|slot| *slot.borrow_mut() = Some(engine));
    // SAFETY: callback has static lifetime; the installing thread pumps messages below.
    let raw = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), GetModuleHandleW(null_mut()), 0) };
    if raw.is_null() { return Err(std::io::Error::last_os_error().into()); }
    let hook = Hook(raw);
    if !unsafe { GetConsoleWindow() }.is_null() && unsafe { SetConsoleCtrlHandler(Some(console_handler), 1) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    println!("keybmouse running. Hold Caps Lock for mouse controls. Ctrl+C in this console quits.");
    if let Some(ready) = ready { let _ = ready.send(()); }
    // Logging never blocks the hook/message thread, even if console output is paused.
    let (logs, receiver) = std::sync::mpsc::sync_channel::<String>(128);
    std::thread::spawn(move || { for message in receiver { eprintln!("{message}"); } });
    let start = Instant::now();
    let mut previous = start;
    let mut desktop_check = start;
    while !STOP.load(Ordering::SeqCst) {
        for command in controls.try_iter() {
            ENGINE.with(|slot| {
                if let Some(engine) = slot.borrow_mut().as_mut() {
                    match command {
                        Control::Configure(config) => { let _ = engine.configure(config); }
                        Control::Enabled(enabled) => engine.set_enabled(enabled),
                        Control::Quit => STOP.store(true, Ordering::SeqCst),
                    }
                }
            });
        }
        let mut message: MSG = unsafe { std::mem::zeroed() };
        // SAFETY: message points to initialized writable storage; no window is required.
        unsafe {
            while PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) != 0 {
                if message.message == WM_QUIT { STOP.store(true, Ordering::SeqCst); break; }
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        let now = Instant::now();
        let mut desktop_unavailable = false;
        if now.duration_since(desktop_check) >= Duration::from_millis(100) {
            // Do not inject into a lock/UAC desktop we cannot access.
            let desktop = unsafe { OpenInputDesktop(0, 0, DESKTOP_READOBJECTS) };
            desktop_unavailable = desktop.is_null();
            if !desktop.is_null() { unsafe { CloseDesktop(desktop); } }
            desktop_check = now;
        }
        let cancel_layer = desktop_unavailable || unsafe { GetForegroundWindow() }.is_null();
        ENGINE.with(|slot| {
            if let Some(engine) = slot.borrow_mut().as_mut() {
                // Losing access to the interactive desktop cancels the layer. Ordinary
                // foreground-window changes do not interrupt a drag between windows.
                if cancel_layer { engine.reset(); }
                engine.tick(now.duration_since(previous).as_secs_f64());
                for message in engine.logs.drain(..) { let _ = logs.try_send(message); }
            }
        });
        flush_mouse();
        previous = now;
        if smoke_test && start.elapsed() >= Duration::from_secs(1) { break; }
        // Wake immediately for keyboard input, otherwise tick at roughly 125 Hz.
        unsafe { MsgWaitForMultipleObjectsEx(0, null_mut(), 8, QS_ALLINPUT, MWMO_INPUTAVAILABLE); }
    }
    drop(hook);
    if OUTPUT_FAILED.load(Ordering::SeqCst) {
        return Err("SendInput failed; stopped and attempted button cleanup. Elevated/secure windows may block input.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Call the real callback with a platform-shaped event, without injecting any
    // keyboard or mouse input into the user's desktop.
    fn captured_event(vk: u32, down: bool) {
        let event = KBDLLHOOKSTRUCT {
            vkCode: vk, scanCode: 0, flags: if down { 0 } else { LLKHF_UP },
            time: 0, dwExtraInfo: 0,
        };
        let result = unsafe {
            keyboard_hook(HC_ACTION as i32,
                if down { WM_KEYDOWN } else { WM_KEYUP } as WPARAM,
                &event as *const _ as LPARAM)
        };
        assert_eq!(result, 1, "captured event must not leak to normal typing");
    }

    #[test]
    fn output_can_reenter_hook_during_movement_and_button_down() {
        for reenter_on_button in [false, true] {
            ENGINE.with(|slot| *slot.borrow_mut() = Some(Engine::new(
                Config::default(), PendingPointer::default(), false)));
            captured_event(0x14, true); // Caps Lock
            captured_event(0x0d, true); // Enter
            captured_event(0x44, true); // D
            ENGINE.with(|slot| slot.borrow_mut().as_mut().unwrap().tick(0.01));

            let mut reentered = false;
            let mut commands = Vec::new();
            flush_pending(|command| {
                let trigger = if reenter_on_button {
                    matches!(command, PointerCommand::Button(0, true))
                } else { matches!(command, PointerCommand::Move(..)) };
                commands.push(command);
                if trigger && !reentered {
                    reentered = true;
                    // Models a callback dispatched synchronously from SendInput.
                    // Also hold the output mutex to catch accidental native output
                    // from the callback (which would deadlock on button release).
                    let _buttons = BUTTONS.lock().unwrap();
                    captured_event(0x14, false);
                    captured_event(0x0d, false);
                    captured_event(0x44, false);
                }
            });
            assert!(reentered);
            assert_eq!(commands.first(), Some(&PointerCommand::Button(0, true)));
            assert!(matches!(commands[1], PointerCommand::Move(x, 0) if x > 0));
            assert_eq!(commands.last(), Some(&PointerCommand::Button(0, false)));
            assert_eq!(commands.len(), 3);
            ENGINE.with(|slot| {
                let mut slot = slot.borrow_mut();
                let engine = slot.as_mut().unwrap();
                assert!(!engine.active);
                engine.tick(0.01);
                assert!(engine.pointer.0.is_empty());
                assert!(!engine.key(Key::Enter, true));
                slot.take();
            });
        }
    }
}
