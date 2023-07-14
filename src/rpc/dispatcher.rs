use std::future::Future;

use super::{api::RequestEnum, incoming::Incoming, outgoing::Outgoing};

pub trait Dispatcher<E>
where
    E: RequestEnum + Send + 'static,
    Self: Send + 'static,
{
    type Check<'t>: Future<Output = ()> + 't;

    fn dispatch_connection<'s>(
        &'s mut self,
        outgoing: Outgoing,
        incoming: Incoming<E>,
    ) -> Self::Check<'s>;
}
