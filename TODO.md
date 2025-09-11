* Track window titles
* Track current window content and screen content separately
* Chat with Haiku in case of procrastination lock
* Screen is unlocked but needs second enter to be accessible again.
* Send Claude the context from records in main.rs
* Actually print claude outputs in the locked screen
* Make timer work
* Give Claude ability to lock screen for a number of seconds, not just minutes.
* Fix spacing between lines for Claude response
* Make the lock screen full screen, no movable mouse
* Move prompt to separate editable file? Or config header?
	* Config header is probably better, given suckless philosophy & editability
* Move prompt from chat.rs to header
* Change to xbcommon for more complex key handling:
	* Why: This is the standard, modern library specifically designed to translate keycodes + modifier state into keysyms and UTF-8 strings based on the user's active keyboard layout. It's exactly the piece you're missing.
	* How:
		* Initialize an xkbcommon context and keymap (often derived from the X11 connection/setup).
		* Create and maintain an xkbcommon keyboard state machine.
		* When you receive a raw KeyPressEvent from x11rb, feed the keycode and modifier state into the xkbcommon state machine (e.g., xkb_state_update_key).
		* Use functions like xkb_state_key_get_utf8 to get the resulting character string for the pressed key under the current layout and modifier state.
		* Append this character to your displayed input buffer.
	* Fit: This directly integrates with your x11rb raw events and provides the rich translation needed. It's the most appropriate and "correct" way.
