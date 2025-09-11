use anyhow::{Result, Context, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::CURRENT_TIME;

// Import timer functions and window utilities
use crate::timer;
use crate::window;

// Import constants
use crate::constants::{
    API_URL, BG_COLOR, TEXT_COLOR,
    SYSTEM_COLOR, USER_COLOR, ASSISTANT_COLOR, FONT_NAME,
    MAX_MESSAGES, MIN_LOCK_MINUTES, MAX_LOCK_MINUTES,
    JUDGE_MODEL, JUDGE_PROMPT, ChatMessage, keysym
};

// Result of a lock screen session
pub enum LockResult {
    Unlocked,
    TimedLock(u64), // Minutes
}

// Message struct for API calls
#[derive(Serialize, Clone)]
struct Message {
    role: String,
    content: String,
}

// API request structure
#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
}

// API response structure
#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: String,
}

// State for the X11 lock screen
enum LockState {
    Init,
    Chat, // New state for chat mode
}

// Main function that runs the interactive lock screen with Claude chat
pub async fn run_interactive_lock_screen(
    api_key: &str,
    unlock_phrase: &str,
    screen_context: &str,
) -> Result<LockResult> {
    println!("Locking screen with interactive chat functionality.");

    // Create a reqwest client for API calls
    let client = Client::new();

    // Clone the unlock phrase
    let unlock_phrase = unlock_phrase.to_string();

    // Initialize X11 and run the lock screen
    match decide(&client, api_key, &unlock_phrase, screen_context).await {
        Ok(result) => {
            match result {
                LockResult::Unlocked => {
                    println!("Screen unlocked.");
                    Ok(LockResult::Unlocked)
                },
                LockResult::TimedLock(minutes) => {
                    // Start the timer within X11 - chat session is done,
                    // but we need to enforce the lock timer
                    println!("Starting lock timer for {} minutes...", minutes);

                    // Run the X11 timer with the lock minutes
                    display_lock_timer(minutes).await?;

                    println!("Lock timer completed.");
                    Ok(LockResult::TimedLock(minutes))
                }
            }
        },
        Err(e) => {
            eprintln!("Error in interactive lock screen: {}", e);
            Err(e)
        }
    }
}

// Use display_lock_timer from timer module
async fn display_lock_timer(minutes: u64) -> Result<()> {
    timer::display_lock_timer(minutes, grab_keyboard_and_mouse).await
}

// Initialize conversation with the system prompt and screen context
fn initialize_conversation(conversation: &mut Vec<Message>, screen_context: &str) {
    // System prompt
    conversation.push(Message {
        role: "assistant".to_string(),
        content: JUDGE_PROMPT.to_string(),
    });

    // Add screen context if provided
    if !screen_context.is_empty() {
        conversation.push(Message {
            role: "user".to_string(),
            content: format!("Here's what was on my screen that triggered the lock:\n\n{}", screen_context),
        });

        // Initial assistant response acknowledging the context
        conversation.push(Message {
            role: "assistant".to_string(),
            content: "I've reviewed the content that was on your screen. Now, please explain why you should be allowed to continue.".to_string(),
        });
    }
}

// Implementation of the interactive lock screen
async fn decide(
    client: &Client,
    api_key: &str,
    unlock_phrase: &str,
    screen_context: &str,
) -> Result<LockResult> {
    // Connect to the X server
    let (conn, screen_num) = x11rb::connect(None)
        .context("Failed to connect to X server")?;

    let conn = Arc::new(conn);
    let screen = &conn.setup().roots[screen_num];

    // Create lock window
    let mut locks = create_lock_windows(&conn, screen)?;

    // Initialize the conversation with system prompt and screen context
    if let Some(conversation) = &mut locks[0].conversation {
        initialize_conversation(conversation, screen_context);
    }

    // Lock keyboard and mouse
    grab_keyboard_and_mouse(&conn, screen)?;

    // Map the windows to display them
    for lock in &locks {
        conn.map_window(lock.win)?;
    }
    conn.flush()?;

    // Set to chat mode
    locks[0].state = LockState::Chat;
    set_lock_color(&conn, &locks, &LockState::Chat)?;

    // Add initial message to display
    let intro_message = "Locked:";
    locks[0].messages.push_back((ChatMessage::System(intro_message.to_string()), SYSTEM_COLOR));

    // Draw the initial chat window
    draw_chat_window(&conn, &locks[0], screen)?;

    // Run the interactive chat loop
    let result = handle_interactive_chat(&conn, client, api_key, &mut locks[0], screen, unlock_phrase).await?;

    Ok(result)
}

struct LockWindow {
    win: Window,
    state: LockState,
    gc: Gcontext,
    input_buffer: String,
    messages: VecDeque<(ChatMessage, u32)>, // Message and its color
    conversation: Option<Vec<Message>>, // Claude conversation history
}

