use api::common;
use tokio::net::TcpStream;

#[tokio::main]
async fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:6142").await.unwrap();
    let message = api::msg::Message {
        data: api::msg::MessageVarient::ConnectMessage {
            name: String::from("Tester 00"),
        },
    };
    api::seri::fullsend(&message, &mut stream).await;
    api::seri::fullsend(&common::ping(), &mut stream).await;
    println!(
        "{:#?}",
        api::seri::get_message_from_socket(&mut stream).await
    );
    drop(tokio::signal::ctrl_c().await);
}
