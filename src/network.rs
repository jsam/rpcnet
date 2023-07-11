use std::marker;

use tokio::{
    io,
    net::{TcpStream, ToSocketAddrs},
};

use crate::{
    queues::{
        listener::{Listener, NetworkConnection},
        multiplex::make_channel,
        receiver::Receiver,
        sender::Sender,
    },
    transport::message::FramedMessage,
};

#[derive(Clone, Debug)]
pub struct NetworkHandler<Request, Response>
where
    Request: Send + 'static,
    Response: Send + 'static,
{
    phantom_data: marker::PhantomData<(Request, Response)>,
}

impl<Request, Response> NetworkHandler<Request, Response>
where
    Request: Send + 'static + FramedMessage<Request>,
    Response: Send + 'static + FramedMessage<Response>,
{
    pub fn new() -> Self {
        Self {
            phantom_data: marker::PhantomData,
        }
    }

    /// Connects to a socket returns two queue handles.
    ///
    /// The endpoint socket is typically specified a `(host, port)` pair. The returned
    /// queue handles can be used to send and receive `MessageBuf` objects on that socket.
    /// Please refer to the [`transport`](transport/index.html) module level documentation
    /// for more details.
    pub async fn connect<E: ToSocketAddrs>(
        &self,
        endpoint: E,
    ) -> io::Result<(Sender, Receiver<Response>)> {
        let stream = TcpStream::connect(endpoint).await?;
        make_channel(stream)
    }

    /// Opens a new socket and returns a handle to receive incomming clients.
    ///
    /// If the `port` is not specified, a random ephemerial port is chosen.
    /// Please refer to the [`transport`](transport/index.html) module level documentation
    /// for more details.
    pub async fn listen<P: Into<Option<u16>>>(
        &self,
        hostname: String,
        port: P,
    ) -> io::Result<Listener<Response, NetworkConnection>> {
        Listener::<Response, NetworkConnection>::start(hostname, port.into().unwrap_or(0)).await
    }
}

#[cfg(test)]
mod tests {

    use crate::{network::NetworkHandler, transport::message::MessageBuf};
    use tokio_stream::StreamExt;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn use_channel_handler() {
        let channel: NetworkHandler<MessageBuf, MessageBuf> = NetworkHandler::new();
        let listener = channel.listen("localhost".to_string(), 0).await.unwrap();
        let (tx, mut rx) = channel.connect(listener.external_addr()).await.unwrap();

        let tx = tokio::spawn(async move {
            let mut ping = MessageBuf::empty();
            ping.push("ping").unwrap();
            println!("send ping");
            tx.send(ping.clone()).await;
            println!("message sent!!!");
            tx
        })
        .await
        .unwrap();

        let server = tokio::spawn(async move {
            tokio::pin!(listener);

            while let Some(msg) = listener.next().await {
                match msg {
                    Ok((respond, mut request)) => loop {
                        println!("[Listener::next] !!! received connection");

                        let mut ping = request.next().await;
                        let mut ping_message = ping.unwrap().unwrap();
                        assert_eq!(ping_message.pop::<String>().unwrap(), "ping");
                        println!("[Listener::next] !!! PING message received connection");

                        let mut pong = MessageBuf::empty();
                        pong.push(&String::from("pong")).unwrap();
                        respond.send(pong).await;
                        println!("[Listener::next] !!! PONG message sent");
                    },
                    Err(err) => panic!("Error: {}", err),
                }
            }
        });

        loop {
            let mut pong = rx.next().await;
            if pong.is_none() {
                continue;
            }

            println!("received pong");
            assert_eq!("pong", pong.unwrap().unwrap().pop::<String>().unwrap());
            break;
        }
    }
}
