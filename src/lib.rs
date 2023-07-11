use crate::transport::message::MessageBuf;
use network::NetworkHandler;
use tokio_stream::StreamExt;

mod file;
mod network;
mod queues;
mod rpc;
mod rpcnet;
mod transport;

#[tokio::main]
async fn main() {
    console_subscriber::init();

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
