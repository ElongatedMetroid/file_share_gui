#![feature(core_intrinsics)]

use std::{process, fs::{File, self}, io::{Read, Write, BufReader, BufRead}, mem, net::TcpStream};

use serde::{Deserialize, Serialize};

mod config;
pub use config::Config;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy)]
/// Contains the type of the command
pub enum CommandType {
    // Runs on client
    Exit,
    Help,

    // Runs on server
    Upload,
    Receive,
    Catalog,
}

impl CommandType {
    /// Returns true if the CommandType has an argument
    fn has_arg(&self) -> bool {
        if *self == CommandType::Exit ||
           *self == CommandType::Help ||
           *self == CommandType::Catalog 
        {
            false
        } else {
            true
        }
    }
    /// Returns true if the command runs on the client side
    pub fn is_client(&self) -> bool {
        if *self == CommandType::Exit ||
           *self == CommandType::Help
        {
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
/// Structure contains all data for a Command, the type of command and arguments provided with the command
pub struct ShareCommand {
    command_type: Option<CommandType>,
    arg: Option<String>,
}

impl ShareCommand {
    // TODO: allow ability to use different command parser
    /// Parse a &str into a ShareCommand structure
    pub fn parse(command: &str) -> Result<ShareCommand, Box<dyn std::error::Error>> {
        // Check if the command is empty
        if command.is_empty() {
            return Err("Parse error: Empty command".into());
        }

        // Create an iterator over each word
        let mut command_tokens = command.split_whitespace();

        // Check the type
        let command_type = match command_tokens.next().unwrap() {
            "EXIT" => CommandType::Exit,
            "HELP" => CommandType::Help,

            "UPLOAD" => CommandType::Upload,
            "RECEIVE" => CommandType::Receive,
            "CATALOG" => CommandType::Catalog,

            unknown => {
                return Err(
                    format!(
                        "Parse error: Unknow command type: {unknown}",
                    ).into()
                );
            }
        };

        let arg: Option<String> = match command_tokens.next() {
            // Command uses argument and the argument was found
            Some(arg) if command_type.has_arg() => Some(arg.to_string()),
            // Argument provided with command, but command does not use an argument
            Some(_) if !command_type.has_arg() => {
                return Err(
                    format!(
                        "Parse Error: {:?} does not have an argument",
                        command_type,
                    ).into()
                );
            }
            // Command requires an argument
            None if command_type.has_arg() => {
                return Err("Parse error: No argument provided for command".into());
            },
            // Command does not require an argument
            None if !command_type.has_arg() => None,

            _ => return Err("Parse error: Unknown".into()),
        };

        // Return parsed command
        Ok(ShareCommand { 
            command_type: Some(command_type), 
            arg, 
        })
    }
    /// Returns the CommandType of self
    pub fn command_type(&self) -> Option<&CommandType> {
        self.command_type.as_ref()
    }
    pub fn command_type_is(&mut self, cmp: CommandType) -> bool {
        if let Some(command_type) = self.command_type {
            if command_type == cmp {
                true
            } else {
                false
            }
        } else {
            false
        }
    }
}

#[derive(Debug)]
pub struct ShareCommandBuilder {
    command_type: Option<CommandType>,
    arg: Option<String>,
}

impl ShareCommandBuilder {
    pub fn new() -> ShareCommandBuilder {
        ShareCommandBuilder { command_type: None, arg: None }
    }
    pub fn command_type(mut self, command_type: CommandType) -> ShareCommandBuilder {
        self.command_type = Some(command_type);
        self
    }
    pub fn arg(mut self, arg: String) -> ShareCommandBuilder {
        self.arg = Some(arg);
        self
    }
    pub fn build(self) -> ShareCommand {
        ShareCommand { command_type: self.command_type, arg: self.arg}
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
/// Contains the current location of the data
pub enum Location {
    Client,
    Server,
}

#[derive(Serialize, Deserialize, Debug)]
/// This structure is sent between a server and client
pub struct Share {
    /// Contains the command in execution
    command: ShareCommand,

    /// Contains file data
    file: Option<Vec<u8>>,
    /// Contains text data, this is interpretted diferent ways depending on the
    /// CommandType. This can be file names, the file catalogue, etc.
    pub text_data: Option<String>,
    /// Contains a ServerResponse
    server_response: ServerResponse,
    /// Contains the current location of the Share structure
    current_location: Location,
}

impl Share {
    /// Create a new Share structure, current_location being the location of the data
    pub fn new(command: ShareCommand, current_location: Location) -> Share {
        Share { 
            command, 
            file: None,
            text_data: None, 
            server_response: ServerResponse::new(),
            current_location
        }
    }
    /// Write self to the given stream, this handles all writing including sending the seperate header containing the size of self
    pub fn write_to_stream(&mut self, stream: &mut TcpStream, current_location: Location) -> Result<(), Box<dyn std::error::Error>>{
        // Convert the share to bytes so it can be written to the stream
        let share = bincode::serialize(self)?;

        // Calculate the size (in bytes) of the struct
        let content_len = mem::size_of_val(&share[..]);

        // Send a header containing the content length and a newline
        stream.write(
        format!("{}\n",
                content_len
            ).as_bytes()
        )?;

        // Write the share to the stream
        stream.write_all(&share[..])?;

        // Set the current_location
        self.current_location = current_location;

        Ok(())
    }
    /// Read data from the given stream, this handles all the reading of the sent Share struct. Returns a Result<T, E> containing the 
    /// recieved Share struct on success. Returns a Result<T, E> containing a Box<dyn std::error::Error> on failure, this can mean many
    /// things such as, failing to read the header, failing to parse the header, failing to read the send Share structure, and lastly
    /// failing to deserialize the Share structure
    pub fn read_from_stream(stream: &mut TcpStream, current_location: Location) -> Result<Share, Box<dyn std::error::Error>> {
        // Create an empty buffer, this will be used to read the header
        let mut share_len: String = String::new() ;
        // Wrap the stream in a buf reader
        let mut buf_reader = BufReader::new(stream);
    
        // Read header, the header is formated like `content_length\n`
        buf_reader.read_line(&mut share_len)?;

        // Parse the header into a usize
        let share_len: usize = share_len.trim().parse()?;

        // Create a new buffer that will store the bytes of the Share
        let mut share_bytes = Vec::new();
        // Resize the buffer to the share_len so we can call read_exact()
        share_bytes.resize(share_len, 0);
        
        // Read all the bytes making up the send Share into the bufferr
        buf_reader.read_exact(&mut share_bytes)?;

        // Convert the bytes back into a Share
        let mut share = bincode::deserialize::<Share>(&share_bytes[..])?;

        // Set the current_location
        share.current_location = current_location;

        Ok(share)
    }
    /// Some commands may require this method to work properly, take the Upload command as an example, the Upload command is useless if
    /// there is no file loaded into self.file. Calling this method will prepare any data (like a file) into self. This method may also
    /// be used to handle commands before anything is sent
    /// 
    /// # Panics
    /// The only case this method will panic is if the command passed was None
    pub fn prepare_data(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self.command.command_type() {
            Some(command) => {
                match command {
                    // We dont want Exit to be passed through the server before it can be executed
                    CommandType::Exit => {
                        process::exit(0);
                    }
                    // Help doesnt need any data from the server, but the sever can still view that you have ran help and return any additional
                    // things.
                    CommandType::Help => {
                        println!(
                            "{}\n{}\n{}\n{}\n{}",
                            "----- Help Guide -----",
                            "EXIT - Exit the client",
                            "UPLOAD [file] - Upload a file to the server",
                            "RECEIVE [file] - Receive a file from the server",
                            "CATALOG - Receive a list of files from the server",
                        );
                    }
                    // Load file into vector
                    CommandType::Upload if self.current_location == Location::Client => {
                        let mut file = File::open(self.command.arg.as_ref().unwrap())?;
                    
                        self.file = Some(Vec::new());
                        file.read_to_end(&mut self.file.as_mut().unwrap())?;
                    },  

                    _ => eprintln!("Nothing to prepare"),
                }
            }
            
            None => {
                panic!("command_type is None!");
            }
            
        }

        Ok(())
    }
    /// Execute the command
    /// # Panics
    /// The only case this method will panic is if the command passed was None
    pub fn execute(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // If we are executing on the client side print the server response
        if self.current_location == Location::Client {
            println!("Server says: {:?}. STATUS: {:?}", self.server_response.text, self.server_response.status);
        }
        // If the server reports an error dont execute the command
        if self.server_response.status == ServerResponseStatus::Error {
            return Ok(());
        }

        // Check which command is trying to be executed
        match self.command.command_type() {
            Some(command) => {
                match command {
                    // Received a file from the server; Move file inside memory to storage
                    CommandType::Receive if self.current_location == Location::Client => {
                        let mut file = File::create(self.command.arg.as_ref().unwrap())?;
                    
                        file.write_all(&self.file.as_ref().unwrap())?;
                    }
                    // Send a file to the client; Move file inside storage to memory
                    CommandType::Receive if self.current_location == Location::Server => {
                        let mut file = File::open(self.command.arg.as_ref().unwrap())?;
                    
                        self.file = Some(Vec::new());
                        file.read_to_end(&mut self.file.as_mut().unwrap())?;
                    }
                    // Send a file to the client; Move file inside storage to memory
                    CommandType::Upload if self.current_location == Location::Server => {
                        let mut file = File::create(self.command.arg.as_ref().unwrap())?;
                    
                        file.write_all(&self.file.as_ref().unwrap())?;
                    }
                    // Load text_data with a list of files the server has
                    CommandType::Catalog if self.current_location == Location::Server => {
                        let paths = fs::read_dir(".")?;
                        self.text_data = Some(String::new());
                    
                        for path in paths {
                            self.text_data.as_mut()
                                .unwrap()
                                .push_str(
                                    format!("{}\n", path?.path().display()).as_str().clone()
                                );
                        }
                    }
                
                    _ => (),
                }
            }

            None => {
                panic!("command_type is None!");
            }
        }

        Ok(())
    }
    /// Set the server error response
    pub fn set_error_response(&mut self, error: Box<dyn std::error::Error>) {
        self.server_response.status = ServerResponseStatus::Error;
        // Convert the error to a string
        self.server_response.text = Some(error.to_string());
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
/// Contains the status of the server
enum ServerResponseStatus {
    Error,
    Success,
}

#[derive(Serialize, Deserialize, Debug)]
/// Contains the servers status, and an Option<String> that contains text or None
struct ServerResponse {
    status: ServerResponseStatus,

    text: Option<String>,
}

impl ServerResponse {
    /// Create a new server response
    fn new() -> ServerResponse {
        ServerResponse {
            status: ServerResponseStatus::Success,
            text: Some(String::from("OK")),
        }
    }
}