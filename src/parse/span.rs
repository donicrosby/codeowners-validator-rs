//! Span tracking for source location information.
//!
//! Provides a custom `Span` struct that tracks byte offset, line number, and column
//! for precise error reporting in CODEOWNERS file parsing.

/// Represents a location span in the source file.
///
/// All positions are 1-based for human-readable error messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Byte offset from the start of the input (0-based).
    pub offset: usize,
    /// Line number (1-based).
    pub line: usize,
    /// Column number (1-based).
    pub column: usize,
    /// Length of the span in bytes.
    pub length: usize,
}

impl Span {
    /// Creates a new span with the given position and length.
    pub fn new(offset: usize, line: usize, column: usize, length: usize) -> Self {
        Self {
            offset,
            line,
            column,
            length,
        }
    }

    /// Creates a zero-length span at the given position.
    pub fn point(offset: usize, line: usize, column: usize) -> Self {
        Self::new(offset, line, column, 0)
    }

    /// Returns the end offset of this span.
    pub fn end_offset(&self) -> usize {
        self.offset + self.length
    }

    /// Extends this span to include another span.
    pub fn extend(&self, other: &Span) -> Span {
        let end = other.offset + other.length;
        Span {
            offset: self.offset,
            line: self.line,
            column: self.column,
            length: end.saturating_sub(self.offset),
        }
    }
}

impl Default for Span {
    fn default() -> Self {
        Self::new(0, 1, 1, 0)
    }
}

/// Tracks position while iterating through input.
///
/// This struct wraps a string slice and maintains current position information
/// for use with nom parsers.
#[derive(Debug, Clone, Copy)]
pub struct SpanTracker<'a> {
    /// The remaining input to parse.
    input: &'a str,
    /// Current byte offset from the original input start.
    offset: usize,
    /// Current line number (1-based).
    line: usize,
    /// Current column number (1-based).
    column: usize,
}

impl<'a> SpanTracker<'a> {
    /// Creates a new span tracker for the given input.
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            offset: 0,
            line: 1,
            column: 1,
        }
    }

    /// Returns the remaining input.
    pub fn as_str(&self) -> &'a str {
        self.input
    }

    /// Returns true if there's no more input.
    pub fn is_empty(&self) -> bool {
        self.input.is_empty()
    }

    /// Returns the current position as a span with zero length.
    pub fn current_span(&self) -> Span {
        Span::point(self.offset, self.line, self.column)
    }

    /// Creates a span from the current position with the given length.
    pub fn span_of(&self, length: usize) -> Span {
        Span::new(self.offset, self.line, self.column, length)
    }

    /// Advances the tracker by the given number of bytes.
    ///
    /// Updates line and column based on the content being skipped.
    pub fn advance(&mut self, bytes: usize) -> &'a str {
        let consumed = &self.input[..bytes];
        
        // Update line and column based on consumed content
        for ch in consumed.chars() {
            if ch == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }
        
        self.offset += bytes;
        self.input = &self.input[bytes..];
        consumed
    }

    /// Advances past a specific string slice, returning its span.
    ///
    /// The slice must be at the start of the current input.
    pub fn consume(&mut self, s: &str) -> Span {
        debug_assert!(self.input.starts_with(s));
        let span = self.span_of(s.len());
        self.advance(s.len());
        span
    }

    /// Returns the current line number (1-based).
    pub fn line(&self) -> usize {
        self.line
    }

    /// Returns the current column number (1-based).
    pub fn column(&self) -> usize {
        self.column
    }

    /// Returns the current byte offset.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Peeks at the next character without consuming it.
    pub fn peek_char(&self) -> Option<char> {
        self.input.chars().next()
    }

    /// Returns the number of remaining bytes.
    pub fn remaining(&self) -> usize {
        self.input.len()
    }
}

