use std::net::{Ipv4Addr, SocketAddrV4, AddrParseError};
use std::fs::File;
use std::io::prelude::*;
use std::num::ParseIntError;
use std::path::PathBuf;

// use clap::{App, Arg};
use clap::{Parser, ArgSettings, Subcommand};
use serde::{Serialize, Deserialize};

/// this should always be a valid u16
const DEFAULT_PORT: &str = "56282";

const ABOUT: &str = "A disruptively terrible chat app that cracks bad jokes";

/// Shown when you do -h
const AFTER_HELP: &str = "NOTES: this uses the `env_logger` crate. to change the log level, set the environment variable FRACTURE_LOG_LEVEL";

const AUTHOR: &str = "Rowan S-L <rowan.with.cats@gmail.com>";

const VERSION: &str = clap::crate_version!();

const NAME: &str = "fracture-server";

#[derive(Clone, Parser, Debug)]
#[clap(name = NAME, about = ABOUT, version = VERSION, author = AUTHOR, after_help = AFTER_HELP)]
pub struct CLI {
    #[clap(subcommand)]
    command: Subcommands
}

#[derive(Subcommand, Debug, Clone)]
pub enum Subcommands {
    Launch {
        #[clap(short, long)]
        #[clap(help = "the name of the server, as shown when connecting to it by the client")]
        #[clap(setting(ArgSettings::Required))]
        #[clap(setting(ArgSettings::TakesValue))]
        name: String,
        #[clap(short, long)]
        #[clap(help = "the address to host the server on. this should be your computers address")]
        #[clap(setting(ArgSettings::Required))]
        #[clap(setting(ArgSettings::TakesValue))]
        addr: String,
        #[clap(short, long, default_value = DEFAULT_PORT)]
        #[clap(help = "the port to host the server on. leave blank for the default port (56282) or `0` for the OS to chose a port")]
        #[clap(setting(ArgSettings::TakesValue))]
        port: String,
        #[clap(short, long, help = "save the current args to a config file and exit.", parse(from_os_str))]
        save: Option<PathBuf>,
    },

    FromConfig {
        #[clap(required = true, parse(from_os_str))]
        #[clap(help = "the filepath to load the configuration from")]
        conf: PathBuf
    }
}

#[derive(Debug)]
pub enum ParserErr {
    BadPort(ParseIntError),
    BadAddr(AddrParseError),
}

impl From<AddrParseError> for ParserErr {
    fn from(other: AddrParseError) -> Self {
        Self::BadAddr(other)
    }
}

impl From<ParseIntError> for ParserErr {
    fn from(other: ParseIntError) -> Self {
        Self::BadPort(other)
    }
}

#[derive(Clone, Debug)]
pub struct ParsedArgs {
    pub name: String,
    pub full_addr: SocketAddrV4,
}

impl TryFrom<SemiParsedArgs> for ParsedArgs {
    type Error = ();

