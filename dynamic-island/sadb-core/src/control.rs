//! scrcpy control message serialization.
//!
//! Wire format is big-endian throughout.
//! Reference: scrcpy/app/src/control_msg.c — `sc_control_msg_serialize`.

use crate::error::Result;

/// SC_CONTROL_MSG_TYPE_INJECT_KEYCODE = 0
const TYPE_INJECT_KEYCODE: u8 = 0;
/// SC_CONTROL_MSG_TYPE_INJECT_TEXT = 1
const TYPE_INJECT_TEXT: u8 = 1;
/// SC_CONTROL_MSG_TYPE_INJECT_TOUCH_EVENT = 2
const TYPE_INJECT_TOUCH: u8 = 2;
/// SC_CONTROL_MSG_TYPE_INJECT_SCROLL_EVENT = 3
const TYPE_INJECT_SCROLL: u8 = 3;
/// SC_CONTROL_MSG_TYPE_SET_CLIPBOARD = 9
const TYPE_SET_CLIPBOARD: u8 = 9;
/// SC_CONTROL_MSG_TYPE_GET_CLIPBOARD = 8
const TYPE_GET_CLIPBOARD: u8 = 8;
/// Device message type for clipboard pushed from server to client.
const DEVICE_MSG_TYPE_CLIPBOARD: u8 = 0;
/// Device message type for SET_CLIPBOARD acknowledgement.
const DEVICE_MSG_TYPE_ACK_CLIPBOARD: u8 = 1;

/// `pointer_id` value for a simulated mouse cursor
/// (`SC_POINTER_ID_MOUSE = UINT64_C(-1)`).
pub const POINTER_ID_MOUSE: u64 = u64::MAX;

// ---------------------------------------------------------------------------
// KeyEventAction
// ---------------------------------------------------------------------------

/// Android `AKEY_EVENT_ACTION_*` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum KeyEventAction {
    Down = 0,
    Up   = 1,
}

/// Common Android keycodes useful for control injection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum AndroidKeycode {
    Enter = 66,
    Del   = 67, // backspace
    Escape = 111,
}

// ---------------------------------------------------------------------------
// MotionEventAction
// ---------------------------------------------------------------------------

/// Android `AMOTION_EVENT_ACTION_*` subset used for touch injection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MotionEventAction {
    Down = 0,
    Up   = 1,
    Move = 2,
}

// ---------------------------------------------------------------------------
// MotionEventButtons
// ---------------------------------------------------------------------------

/// Bitmask of `AMOTION_EVENT_BUTTON_*` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionEventButtons(pub u32);

impl MotionEventButtons {
    pub const NONE: Self      = Self(0);
    pub const PRIMARY: Self   = Self(1); // left mouse button
    pub const SECONDARY: Self = Self(2); // right
    pub const TERTIARY: Self  = Self(4); // middle
}

// ---------------------------------------------------------------------------
// InjectTouchEvent — 32 bytes on the wire
// ---------------------------------------------------------------------------

/// Synthetic touch / mouse event injected into the device.
///
/// Wire layout (all big-endian):
/// ```text
/// [0]      type      = 2
/// [1]      action    (MotionEventAction u8)
/// [2..10]  pointer_id (u64)
/// [10..14] x          (i32)
/// [14..18] y          (i32)
/// [18..20] screen_width  (u16)
/// [20..22] screen_height (u16)
/// [22..24] pressure   (u16 fixed-point 0=0.0 0xFFFF=1.0)
/// [24..28] action_button (u32)
/// [28..32] buttons       (u32)
/// ```
#[derive(Debug, Clone)]
pub struct InjectTouchEvent {
    pub action: MotionEventAction,
    pub pointer_id: u64,
    /// X coordinate in device screen pixels (scrcpy stream resolution).
    pub x: i32,
    /// Y coordinate in device screen pixels.
    pub y: i32,
    pub screen_width: u16,
    pub screen_height: u16,
    /// Finger pressure in `[0.0, 1.0]`.  Use `1.0` for DOWN/MOVE, `0.0` for UP.
    pub pressure: f32,
    /// Which button triggered this action (0 for MOVE).
    pub action_button: MotionEventButtons,
    /// All buttons currently pressed.
    pub buttons: MotionEventButtons,
}

