# Perimedes: Anti-Procrastination Tool

## Overview

Perimedes is an LLM-powered productivity tool that monitors computer usage and proactively blocks access when it detects procrastination. Named after the loyal companion who helped bind Odysseus to the mast to resist the Sirens' call, Perimedes enforces the user's pre-committed intention to work against momentary temptations.

> "They sang these words most musically, and as I longed to hear them
further I made signs by frowning to my men that they should set me free;
but they quickened their stroke, and Eurylochus and Perimedes bound me
with still stronger bonds till we had got out of hearing of the Sirens'
voices."

— *The Odyssey*

## Technical Implementation

### Approach Options

1. **Kernel Module**
   - **Pros**: Extremely difficult to circumvent
   - **Cons**: Complex development, stability risks, security concerns, difficult debugging
   - **Verdict**: Overkill for most implementations

2. **Root-Level Service** (Recommended)
   - **Pros**: Strong circumvention resistance, simpler implementation, better stability
   - **Components**:
     - Core service running as root
     - Potential watchdog process to restart if killed
     - System-level screen locking
   - **Circumvention Prevention**:
     - Running with setuid root privileges
     - Capturing keyboard/mouse input
     - Watchdog process for automatic restart
     - Time-delayed disabling commands (cooling-off period)

### Core Components

1. **Screen Blocking Mechanism**
   - Full-screen X11 window with override_redirect flag
   - Keyboard and mouse input capture
   - Text input field for explanations to the LLM

2. **LLM Integration**
   - Initial monitoring with cheaper model (optional)
   - Conversation interface with more capable model to evaluate unlocking requests
   - Decision protocol with clear "UNLOCK:YES/NO" format

3. **Anti-Circumvention Techniques**
   - Root privileges for core process
   - Lock file to prevent multiple instances
   - Minimum mandatory blocking time
   - Signal handling to prevent easy termination

## Rust Implementation

Rust is the ideal language for Perimedes due to its strong safety guarantees, which are critical for a security-sensitive application running with elevated privileges.

### Key Benefits of Rust

1. **Memory safety** - Prevents buffer overflows, use-after-free, and data races without runtime overhead
2. **Ownership model** - Ensures thread safety and prevents common concurrency bugs
3. **Modern ecosystem** - Rich libraries for X11, HTTP clients, JSON parsing
4. **Error handling** - Pattern matching and Result type for robust error management
5. **Security** - Fewer potential vulnerabilities in privileged code

### Key Libraries

```rust
// Core dependencies
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use tokio::sync::mpsc;
use clap::Parser;
use log::{info, error, warn};
```

### Core Architecture

The Rust implementation follows a multi-module architecture:

```rust
perimedes/
├── src/
│   ├── main.rs           // Entry point and CLI handling
│   ├── config.rs         // Configuration management
│   ├── x11/              // X11 display handling
│   │   ├── mod.rs
│   │   ├── window.rs     // Blocking window implementation
│   │   └── input.rs      // Keyboard/mouse capture
│   ├── llm/              // LLM integration
│   │   ├── mod.rs
│   │   ├── client.rs     // API client
│   │   └── prompt.rs     // Prompt engineering
│   ├── monitoring/       // Usage monitoring
│   │   ├── mod.rs
│   │   ├── activity.rs   // Process/window tracking
│   │   └── screenshot.rs // Optional screenshot analysis
│   └── security.rs       // Privilege and lockfile management
└── Cargo.toml
```

### Core Functionality

```rust
// Simplified pseudocode for main blocking functionality
async fn run_blocker(config: Config) -> Result<(), Error> {
    // Initialize X11 connection
    let (conn, screen_num) = Connection::connect(None)?;

    // Create full-screen blocking window
    let window = Window::create_blocking_window(&conn, screen_num)?;

    // Capture keyboard and mouse
    input::grab_keyboard(&conn, window.id)?;

    // Record start time
    let block_start_time = Instant::now();

    // Main event loop with async LLM communication
    let (tx, mut rx) = mpsc::channel(1);

    loop {
        // Check if minimum block time has passed
        if block_start_time.elapsed() < config.min_block_time {
            // Display "taking a break" message
            window.draw_text("Taking a mandatory break...")?;
        } else {
            // Handle user input to explain to LLM
            if let Some(explanation) = window.get_user_input() {
                // Send to LLM for evaluation
                let llm_client = LlmClient::new(&config.api_key);

                match llm_client.evaluate_explanation(&explanation).await {
                    Ok(Decision::Unlock) => break, // Exit loop to unlock
                    Ok(Decision::KeepBlocked(reason)) => {
                        window.draw_text(&format!("Still blocked: {}", reason))?;
                    }
                    Err(e) => {
                        window.draw_text(&format!("Error: {}", e))?;
                    }
                }
            }
        }

        // Process X11 events
        conn.flush()?;
        while let Some(event) = conn.poll_for_event()? {
            window.handle_event(event)?;
        }
    }

    // Release resources and exit
    input::ungrab_keyboard(&conn)?;
    Ok(())
}

### Deployment Considerations

To build and install:
```bash
cargo build --release
sudo chown root:root target/release/perimedes
sudo chmod u+s target/release/perimedes
sudo cp target/release/perimedes /usr/local/bin/
```

Create a systemd service:
```
[Unit]
Description=Perimedes Anti-Procrastination Tool
After=network.target

[Service]
ExecStart=/usr/local/bin/perimedes
Restart=always
User=root

[Install]
WantedBy=multi-user.target
```

## Monitoring Options

### Screenshot-Based Monitoring
- Capture screenshots every ~5 minutes
- Process with vision-capable LLM
- Trigger blocking on detected procrastination
- Cost estimate: ~$4-5/month for basic monitoring (2200 analyses)

### Activity-Based Monitoring
- Monitor active window titles and process names
- Track application usage time
- More efficient but potentially less accurate
- Could be used as an initial filter before screenshot analysis

## Licensing

The project is available under the MIT License, which allows for both personal and commercial use while providing transparency for this privacy-sensitive tool. This openness is particularly important for a tool that monitors user activity, as it builds trust by allowing code inspection.

## Privacy Considerations

- Screenshot analysis raises significant privacy concerns
- Local-first processing preferred where possible
- Clear data handling policies required
- Open source core builds trust by allowing code inspection

## Future Enhancements

1. Advanced monitoring with website/application categorization
2. Machine learning for personalized procrastination detection
3. Cross-device synchronization (with privacy controls)
4. Productivity analytics and insights dashboard
5. Customizable intervention strategies and "strictness levels"

---

*"Higher Self As A Service"*