    /// Converts to ParsedArgs, failing if if the variant is `SemiParsedArgs::LoadLaunch`
    fn try_from(value: SemiParsedArgs) -> Result<Self, Self::Error> {
        match value {
            SemiParsedArgs::Launch {name, addr, save: _} => {
                Ok(
                    ParsedArgs {
                        name,
                        full_addr: addr,
                    }
                )
            }
            SemiParsedArgs::LoadLaunch {..} => {
                Err(())
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum SemiParsedArgs {
    Launch {
        name: String,
        addr: SocketAddrV4,
        save: Option<PathBuf>,
    },
    LoadLaunch {
        path: PathBuf
    }
}

impl TryFrom<CLI> for SemiParsedArgs {
    type Error = ParserErr;

    fn try_from(args: CLI) -> Result<Self, Self::Error> {
        match args.command {
            Subcommands::Launch {name, addr, port, save} => {
                let parsed_addr = addr.parse::<Ipv4Addr>()?;
                let parsed_port = port.parse::<u16>()?;
                Ok(
                    SemiParsedArgs::Launch {
                        name,
                        addr: SocketAddrV4::new(parsed_addr, parsed_port),
                        save,
                    }
                )
            }
            Subcommands::FromConfig {conf} => {
                Ok(
                    SemiParsedArgs::LoadLaunch {
                        path: conf
                    }
                )
            }
        }
    }
}

// //TODO use configuartion saving (this \/)

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Configuration {
    pub server_name: String,
    pub address: SocketAddrV4,
}

impl From<ParsedArgs> for Configuration {
    fn from(args: ParsedArgs) -> Self {
        Self {
            server_name: args.name,
            address: args.full_addr,
        }
    }
}

impl From<Configuration> for ParsedArgs {
    fn from(conf: Configuration) -> Self {
        Self {
            name: conf.server_name,
            full_addr: conf.address,
        }
    }
}

impl TryFrom<&str> for Configuration {
    type Error = serde_json::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        serde_json::from_str(value)
    }
}

impl TryFrom<String> for Configuration {
    type Error = serde_json::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        serde_json::from_str(&value)
    }
}

impl TryFrom<&Configuration> for String {
    type Error = serde_json::Error;

    fn try_from(conf: &Configuration) -> Result<Self, Self::Error> {
        serde_json::to_string_pretty(conf)
    }
}

impl TryFrom<Configuration> for String {
    type Error = serde_json::Error;

    fn try_from(conf: Configuration) -> Result<Self, Self::Error> {
        serde_json::to_string_pretty(&conf)
    }
}


#[derive(Debug)]
pub enum GetArgsError {
    FileError(std::io::Error),
    Serialization(serde_json::Error),
    InvalidAddr(AddrParseError),
    InvalidPort(ParseIntError),
    Exit,
}

pub fn get_args() -> Result<ParsedArgs, GetArgsError> {
    let possible_args: Result<SemiParsedArgs, ParserErr> = CLI::parse().try_into();
    let semi_parsed = match possible_args {
        Ok(a) => a,
        Err(err) => match err {
            ParserErr::BadAddr(addr_err) => {
                eprintln!("Invalid address!");
                return Err(GetArgsError::InvalidAddr(addr_err));
            }
            ParserErr::BadPort(port_err) => {
                eprintln!("Invalid port!");
                return Err(GetArgsError::InvalidPort(port_err));
            }
        }
    };
    let args = match semi_parsed {
        SemiParsedArgs::Launch {name, addr, save} => {
            match save {
                None => {
                    ParsedArgs {
                        name,
                        full_addr: addr,
                    }
                }
                Some(path) => {
                    let path = path.with_extension("json");
                    if path.exists() {
                        eprintln!("Warning: Overwriting existing configuration at {}", path.display());
                    }
                    let mut conf_file = match File::create(path) {
                        Ok(file) => {file}
                        Err(error) => {
                            eprintln!("Could not open config file for writing:\n{}", error);
                            return Err(GetArgsError::FileError(error));
                        }
                    };
                    let config: Configuration = ParsedArgs {
                        name,
                        full_addr: addr,
                    }.into();
                    let serialize_res: Result<String, serde_json::Error> = config.try_into();
                    let json_config = match serialize_res {
                        Ok(c) => c,
                        Err(error) => {
                            eprintln!("Failed to serialize configuartion:\n{}", error);
                            return Err(GetArgsError::Serialization(error));
                        }
                    };
                    match conf_file.write_all(&mut (&json_config).as_bytes()) {
                        Ok(_) => {
                            println!("Wrote configuartion file");
                            return Err(GetArgsError::Exit)
                        }
                        Err(error) => {
                            eprintln!("Failed to write file!\n{}", error);
                            return Err(GetArgsError::FileError(error));
                        }
                    }
                }
            }
        }
        SemiParsedArgs::LoadLaunch {path} => {
            let display = path.display();
            let mut conf_file = match File::open(path.clone()) {
                Ok(f) => f,
                Err(err) => {
                    eprintln!("Failed to open {}: {}", display, err);
                    return Err(GetArgsError::FileError(err));
                }
            };
            let mut conf_json = String::new();
            match conf_file.read_to_string(&mut conf_json) {
                Err(err) => {
                    eprintln!("Failed to read {}: {}", display, err);
                    return Err(GetArgsError::FileError(err))
                }
                Ok(_) => {
                    println!("Read configuration from {}", display);
                }
            }
            //close the file
            drop(conf_file);
            let conf = match Configuration::try_from(conf_json) {
                Ok(c) => c,
                Err(error) => {
                    println!("Failed to deserialize configuration:\n{}", error);
                    return Err(GetArgsError::Serialization(error));
                }
            };
            conf.into()
        }
    };
    Ok(args)
}