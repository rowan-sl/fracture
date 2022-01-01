mod args;
mod client;
mod handlers;
mod main_task;
mod types;
mod ui;

pub use fracture_config::client as conf;

use std::sync::mpsc;
use std::thread;

use iced::{Application, Settings};
use tokio::runtime::Builder;

use args::get_args;
use main_task::comm_main;
use types::{CommChannels, CommMessage};
use ui::{types::FractureGUIFlags, FractureClientGUI};

fn main() -> Result<(), ()> {
    let args = get_args()?;

    println!("Connecting to: {:?} as {}", args.addr, args.name);
    let name_clone = args.name.clone();

    let (comm_incoming_send, comm_incoming_recv) = mpsc::channel::<CommMessage>();
    let (comm_outgoing_send, comm_outgoing_recv) = mpsc::channel::<CommMessage>();
    let gui_comm_channels = CommChannels {
        sending: comm_outgoing_send,
        receiving: comm_incoming_recv,
    };

    let comm_res = thread::spawn(move || {
        let comm_ctx = Builder::new_current_thread().enable_all().build().unwrap();
        comm_ctx
            .block_on(comm_main(
                comm_incoming_send,
                comm_outgoing_recv,
                args.addr,
                args.name,
            ))
            .expect("Connected sucsessuflly to server");
    });

    FractureClientGUI::run(Settings::with_flags(FractureGUIFlags::new(
        gui_comm_channels,
        name_clone,
    )))
    .unwrap();
    comm_res.join().unwrap();
    Ok(())
}