impl<'a> AsRef<str> for SpanTracker<'a> {
    fn as_ref(&self) -> &str {
        self.input
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_new_and_accessors() {
        let span = Span::new(10, 2, 5, 15);
        assert_eq!(span.offset, 10);
        assert_eq!(span.line, 2);
        assert_eq!(span.column, 5);
        assert_eq!(span.length, 15);
        assert_eq!(span.end_offset(), 25);
    }

    #[test]
    fn span_point_has_zero_length() {
        let span = Span::point(5, 1, 6);
        assert_eq!(span.length, 0);
        assert_eq!(span.end_offset(), 5);
    }

    #[test]
    fn span_extend_combines_spans() {
        let span1 = Span::new(0, 1, 1, 5);
        let span2 = Span::new(10, 1, 11, 3);
        let extended = span1.extend(&span2);
        
        assert_eq!(extended.offset, 0);
        assert_eq!(extended.line, 1);
        assert_eq!(extended.column, 1);
        assert_eq!(extended.length, 13); // 0 to 13
    }

    #[test]
    fn tracker_initial_position() {
        let tracker = SpanTracker::new("hello\nworld");
        assert_eq!(tracker.line(), 1);
        assert_eq!(tracker.column(), 1);
        assert_eq!(tracker.offset(), 0);
        assert_eq!(tracker.as_str(), "hello\nworld");
    }

    #[test]
    fn tracker_advance_updates_position() {
        let mut tracker = SpanTracker::new("hello\nworld");
        
        // Advance past "hello"
        tracker.advance(5);
        assert_eq!(tracker.line(), 1);
        assert_eq!(tracker.column(), 6);
        assert_eq!(tracker.offset(), 5);
        assert_eq!(tracker.as_str(), "\nworld");
    }

    #[test]
    fn tracker_advance_handles_newlines() {
        let mut tracker = SpanTracker::new("hello\nworld");
        
        // Advance past "hello\n"
        tracker.advance(6);
        assert_eq!(tracker.line(), 2);
        assert_eq!(tracker.column(), 1);
        assert_eq!(tracker.offset(), 6);
        assert_eq!(tracker.as_str(), "world");
    }

    #[test]
    fn tracker_consume_returns_span() {
        let mut tracker = SpanTracker::new("*.rs @owner");
        let span = tracker.consume("*.rs");
        
        assert_eq!(span.offset, 0);
        assert_eq!(span.line, 1);
        assert_eq!(span.column, 1);
        assert_eq!(span.length, 4);
        assert_eq!(tracker.as_str(), " @owner");
    }

    #[test]
    fn tracker_multiple_lines() {
        let mut tracker = SpanTracker::new("line1\nline2\nline3");
        
        tracker.advance(6); // "line1\n"
        assert_eq!(tracker.line(), 2);
        assert_eq!(tracker.column(), 1);
        
        tracker.advance(6); // "line2\n"
        assert_eq!(tracker.line(), 3);
        assert_eq!(tracker.column(), 1);
        
        tracker.advance(5); // "line3"
        assert_eq!(tracker.line(), 3);
        assert_eq!(tracker.column(), 6);
    }

    #[test]
    fn tracker_span_of_creates_correct_span() {
        let mut tracker = SpanTracker::new("hello\nworld");
        tracker.advance(6); // Move to line 2
        
        let span = tracker.span_of(5);
        assert_eq!(span.offset, 6);
        assert_eq!(span.line, 2);
        assert_eq!(span.column, 1);
        assert_eq!(span.length, 5);
    }

    #[test]
    fn tracker_peek_char() {
        let tracker = SpanTracker::new("hello");
        assert_eq!(tracker.peek_char(), Some('h'));
        
        let empty_tracker = SpanTracker::new("");
        assert_eq!(empty_tracker.peek_char(), None);
    }

    #[test]
    fn tracker_is_empty() {
        let mut tracker = SpanTracker::new("hi");
        assert!(!tracker.is_empty());
        
        tracker.advance(2);
        assert!(tracker.is_empty());
    }
}
