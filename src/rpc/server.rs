use std::{marker::PhantomData, sync::Arc};

use tokio::{io, net::TcpStream, task::JoinHandle};

use crate::queues::listener::{ConnectionSetup, Listener};

use super::{
    api::Name, incoming::Incoming, multiplex::split, outgoing::Outgoing, request::RequestBuf,
};
use tokio_stream::StreamExt;

pub struct RpcConnection;

impl<N> ConnectionSetup<RequestBuf<N>> for RpcConnection
where
    N: Name,
{
    type Item = io::Result<(Outgoing, Incoming<N>)>;

    fn setup(stream: TcpStream) -> Self::Item {
        split(stream)
    }
}

pub struct Server<N>
where
    N: Name,
{
    listener: JoinHandle<()>,
    pub external: Arc<String>,
    pub port: u16,

    phantom_data: PhantomData<N>,
}

impl<N> Server<N>
where
    N: Name,
{
    pub async fn start(hostname: Arc<String>, port: u16) -> io::Result<Self> {
        let listener =
            Listener::<RequestBuf<N>, RpcConnection>::start("localhost".to_string(), 0).await?;
        let addr = listener.external_addr();

        let listener_task = tokio::spawn(async move {
            tokio::pin!(listener);

            while let Some(msg) = listener.next().await {
                match msg {
                    Ok((respond, mut request)) => loop {
                        println!("[Listener::next] !!! received connection");

                        let mut ping = request.next().await;
                        let mut ping_message: RequestBuf<N> = ping.unwrap().unwrap();
                        // assert_eq!(ping_message.pop::<String>().unwrap(), "ping");
                        // println!("[Listener::next] !!! PING message received connection");

                        // let mut pong = MessageBuf::empty();
                        // pong.push(&String::from("pong")).unwrap();
                        // respond.send(pong).await;
                        // println!("[Listener::next] !!! PONG message sent");
                    },
                    Err(err) => panic!("Error: {}", err),
                }
            }
        });

        Ok(Server {
            listener: listener_task,
            external: hostname,
            port: port,
            phantom_data: PhantomData,
        })
    }

    pub fn external_addr(&self) -> (&str, u16) {
        (&*self.external, self.port)
    }
}
