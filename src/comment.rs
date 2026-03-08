//! Core comment toggling logic.
//!
//! Parses Neovim's `commentstring` format (e.g., `// %s`, `/* %s */`) and
//! provides pure-function toggling of line and block comments.

/// Parsed comment delimiters extracted from `commentstring`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentStyle {
    /// The prefix placed before the comment body (e.g., `//` or `/*`).
    pub left: String,
    /// The suffix placed after the comment body (e.g., `*/`), empty for line comments.
    pub right: String,
}

impl CommentStyle {
    /// Parse a Neovim `commentstring` value into delimiters.
    ///
    /// The format is `<left>%s<right>`, where `%s` marks where the text goes.
    /// Examples: `// %s`, `/* %s */`, `# %s`, `-- %s`.
    ///
    /// Returns `None` if the format is unrecognized (no `%s` placeholder).
    #[must_use]
    pub fn parse(commentstring: &str) -> Option<Self> {
        let idx = commentstring.find("%s")?;
        let left = commentstring[..idx].trim_end().to_string();
        let right = commentstring[idx + 2..].trim_start().to_string();

        if left.is_empty() {
            return None;
        }

        Some(Self { left, right })
    }

    /// Whether this is a line-style comment (no closing delimiter).
    #[must_use]
    pub fn is_line_style(&self) -> bool {
        self.right.is_empty()
    }
}

/// Determine the minimum indentation level across a set of non-empty lines.
///
/// Returns the number of leading whitespace characters of the least-indented
/// non-empty line. Empty/whitespace-only lines are ignored.
#[must_use]
pub fn min_indent(lines: &[&str]) -> usize {
    lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0)
}

/// Check whether a single line is commented with the given style at the specified indent.
#[must_use]
fn is_line_commented(line: &str, style: &CommentStyle, indent: usize) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return true; // blank lines are considered "commented" to avoid toggling artifacts
    }

    let after_indent = &line[indent..];
    let after_indent = after_indent.trim_start_matches(' ');

    if !after_indent.starts_with(&style.left) {
        return false;
    }

    if style.is_line_style() {
        return true;
    }

    // For block-style used as line comments, check for closing delimiter too.
    trimmed.ends_with(&style.right)
}

/// Check whether ALL non-empty lines in the range are already commented.
#[must_use]
pub fn all_commented(lines: &[&str], style: &CommentStyle) -> bool {
    let indent = min_indent(lines);
    lines.iter().all(|line| is_line_commented(line, style, indent))
}

/// Toggle line comments on a set of lines.
///
/// If all non-empty lines are already commented, removes the comment prefix
/// (and suffix for block-style line comments). Otherwise, adds the comment
/// prefix to every line at the minimum indentation level.
///
/// Blank lines are left blank (not prefixed with a comment marker).
#[must_use]
pub fn toggle_lines(lines: &[&str], style: &CommentStyle) -> Vec<String> {
    if lines.is_empty() {
        return Vec::new();
    }

    let indent = min_indent(lines);
    let commenting = !all_commented(lines, style);

    lines
        .iter()
        .map(|&line| {
            if commenting {
                comment_line(line, style, indent)
            } else {
                uncomment_line(line, style, indent)
            }
        })
        .collect()
}

/// Add a comment to a single line at the given indentation level.
fn comment_line(line: &str, style: &CommentStyle, indent: usize) -> String {
    if line.trim().is_empty() {
        return line.to_string();
    }

    let prefix = &line[..indent];
    let content = &line[indent..];

    if style.is_line_style() {
        format!("{prefix}{} {content}", style.left)
    } else {
        format!("{prefix}{} {content} {}", style.left, style.right)
    }
}

/// Remove a comment from a single line.
fn uncomment_line(line: &str, style: &CommentStyle, indent: usize) -> String {
    if line.trim().is_empty() {
        return line.to_string();
    }

    let prefix = &line[..indent];
    let after_indent = &line[indent..];

    // Strip the left delimiter.
    let stripped = after_indent
        .strip_prefix(&style.left)
        .unwrap_or(after_indent);

    // Strip optional single space after the left delimiter.
    let stripped = stripped.strip_prefix(' ').unwrap_or(stripped);

    // Strip the right delimiter if present.
    let stripped = if !style.is_line_style() {
        let s = stripped.trim_end();
        let s = s.strip_suffix(&style.right).unwrap_or(s);
        // Strip optional single space before the right delimiter.
        let s = s.strip_suffix(' ').unwrap_or(s);
        s
    } else {
        stripped
    };

    format!("{prefix}{stripped}")
}

