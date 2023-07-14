#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]
#![feature(impl_trait_in_assoc_type)]

use std::sync::Arc;

use crate::rpc::{
    multiplex,
    server::Server,
    status::{PingRequest, StatusDispatcher, StatusRPC},
};
use tokio::net::TcpStream;

mod file;
mod network;
mod queues;
pub mod rpc;
mod rpcnet;
mod transport;

#[tokio::main]
async fn main() {
    let server = Server::start::<StatusRPC, StatusDispatcher>(
        Arc::new("localhost".to_string()),
        6789,
        StatusDispatcher {},
    )
    .await
    .unwrap();
    let socket = TcpStream::connect(server.addr()).await.unwrap();
    let (outgoing, _) =
        multiplex::make_rpc_connection::<StatusRPC>(socket).unwrap();

    let response = outgoing.request(&PingRequest {});
    assert!(response.id == 0);

    let response = outgoing.request(&PingRequest {});
    assert!(response.id == 1);

    let response = outgoing.request(&PingRequest {});
    assert!(response.id == 2);

    let response = response.await.unwrap();
    assert!(response.is_ok());
}
