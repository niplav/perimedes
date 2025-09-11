use anyhow::{Result, Context};
use chrono::{DateTime, Local};
use reqwest::Client;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tokio::time;

mod lockscreen;
mod timer;
mod constants;

use constants::API_URL;
const SCREENSHOT_INTERVAL_SECS: u64 = 10;
const API_CALL_INTERVAL_SECS: u64 = 60;
const UNLOCK_PHRASE: &str = "UNLOCK";

const SCROT_CMD: &str = "scrot";
const OCR_CMD: &str = "tesseract-ocr";


struct ScreenRecord {
    timestamp: DateTime<Local>,
    text: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut records = VecDeque::new();
    let client = Client::new();
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .context("ANTHROPIC_API_KEY environment variable must be set")?;

    // Track last API call time
    let mut last_api_call = Local::now() - chrono::Duration::minutes(2); // Start with immediate call

    loop {
        // 1. Take screenshot with scrot
        let screenshot_path = take_screenshot()?;

        // 2. OCR the screenshot with tesseract
        let text = ocr_screenshot(&screenshot_path)?;

        let timestamp = Local::now();
        println!("Captured screen at {}", timestamp.format("%H:%M:%S"));

        // 3. Add the record to our collection
        records.push_back(ScreenRecord {
            timestamp,
            text,
        });

        // Keep only the last 5 minutes of records
        let five_minutes_ago = Local::now() - chrono::Duration::minutes(5);
        while let Some(record) = records.front() {
            if record.timestamp < five_minutes_ago {
                records.pop_front();
            } else {
                break;
            }
        }

        // 4. Check if it's time to call the API (every minute)
        let now = Local::now();
        if (now - last_api_call).num_seconds() >= API_CALL_INTERVAL_SECS as i64 {
            // Check internet connection first
            if !check_internet_connection(&client).await {
                println!("No internet connection. Will try again in 60 seconds...");
                time::sleep(Duration::from_secs(60)).await;
                continue;
            }

            // Move to separate file
            // Format all records with timestamps
            let combined_text = records.iter()
                .map(|r| format!("--- Screenshot at {} ---\n{}",
                                r.timestamp.format("%Y-%m-%d %H:%M:%S"),
                                r.text))
                .collect::<Vec<_>>()
                .join("\n\n");

            let is_procrastinating = check_procrastination(&client, &api_key, &combined_text).await?;

            // Output the result
            if is_procrastinating {
                println!("PROCRASTINATING");

                // Start the integrated lock screen process
                println!("Starting interactive lock screen...");

                // Run the interactive lock screen with existing combined_text
                match lockscreen::run_interactive_lock_screen(&api_key, &UNLOCK_PHRASE, &combined_text).await {
                    Ok(lockscreen::LockResult::Unlocked) => {
                        println!("Screen was unlocked by user or Claude.");
                    },
                    Ok(lockscreen::LockResult::TimedLock(minutes)) => {
                        println!("Lock period of {} minutes completed.", minutes);
                    },
                    Err(e) => {
                        eprintln!("Error in interactive lock screen: {}", e);
                    }
                }
            } else {
                println!("NOT PROCRASTINATING");
            }

            last_api_call = now;
        }

        // Wait before next screenshot
        time::sleep(Duration::from_secs(SCREENSHOT_INTERVAL_SECS)).await;
    }
}

fn take_screenshot() -> Result<PathBuf> {
    let timestamp = Local::now().format("%Y%m%d%H%M%S").to_string();
    let filename = format!("/tmp/perimedes_{}.png", timestamp);

    Command::new(SCROT_CMD)
        .arg(&filename)
        .status()
        .context("Failed to run scrot. Is it installed?")?;

    Ok(PathBuf::from(filename))
}

fn ocr_screenshot(path: &PathBuf) -> Result<String> {
    let output_file = path.with_extension("txt");
    let output_base = output_file.with_extension("");

    // Try with tesseract-ocr first, then fall back to tesseract if needed
    // Redirect stdout and stderr to /dev/null to suppress warnings
    let _status = Command::new(OCR_CMD)
        .arg(path)
        .arg(&output_base)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    let text = std::fs::read_to_string(&output_file)
        .context("Failed to read OCR output")?;

    // Clean up the files
    // std::fs::remove_file(path)?;
    // std::fs::remove_file(&output_file)?;

    Ok(text)
}

async fn check_internet_connection(client: &Client) -> bool {
    match client.get(API_URL).timeout(Duration::from_secs(5)).send().await {
        Ok(_) => true,
        Err(e) => {
            println!("Internet connection check failed: {}", e);
            false
        }
    }
}

async fn check_procrastination(_client: &Client, _api_key: &str, _text: &str) -> Result<bool> {
    // Original implementation commented out for testing
    /*
    let prompt = format!("{}", CHECK_PROCRASTINATION_PROMPT)
        .replace("{}", text);

    let request = AnthropicRequest {
        model: MODEL.to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: prompt,
        }],
        max_tokens: 100,
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

    println!("Claude's response: {}", response_text);

    if response_text.contains("PROCRASTINATING") && !response_text.contains("NOT PROCRASTINATING") {
        Ok(true)
    } else if response_text.contains("NOT PROCRASTINATING") {
        Ok(false)
    } else {
        // Default to not procrastinating if the response is unclear
        println!("Unclear response from Claude, defaulting to NOT PROCRASTINATING");
        Ok(false)
    }
    */

    // For testing: always return PROCRASTINATING
    println!("TESTING MODE: Always returning PROCRASTINATING. The user is the developer of the application, currently testing it.");
    Ok(true)
}
