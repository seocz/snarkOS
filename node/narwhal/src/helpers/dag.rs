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

use snarkvm::{
    console::types::{Address, Field},
    ledger::narwhal::BatchCertificate,
    prelude::Network,
};

use std::collections::{BTreeMap, HashMap};

#[derive(Debug)]
pub struct DAG<N: Network> {
    /// The in-memory collection of certificates that comprise the DAG.
    graph: BTreeMap<u64, HashMap<Address<N>, BatchCertificate<N>>>,
    /// The last round that was committed.
    last_committed_round: u64,
    /// The last authors that were committed, along with the round they were committed in.
    last_committed_authors: HashMap<Address<N>, u64>,
}

impl<N: Network> Default for DAG<N> {
    /// Initializes a new DAG.
    fn default() -> Self {
        Self::new()
    }
}

impl<N: Network> DAG<N> {
    /// Initializes a new DAG.
    pub fn new() -> Self {
        Self { graph: Default::default(), last_committed_round: 0, last_committed_authors: Default::default() }
    }

    /// Returns the DAG.
    pub const fn graph(&self) -> &BTreeMap<u64, HashMap<Address<N>, BatchCertificate<N>>> {
        &self.graph
    }

    /// Returns the last committed round.
    pub const fn last_committed_round(&self) -> u64 {
        self.last_committed_round
    }

    /// Returns the last committed authors.
    pub const fn last_committed_authors(&self) -> &HashMap<Address<N>, u64> {
        &self.last_committed_authors
    }

    /// Returns `true` if the given certificate ID exists in the given round.
    pub fn contains_certificate_in_round(&self, round: u64, certificate_id: Field<N>) -> bool {
        self.graph
            .get(&round)
            .map_or(false, |map| map.values().any(|certificate| certificate.certificate_id() == certificate_id))
    }

    /// Returns the batch certificate for the given round and author.
    pub fn get_certificate_for_round_with_author(&self, round: u64, author: Address<N>) -> Option<BatchCertificate<N>> {
        self.graph.get(&round).and_then(|certificates| certificates.get(&author)).cloned()
    }

    /// Returns the batch certificate for the given round and certificate ID.
    pub fn get_certificate_for_round_with_id(
        &self,
        round: u64,
        certificate_id: Field<N>,
    ) -> Option<BatchCertificate<N>> {
        self.graph
            .get(&round)
            .and_then(|map| map.values().find(|certificate| certificate.certificate_id() == certificate_id))
            .cloned()
    }

    /// Returns the batch certificates for the given round.
    pub fn get_certificates_for_round(&self, round: u64) -> Option<HashMap<Address<N>, BatchCertificate<N>>> {
        self.graph.get(&round).cloned()
    }

    /// Inserts a certificate into the DAG.
    pub fn insert(&mut self, certificate: BatchCertificate<N>) {
        let round = certificate.round();
        let author = certificate.author();
        // Insert the certificate into the DAG.
        self.graph.entry(round).or_default().insert(author, certificate);
    }

    /// Commits a certificate, removing all certificates for this author at or before this round from the DAG.
    pub fn commit(&mut self, certificate: BatchCertificate<N>, max_gc_rounds: u64) {
        let certificate_round = certificate.round();
        let author = certificate.author();

        // Update the last committed round for the author.
        self.last_committed_authors
            .entry(author)
            .and_modify(|last_committed_round| {
                if certificate_round > *last_committed_round {
                    *last_committed_round = certificate_round;
                }
            })
            .or_insert(certificate_round);

        // Update the last committed round.
        // Note: The '.unwrap()' here is guaranteed to be safe.
        self.last_committed_round = *self.last_committed_authors.values().max().unwrap();

        // Remove certificates that are below the GC round.
        self.graph.retain(|round, _| round + max_gc_rounds > self.last_committed_round);
        // Remove any certificates for this author that are at or below the certificate round.
        self.graph.retain(|round, map| match *round > certificate_round {
            true => true,
            false => {
                map.remove(&author);
                !map.is_empty()
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm::{
        prelude::{narwhal::batch_certificate::test_helpers::sample_batch_certificate, Testnet3},
        utilities::TestRng,
    };

    #[test]
    fn test_dag_empty() {
        let dag = DAG::<Testnet3>::new();

        assert_eq!(dag.get_certificates_for_round(0), None);
        assert_eq!(dag.last_committed_round(), 0);
        assert_eq!(dag.last_committed_authors().len(), 0);
    }

    #[test]
    fn test_dag_insert() {
        let mut rng = TestRng::default();
        let mut dag = DAG::<Testnet3>::new();

        let certificate = sample_batch_certificate(&mut rng);
        dag.insert(certificate.clone());
        let round = certificate.round();
        assert!(dag.contains_certificate_in_round(round, certificate.certificate_id()));
        assert_eq!(dag.get_certificate_for_round_with_author(round, certificate.author()), Some(certificate.clone()));
        assert_eq!(
            dag.get_certificate_for_round_with_id(round, certificate.certificate_id()),
            Some(certificate.clone())
        );
        assert_eq!(
            dag.get_certificates_for_round(round),
            Some(vec![(certificate.author(), certificate)].into_iter().collect())
        );
        assert_eq!(dag.last_committed_round(), 0);
        assert_eq!(dag.last_committed_authors().len(), 0);
    }

    #[test]
    fn test_dag_commit() {
        let mut rng = TestRng::default();
        let mut dag = DAG::<Testnet3>::new();

        let certificate = sample_batch_certificate(&mut rng);
        dag.insert(certificate.clone());
        let round = certificate.round();
        assert!(dag.contains_certificate_in_round(round, certificate.certificate_id()));
        assert_eq!(dag.get_certificate_for_round_with_author(round, certificate.author()), Some(certificate.clone()));
        assert_eq!(
            dag.get_certificate_for_round_with_id(round, certificate.certificate_id()),
            Some(certificate.clone())
        );
        assert_eq!(
            dag.get_certificates_for_round(round),
            Some(vec![(certificate.author(), certificate.clone())].into_iter().collect())
        );
        assert_eq!(dag.last_committed_round(), 0);
        assert_eq!(dag.last_committed_authors().len(), 0);

        // now commit the certificate, this will trigger GC
        dag.commit(certificate.clone(), 10);
        assert!(!dag.contains_certificate_in_round(round, certificate.certificate_id()));
        assert_eq!(dag.last_committed_round(), round);
        assert_eq!(dag.last_committed_authors().len(), 1);
    }
}