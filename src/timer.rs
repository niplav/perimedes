use anyhow::{Result, Context};
use std::sync::Arc;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;

// Import constants and window utilities
use crate::constants::{BG_COLOR, TEXT_COLOR, FONT_NAME};
use crate::window;

// Function to display a X11 lock timer window
// Using RustConnection directly since that's what x11rb::connect returns
pub async fn display_lock_timer(
    minutes: u64,
    grab_func: fn(&Arc<x11rb::rust_connection::RustConnection>, &Screen) -> Result<()>
) -> Result<()> {
    // Connect to the X server
    let (conn, screen_num) = x11rb::connect(None)
        .context("Failed to connect to X server")?;

    let conn = Arc::new(conn);
    let screen = &conn.setup().roots[screen_num];

    // Create a fullscreen timer window
    let win = conn.generate_id()?;
    let values = CreateWindowAux::new()
        .background_pixel(BG_COLOR)
        .override_redirect(1)
        .event_mask(EventMask::EXPOSURE | EventMask::KEY_PRESS);

    conn.create_window(
        screen.root_depth,
        win,
        screen.root,
        0, 0,
        screen.width_in_pixels, screen.height_in_pixels,
        0,
        WindowClass::INPUT_OUTPUT,
        screen.root_visual,
        &values,
    )?;

    // Create invisible cursor
    let cursor = window::create_invisible_cursor(&conn, win)?;
    let values = ChangeWindowAttributesAux::new().cursor(cursor);
    conn.change_window_attributes(win, &values)?;

    // Load font
    let font = conn.generate_id()?;
    conn.open_font(font, FONT_NAME.as_bytes())?;

    // Create graphics context
    let gc = conn.generate_id()?;
    let gc_aux = CreateGCAux::new()
        .foreground(TEXT_COLOR)
        .background(BG_COLOR)
        .font(font);
    conn.create_gc(gc, win, &gc_aux)?;

    // Grab keyboard and mouse
    grab_func(&conn, screen)?;

    // Map the window
    conn.map_window(win)?;
    conn.flush()?;

    // Initialize timer
    let start_time = std::time::Instant::now();
    let lock_duration = Duration::from_secs(minutes * 60);

    // Timer loop
    let mut running = true;
    while running {
        // Check for keyboard events (any key exits the timer)
        while let Ok(Some(event)) = conn.poll_for_event() {
            match event {
                Event::KeyPress(_) => {
                    // Ignore key presses - timer must complete
                },
                Event::Expose(_) => {
                    // Redraw on expose
                },
                _ => {}
            }
        }

        // Update timer display
        let elapsed = start_time.elapsed();
        if elapsed >= lock_duration {
            running = false;
        } else {
            let remaining = lock_duration - elapsed;
            let remaining_minutes = remaining.as_secs() / 60;
            let remaining_seconds = remaining.as_secs() % 60;

            // Clear window
            conn.clear_area(true, win, 0, 0, 0, 0)?;

            // Calculate center positions
            let center_x = screen.width_in_pixels as i16 / 2 - 100; // Approximate text width offset
            let center_y = screen.height_in_pixels as i16 / 2;

            // Draw centered timer text
            let countdown_text = format!("{}:{:02}", remaining_minutes, remaining_seconds);

            // Draw text centered on screen
            window::draw_text(&conn, win, gc, &countdown_text, center_x, center_y - 20, TEXT_COLOR)?;
            window::draw_text(&conn, win, gc, info_text, center_x - 50, center_y + 20, TEXT_COLOR)?;

            conn.flush()?;
        }

        // Sleep briefly
        std::thread::sleep(Duration::from_millis(100));
    }

    // Close the window
    conn.unmap_window(win)?;
    conn.destroy_window(win)?;
    conn.flush()?;

    Ok(())
}


