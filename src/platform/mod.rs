#[cfg(windows)]
mod windows;
#[cfg(windows)]
mod windows_ui;
#[cfg(windows)]
pub use windows::run;

#[cfg(not(windows))]
pub fn run(_debug: bool, _smoke_test: bool) -> Result<(), Box<dyn std::error::Error>> {
    Err("Only Windows input is implemented. macOS needs an Accessibility event-tap listener and CoreGraphics PointerOutput adapter.".into())
}
