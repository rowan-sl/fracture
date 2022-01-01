use std::net::{Ipv4Addr, SocketAddrV4};

// use clap::{App, Arg};
use clap::{Parser, ArgSettings};

/// this should always be a valid u16
const DEFAULT_PORT: &str = "56282";

const ABOUT: &str = "A disruptively terrible chat app that cracks terrible jokes";

const AUTHOR: &str = "Rowan S-L <rowan.with.cats@gmail.com>";

const VERSION: &str = clap::crate_version!();

const NAME: &str = "Fracture (Server)";

#[derive(Clone, Parser, Debug)]
#[clap(name = NAME, about = ABOUT, version = VERSION, author = AUTHOR)]
pub struct RawArgs {
    #[clap(short, long)]
    #[clap(setting(ArgSettings::Required))]
    #[clap(setting(ArgSettings::TakesValue))]
    pub name: String,
    #[clap(short, long)]
    #[clap(setting(ArgSettings::Required))]
    #[clap(setting(ArgSettings::TakesValue))]
    pub addr: String,
    #[clap(short, long, default_value = DEFAULT_PORT)]
    #[clap(setting(ArgSettings::TakesValue))]
    pub port: String,
}

#[derive(Debug)]
pub enum ParserError {
    BadAddr(std::net::AddrParseError),
    BadPort(std::num::ParseIntError),
}

impl From<std::num::ParseIntError> for ParserError {
    fn from(other: std::num::ParseIntError) -> Self {
        Self::BadPort(other)
    }
}

impl From<std::net::AddrParseError> for ParserError {
    fn from(other: std::net::AddrParseError) -> Self {
        Self::BadAddr(other)
    }
}

#[derive(Clone, Parser, Debug)]
pub struct ParsedArgs {
    pub name: String,
    pub full_addr: SocketAddrV4
}

impl ParsedArgs {
    /// Get args and return them, or a error
    pub fn get() -> Result<Self, ParserError> {
        RawArgs::parse().try_into()
    }
}

impl TryFrom<RawArgs> for ParsedArgs {
    type Error = ParserError;

    fn try_from(other: RawArgs) -> Result<Self, Self::Error> {
        Ok(
            Self {
                name: other.name,
                full_addr: {
                    let port = other.port.parse::<u16>()?;
                    let addr = other.addr.parse::<Ipv4Addr>()?;
                    SocketAddrV4::new(addr, port)
                }
            }
        )
    }
}
