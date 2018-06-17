//! Socket primitives for working with MDP.
//!
//! The `Socket` type provides the equivalent of a UDP socket, except over an MDP virtual overlay
//! network. It's provided by and associated with a particular instance of Protocol. MDP has its
//! own address and port space that is distinct from whatever IP-based or other network it happens
//! to be running on, but provides an interface similar to other socket libraries. Multiple sockets
//! can be associated with a single instance of `Protocol`, using the same or different MDP addresses
//! (each of which represents an elliptic-curve public key), but only one socket can be bound to
//! any particular address/port combination at one time.
use std::sync::{Arc, Mutex};
use futures::{prelude::*, sync::mpsc::UnboundedReceiver};
use addr::{Addr, LocalAddr, SocketAddr, ADDR_EMPTY};
use bytes::{BufMut, BytesMut};
use error::Error;
use packet::Packet;
use protocol::ProtocolInfo;
use qos;

pub use packet::QOS_DEFAULT;
pub use packet::TTL_MAX;
pub use packet::State;
pub use qos::Class;

/// The default TTL value for MDP sockets.
pub const TTL_DEFAULT: u8 = 31;
pub const READ_BUFFER_DEFAULT: usize = 64 * 1200;
pub const WRITE_BUFFER_DEFAULT: usize = 8 * 1200;

/// A virtual socket on an MDP overlay network.
///
/// Provided by the bind method on `Protocol`, `SocketAddr` implements both `Futures::Stream` and
/// `Futures::Sink` for bidirectional asynchronous handling of messages over the network. The Socket
/// is bound to an MDP SocketAddr, which is comprised of both a public key and a 32-bit MDP port
/// number (which is only meaningful within the overlay network and is distinct from an IP port).
///
/// # Example
/// 
/// Bind MDP socket on port 5000 using the default address and a second socket on port 5001 using a
/// different address.
/// ```
/// use mdp::addr::LocalAddr;
/// use mdp::protocol::Protocol;
///
/// let local_addr = LocalAddr::new();
/// let mut proto = Protocol::new(&local_addr);
/// let socket0 = proto.bind(&local_addr, 5000).expect("Failed to bind MDP socket.");
/// let new_addr = LocalAddr::new();
/// let socket1= proto.bind(&new_addr, 5001).expect("Failed to bind MDP socket.");
/// ```
pub struct Socket {
    proto: Arc<Mutex<ProtocolInfo>>,
    incoming: UnboundedReceiver<Packet>,
    index: usize,
    addr: LocalAddr,
    port: u32,
    qos: qos::Class,
    ttl: u8,
}

impl Socket {
    pub(crate) fn new(
        proto: &Arc<Mutex<ProtocolInfo>>,
        incoming: UnboundedReceiver<Packet>,
        index: usize,
        addr: &LocalAddr,
        port: u32,
    ) -> Socket {
        Socket {
            proto: proto.clone(),
            incoming: incoming,
            index: index,
            addr: addr.clone(),
            port: port,
            qos: qos::Class::Ordinary,
            ttl: TTL_DEFAULT,
        }
    }

    /// Get the time-to-live value for this socket.
    ///
    /// For more information about this option, see ['set_ttl'][link]
    ///
    /// [link]: #method.set_ttl
    /// # Example
    /// ```
    /// use mdp::addr::LocalAddr;
    /// use mdp::protocol::Protocol;
    /// use mdp::socket::{Socket, TTL_DEFAULT};
    ///
    /// let local_addr = LocalAddr::new();
    /// let mut proto = Protocol::new(&local_addr);
    /// let socket = proto.bind(&local_addr, 5000).expect("Failed to bind MDP socket.");
    /// assert_eq!(socket.ttl(), TTL_DEFAULT);
    /// ```
    pub fn ttl(&self) -> u8 {
        self.ttl
    }

    /// Set the time-to-live value for this socket.
    ///
    /// This sets the TTL value that is used for every message sent from this socket. The maximum
    /// TTL value for MDP is 31, so any value higher than 31 is automatically set to the maximum.
    ///
    /// # Example
    /// ```
    /// use mdp::addr::LocalAddr;
    /// use mdp::protocol::Protocol;
    /// use mdp::socket::Socket;
    ///
    /// let local_addr = LocalAddr::new();
    /// let mut proto = Protocol::new(&local_addr);
    /// let mut socket = proto.bind(&local_addr, 5000).expect("Failed to bind MDP socket.");
    /// socket.set_ttl(6);
    /// assert_eq!(socket.ttl(), 6);
    /// ```
    pub fn set_ttl(&mut self, ttl: u8) {
        if ttl > TTL_MAX {
            self.ttl = TTL_MAX
        } else {
            self.ttl = ttl
        }
    }

