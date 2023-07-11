use std::{
    borrow::BorrowMut,
    io::{self, ErrorKind},
    marker::PhantomData,
};

use crate::{
    queues::sender::Sender,
    transport::message::{FramedMessage, MessageBuf},
};

use super::api::{Name, Request, RequestId, Type};

pub struct RequestBuf<N: Name> {
    pub id: RequestId,
    pub name: N,
    //origin: Sender,
    pub msg: MessageBuf,
    pub buf_type: Type,
    _n: PhantomData<N>,
}

impl<N: Name> FramedMessage<RequestBuf<N>> for RequestBuf<N> {
    fn from_message(mut msg: MessageBuf) -> io::Result<RequestBuf<N>> {
        let ty = msg.pop().and_then(Type::from_u8)?;
        let id = msg.pop::<RequestId>()?;
        let name = msg.pop::<N::Discriminant>()?;

        let name = N::from_discriminant(&name)
            .and_then(|n| Some(Ok(n)))
            .unwrap_or_else(|| {
                Err(io::Error::new(
                    ErrorKind::Other,
                    "decoding discriminant failed",
                ))
            })?;

        let buf = RequestBuf::new(id, name, msg, ty);
        Ok(buf)
    }
}

impl<N: Name> RequestBuf<N> {
    pub fn new(id: RequestId, name: N, msg: MessageBuf, buf_type: Type) -> Self {
        Self {
            id,
            name,
            //origin,
            msg,
            buf_type: buf_type,
            _n: PhantomData,
        }
    }

    /// Extracting the `Request::NAME` to identfy the request type.
    pub fn name(&self) -> &N {
        &self.name
    }

    // / Decodes the request into the request data and a responder handle.
    // /
    // / The returned [`Responder`](struct.Responder.html) handle is to be used
    // / to serve the decoded request `R`.
    // pub fn decode<R: Request<N>>(mut self) -> io::Result<(R, Responder<N, R>)> {
    //     let payload = self.msg.pop::<R>()?;
    //     let responder = Responder::new(self.id, self.origin);

    //     Ok((payload, responder))
    // }
}
