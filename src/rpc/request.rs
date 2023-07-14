use std::{
    io::{self, ErrorKind},
    marker::PhantomData,
};

use crate::{
    queues::sender::Sender,
    transport::message::{FramedMessage, MessageBuf},
};

use super::{
    api::{Request, RequestEnum, RequestId, Type},
    response::Responder,
};

pub struct RequestBuf<N: RequestEnum> {
    pub id: RequestId,
    pub name: N,
    pub origin: Sender,
    pub msg: MessageBuf,
    pub buf_type: Type,

    _n: PhantomData<N>,
}

impl<N: RequestEnum> FramedMessage<RequestBuf<N>> for RequestBuf<N> {
    fn from_message(
        sender: Sender,
        mut msg: MessageBuf,
    ) -> io::Result<RequestBuf<N>> {
        let ty = msg.pop().and_then(Type::from_u8)?;
        let id = msg.pop::<RequestId>()?;
        let name = msg.pop::<u8>()?;

        let name = N::from_byte(&name)
            .and_then(|n| Some(Ok(n)))
            .unwrap_or_else(|| {
                Err(io::Error::new(
                    ErrorKind::Other,
                    "decoding identifier failed",
                ))
            })?;

        let buf = RequestBuf::new(id, name, sender, msg, ty);
        Ok(buf)
    }
}

impl<N: RequestEnum> RequestBuf<N> {
    pub fn new(
        id: RequestId,
        name: N,
        origin: Sender,
        msg: MessageBuf,
        buf_type: Type,
    ) -> Self {
        Self {
            id,
            name,
            origin,
            msg,
            buf_type: buf_type,
            _n: PhantomData,
        }
    }

    pub fn name(&self) -> &N {
        &self.name
    }

    pub fn decode<R: Request<N>>(mut self) -> io::Result<(R, Responder<N, R>)> {
        let payload = self.msg.pop::<R>()?;
        let responder = Responder::new(self.id, self.origin);
        Ok((payload, responder))
    }
}
