use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::sync::mpsc::{Sender, Receiver};
use std::time::{Duration, Instant};
use tokio::time;

// For screen-based chat
#[derive(Clone)]
pub enum ChatMessage {
    System(String),
    User(String),
    Assistant(String),
    Decision(String),
}

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const MODEL: &str = "claude-3-sonnet-20240229";
const MAX_MESSAGES: usize = 4;
const MIN_LOCK_MINUTES: u64 = 1;
const MAX_LOCK_MINUTES: u64 = 10;

// Initial system instruction for the LLM
const JUDGE_PROMPT: &str = "You are a productivity enforcer. Your job is to\
decide whether to unlock the user's screen or keep it locked for another\
1-10 minutes. The user's screen was locked because they were detected\
to be procrastinating. Ask them about what they were doing and what they\
intend to do if unlocked.\
\
First, reason through the content; common patterns of procrastination are:\
* Spending lots of time scrolling through twitter, LessWrong, the EA Forum, lobste.rs, Hacker News, reddit and reading random blogposts\
* Watching YouTube videos\
\
Non-cases of procrastination are:\
\
* Responding to WhatsApp/Telegram/Signal messages\
\
The conversation will last at most 4 messages, after which you MUST make\
a decision.  If you decide to unlock, respond with exactly 'UNLOCK'. If\
you decide to keep it locked, respond with 'LOCK:X' where X is a number\
of minutes between 1 and 10.";

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
}

#[derive(Serialize, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: String,
}

pub enum LockDecision {
    Unlock,
    Lock(u64), // Lock for specified minutes
}

