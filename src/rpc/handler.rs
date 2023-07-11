use tokio::io;

use super::{api::Name, incoming::Incoming, outgoing::Outgoing};

pub trait ConnectionHandler<N: Name> {
    fn handle_connection(&self, connection: io::Result<(Outgoing, Incoming<N>)>);
}
