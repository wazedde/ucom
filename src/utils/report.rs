use std::fmt::Display;
use yansi::Paint;

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
