use std::net::{Ipv4Addr, SocketAddrV4};

use serde::{Serialize, Deserialize};

use clap::{App, Arg};

/// this should always be a valid u16
const DEFAULT_PORT: &str = "56282";

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Args {
    pub name: String,
    pub addr: SocketAddrV4,
}

impl Args {
    pub fn deserialize(input: String) -> Result<Self, serde_json::Error> {
        serde_json::from_str(&input)
    }

    pub fn serialize(self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self)
    }
}

#[derive(Debug)]
pub enum ArgsError {
    BadPort,
    BadAddr,
}

/// Get args
///
/// # Panics
/// if anything is wrong with the args. so a lot
pub fn get_args() -> Result<Args, ArgsError> {
    let args = App::new("Fracture")
        .version("v1.0")
        .long_version(clap::crate_version!())
        .author("Rowan S-L <rowan.with.cats@gmail.com>")
        .about("Fracture -- A disruptively terrible chat app that cracks terrible jokes")
        .arg(
            Arg::with_name("name")
                .help("name of the server")
                .long("name")
                .short("n")
                .takes_value(true)
                .multiple(false)
                .required(true)
        )
        .arg(
            Arg::with_name("addr")
                .help("address to host the server on (find this with `ip address` and `good question` on windows. this DOES NOT include the port!!!")
                .long("addr")
                .short("a")
                .takes_value(true)
                .multiple(false)
                .required(true)
        )
        .arg(
            Arg::with_name("port")
                .help(&format!("port to host the server on. leave unspecified for the standard port {} or enter 0 to have one automaticaly chosen.", DEFAULT_PORT))
                .long("port")
                .short("p")
                .takes_value(true)
                .multiple(false)
                .default_value(DEFAULT_PORT)
        )
        .get_matches();

    // println!("{:#?}", args);

    let port = match args.value_of("port").unwrap().parse::<u16>() {
        Ok(port) => port,
        Err(_) => {
            eprintln!("Invalid port!");
            return Err(ArgsError::BadPort)
        }
    };

    let addr = if let Ok(parsed_addr) = args.value_of("addr").unwrap().parse::<Ipv4Addr>() {
        parsed_addr
    } else {
        eprintln!("Failed to parse IP address!");
        return Err(ArgsError::BadAddr);
    };

    let name = args.value_of("name").unwrap().to_string();

    Ok(Args {
        name,
        addr: SocketAddrV4::new(addr, port),
    })
}
