//!
//! Provides abstractions for transport layers used in JSON RPC
//!
//!

use std::io;
use std::convert::From;

///
/// Errors that a transport layer can report
///
#[derive(Debug)]
pub enum TransportError {
    /// The transport layer has reached an end-of-file condition or something similar
    /// and will not be able to send or receive anything
    EndOfFile,
    /// An operation timed out
    TimedOut,
    /// The thread was interrupted while waiting
    Interrupted,
    /// Data received could not be parsed
    ParseError,
    /// Data to be sent could not be encoded
    EncodeError,
    /// A different error
    IOError(io::Error),
}

/// Creates a TransportError from a corresponding io::Error
impl From<io::Error> for TransportError {
    fn from(io_err: io::Error) -> Self {
        match io_err.kind() {
            io::ErrorKind::NotFound
            | io::ErrorKind::PermissionDenied
            | io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::NotConnected
            | io::ErrorKind::AddrInUse
            | io::ErrorKind::AddrNotAvailable
            | io::ErrorKind::BrokenPipe => TransportError::EndOfFile,

            io::ErrorKind::TimedOut
            | io::ErrorKind::WriteZero
            | io::ErrorKind::WouldBlock => TransportError::TimedOut,

            io::ErrorKind::Interrupted => TransportError::Interrupted,

            _ => TransportError::IOError(io_err),
        }
    }
}

///
/// Trait for something to be notified when a payload from the server is received
///
pub trait PayloadHandler: 'static + Send {
    ///
    /// Called when a response from the server is received
    ///
    fn payload_received(&mut self, result: Result<String, TransportError>);
}
/// PayloadHandler implementation for closures
impl<F> PayloadHandler for F where F: Fn(Result<String, TransportError>), F: 'static + Send {
    fn payload_received(&mut self, result: Result<String, TransportError>) {
        self(result)
    }
}

///
/// Trait for a transport layer used by a client
///
/// I is a key type used to identify requests and corresponding responses.
///
pub trait ClientTransport : 'static + Send {

    ///
    /// Sets the payload handler that this object should notify when a payload is received
    ///
    fn set_payload_handler<H>(&mut self, handler: H) where H: PayloadHandler;

    ///
    /// Sends a payload
    ///
    /// If a response is received, the response handler associated with this object will be called
    /// and provided with the response.
    ///
    /// This function must not block.
    ///
    fn send(&mut self, payload: &str) -> Result<(), TransportError>;
}

///
/// A trait for something that can handle a request and provide a response
///
/// This trait includes Send and Sync so that transport layers can use multiple threads.
///
pub trait ServerCallback: 'static + Send + Sync {
    ///
    /// Handles a received request. Returns an optional response to send back to the client.
    ///
    fn handle_request(&mut self, request: String) -> Option<String>;
}

/// ServerCallback implementation for closures
impl<F> ServerCallback for F where F: Fn(String) -> Option<String>, F: 'static + Send + Sync {
    fn handle_request(&mut self, request: String) -> Option<String> {
        self(request)
    }
}

///
/// Trait for a transport layer used by a server
///
pub trait ServerTransport : 'static + Send {
    ///
    /// Sets the callback that this transport layer will use to respond to requests
    ///
    fn set_callback<C>(&mut self, callback: C) where C: ServerCallback;
    ///
    /// Runs the transport mechanism and returns when an end of file is reached
    ///
    fn run(self);
}
