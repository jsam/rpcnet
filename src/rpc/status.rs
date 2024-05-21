use serde_derive::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use tokio::task::JoinHandle;
use crate::rpc::{api::Request, incoming::Incoming, outgoing::Outgoing};

use super::{api::RequestEnum, dispatcher::Dispatcher};
use tokio_stream::StreamExt;

pub enum PingRPC {
    Ping = 10,
    Pong = 20,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PingRequest;

#[derive(Serialize, Deserialize, Debug)]
pub struct PongResponse;

impl Request<StatusRPC> for PingRequest {
    const NAME: StatusRPC = StatusRPC::Ping(PingRPC::Ping);

    type Success = PongResponse;

    type Error = ();
}

impl RequestEnum for PingRPC {
    fn to_byte(&self) -> u8 {
        match self {
            PingRPC::Ping => 10,
            PingRPC::Pong => 20,
        }
    }

    fn from_byte(byte: &u8) -> Option<Self> {
        match byte {
            10 => Some(PingRPC::Ping),
            20 => Some(PingRPC::Pong),
            _ => None,
        }
    }
}

pub enum EchoRPC {
    Echo = 30,
    EchoResponse = 40,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EchoRequest(usize);

#[derive(Serialize, Deserialize, Debug)]
pub struct EchoResponse(usize);

impl Request<StatusRPC> for EchoRequest {
    const NAME: StatusRPC = StatusRPC::Echo(EchoRPC::Echo);

    type Success = EchoResponse;

    type Error = ();
}

impl RequestEnum for EchoRPC {
    fn to_byte(&self) -> u8 {
        match self {
            EchoRPC::Echo => 30,
            EchoRPC::EchoResponse => 40,
        }
    }

    fn from_byte(byte: &u8) -> Option<Self> {
        match byte {
            30 => Some(EchoRPC::Echo),
            40 => Some(EchoRPC::EchoResponse),
            _ => None,
        }
    }
}

pub enum StatusRPC {
    Ping(PingRPC),
    Echo(EchoRPC),
}

impl RequestEnum for StatusRPC {
    fn to_byte(&self) -> u8 {
        match self {
            StatusRPC::Ping(rpc) => rpc.to_byte(),
            StatusRPC::Echo(rpc) => rpc.to_byte(),
        }
    }

    fn from_byte(byte: &u8) -> Option<Self> {
        match byte {
            10 => Some(StatusRPC::Ping(PingRPC::Ping)),
            20 => Some(StatusRPC::Ping(PingRPC::Pong)),
            30 => Some(StatusRPC::Echo(EchoRPC::Echo)),
            40 => Some(StatusRPC::Echo(EchoRPC::EchoResponse)),
            _ => None,
        }
    }
}

#[derive(Copy, Clone)]
pub struct StatusDispatcher
where
    Self: Send;

impl Dispatcher<StatusRPC> for StatusDispatcher {

    fn dispatch_connection(
        &mut self,
        outgoing: Outgoing,
        mut incoming: Incoming<StatusRPC>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(request) = incoming.next().await {
                let _ = match request {
                    Ok(request) => match request.name() {
                        StatusRPC::Ping(PingRPC::Ping) => {
                            let (req, res) =
                                request.decode::<PingRequest>().unwrap();
                            tokio::spawn(async move {
                                res.respond(Ok(PongResponse {}))
                            });
                        }
                        StatusRPC::Echo(EchoRPC::Echo) => {
                            let (req, res) =
                                request.decode::<EchoRequest>().unwrap();
                            tokio::spawn(async move {
                                let value = req.0 + 1;
                                res.respond(Ok(EchoResponse(value)))
                            });
                        }
                        _ => {}
                    },
                    Err(err) => {
                        panic!("Error: {}", err);
                    }
                };
            }
        })
    }
}
