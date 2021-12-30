use std::net::SocketAddrV4;

use clap::{App, Arg, ArgGroup};

use fracture_core::utils::ipencoding::code_to_ip_safe;

pub struct Args {
    pub name: String,
    pub addr: SocketAddrV4,
}

/// Get args
///
/// # Panics
/// if anything is wrong with the args. so a lot
pub fn get_args() -> Result<Args, ()> {
    let args = App::new("Fracture")
        .version("v1.0")
        .long_version(clap::crate_version!())
        .author("Rowan S-L <rowan.with.cats@gmail.com>")
        .about("Fracture -- A disruptively terrible chat app that cracks terrible jokes")
        .arg(
            Arg::with_name("code")
                .help("connect to a server, using the servers code")
                .long("code")
                .short("c")
                .takes_value(true)
                .multiple(false),
        )
        .arg(
            Arg::with_name("addr")
                .help("connect to a server, using the servers ip address")
                .long("addr")
                .short("a")
                .takes_value(true)
                .multiple(false),
        )
        .group(
            ArgGroup::with_name("method")
                .arg("code")
                .arg("addr")
                .multiple(false)
                .required(true),
        )
        .arg(
            Arg::with_name("name")
                .help("username for connecting to the server with")
                .long("name")
                .short("n")
                .takes_value(true)
                .multiple(false)
                .required(true),
        )
        .get_matches();

    println!("{:#?}", args);

    let addr = match (args.value_of("code"), args.value_of("addr")) {
        (None, Some(addr)) => {
            if let Ok(parsed_addr) = addr.parse::<SocketAddrV4>() {
                parsed_addr
            } else {
                eprintln!("Failed to parse IP address!");
                return Err(());
            }
        }
        (Some(code), None) => match code_to_ip_safe(String::from(code)) {
            Ok(addr) => addr,
            Err(err) => {
                use fracture_core::utils::ipencoding::CodeToIpError;
                match err {
                    CodeToIpError::Conversion(_) => {
                        eprintln!("Failed to convert code to a valid IP address!");
                        return Err(());
                    }
                    CodeToIpError::Char(ch) => {
                        eprintln!("Unknown charecter {} in input!", ch);
                        return Err(());
                    }
                }
            }
        },
        _ => {
            unreachable!()
        } //handled by arg parser
    };

    let name = if let Some(n) = args.value_of("name") {
        String::from(n)
    } else {
        unreachable!()
    };

    Ok(Args { name, addr })
}
