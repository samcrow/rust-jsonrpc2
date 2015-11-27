//!
//! Provides a stream-based client transport layer
//!

use transport::{ClientTransport, PayloadHandler, TransportError};
use std::io::{Read, Write, BufRead, BufReader, Lines, BufWriter};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::thread;


/// The character that separates payloads in a stream
const SEPARATOR: u8 = '\n' as u8;

///
/// A client transport that supports byte streams through Read and Write objects
///
pub struct ClientStreamTransport<W> where W: Write + Send {
    /// Output writer
    output: BufWriter<W>,
    /// Channel used to send new callbacks to the reader
    callback_tx: Sender<Box<PayloadHandler>>,
}

impl<W> ClientStreamTransport<W> where W: Write + Send {
    pub fn new<R>(input: R, output: W) -> ClientStreamTransport<W> where R: 'static + Read + Send {
        let (tx, rx) = channel();
        let mut reader = LineReader::new(input, rx);
        thread::Builder::new().name("ClientStreamTransport reader".to_string()).spawn(move || {
            reader.run();
        }).unwrap();
        ClientStreamTransport {
            output: BufWriter::new(output),
            callback_tx: tx,
        }
    }
}

impl<W> ClientTransport for ClientStreamTransport<W> where W: 'static + Write + Send {
    fn set_payload_handler<H>(&mut self, handler: H) where H: PayloadHandler {
        match self.callback_tx.send(Box::new(handler)) {
            Ok(()) => {},
            Err(_) => println!("ClientStreamTransport: Reader thread has stopped. Cannot set callback."),
        };
    }

    fn send(&mut self, payload: &str) -> Result<(), TransportError>{
        try!(self.output.write_all(payload.as_bytes()));
        try!(self.output.write_all(&[SEPARATOR]));
        try!(self.output.flush());
        Ok(())
    }
}

/// Reads lines from a Read object and provides them to a callback
struct LineReader<R> where R: Read {
    /// Line iterator used for reading
    lines: Lines<BufReader<R>>,
    /// Handler that handles lines that have been read, or None if no handler
    /// has been provided
    handler: Option<Box<PayloadHandler>>,
    /// Channel used to receive new handlers
    handler_rx: Receiver<Box<PayloadHandler>>
}

impl<R> LineReader<R> where R: Read {
    pub fn new(input: R, handler_channel: Receiver<Box<PayloadHandler>>) -> LineReader<R> {
        LineReader {
            lines: BufReader::new(input).lines(),
            handler: None,
            handler_rx: handler_channel,
        }
    }

    /// Updates the handler
    fn update_payload_handler(&mut self) {
        match self.handler_rx.try_recv() {
            Ok(handler) => self.handler = Some(handler),
            Err(TryRecvError::Empty) => {},
            Err(TryRecvError::Disconnected) => panic!("LineReader: Client has hung up, exiting"),
        };
    }
    /// Handles a line that has been read from the input
    fn handle_line(&mut self, line: String) {
        self.update_payload_handler();
        match self.handler {
            Some(ref mut handler) => handler.payload_received(Ok(line)),
            None => println!("LineReader: Read a payload, but no handler is available"),
        };
    }

    pub fn run(&mut self) {
        loop {
            match self.lines.next() {
                Some(Ok(line)) => self.handle_line(line),
                Some(Err(_))
                | None => {
                    println!("LineReader: Failed to read line, exiting");
                    return;
                }
            }
        }
    }
}