impl InjectTouchEvent {
    /// Serialise to the 32-byte on-wire representation.
    pub fn serialize(&self) -> [u8; 32] {
        let mut b = [0u8; 32];
        b[0] = TYPE_INJECT_TOUCH;
        b[1] = self.action as u8;
        b[2..10].copy_from_slice(&self.pointer_id.to_be_bytes());
        b[10..14].copy_from_slice(&self.x.to_be_bytes());
        b[14..18].copy_from_slice(&self.y.to_be_bytes());
        b[18..20].copy_from_slice(&self.screen_width.to_be_bytes());
        b[20..22].copy_from_slice(&self.screen_height.to_be_bytes());
        let p = (self.pressure.clamp(0.0, 1.0) * u16::MAX as f32) as u16;
        b[22..24].copy_from_slice(&p.to_be_bytes());
        b[24..28].copy_from_slice(&self.action_button.0.to_be_bytes());
        b[28..32].copy_from_slice(&self.buttons.0.to_be_bytes());
        b
    }
}

// ---------------------------------------------------------------------------
// InjectKeycodeEvent — 14 bytes on the wire
// ---------------------------------------------------------------------------

/// Keycode injection (used for special keys like Backspace, Enter, Escape).
///
/// Wire layout (all big-endian):
/// ```text
/// [0]      type   = 0
/// [1]      action (KeyEventAction u8)
/// [2..6]   keycode (i32)
/// [6..10]  repeat  (u32)
/// [10..14] metastate (u32)
/// ```
#[derive(Debug, Clone)]
pub struct InjectKeycodeEvent {
    pub action: KeyEventAction,
    pub keycode: i32,
    pub repeat: u32,
    pub metastate: u32,
}

impl InjectKeycodeEvent {
    /// Serialise to the 14-byte on-wire representation.
    pub fn serialize(&self) -> [u8; 14] {
        let mut b = [0u8; 14];
        b[0] = TYPE_INJECT_KEYCODE;
        b[1] = self.action as u8;
        b[2..6].copy_from_slice(&self.keycode.to_be_bytes());
        b[6..10].copy_from_slice(&self.repeat.to_be_bytes());
        b[10..14].copy_from_slice(&self.metastate.to_be_bytes());
        b
    }
}

// ---------------------------------------------------------------------------
// InjectTextEvent — variable length: 5 + text.len() (≤ 305)
// ---------------------------------------------------------------------------

/// Text to inject into the Android input field (via `InputConnection`).
///
/// Wire layout:
/// ```text
/// [0]        type = 1
/// [1..5]     text_len (u32 big-endian)
/// [5..]      UTF-8 bytes (not null-terminated)
/// ```
#[derive(Debug, Clone)]
pub struct InjectTextEvent {
    pub text: String,
}

/// Max length enforced by scrcpy server (300 bytes).
const INJECT_TEXT_MAX_LEN: usize = 300;

impl InjectTextEvent {
    /// Serialise to a dynamically-sized byte vector.
    pub fn serialize(&self) -> Vec<u8> {
        let bytes = self.text.as_bytes();
        let len = bytes.len().min(INJECT_TEXT_MAX_LEN);
        let mut v = Vec::with_capacity(5 + len);
        v.push(TYPE_INJECT_TEXT);
        v.extend_from_slice(&(len as u32).to_be_bytes());
        v.extend_from_slice(&bytes[..len]);
        v
    }
}

/// Max clipboard text payload length (matching scrcpy server's CLIPBOARD_TEXT_MAX_LENGTH).
/// The server enforces a total message size of 256 KiB, so with a 14-byte header
/// the remaining capacity is 262130 bytes.
const CLIPBOARD_TEXT_MAX_LEN: usize = 262_130;

// ---------------------------------------------------------------------------
// SetClipboard — variable length: 14 + text.len()
// ---------------------------------------------------------------------------

