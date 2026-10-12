//! System clipboard access shared by text fields and invite buttons.
//! Miniquad's macOS clipboard is a stub, so use the native pasteboard via arboard.

pub fn get() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.get_text())
            .map_err(|error| format!("Could not paste clipboard text: {error}"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        macroquad::miniquad::window::clipboard_get()
            .ok_or_else(|| "The clipboard does not contain text.".into())
    }
}

pub fn set(text: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_text(text))
            .map_err(|error| format!("Could not copy text: {error}"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        macroquad::miniquad::window::clipboard_set(text);
        Ok(())
    }
}

pub fn field_text(text: &str, limit: usize) -> String {
    text.trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pasted_invite_is_single_line_and_trimmed() {
        assert_eq!(
            field_text(" \r\nuno://localhost:34197/abc\t\n", 512),
            "uno://localhost:34197/abc"
        );
        assert_eq!(field_text("ab\tcd\nef", 4), "abcd");
    }

    #[test]
    fn pasted_text_limit_counts_characters_not_bytes() {
        assert_eq!(field_text("é🙂abc", 3), "é🙂a");
        assert_eq!(field_text("abc", 0), "");
    }

    // Explicit opt-in: this test changes the system clipboard. No window needed.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "uses the system clipboard; run manually on macOS"]
    fn native_clipboard_roundtrip() {
        let previous = get().ok();
        let expected = "uno://localhost:34197/0123456789abcdef0123456789abcdef";
        set(expected).unwrap();
        let actual = get();
        set(previous.as_deref().unwrap_or_default()).unwrap();
        assert_eq!(actual.unwrap(), expected);
    }
}