    /// Get the default Quality-of-Service class for this socket.
    ///
    /// For more information about this option, see [`set_qos`][link].
    ///
    /// [link]: #method.set_qos
    /// # Example
    /// ```
    /// use mdp::addr::LocalAddr;
    /// use mdp::protocol::Protocol;
    /// use mdp::socket::{Socket, QOS_DEFAULT};
    ///
    /// let local_addr = LocalAddr::new();
    /// let mut proto = Protocol::new(&local_addr);
    /// let socket = proto.bind(&local_addr, 5000).expect("Failed to bind MDP socket.");
    /// assert_eq!(socket.qos(), QOS_DEFAULT);
    /// ```
    pub fn qos(&self) -> qos::Class {
        self.qos
    }

    /// Set the default Quality-of-Service class for this socket.
    ///
    /// This sets the Quality-of-Service class that every message sent from this socket is sorted
    /// into. This affects scheduling and timeouts of outgoing messages.
    /// # Example
    /// ```
    /// use mdp::addr::LocalAddr;
    /// use mdp::protocol::Protocol;
    /// use mdp::socket::{Class, Socket};
    /// # fn main() {
    /// let local_addr = LocalAddr::new();
    /// let mut proto = Protocol::new(&local_addr);
    /// let mut socket = proto.bind(&local_addr, 5000).expect("Failed to bind MDP socket.");
    /// socket.set_qos(Class::Management);
    /// assert_eq!(socket.qos(), Class::Management);
    /// # }
    /// ```
    pub fn set_qos(&mut self, class: qos::Class) {
        self.qos = class
    }

    /// Get the value of the broadcast flag for this socket.
    ///
    /// For more information about this option, see [`set_broadcast`][link].
    ///
    /// [link]: #method.set_broadcast
    /// # Example
    /// ```
    /// use mdp::addr::LocalAddr;
    /// use mdp::protocol::Protocol;
    /// use mdp::socket::Socket;
    ///
    /// let local_addr = LocalAddr::new();
    /// let mut proto = Protocol::new(&local_addr);
    /// let socket = proto.bind(&local_addr, 5000).expect("Failed to bind MDP socket.");
    /// assert_eq!(socket.broadcast(), false);
    /// ```
    pub fn broadcast(&self) -> bool {
        let mut proto = self.proto.lock().unwrap();
        if let Some(ref info) = proto.socket_info(self.index) {
            info.broadcast()
        } else {
            error!("MDP Protocol is unaware of socket #{}!", self.index);
            false
        }
    }

    /// Set the broadcast flag for this socket.
    ///
    /// When set, this Socket will receive MDP broadcast messages sent to its port. Defaults to
    /// `false`.
    /// # Example
    /// ```
    /// use mdp::addr::LocalAddr;
    /// use mdp::protocol::Protocol;
    /// use mdp::socket::Socket;
    ///
    /// let local_addr = LocalAddr::new();
    /// let mut proto = Protocol::new(&local_addr);
    /// let mut socket = proto.bind(&local_addr, 5000).expect("Failed to bind MDP socket.");
    /// socket.set_broadcast(true);
    /// assert_eq!(socket.broadcast(), true);
    /// ```
    pub fn set_broadcast(&mut self, on: bool) {
        let mut proto = self.proto.lock().unwrap();
        if let Some(ref mut info) = proto.socket_info(self.index) {
            info.set_broadcast(on)
        } else {
            error!("MDP Protocol is unaware of socket #{}!", self.index);
        }
    }

