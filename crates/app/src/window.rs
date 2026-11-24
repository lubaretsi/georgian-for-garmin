//! Opens the window inside the screen's work area, in front.
//!
//! `tauri.conf.json` opens the window resizable at 480x720, with a 420x560 minimum; the content
//! scrolls above the fixed footer. On a short work area (1024x768 at 125%, or 1920x1080 at 150%
//! with the taskbar), 720 runs under the taskbar, so at startup the window's height is cut to the
//! work area and the window is centered in it. Where even the minimum doesn't fit (1024x768 at
//! 125%), the minimum comes down to the work area as well, so the footer and its main action stay
//! on screen. The width, and the size on screens tall enough, stay as configured.
//!
//! The window starts hidden (`"visible": false`) and is shown only after fitting: resizing and
//! moving an already visible window at startup left it behind the installer's caller (Chrome)
//! on Windows.
//!
//! A tiling window manager (yabai, AeroSpace, OmniWM on macOS; i3, Sway on Linux) resizes the
//! window after it is shown, like any other; this module doesn't fight it.

use tauri::{LogicalSize, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewWindow};

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
    let app = window.app_handle();
    let Some(config) = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == window.label())
    else {
        return Ok(());
    };
    // macOS creates the window 1 point taller than configured (480x721 for 480x720, with or
    // without the overlay title bar); setting the configured size again before it is shown
    // gives the exact size.
    #[cfg(target_os = "macos")]
    window.set_size(LogicalSize::new(config.width, config.height))?;
    let Some(monitor) = window.current_monitor()? else {
        return Ok(());
    };
    let work = monitor.work_area();
    let inner = window.inner_size()?;
    let outer = window.outer_size()?;
    // Title bar and borders, in the monitor's physical pixels (none under macOS's overlay
    // title bar).
    let frame = outer.height.saturating_sub(inner.height);
    let Some(height) = fitted_height(work.size.height, frame, inner.height) else {
        return Ok(());
    };
    // The OS keeps a window at least at its minimum size (Windows clamps any resize to it), so a
    // minimum the work area can't hold comes down first.
    let min: PhysicalSize<u32> = LogicalSize::new(
        config.min_width.unwrap_or(0.0),
        config.min_height.unwrap_or(0.0),
    )
    .to_physical(window.scale_factor()?);
    if height < min.height {
        window.set_min_size(Some(PhysicalSize::new(min.width, height)))?;
    }
    window.set_size(PhysicalSize::new(inner.width, height))?;
    let x = work.position.x + centered(work.size.width, outer.width);
    let y = work.position.y + centered(work.size.height, height + frame);
    window.set_position(PhysicalPosition::new(x, y))
}

/// The inner height that fits a work area `work` tall with a `frame`-high title bar and borders,
/// or `None` when the current inner height already fits. All sizes are the monitor's physical
/// pixels.
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

    /// `tauri.conf.json`'s window height.
    const HEIGHT: f64 = 720.0;

    /// The configured inner height in physical pixels at `scale`.
    fn configured(scale: f64) -> u32 {
        (HEIGHT * scale).round() as u32
    }

    #[test]
    fn fits_unchanged_on_tall_work_areas() {
        // 100%: 1920x1080 minus a 48 px taskbar, 30 px title bar.
        assert_eq!(fitted_height(1032, 30, configured(1.0)), None);
        assert_eq!(fitted_height(750, 30, configured(1.0)), None);
        // 200%: a 2560x1440 Retina screen under the 31 pt menu bar, overlay title bar.
        assert_eq!(fitted_height(2818, 0, configured(2.0)), None);
        // 200%: a 1440x900 MacBook Air.
        assert_eq!(fitted_height(1738, 0, configured(2.0)), None);
        // 125%: 2560x1440 minus a 60 px taskbar, 40 px title bar.
        assert_eq!(fitted_height(1380, 40, configured(1.25)), None);
        // 125%: 1920x1080 minus a 60 px taskbar, 40 px title bar.
        assert_eq!(fitted_height(1020, 40, configured(1.25)), None);
    }

    #[test]
    fn shrinks_to_short_work_areas() {
        // 100%: 1366x768 minus a 40 px taskbar, 30 px title bar.
        assert_eq!(fitted_height(728, 30, configured(1.0)), Some(698));
        // 125%: 1600x900 minus a 60 px taskbar, 40 px title bar.
        assert_eq!(fitted_height(840, 40, configured(1.25)), Some(800));
        // 150%: 1920x1080 minus a 72 px taskbar, 47 px title bar.
        assert_eq!(fitted_height(1008, 47, configured(1.5)), Some(961));
        // 200%: a 1280x720 (logical) screen under a 25 pt menu bar, overlay title bar.
        assert_eq!(fitted_height(1390, 0, configured(2.0)), Some(1390));
    }

    #[test]
    fn below_the_minimum_the_work_area_wins() {
        // 125%: 1024x768 minus a 60 px taskbar, 40 px title bar: 668 px, 534 logical, under the
        // 560 minimum; the window still ends at the taskbar.
        assert_eq!(fitted_height(708, 40, configured(1.25)), Some(668));
    }

    #[test]
    fn ends_at_the_work_area() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let inner = configured(scale);
            for frame in [0, (28.0 * scale) as u32] {
                for work in (frame..=2 * inner).step_by(7) {
                    match fitted_height(work, frame, inner) {
                        None => assert!(inner + frame <= work, "{scale} {frame} {work}"),
                        Some(height) => {
                            assert!(height < inner, "{scale} {frame} {work}: {height}");
                            assert_eq!(height + frame, work, "{scale} {frame} {work}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn centers_or_pins_to_the_edge() {
        assert_eq!(centered(1024, 540), 242);
        assert_eq!(centered(708, 708), 0);
        assert_eq!(centered(500, 708), 0);
    }

    #[test]
    fn the_height_is_the_configured_one() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(config["app"]["windows"][0]["height"], HEIGHT);
    }
}
