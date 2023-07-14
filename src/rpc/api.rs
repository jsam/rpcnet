use core::fmt;
use std::{
    fmt::Formatter,
    io::{self, ErrorKind},
};

use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::oneshot;

use crate::transport::message::MessageBuf;

pub type Pending = oneshot::Sender<MessageBuf>;

pub trait RequestEnum: Send + Sized + Unpin + 'static {
    fn to_byte(&self) -> u8;
    fn from_byte(byte: &u8) -> Option<Self>;
}

pub trait Request<N: RequestEnum>:
    Serialize + DeserializeOwned + Unpin
{
    /// The type of a successful response.
    type Success: Serialize + DeserializeOwned;

    /// The type of a failed response.
    type Error: Serialize + DeserializeOwned;

    /// A unique value identifying this type of request.
    const NAME: N;
}

pub type RequestId = u32;

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum Type {
    Request = 0,
    Response = 1,
}

impl fmt::Debug for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Type::Request => write!(f, "Request"),
            Type::Response => write!(f, "Response"),
        }
    }
}

impl Type {
    pub fn from_u8(num: u8) -> io::Result<Self> {
        match num {
            0 => Ok(Type::Request),
            1 => Ok(Type::Response),
            _ => Err(io::Error::new(
                ErrorKind::InvalidData,
                "invalid req/resp type",
            )),
        }
    }
}
