use crate::{Command, Hello, VarValue, PROTOCOL_VERSION};

/// The other side speaks another protocol version.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("protocol version {theirs} does not match {ours}")]
pub struct HelloError {
    pub theirs: u32,
    pub ours: u32,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CommandError {
    #[error("name is empty or longer than {0} characters")]
    BadName(usize),
    #[error("name contains a path separator or control character")]
    PathLike,
    #[error("text value contains a control character")]
    ControlInText,
    #[error("text value is longer than {0} characters")]
    TextTooLong(usize),
    #[error("more than {0} names in one subscription")]
    TooManyNames(usize),
    #[error("value is not a finite number")]
    NotFinite,
    #[error("file index {0} is not in the vehicle's list")]
    UnknownFile(u32),
}

const MAX_NAME: usize = 128;
/// Longest text a tool may write into a script variable.
pub const MAX_TEXT: usize = 1024;
/// Most variables one subscription may name.
pub const MAX_SUBSCRIPTION: usize = 512;

/// Handshake rule: both sides must speak the same version.
pub fn check_hello(theirs: &Hello) -> Result<(), HelloError> {
    if theirs.protocol == PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(HelloError {
            theirs: theirs.protocol,
            ours: PROTOCOL_VERSION,
        })
    }
}

fn check_name(name: &str) -> Result<(), CommandError> {
    if name.is_empty() || name.chars().count() > MAX_NAME {
        return Err(CommandError::BadName(MAX_NAME));
    }
    if name
        .chars()
        .any(|c| c == '/' || c == '\\' || c.is_control())
    {
        return Err(CommandError::PathLike);
    }
    Ok(())
}

/// Shape check the game runs before it looks a command up. Whether a variable name exists
/// and whether a value is in range is decided by the game, which knows the vehicle;
/// `reload_files` is the number of files in its reload list.
pub fn validate_command(cmd: &Command, reload_files: u32) -> Result<(), CommandError> {
    match cmd {
        Command::Subscribe { names } if names.len() > MAX_SUBSCRIPTION => {
            Err(CommandError::TooManyNames(MAX_SUBSCRIPTION))
        }
        Command::Subscribe { names } => names.iter().try_for_each(|n| check_name(n)),
        Command::SetVariable { name, value } => {
            check_name(name)?;
            match value {
                VarValue::Float(v) if !v.is_finite() => Err(CommandError::NotFinite),
                VarValue::Text(t) if t.chars().count() > MAX_TEXT => {
                    Err(CommandError::TextTooLong(MAX_TEXT))
                }
                VarValue::Text(t) if t.chars().any(|c| c.is_control()) => {
                    Err(CommandError::ControlInText)
                }
                _ => Ok(()),
            }
        }
        Command::Reload { file_index } if *file_index >= reload_files => {
            Err(CommandError::UnknownFile(*file_index))
        }
        Command::Reload { .. } => Ok(()),
    }
}
