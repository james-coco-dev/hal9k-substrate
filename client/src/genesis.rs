// Copyright 2017 Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! Tool for creating the genesis block.

use std::collections::HashMap;
use primitives::block::{Block, Header};
use primitives::H256;
use triehash::trie_root;

/// Create a genesis block, given the initial storage.
pub fn construct_genesis_block(storage: &HashMap<Vec<u8>, Vec<u8>>) -> Block {
	let state_root = H256(trie_root(storage.clone().into_iter()).0);
	let header = Header {
		parent_hash: Default::default(),
		number: 0,
		state_root,
		transaction_root: H256(trie_root(vec![].into_iter()).0),
		digest: Default::default(),
	};
	Block {
		header,
		transactions: vec![],
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use codec::{Slicable, Joiner};
	use native_runtime::support::{one, two, Hashable};
	use native_runtime::runtime::genesismap::{GenesisConfig, additional_storage_with_genesis};
	use state_machine::execute;
	use state_machine::OverlayedChanges;
	use state_machine::backend::InMemory;
	use polkadot_executor::executor;
	use primitives::{AccountId, Hash, H256};
	use primitives::block::{Number as BlockNumber, Header, Digest};
	use primitives::runtime_function::Function;
	use primitives::transaction::{UncheckedTransaction, Transaction};
	use primitives::contract::CallData;
	use ed25519::Pair;

	fn secret_for(who: &AccountId) -> Option<Pair> {
		match who {
			x if *x == one() => Some(Pair::from_seed(b"12345678901234567890123456789012")),
			x if *x == two() => Some("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60".into()),
			_ => None,
		}
	}

	fn construct_block(backend: &InMemory, number: BlockNumber, parent_hash: Hash, state_root: Hash, txs: Vec<Transaction>) -> (Vec<u8>, Hash) {
		use triehash::ordered_trie_root;

		let transactions = txs.into_iter().map(|transaction| {
			let signature = secret_for(&transaction.signed).unwrap()
				.sign(&transaction.to_vec());

			UncheckedTransaction { transaction, signature }
		}).collect::<Vec<_>>();

		let transaction_root = H256(ordered_trie_root(transactions.iter().map(Slicable::to_vec)).0);

		let mut header = Header {
			parent_hash,
			number,
			state_root,
			transaction_root,
			digest: Digest { logs: vec![], },
		};
		let hash = header.blake2_256();

		let mut overlay = OverlayedChanges::default();

		for tx in transactions.iter() {
			let ret_data = execute(
				backend,
				&mut overlay,
				&executor(),
				"execute_transaction",
				&CallData(vec![].join(&header).join(tx))
			).unwrap();
			header = Header::from_slice(&mut &ret_data[..]).unwrap();
		}

		let ret_data = execute(
			backend,
			&mut overlay,
			&executor(),
			"finalise_block",
			&CallData(vec![].join(&header))
		).unwrap();
		header = Header::from_slice(&mut &ret_data[..]).unwrap();

		(vec![].join(&Block { header, transactions }), H256(hash))
	}

	fn block1(genesis_hash: Hash, backend: &InMemory) -> (Vec<u8>, Hash) {
		construct_block(
			backend,
			1,
			genesis_hash,
			H256(hex!("25e5b37074063ab75c889326246640729b40d0c86932edc527bc80db0e04fe5c")),
			vec![Transaction {
				signed: one(),
				nonce: 0,
				function: Function::StakingTransfer(two(), 69),
			}]
		)
	}

	#[test]
	fn construct_genesis_should_work() {
		let mut storage = GenesisConfig::new_simple(
			vec![one(), two()], 1000
		).genesis_map();
		let block = construct_genesis_block(&storage);
		let genesis_hash = H256(block.header.blake2_256());
		storage.extend(additional_storage_with_genesis(&block).into_iter());

		let mut overlay = OverlayedChanges::default();
		let backend = InMemory::from(storage);
		let (b1data, _b1hash) = block1(genesis_hash, &backend);

		let _ = execute(
			&backend,
			&mut overlay,
			&executor(),
			"execute_block",
			&CallData(b1data)
		).unwrap();
	}
}
