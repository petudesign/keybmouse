use std::{cell::RefCell, collections::VecDeque, error::Error, io::Write, mem::size_of,
    path::{Path, PathBuf}, ptr::null_mut, sync::mpsc, time::Duration};
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*, Storage::FileSystem::*,
    System::LibraryLoader::GetModuleHandleW, UI::{Controls::*, HiDpi::*, Shell::*, WindowsAndMessaging::*}};
use crate::config::{Config, Key};
use super::windows::{self, Control};

const SAVE: u16 = 1;
const HIDE: u16 = 2;
const DEFAULTS: u16 = 3;
const QUIT: u16 = 4;
const ENABLED: u16 = 5;
const OPEN: u16 = 6;
const STATUS: i32 = 7;
const TRAY: u32 = WM_APP + 1;
const KEY_LABELS: [&str; 11] = ["Aktivointi (pidä pohjassa)", "Ylös", "Vasemmalle", "Alas", "Oikealle",
    "Vasen painike / raahaus", "Oikea painike", "Vieritä ylös", "Vieritä alas", "Tarkkuusnäppäin 1", "Tarkkuusnäppäin 2"];
const VALUE_LABELS: [&str; 6] = ["Lähtönopeus (1–5000)", "Enimmäisnopeus (1–5000)", "Kiihdytys (0–20000)",
    "Tarkkuuskerroin (0,01–1)", "Vieritys: pykälää/s (0,1–60)", "Suunnanvaihdon tauko: ms (0–300)"];
thread_local! { static EVENTS: RefCell<VecDeque<u16>> = const { RefCell::new(VecDeque::new()) }; }
fn wide(text: &str) -> Vec<u16> { text.encode_utf16().chain(Some(0)).collect() }

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_CTLCOLORSTATIC => {
            unsafe {
                SetBkColor(w as HDC, GetSysColor(COLOR_WINDOW));
                SetTextColor(w as HDC, GetSysColor(COLOR_WINDOWTEXT));
                return GetSysColorBrush(COLOR_WINDOW) as LRESULT;
            }
        }
        WM_COMMAND => {
            let id = (w & 0xffff) as u16;
            let notification = ((w >> 16) & 0xffff) as u32;
            if id < 100 || (id < 200 && notification == CBN_SELCHANGE)
                || (id >= 200 && notification == EN_CHANGE) {
                EVENTS.with(|queue| queue.borrow_mut().push_back(id));
            }
            return 0;
        }
        WM_CLOSE => { EVENTS.with(|q| q.borrow_mut().push_back(HIDE)); return 0; }
        TRAY => {
            let event = match l as u32 {
                WM_LBUTTONUP | WM_LBUTTONDBLCLK => OPEN,
                WM_RBUTTONUP | WM_CONTEXTMENU => 8,
                _ => return 0,
            };
            EVENTS.with(|q| q.borrow_mut().push_back(event));
            return 0;
        }
        WM_QUERYENDSESSION => { EVENTS.with(|q| q.borrow_mut().push_back(QUIT)); return 1; }
        _ => {}
    }
    unsafe { DefWindowProcW(hwnd, msg, w, l) }
}

fn settings_path() -> Result<PathBuf, Box<dyn Error>> {
    Ok(PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA ei ole saatavilla.")?)
        .join("Keybmouse").join("settings.conf"))
}

fn load(path: &Path) -> Result<Config, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Config::decode(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(error) => Err(format!("Asetuksia ei voitu lukea: {error}")),
    }
}

fn save(path: &Path, config: &Config) -> Result<(), String> {
    config.validate()?;
    let parent = path.parent().ok_or("Asetuskansio puuttuu.")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temporary)
        .map_err(|e| format!("Tilapäistiedostoa ei voitu luoda: {e}"))?;
    // Write and sync a separate file before replacing the old settings atomically.
    let result = (|| -> Result<(), Box<dyn Error>> {
        file.write_all(config.encode().as_bytes())?;
        file.sync_all()?;
        drop(file);
        use std::os::windows::ffi::OsStrExt;
        let from: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(())
    })();
    if result.is_err() { let _ = std::fs::remove_file(&temporary); }
    result.map_err(|e| format!("Tallennus epäonnistui. Vanhat asetukset säilytettiin. {e}"))
}

struct SettingsWindow {
    hwnd: HWND,
    font: HFONT,
    heading: HFONT,
    scale: f64,
    tray: bool,
    saved: Config,
    path: PathBuf,
    enabled: bool,
}

