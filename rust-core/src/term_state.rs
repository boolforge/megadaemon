//! Terminal ANSI/VT Screen State Parser via `vt100` (PoC 6).
//!
//! Maintains an in-memory rendered terminal screen with bounded scrollback
//! and clean snapshot extraction for agent_coding and cockpit UIs.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenSnapshot {
    pub rows: u16,
    pub cols: u16,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub contents: String,
}

/// Parses raw terminal bytes (including ANSI/VT escape codes) into a rendered snapshot.
pub fn render_vt100_snapshot(raw_bytes: &[u8], rows: u16, cols: u16) -> ScreenSnapshot {
    let mut parser = vt100::Parser::new(rows.max(1), cols.max(1), 0);
    parser.process(raw_bytes);
    let screen = parser.screen();
    let (cursor_row, cursor_col) = screen.cursor_position();
    ScreenSnapshot {
        rows,
        cols,
        cursor_row,
        cursor_col,
        contents: screen.contents(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vt100_strips_ansi_and_tracks_cursor() {
        let snap = render_vt100_snapshot(b"\x1b[32mOK\x1b[0m\r\nready>", 24, 80);
        assert!(snap.contents.contains("OK"));
        assert!(snap.contents.contains("ready>"));
        assert_eq!(snap.cursor_row, 1);
        assert_eq!(snap.cursor_col, 6);
    }
}
