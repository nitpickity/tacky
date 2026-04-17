use std::io;

/// An error enum
#[derive(Debug)]
pub enum Error {
    /// IO error
    Io(io::Error),
    /// Nom Error
    Nom(nom::Err<nom::error::Error<String>>),
    /// Nom's other failure case; giving up in the middle of a file
    TrailingGarbage(String),
    /// No .proto file provided
    NoProto,
    /// Cannot read input file
    InputFile(String),
    /// Invalid message
    InvalidMessage(String),
    /// Import file not found
    InvalidImport(String),
    /// Empty read
    EmptyRead,
    /// Enum or message not found
    MessageOrEnumNotFound(String),
    /// Invalid default enum
    InvalidDefaultEnum(String),
}

/// A wrapper for `Result<T, Error>`
pub type Result<T> = ::std::result::Result<T, Error>;

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::Io(e)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Nom(e) => Some(e),
            _ => None,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        match self {
            Error::Io(e) => write!(f, "{}", e),
            Error::Nom(e) => write!(f, "{}", e),
            Error::TrailingGarbage(s) => write!(f, "parsing abandoned near: {:?}", s),
            Error::NoProto => write!(f, "No .proto file provided"),
            Error::InputFile(file) => write!(f, "Cannot read input file '{}'", file),
            Error::InvalidMessage(msg) => write!(
                f,
                "Message checks errored: {}\r\n\
                Proto definition might be invalid or something got wrong in the parsing",
                msg
            ),
            Error::InvalidImport(imp) => write!(
                f,
                "Import not found: {}\r\n\
                Import definition might be invalid, some characters may not be supported",
                imp
            ),
            Error::EmptyRead => write!(
                f,
                "No message or enum were read; \
                either definition might be invalid or there were only unsupported structures"
            ),
            Error::MessageOrEnumNotFound(me) => write!(f, "Could not find message or enum {}", me),
            Error::InvalidDefaultEnum(en) => write!(
                f,
                "Enum field cannot be set to '{}', this variant does not exist",
                en
            ),
        }
    }
}
