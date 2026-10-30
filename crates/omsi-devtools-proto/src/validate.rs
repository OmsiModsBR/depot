use crate::{Command, Hello, VarValue, PROTOCOL_VERSION};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CommandError {
    #[error("protocol version {theirs} does not match {ours}")]
    Version { theirs: u32, ours: u32 },
    #[error("name is empty or longer than {0} characters")]
    BadName(usize),
    #[error("name contains a path separator or control character")]
    PathLike,
    #[error("value is not a finite number")]
    NotFinite,
    #[error("file index {0} is not in the vehicle's list")]
    UnknownFile(u32),
}

const MAX_NAME: usize = 128;

/// Handshake rule: both sides must speak the same version.
pub fn check_hello(theirs: &Hello) -> Result<(), CommandError> {
    if theirs.protocol == PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(CommandError::Version {
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
        Command::Subscribe { names } => names.iter().try_for_each(|n| check_name(n)),
        Command::SetVariable { name, value } => {
            check_name(name)?;
            match value {
                VarValue::Float(v) if !v.is_finite() => Err(CommandError::NotFinite),
                VarValue::Text(t) if t.chars().any(|c| c.is_control()) => {
                    Err(CommandError::PathLike)
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
