//! Scrcpy protocol implementation
//!
//! Handles the binary protocol between client and server:
//! - Device metadata on first socket
//! - Codec metadata (12 bytes for video, 4 bytes for audio)
//! - Frame headers (12 bytes) with PTS, flags, and size
//! - Control messages (bidirectional)

use crate::error::{Error, Result};
use bytes::{Bytes, BytesMut};
use tracing::{debug, trace};

/// FourCC helper: pack ASCII bytes into a big-endian u32.
const fn fourcc(s: &[u8; 4]) -> u32 {
    ((s[0] as u32) << 24) | ((s[1] as u32) << 16) | ((s[2] as u32) << 8) | (s[3] as u32)
}

/// Video codec IDs. Values match the 4CC codes emitted by scrcpy-server:
/// `'h264'`, `'h265'`, `'av01'`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VideoCodec {
    H264 = fourcc(b"h264"),
    H265 = fourcc(b"h265"),
    AV1  = fourcc(b"av01"),
}

impl TryFrom<u32> for VideoCodec {
    type Error = Error;

    fn try_from(value: u32) -> Result<Self> {
        const H264: u32 = fourcc(b"h264");
        const H265: u32 = fourcc(b"h265");
        const AV1:  u32 = fourcc(b"av01");
        match value {
            H264 => Ok(VideoCodec::H264),
            H265 => Ok(VideoCodec::H265),
            AV1  => Ok(VideoCodec::AV1),
            _ => Err(Error::Protocol(format!(
                "Unknown video codec: 0x{:08x} ('{}')",
                value,
                fourcc_to_string(value)
            ))),
        }
    }
}

/// Audio codec IDs. Values match the 4CC codes emitted by scrcpy-server:
/// `'opus'`, `'aac '`, `'flac'`, `'raw '`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum AudioCodec {
    OPUS = fourcc(b"opus"),
    AAC  = fourcc(b"aac "),
    FLAC = fourcc(b"flac"),
    RAW  = fourcc(b"raw "),
}

impl TryFrom<u32> for AudioCodec {
    type Error = Error;

    fn try_from(value: u32) -> Result<Self> {
        const OPUS: u32 = fourcc(b"opus");
        const AAC:  u32 = fourcc(b"aac ");
        const FLAC: u32 = fourcc(b"flac");
        const RAW:  u32 = fourcc(b"raw ");
        match value {
            OPUS => Ok(AudioCodec::OPUS),
            AAC  => Ok(AudioCodec::AAC),
            FLAC => Ok(AudioCodec::FLAC),
            RAW  => Ok(AudioCodec::RAW),
            _ => Err(Error::Protocol(format!(
                "Unknown audio codec: 0x{:08x} ('{}')",
                value,
                fourcc_to_string(value)
            ))),
        }
    }
}

/// Render a u32 4CC back to an ASCII-ish string for error messages.
fn fourcc_to_string(v: u32) -> String {
    let bytes = v.to_be_bytes();
    bytes
        .iter()
        .map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' })
        .collect()
}

/// Device metadata sent on first socket
#[derive(Debug, Clone)]
pub struct DeviceMetadata {
    /// Device name
    pub name: String,
}

/// Video codec metadata
#[derive(Debug, Clone)]
pub struct VideoCodecMetadata {
    /// Video codec
    pub codec: VideoCodec,
    /// Initial width
    pub width: u32,
    /// Initial height
    pub height: u32,
}

/// Audio codec metadata
#[derive(Debug, Clone)]
pub struct AudioCodecMetadata {
    /// Audio codec
    pub codec: AudioCodec,
}

/// Frame header (12 bytes)
#[derive(Debug, Clone)]
pub struct FrameHeader {
    /// Is this a config packet?
    pub config_packet: bool,
    /// Is this a key frame?
    pub key_frame: bool,
    /// Presentation timestamp (62 bits)
    pub pts: u64,
    /// Packet data size
    pub size: u32,
}

/// Video/Audio packet
#[derive(Debug, Clone)]
pub struct Packet {
    /// Frame header
    pub header: FrameHeader,
    /// Packet data
    pub data: Bytes,
}