fn create_lock_windows(
    conn: &Arc<x11rb::rust_connection::RustConnection>,
    screen: &Screen,
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
    let cursor = window::create_invisible_cursor(conn, win)?;
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

    Ok(vec![LockWindow {
        win,
        state: LockState::Init,
        gc,
        input_buffer: String::new(),
        messages: VecDeque::new(),
        conversation: Some(Vec::new()),
    }])
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

fn grab_keyboard_and_mouse(conn: &Arc<x11rb::rust_connection::RustConnection>, screen: &Screen) -> Result<()> {
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
    conn: &Arc<x11rb::rust_connection::RustConnection>,
    lock: &LockWindow,
    text: &str,
    x: i16,
    y: i16,
    color: u32
) -> Result<()> {
    window::draw_text(conn, lock.win, lock.gc, text, x, y, color)?;
    conn.flush()?;
    Ok(())
}

fn draw_chat_window(
    conn: &Arc<x11rb::rust_connection::RustConnection>,
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

// Main handler for the interactive chat
async fn handle_interactive_chat(
    conn: &Arc<x11rb::rust_connection::RustConnection>,
    client: &Client,
    api_key: &str,
    lock: &mut LockWindow,
    screen: &Screen,
    unlock_phrase: &str,
) -> Result<LockResult> {
    // Chat loop - allow up to MAX_MESSAGES interactions
    for i in 0..MAX_MESSAGES {
        println!("DEBUG: Waiting for user input (message {}/{})", i+1, MAX_MESSAGES);

        // Get user input
        let user_input = get_user_input(conn, lock, screen, unlock_phrase)?;

        // Check for auto-unlock
        if user_input == "__AUTO_UNLOCK__" {
            return Ok(LockResult::Unlocked);
        }

        // Process the message with Claude
        if let Some(result) = process_message_with_claude(
            conn, client, api_key, lock, screen, &user_input
        ).await? {
            return Ok(result);
        }
    }

    // If we reach here, we've gone through all messages without a decision
    // Default to minimum lock time
    lock.messages.push_back((
        ChatMessage::Decision(format!("SCREEN LOCKED FOR {} MINUTES", MIN_LOCK_MINUTES)),
        TEXT_COLOR
    ));
    draw_chat_window(conn, lock, screen)?;

    // Wait briefly so user can see the message
    std::thread::sleep(Duration::from_secs(1));

    Ok(LockResult::TimedLock(MIN_LOCK_MINUTES))
}

// Get user input from the X11 window
fn get_user_input(
    conn: &Arc<x11rb::rust_connection::RustConnection>,
    lock: &mut LockWindow,
    screen: &Screen,
    unlock_phrase: &str,
) -> Result<String> {
    // Clear the input buffer
    lock.input_buffer.clear();
    draw_chat_window(conn, lock, screen)?;

    // Loop until we get user input
    loop {
        match conn.wait_for_event() {
            Ok(event) => {
                if let Event::KeyPress(key) = event {
                    // Get the pressed key
                    let reply = conn.get_keyboard_mapping(key.detail, 1)?.reply()?;
                    if reply.keysyms.len() > 0 {
                        let keysym = reply.keysyms[0];

                        match keysym {
                            // Enter key - submit the input
                            keysym::ENTER => {
                                if !lock.input_buffer.is_empty() {
                                    // Check for auto-unlock phrase
                                    if lock.input_buffer.trim().to_lowercase() == "unlock pls" {
                                        // Add message to display queue
                                        lock.messages.push_back((
                                            ChatMessage::Decision("UNLOCKING SCREEN (Auto-unlock)".to_string()),
                                            TEXT_COLOR
                                        ));
                                        draw_chat_window(conn, lock, screen)?;

                                        return Ok("__AUTO_UNLOCK__".to_string());
                                    }

                                    // Return the user input
                                    let input = lock.input_buffer.clone();
                                    lock.input_buffer.clear();

                                    // Add message to display queue
                                    lock.messages.push_back((
                                        ChatMessage::User(input.clone()),
                                        USER_COLOR
                                    ));
                                    draw_chat_window(conn, lock, screen)?;

                                    return Ok(input);
                                }
                            },
                            // Escape key - clear input
                            keysym::ESCAPE => {
                                lock.input_buffer.clear();
                                draw_chat_window(conn, lock, screen)?;
                            },
                            // Backspace key - delete last character
                            keysym::BACKSPACE => {
                                if !lock.input_buffer.is_empty() {
                                    lock.input_buffer.pop();
                                    draw_chat_window(conn, lock, screen)?;
                                }
                            },
                            // Normal key - add to input
                            _ => {
                                if process_key_input(keysym, &mut lock.input_buffer) {
                                    // Regular unlock phrase check
                                    if check_unlock_phrase(&lock.input_buffer, unlock_phrase) {
                                        return Ok("__AUTO_UNLOCK__".to_string());
                                    }

                                    // Update the display
                                    draw_chat_window(conn, lock, screen)?;
                                }
                            }
                        }
                    }
                } else if let Event::Expose(_) = event {
                    // Redraw on expose event
                    draw_chat_window(conn, lock, screen)?;
                }
            },
            Err(e) => return Err(anyhow!("Error getting X11 event: {}", e)),
        }
    }
}

// Process a message with Claude API
async fn process_message_with_claude(
    conn: &Arc<x11rb::rust_connection::RustConnection>,
    client: &Client,
    api_key: &str,
    lock: &mut LockWindow,
    screen: &Screen,
    user_input: &str,
) -> Result<Option<LockResult>> {
    // Get a reference to the conversation
    // Scope the mutable borrow to fix the borrow checker error
    let user_message = Message {
        role: "user".to_string(),
        content: user_input.to_string(),
    };

    let conversation_clone = {
        // Create a temporary mutable borrow
        let conversation = match &mut lock.conversation {
            Some(c) => c,
            None => return Err(anyhow!("Conversation not initialized")),
        };

        // Add user message to conversation
        conversation.push(user_message);

        // Create a clone for the API call
        conversation.clone()
    };

    // Show "thinking" indicator in the UI
    lock.messages.push_back((
        ChatMessage::System("Claude is thinking...".to_string()),
        SYSTEM_COLOR
    ));
    draw_chat_window(conn, lock, screen)?;

    // Call Claude API
    println!("DEBUG: Calling Claude API");
    let response = call_claude_api(client, api_key, &conversation_clone).await?;
    println!("DEBUG: Received Claude response: {}", response);

    // Remove the "thinking" message
    lock.messages.pop_back();

    // Add Claude's response to conversation
    // Now we can borrow mutably again since the previous mutable borrow is out of scope
    if let Some(conversation) = &mut lock.conversation {
        conversation.push(Message {
            role: "assistant".to_string(),
            content: response.clone(),
        });
    }

    // Add message to display
    lock.messages.push_back((
        ChatMessage::Assistant(response.clone()),
        ASSISTANT_COLOR
    ));
    draw_chat_window(conn, lock, screen)?;

    // Check for decision
    if response.contains("UNLOCK") {
        // Add decision message
        lock.messages.push_back((
            ChatMessage::Decision("UNLOCKING SCREEN".to_string()),
            TEXT_COLOR
        ));
        draw_chat_window(conn, lock, screen)?;

        // Wait briefly so user can see the message
        std::thread::sleep(Duration::from_secs(1));

        return Ok(Some(LockResult::Unlocked));
    } else if response.contains("LOCK:") {
        // Extract lock duration
        let parts: Vec<&str> = response.split("LOCK:").collect();
        if parts.len() >= 2 {
            if let Ok(minutes) = parts[1].trim().parse::<u64>() {
                let minutes = minutes.clamp(MIN_LOCK_MINUTES, MAX_LOCK_MINUTES);

                // Add decision message
                lock.messages.push_back((
                    ChatMessage::Decision(format!("SCREEN LOCKED FOR {} MINUTES", minutes)),
                    TEXT_COLOR
                ));
                draw_chat_window(conn, lock, screen)?;

                // Wait briefly so user can see the message
                std::thread::sleep(Duration::from_secs(1));

                return Ok(Some(LockResult::TimedLock(minutes)));
            }
        }

        // Default to minimum lock time if parsing fails
        lock.messages.push_back((
            ChatMessage::Decision(format!("SCREEN LOCKED FOR {} MINUTES", MIN_LOCK_MINUTES)),
            TEXT_COLOR
        ));
        draw_chat_window(conn, lock, screen)?;

        // Wait briefly so user can see the message
        std::thread::sleep(Duration::from_secs(1));

        return Ok(Some(LockResult::TimedLock(MIN_LOCK_MINUTES)));
    }

    // No decision made
    Ok(None)
}


fn set_lock_color(
    conn: &Arc<x11rb::rust_connection::RustConnection>,
    locks: &[LockWindow],
    state: &LockState
) -> Result<()> {
    let color = match state {
        LockState::Init => BG_COLOR,
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

// Call the Claude API with the current conversation
async fn call_claude_api(client: &Client, api_key: &str, conversation: &[Message]) -> Result<String> {
    let request = AnthropicRequest {
        model: JUDGE_MODEL.to_string(),
        messages: conversation.to_vec(),
        max_tokens: 300,
    };

    println!("DEBUG: Sending request to Anthropic API with model: {}", JUDGE_MODEL);

    let response = client.post(API_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&request)
        .send()
        .await
        .context("Failed to send request to Anthropic API")?;

    let status = response.status();
    println!("DEBUG: API response status: {}", status);

    // Get the raw response text
    let response_text = response.text().await
        .context("Failed to get raw response text")?;

    println!("DEBUG: Raw API response: {}", response_text);

    // Parse the JSON response manually after logging it
    let response_data: AnthropicResponse = serde_json::from_str(&response_text)
        .context("Failed to parse Anthropic API response")?;

    let parsed_text = response_data.content
        .first()
        .map(|block| block.text.trim())
        .unwrap_or("")
        .to_string();

    println!("DEBUG: Parsed text from response: {}", parsed_text);

    Ok(parsed_text)
}