impl SettingsWindow {
    fn new(config: Config, path: PathBuf) -> Result<Self, Box<dyn Error>> {
        unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE); }
        let scale = unsafe { GetDpiForSystem() } as f64 / 96.0;
        let instance = unsafe { GetModuleHandleW(null_mut()) };
        let class = wide("KeybmouseSettings");
        let icon = unsafe { LoadIconW(null_mut(), IDI_APPLICATION) };
        let wc = WNDCLASSW { lpfnWndProc: Some(window_proc), hInstance: instance, lpszClassName: class.as_ptr(),
            hCursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) }, hIcon: icon,
            hbrBackground: (COLOR_WINDOW + 1) as HBRUSH, ..unsafe { std::mem::zeroed() } };
        if unsafe { RegisterClassW(&wc) } == 0 && unsafe { GetLastError() } != ERROR_CLASS_ALREADY_EXISTS {
            return Err(std::io::Error::last_os_error().into());
        }
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
        let mut rect = RECT { left: 0, top: 0, right: (760.0 * scale) as i32, bottom: (620.0 * scale) as i32 };
        unsafe { AdjustWindowRectEx(&mut rect, style, 0, WS_EX_CONTROLPARENT); }
        let hwnd = unsafe { CreateWindowExW(WS_EX_CONTROLPARENT, class.as_ptr(), wide("Keybmouse — asetukset").as_ptr(), style,
            CW_USEDEFAULT, CW_USEDEFAULT, rect.right - rect.left, rect.bottom - rect.top,
            null_mut(), null_mut(), instance, null_mut()) };
        if hwnd.is_null() { return Err(std::io::Error::last_os_error().into()); }
        let make_font = |size, weight| unsafe { CreateFontW(-(size as f64 * scale).round() as i32, 0, 0, 0, weight,
            0, 0, 0, DEFAULT_CHARSET as u32, OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32, DEFAULT_PITCH as u32, wide("Segoe UI").as_ptr()) };
        let window = Self { hwnd, font: make_font(15, 400), heading: make_font(25, 600),
            scale, tray: false, saved: config, path, enabled: true };
        window.label("Keybmouse", 28, 22, 400, 36, true)?;
        window.label("Hiiriohjaus näppäimistöllä", 28, 60, 430, 24, false)?;
        window.control("BUTTON", "&Ohjaus käytössä", WS_TABSTOP | BS_AUTOCHECKBOX as u32,
            530, 30, 200, 30, ENABLED as i32)?;
        unsafe { SendMessageW(window.item(ENABLED as i32), BM_SETCHECK, BST_CHECKED as usize, 0); }
        window.label("Näppäinsidonnat", 28, 112, 310, 24, false)?;
        window.label("Liike ja tarkkuus", 396, 112, 325, 24, false)?;
        for (i, label) in KEY_LABELS.iter().enumerate() {
            window.label(label, 28, 152 + i as i32 * 30, 215, 24, false)?;
            let control = window.control("COMBOBOX", "", WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
                248, 149 + i as i32 * 30, 116, 300, 100 + i as i32)?;
            for key in Key::choices() {
                unsafe { SendMessageW(control, CB_ADDSTRING, 0, wide(&key.label()).as_ptr() as isize); }
            }
        }
        for (i, label) in VALUE_LABELS.iter().enumerate() {
            window.label(label, 396, 148 + i as i32 * 54, 335, 22, false)?;
            window.control("EDIT", "", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
                396, 172 + i as i32 * 54, 335, 27, 200 + i as i32)?;
        }
        window.label("Suunnanvaihto säilyttää nopeuden.\nVapautus pysäyttää liikkeen heti.", 28, 497, 336, 44, false)?;
        window.label("Nopeus säätää kursorin liikettä, ei hiiren DPI:tä.", 396, 497, 340, 44, false)?;
        window.control("STATIC", "Asetukset ovat käytössä.", 0, 28, 550, 705, 24, STATUS)?;
        for (text, x, width, id) in [("&Palauta oletukset", 28, 165, DEFAULTS), ("&Lopeta", 204, 90, QUIT),
            ("&Taustalle", 496, 108, HIDE), ("&Tallenna", 616, 116, SAVE)] {
            window.control("BUTTON", text, WS_TABSTOP | BS_PUSHBUTTON as u32, x, 580, width, 30, id as i32)?;
        }
        window.fill(&window.saved);
        Ok(window)
    }

    fn item(&self, id: i32) -> HWND { unsafe { GetDlgItem(self.hwnd, id) } }
    fn control(&self, class: &str, text: &str, style: u32, x: i32, y: i32, w: i32, h: i32, id: i32) -> Result<HWND, Box<dyn Error>> {
        let px = |value: i32| (value as f64 * self.scale).round() as i32;
        let control = unsafe { CreateWindowExW(0, wide(class).as_ptr(), wide(text).as_ptr(), WS_CHILD | WS_VISIBLE | style,
            px(x), px(y), px(w), px(h), self.hwnd, id as usize as HMENU, GetModuleHandleW(null_mut()), null_mut()) };
        if control.is_null() { return Err(std::io::Error::last_os_error().into()); }
        unsafe { SendMessageW(control, WM_SETFONT, self.font as usize, 1); }
        Ok(control)
    }
    fn label(&self, text: &str, x: i32, y: i32, w: i32, h: i32, heading: bool) -> Result<(), Box<dyn Error>> {
        let control = self.control("STATIC", text, 0, x, y, w, h, -1)?;
        if heading { unsafe { SendMessageW(control, WM_SETFONT, self.heading as usize, 1); } }
        Ok(())
    }
    fn status(&self, text: &str) { unsafe { SetWindowTextW(self.item(STATUS), wide(text).as_ptr()); } }
    fn fill(&self, config: &Config) {
        let choices = Key::choices();
        for (i, key) in config.bindings().iter().enumerate() {
            if let Some(index) = choices.iter().position(|candidate| candidate == key) {
                unsafe { SendMessageW(self.item(100 + i as i32), CB_SETCURSEL, index, 0); }
            }
        }
        for (i, value) in config.values().iter().enumerate() {
            unsafe { SetWindowTextW(self.item(200 + i as i32), wide(&value.to_string()).as_ptr()); }
        }
        EVENTS.with(|q| q.borrow_mut().retain(|id| *id < 100));
    }
    fn read(&self) -> Result<Config, String> {
        let choices = Key::choices();
        let mut config = self.saved.clone();
        let mut keys = config.bindings();
        for (i, key) in keys.iter_mut().enumerate() {
            let index = unsafe { SendMessageW(self.item(100 + i as i32), CB_GETCURSEL, 0, 0) };
            *key = *choices.get(index as usize).ok_or("Valitse näppäin jokaiselle toiminnolle.")?;
        }
        config.set_bindings(keys);
        for (i, value) in [&mut config.base_speed, &mut config.max_speed, &mut config.acceleration,
            &mut config.precision_multiplier, &mut config.scroll_notches_per_second, &mut config.direction_grace_ms].into_iter().enumerate() {
            let mut text = [0u16; 128];
            let len = unsafe { GetWindowTextW(self.item(200 + i as i32), text.as_mut_ptr(), text.len() as i32) };
            *value = String::from_utf16_lossy(&text[..len as usize]).trim().replace(',', ".").parse()
                .map_err(|_| format!("{}: kirjoita numero.", VALUE_LABELS[i]))?;
        }
        config.validate()?;
        Ok(config)
    }
    fn dirty(&self) -> bool { self.read().map_or(true, |config| config != self.saved) }
    fn notify_data(&self) -> NOTIFYICONDATAW {
        let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = self.hwnd; data.uID = 1;
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP; data.uCallbackMessage = TRAY;
        data.hIcon = unsafe { LoadIconW(null_mut(), IDI_APPLICATION) };
        let tip = wide(if self.enabled { "Keybmouse — ohjaus käytössä" } else { "Keybmouse — tauolla" });
        data.szTip[..tip.len()].copy_from_slice(&tip);
        data
    }
    fn add_tray(&mut self) {
        self.tray = unsafe { Shell_NotifyIconW(NIM_ADD, &self.notify_data()) } != 0;
        if !self.tray { self.status("Ilmoitusalueen kuvake ei ole saatavilla. Pidä ikkuna auki."); }
    }
    fn show(&self) { unsafe { ShowWindow(self.hwnd, SW_RESTORE); SetForegroundWindow(self.hwnd); } }
    fn menu(&self) {
        unsafe {
            let menu = CreatePopupMenu();
            AppendMenuW(menu, MF_STRING, OPEN as usize, wide("Asetukset").as_ptr());
            AppendMenuW(menu, MF_STRING | if self.enabled { MF_CHECKED } else { MF_UNCHECKED }, ENABLED as usize,
                wide("Ohjaus käytössä").as_ptr());
            AppendMenuW(menu, MF_SEPARATOR, 0, null_mut());
            AppendMenuW(menu, MF_STRING, QUIT as usize, wide("Lopeta").as_ptr());
            let mut point: POINT = std::mem::zeroed(); GetCursorPos(&mut point);
            SetForegroundWindow(self.hwnd);
            let chosen = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, point.x, point.y, 0, self.hwnd, null_mut());
            DestroyMenu(menu);
            PostMessageW(self.hwnd, WM_NULL, 0, 0);
            if chosen != 0 { EVENTS.with(|q| q.borrow_mut().push_back(chosen as u16)); }
        }
    }
    fn handle(&mut self, id: u16, sender: &mpsc::Sender<Control>) -> Result<bool, String> {
        match id {
            SAVE => {
                let config = self.read()?;
                save(&self.path, &config)?;
                sender.send(Control::Configure(config.clone())).map_err(|_| "Hiiriohjaus on pysähtynyt.")?;
                self.saved = config;
                self.status("Tallennettu. Uudet asetukset ovat käytössä.");
            }
            DEFAULTS => { self.fill(&Config::default()); self.status("Oletukset palautettu lomakkeelle. Ota käyttöön painamalla Tallenna."); }
            HIDE => {
                if self.tray { unsafe { ShowWindow(self.hwnd, SW_HIDE); } }
                else { unsafe { ShowWindow(self.hwnd, SW_MINIMIZE); } }
            }
            OPEN => self.show(),
            ENABLED => {
                self.enabled = !self.enabled;
                sender.send(Control::Enabled(self.enabled)).map_err(|_| "Hiiriohjaus on pysähtynyt.")?;
                unsafe {
                    SendMessageW(self.item(ENABLED as i32), BM_SETCHECK, usize::from(self.enabled), 0);
                    if self.tray { Shell_NotifyIconW(NIM_MODIFY, &self.notify_data()); }
                }
                self.status(if self.enabled { "Ohjaus käytössä. Tallentamattomat muutokset eivät ole vielä käytössä." }
                    else { "Ohjaus tauolla. Näppäimistö toimii normaalisti." });
            }
            QUIT => {
                if self.dirty() {
                    let choice = unsafe { MessageBoxW(self.hwnd, wide("Suljetaanko tallentamatta muutoksia?").as_ptr(),
                        wide("Keybmouse").as_ptr(), MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2) };
                    if choice != IDYES { return Ok(false); }
                }
                return Ok(true);
            }
            8 => self.menu(),
            100..=205 => self.status("Tallentamattomia muutoksia. Ota käyttöön painamalla Tallenna."),
            _ => {}
        }
        Ok(false)
    }
}