/// Wrap a range of lines in a block comment.
///
/// Adds `/* ... */` style wrapping around the entire block rather than
/// per-line commenting.
#[must_use]
pub fn toggle_block(lines: &[&str], style: &CommentStyle) -> Vec<String> {
    if lines.is_empty() {
        return Vec::new();
    }

    // For line-style comments, fall back to line toggling.
    if style.is_line_style() {
        return toggle_lines(lines, style);
    }

    let indent = min_indent(lines);
    let indent_str: String = " ".repeat(indent);

    // Check if already block-commented: first non-empty line starts with left,
    // last non-empty line ends with right.
    let first_content = lines.iter().find(|l| !l.trim().is_empty());
    let last_content = lines.iter().rev().find(|l| !l.trim().is_empty());

    if let (Some(first), Some(last)) = (first_content, last_content) {
        let first_trimmed = first[indent..].trim_start();
        let last_trimmed = last.trim_end();
        if first_trimmed.starts_with(&style.left) && last_trimmed.ends_with(&style.right) {
            // Uncomment block.
            return uncomment_block(lines, style, indent);
        }
    }

    // Comment block: wrap with delimiters.
    let mut result = Vec::with_capacity(lines.len() + 2);
    result.push(format!("{indent_str}{}", style.left));
    for &line in lines {
        result.push(line.to_string());
    }
    result.push(format!("{indent_str}{}", style.right));
    result
}

