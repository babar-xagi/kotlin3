use k3_syntax::Span;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticCode {
    UnresolvedReference(String),
    TypeMismatch { expected: String, actual: String },
    ValReassignment(String),
    ConditionMustBeBoolean(String),
    WrongNumberOfArguments { expected: usize, actual: usize },
    DuplicateDeclaration(String),
    CannotInferType(String),
    SuspendCallOutsideCoroutine(String),
    AbstractMemberNotImplemented { class_name: String, member: String },
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticCode::UnresolvedReference(name) => {
                write!(f, "Unresolved reference: '{}'", name)
            }
            DiagnosticCode::TypeMismatch { expected, actual } => {
                write!(f, "Type mismatch: inferred type is '{}' but '{}' was expected", actual, expected)
            }
            DiagnosticCode::ValReassignment(name) => {
                write!(f, "Val cannot be reassigned: '{}'", name)
            }
            DiagnosticCode::ConditionMustBeBoolean(actual) => {
                write!(f, "Condition must be of type 'Boolean', found '{}'", actual)
            }
            DiagnosticCode::WrongNumberOfArguments { expected, actual } => {
                write!(f, "Wrong number of arguments: expected {}, found {}", expected, actual)
            }
            DiagnosticCode::DuplicateDeclaration(name) => {
                write!(f, "Conflicting declaration: '{}'", name)
            }
            DiagnosticCode::CannotInferType(name) => {
                write!(f, "Cannot infer type for '{}', please specify type explicitly", name)
            }
            DiagnosticCode::SuspendCallOutsideCoroutine(name) => {
                write!(f, "Suspend function '{}' should be called only from a coroutine or another suspend function", name)
            }
            DiagnosticCode::AbstractMemberNotImplemented { class_name, member } => {
                write!(f, "Class '{}' is not abstract and does not implement abstract member '{}'", class_name, member)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub code: DiagnosticCode,
    pub span: Span,
}

impl Diagnostic {
    pub fn error(code: DiagnosticCode, span: Span) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            code,
            span,
        }
    }

    pub fn warning(code: DiagnosticCode, span: Span) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            code,
            span,
        }
    }

    pub fn format_with_source(&self, source: &str, file_name: &str) -> String {
        let (line, col) = compute_line_col(source, self.span.start as usize);
        let sev_str = match self.severity {
            DiagnosticSeverity::Error => "error",
            DiagnosticSeverity::Warning => "warning",
            DiagnosticSeverity::Info => "info",
        };

        let snippet = extract_line_snippet(source, line);
        let caret = " ".repeat(col.saturating_sub(1)) + "^";

        format!(
            "{}:{}:{}: {}: {}\n{}\n{}",
            file_name, line, col, sev_str, self.code, snippet, caret
        )
    }
}

fn compute_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in source.chars().enumerate() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn extract_line_snippet(source: &str, target_line: usize) -> String {
    let mut current_line = 1;
    for line_str in source.lines() {
        if current_line == target_line {
            return line_str.to_string();
        }
        current_line += 1;
    }
    String::new()
}
