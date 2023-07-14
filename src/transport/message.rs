use std::io::{self, Cursor, ErrorKind, Write};

use serde::de::{Deserialize, DeserializeOwned};
use serde::ser::Serialize;

use bytes::BytesMut;
use rmp_serde::{decode, encode, from_slice};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::queues::sender::Sender;

pub trait FramedMessage<R: Send + 'static> {
    fn from_message(sender: Sender, msg: MessageBuf) -> io::Result<R>;
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageBuf {
    buf: BytesMut,
}

impl FramedMessage<MessageBuf> for MessageBuf {
    fn from_message(sender: Sender, msg: MessageBuf) -> io::Result<MessageBuf> {
        Ok(msg)
    }
}

struct Writer<'a> {
    buf: &'a mut BytesMut,
}

impl<'a> Write for Writer<'a> {
    fn write(&mut self, src: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(src);
        Ok(src.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> Writer<'a> {
    fn new(buf: &'a mut BytesMut) -> Writer<'a> {
        Writer { buf }
    }
}

impl MessageBuf {
    pub fn new<S: Serialize>(item: S) -> io::Result<Self> {
        let mut msg = MessageBuf::empty();
        msg.push(item)
            .map_err(|err| io::Error::new(ErrorKind::Other, err))?;

        Ok(msg)
    }

    pub fn empty() -> Self {
        MessageBuf {
            buf: BytesMut::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn push<S: Serialize>(&mut self, item: S) -> io::Result<()> {
        let mut writer = Writer::new(&mut self.buf);
        encode::write(&mut writer, &item)
            .map_err(|err| io::Error::new(ErrorKind::Other, err))
    }

    pub fn pop<D: DeserializeOwned>(&mut self) -> io::Result<D> {
        let (item, bytes_read) = {
            let mut reader = Cursor::new(&self.buf);
            let item = decode::from_read(&mut reader)
                .map_err(|err| io::Error::new(ErrorKind::Other, err))?;
            let bytes_read = reader.position() as usize;
            (item, bytes_read)
        };

        // TODO: use drain_to instead / avoid the copy / test
        let _ = self.buf.split_to(bytes_read);

        Ok(item)
    }

    pub fn peek<'de, D: Deserialize<'de>>(&'de self) -> io::Result<D> {
        from_slice(&self.buf)
            .map_err(|err| io::Error::new(ErrorKind::Other, err))
    }

    pub async fn write_async<W: AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
    ) -> io::Result<()> {
        writer.write_u32(self.buf.len() as u32).await?;
        let result = writer.write_all(&self.buf).await;
        
        result
    }

    pub async fn read_async<R: AsyncRead + Unpin>(
        mut reader: R,
    ) -> io::Result<Option<MessageBuf>> {
        let length = 4;
        let mut bytes = vec![0u8; length];
        let result = reader.read_exact(&mut bytes).await;

        let length = u32::from_be_bytes(bytes.try_into().unwrap()) as usize;

        let mut bytes = vec![0u8; length];
        let result = reader.read_exact(bytes.as_mut_slice()).await;

        match result {
            Ok(size) => {
                // TODO: use drain_to instead / avoid the copy / test
            }
            // special case: remote host disconnected without sending any new message
            Err(ref err) if err.kind() == ErrorKind::UnexpectedEof => {
                return Ok(None)
            }
            Err(err) => return Err(err),
        }

        let mut buf = BytesMut::new();
        buf.extend_from_slice(&bytes);
        Ok(Some(MessageBuf { buf: buf }))
    }

    #[cfg(feature = "tracing")]
    /// Returns a decoded printable representation of the message
    pub fn debug<'a>(&'a self) -> tracing::Debug<'a> {
        tracing::Debug::new(&*self.buf)
    }
}

impl From<BytesMut> for MessageBuf {
    fn from(buf: BytesMut) -> Self {
        MessageBuf { buf }
    }
}

impl Into<BytesMut> for MessageBuf {
    fn into(self) -> BytesMut {
        self.buf
    }
}

#[cfg(feature = "tracing")]
mod tracing {
    use super::*;
    use rmpv::decode::read_value_ref;
    use std::fmt;

    /// A proxy type implementing a more detailed version of `fmt::Debug`
    pub struct Debug<'a> {
        buf: &'a [u8],
    }

    impl<'a> Debug<'a> {
        pub fn new(buf: &'a [u8]) -> Debug<'a> {
            Debug { buf }
        }
    }

    impl<'a> fmt::Debug for Debug<'a> {
        fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
            let mut remaining = self.buf.len() as u64;
            let mut reader = Cursor::new(self.buf);
            let mut fmt = fmt.debug_tuple("MessageBuf");
            while remaining > 0 {
                let item =
                    read_value_ref(&mut reader).map_err(|_| fmt::Error)?;
                remaining = self.buf.len() as u64 - reader.position();
                fmt.field(&DisplayDebug(item));
            }
            fmt.finish()
        }
    }

    /// Adapter which implements `Debug` by calling into `Display`.
    struct DisplayDebug<T>(T);

    impl<T: fmt::Display> fmt::Debug for DisplayDebug<T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            self.0.fmt(f)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MessageBuf;

    #[test]
    fn push_and_pop_many_msg() {
        let string = String::from("hi");
        let vector = vec![1u8, 2, 3];
        let integer = 42i32;

        let mut buf = MessageBuf::empty();
        buf.push(&string).unwrap();

        assert_eq!(string, buf.peek::<&str>().unwrap());

        buf.push(&vector).unwrap();
        buf.push(&integer).unwrap();
        assert_eq!(string, buf.pop::<String>().unwrap());
        assert_eq!(vector, buf.pop::<Vec<u8>>().unwrap());
        assert_eq!(integer, buf.pop::<i32>().unwrap());
    }

    #[test]
    #[should_panic]
    fn pop_empty() {
        let mut buf = MessageBuf::empty();
        buf.pop::<i32>().unwrap();
    }

    #[test]
    fn type_mismatch() {
        let mut buf = MessageBuf::new(6).unwrap();
        buf.pop::<String>().unwrap_err();
    }
}
