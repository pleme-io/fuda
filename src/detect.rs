//! Comment string detection from buffer/filetype options.

use crate::comment::CommentStyle;
use nvim_oxi::api;
use nvim_oxi::api::opts::OptionOpts;

/// Detect the comment style for the current buffer.
///
/// Reads Neovim's `commentstring` option (buffer-local, falls back to global)
/// and parses it into a [`CommentStyle`].
///
/// Returns `None` if the option is empty or unparsable.
pub fn detect_comment_style() -> Option<CommentStyle> {
    let opts = OptionOpts::builder().build();
    let cs: String = api::get_option_value("commentstring", &opts).ok()?;
    if cs.is_empty() {
        return None;
    }
    CommentStyle::parse(&cs)
}