    fn send_to(&mut self, buf: &mut BytesMut, to: &SocketAddr, state: State) -> StartSend<(), Error> {
        let mut proto = self.proto.lock().unwrap();
        let seq = match proto.socket_info(self.index) {
            Some(s) => s.next_seq(),
            None => -1,
        };
        let addr = Addr::from(&self.addr);
        let mut packet = Packet::new(
            SocketAddr::from((addr, self.port)),
            *to,
            ADDR_EMPTY,
            self.ttl,
            self.qos,
            seq,
            false,
            buf,
        );

        match state {
            State::Signed => {
                packet.sign(&self.addr)?;
            }
            State::Encrypted => {
                packet.encrypt(&self.addr)?;
            }
            State::Plain => (),
        }

        trace!("socket start_send: try_send #1");
        let packet = match proto.try_send(packet)? {
            Some(packet) => packet,
            None => return Ok(AsyncSink::Ready)
        };

        if !proto.poll_interfaces_complete()? {
            trace!("socket start_send: try_send #2");
            match proto.try_send(packet)? {
                Some(_) => Ok(AsyncSink::NotReady(())),
                None => Ok(AsyncSink::Ready),
            }
        } else {
            trace!("socket start_send: ready");
            Ok(AsyncSink::Ready)
        }
    }

    /// Sends a single encrypted MDP Message on this socket to the address specified.
    ///
    /// The destination address can be any object that implements `Into<SocketAddr>`. This method
    /// will send the contents of the provided `BytesMut` (encrypted with the destination key) and 
    /// return `AsyncSink::Ready` when the socket is ready for another message.
    ///
    /// # Example
    /// 
    /// Bind MDP socket on port 5000 using the default address and a second socket on port 5001 using a
    /// different address, then send a packet.
    /// ```
    /// extern crate bytes;
    /// extern crate futures;
    /// extern crate mdp;
    /// use bytes::BytesMut;
    /// use futures::Async;
    /// use mdp::addr::{LocalAddr, SocketAddr};
    /// use mdp::protocol::Protocol;
    /// use mdp::socket::State;
    ///
    /// let local_addr = LocalAddr::new();
    /// let mut proto = Protocol::new(&local_addr);
    /// let mut socket0 = proto.bind(&local_addr, 5000).expect("Failed to bind MDP socket.");
    /// let new_addr = LocalAddr::new();
    /// let mut socket1= proto.bind(&new_addr, 5001).expect("Failed to bind MDP socket.");
    /// let mut text = BytesMut::from(&b"Hello!"[..]);
    /// socket0.send_to_encrypt(&mut text.clone(), (&new_addr, 5001)).expect("Failed to send message");
    /// if let Ok(Async::Ready((received, from, state))) = socket1.recv_from() {
    ///     assert_eq!(text, received);
    ///     assert_eq!(from, SocketAddr::from((&local_addr, 5000)));
    ///     assert_eq!(state, State::Encrypted);
    /// } else {
    ///     panic!("Failed to receive message.");
    /// }
    /// ```
    pub fn send_to_encrypt<A: Into<SocketAddr>>(&mut self, buf: &mut BytesMut, to: A) -> StartSend<(), Error> {
        self.send_to(buf, &to.into(), State::Encrypted)
    }

    /// Sends a single signed MDP Message on this socket to the address specified.
    ///
    /// The destination address can be any object that implements `Into<SocketAddr>`. This method
    /// will send the contents of the provided `BytesMut` (signed with the source key) and return 
    /// `AsyncSink::Ready` when the socket is ready for another message.
    ///
    /// # Example
    /// 
    /// Bind MDP socket on port 5000 using the default address and a second socket on port 5001 using a
    /// different address, then send a packet.
    /// ```
    /// extern crate bytes;
    /// extern crate futures;
    /// extern crate mdp;
    /// use bytes::BytesMut;
    /// use futures::Async;
    /// use mdp::addr::{LocalAddr, SocketAddr};
    /// use mdp::protocol::Protocol;
    /// use mdp::socket::State;
    ///
    /// let local_addr = LocalAddr::new();
    /// let mut proto = Protocol::new(&local_addr);
    /// let mut socket0 = proto.bind(&local_addr, 5000).expect("Failed to bind MDP socket.");
    /// let new_addr = LocalAddr::new();
    /// let mut socket1= proto.bind(&new_addr, 5001).expect("Failed to bind MDP socket.");
    /// let mut text = BytesMut::from(&b"Hello!"[..]);
    /// socket0.send_to_sign(&mut text.clone(), (&new_addr, 5001)).expect("Failed to send message");
    /// if let Ok(Async::Ready((received, from, state))) = socket1.recv_from() {
    ///     assert_eq!(text, received);
    ///     assert_eq!(from, SocketAddr::from((&local_addr, 5000)));
    ///     assert_eq!(state, State::Signed);
    /// } else {
    ///     panic!("Failed to receive message.");
    /// }
    /// ```
    pub fn send_to_sign<A: Into<SocketAddr>>(&mut self, buf: &mut BytesMut, to: A) -> StartSend<(), Error> {
        self.send_to(buf, &to.into(), State::Signed)
    }

