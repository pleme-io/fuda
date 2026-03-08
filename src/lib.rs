//! Fuda (札) — fast comment toggling for Neovim with treesitter context awareness.
//!
//! Part of the blnvim-ng distribution — a Rust-native Neovim plugin suite.
//! Built with [`nvim-oxi`](https://github.com/noib3/nvim-oxi) for zero-cost
//! Neovim API bindings.
//!
//! # Keymaps
//!
//! | Mode   | Key   | Action                          |
//! |--------|-------|---------------------------------|
//! | Normal | `gcc` | Toggle line comment             |
//! | Visual | `gc`  | Toggle line comment (selection) |
//! | Normal | `gbc` | Toggle block comment            |

pub mod comment;
pub mod detect;

use nvim_oxi as oxi;
use tane::prelude::*;

/// Create an `oxi::Error` from a string message.
fn err(msg: impl std::fmt::Display) -> oxi::Error {
    oxi::api::Error::Other(msg.to_string()).into()
}

/// Convert a `tane::Error` into an `oxi::Error`.
fn from_tane(e: tane::Error) -> oxi::Error {
    err(e)
}

/// Convert a `tane::Error` into a local `tane::Error::Custom`.
fn to_tane(e: oxi::Error) -> tane::Error {
    tane::Error::Custom(e.to_string())
}

/// Toggle line comment on the current line (normal mode).
fn toggle_line_current() -> oxi::Result<()> {
    let style = detect::detect_comment_style()
        .ok_or_else(|| err("fuda: no commentstring set for this buffer"))?;

    let mut buf = oxi::api::get_current_buf();
    let cursor = oxi::api::get_current_win().get_cursor()?;
    let line_idx = cursor.0 as usize - 1; // cursor is 1-indexed

    let line_text = kakitori::lines::get_line(&buf, line_idx).map_err(from_tane)?;

    let toggled = comment::toggle_lines(&[line_text.as_str()], &style);
    let refs: Vec<&str> = toggled.iter().map(String::as_str).collect();

    kakitori::lines::set_lines(&mut buf, line_idx, line_idx + 1, &refs)
        .map_err(from_tane)?;

    Ok(())
}

/// Toggle line comment on a visual selection.
fn toggle_line_visual() -> oxi::Result<()> {
    let style = detect::detect_comment_style()
        .ok_or_else(|| err("fuda: no commentstring set for this buffer"))?;

    let mut buf = oxi::api::get_current_buf();

    let (start_mark, end_mark) = kakitori::marks::get_visual_range(&buf)
        .map_err(from_tane)?
        .ok_or_else(|| err("fuda: no visual selection"))?;

    let start = start_mark.line;
    let end = end_mark.line + 1; // exclusive

    let all_lines =
        kakitori::lines::get_all_lines(&buf).map_err(from_tane)?;

    let selected: Vec<&str> =
        all_lines[start..end].iter().map(String::as_str).collect();
    let toggled = comment::toggle_lines(&selected, &style);
    let refs: Vec<&str> = toggled.iter().map(String::as_str).collect();

    kakitori::lines::set_lines(&mut buf, start, end, &refs)
        .map_err(from_tane)?;

    Ok(())
}

/// Toggle block comment on the current line (normal mode).
fn toggle_block_current() -> oxi::Result<()> {
    let style = detect::detect_comment_style()
        .ok_or_else(|| err("fuda: no commentstring set for this buffer"))?;

    let mut buf = oxi::api::get_current_buf();
    let cursor = oxi::api::get_current_win().get_cursor()?;
    let line_idx = cursor.0 as usize - 1;

    let line_text =
        kakitori::lines::get_line(&buf, line_idx).map_err(from_tane)?;

    let toggled = comment::toggle_block(&[line_text.as_str()], &style);
    let refs: Vec<&str> = toggled.iter().map(String::as_str).collect();

    kakitori::lines::set_lines(&mut buf, line_idx, line_idx + 1, &refs)
        .map_err(from_tane)?;

    Ok(())
}

/// Register user commands and keymaps.
fn setup() -> oxi::Result<()> {
    // User commands that bridge keymaps to Rust callbacks.
    UserCommand::new("FudaToggleLine")
        .desc("Toggle line comment")
        .register(|_| {
            toggle_line_current().map_err(to_tane)
        })
        .map_err(from_tane)?;

    UserCommand::new("FudaToggleLineVisual")
        .desc("Toggle line comment (visual)")
        .register(|_| {
            toggle_line_visual().map_err(to_tane)
        })
        .map_err(from_tane)?;

    UserCommand::new("FudaToggleBlock")
        .desc("Toggle block comment")
        .register(|_| {
            toggle_block_current().map_err(to_tane)
        })
        .map_err(from_tane)?;

    // Keymaps.
    Keymap::normal("gcc", "<Cmd>FudaToggleLine<CR>")
        .desc("Toggle line comment")
        .register()
        .map_err(from_tane)?;

    Keymap::visual("gc", "<Esc><Cmd>FudaToggleLineVisual<CR>")
        .desc("Toggle line comment (visual)")
        .register()
        .map_err(from_tane)?;

    Keymap::normal("gbc", "<Cmd>FudaToggleBlock<CR>")
        .desc("Toggle block comment")
        .register()
        .map_err(from_tane)?;

    Ok(())
}

#[oxi::plugin]
fn fuda() -> oxi::Result<()> {
    setup()
}
