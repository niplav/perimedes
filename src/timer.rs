use anyhow::{Result, Context};
use std::sync::Arc;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;

// Import constants
use crate::constants::{BG_COLOR, TEXT_COLOR, FONT_NAME};

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

    // Create a simple timer window
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
        400, 200, // Small window for timer
        0,
        WindowClass::INPUT_OUTPUT,
        screen.root_visual,
        &values,
    )?;

    // Create invisible cursor
    let cursor = create_invisible_cursor(&conn, win)?;
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

    // Center the window on screen
    let x = (screen.width_in_pixels as i16 - 400) / 2;
    let y = (screen.height_in_pixels as i16 - 200) / 2;

    let values = ConfigureWindowAux::new()
        .x(x as i32)
        .y(y as i32);
    conn.configure_window(win, &values)?;

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

            // Draw timer text
            let timer_text = format!("Screen locked for {} more minutes", minutes);
            let countdown_text = format!("Remaining: {}:{:02}", remaining_minutes, remaining_seconds);
            let info_text = "Please wait for timer to complete...";

            // Draw text
            draw_timer_text(&conn, win, gc, &timer_text, 50, 50, TEXT_COLOR)?;
            draw_timer_text(&conn, win, gc, &countdown_text, 50, 90, TEXT_COLOR)?;
            draw_timer_text(&conn, win, gc, info_text, 50, 130, TEXT_COLOR)?;

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

// Helper function for timer window text
pub fn draw_timer_text(
    conn: &Arc<x11rb::rust_connection::RustConnection>,
    win: Window,
    gc: Gcontext,
    text: &str,
    x: i16,
    y: i16,
    color: u32
) -> Result<()> {
    // Update color
    let values = ChangeGCAux::new().foreground(color);
    conn.change_gc(gc, &values)?;

    // Draw text
    conn.image_text8(win, gc, x, y, text.as_bytes())?;

    Ok(())
}

// Create an invisible cursor
fn create_invisible_cursor(conn: &Arc<x11rb::rust_connection::RustConnection>, win: Window) -> Result<Cursor> {
    let cursor = conn.generate_id()?;
    let pixmap = conn.generate_id()?;

    // Create a 1x1 pixmap for the invisible cursor
    conn.create_pixmap(1, pixmap, win, 1, 1)?;

    // Create an empty cursor
    conn.create_cursor(
        cursor,
        pixmap,
        pixmap,
        0, 0, 0,  // Foreground color (RGB)
        0, 0, 0,  // Background color (RGB)
        0, 0,     // X and Y position
    )?;

    conn.free_pixmap(pixmap)?;

    Ok(cursor)
}