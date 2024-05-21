use std::future::Future;
use tokio::task::JoinHandle;

use super::{api::RequestEnum, incoming::Incoming, outgoing::Outgoing};

pub trait Dispatcher<E>
where
    E: RequestEnum + Send + 'static,
    Self: Send + 'static,
{
    fn dispatch_connection(
        &mut self,
        outgoing: Outgoing,
        incoming: Incoming<E>,
    ) -> JoinHandle<()>;
}