    /// Sends a single plaintext MDP Message on this socket to the address specified.
    ///
    /// The destination address can be any object that implements `Into<SocketAddr>`. This method
    /// will send the contents of the provided `BytesMut` and return `AsyncSink::Ready` when the
    /// socket is ready for another message.
    ///
    /// # Example
    /// 
    /// Bind MDP socket on port 5000 using the default address and a second socket on port 5001 using a
    /// different address, then send a packet.
    /// ```
    /// extern crate bytes;
    /// extern crate futures;
    /// extern crate mdp;
    /// use bytes::BytesMut;
    /// use futures::Async;
    /// use mdp::addr::{LocalAddr, SocketAddr};
    /// use mdp::protocol::Protocol;
    /// use mdp::socket::State;
    ///
    /// let local_addr = LocalAddr::new();
    /// let mut proto = Protocol::new(&local_addr);
    /// let mut socket0 = proto.bind(&local_addr, 5000).expect("Failed to bind MDP socket.");
    /// let new_addr = LocalAddr::new();
    /// let mut socket1= proto.bind(&new_addr, 5001).expect("Failed to bind MDP socket.");
    /// let mut text = BytesMut::from(&b"Hello!"[..]);
    /// socket0.send_to_plain(&mut text.clone(), (&new_addr, 5001)).expect("Failed to send message");
    /// if let Ok(Async::Ready((received, from, state))) = socket1.recv_from() {
    ///     assert_eq!(text, received);
    ///     assert_eq!(from, SocketAddr::from((&local_addr, 5000)));
    ///     assert_eq!(state, State::Plain);
    /// } else {
    ///     panic!("Failed to receive message.");
    /// }
    /// ```
    pub fn send_to_plain<A: Into<SocketAddr>>(&mut self, buf: &mut BytesMut, to: A) -> StartSend<(), Error> {
        self.send_to(buf, &to.into(), State::Plain)
    }

    /// Receives a single MDP Message on this socket from the address specified.
    ///
    /// The destination address can be any object that implements `Into<SocketAddr>`. This method
    /// will send the contents of the provided `BytesMut` and return `AsyncSink::Ready` when the
    /// socket is ready for another message.
    ///
    /// # Example
    /// 
    /// For examples, see ['send_to_plain'][plain], ['send_to_sign'][sign], and
    /// ['send_to_encrypt'][encrypt].
    ///
    /// [plain]: #method.send_to_plain
    /// [sign]: #method.send_to_sign
    /// [encrypt]: #method.send_to_encrypt
    pub fn recv_from(&mut self) -> Poll<(BytesMut, SocketAddr, State), Error> {
        while let Some(mut packet) =
            try_ready!(self.incoming.poll().map_err(|_| Error::InvalidSocket))
        {
            trace!("Received message: {:?}", packet);
            let state = packet.state();
            match state {
                State::Plain => {
                    return Ok(Async::Ready((
                        packet.take()?,
                        (*packet.src(), packet.src_port().unwrap()).into(),
                        state
                    )))
                }
                State::Signed => {
                    packet.verify()?;
                    return Ok(Async::Ready((
                        packet.take()?,
                        (*packet.src(), packet.src_port().unwrap()).into(),
                        state
                    )));
                }
                State::Encrypted => {
                    packet.decrypt(&self.addr)?;
                    if let Some(dst_port) = packet.dst_port() {
                        if dst_port == self.port {
                            return Ok(Async::Ready((
                                packet.take()?,
                                (*packet.src(), packet.src_port().unwrap()).into(),
                                state
                            )));
                        } else {
                            debug!("Decrypted message for a different socket, requeueing it.");
                            let mut proto = self.proto.lock().unwrap();
                            proto.try_send(packet)?;
                            continue;
                        }
                    }
                }
            }
        }
        Ok(Async::NotReady)
    }

    pub(crate) fn proto(&self) -> &Arc<Mutex<ProtocolInfo>> {
        &self.proto
    }
}

/// Trait of helper objects to write out messages as bytes, for use with
/// `FramedWrite`.
pub trait Encoder {
    /// The type of items consumed by the `Encoder`
    type Item;