/// Remove block comment wrapping.
fn uncomment_block(lines: &[&str], style: &CommentStyle, indent: usize) -> Vec<String> {
    if lines.len() <= 1 {
        // Single line with both delimiters: `/* content */` -> `content`
        if let Some(&line) = lines.first() {
            let after_indent = &line[indent..];
            let stripped = after_indent
                .strip_prefix(&style.left)
                .unwrap_or(after_indent);
            let stripped = stripped.strip_prefix(' ').unwrap_or(stripped);
            let stripped = stripped.trim_end();
            let stripped = stripped.strip_suffix(&style.right).unwrap_or(stripped);
            let stripped = stripped.strip_suffix(' ').unwrap_or(stripped);
            let prefix = &line[..indent];
            return vec![format!("{prefix}{stripped}")];
        }
        return Vec::new();
    }

    let mut result = Vec::new();

    for (i, &line) in lines.iter().enumerate() {
        if i == 0 {
            // First line: remove left delimiter.
            let after_indent = &line[indent..];
            let stripped = after_indent
                .strip_prefix(&style.left)
                .unwrap_or(after_indent);
            let stripped = stripped.strip_prefix(' ').unwrap_or(stripped);
            let prefix = &line[..indent];
            let content = format!("{prefix}{stripped}");
            if !content.trim().is_empty() {
                result.push(content);
            }
        } else if i == lines.len() - 1 {
            // Last line: remove right delimiter.
            let trimmed = line.trim_end();
            let stripped = trimmed.strip_suffix(&style.right).unwrap_or(trimmed);
            let stripped = stripped.strip_suffix(' ').unwrap_or(stripped);
            if !stripped.trim().is_empty() {
                result.push(stripped.to_string());
            }
        } else {
            result.push(line.to_string());
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- CommentStyle::parse tests ---

    #[test]
    fn parse_line_comment_double_slash() {
        let style = CommentStyle::parse("// %s").unwrap();
        assert_eq!(style.left, "//");
        assert_eq!(style.right, "");
        assert!(style.is_line_style());
    }

    #[test]
    fn parse_line_comment_hash() {
        let style = CommentStyle::parse("# %s").unwrap();
        assert_eq!(style.left, "#");
        assert_eq!(style.right, "");
    }

    #[test]
    fn parse_line_comment_double_dash() {
        let style = CommentStyle::parse("-- %s").unwrap();
        assert_eq!(style.left, "--");
        assert_eq!(style.right, "");
    }

    #[test]
    fn parse_block_comment_c_style() {
        let style = CommentStyle::parse("/* %s */").unwrap();
        assert_eq!(style.left, "/*");
        assert_eq!(style.right, "*/");
        assert!(!style.is_line_style());
    }

    #[test]
    fn parse_block_comment_html() {
        let style = CommentStyle::parse("<!-- %s -->").unwrap();
        assert_eq!(style.left, "<!--");
        assert_eq!(style.right, "-->");
    }

    #[test]
    fn parse_no_placeholder_returns_none() {
        assert!(CommentStyle::parse("//").is_none());
    }

    #[test]
    fn parse_empty_left_returns_none() {
        assert!(CommentStyle::parse("%s -->").is_none());
    }

    #[test]
    fn parse_percent_s_no_spaces() {
        let style = CommentStyle::parse("//%s").unwrap();
        assert_eq!(style.left, "//");
        assert_eq!(style.right, "");
    }

    #[test]
    fn parse_semicolon_comment() {
        let style = CommentStyle::parse("; %s").unwrap();
        assert_eq!(style.left, ";");
        assert_eq!(style.right, "");
    }

    // --- min_indent tests ---

    #[test]
    fn min_indent_uniform() {
        let lines = vec!["    foo", "    bar", "    baz"];
        assert_eq!(min_indent(&lines), 4);
    }

    #[test]
    fn min_indent_mixed() {
        let lines = vec!["  foo", "    bar", "      baz"];
        assert_eq!(min_indent(&lines), 2);
    }

    #[test]
    fn min_indent_with_blank_lines() {
        let lines = vec!["    foo", "", "    bar"];
        assert_eq!(min_indent(&lines), 4);
    }

    #[test]
    fn min_indent_no_indent() {
        let lines = vec!["foo", "bar"];
        assert_eq!(min_indent(&lines), 0);
    }

    #[test]
    fn min_indent_all_blank() {
        let lines: Vec<&str> = vec!["", "   ", ""];
        assert_eq!(min_indent(&lines), 0);
    }

    // --- toggle_lines tests (line-style comments) ---

    #[test]
    fn toggle_adds_line_comments() {
        let style = CommentStyle::parse("// %s").unwrap();
        let lines = vec!["    foo", "    bar"];
        let result = toggle_lines(&lines, &style);
        assert_eq!(result, vec!["    // foo", "    // bar"]);
    }

    #[test]
    fn toggle_removes_line_comments() {
        let style = CommentStyle::parse("// %s").unwrap();
        let lines = vec!["    // foo", "    // bar"];
        let result = toggle_lines(&lines, &style);
        assert_eq!(result, vec!["    foo", "    bar"]);
    }

    #[test]
    fn toggle_mixed_comments_and_uncomments() {
        // When not all lines are commented, should add comments to all.
        let style = CommentStyle::parse("// %s").unwrap();
        let lines = vec!["    foo", "    // bar"];
        let result = toggle_lines(&lines, &style);
        assert_eq!(result, vec!["    // foo", "    // // bar"]);
    }

    #[test]
    fn toggle_preserves_blank_lines() {
        let style = CommentStyle::parse("// %s").unwrap();
        let lines = vec!["    foo", "", "    bar"];
        let result = toggle_lines(&lines, &style);
        assert_eq!(result, vec!["    // foo", "", "    // bar"]);
    }

    #[test]
    fn toggle_hash_comments() {
        let style = CommentStyle::parse("# %s").unwrap();
        let lines = vec!["def hello:", "    pass"];
        let result = toggle_lines(&lines, &style);
        // min_indent is 0, so comment marker is placed at column 0;
        // inner indentation of "    pass" is preserved.
        assert_eq!(result, vec!["# def hello:", "#     pass"]);
    }

    #[test]
    fn toggle_single_line() {
        let style = CommentStyle::parse("// %s").unwrap();
        let lines = vec!["let x = 5;"];
        let result = toggle_lines(&lines, &style);
        assert_eq!(result, vec!["// let x = 5;"]);
    }

    #[test]
    fn toggle_uncomment_single_line() {
        let style = CommentStyle::parse("// %s").unwrap();
        let lines = vec!["// let x = 5;"];
        let result = toggle_lines(&lines, &style);
        assert_eq!(result, vec!["let x = 5;"]);
    }

    #[test]
    fn toggle_empty_input() {
        let style = CommentStyle::parse("// %s").unwrap();
        let lines: Vec<&str> = vec![];
        let result = toggle_lines(&lines, &style);
        assert!(result.is_empty());
    }

    #[test]
    fn toggle_block_style_as_line_comment() {
        // When commentstring is block-style, line toggle wraps each line.
        let style = CommentStyle::parse("/* %s */").unwrap();
        let lines = vec!["    foo", "    bar"];
        let result = toggle_lines(&lines, &style);
        assert_eq!(result, vec!["    /* foo */", "    /* bar */"]);
    }

    #[test]
    fn toggle_uncomment_block_style_line() {
        let style = CommentStyle::parse("/* %s */").unwrap();
        let lines = vec!["    /* foo */", "    /* bar */"];
        let result = toggle_lines(&lines, &style);
        assert_eq!(result, vec!["    foo", "    bar"]);
    }

    // --- toggle_block tests ---

    #[test]
    fn toggle_block_adds_wrapper() {
        let style = CommentStyle::parse("/* %s */").unwrap();
        let lines = vec!["    foo", "    bar"];
        let result = toggle_block(&lines, &style);
        assert_eq!(
            result,
            vec!["    /*", "    foo", "    bar", "    */"]
        );
    }

    #[test]
    fn toggle_block_removes_wrapper() {
        let style = CommentStyle::parse("/* %s */").unwrap();
        let lines = vec!["    /*", "    foo", "    bar", "    */"];
        let result = toggle_block(&lines, &style);
        assert_eq!(result, vec!["    foo", "    bar"]);
    }

    #[test]
    fn toggle_block_single_line_wrap() {
        let style = CommentStyle::parse("/* %s */").unwrap();
        let lines = vec!["    foo"];
        let result = toggle_block(&lines, &style);
        assert_eq!(result, vec!["    /*", "    foo", "    */"]);
    }

    #[test]
    fn toggle_block_single_line_unwrap() {
        let style = CommentStyle::parse("/* %s */").unwrap();
        let lines = vec!["    /* foo */"];
        let result = toggle_block(&lines, &style);
        assert_eq!(result, vec!["    foo"]);
    }

    #[test]
    fn toggle_block_falls_back_for_line_style() {
        let style = CommentStyle::parse("// %s").unwrap();
        let lines = vec!["    foo", "    bar"];
        let result = toggle_block(&lines, &style);
        // Should behave like toggle_lines for line-style comments.
        assert_eq!(result, vec!["    // foo", "    // bar"]);
    }

    #[test]
    fn toggle_block_html() {
        let style = CommentStyle::parse("<!-- %s -->").unwrap();
        let lines = vec!["  <div>hello</div>"];
        let result = toggle_block(&lines, &style);
        assert_eq!(
            result,
            vec!["  <!--", "  <div>hello</div>", "  -->"]
        );
    }

    // --- Roundtrip tests ---

    #[test]
    fn roundtrip_line_comment() {
        let style = CommentStyle::parse("// %s").unwrap();
        let original = vec!["    let x = 1;", "    let y = 2;"];
        let commented = toggle_lines(&original, &style);
        let uncommented: Vec<String> = toggle_lines(
            &commented.iter().map(String::as_str).collect::<Vec<_>>(),
            &style,
        );
        assert_eq!(
            uncommented,
            original.iter().map(|s| (*s).to_string()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn roundtrip_hash_comment() {
        let style = CommentStyle::parse("# %s").unwrap();
        let original = vec!["def foo:", "    return 1"];
        let commented = toggle_lines(&original, &style);
        let uncommented: Vec<String> = toggle_lines(
            &commented.iter().map(String::as_str).collect::<Vec<_>>(),
            &style,
        );
        assert_eq!(
            uncommented,
            original.iter().map(|s| (*s).to_string()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn roundtrip_block_comment_per_line() {
        let style = CommentStyle::parse("/* %s */").unwrap();
        let original = vec!["    int x = 1;", "    int y = 2;"];
        let commented = toggle_lines(&original, &style);
        let uncommented: Vec<String> = toggle_lines(
            &commented.iter().map(String::as_str).collect::<Vec<_>>(),
            &style,
        );
        assert_eq!(
            uncommented,
            original.iter().map(|s| (*s).to_string()).collect::<Vec<_>>()
        );
    }

    // --- Edge case tests ---

    #[test]
    fn comment_preserves_trailing_whitespace() {
        let style = CommentStyle::parse("// %s").unwrap();
        let lines = vec!["    foo   "];
        let result = toggle_lines(&lines, &style);
        assert_eq!(result, vec!["    // foo   "]);
    }

    #[test]
    fn uncomment_without_space_after_delimiter() {
        let style = CommentStyle::parse("// %s").unwrap();
        let lines = vec!["    //foo"];
        let result = toggle_lines(&lines, &style);
        assert_eq!(result, vec!["    foo"]);
    }

    #[test]
    fn all_commented_with_blank() {
        let style = CommentStyle::parse("// %s").unwrap();
        let lines = vec!["    // foo", "", "    // bar"];
        assert!(all_commented(&lines, &style));
    }

    #[test]
    fn not_all_commented() {
        let style = CommentStyle::parse("// %s").unwrap();
        let lines = vec!["    // foo", "    bar"];
        assert!(!all_commented(&lines, &style));
    }
}
