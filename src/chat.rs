// For screen-based chat
#[derive(Clone)]
pub enum ChatMessage {
    System(String),
    User(String),
    Assistant(String),
    Decision(String),
}

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const MODEL: &str = "claude-3-7-sonnet-20250219";
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
of minutes between 1 and 2.";

pub enum LockDecision {
    Unlock,
    Lock(u64), // Lock for specified minutes
}
