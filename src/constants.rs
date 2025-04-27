// Constants shared across multiple modules

// X11 font
pub const FONT_NAME: &str = "-misc-fixed-medium-r-normal--13-120-75-75-c-70-iso8859-1";

// Colors for the lock screen
pub const BG_COLOR: u32 = 0x282828; // Dark gray background
pub const INPUT_COLOR: u32 = 0x444444; // Lighter gray when typing
pub const FAILED_COLOR: u32 = 0xcc0000; // Red on failed unlock attempt
pub const TEXT_COLOR: u32 = 0xebdbb2; // Light text color
pub const SYSTEM_COLOR: u32 = 0xfabd2f; // Yellow for system messages
pub const USER_COLOR: u32 = 0x83a598; // Blue for user messages
pub const ASSISTANT_COLOR: u32 = 0xb8bb26; // Green for assistant messages

// API constants
pub const API_URL: &str = "https://api.anthropic.com/v1/messages";
pub const MAX_MESSAGES: usize = 4;
pub const MIN_LOCK_MINUTES: u64 = 1;
pub const MAX_LOCK_MINUTES: u64 = 10;

// X11 keysym constants for special keys
pub mod keysym {
    pub const SPACE: u32 = 0x20;
    pub const ENTER: u32 = 0xff0d;
    pub const ESCAPE: u32 = 0xff1b;
    pub const BACKSPACE: u32 = 0xff08;
}