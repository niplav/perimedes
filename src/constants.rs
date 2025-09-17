// Constants shared across multiple modules

// X11 font options (uncomment the one you want)
pub const FONT_NAME: &str = "-misc-fixed-medium-r-normal--18-180-75-75-c-90-iso8859-1"; // Large (18px)
// pub const FONT_NAME: &str = "-misc-fixed-medium-r-normal--15-150-75-75-c-80-iso8859-1"; // Medium (15px)
// pub const FONT_NAME: &str = "-misc-fixed-medium-r-normal--13-120-75-75-c-70-iso8859-1"; // Small (13px, original)

// Colors for the lock screen
pub const BG_COLOR: u32 = 0x282828; // Dark gray background
pub const TEXT_COLOR: u32 = 0xebdbb2; // Light text color
pub const SYSTEM_COLOR: u32 = 0xfabd2f; // Yellow for system messages
pub const USER_COLOR: u32 = 0x83a598; // Blue for user messages
pub const ASSISTANT_COLOR: u32 = 0xb8bb26; // Green for assistant messages

// API constants
pub const API_URL: &str = "https://api.anthropic.com/v1/messages";
pub const MAX_MESSAGES: usize = 4;
pub const MIN_LOCK_MINUTES: u64 = 1;
pub const MAX_LOCK_MINUTES: u64 = 10;

pub const SCREENSHOT_INTERVAL_SECS: u64 = 10;
pub const API_CALL_INTERVAL_SECS: u64 = 60;
pub const UNLOCK_PHRASE: &str = "UNLOCK";

pub const SCROT_CMD: &str = "scrot";
pub const OCR_CMD: &str = "tesseract-ocr";

// Models
pub const PROCRASTINATION_MODEL: &str = "claude-3-5-haiku-20241022";
pub const JUDGE_MODEL: &str = "claude-3-5-haiku-20241022";

// Prompts
pub const CHECK_PROCRASTINATION_PROMPT: &str = "Here is text extracted from my computer screen over the past 5 minutes. \
Based only on this text, am I procrastinating or working productively? \
First, reason through the content; common patterns of procrastination are: \
* Spending lots of time scrolling through twitter, LessWrong, the EA Forum, lobste.rs, Hacker News, reddit and reading random blogposts \
* Watching YouTube videos \
 \
Non-cases of procrastination are: \
 \
* Responding to WhatsApp/Telegram/Signal messages \
 \
Finally respond, in a single line, with either exactly 'PROCRASTINATING' \
or exactly 'NOT PROCRASTINATING', depending on the previous \
reasoning.\n\n{}";

pub const JUDGE_PROMPT: &str = "You are a productivity enforcer. Your job is to \
decide whether to unlock the user's screen or keep it locked for another \
1-10 minutes. The user's screen was locked because they were detected \
to be procrastinating. Ask them about what they were doing and what they \
intend to do if unlocked. \
\
First, reason through the content; common patterns of procrastination are: \
* Spending lots of time scrolling through twitter, LessWrong, the EA Forum, lobste.rs, Hacker News, reddit and reading random blogposts \
* Watching YouTube videos \
\
Non-cases of procrastination are: \
\
* Responding to WhatsApp/Telegram/Signal messages \
\
The conversation will last at most 4 messages, after which you MUST make \
a decision. If you decide to unlock, respond with exactly 'UNLOCK'. If \
you decide to keep it locked, respond with 'LOCK:X' where X is a number \
of minutes between 1 and 10.";

// X11 keysym constants for special keys
pub mod keysym {
    pub const SPACE: u32 = 0x20;
    pub const ENTER: u32 = 0xff0d;
    pub const ESCAPE: u32 = 0xff1b;
    pub const BACKSPACE: u32 = 0xff08;
}