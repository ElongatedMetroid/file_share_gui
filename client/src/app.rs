use std::{net::TcpStream, process, io::{self, Write}};

use retry::{delay::Fixed, retry_with_index};

use eframe::{egui::{self, Key}};

use file_share::{ShareCommand, Share, Location, Config, ShareCommandBuilder, CommandType};

pub struct App {
    stream: TcpStream,
    share: Option<Share>,
    catalog_cache: String,
}

impl Default for App {
    fn default() -> Self {
        let config = Config::build("Config.toml").unwrap_or_else(|error| {
            eprintln!("Config build error: {error}");
            process::exit(1);
        }).client().unwrap_or_else(|error| {
            eprintln!("Config build error: {error}");
            process::exit(1);
        });
    
        let stream = 
        // Retry connecting to the server 10 times, once every 1000 milliseconds
        retry_with_index(Fixed::from_millis(config.retry_delay()).take(config.retry_amount()), |current_try| {
            match TcpStream::connect(config.server()) {
                Ok(stream) => Ok(stream),
                Err(error) => {
                    eprintln!("Connection to server failed, attempt: {current_try}");
                    Err(error)
                },
            }
        });
    
        let stream = stream.unwrap_or_else(|error| {
            eprintln!("Failed to connect to server!: {error}");
            process::exit(1)
        });

        println!("Connected to server!");
        
        Self { stream, share: None, catalog_cache: String::new() }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut command = ShareCommandBuilder::new()
            .command_type(CommandType::Catalog)
            .build();

        egui::CentralPanel::default()
            .show(ctx, |ui| {
                // Update server catalog
                ui.label("Catalog");
            
                // Read in the response the server send, this can contain requested files, text data, etc.
                self.share = Some(send_to_server_receive_from_server(&mut self.stream, &command));
   
                if command.command_type_is(CommandType::Catalog) {
                    self.catalog_cache = self.share.as_ref().unwrap().text_data.as_ref().unwrap().to_string();
                }

                // Display catalog with each file as a button
                for file in self.catalog_cache.split_whitespace() {
                    if ui.button(file).clicked() {
                        command = ShareCommandBuilder::new()
                            .command_type(CommandType::Receive)
                            .arg(file.to_string())
                            .build();
                    }
                }

                // Send the command to the server and get a response (Command will still be CATALOG if none of the buttons were clicked)
                self.share = Some(send_to_server_receive_from_server(&mut self.stream, &command));

                // Execute the command
                self.share.as_mut().unwrap().execute().unwrap();
            });
    }
}

fn send_to_server_receive_from_server(stream: &mut TcpStream, command: &ShareCommand) -> Share {
    let mut share = Share::new(command.clone(), Location::Client);
    // Prepare data (if needed) for the specified command
    if let Err(error) = share.prepare_data() {
        // The error message should be clear to the user (file not found, is a directory, etc.) but better error handling will be 
        // added later
        eprintln!("Error occured while preparing data: {error}");
        process::exit(1);
    }

    // Write the share we prepared to the server/stream
    share.write_to_stream(stream, Location::Client).unwrap_or_else(|error| {
        // will handle these errors later.
        eprintln!("Error occurred: {error}");
        process::exit(1);
    });

    // Make sure all buffered contents reach there destination
    stream.flush().unwrap_or_else(|error| {
        // will handle these errors later.
        eprintln!("Error occurred: {error}");
        process::exit(1);
    });

    // Read in the response the server send, this can contain requested files, text data, etc.
    Share::read_from_stream(stream, Location::Client).unwrap_or_else(|error| {
        // will handle these errors later.
        eprintln!("Error occurred: {error}");
        process::exit(1);
    })
}