    /// The type of encoding errors.
    ///
    /// `FramedWrite` requires `Encoder`s errors to implement `From<io::Error>`
    /// in the interest letting it return `Error`s directly.
    type Error: From<Error>;

    /// Encodes a frame into the buffer provided.
    ///
    /// This method will encode `item` into the byte buffer provided by `dst`.
    /// The `dst` provided is an internal buffer of the `Framed` instance and
    /// will be written out when possible.
    fn encode(&mut self, item: Self::Item, buf: &mut BytesMut)
              -> Result<(), Self::Error>;
}

/// Decoding of frames via buffers.
///
/// This trait is used when constructing an instance of `Framed` or
/// `FramedRead`. An implementation of `Decoder` takes a byte stream that has
/// already been buffered in `src` and decodes the data into a stream of
/// `Self::Item` frames.
///
/// Implementations are able to track state on `self`, which enables
/// implementing stateful streaming parsers. In many cases, though, this type
/// will simply be a unit struct (e.g. `struct HttpDecoder`).
pub trait Decoder {
    /// The type of decoded frames.
    type Item;

    /// The type of unrecoverable frame decoding errors.
    ///
    /// If an individual message is ill-formed but can be ignored without
    /// interfering with the processing of future messages, it may be more
    /// useful to report the failure as an `Item`.
    ///
    /// `From<io::Error>` is required in the interest of making `Error` suitable
    /// for returning directly from a `FramedRead`, and to enable the default
    /// implementation of `decode_eof` to yield an `io::Error` when the decoder
    /// fails to consume all available data.
    ///
    /// Note that implementors of this trait can simply indicate `type Error =
    /// io::Error` to use I/O errors as this type.
    type Error: From<Error>;

    /// Attempts to decode a frame from the provided buffer of bytes.
    ///
    /// This method is called by `FramedRead` whenever bytes are ready to be
    /// parsed.  The provided buffer of bytes is what's been read so far, and
    /// this instance of `Decode` can determine whether an entire frame is in
    /// the buffer and is ready to be returned.
    ///
    /// If an entire frame is available, then this instance will remove those
    /// bytes from the buffer provided and return them as a decoded
    /// frame. Note that removing bytes from the provided buffer doesn't always
    /// necessarily copy the bytes, so this should be an efficient operation in
    /// most circumstances.
    ///
    /// If the bytes look valid, but a frame isn't fully available yet, then
    /// `Ok(None)` is returned. This indicates to the `Framed` instance that
    /// it needs to read some more bytes before calling this method again.
    ///
    /// Note that the bytes provided may be empty. If a previous call to
    /// `decode` consumed all the bytes in the buffer then `decode` will be
    /// called again until it returns `None`, indicating that more bytes need to
    /// be read.
    ///
    /// Finally, if the bytes in the buffer are malformed then an error is
    /// returned indicating why. This informs `Framed` that the stream is now
    /// corrupt and should be terminated.
    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error>;

    /// A default method available to be called when there are no more bytes
    /// available to be read from the underlying I/O.
    ///
    /// This method defaults to calling `decode` and returns an error if
    /// `Ok(None)` is returned while there is unconsumed data in `buf`.
    /// Typically this doesn't need to be implemented unless the framing
    /// protocol differs near the end of the stream.
    ///
    /// Note that the `buf` argument may be empty. If a previous call to
    /// `decode_eof` consumed all the bytes in the buffer, `decode_eof` will be
    /// called again until it returns `None`, indicating that there are no more
    /// frames to yield. This behavior enables returning finalization frames
    /// that may not be based on inbound data.
    fn decode_eof(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match try!(self.decode(buf)) {
            Some(frame) => Ok(Some(frame)),
            None => {
                if buf.is_empty() {
                    Ok(None)
                } else {
                    Err(Error::MalformedPacket.into())
                }
            }
        }
    }
}

/// A simple `Codec` implementation that just ships bytes around.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BytesCodec(());

impl BytesCodec {
    /// Creates a new `BytesCodec` for shipping around raw bytes.
    pub fn new() -> BytesCodec { BytesCodec(())  }
}

impl Decoder for BytesCodec {
    type Item = BytesMut;
    type Error = Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if buf.len() > 0 {
            let len = buf.len();
            Ok(Some(buf.split_to(len)))
        } else {
            Ok(None)
        }
    }
}

impl Encoder for BytesCodec {
    type Item = BytesMut;
    type Error = Error;

