use std::fmt;

/// Linework action encoded in a field code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeAction {
    /// Start a new figure
    Begin,
    /// Continue an existing figure
    Continue,
    /// End the current figure
    End,
    /// No linework information
    None,
}

/// Parsed field code with optional linework action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldCode {
    pub action: CodeAction,
    pub code: String,
}

impl FieldCode {
    /// Parses a raw field code string using simple begin/continue/end prefixes.
    pub fn parse(raw: &str) -> Self {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Self {
                action: CodeAction::None,
                code: String::new(),
            };
        }
        let mut chars = trimmed.chars();
        let first = chars.next().unwrap();
        let rest: String = chars.collect();
        let rest = rest
            .trim_start_matches(|c: char| c == '-' || c.is_whitespace())
            .to_string();
        match first.to_ascii_uppercase() {
            'B' if !rest.is_empty() => Self {
                action: CodeAction::Begin,
                code: rest,
            },
            'C' if !rest.is_empty() => Self {
                action: CodeAction::Continue,
                code: rest,
            },
            'E' if !rest.is_empty() => Self {
                action: CodeAction::End,
                code: rest,
            },
            _ => Self {
                action: CodeAction::None,
                code: trimmed.to_string(),
            },
        }
    }
}

impl fmt::Display for FieldCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.action {
            CodeAction::Begin => write!(f, "B{}", self.code),
            CodeAction::Continue => write!(f, "C{}", self.code),
            CodeAction::End => write!(f, "E{}", self.code),
            CodeAction::None => write!(f, "{}", self.code),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_codes() {
        let b = FieldCode::parse("BCURB");
        assert_eq!(b.action, CodeAction::Begin);
        assert_eq!(b.code, "CURB");
        let c = FieldCode::parse("CCURB");
        assert_eq!(c.action, CodeAction::Continue);
        assert_eq!(c.code, "CURB");
        let e = FieldCode::parse("ECURB");
        assert_eq!(e.action, CodeAction::End);
        assert_eq!(e.code, "CURB");
        let n = FieldCode::parse("TREE");
        assert_eq!(n.action, CodeAction::None);
        assert_eq!(n.code, "TREE");
    }
}
