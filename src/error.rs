use std::fmt;

pub type Result<T> = std::result::Result<T, VcdError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VcdError {
    stage: &'static str,
    message: String,
    hint: Option<String>,
}

impl VcdError {
    pub fn new(stage: &'static str, message: impl Into<String>) -> Self {
        Self {
            stage,
            message: message.into(),
            hint: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

impl fmt::Display for VcdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter, "{}: {}", self.stage, self.message)?;
        if let Some(hint) = &self.hint {
            write!(formatter, "提示: {hint}")?;
        }
        Ok(())
    }
}

impl std::error::Error for VcdError {}