    fn encode(&mut self, data: Self::Item, buf: &mut BytesMut) -> Result<(), Self::Error> {
        buf.reserve(data.len());
        buf.put(data);
        Ok(())
    }
}

pub struct Framed<C> {
    socket: Socket,
    codec: C,
    wr: BytesMut,
}

impl<C> Framed<C> {
    pub fn new(socket: Socket, codec: C) -> Framed<C> {
        Framed {
            socket: socket,
            codec: codec,
            wr: BytesMut::with_capacity(WRITE_BUFFER_DEFAULT),
        }
    }

    pub fn get_ref(&self) -> &Socket {
        &self.socket
    }

    pub fn get_mut(&mut self) -> &mut Socket {
        &mut self.socket
    }

    pub fn into_inner(self) -> Socket {
        self.socket
    }
}

impl<C> Stream for Framed<C> where C: Decoder {
    type Item = (C::Item, SocketAddr, State);
    type Error = C::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        while let Some(mut packet) = try_ready!(self.socket.incoming.poll().map_err(|_| Error::InvalidSocket)) {
            trace!("Received message: {:?}", packet);
            let state = packet.state();
            match state {
                State::Plain => {
                    let src_port = packet.src_port().ok_or(Error::MalformedPacket)?;
                    let src = SocketAddr::from((*packet.src(), src_port));
                    let item = self.codec.decode(packet.contents_mut()?)?.ok_or(Error::MalformedPacket)?;
                    return Ok(Async::Ready(Some((item, src, state))))
                },
                State::Signed => {
                    packet.verify()?;
                    let src_port = packet.src_port().ok_or(Error::MalformedPacket)?;
                    let src = SocketAddr::from((*packet.src(), src_port));
                    let item = self.codec.decode(packet.contents_mut()?)?.ok_or(Error::MalformedPacket)?;
                    return Ok(Async::Ready(Some((item, src, state))))
                },
                State::Encrypted => {
                    packet.decrypt(&self.socket.addr)?;
                    if let Some(dst_port) = packet.dst_port() {
                        if dst_port == self.socket.port {
                            let src_port = packet.src_port().ok_or(Error::MalformedPacket)?;
                            let src = SocketAddr::from((*packet.src(), src_port));
                            let item = self.codec.decode(packet.contents_mut()?)?.ok_or(Error::MalformedPacket)?;
                            return Ok(Async::Ready(Some((item, src, state))))
                        } else {
                            debug!("Decrypted message for a different socket, requeueing it.");
                            let mut proto = self.socket.proto.lock().unwrap();
                            proto.try_send(packet)?;
                            continue;
                        }
                    }
                }
            }
        }
        Ok(Async::NotReady)
    }
}

impl<C: Encoder> Sink for Framed<C> where <C as Encoder>::Error: From<Error> {
    type SinkItem = (C::Item, SocketAddr, State);
    type SinkError = C::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        let mut proto = self.socket.proto.lock().unwrap();
        let seq = match proto.socket_info(self.socket.index) {
            Some(s) => s.next_seq(),
            None => -1,
        };
        let addr = Addr::from(&self.socket.addr);
        let _contents = self.codec.encode(item.0, &mut self.wr)?;
        let mut packet = Packet::new(
            (addr, self.socket.port).into(),
            item.1,
            ADDR_EMPTY,
            self.socket.ttl,
            self.socket.qos,
            seq,
            false,
            &mut self.wr,
        );
        match item.2 {
            State::Signed => {
                packet.sign(&self.socket.addr)?;
            }
            State::Encrypted => {
                packet.encrypt(&self.socket.addr)?;
            }
            State::Plain => (),
        }

        trace!("socket start_send: try_send #1");
        let packet = match proto.try_send(packet)? {
            Some(packet) => packet,
            None => return Ok(AsyncSink::Ready),
        };

        if !proto.poll_interfaces_complete()? {
            trace!("socket start_send: try_send #2");
            let _ = proto.try_send(packet)?;
        } else {
            trace!("socket start_send: ready");
        }
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        let mut proto = self.socket.proto.lock().unwrap();
        if !proto.poll_interfaces_complete()? {
            trace!("socket poll_complete: not all interfaces complete");
            Ok(Async::NotReady)
        } else {
            trace!("socket poll_complete: all interfaces complete");
            Ok(Async::Ready(()))
        }
    }
}
