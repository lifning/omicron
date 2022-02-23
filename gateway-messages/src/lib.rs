// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

pub mod sp_impl;

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

pub use hubpack::error::Error as HubpackError;
pub use hubpack::{deserialize, serialize, SerializedSize};

pub mod version {
    pub const V1: u32 = 1;
}

// TODO: Ignition messages have a `target` for identification; what do the
// other messages need?

/// Messages from a gateway to an SP.
#[derive(Debug, Clone, Copy, SerializedSize, Serialize, Deserialize)]
pub struct Request {
    pub version: u32,
    pub request_id: u32,
    pub kind: RequestKind,
}

#[derive(Debug, Clone, Copy, SerializedSize, Serialize, Deserialize)]
pub enum RequestKind {
    Ping,
    // TODO do we want to be able to request IgnitionState for all targets in
    // one message?
    IgnitionState { target: u8 },
    IgnitionCommand { target: u8, command: IgnitionCommand },
}

// TODO: Not all SPs are capable of crafting all these response kinds, but the
// way we're using hubpack requires everyone to allocate Response::MAX_SIZE. Is
// that okay, or should we break this up more?
#[derive(Debug, Clone, Copy, SerializedSize, Serialize, Deserialize)]
pub enum ResponseKind {
    Pong,
    IgnitionState(IgnitionState),
    IgnitionCommandAck,
    Error(ResponseError),
}

#[derive(Debug, Clone, Copy, SerializedSize, Serialize, Deserialize)]
pub enum ResponseError {
    /// The [RequestKind] is not supported by the receiving SP; e.g., asking an
    /// SP without an attached ignition controller for ignition state.
    RequestUnsupported,
}

/// Messages from an SP to a gateway. Includes both responses to [`Request`]s as
/// well as SP-initiated messages like serial console output.
#[derive(Debug, Clone, Copy, SerializedSize, Serialize, Deserialize)]
pub struct SpMessage {
    pub version: u32,
    pub kind: SpMessageKind,
}

#[derive(Debug, Clone, Copy, SerializedSize, Serialize, Deserialize)]
pub enum SpMessageKind {
    // TODO: Is only sending the new state sufficient?
    // IgnitionChange { target: u8, new_state: IgnitionState },
    /// Response to a [`Request`] from MGS.
    Response { request_id: u32, kind: ResponseKind },

    /// Data traveling from an SP-attached component (in practice, a CPU) on the
    /// component's serial console.
    SerialConsole(SerialConsole),
}

#[derive(Debug, Clone, Copy, SerializedSize, Serialize, Deserialize)]
pub struct IgnitionState {
    pub id: u16,
    pub flags: IgnitionFlags,
}

bitflags! {
    #[derive(SerializedSize, Serialize, Deserialize)]
    pub struct IgnitionFlags: u8 {
        // RFD 142, 5.2.4 status bits
        const POWER = 0b0000_0001;
        const CTRL_DETECT_0 = 0b0000_0010;
        const CTRL_DETECT_1 = 0b0000_0100;
        // const RESERVED_3 = 0b0000_1000;

        // RFD 142, 5.2.3 fault signals
        const FLT_A3 = 0b0001_0000;
        const FLT_A2 = 0b0010_0000;
        const FLT_ROT = 0b0100_0000;
        const FLT_SP = 0b1000_0000;
    }
}

#[derive(Debug, Clone, Copy, SerializedSize, Serialize, Deserialize)]
pub enum IgnitionCommand {
    PowerOn,
    PowerOff,
}

/// Identifier for a single component managed by an SP.
#[derive(Debug, Clone, Copy, SerializedSize, Serialize, Deserialize)]
pub struct SpComponent {
    /// The ID of the component.
    ///
    /// TODO This needs some thought/polish. Is this "up to 16 bytes of human
    /// readable data" (my current thought)? If so, should we add a length field
    /// or specify padding?
    pub id: [u8; 16],
}

#[derive(Debug, Clone, Copy, SerializedSize, Serialize, Deserialize)]
pub struct SerialConsole {
    /// Source component of this serial console data.
    pub component: SpComponent,

    /// Offset of this chunk of data relative to all console ouput this
    /// SP+component has seen since it booted. MGS can determine if it's missed
    /// data and reconstruct out-of-order packets based on this value plus
    /// `len`.
    pub offset: u64,

    /// Number of bytes in `data`.
    pub len: u8,

    /// TODO: What's a reasonable chunk size? Or do we want some variability
    /// here (subject to hubpack limitations or outside-of-hubpack encoding)?
    ///
    /// Another minor annoyance - serde doesn't support arbitrary array sizes
    /// and only implements up to [T; 32], so we'd need a wrapper of some kind to
    /// go higher. See https://github.com/serde-rs/serde/issues/1937
    pub data: [u8; Self::MAX_DATA_PER_PACKET],
}

impl SerialConsole {
    const MAX_DATA_PER_PACKET: usize = 32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_serial_console() {
        let line = "hello world\n";
        let mut console = SerialConsole {
            component: SpComponent { id: *b"0000111122223333" },
            offset: 12345,
            len: line.len() as u8,
            data: [0xff; 32],
        };
        console.data[..line.len()].copy_from_slice(line.as_bytes());

        let mut serialized = [0; SerialConsole::MAX_SIZE];
        let n = serialize(&mut serialized, &console).unwrap();

        let (deserialized, _) =
            deserialize::<SerialConsole>(&serialized[..n]).unwrap();
        assert_eq!(deserialized.len, console.len);
        assert_eq!(deserialized.data, console.data);
    }
}
