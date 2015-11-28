//!
//! Provides a basic ServerTransport implementation
//!

use std::io;
use std::io::{Read, Write, Lines, BufWriter, BufRead, BufReader};
use transport::{ServerTransport, ServerCallback, TransportError};
use std::sync::mpsc::{channel, Sender, Receiver, TryRecvError};
use std::boxed::Box;
use std::thread::{Builder, JoinHandle};

    /// The character that separates payloads in a stream
    const SEPARATOR: u8 = '\n' as u8;

///
/// A server transport that uses a Read stream and a Write stream
///
pub struct ServerStreamTransport {
    /// A channel used to communicate with the reader thread.
    /// A callback can be sent to set the callback to use.
    /// When the channel is closed, the reader thread may terminate.
    channel: Sender<Box<ServerCallback>>,
    /// The handle used to wait for the reader thread to terminate
    handle: JoinHandle<()>,
}

impl ServerStreamTransport {
    pub fn new<R, W>(input: R, output: W) -> Result<ServerStreamTransport, io::Error> where R: 'static + Read + Send, W: 'static + Write + Send {
        let (tx, rx) = channel();

        let mut reader = Reader::new(input, output, rx);
        let handle = try!(Builder::new().name("ServerStreamTransport reader".to_string()).spawn(move || {
            reader.run();
        }));

        Ok(ServerStreamTransport {
            channel: tx,
            handle: handle,
        })
    }
}

impl ServerTransport for ServerStreamTransport {
    fn set_callback<C>(&mut self, callback: C) where C: ServerCallback {
        // Send it to the thread
        match self.channel.send(Box::new(callback)) {
            Err(_) => println!("ServerStreamTransport: Reader thread has terminated"),
            Ok(()) => {},
        }
    }
    fn run(self) {
        let _ = self.handle.join();
    }
}

struct Reader<R, W> where R: 'static + Read + Send, W: 'static + Write + Send {
    /// The iterator used to read lines from the input
    lines: Lines<BufReader<R>>,
    /// The writer used to send output
    writer: BufWriter<W>,
    /// The channel used to receive callbacks from the transport object
    channel: Receiver<Box<ServerCallback>>,
    /// The callback used to handle requests
    callback: Option<Box<ServerCallback>>,
}

impl<R, W> Reader<R, W> where R: 'static + Read + Send, W: 'static + Write + Send {
    pub fn new(input: R, output: W, channel: Receiver<Box<ServerCallback>>) -> Reader<R, W> {
        Reader {
            lines: BufReader::new(input).lines(),
            writer: BufWriter::new(output),
            channel: channel,
            callback: None,
        }
    }

    pub fn handle_read_line(&mut self, line: String) -> Option<String> {
        match self.callback {
            Some(ref mut callback) => callback.handle_request(line),
            None => None,
        }
    }

    pub fn send_response(&mut self, response: &str) -> Result<(), io::Error> {
        try!(self.writer.write_all(response.as_bytes()));
        try!(self.writer.write_all(&[SEPARATOR]));
        try!(self.writer.flush());
        Ok(())
    }

    /// Thread entry point
    pub fn run(&mut self) {
        loop {
            // Check for a new callback or close request
            match self.channel.try_recv() {
                Ok(new_callback) => self.callback = Some(new_callback),
                Err(TryRecvError::Disconnected) => return,
                Err(TryRecvError::Empty) => {},
            };
            // Read a line and get a Result<String, TransportError>
            let line_result = match self.lines.next() {
                Some(Ok(line)) => Ok(line),
                Some(Err(ioe)) => Err(TransportError::from(ioe)),
                None => Err(TransportError::EndOfFile),
            };
            match line_result {
                Ok(line) => {
                    let response = self.handle_read_line(line);
                    if let Some(response) = response {
                        // Write response
                        match self.send_response(&response) {
                            Ok(()) => {},
                            Err(e) => self.handle_transport_error(TransportError::from(e)),
                        }
                    }
                },
                Err(e) => self.handle_transport_error(e),
            };
        }
    }

    fn handle_transport_error(&mut self, e: TransportError) {
        match e {
            // Handle some errors by ignoring this line and proceeding
            TransportError::TimedOut
            | TransportError::Interrupted
            | TransportError::ParseError => {},
            // Handle EOF and other IO errors by terminating
            _ => panic!(format!("IO error: {:?}", e)),
        };
    }
}
