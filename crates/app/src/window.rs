//! Keeps the fixed-size window inside the screen's work area.
//!
//! `tauri.conf.json` sizes the window for its tallest page. On a short work area (1024x768 at
//! 125%, or 1920x1080 at 150% with the taskbar), that size runs under the taskbar, so at startup
//! the window's height is cut to the work area and the window is centered in it; the page then
//! scrolls (`ui/style.css`). The width, and the size on screens tall enough, stay as configured.

use tauri::{PhysicalPosition, PhysicalSize, Runtime, WebviewWindow};

/// Shrinks `window` to fit the work area of its monitor, if it doesn't already fit.
pub fn fit_to_work_area<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<()> {
    let Some(monitor) = window.current_monitor()? else {
        return Ok(());
    };
    let work = monitor.work_area();
    let inner = window.inner_size()?;
    let outer = window.outer_size()?;
    // Title bar and borders, in the monitor's physical pixels.
    let frame = outer.height.saturating_sub(inner.height);
    let Some(height) = fitted_height(work.size.height, frame, inner.height) else {
        return Ok(());
    };
    window.set_size(PhysicalSize::new(inner.width, height))?;
    let x = work.position.x + centered(work.size.width, outer.width);
    let y = work.position.y + centered(work.size.height, height + frame);
    window.set_position(PhysicalPosition::new(x, y))
}

/// The inner height that fits a work area `work` tall with a `frame`-high title bar and borders,
/// or `None` when the current inner height already fits.
fn fitted_height(work: u32, frame: u32, inner: u32) -> Option<u32> {
    let available = work.saturating_sub(frame);
    (inner > available).then_some(available)
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
        // 680 logical px at 100%, 30 px title bar, 1080p minus a 48 px taskbar.
        assert_eq!(fitted_height(1032, 30, 680), None);
        assert_eq!(fitted_height(710, 30, 680), None);
    }

    #[test]
    fn shrinks_to_short_work_areas() {
        // 1024x768 at 125%: 850 px inner, 40 px title bar, 768 - 60 px taskbar.
        assert_eq!(fitted_height(708, 40, 850), Some(668));
        // 1920x1080 at 150%: 1020 px inner, 47 px title bar, 1080 - 72 px taskbar.
        assert_eq!(fitted_height(1008, 47, 1020), Some(961));
        assert_eq!(fitted_height(20, 40, 850), Some(0));
    }

    #[test]
    fn centers_or_pins_to_the_edge() {
        assert_eq!(centered(1024, 540), 242);
        assert_eq!(centered(708, 708), 0);
        assert_eq!(centered(500, 708), 0);
    }
}
