use std::io::{self, ErrorKind};

use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::oneshot;

use crate::transport::message::MessageBuf;

pub type Pending = oneshot::Sender<MessageBuf>;

pub trait Name: Send + Sized + Unpin + 'static {
    /// The discriminant type representing `Self`.
    type Discriminant: Serialize + DeserializeOwned + 'static;

    /// Convert `Self` into a discriminant.
    fn discriminant(&self) -> Self::Discriminant;

    /// Restore `Self` from a discriminant. Returns `None` if `Self` cannot be restored.
    fn from_discriminant(_: &Self::Discriminant) -> Option<Self>;
}

pub trait Request<N: Name>: Serialize + DeserializeOwned + Unpin {
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
