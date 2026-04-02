use std::io::Write;
use std::process::{Command, Stdio};

/// Copies `text` to the system clipboard.
/// Returns `true` on success. Silently fails if no clipboard tool is available.
pub fn copy(text: &str) -> bool {
    // Try each platform tool in order of preference.
    for argv in clipboard_commands() {
        if try_copy(argv, text) {
            return true;
        }
    }
    false
}

fn try_copy(argv: &[&str], text: &str) -> bool {
    let (prog, args) = argv.split_first().expect("empty argv");
    let Ok(mut child) = Command::new(prog)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }
    child.wait().map(|s| s.success()).unwrap_or(false)
}

fn clipboard_commands() -> &'static [&'static [&'static str]] {
    #[cfg(target_os = "macos")]
    return &[&["pbcopy"]];

    #[cfg(target_os = "windows")]
    return &[&["clip"]];

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return &[
        &["xclip", "-selection", "clipboard"],
        &["xsel", "--clipboard", "--input"],
        &["wl-copy"],
    ];
}
