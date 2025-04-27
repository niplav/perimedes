use anyhow::{Result, Context, anyhow};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::mpsc::{Sender, Receiver};
use std::thread;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::CURRENT_TIME;

// Re-export ChatMessage from chat.rs to avoid duplication
pub use crate::chat::ChatMessage;

// X11 font
const FONT_NAME: &str = "-misc-fixed-medium-r-normal--13-120-75-75-c-70-iso8859-1";

// Colors for the lock screen
const BG_COLOR: u32 = 0x282828; // Dark gray background
const INPUT_COLOR: u32 = 0x444444; // Lighter gray when typing
const FAILED_COLOR: u32 = 0xcc0000; // Red on failed unlock attempt
const TEXT_COLOR: u32 = 0xebdbb2; // Light text color
const SYSTEM_COLOR: u32 = 0xfabd2f; // Yellow for system messages
const USER_COLOR: u32 = 0x83a598; // Blue for user messages
const ASSISTANT_COLOR: u32 = 0xb8bb26; // Green for assistant messages

// X11 keysym constants for special keys
mod keysym {
    pub const SPACE: u32 = 0x20;
    pub const ENTER: u32 = 0xff0d;
    pub const ESCAPE: u32 = 0xff1b;
    pub const BACKSPACE: u32 = 0xff08;
}

// State for the X11 lock screen
enum LockState {
    Init,
    Input,
    Failed,
    Chat, // New state for chat mode
}

