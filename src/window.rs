// Shared X11 window utilities

use anyhow::Result;
use std::sync::Arc;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;

// Create an invisible cursor for lock screens
pub fn create_invisible_cursor(conn: &Arc<x11rb::rust_connection::RustConnection>, win: Window) -> Result<Cursor> {
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

// Draw text on a window with specified color
pub fn draw_text(
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