impl Drop for SettingsWindow {
    fn drop(&mut self) {
        unsafe {
            if self.tray { Shell_NotifyIconW(NIM_DELETE, &self.notify_data()); }
            DestroyWindow(self.hwnd); DeleteObject(self.font); DeleteObject(self.heading);
        }
    }
}

pub fn run(debug: bool) -> Result<(), Box<dyn Error>> {
    let path = settings_path()?;
    let loaded = load(&path);
    let config = loaded.clone().unwrap_or_default();
    let mut window = SettingsWindow::new(config.clone(), path)?;
    let (sender, receiver) = mpsc::channel();
    let (ready_sender, ready) = mpsc::channel();
    let worker = std::thread::spawn(move || windows::run_input(debug, false, config, receiver, Some(ready_sender)).map_err(|e| e.to_string()));
    if ready.recv_timeout(Duration::from_secs(5)).is_err() {
        let _ = sender.send(Control::Quit);
        let result = worker.join().map_err(|_| "Hiiriohjauksen käynnistys epäonnistui.")?;
        return Err(result.err().unwrap_or_else(|| "Hiiriohjaus ei käynnistynyt.".into()).into());
    }
    window.add_tray(); window.show();
    if let Err(error) = loaded { window.status(&format!("Asetuksia ei ladattu: {error}")); }
    let taskbar_created = unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) };
    let mut done = false;
    while !done && !worker.is_finished() {
        let mut message: MSG = unsafe { std::mem::zeroed() };
        unsafe {
            while PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) != 0 {
                if message.message == WM_QUIT { done = true; break; }
                if message.message == taskbar_created { window.add_tray(); }
                if IsDialogMessageW(window.hwnd, &message) == 0 {
                    TranslateMessage(&message); DispatchMessageW(&message);
                }
            }
        }
        loop {
            let event = EVENTS.with(|q| q.borrow_mut().pop_front());
            let Some(event) = event else { break; };
            match window.handle(event, &sender) {
                Ok(quit) => { if quit { done = true; break; } }
                Err(error) => {
                    window.status(&error);
                    unsafe { MessageBoxW(window.hwnd, wide(&error).as_ptr(), wide("Tarkista asetukset").as_ptr(), MB_OK | MB_ICONWARNING); }
                }
            }
        }
        unsafe { MsgWaitForMultipleObjectsEx(0, null_mut(), 100, QS_ALLINPUT, MWMO_INPUTAVAILABLE); }
    }
    let _ = sender.send(Control::Quit);
    worker.join().map_err(|_| "Hiiriohjaus kaatui.")?.map_err(|e| e.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_form_validates_saves_reloads_and_pauses() {
        let path = std::env::temp_dir().join(format!("keybmouse-ui-test-{}.conf", std::process::id()));
        let mut window = SettingsWindow::new(Config::default(), path.clone()).unwrap();
        if let Ok(path) = std::env::var("KEYBMOUSE_UI_SNAPSHOT") {
            // Optional visual QA of this test window only; no input hook is installed.
            unsafe {
                ShowWindow(window.hwnd, SW_SHOWNOACTIVATE); UpdateWindow(window.hwnd);
                let mut rect: RECT = std::mem::zeroed(); GetWindowRect(window.hwnd, &mut rect);
                let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
                let screen = GetDC(window.hwnd);
                let dc = CreateCompatibleDC(screen);
                let bitmap = CreateCompatibleBitmap(screen, width, height);
                let old = SelectObject(dc, bitmap);
                assert_ne!(windows_sys::Win32::Storage::Xps::PrintWindow(window.hwnd, dc, 2), 0);
                SelectObject(dc, old);
                let stride = (width as usize * 3 + 3) & !3;
                let mut pixels = vec![0u8; stride * height as usize];
                let mut info: BITMAPINFO = std::mem::zeroed();
                info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
                info.bmiHeader.biWidth = width; info.bmiHeader.biHeight = -height;
                info.bmiHeader.biPlanes = 1; info.bmiHeader.biBitCount = 24;
                assert_ne!(GetDIBits(dc, bitmap, 0, height as u32, pixels.as_mut_ptr().cast(), &mut info, DIB_RGB_COLORS), 0);
                let mut bytes = b"BM".to_vec();
                bytes.extend(((54 + pixels.len()) as u32).to_le_bytes()); bytes.extend([0u8; 4]);
                bytes.extend(54u32.to_le_bytes()); bytes.extend(40u32.to_le_bytes());
                bytes.extend(width.to_le_bytes()); bytes.extend((-height).to_le_bytes());
                bytes.extend(1u16.to_le_bytes()); bytes.extend(24u16.to_le_bytes()); bytes.extend([0u8; 24]);
                bytes.extend(pixels); std::fs::write(path, bytes).unwrap();
                DeleteObject(bitmap); DeleteDC(dc); ReleaseDC(window.hwnd, screen);
            }
        }
        let (sender, receiver) = mpsc::channel();
        assert_eq!(window.read().unwrap(), Config::default());
        let mut changed = Config::default(); changed.activation = Key::Function(8); changed.base_speed = 220.0;
        changed.direction_grace_ms = 75.0;
        window.fill(&changed); assert!(window.dirty());
        window.handle(SAVE, &sender).unwrap();
        assert!(matches!(receiver.recv().unwrap(), Control::Configure(config) if config == changed));
        assert_eq!(load(&path).unwrap(), changed); assert!(!window.dirty());
        // Invalid and duplicate form values cannot replace the last good file.
        unsafe { SetWindowTextW(window.item(200), wide("NaN").as_ptr()); }
        assert!(window.handle(SAVE, &sender).is_err());
        assert_eq!(load(&path).unwrap(), changed);
        let mut duplicate = changed.clone(); duplicate.clicks[0] = duplicate.activation;
        window.fill(&duplicate); assert!(window.handle(SAVE, &sender).is_err());
        assert_eq!(load(&path).unwrap(), changed);
        window.fill(&changed);
        window.handle(ENABLED, &sender).unwrap();
        assert!(matches!(receiver.recv().unwrap(), Control::Enabled(false)));
        window.handle(DEFAULTS, &sender).unwrap(); assert_eq!(window.read().unwrap(), Config::default());
        assert_eq!(load(&path).unwrap(), changed); // Defaults need explicit Save.
        window.handle(SAVE, &sender).unwrap(); assert_eq!(load(&path).unwrap(), Config::default());
        drop(window);
        std::fs::remove_file(path).unwrap();
    }
}
