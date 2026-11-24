//! Opens the window inside the screen's work area, in front.
//!
//! `tauri.conf.json` opens the window resizable at 480x720, with a 420x560 minimum; the content
//! scrolls above the fixed footer, so any height down to the minimum works. On a short work area
//! (1024x768 at 125%, or 1920x1080 at 150% with the taskbar), 720 runs under the taskbar, so at
//! startup the window's height is cut to the work area, never below the minimum, and the window
//! is centered in it. The width, and the size on screens tall enough, stay as configured.
//!
//! The window starts hidden (`"visible": false`) and is shown only after fitting: resizing and
//! moving an already visible window at startup left it behind the installer's caller (Chrome)
//! on Windows.

use tauri::{PhysicalPosition, PhysicalSize, Runtime, WebviewWindow};

/// The window's minimum inner height in logical pixels, as `tauri.conf.json` sets it.
const MIN_HEIGHT: f64 = 560.0;

/// Fits `window` to the screen, then shows and focuses it. It is shown even when fitting fails.
pub fn open<R: Runtime>(window: &WebviewWindow<R>) {
    if let Err(e) = fit_to_work_area(window) {
        // The window still opens at its configured size.
        eprintln!("could not fit the window to the screen: {e}");
    }
    if let Err(e) = window.show() {
        eprintln!("could not show the window: {e}");
    }
    if let Err(e) = window.set_focus() {
        eprintln!("could not bring the window to the front: {e}");
    }
}

/// Shrinks `window` to fit the work area of its monitor, if it doesn't already fit.
fn fit_to_work_area<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<()> {
    let Some(monitor) = window.current_monitor()? else {
        return Ok(());
    };
    let work = monitor.work_area();
    let inner = window.inner_size()?;
    let outer = window.outer_size()?;
    // Title bar and borders, in the monitor's physical pixels (none under macOS's overlay
    // title bar).
    let frame = outer.height.saturating_sub(inner.height);
    let min = (MIN_HEIGHT * window.scale_factor()?).round() as u32;
    let Some(height) = fitted_height(work.size.height, frame, inner.height, min) else {
        return Ok(());
    };
    window.set_size(PhysicalSize::new(inner.width, height))?;
    let x = work.position.x + centered(work.size.width, outer.width);
    let y = work.position.y + centered(work.size.height, height + frame);
    window.set_position(PhysicalPosition::new(x, y))
}

/// The inner height that fits a work area `work` tall with a `frame`-high title bar and borders,
/// but not below `min`; `None` when the current inner height already fits.
fn fitted_height(work: u32, frame: u32, inner: u32, min: u32) -> Option<u32> {
    let available = work.saturating_sub(frame);
    (inner > available).then_some(available.max(min).min(inner))
}

/// Offset that centers `size` in `space`; 0 when it doesn't fit.
fn centered(space: u32, size: u32) -> i32 {
    i32::try_from(space.saturating_sub(size) / 2).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_unchanged_on_tall_work_areas() {
        // 720 logical px at 100%, 30 px title bar, 1080p minus a 48 px taskbar.
        assert_eq!(fitted_height(1032, 30, 720, 560), None);
        assert_eq!(fitted_height(750, 30, 720, 560), None);
    }

    #[test]
    fn shrinks_to_short_work_areas() {
        // 1024x768 at 125%: 900 px inner, 40 px title bar, 768 - 60 px taskbar.
        assert_eq!(fitted_height(708, 40, 900, 700), Some(700));
        // 1366x768 at 100%: 720 px inner, 30 px title bar, 768 - 40 px taskbar.
        assert_eq!(fitted_height(728, 30, 720, 560), Some(698));
        // 1920x1080 at 150%: 1080 px inner, 47 px title bar, 1080 - 72 px taskbar.
        assert_eq!(fitted_height(1008, 47, 1080, 840), Some(961));
    }

    #[test]
    fn never_below_the_minimum() {
        assert_eq!(fitted_height(580, 30, 720, 560), Some(560));
        assert_eq!(fitted_height(20, 40, 720, 560), Some(560));
    }

    #[test]
    fn centers_or_pins_to_the_edge() {
        assert_eq!(centered(1024, 540), 242);
        assert_eq!(centered(708, 708), 0);
        assert_eq!(centered(500, 708), 0);
    }
}
