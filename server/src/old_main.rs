use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use queues::Queue;
use queues::IsQueue;
use queues::queue;

use api::Message;

enum SendStatus {
    Sent (usize),
    NoTask,
    Failure (std::io::Error),
}

struct ClientInterface {
    to_send: Queue<Message>,
    socket: TcpStream,
}

impl ClientInterface {
    fn new(sock: TcpStream) -> ClientInterface {
        ClientInterface {
            to_send: queue![],
            socket: sock,
        }
    }

    async fn close(&mut self) {
        //TODO make it send a shutdown message and clear queue and stuff
        match self.socket.shutdown().await {
            Ok(_) => {},
            Err(err) => {
                panic!("Cound not shutdown socket!\n{}", err);
            }
        };
        //& bye bye me
        drop(self);
    }

    async fn send_queued_messages(&mut self) -> SendStatus {
        let value = self.to_send.remove();
        match value {
            Ok(v) => {
                let msg_size = bincode::serialized_size(&v).unwrap();
                
                let encoded: Vec<u8> = bincode::serialize(&v).unwrap();
                let wrote_size = encoded.len();
                let write_status = self.socket.write_all(&encoded).await;
                match write_status {
                    Ok(_) => SendStatus::Sent(wrote_size),
                    Err(err) => SendStatus::Failure(err)
                }
            },
            Err(_err) => {
                SendStatus::NoTask
            }//Do nothing as there is nothing to do ;)
        }
    }

    async fn process_new_message(&mut self) {

    }

    // update the ClientInterface, reading and writing i/o, and doing processing
    async fn update(&mut self) {

    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6142").await?;

    loop {
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(
            async move {
                let interface = ClientInterface::new(socket);
                println!("Connected to {:?}", interface.socket.peer_addr());
            }
        );

        // tokio::spawn(async move {
        //     let mut buf = vec![0; 1024];

        //     loop {
        //         match socket.read(&mut buf).await {
        //             // Return value of `Ok(0)` signifies that the remote has
        //             // closed
        //             Ok(0) => return,
        //             Ok(n) => {
        //                 if socket.write_all(&buf[..n]).await.is_err() {
        //                     // Unexpected socket error. There isn't much we can
        //                     // do here so just stop processing.
        //                     return;
        //                 }
        //                 println!("Echoing {:?}", buf);
        //             }
        //             Err(_) => {
        //                 // Unexpected socket error. There isn't much we can do
        //                 // here so just stop processing.
        //                 return;
        //             }
        //         }
        //     }
        // });
    }
}