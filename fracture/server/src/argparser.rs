use std::net::{Ipv4Addr, SocketAddrV4, AddrParseError};
use std::fs::File;
use std::io::prelude::*;
use std::num::ParseIntError;
use std::path::PathBuf;

// ! note to self: do not use logging here as it is before the logger is initialized

// use clap::{App, Arg};
use clap::{Parser, ArgSettings, Subcommand};
use serde::{Serialize, Deserialize};


/// this should always be a valid u16
const DEFAULT_PORT: &str = "56282";

const DEFAULT_LOGGER_LEVEL: &str = "info";

const DEFAULT_LOGGER_COLORMODE: &str = "auto";//auto, never, or always

const ABOUT: &str = "A disruptively terrible chat app that cracks bad jokes";

/// Shown when you do -h
// const AFTER_HELP: &str = "NOTES: this uses the `env_logger` crate. to change the log level, set the environment variable FRACTURE_LOG_LEVEL";

const AUTHOR: &str = clap::crate_authors!();

const VERSION: &str = clap::crate_version!();

const NAME: &str = "fracture-server";

#[derive(Clone, Parser, Debug)]
#[clap(name = NAME, about = ABOUT, version = VERSION, author = AUTHOR)]
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

        #[clap(short = 'L', long, default_value = DEFAULT_LOGGER_LEVEL)]
        #[clap(help = "logging level, can be one of error, warn, info, debug, trace. for more details see the clap documentation")]
        #[clap(setting(ArgSettings::TakesValue))]
        logger_level: String,

        #[clap(short = 'S', long, default_value = DEFAULT_LOGGER_COLORMODE)]
        #[clap(help = "color settings for logging. can be auto (color when supported), never (no color), or always (color even if it is unsuported). for more details see the clap documentation")]
        #[clap(setting(ArgSettings::TakesValue))]
        logger_colormode: String,

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
    pub log_style: String,
    pub log_level: String,
}

impl TryFrom<SemiParsedArgs> for ParsedArgs {
    type Error = ();

    /// Converts to ParsedArgs, failing if if the variant is `SemiParsedArgs::LoadLaunch`
    fn try_from(value: SemiParsedArgs) -> Result<Self, Self::Error> {
        match value {
            SemiParsedArgs::Launch {name, addr, log_level, log_style, save: _} => {
                Ok(
                    ParsedArgs {
                        name,
                        full_addr: addr,
                        log_style,
                        log_level
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
        log_level: String,//parsed later
        log_style: String,//also parsed later
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
            Subcommands::Launch {name, addr, port, logger_colormode, logger_level, save} => {
                let parsed_addr = addr.parse::<Ipv4Addr>()?;
                let parsed_port = port.parse::<u16>()?;
                Ok(
                    SemiParsedArgs::Launch {
                        name,
                        addr: SocketAddrV4::new(parsed_addr, parsed_port),
                        save,
                        log_level: logger_level,
                        log_style: logger_colormode,
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
    pub log_level: String,//parsed later
    pub log_style: String,//also parsed later
}

impl From<ParsedArgs> for Configuration {
    fn from(args: ParsedArgs) -> Self {
        Self {
            server_name: args.name,
            address: args.full_addr,
            log_level: args.log_level,
            log_style: args.log_style,
        }
    }
}

impl From<Configuration> for ParsedArgs {
    fn from(conf: Configuration) -> Self {
        Self {
            name: conf.server_name,
            full_addr: conf.address,
            log_level: conf.log_level,
            log_style: conf.log_style,
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
        SemiParsedArgs::Launch {name, addr, log_level, log_style, save} => {
            match save {
                None => {
                    ParsedArgs {
                        name,
                        full_addr: addr,
                        log_level,
                        log_style,
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
                        log_level,
                        log_style,
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