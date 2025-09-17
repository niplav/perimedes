use serde::{Deserialize, Serialize};
use chrono::{DateTime, Local};

// Types shared across multiple modules

// API request structure
#[derive(Serialize)]
pub struct AnthropicRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub max_tokens: u32,
}

// API response structure
#[derive(Deserialize)]
pub struct AnthropicResponse {
    pub content: Vec<ContentBlock>,
}

// Chat message types
#[derive(Clone)]
pub enum ChatMessage {
    System(String),
    User(String),
    Assistant(String),
    Decision(String),
}

// Message struct for API calls
#[derive(Serialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

// Result of a lock screen session
pub enum LockResult {
    Unlocked,
    TimedLock(u64), // Minutes
}

// State for the X11 lock screen
pub enum LockState {
    Init,
    Chat, // New state for chat mode
}

#[derive(Deserialize)]
pub struct ContentBlock {
    pub text: String,
}

pub struct ScreenRecord {
    pub timestamp: DateTime<Local>,
    pub text: String,
}
