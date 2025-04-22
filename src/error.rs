use derive_more::{Display, From};

use crate::Rule; // auto-generated pest Rule type

pub type Result<T> = core::result::Result<T, Error>;

// Prod
#[non_exhaustive]
#[derive(Debug, From, Display)]
pub enum Error {
    // -- lib
    #[display("Error: invalid range \"[{start}-{end}]\": start greater than end")]
    InvalidRangeReversed { start: u32, end: u32 },

    #[display("Error: integer value {_0} exceeds limits")]
    TooLarge(u32),

    #[display("Error: hostlist is too large")]
    HostlistTooLarge,

    #[display("Error: unexpected parser state while processing rule:\n{_0:?}")]
    UnexpectedParserState(Rule),

    #[display("Error: invalid hostname: \"{_0}\"")]
    InvalidHostname(String),

    #[display("Error: internal error: \"{_0}\"")]
    Internal(String),

    // -- Externals
    #[display("Error: parse error:\n{_0}")]
    ParseError(Box<pest::error::Error<Rule>>),

    #[from]
    #[display("Error: integer parse error: {_0}")]
    ParseIntError(std::num::ParseIntError),
}

impl std::error::Error for Error {}

// The pest error type is quite large, so to reduce Result size (and fix clippy warnings) we Box
// it. But this means we have to write our own From implementation.
impl From<pest::error::Error<Rule>> for Error {
    fn from(err: pest::error::Error<Rule>) -> Self {
        Self::ParseError(Box::new(err))
    }
}