// A blocking function that locks the screen with chat functionality
pub fn lock_screen_with_chat(
    unlock_phrase: &str,
    chat_msg_receiver: Receiver<ChatMessage>,
    chat_input_sender: Sender<String>,
) -> Result<()> {
    println!("Locking screen with chat functionality.");

    // Clone the unlock phrase for the thread
    let unlock_phrase = unlock_phrase.to_string();

    // Run the lock screen with chat functionality
    match run_lock_screen(&unlock_phrase, Some(chat_msg_receiver), Some(chat_input_sender)) {
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

fn run_lock_screen(
    unlock_phrase: &str,
    chat_msg_receiver: Option<Receiver<ChatMessage>>,
    chat_input_sender: Option<Sender<String>>,
) -> Result<()> {
    // Connect to the X server
    let (conn, screen_num) = x11rb::connect(None)
        .context("Failed to connect to X server")?;

    let conn = Arc::new(conn);
    let screen = &conn.setup().roots[screen_num];

    // Create lock for each screen
    let mut locks = create_lock_windows(&conn, screen, chat_msg_receiver, chat_input_sender)?;

    // Lock keyboard and mouse
    grab_keyboard_and_mouse(&conn, screen)?;

    // Map the windows to display them
    for lock in &locks {
        conn.map_window(lock.win)?;
    }
    conn.flush()?;

    // Determine which mode to use based on whether chat is enabled
    let is_chat_mode = locks[0].chat_message_receiver.is_some();

    // Handle unlocking with unified function
    handle_unlock_loop(&conn, &mut locks, unlock_phrase, is_chat_mode)?;

    Ok(())
}

struct LockWindow {
    win: Window,
    state: LockState,
    font: Font,
    gc: Gcontext,
    input_buffer: String,
    messages: VecDeque<(ChatMessage, u32)>, // Message and its color
    chat_input_sender: Option<Sender<String>>,
    chat_message_receiver: Option<Receiver<ChatMessage>>,
}

fn create_lock_windows(
    conn: &Arc<impl Connection>,
    screen: &Screen,
    chat_msg_receiver: Option<Receiver<ChatMessage>>,
    chat_input_sender: Option<Sender<String>>,
) -> Result<Vec<LockWindow>> {
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

    // Load font for text
    let font = conn.generate_id()?;
    conn.open_font(font, FONT_NAME.as_bytes())?;

    // Create graphics context
    let gc = conn.generate_id()?;
    let gc_aux = CreateGCAux::new()
        .foreground(TEXT_COLOR)
        .background(BG_COLOR)
        .font(font);
    conn.create_gc(gc, win, &gc_aux)?;

    // Determine the initial state
    let initial_state = if chat_msg_receiver.is_some() {
        LockState::Chat
    } else {
        LockState::Init
    };

    Ok(vec![LockWindow {
        win,
        state: initial_state,
        font,
        gc,
        input_buffer: String::new(),
        messages: VecDeque::new(),
        chat_input_sender,
        chat_message_receiver: chat_msg_receiver,
    }])
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

// Process a keyboard key and add it to the input buffer if it's a supported character
// Returns true if a character was added to the buffer
fn process_key_input(
    keysym: u32,
    input_buffer: &mut String,
) -> bool {
    // Skip special keys that should be handled separately
    if keysym == keysym::ENTER || keysym == keysym::ESCAPE || keysym == keysym::BACKSPACE {
        return false;
    }

    // Handle supported characters
    match keysym {
        // Space
        keysym::SPACE => {
            input_buffer.push(' ');
            true
        },
        // Alphanumeric: digits (0-9), letters (A-Z, a-z)
        0x30..=0x39 | 0x41..=0x5a | 0x61..=0x7a => {
            if let Some(c) = char::from_u32(keysym) {
                input_buffer.push(c);
                true
            } else {
                false
            }
        },
        // Ignore other keys
        _ => false
    }
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

fn draw_text(
    conn: &Arc<impl Connection>,
    lock: &LockWindow,
    text: &str,
    x: i16,
    y: i16,
    color: u32
) -> Result<()> {
    // Update foreground color
    let values = ChangeGCAux::new().foreground(color);
    conn.change_gc(lock.gc, &values)?;

    // Draw text
    conn.image_text8(lock.win, lock.gc, x, y, text.as_bytes())?;
    conn.flush()?;

    Ok(())
}

fn draw_chat_window(
    conn: &Arc<impl Connection>,
    lock: &LockWindow,
    screen: &Screen
) -> Result<()> {
    // Clear window first
    conn.clear_area(false, lock.win, 0, 0, 0, 0)?;

    // Draw chat history
    let y_pos = 50; // Starting Y position
    let line_height = 20; // Space between lines
    let max_visible_lines = (screen.height_in_pixels as i16 - 120) / line_height;

    // Calculate range of messages to display (most recent ones)
    let start_idx = if lock.messages.len() > max_visible_lines as usize {
        lock.messages.len() - max_visible_lines as usize
    } else {
        0
    };

    // Draw each message
    for (i, (message, color)) in lock.messages.iter().enumerate().skip(start_idx) {
        let y = y_pos + (i - start_idx) as i16 * line_height;

        // Format and draw based on message type
        match message {
            ChatMessage::System(text) => {
                draw_text(conn, lock, &format!("System: {}", text), 20, y, *color)?;
            },
            ChatMessage::User(text) => {
                draw_text(conn, lock, &format!("You: {}", text), 20, y, *color)?;
            },
            ChatMessage::Assistant(text) => {
                // Split long messages into multiple lines
                let max_line_length = 80;
                let mut lines = Vec::new();
                let mut current_line = String::new();

                for word in text.split_whitespace() {
                    if current_line.len() + word.len() + 1 > max_line_length {
                        lines.push(current_line);
                        current_line = word.to_string();
                    } else {
                        if !current_line.is_empty() {
                            current_line.push(' ');
                        }
                        current_line.push_str(word);
                    }
                }

                if !current_line.is_empty() {
                    lines.push(current_line);
                }

                // Draw first line with the prefix
                if let Some(first_line) = lines.first() {
                    draw_text(conn, lock, &format!("Claude: {}", first_line), 20, y, *color)?;
                }

                // Draw remaining lines with proper indentation
                for (line_idx, line) in lines.iter().enumerate().skip(1) {
                    let line_y = y + line_idx as i16 * (line_height / 2);
                    draw_text(conn, lock, &format!("        {}", line), 20, line_y, *color)?;
                }
            },
            ChatMessage::Decision(text) => {
                draw_text(conn, lock, &format!("=== {} ===", text), 20, y, *color)?;
            },
        }
    }

    // Draw input field
    let input_y = screen.height_in_pixels as i16 - 50;
    draw_text(conn, lock, "Input: ", 20, input_y, TEXT_COLOR)?;
    draw_text(conn, lock, &lock.input_buffer, 80, input_y, TEXT_COLOR)?;

    conn.flush()?;
    Ok(())
}

// Helper function to check if input matches unlock phrase
fn check_unlock_phrase(input: &str, unlock_phrase: &str) -> bool {
    input.to_uppercase() == unlock_phrase
}

// Handle a key press event in chat mode
fn handle_chat_key_press(
    conn: &Arc<impl Connection>,
    lock: &mut LockWindow,
    screen: &Screen,
    keysym: u32,
    unlock_phrase: &str,
) -> Result<Option<()>> {
    match keysym {
        // Enter key
        keysym::ENTER => {
            // Check for unlock phrase
            if check_unlock_phrase(&lock.input_buffer, unlock_phrase) {
                // Unlock and exit
                return Ok(Some(()));
            }

            // Send input to chat handler
            if let Some(sender) = &lock.chat_input_sender {
                // Clone the input before sending
                let input_text = lock.input_buffer.clone();
                if let Err(e) = sender.send(input_text) {
                    eprintln!("Failed to send user input: {}", e);
                }

                // Clear input buffer after sending
                lock.input_buffer.clear();

                // Redraw the chat
                draw_chat_window(conn, lock, screen)?;
            }
        },
        // Escape key
        keysym::ESCAPE => {
            lock.input_buffer.clear();
            draw_chat_window(conn, lock, screen)?;
        },
        // Backspace key
        keysym::BACKSPACE => {
            if !lock.input_buffer.is_empty() {
                lock.input_buffer.pop();
                draw_chat_window(conn, lock, screen)?;
            }
        },
        // Normal key
        _ => {
            if process_key_input(keysym, &mut lock.input_buffer) {
                // Input was added

                // Check if input matches unlock phrase
                if check_unlock_phrase(&lock.input_buffer, unlock_phrase) {
                    return Ok(Some(()));
                }

                draw_chat_window(conn, lock, screen)?;
            }
        }
    }

    Ok(None)
}

// Handle a key press event in regular lock mode
fn handle_regular_key_press(
    conn: &Arc<impl Connection>,
    lock: &mut LockWindow,
    keysym: u32,
    unlock_phrase: &str,
    failure: &mut bool
) -> Result<Option<()>> {
    match keysym {
        // Enter key
        keysym::ENTER => {
            // Check for unlock phrase
            if check_unlock_phrase(&lock.input_buffer, unlock_phrase) {
                // Unlock and exit
                return Ok(Some(()));
            }

            // Wrong password
            lock.input_buffer.clear();
            *failure = true;
            lock.state = LockState::Failed;
        },
        // Escape key
        keysym::ESCAPE => {
            lock.input_buffer.clear();
            lock.state = if *failure { LockState::Failed } else { LockState::Init };
        },
        // Backspace key
        keysym::BACKSPACE => {
            if !lock.input_buffer.is_empty() {
                lock.input_buffer.pop();

                lock.state = if lock.input_buffer.is_empty() {
                    if *failure { LockState::Failed } else { LockState::Init }
                } else {
                    LockState::Input
                };
            }
        },
        // Normal key
        _ => {
            if process_key_input(keysym, &mut lock.input_buffer) {
                // Input was added

                // Check if input matches unlock phrase
                if check_unlock_phrase(&lock.input_buffer, unlock_phrase) {
                    return Ok(Some(()));
                }

                lock.state = LockState::Input;
            }
        }
    }

    Ok(None)
}

// Process incoming chat messages
fn process_chat_messages(
    conn: &Arc<impl Connection>,
    lock: &mut LockWindow,
    screen: &Screen,
    receiver: &Receiver<ChatMessage>
) -> Result<Option<()>> {
    let mut should_unlock = false;

    while let Ok(message) = receiver.try_recv() {
        let color = match &message {
            ChatMessage::System(_) => SYSTEM_COLOR,
            ChatMessage::User(_) => USER_COLOR,
            ChatMessage::Assistant(_) => ASSISTANT_COLOR,
            ChatMessage::Decision(_) => TEXT_COLOR,
        };

        // Add message to the display queue
        lock.messages.push_back((message.clone(), color));

        // If we have a Decision message, check if we need to unlock
        if let ChatMessage::Decision(text) = &message {
            if text.contains("UNLOCKING") {
                should_unlock = true;
            }
        }
    }

    // If messages were processed, redraw the chat window
    if lock.messages.len() > 0 {
        draw_chat_window(conn, lock, screen)?;
    }

    if should_unlock {
        Ok(Some(()))
    } else {
        Ok(None)
    }
}

fn handle_unlock_loop(
    conn: &Arc<impl Connection>,
    locks: &mut [LockWindow],
    unlock_phrase: &str,
    chat_enabled: bool
) -> Result<()> {
    let mut failure = false;
    let screen = &conn.setup().roots[0]; // Assuming single screen

    // Set initial color and state
    if chat_enabled {
        locks[0].state = LockState::Chat;
    }
    set_lock_color(conn, locks, &locks[0].state)?;

    // Get chat message receiver but keep it in the main thread for chat mode
    let chat_msg_receiver = if chat_enabled {
        locks[0].chat_message_receiver.take()
    } else {
        None
    };

    while let Ok(event) = conn.wait_for_event() {
        match event {
            Event::KeyPress(key) => {
                // Get the pressed key
                let reply = conn.get_keyboard_mapping(key.detail, 1)?.reply()?;
                if reply.keysyms.len() > 0 {
                    let keysym = reply.keysyms[0];

                    // Handle key press based on mode
                    let unlock_result = if chat_enabled {
                        handle_chat_key_press(conn, &mut locks[0], screen, keysym, unlock_phrase)?
                    } else {
                        // Handle regular mode key press
                        let result = handle_regular_key_press(conn, &mut locks[0], keysym, unlock_phrase, &mut failure)?;

                        // Update lock color after state change
                        set_lock_color(conn, locks, &locks[0].state)?;

                        result
                    };

                    // If unlock condition met, return
                    if unlock_result.is_some() {
                        return Ok(());
                    }
                }
            },
            Event::Expose(_) => {
                if chat_enabled {
                    draw_chat_window(conn, &locks[0], screen)?;
                } else {
                    set_lock_color(conn, locks, &locks[0].state)?;
                }
            },
            // Any other event, also check for messages in chat mode
            _ if chat_enabled => {
                // Check for new messages from the chat backend
                if let Some(ref receiver) = chat_msg_receiver {
                    if let Some(_) = process_chat_messages(conn, &mut locks[0], screen, receiver)? {
                        return Ok(());
                    }
                }
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
        LockState::Chat => BG_COLOR, // Use same background for chat
    };

    for lock in locks {
        let values = ChangeWindowAttributesAux::new().background_pixel(color);
        conn.change_window_attributes(lock.win, &values)?;
        conn.clear_area(false, lock.win, 0, 0, 0, 0)?; // Clear the entire window
    }

    conn.flush()?;
    Ok(())
}