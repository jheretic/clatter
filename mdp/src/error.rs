//! The error types for MDP.
use std::{error, fmt, io, result};
use cookie_factory::GenError;
use nom;

use addr::SocketAddr;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    ParseError(nom::ErrorKind),
    ParseIncomplete(nom::Needed),
    EncodeError(GenError),
    MessageNeedsPlain,
    MessageNeedsEncrypted,
    MessageNeedsSigned,
    FrameFull,
    FrameTooLarge,
    MessageBadTtl(u8),
    MessageBadLen(usize),
    PayloadDecodeEncrypted,
    Encrypt,
    Decrypt,
    Verify,
    ConvertPublicKey,
    ConvertPrivateKey,
    OverlayInvalidIP,
    InvalidInterface,
    InvalidSocket,
    RoutingTableInvalid,
    QueueCongestion,
    AddrAlreadyInUse(SocketAddr),
    MalformedMessage,
    UdpIncompleteSend(usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Io(ref err) => fmt::Display::fmt(err, f),
            Error::ParseError(ref err) => fmt::Debug::fmt(err, f),
            Error::EncodeError(ref err) => fmt::Debug::fmt(err, f),
            Error::ParseIncomplete(i) => match i {
                nom::Needed::Unknown => {
                    write!(f, "Missing unknown amount of bytes while deserializing.")
                }
                nom::Needed::Size(s) => {
                    write!(f, "Missing {:?} bytes of data while deserializing.", s)
                }
            },
            Error::MessageNeedsPlain => write!(
                f,
                "Function needs a plaintext frame but was passed some other type."
            ),
            Error::MessageNeedsEncrypted => write!(
                f,
                "Function needs an encrypted frame but was passed some other type."
            ),
            Error::MessageNeedsSigned => write!(
                f,
                "Function needs a signed frame but was passed some other type."
            ),
            Error::FrameFull => write!(
                f,
                "The current frame cannot take the specified message(s) and remain under the MTU."
            ),
            Error::FrameTooLarge => write!(f, "The current frame is too large for the MTU."),
            Error::MessageBadTtl(n) => write!(
                f,
                "The message's TTL of {:?} is not within the valid range.",
                n
            ),
            Error::MessageBadLen(n) => write!(
                f,
                "The message's data length of {:?} is not within the valid range.",
                n
            ),
            Error::PayloadDecodeEncrypted => write!(f, "Error deserializing a decrypted payload."),
            Error::Encrypt => write!(f, "Error encrypting payload."),
            Error::Decrypt => write!(f, "Error decrypting payload."),
            Error::Verify => write!(f, "Error verifying signed payload."),
            Error::ConvertPublicKey => {
                write!(f, "Error transforming public signing key to crypto key.")
            }
            Error::ConvertPrivateKey => {
                write!(f, "Error transforming private signing key to crypto key.")
            }
            Error::OverlayInvalidIP => write!(f, "Invalid Overlay IP address."),
            Error::InvalidInterface => write!(f, "Invalid interface ID."),
            Error::InvalidSocket => write!(f, "Invalid socket ID."),
            Error::RoutingTableInvalid => write!(f, "Invalid or corrupted routing table."),
            Error::QueueCongestion => write!(f, "Outgoing QoS queue is congested."),
            Error::AddrAlreadyInUse(s) => write!(f, "Socket address {:?} already in use.", s),
            Error::MalformedMessage => write!(f, "Malformed message detected."),
            Error::UdpIncompleteSend(n) => {
                write!(f, "Was only able to send {:?} bytes of UDP message.", n)
            }
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Io(ref err) => err.description(),
            Error::ParseError(ref err) => err.description(),
            Error::EncodeError(_) => "Error while serializing to buffer.",
            Error::ParseIncomplete(_) => "Missing bytes while deserializing.",
            Error::MessageNeedsPlain => {
                "Function needs a plaintext frame but was passed some other type."
            }
            Error::MessageNeedsEncrypted => {
                "Function needs an encrypted frame but was passed some other type."
            }
            Error::MessageNeedsSigned => {
                "Function needs a signed frame but was passed some other type."
            }
            Error::FrameFull => {
                "The current frame cannot take the specified message(s) and remain under the MTU."
            }
            Error::FrameTooLarge => "The current frame is too large for the MTU.",
            Error::MessageBadTtl(_) => "The message's TTL is not within the valid range.",
            Error::MessageBadLen(_) => "The message's data length is not within the valid range.",
            Error::PayloadDecodeEncrypted => "Error deserializing a decrypted payload.",
            Error::Encrypt => "Error encrypting payload.",
            Error::Decrypt => "Error decrypting payload.",
            Error::Verify => "Error verifying signed payload.",
            Error::ConvertPublicKey => "Error transforming public signing key to crypto key.",
            Error::ConvertPrivateKey => "Error transforming private signing key to crypto key.",
            Error::OverlayInvalidIP => "Invalid Overlay IP address.",
            Error::InvalidInterface => "Invalid interface ID.",
            Error::InvalidSocket => "Invalid socket ID.",
            Error::RoutingTableInvalid => "Invalid or corrupted routing table.",
            Error::QueueCongestion => "Outgoing QoS queue is congested.",
            Error::AddrAlreadyInUse(_) => "Socket address is already in use.",
            Error::MalformedMessage => "Malformed message detected.",
            Error::UdpIncompleteSend(_) => "Was unable to send complete UDP message.",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            Error::Io(ref err) => Some(err),
            Error::ParseError(_) => None,
            Error::EncodeError(_) => None,
            Error::ParseIncomplete(_) => None,
            Error::MessageNeedsPlain => None,
            Error::MessageNeedsEncrypted => None,
            Error::MessageNeedsSigned => None,
            Error::FrameFull => None,
            Error::FrameTooLarge => None,
            Error::MessageBadTtl(_) => None,
            Error::MessageBadLen(_) => None,
            Error::PayloadDecodeEncrypted => None,
            Error::Encrypt => None,
            Error::Decrypt => None,
            Error::Verify => None,
            Error::ConvertPublicKey => None,
            Error::ConvertPrivateKey => None,
            Error::OverlayInvalidIP => None,
            Error::InvalidInterface => None,
            Error::InvalidSocket => None,
            Error::RoutingTableInvalid => None,
            Error::QueueCongestion => None,
            Error::AddrAlreadyInUse(_) => None,
            Error::MalformedMessage => None,
            Error::UdpIncompleteSend(_) => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

impl<'a> From<nom::Err<&'a [u8]>> for Error {
    fn from(err: nom::Err<&'a [u8]>) -> Error {
        match err {
            nom::Err::Incomplete(needed) => Error::ParseIncomplete(needed),
            nom::Err::Error(e) | nom::Err::Failure(e) => Error::ParseError(e.into_error_kind())
        }
    }
}

impl From<GenError> for Error {
    fn from(err: GenError) -> Error {
        Error::EncodeError(err)
    }
}

impl From<Error> for () {
    fn from(_: Error) -> () {}
}

pub type Result<T> = result::Result<T, Error>;

// Alias to provide an analogue to IResult, except for the encoding pipeline.
pub type GResult<T> = result::Result<T, GenError>;
