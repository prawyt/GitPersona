use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Check,
    Usage,
    Dependency,
}

#[derive(Debug)]
pub struct GitPersonaError {
    kind: ErrorKind,
    message: String,
}

pub trait AppError: Error + Send + Sync {
    fn exit_code(&self) -> u8;
}

impl GitPersonaError {
    pub fn check(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Check,
            message: message.into(),
        }
    }

    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Usage,
            message: message.into(),
        }
    }

    pub fn dependency(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Dependency,
            message: message.into(),
        }
    }

    pub fn exit_code(&self) -> u8 {
        match self.kind {
            ErrorKind::Check => 1,
            ErrorKind::Usage => 2,
            ErrorKind::Dependency => 3,
        }
    }
}

impl fmt::Display for GitPersonaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for GitPersonaError {}

impl AppError for GitPersonaError {
    fn exit_code(&self) -> u8 {
        self.exit_code()
    }
}
