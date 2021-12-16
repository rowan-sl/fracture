use tokio::io::{ AsyncWriteExt };
use tokio::net::TcpStream;

#[tokio::main]
async fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:6142").await.unwrap();
    let message = api::msg::Message {
        data: api::msg::MessageVarient::ConnectMessage {},
    };
    api::seri::fullsend(&message, &mut stream).await;
}