/// Set the device clipboard and optionally paste it immediately.
///
/// Wire layout:
/// ```text
/// [0]        type = 9
/// [1..9]     sequence (u64 big-endian, client→server = 0)
/// [9]        paste flag (u8: 0 = set only, 1 = paste)
/// [10..14]   text_len (u32 big-endian)
/// [14..]     UTF-8 bytes (not null-terminated)
/// ```
#[derive(Debug, Clone)]
pub struct SetClipboard {
    pub text: String,
    pub paste: bool,
}

impl SetClipboard {
    /// Serialise to a dynamically-sized byte vector.
    /// Text longer than [`CLIPBOARD_TEXT_MAX_LEN`] is silently truncated to avoid
    /// exceeding the server's 256 KiB message limit.
    pub fn serialize(&self) -> Vec<u8> {
        let bytes = self.text.as_bytes();
        let len = bytes.len().min(CLIPBOARD_TEXT_MAX_LEN);
        let mut v = Vec::with_capacity(14 + len);
        v.push(TYPE_SET_CLIPBOARD);
        v.extend_from_slice(&0u64.to_be_bytes()); // sequence = 0
        v.push(if self.paste { 1 } else { 0 });
        v.extend_from_slice(&(len as u32).to_be_bytes());
        v.extend_from_slice(&bytes[..len]);
        v
    }
}

// ---------------------------------------------------------------------------
// GetClipboard — 2 bytes on the wire
// ---------------------------------------------------------------------------

/// Whether to press a copy/cut key before reading the clipboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CopyKey {
    None = 0,
    Copy = 1,
    Cut = 2,
}

/// Request the device clipboard content.
///
/// Wire layout:
/// ```text
/// [0]        type = 8
/// [1]        copy_key (u8: 0=none, 1=copy, 2=cut)
/// ```
#[derive(Debug, Clone)]
pub struct GetClipboard {
    pub copy_key: CopyKey,
}

impl GetClipboard {
    pub fn serialize(&self) -> [u8; 2] {
        [TYPE_GET_CLIPBOARD, self.copy_key as u8]
    }
}

// ---------------------------------------------------------------------------
// DeviceMessage — server-to-client messages
// ---------------------------------------------------------------------------

/// Messages sent from the scrcpy server back to the client through the control
/// socket (e.g. clipboard change notifications).
///
/// Wire formats:
///
/// ```text
/// CLIPBOARD   [0][text_len:be32(4)][text(N)]
/// ACK_CLIPBOARD [1][sequence:be64(8)]
/// UHID_OUTPUT   [2][id:be16(2)][size:be16(2)][data(N)]
/// ```
#[derive(Debug, Clone)]
pub enum DeviceMessage {
    /// Phone clipboard content (pushed by autosync or in response to GET_CLIPBOARD).
    Clipboard { text: String },
    /// Acknowledgment for a SET_CLIPBOARD request with non-zero sequence.
    AckClipboard { sequence: u64 },
    /// HID output report from a UHID device.
    UhidOutput { id: u16, data: Vec<u8> },
}

