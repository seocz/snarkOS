// Copyright (C) 2019-2023 Aleo Systems Inc.
// This file is part of the snarkOS library.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:
// http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use parking_lot::RwLock;
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    net::SocketAddr,
    sync::Arc,
};

#[derive(Clone, Debug)]
pub struct Pending<T: PartialEq + Eq + Hash> {
    /// The map of pending `transmission IDs` to `peer IPs` that have the transmission.
    pending: Arc<RwLock<HashMap<T, HashSet<SocketAddr>>>>,
}

impl<T: PartialEq + Eq + Hash> Default for Pending<T> {
    /// Initializes a new instance of the pending queue.
    fn default() -> Self {
        Self::new()
    }
}

impl<T: PartialEq + Eq + Hash> Pending<T> {
    /// Initializes a new instance of the pending queue.
    pub fn new() -> Self {
        Self { pending: Default::default() }
    }

    /// Returns `true` if the pending queue is empty.
    pub fn is_empty(&self) -> bool {
        self.pending.read().is_empty()
    }

    /// Returns the number of pending in the pending queue.
    pub fn len(&self) -> usize {
        self.pending.read().len()
    }

    /// Returns `true` if the pending queue contains the specified `transmission ID`.
    pub fn contains(&self, item: impl Into<T>) -> bool {
        self.pending.read().contains_key(&item.into())
    }

    /// Returns `true` if the pending queue contains the specified `transmission ID` for the specified `peer IP`.
    pub fn contains_peer(&self, item: impl Into<T>, peer_ip: SocketAddr) -> bool {
        self.pending.read().get(&item.into()).map_or(false, |peer_ips| peer_ips.contains(&peer_ip))
    }

    /// Returns the peer IPs for the specified `transmission ID`.
    pub fn get(&self, item: impl Into<T>) -> Option<HashSet<SocketAddr>> {
        self.pending.read().get(&item.into()).cloned()
    }

    /// Inserts the specified `transmission ID` and `peer IP` to the pending queue.
    /// If the `transmission ID` already exists, the `peer IP` is added to the existing transmission.
    pub fn insert(&self, item: impl Into<T>, peer_ip: SocketAddr) {
        self.pending.write().entry(item.into()).or_default().insert(peer_ip);
    }

    /// Removes the specified `transmission ID` from the pending queue.
    /// If the `transmission ID` exists and is removed, `true` is returned.
    /// If the `transmission ID` does not exist, `false` is returned.
    pub fn remove(&self, item: impl Into<T>) -> bool {
        self.pending.write().remove(&item.into()).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm::{
        ledger::{coinbase::PuzzleCommitment, narwhal::TransmissionID},
        prelude::{Rng, TestRng},
    };

    type CurrentNetwork = snarkvm::prelude::Testnet3;

    #[test]
    fn test_pending() {
        let rng = &mut TestRng::default();

        // Initialize the ready queue.
        let pending = Pending::<TransmissionID<CurrentNetwork>>::new();

        // Check initially empty.
        assert!(pending.is_empty());
        assert_eq!(pending.len(), 0);

        // Initialize the commitments.
        let commitment_1 = TransmissionID::Solution(PuzzleCommitment::from_g1_affine(rng.gen()));
        let commitment_2 = TransmissionID::Solution(PuzzleCommitment::from_g1_affine(rng.gen()));
        let commitment_3 = TransmissionID::Solution(PuzzleCommitment::from_g1_affine(rng.gen()));

        // Initialize the SocketAddrs.
        let addr_1 = SocketAddr::from(([127, 0, 0, 1], 1234));
        let addr_2 = SocketAddr::from(([127, 0, 0, 1], 2345));
        let addr_3 = SocketAddr::from(([127, 0, 0, 1], 3456));

        // Insert the commitments.
        pending.insert(commitment_1, addr_1);
        pending.insert(commitment_2, addr_2);
        pending.insert(commitment_3, addr_3);

        // Check the number of SocketAddrs.
        assert_eq!(pending.len(), 3);
        assert!(!pending.is_empty());

        // Check the transmission IDs.
        let ids = vec![commitment_1, commitment_2, commitment_3];
        let peers = vec![addr_1, addr_2, addr_3];

        for i in 0..3 {
            let id = ids[i];
            assert!(pending.contains(id));
            assert!(pending.contains_peer(id, peers[i]));
        }
        let unknown_id = TransmissionID::Solution(PuzzleCommitment::from_g1_affine(rng.gen()));
        assert!(!pending.contains(unknown_id));

        // Check get.
        assert_eq!(pending.get(commitment_1), Some(HashSet::from([addr_1])));
        assert_eq!(pending.get(commitment_2), Some(HashSet::from([addr_2])));
        assert_eq!(pending.get(commitment_3), Some(HashSet::from([addr_3])));
        assert_eq!(pending.get(unknown_id), None);

        // Check remove.
        assert!(pending.remove(commitment_1));
        assert!(pending.remove(commitment_2));
        assert!(pending.remove(commitment_3));
        assert!(!pending.remove(unknown_id));

        // Check empty again.
        assert!(pending.is_empty());
    }
}