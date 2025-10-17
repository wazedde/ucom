use std::borrow::Cow;
use std::fmt::Display;
use yansi::{Paint, Style};

/// Represents the level of a header.
#[allow(dead_code)]
pub enum HeaderLevel {
    H1 = 1,
    H2 = 2,
    H3 = 3,
    H4 = 4,
}

impl HeaderLevel {
    /// Returns the header level as a string.
    pub const fn as_str(&self) -> &str {
        match self {
            HeaderLevel::H1 => "#",
            HeaderLevel::H2 => "##",
            HeaderLevel::H3 => "###",
            HeaderLevel::H4 => "####",
        }
    }
}

/// A utility for printing messages to the terminal or in Markdown format.
pub enum Report {
    /// Prints to the terminal.
    Terminal,
    /// Prints in Markdown format.
    Markdown,
    /// Does not print anything.
    Null,
}

#[allow(dead_code)]
pub enum WrapMode {
    /// Wraps text to the terminal width.
    TerminalWidth,
    /// Wraps text to the given fixed width.
    FixedWidth(usize),
    /// Does not wrap text.
    None,
}

#[allow(dead_code)]
impl Report {
    /// Returns the `Report` instance based on the given condition.
    /// If the condition is true, it returns `self`, otherwise it returns `Report::Null`.
    pub const fn when(&self, condition: bool) -> &Self {
        if condition { self } else { &Report::Null }
    }

    /// Prints a header.
    pub fn header(&self, text: impl Display, level: HeaderLevel) -> &Self {
        match self {
            Report::Terminal => println!("{}", text.bold()),
            Report::Markdown => println!("{} {}\n", level.as_str(), text),
            Report::Null => {}
        }
        self
    }

    /// Prints a list item.
    pub fn list_item(&self, text: impl Display) -> &Self {
        match self {
            Report::Terminal => println!("  {text}"),
            Report::Markdown => println!("- {text}"),
            Report::Null => {}
        }
        self
    }

    /// Prints a marked item.
    pub fn marked_item(&self, text: impl Display, mark: impl Display) -> &Self {
        match self {
            Report::Terminal => println!("{mark} {text}"),
            Report::Markdown => println!("- {text}"),
            Report::Null => {}
        }
        self
    }

    /// Prints a paragraph.
    pub fn paragraph(&self, text: impl Display) -> &Self {
        match self {
            Report::Markdown | Report::Terminal => println!("{text}"),
            Report::Null => {}
        }
        self
    }

    /// Prints a blank line.
    pub fn blank_line(&self) -> &Self {
        match self {
            Report::Markdown | Report::Terminal => println!(),
            Report::Null => {}
        }
        self
    }

    /// Wraps the given text to the specified wrap mode.
    pub fn wrap_text<'a>(&self, text: &'a str, wrap_mode: WrapMode) -> Cow<'a, str> {
        match (self, &wrap_mode) {
            (Report::Terminal, WrapMode::TerminalWidth) => word_wrap_to_terminal_width(text),
            (Report::Terminal, WrapMode::FixedWidth(width)) => word_wrap(text, *width),
            (Report::Markdown, WrapMode::FixedWidth(width)) => word_wrap(text, *width),
            _ => Cow::Borrowed(text),
        }
    }
    /// Formats links in the given text.
    pub fn format_links<'a>(&self, text: &'a str) -> Cow<'a, str> {
        match self {
            Report::Terminal => convert_html_links(text, |anchor, url| format!("{anchor}: {url}")),
            Report::Markdown => {
                convert_html_links(text, |anchor, url| format!("[{anchor}]({url})"))
            }
            Report::Null => Cow::Borrowed(text),
        }
    }

    /// Formats links in the given text using the given styles.
    pub fn render_links<'a>(
        &self,
        text: &'a str,
        anchor_style: Style,
        url_style: Style,
    ) -> Cow<'a, str> {
        match self {
            Report::Terminal => convert_html_links(text, |anchor, url| {
                format!(
                    "{anchor}: {url}",
                    anchor = anchor.paint(anchor_style),
                    url = url.paint(url_style)
                )
            }),
            Report::Markdown => {
                convert_html_links(text, |anchor, url| format!("[{anchor}]({url})"))
            }
            Report::Null => Cow::Borrowed(text),
        }
    }
}

#[allow(dead_code)]
impl Report {
    pub const fn is_markdown(&self) -> bool {
        matches!(self, Report::Markdown)
    }

    pub const fn is_terminal(&self) -> bool {
        matches!(self, Report::Terminal)
    }

    pub const fn is_null(&self) -> bool {
        matches!(self, Report::Null)
    }
}

/// Converts HTML anchor tags into a custom formatted link representation.
///
/// This function takes a string containing HTML content and converts
/// any anchor tags (`<a href="...">...</a>`) using a user-supplied formatter closure.
/// The formatter receives the anchor text and URL and returns the desired output format.
///
/// Example usage:
/// - Markdown: `convert_html_links(text, |anchor, url| format!("[{}]({})", anchor, url))`
/// - Plain text: `convert_html_links(text, |anchor, url| format!("{} ({})", anchor, url))`
///
/// Note:
/// - Uses regular expressions to find and replace anchor tags.
/// - Does not handle nested anchor tags or malformed HTML.
/// - Any content outside valid anchor tags remains unmodified.
///
/// # Arguments
/// * `text` - The input HTML string.
/// * `formatter` - A closure that takes anchor text and URL, returning the formatted string.
///
/// # Returns
/// A `Cow<str>` with anchor tags replaced by the formatter output.
pub fn convert_html_links<F>(text: &str, formatter: F) -> Cow<'_, str>
where
    F: Fn(&str, &str) -> String,
{
    use regex::Regex;
    use std::sync::LazyLock;
    static HTML_LINK_REGEX: LazyLock<Regex> = LazyLock::new(|| {
        // Match any anchor tag with a valid URL.
        // E.g. <a target="_blank" href="https://example.com">Example</a>
        Regex::new(r#"<a\s+[^>]*href="([^"]+)"[^>]*>([^<]+)</a>"#).expect("Invalid regex")
    });

    HTML_LINK_REGEX.replace_all(text, |caps: &regex::Captures| {
        let url = &caps[1];
        let anchor_text = &caps[2];
        formatter(anchor_text, url)
    })
}

pub fn word_wrap(text: &str, width: usize) -> Cow<'_, str> {
    if text.len() <= width {
        return Cow::Borrowed(text);
    }

    let mut wrapped = String::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            // First word, just add it to the current line
            current_line.push_str(word);
        } else if current_line.len() + 1 + word.len() <= width {
            // Word fits on the current line, just add a space and the word
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            // Word doesn't fit on the current line, so wrap it and add it to the output
            if !wrapped.is_empty() {
                wrapped.push('\n');
            }
            wrapped.push_str(&current_line);
            String::clear(&mut current_line);
            current_line.push_str(word);
        }
    }

    if !current_line.is_empty() {
        if !wrapped.is_empty() {
            wrapped.push('\n');
        }
        wrapped.push_str(&current_line);
    }

    Cow::Owned(wrapped)
}

pub fn word_wrap_to_terminal_width(text: &str) -> Cow<'_, str> {
    let width = crossterm::terminal::size().map_or(80, |(w, _)| w as usize);
    word_wrap(text, width)
}