impl DeviceMessage {
    /// Try to parse a `DeviceMessage` from the beginning of `data`.
    ///
    /// Returns `Ok(None)` if more bytes are needed; `Ok(Some((msg, consumed)))`
    /// on success; `Err` on invalid data.
    pub fn deserialize(data: &[u8]) -> Result<Option<(Self, usize)>> {
        use crate::error::Error;
        if data.is_empty() {
            return Ok(None);
        }
        match data[0] {
            DEVICE_MSG_TYPE_CLIPBOARD => {
                if data.len() < 5 {
                    return Ok(None);
                }
                let len = u32::from_be_bytes(data[1..5].try_into().unwrap()) as usize;
                if data.len() < 5 + len {
                    return Ok(None);
                }
                let text = String::from_utf8_lossy(&data[5..5 + len]).to_string();
                Ok(Some((DeviceMessage::Clipboard { text }, 5 + len)))
            }
            DEVICE_MSG_TYPE_ACK_CLIPBOARD => {
                if data.len() < 9 {
                    return Ok(None);
                }
                let sequence = u64::from_be_bytes(data[1..9].try_into().unwrap());
                Ok(Some((DeviceMessage::AckClipboard { sequence }, 9)))
            }
            _ => Err(Error::Protocol(format!(
                "Unknown device message type: {}",
                data[0]
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// InjectScrollEvent — 21 bytes on the wire
// ---------------------------------------------------------------------------

/// Synthetic scroll (mouse-wheel) event injected into the device.
///
/// Wire layout (all big-endian):
/// ```text
/// [0]      type  = 3
/// [1..5]   x          (i32)
/// [5..9]   y          (i32)
/// [9..11]  screen_width  (u16)
/// [11..13] screen_height (u16)
/// [13..15] hscroll (i16 fixed-point, value normalised from [-16,16])
/// [15..17] vscroll (i16 fixed-point)
/// [17..21] buttons (u32)
/// ```
#[derive(Debug, Clone)]
pub struct InjectScrollEvent {
    pub x: i32,
    pub y: i32,
    pub screen_width: u16,
    pub screen_height: u16,
    /// Horizontal scroll in `[-16, 16]`.
    pub hscroll: f32,
    /// Vertical scroll in `[-16, 16]`.  Negative = scroll up.
    pub vscroll: f32,
    pub buttons: MotionEventButtons,
}

impl InjectScrollEvent {
    /// Serialise to the 21-byte on-wire representation.
    pub fn serialize(&self) -> [u8; 21] {
        let mut b = [0u8; 21];
        b[0] = TYPE_INJECT_SCROLL;
        b[1..5].copy_from_slice(&self.x.to_be_bytes());
        b[5..9].copy_from_slice(&self.y.to_be_bytes());
        b[9..11].copy_from_slice(&self.screen_width.to_be_bytes());
        b[11..13].copy_from_slice(&self.screen_height.to_be_bytes());
        let h = float_to_i16fp((self.hscroll / 16.0).clamp(-1.0, 1.0));
        let v = float_to_i16fp((self.vscroll / 16.0).clamp(-1.0, 1.0));
        b[13..15].copy_from_slice(&(h as u16).to_be_bytes());
        b[15..17].copy_from_slice(&(v as u16).to_be_bytes());
        b[17..21].copy_from_slice(&self.buttons.0.to_be_bytes());
        b
    }
}

/// Map a float in `[-1.0, 1.0]` to `[i16::MIN, i16::MAX]`
/// (equivalent to scrcpy's `sc_float_to_i16fp`).
fn float_to_i16fp(f: f32) -> i16 {
    if f >= 1.0 {
        i16::MAX
    } else if f <= -1.0 {
        i16::MIN
    } else {
        (f * i16::MAX as f32) as i16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_down_32_bytes() {
        let msg = InjectTouchEvent {
            action: MotionEventAction::Down,
            pointer_id: POINTER_ID_MOUSE,
            x: 100,
            y: 200,
            screen_width: 880,
            screen_height: 1920,
            pressure: 1.0,
            action_button: MotionEventButtons::PRIMARY,
            buttons: MotionEventButtons::PRIMARY,
        };
        let buf = msg.serialize();
        assert_eq!(buf.len(), 32);
        assert_eq!(buf[0], 2); // TYPE_INJECT_TOUCH
        assert_eq!(buf[1], 0); // DOWN
        assert_eq!(&buf[2..10], &u64::MAX.to_be_bytes()); // POINTER_ID_MOUSE
    }

    #[test]
    fn scroll_21_bytes() {
        let msg = InjectScrollEvent {
            x: 0, y: 0, screen_width: 880, screen_height: 1920,
            hscroll: 0.0, vscroll: -3.0,
            buttons: MotionEventButtons::NONE,
        };
        let buf = msg.serialize();
        assert_eq!(buf.len(), 21);
        assert_eq!(buf[0], 3); // TYPE_INJECT_SCROLL
    }
}
