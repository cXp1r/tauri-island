//! Stream reader for scrcpy video/audio data
//!
//! Provides async stream interface for reading packets from scrcpy server

use crate::error::Result;
use crate::protocol::{Packet, ProtocolReader};
use futures_util::stream::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncReadExt};
use tracing::{debug, trace};

/// Async stream of scrcpy packets
pub struct PacketStream<R> {
    reader: R,
    protocol_reader: ProtocolReader,
    buffer: Vec<u8>,
}

impl<R> PacketStream<R>
where
    R: AsyncRead + Unpin,
{
    /// Create new packet stream
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            protocol_reader: ProtocolReader::new(),
            buffer: Vec::new(),
        }
    }

    /// Read raw data and try to parse packets
    #[allow(dead_code)]
    async fn read_and_parse(&mut self) -> Result<Option<Packet>> {
        // Read some data if buffer is empty
        if self.buffer.is_empty() {
            let mut temp_buf = [0u8; 8192];
            match self.reader.read(&mut temp_buf).await {
                Ok(0) => {
                    debug!("Connection closed");
                    return Ok(None);
                }
                Ok(n) => {
                    trace!("Read {} bytes from socket", n);
                    self.buffer.extend_from_slice(&temp_buf[..n]);
                }
                Err(e) => return Err(e.into()),
            }
        }

        // Add buffer to protocol reader
        self.protocol_reader.extend(&self.buffer);
        self.buffer.clear();

        // Try to parse packet
        if let Some(result) = self.protocol_reader.try_parse_packet() {
            // Keep remaining data in buffer
            self.buffer.extend_from_slice(&self.protocol_reader.buffer);
            self.protocol_reader.clear();
            Some(result).transpose()
        } else {
            // No complete packet yet, keep remaining data
            self.buffer.extend_from_slice(&self.protocol_reader.buffer);
            self.protocol_reader.clear();
            Ok(None) // Need more data
        }
    }
}

impl<R> Stream for PacketStream<R>
where
    R: AsyncRead + Unpin,
{
    type Item = Result<Packet>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // This is a simplified implementation
        // In a real async implementation, we'd need to handle the async read properly
        Poll::Pending
    }
}

/// Synchronous packet stream for demo purposes
pub struct SyncPacketStream<R>
where
    R: std::io::Read,
{
    reader: R,
    protocol_reader: ProtocolReader,
}

impl<R> SyncPacketStream<R>
where
    R: std::io::Read,
{
    /// Create new sync packet stream
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            protocol_reader: ProtocolReader::new(),
        }
    }

    /// Read the next complete packet. Blocks until a full packet is available,
    /// or returns `Err(ConnectionClosed)` on EOF.
    ///
    /// Returns `Ok(None)` only if a spurious zero-byte read occurs before any
    /// data is buffered (practically never).
    pub fn read_packet(&mut self) -> Result<Option<Packet>> {
        use crate::error::Error;
        let mut temp_buf = [0u8; 16 * 1024];

        loop {
            // First try to parse with existing buffered data.
            if let Some(result) = self.protocol_reader.try_parse_packet() {
                return result.map(Some);
            }

            // Need more bytes.
            match self.reader.read(&mut temp_buf) {
                Ok(0) => {
                    debug!("Connection closed (EOF)");
                    return Err(Error::ConnectionClosed);
                }
                Ok(n) => {
                    trace!("Read {} bytes from socket", n);
                    self.protocol_reader.extend(&temp_buf[..n]);
                    // loop: try to parse again
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
}

/// Utility to write packets to file
pub struct PacketWriter<W>
where
    W: std::io::Write,
{
    writer: W,
}

impl<W> PacketWriter<W>
where
    W: std::io::Write,
{
    /// Create new packet writer
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Write packet data (H.264 raw stream)
    pub fn write_packet(&mut self, packet: &Packet) -> Result<()> {
        self.writer.write_all(&packet.data)?;
        Ok(())
    }

    /// Flush writer
    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
        use bytes::Bytes;

    #[test]
    fn test_packet_writer() {
        let data = vec![0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00];
        let packet = Packet::new(
            crate::protocol::FrameHeader {
                config_packet: true,
                key_frame: false,
                pts: 0,
                size: data.len() as u32,
            },
            Bytes::from(data),
        );

        let mut output = Vec::new();
        let mut writer = PacketWriter::new(&mut output);
        writer.write_packet(&packet).unwrap();
        writer.flush().unwrap();

        assert_eq!(output, packet.data);
    }
}