impl DeviceMetadata {
    /// Parse device metadata from bytes
    pub fn parse(data: &[u8]) -> Result<Self> {
        let name = String::from_utf8(data.to_vec())?;
        debug!("Device name: {}", name);
        Ok(Self { name })
    }

    /// Serialize to bytes
    pub fn serialize(&self) -> Vec<u8> {
        self.name.as_bytes().to_vec()
    }
}

impl VideoCodecMetadata {
    /// Parse from 12 bytes
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 12 {
            return Err(Error::Protocol("Video codec metadata too short".to_string()));
        }

        let codec_id = u32::from_be_bytes(data[0..4].try_into().unwrap());
        let width = u32::from_be_bytes(data[4..8].try_into().unwrap());
        let height = u32::from_be_bytes(data[8..12].try_into().unwrap());

        let codec = VideoCodec::try_from(codec_id)?;
        
        debug!(
            "Video codec: {:?}, size: {}x{}",
            codec, width, height
        );

        Ok(Self { codec, width, height })
    }

    /// Serialize to 12 bytes
    pub fn serialize(&self) -> [u8; 12] {
        let mut buf = [0u8; 12];
        buf[0..4].copy_from_slice(&(self.codec as u32).to_be_bytes());
        buf[4..8].copy_from_slice(&self.width.to_be_bytes());
        buf[8..12].copy_from_slice(&self.height.to_be_bytes());
        buf
    }
}

impl AudioCodecMetadata {
    /// Parse from 4 bytes
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 4 {
            return Err(Error::Protocol("Audio codec metadata too short".to_string()));
        }

        let codec_id = u32::from_be_bytes(data[0..4].try_into().unwrap());
        let codec = AudioCodec::try_from(codec_id)?;
        
        debug!("Audio codec: {:?}", codec);

        Ok(Self { codec })
    }

    /// Serialize to 4 bytes
    pub fn serialize(&self) -> [u8; 4] {
        (self.codec as u32).to_be_bytes()
    }
}

impl FrameHeader {
    /// Parse from 12 bytes
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 12 {
            return Err(Error::Protocol("Frame header too short".to_string()));
        }

        // Read first 8 bytes (PTS + flags), big-endian
        let pts_high = u32::from_be_bytes(data[0..4].try_into().unwrap());
        let pts_low = u32::from_be_bytes(data[4..8].try_into().unwrap());

        // Extract flags from most significant bits
        let config_packet = (pts_high & 0x80000000) != 0;
        let key_frame = (pts_high & 0x40000000) != 0;

        // Reconstruct 62-bit PTS
        let pts = ((pts_high & 0x3fffffff) as u64) << 32 | pts_low as u64;

        // Packet size is big-endian u32
        let size = u32::from_be_bytes(data[8..12].try_into().unwrap());

        trace!(
            "Frame header: config={}, key={}, pts={}, size={}",
            config_packet, key_frame, pts, size
        );

        Ok(Self {
            config_packet,
            key_frame,
            pts,
            size,
        })
    }

    /// Serialize to 12 bytes
    pub fn serialize(&self) -> [u8; 12] {
        let mut buf = [0u8; 12];

        // Pack flags and PTS into first 8 bytes
        let pts_high: u32 = (((self.pts >> 32) & 0x3fffffff) as u32)
            | (if self.key_frame { 0x40000000 } else { 0 })
            | (if self.config_packet { 0x80000000 } else { 0 });
        let pts_low = (self.pts & 0xffffffff) as u32;

        buf[0..4].copy_from_slice(&pts_high.to_be_bytes());
        buf[4..8].copy_from_slice(&pts_low.to_be_bytes());
        buf[8..12].copy_from_slice(&self.size.to_be_bytes());

        buf
    }
}

impl Packet {
    /// Parse packet from header and data
    pub fn new(header: FrameHeader, data: Bytes) -> Self {
        Self { header, data }
    }

    /// Check if this is a config packet (SPS/PPS for H.264)
    pub fn is_config(&self) -> bool {
        self.header.config_packet
    }

    /// Check if this is a key frame
    pub fn is_key_frame(&self) -> bool {
        self.header.key_frame
    }

