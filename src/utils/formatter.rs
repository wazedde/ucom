use std::fmt::{Display, Formatter, Result};
use yansi::Paint;

/// A trait that provides a method to conditionally format content based on a boolean condition.
pub trait FormatWhen: Display {
    fn when(&self, condition: bool) -> ConditionalFormat<'_>
    where
        Self: Sized,
    {
        // Default formatter is to just display the content.
        ConditionalFormat {
            content: self,
            condition,
            formatter: |content, f| write!(f, "{content}"),
        }
    }
}

/// Implement the `FormatWhen` trait for all types that implement `Display`.
impl<T: Display + ?Sized> FormatWhen for T {}

/// A struct that conditionally formats content based on a boolean condition.
pub struct ConditionalFormat<'a> {
    /// The content to be formatted.
    content: &'a dyn Display,
    /// The condition to check for formatting.
    condition: bool,
    /// The formatter function to apply if the condition is true.
    formatter: fn(&dyn Display, &mut Formatter) -> Result,
}

impl Display for ConditionalFormat<'_> {
    fn fmt(&self, f: &mut Formatter) -> Result {
        if self.condition {
            (self.formatter)(self.content, f)
        } else {
            write!(f, "{}", self.content)
        }
    }
}

#[allow(dead_code)]
impl ConditionalFormat<'_> {
    /// Wraps the content with `*` for Markdown italic formatting.
    pub const fn md_italic(self) -> Self {
        Self {
            formatter: |content, f| write!(f, "*{content}*"),
            ..self
        }
    }

    /// Wraps the content with `**` for Markdown bold formatting.
    pub const fn md_bold(self) -> Self {
        Self {
            formatter: |content, f| write!(f, "**{content}**"),
            ..self
        }
    }

    /// Wraps the content with backticks for Markdown code formatting.
    pub const fn md_code(self) -> Self {
        Self {
            formatter: |content, f| write!(f, "`{content}`"),
            ..self
        }
    }

    /// Wraps the content with triple backticks for Markdown code block formatting.
    pub const fn md_code_block(self) -> Self {
        Self {
            formatter: |content, f| write!(f, "```\n{content}\n```"),
            ..self
        }
    }

    /// Paints the content in bold.
    pub const fn bold(self) -> Self {
        Self {
            formatter: |content, f| write!(f, "{}", Paint::new(content.to_string()).bold()),
            ..self
        }
    }

    /// Hides the content by writing an empty string.
    pub const fn hide(self) -> Self {
        Self {
            formatter: |_, f| write!(f, ""),
            ..self
        }
    }
}
