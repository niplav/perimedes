use anyhow::{Result, Context, anyhow};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::CURRENT_TIME;

// Colors for the lock screen
const BG_COLOR: u32 = 0x282828; // Dark gray background
const INPUT_COLOR: u32 = 0x444444; // Lighter gray when typing
const FAILED_COLOR: u32 = 0xcc0000; // Red on failed unlock attempt

// State for the X11 lock screen
enum LockState {
    Init,
    Input,
    Failed,
}

// A blocking function that locks the screen and returns when unlocked
pub fn lock_screen(unlock_phrase: &str) -> Result<()> {
    println!("Locking screen. Type '{}' to unlock.", unlock_phrase);

    // Clone the unlock phrase for the thread
    let unlock_phrase = unlock_phrase.to_string();

    // Run the lock screen on the current thread since we're blocking anyway
    match run_lock_screen(&unlock_phrase) {
        Ok(_) => {
            println!("Screen unlocked!");
            Ok(())
        },
        Err(e) => {
            eprintln!("Error in lock screen: {}", e);
            Err(e)
        }
    }
}

fn run_lock_screen(unlock_phrase: &str) -> Result<()> {
    // Connect to the X server
    let (conn, screen_num) = x11rb::connect(None)
        .context("Failed to connect to X server")?;

    let conn = Arc::new(conn);
    let screen = &conn.setup().roots[screen_num];

    // Create lock for each screen
    let locks = create_lock_windows(&conn, screen)?;

    // Lock keyboard and mouse
    grab_keyboard_and_mouse(&conn, screen)?;

    // Map the windows to display them
    for lock in &locks {
        conn.map_window(lock.win)?;
    }
    conn.flush()?;

    // Handle unlocking (blocks until unlocked)
    handle_unlock_loop(&conn, &locks, unlock_phrase)?;

    Ok(())
}

struct LockWindow {
    win: Window,
    state: LockState,
}

fn create_lock_windows(conn: &Arc<impl Connection>, screen: &Screen) -> Result<Vec<LockWindow>> {
    let win = conn.generate_id()?;

    // Create a fullscreen window
    let values = CreateWindowAux::new()
        .background_pixel(BG_COLOR)
        .override_redirect(1)
        .event_mask(EventMask::KEY_PRESS | EventMask::EXPOSURE);

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
    let cursor = create_invisible_cursor(conn, win)?;
    let values = ChangeWindowAttributesAux::new().cursor(cursor);
    conn.change_window_attributes(win, &values)?;

    Ok(vec![LockWindow { win, state: LockState::Init }])
}

fn create_invisible_cursor(conn: &Arc<impl Connection>, win: Window) -> Result<Cursor> {
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

fn grab_keyboard_and_mouse(conn: &Arc<impl Connection>, screen: &Screen) -> Result<()> {
    // Try to grab keyboard and mouse for 600ms, similar to slock
    for _ in 0..6 {
        let kb_grab = conn.grab_keyboard(
            false,
            screen.root,
            CURRENT_TIME,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
        )?.reply();

        let ptr_grab = conn.grab_pointer(
            false,
            screen.root,
            EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::POINTER_MOTION,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
            x11rb::NONE,
            x11rb::NONE,
            CURRENT_TIME,
        )?.reply();

        if let (Ok(kb), Ok(ptr)) = (&kb_grab, &ptr_grab) {
            if kb.status == GrabStatus::SUCCESS && ptr.status == GrabStatus::SUCCESS {
                return Ok(());
            }
        }

        thread::sleep(Duration::from_millis(100));
    }

    Err(anyhow!("Failed to grab keyboard and mouse"))
}

fn handle_unlock_loop(
    conn: &Arc<impl Connection>,
    locks: &[LockWindow],
    unlock_phrase: &str
) -> Result<()> {
    let mut input_buffer = String::new();
    let mut current_state = LockState::Init;
    let mut failure = false;

    // Set initial color
    set_lock_color(conn, locks, &current_state)?;

    while let Ok(event) = conn.wait_for_event() {
        match event {
            Event::KeyPress(key) => {
                // Get the pressed key
                let reply = conn.get_keyboard_mapping(key.detail, 1)?.reply()?;
                if reply.keysyms.len() > 0 {
                    let keysym = reply.keysyms[0];

                    match keysym {
                        // Enter key
                        0xff0d => {
                            if input_buffer.to_uppercase() == unlock_phrase {
                                // Unlock and exit
                                return Ok(());
                            } else {
                                // Wrong password
                                input_buffer.clear();
                                failure = true;
                                current_state = LockState::Failed;
                                set_lock_color(conn, locks, &current_state)?;
                            }
                        },
                        // Escape key
                        0xff1b => {
                            input_buffer.clear();
                            current_state = if failure { LockState::Failed } else { LockState::Init };
                            set_lock_color(conn, locks, &current_state)?;
                        },
                        // Backspace key
                        0xff08 => {
                            if !input_buffer.is_empty() {
                                input_buffer.pop();
                                current_state = if input_buffer.is_empty() {
                                    if failure { LockState::Failed } else { LockState::Init }
                                } else {
                                    LockState::Input
                                };
                                set_lock_color(conn, locks, &current_state)?;
                            }
                        },
                        // Normal key
                        _ => {
                            // Convert keysym to char if it's a printable ASCII character
                            if keysym >= 0x20 && keysym <= 0x7e {
                                let c = char::from_u32(keysym).unwrap_or('?');
                                input_buffer.push(c);

                                // Check if input matches unlock phrase
                                if input_buffer.to_uppercase() == unlock_phrase {
                                    return Ok(());
                                }

                                current_state = LockState::Input;
                                set_lock_color(conn, locks, &current_state)?;
                            }
                        }
                    }
                }
            },
            Event::Expose(_) => {
                set_lock_color(conn, locks, &current_state)?;
            },
            _ => {}
        }
    }

    Err(anyhow!("X11 event loop terminated unexpectedly"))
}

fn set_lock_color(
    conn: &Arc<impl Connection>,
    locks: &[LockWindow],
    state: &LockState
) -> Result<()> {
    let color = match state {
        LockState::Init => BG_COLOR,
        LockState::Input => INPUT_COLOR,
        LockState::Failed => FAILED_COLOR,
    };

    for lock in locks {
        let values = ChangeWindowAttributesAux::new().background_pixel(color);
        conn.change_window_attributes(lock.win, &values)?;
        conn.clear_area(false, lock.win, 0, 0, 0, 0)?; // Clear the entire window
    }

    conn.flush()?;
    Ok(())
}