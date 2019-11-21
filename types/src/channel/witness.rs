use crate::write_set::WriteSet;
use libra_crypto::ed25519::Ed25519Signature;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct WitnessData {
    channel_sequence_number: u64,
    write_set: WriteSet,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Witness {
    data: WitnessData,
    /// Channel participant's signatures.
    signatures: Vec<Ed25519Signature>,
}

impl Witness {
    pub fn new(
        channel_sequence_number: u64,
        write_set: WriteSet,
        signatures: Vec<Ed25519Signature>,
    ) -> Self {
        Self {
            data: WitnessData {
                channel_sequence_number,
                write_set,
            },
            signatures,
        }
    }

    pub fn channel_sequence_number(&self) -> u64 {
        self.data.channel_sequence_number
    }

    pub fn write_set(&self) -> &WriteSet {
        &self.data.write_set
    }

    pub fn signatures(&self) -> &[Ed25519Signature] {
        self.signatures.as_slice()
    }
}
