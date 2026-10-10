// Dart source: pkg/analysis_server/lib/src/services/correction/status.dart

//! Dart `RefactoringStatus`: the problems found by the checks of a
//! refactoring.

/// Dart `RefactoringProblemSeverity`, in increasing order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
    Fatal,
}

impl Severity {
    /// Dart `RefactoringProblemSeverity.name`.
    pub fn name(self) -> &'static str {
        match self {
            Severity::Info => "INFO",
            Severity::Warning => "WARNING",
            Severity::Error => "ERROR",
            Severity::Fatal => "FATAL",
        }
    }
}

/// Dart `Location` of a problem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub file: String,
    pub offset: u32,
    pub length: u32,
}

/// Dart `RefactoringProblem`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    pub severity: Severity,
    pub message: String,
    pub location: Option<Location>,
}

/// Dart `RefactoringStatus`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RefactoringStatus {
    severity: Option<Severity>,
    pub problems: Vec<Problem>,
}

impl RefactoringStatus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn error(message: impl Into<String>) -> Self {
        let mut s = Self::new();
        s.add_error(message, None);
        s
    }

    pub fn fatal(message: impl Into<String>) -> Self {
        let mut s = Self::new();
        s.add_fatal_error(message, None);
        s
    }

    pub fn warning(message: impl Into<String>) -> Self {
        let mut s = Self::new();
        s.add_warning(message, None);
        s
    }

    pub fn has_error(&self) -> bool {
        matches!(self.severity, Some(Severity::Fatal | Severity::Error))
    }

    pub fn has_fatal_error(&self) -> bool {
        self.severity == Some(Severity::Fatal)
    }

    pub fn has_warning(&self) -> bool {
        self.severity == Some(Severity::Warning)
    }

    pub fn is_ok(&self) -> bool {
        self.severity.is_none()
    }

    pub fn severity(&self) -> Option<Severity> {
        self.severity
    }

    /// Dart `problem`: the first problem with the severity of the status.
    pub fn problem(&self) -> Option<&Problem> {
        self.problems
            .iter()
            .find(|p| Some(p.severity) == self.severity)
    }

    /// Dart `message`.
    pub fn message(&self) -> Option<&str> {
        self.problem().map(|p| p.message.as_str())
    }

    pub fn add_error(&mut self, message: impl Into<String>, location: Option<Location>) {
        self.add(Severity::Error, message.into(), location);
    }

    pub fn add_fatal_error(&mut self, message: impl Into<String>, location: Option<Location>) {
        self.add(Severity::Fatal, message.into(), location);
    }

    pub fn add_warning(&mut self, message: impl Into<String>, location: Option<Location>) {
        self.add(Severity::Warning, message.into(), location);
    }

    /// Dart `addStatus`.
    pub fn add_status(&mut self, other: RefactoringStatus) {
        self.problems.extend(other.problems);
        self.severity = self.severity.max(other.severity);
    }

    fn add(&mut self, severity: Severity, message: String, location: Option<Location>) {
        self.problems.push(Problem {
            severity,
            message,
            location,
        });
        self.severity = self.severity.max(Some(severity));
    }
}