// Screen-based chat function that works with the lock screen
pub async fn screen_chat_unlock(
    api_key: &str,
    msg_sender: Sender<ChatMessage>,
    input_receiver: Receiver<String>,
) -> Result<LockDecision> {
    // Original implementation commented out for testing
    /*
    let client = Client::new();

    // Initial system instruction for the LLM
    let system_instruction = "You are a productivity enforcer. Your job is to decide whether to unlock the user's screen \
        or keep it locked for another 1-10 minutes. The user's screen was locked because they were detected \
        to be procrastinating. Ask them about what they were doing and what they intend to do if unlocked. \
        The conversation will last at most 4 messages, after which you MUST make a decision. \
        If you decide to unlock, respond with exactly 'UNLOCK'. If you decide to keep it locked, \
        respond with 'LOCK:X' where X is a number of minutes between 1 and 10.";

    let mut conversation = vec![
        Message {
            role: "system".to_string(),
            content: system_instruction.to_string(),
        },
    ];

    // Send initial message to lock screen
    let intro_message = "Your screen has been locked due to procrastination detection.\nPlease explain why you should be allowed to continue:";
    msg_sender.send(ChatMessage::System(intro_message.to_string()))?;

    for i in 0..MAX_MESSAGES {
        // Get user input from lock screen
        let user_input = match input_receiver.recv() {
            Ok(input) => input,
            Err(_) => {
                // Channel closed, probably screen was forcibly unlocked
                return Ok(LockDecision::Lock(MIN_LOCK_MINUTES));
            }
        };

        // Add user message to conversation
        conversation.push(Message {
            role: "user".to_string(),
            content: user_input.trim().to_string(),
        });

        // Send user message to display
        msg_sender.send(ChatMessage::User(user_input.trim().to_string()))?;

        // Call Claude API
        let response = call_claude_api(&client, api_key, &conversation).await?;

        // Add Claude's response to conversation
        conversation.push(Message {
            role: "assistant".to_string(),
            content: response.clone(),
        });

        // Send Claude's response to display
        msg_sender.send(ChatMessage::Assistant(response.clone()))?;

        // Check for decision on final message or if decision made early
        if i == MAX_MESSAGES - 1 || response.contains("UNLOCK") || response.contains("LOCK:") {
            // Parse decision
            if response.contains("UNLOCK") {
                // Send unlock decision message to display
                msg_sender.send(ChatMessage::Decision("UNLOCKING SCREEN".to_string()))?;
                return Ok(LockDecision::Unlock);
            } else if response.contains("LOCK:") {
                // Extract lock duration
                let parts: Vec<&str> = response.split("LOCK:").collect();
                if parts.len() >= 2 {
                    if let Ok(minutes) = parts[1].trim().parse::<u64>() {
                        let minutes = minutes.clamp(MIN_LOCK_MINUTES, MAX_LOCK_MINUTES);
                        // Send lock decision message to display
                        msg_sender.send(ChatMessage::Decision(format!("SCREEN LOCKED FOR {} MINUTES", minutes)))?;
                        return Ok(LockDecision::Lock(minutes));
                    }
                }
                // Default to minimum lock if parsing fails
                msg_sender.send(ChatMessage::Decision(format!("SCREEN LOCKED FOR {} MINUTES", MIN_LOCK_MINUTES)))?;
                return Ok(LockDecision::Lock(MIN_LOCK_MINUTES));
            } else {
                // Default to minimum lock if format is incorrect
                msg_sender.send(ChatMessage::Decision(format!("SCREEN LOCKED FOR {} MINUTES", MIN_LOCK_MINUTES)))?;
                return Ok(LockDecision::Lock(MIN_LOCK_MINUTES));
            }
        }
    }

    // Default decision if conversation didn't result in a clear decision
    msg_sender.send(ChatMessage::Decision(format!("SCREEN LOCKED FOR {} MINUTES", MIN_LOCK_MINUTES)))?;
    Ok(LockDecision::Lock(MIN_LOCK_MINUTES))
    */

    // TESTING MODE: Simplified chat interaction that always unlocks after one message

    // Send welcome message
    let intro_message = "TESTING MODE: Your screen has been locked. Type anything to unlock.";
    msg_sender.send(ChatMessage::System(intro_message.to_string()))?;

    // Wait for any user input
    match input_receiver.recv() {
        Ok(input) => {
            // Echo back user input
            msg_sender.send(ChatMessage::User(input.trim().to_string()))?;

            // Send test response
            msg_sender.send(ChatMessage::Assistant("Thanks for testing the on-screen chat. I'll unlock your screen now.".to_string()))?;

            // Wait a moment for user to read the message
            time::sleep(Duration::from_secs(1)).await;

            // Send unlock decision
            msg_sender.send(ChatMessage::Decision("UNLOCKING SCREEN".to_string()))?;
            return Ok(LockDecision::Unlock);
        },
        Err(_) => {
            // Channel closed, probably screen was forcibly unlocked
            return Ok(LockDecision::Lock(MIN_LOCK_MINUTES));
        }
    }
}

async fn call_claude_api(client: &Client, api_key: &str, conversation: &[Message]) -> Result<String> {
    let request = AnthropicRequest {
        model: MODEL.to_string(),
        messages: conversation.to_vec(),
        max_tokens: 300,
    };

    let response = client.post(API_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&request)
        .send()
        .await
        .context("Failed to send request to Anthropic API")?;

    let response_data: AnthropicResponse = response.json().await
        .context("Failed to parse Anthropic API response")?;

    let response_text = response_data.content
        .first()
        .map(|block| block.text.trim())
        .unwrap_or("")
        .to_string();

    Ok(response_text)
}

pub async fn enforce_lock_period(minutes: u64) -> Result<()> {
    let start_time = Instant::now();
    let lock_duration = Duration::from_secs(minutes * 60);

    println!("\nScreen will remain locked for {} minutes.", minutes);

    while start_time.elapsed() < lock_duration {
        let remaining = lock_duration - start_time.elapsed();
        let remaining_minutes = (remaining.as_secs() + 59) / 60; // Round up to nearest minute

        print!("\rRemaining time: {} minutes    ", remaining_minutes);
        io::stdout().flush()?;

        // Sleep for a second before updating
        time::sleep(Duration::from_secs(1)).await;
    }

    println!("\nLock period complete. Unlocking screen.");
    Ok(())
}