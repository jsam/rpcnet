use tokio::{
    io,
    net::{TcpStream, ToSocketAddrs},
};

use crate::{
    queues::{
        listener::Listener, multiplex::make_connection, receiver::Receiver,
        sender::Sender,
    },
    transport::message::MessageBuf,
};

#[derive(Clone, Debug)]
pub struct NetworkHandler {}

impl NetworkHandler {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn connect<E: ToSocketAddrs>(
        &self,
        endpoint: E,
    ) -> io::Result<(Sender, Receiver<MessageBuf>)> {
        let stream = TcpStream::connect(endpoint).await?;
        make_connection(stream)
    }

    pub async fn listen<P: Into<Option<u16>>>(
        &self,
        hostname: String,
        port: P,
    ) -> io::Result<Listener> {
        Listener::start(hostname, port.into().unwrap_or(0)).await
    }
}

#[cfg(test)]
mod tests {

    use crate::{network::NetworkHandler, transport::message::MessageBuf};
    use tokio_stream::StreamExt;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn use_channel_handler() {
        let channel: NetworkHandler = NetworkHandler::new();
        let listener =
            channel.listen("localhost".to_string(), 0).await.unwrap();
        let (tx, mut rx) =
            channel.connect(listener.external_addr()).await.unwrap();

        let tx = tokio::spawn(async move {
            let mut ping = MessageBuf::empty();
            ping.push("ping").unwrap();
            tx.send(ping.clone());
            tx
        })
        .await
        .unwrap();

        let server = tokio::spawn(async move {
            tokio::pin!(listener);

            while let Some(msg) = listener.next().await {
                match msg {
                    Ok((respond, mut request)) => loop {
                        let mut ping = request.next().await;
                        let mut ping_message = ping.unwrap().unwrap();
                        assert_eq!(
                            ping_message.pop::<String>().unwrap(),
                            "ping"
                        );
                        
                        let mut pong = MessageBuf::empty();
                        pong.push(&String::from("pong")).unwrap();
                        respond.send(pong);
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

            assert_eq!("pong", pong.unwrap().unwrap().pop::<String>().unwrap());
            break;
        }
    }
}