    /// Get packet size
    pub fn size(&self) -> usize {
        self.data.len()
    }
}

/// Protocol reader for parsing incoming data stream
pub struct ProtocolReader {
    pub(crate) buffer: BytesMut,
}

impl ProtocolReader {
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::new(),
        }
    }

    /// Add data to buffer
    pub fn extend(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Try to parse device metadata
    pub fn try_parse_device_metadata(&mut self) -> Option<Result<DeviceMetadata>> {
        // Device metadata is null-terminated string
        if let Some(pos) = self.buffer.iter().position(|&b| b == 0) {
            let data = self.buffer.split_to(pos + 1);
            let data = &data[..pos]; // Remove null terminator
            Some(DeviceMetadata::parse(data))
        } else {
            None
        }
    }

    /// Try to parse video codec metadata (12 bytes)
    pub fn try_parse_video_codec_metadata(&mut self) -> Option<Result<VideoCodecMetadata>> {
        if self.buffer.len() >= 12 {
            let data = self.buffer.split_to(12);
            Some(VideoCodecMetadata::parse(&data))
        } else {
            None
        }
    }

    /// Try to parse audio codec metadata (4 bytes)
    pub fn try_parse_audio_codec_metadata(&mut self) -> Option<Result<AudioCodecMetadata>> {
        if self.buffer.len() >= 4 {
            let data = self.buffer.split_to(4);
            Some(AudioCodecMetadata::parse(&data))
        } else {
            None
        }
    }

    /// Try to parse next packet (12-byte header + data)
    pub fn try_parse_packet(&mut self) -> Option<Result<Packet>> {
        if self.buffer.len() < 12 {
            return None;
        }

        // Parse header first
        let header_result = FrameHeader::parse(&self.buffer[..12]);
        let header = match header_result {
            Ok(h) => h,
            Err(e) => return Some(Err(e)),
        };

        let required_size = 12 + header.size as usize;
        if self.buffer.len() < required_size {
            return None; // Need more data
        }

        // Consume header bytes (already parsed above), then take payload
        let _header_bytes = self.buffer.split_to(12);
        let data = self.buffer.split_to(header.size as usize);

        Some(Ok(Packet::new(header, data.freeze())))
    }

    /// Get remaining buffer length
    pub fn remaining(&self) -> usize {
        self.buffer.len()
    }

    /// Clear buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

impl Default for ProtocolReader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_codec_metadata() {
        let metadata = VideoCodecMetadata {
            codec: VideoCodec::H264,
            width: 1920,
            height: 1080,
        };

        let data = metadata.serialize();
        let parsed = VideoCodecMetadata::parse(&data).unwrap();
        
        assert_eq!(parsed.codec, VideoCodec::H264);
        assert_eq!(parsed.width, 1920);
        assert_eq!(parsed.height, 1080);
    }

    #[test]
    fn test_frame_header() {
        let header = FrameHeader {
            config_packet: false,
            key_frame: true,
            pts: 0x123456789ABCDEF0,
            size: 1024,
        };

        let data = header.serialize();
        let parsed = FrameHeader::parse(&data).unwrap();
        
        assert_eq!(parsed.config_packet, false);
        assert_eq!(parsed.key_frame, true);
        assert_eq!(parsed.pts, 0x123456789ABCDEF0);
        assert_eq!(parsed.size, 1024);
    }

    #[test]
    fn test_protocol_reader() {
        let mut reader = ProtocolReader::new();
        
        // Test incomplete data
        reader.extend(&[1, 2, 3]);
        assert!(reader.try_parse_video_codec_metadata().is_none());
        
        // Test complete metadata
        let metadata = VideoCodecMetadata {
            codec: VideoCodec::H264,
            width: 1920,
            height: 1080,
        };
        reader.extend(&metadata.serialize()[3..]); // Add remaining bytes
        
        let parsed = reader.try_parse_video_codec_metadata().unwrap().unwrap();
        assert_eq!(parsed.codec, VideoCodec::H264);
        assert_eq!(parsed.width, 1920);
        assert_eq!(parsed.height, 1080);
    }
}
