#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::traits::Get;
use frame_system::{
	offchain::{
		AppCrypto, CreateSignedTransaction, SendSignedTransaction,
		SignedPayload, Signer, SigningTypes,
	},
};
use sp_core::{crypto::KeyTypeId, convert_hash, H256};
use sp_runtime::{
	offchain::{
		storage::{StorageValueRef},
	},
	traits::{Hash},
	RuntimeDebug,
};
use sp_std::{convert::TryInto, prelude::*, vec, collections::btree_map::BTreeMap};

pub(crate) const LOCAL_RANDOM: &[u8] = b"parity/commit-reveal-local-random";
pub(crate) const CURRENT_STATE: &[u8] = b"parity/commit-reveal-state";
pub(crate) const STATE_SET_INDEX: &[u8] = b"partity/commit-reveal-state-set-index";

pub(crate) const STATE_INIT :u64 = 0;
pub(crate) const STATE_COMMIT :u64 = 1;
pub(crate) const STATE_WAIT_COMMIT_DONE :u64 = 2;
pub(crate) const STATE_REVEAL :u64 = 3;
pub(crate) const STATE_WAIT_REVEAL_DONE :u64 = 4;
pub(crate) const STATE_REVEAL_DONE :u64 = 5;
pub(crate) const STATE_FINISH :u64 = 6;

pub(crate) const MEMBER_COUNT: usize = 5;

#[cfg(test)]
mod tests;

/// Defines application identifier for crypto keys of this module.
///
/// Every module that deals with signatures needs to declare its unique identifier for
/// its crypto keys.
/// When offchain worker is signing transactions it's going to request keys of type
/// `KeyTypeId` from the keystore and use the ones it finds to sign the transaction.
/// The keys can be inserted manually via RPC (see `author_insertKey`).
pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"comm");

/// Based on the above `KeyTypeId` we need to generate a pallet-specific crypto type wrappers.
/// We can use from supported crypto kinds (`sr25519`, `ed25519` and `ecdsa`) and augment
/// the types with this pallet-specific identifier.
pub mod crypto {
	use super::KEY_TYPE;
	use sp_core::sr25519::Signature as Sr25519Signature;
	use sp_runtime::{
		app_crypto::{app_crypto, sr25519},
		traits::Verify,
		MultiSignature, MultiSigner,
	};
	app_crypto!(sr25519, KEY_TYPE);

	pub struct TestAuthId;
	impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
		type RuntimeAppPublic = Public;
		type GenericSignature = sp_core::sr25519::Signature;
		type GenericPublic = sp_core::sr25519::Public;
	}
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// This pallet's configuration trait
	#[pallet::config]
	pub trait Config: CreateSignedTransaction<Call<Self>> + frame_system::Config {
		/// The identifier type for an offchain worker.
		type AuthorityId: AppCrypto<Self::Public, Self::Signature>;

		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The overarching dispatch call type.
		type Call: From<Call<Self>>;

		// Configuration parameters

		/// A grace period after we send transaction.
		///
		/// To avoid sending too many transactions, we only attempt to send one
		/// every `GRACE_PERIOD` blocks. We use Local Storage to coordinate
		/// sending between distinct runs of this offchain worker.
		#[pallet::constant]
		type GracePeriod: Get<Self::BlockNumber>;

		/// Number of blocks of cooldown after unsigned transaction is included.
		///
		/// This ensures that we only accept unsigned transactions once, every `UnsignedInterval` blocks.
		#[pallet::constant]
		type UnsignedInterval: Get<Self::BlockNumber>;

		/// A configuration for base priority of unsigned transactions.
		///
		/// This is exposed so that it can be tuned for particular runtime, when
		/// multiple pallets send unsigned transactions.
		#[pallet::constant]
		type UnsignedPriority: Get<TransactionPriority>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// A public part of the pallet.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
        #[pallet::weight(0)]
        pub fn proposal_commit(origin: OriginFor<T>, _now: T::BlockNumber, hash: T::Hash)
		-> DispatchResult{
			let caller = ensure_signed(origin)?;

			log::info!("[insert >>> ([{:?}]: {:?}]", caller, hash);
			<CommitResult<T>>::insert(&caller, hash);
			Ok(())
        }

		// user method
		#[pallet::weight(0)]
		pub fn proposal_reveal(origin: OriginFor<T>, _now: T::BlockNumber, num: u64)
		-> DispatchResult{
			let caller = ensure_signed(origin)?;

			log::info!("[insert >>> ([{:?}]: {}]", caller, num);
			<RevealResult<T>>::insert(&caller, num);

			Ok(().into())
		}
	}

	/// Events for the pallet.
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event generated when new price is accepted to contribute to the average.
		/// \[price, who\]
		NewPrice(u32, T::AccountId),
	}

	/// A vector of recently submitted prices.
	///
	/// This is used to calculate average price, should have bounded size.
	#[pallet::storage]
	#[pallet::getter(fn prices)]
	pub(super) type Prices<T: Config> = StorageValue<_, Vec<u32>, ValueQuery>;

	/// Defines the block when next unsigned transaction will be accepted.
	///
	/// To prevent spam of unsigned (and unpayed!) transactions on the network,
	/// we only allow one transaction every `T::UnsignedInterval` blocks.
	/// This storage entry defines when new transaction is going to be accepted.
	#[pallet::storage]
	#[pallet::getter(fn next_unsigned_at)]
	pub(super) type NextUnsignedAt<T: Config> = StorageValue<_, T::BlockNumber, ValueQuery>;

	// user storage
    #[pallet::storage]
    pub(crate) type RevealResult<T: Config> =
        StorageMap<_, Twox64Concat, T::AccountId, u64, ValueQuery>;

    #[pallet::storage]
    pub(crate) type CommitResult<T: Config> =
        StorageMap<_, Twox64Concat, T::AccountId, T::Hash, ValueQuery>;

    #[pallet::storage]
    pub(crate) type NextMembers<T: Config> =
        StorageValue<_, Vec<T::AccountId>, ValueQuery>;

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn offchain_worker(block_number: T::BlockNumber) {
			// let res = Self::submit_signed_tx(block_number);
			// if let Err(e) = res {
			// 	log::error!("offchain err: {:?}", e);
			// }

			if let Err(e) = Self::offchain_cb(block_number){
				log::info!("offchain err: {:?}", e);
			}
		}
	}
}

/// Payload used by this example crate to hold price
/// data required to submit a transaction.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct PricePayload<Public, BlockNumber> {
	block_number: BlockNumber,
	price: u32,
	public: Public,
}

impl<T: SigningTypes> SignedPayload<T> for PricePayload<T::Public, T::BlockNumber> {
	fn public(&self) -> T::Public {
		self.public.clone()
	}
}

impl<T: Config> Pallet<T> {
	fn offchain_cb(block_number: T::BlockNumber) -> Result<(), &'static str>{
		let block_number_u64 = TryInto::<u64>::try_into(block_number).unwrap_or(0u64);

		if block_number_u64%10 == 2{
			// let n = 100u64;
			// let n_hash = T::Hashing::hash(&n.to_be_bytes());
			// let n2_hash = T::Hashing::hash_of(&n);
			// let n3_hash = T::Hashing::hash(&n.to_le_bytes());
			// log::info!("{:?}, {:?}, {:?}", n_hash, n2_hash, n3_hash);

			// let seed = sp_io::offchain::random_seed();
			// let s1_hash = T::Hashing::hash_of(&seed);
			// let s2_hash = H256::from_slice(&seed);
			// log::info!("{:?}, {:?}",s1_hash, s2_hash);
			// let r = s1_hash^s2_hash;
			// log::info!("{:?}", r);

			let seed = sp_io::offchain::random_seed();
			let s0_hash = T::Hashing::hash_of(&seed);
			log::info!("{:?}", s0_hash);

			let s0_raw = s0_hash.encode();
			let s0 = H256::from_slice(&s0_raw);

			// let s1_hash = H256::from(&seed);
			// let s1_hash = seed.hash();
			// let s1_hash = H256::hash(&seed);
			// let s1_hash = H256::from_low_u64_be(0);

			// let hash_raw = sp_io::hashing::blake2_256(&seed);
			// let s1_hash = T::Hash::from(seed);
			// log::info!("{:?}", s1_hash);

			// let r = s1_hash^s1_hash;
			// log::info!("{:?}", r);

			let s2_hash = H256::from_slice(&seed);
			log::info!("{:?}", s2_hash);

			let r = s0^s2_hash;
			log::info!("{:?}", r);


			// let r = s1_hash^s2_hash;
			// log::info!("{:?}", r);
		}

		Ok(())
	}

	// user method
	fn submit_signed_tx(block_number: T::BlockNumber) -> Result<(), &'static str> {
		let block_number_u64 = TryInto::<u64>::try_into(block_number).unwrap_or(0u64);

		if block_number_u64 < 3 {
			if block_number_u64 == 1{
				let state_memory = StorageValueRef::persistent(&CURRENT_STATE);
				let index_memory = StorageValueRef::persistent(&STATE_SET_INDEX);

				state_memory.set(&STATE_INIT);
				index_memory.set(&block_number_u64);
			}
			return Ok(())
		}

		let state_memory = StorageValueRef::persistent(&CURRENT_STATE);
		let state_ret = state_memory.get::<u64>().map_err(|_|"GetLocalError")?;
		let state = state_ret.unwrap();

		let index_memory = StorageValueRef::persistent(&STATE_SET_INDEX);
		let index_ret = index_memory.get::<u64>().map_err(|_|"GetLocalError")?;
		let last_index = index_ret.unwrap();

		match state{
			STATE_INIT => {
				log::info!("## state init");
				Self::clean_pallet_storage();
				state_memory.set(&STATE_COMMIT);
				index_memory.set(&block_number_u64);


				let random_number = {
					let seed = sp_io::offchain::random_seed();
					let mut raw = [0u8;8];
					raw[0..8].copy_from_slice(&seed[0..8]);
					u64::from_be_bytes(raw)
				};

				let local_mem = StorageValueRef::persistent(&LOCAL_RANDOM);
				local_mem.set(&random_number);

				let random_hash = T::Hashing::hash(&random_number.to_be_bytes());
				log::info!("{:?}", random_hash);
				log::info!("store random number: {}", random_number);
			},
			STATE_COMMIT => {
				if block_number_u64 == last_index + 5{
					log::info!("## state commit");
					let ret = Self::send_commit_hash(block_number);
					if let Ok(_) = ret{
						state_memory.set(&STATE_WAIT_COMMIT_DONE);
						index_memory.set(&block_number_u64);
					}
					return ret;
				}
				else{
					log::info!("## state commit standby");
				}
			},
			STATE_WAIT_COMMIT_DONE => {
				log::info!("## state wait commit done");
				if <CommitResult<T>>::iter().count() == MEMBER_COUNT {
					state_memory.set(&STATE_REVEAL);
					index_memory.set(&block_number_u64);
				}
			},
			STATE_REVEAL => {
				if block_number_u64 == last_index + 5{
					log::info!("## state reveal");
					let ret = Self::send_reveal_number(block_number);
					if let Ok(_) = ret {
						state_memory.set(&STATE_WAIT_REVEAL_DONE);
						index_memory.set(&block_number_u64);
					}
					return ret;
				}
				else{
					log::info!("## state reveal standby");
				}
			},
			STATE_WAIT_REVEAL_DONE => {
				log::info!("## state wait reveal done");
				if <RevealResult<T>>::iter().count() == MEMBER_COUNT {
					// if block_number_u64 == last_index + 5{
					state_memory.set(&STATE_REVEAL_DONE);
					index_memory.set(&block_number_u64);
					// }
				}
			},
			STATE_REVEAL_DONE => {
				log::info!("## state wait verify");
				if block_number_u64 == last_index + 5{
					let ret = Self::verify_vote_result(block_number);
					state_memory.set(&STATE_FINISH);
					index_memory.set(&block_number_u64);
					return ret;
				}
			},
			STATE_FINISH => {
				if block_number_u64 == last_index + 20{
					log::info!("next round");
					state_memory.set(&STATE_INIT);
					index_memory.set(&block_number_u64);
				}
				else{
					log::info!("## state finish");
				}
			},
			_ =>{}
		}

		Ok(())
	}

	fn send_commit_hash(block_number: T::BlockNumber)->Result<(), &'static str>
	{
		let signer = Signer::<T, T::AuthorityId>::all_accounts();
		if !signer.can_sign() {
			return Err(
				"No local accounts available. Consider adding one via `author_insertKey` RPC.",
			)?
		}

		let local_random = StorageValueRef::persistent(&LOCAL_RANDOM);
		let random_ret = local_random.get::<u64>().map_err(|_|"GetLocalErr")?;
		let random_number = random_ret.unwrap();

		let number_hash = T::Hashing::hash(&random_number.to_be_bytes()[..]);

		let results = signer.send_signed_transaction(|_account| {
			// let raw_vec = account.id.encode();
			// log::info!("{:?}", raw_vec);
			Call::proposal_commit(block_number, number_hash)
		});

		for (acc, res) in &results {
			match res {
				Ok(()) => log::info!("Submitted commit [{:?}]: {:?}({}) ", acc.id, number_hash, random_number),
				Err(e) => log::error!("[{:?}] Failed to submit transaction: {:?}", acc.id, e),
			}
		}
		Ok(())
	}

	fn send_reveal_number(block_number: T::BlockNumber) -> Result<(), &'static str>{
		let signer = Signer::<T, T::AuthorityId>::all_accounts();
		if !signer.can_sign() {
			return Err(
				"No local accounts available. Consider adding one via `author_insertKey` RPC.",
			)?
		}

		let local_random = StorageValueRef::persistent(&LOCAL_RANDOM);
		let random_ret = local_random.get::<u64>().map_err(|_|"GetLocalErr")?;
		let random_number = random_ret.unwrap();

		let results = signer.send_signed_transaction(|_account| {
			Call::proposal_reveal(block_number, random_number)
		});

		for (acc, res) in &results {
			match res {
				Ok(()) => log::info!("Submitted reveal [{:?}]: {}", acc.id, random_number),
				Err(e) => log::error!("[{:?}] Failed to submit transaction: {:?}", acc.id, e),
			}
		}

		Ok(())
	}

	fn verify_vote_result(block_number: T::BlockNumber) -> Result<(), &'static str>{
		let mut random_vec: vec::Vec<u64> = vec![];
		for (account_id, commit_hash) in <CommitResult<T>>::iter(){
			// ensure!(
			// 	<RevealResult<T>>::contains_key(&authority_id),
			// 	// Error::<T>::NoValueStored
			// 	Err(OffchainErr::TooEarly)
			// );

			let new_random = <RevealResult<T>>::get(&account_id);
			let verify_hash = T::Hashing::hash(&new_random.to_be_bytes()[..]);
			if verify_hash == commit_hash{
				// log::info!("accept random: {}", new_random);
				random_vec.push(new_random);
			}
			else{
				log::info!("================ Result: Failed (commit reveal failed");
				log::info!("verify failed: {:?}!={:?}({:?})", commit_hash, verify_hash, account_id);
				return Err("hash verify failed")?
			}
		}

		// generate consensus random number
		let mut random_number = 0u64;
		for i in random_vec.iter(){
			random_number ^= i;
		}

		// generate btreemap: (distance, authority_id)
		let random_number_hash = T::Hashing::hash(&random_number.to_be_bytes());
		let mut priority_map = BTreeMap::new();
		for (account_id, _) in <CommitResult<T>>::iter(){
			//TODO: public key to hash
			// let user_public_hash = T::Hashing::hash(&account_id.to_raw_vec()[..]);
			let user_public_hash = T::Hashing::hash(&account_id.encode());

			let distance = user_public_hash ^ random_number_hash;
			priority_map.insert(distance, account_id);
		}

		<NextMembers<T>>::kill();
		let mut members = <NextMembers<T>>::get();

		let mut member_count = 0;
		for (_, authority_id) in priority_map.iter(){
			let insert_index = member_count;
			members.insert(insert_index, authority_id.clone());
			member_count += 1;
			if member_count >= 3 {
				break;
			}
		}
		<NextMembers<T>>::put(members);
		Self::show_result(block_number, &random_vec, random_number);
		Ok(())
	}

	fn clean_pallet_storage(){
		log::info!("clean pallet sotrage");
		<CommitResult<T>>::remove_all(None);
		<RevealResult<T>>::remove_all(None);
		// <NextMembers<T>>::kill();
	}
	
	fn show_result(_now: T::BlockNumber, random_vec: &Vec<u64>, random_number: u64){
		log::info!("================ Result: Success");
		log::info!("Commit:");
		for i in <CommitResult<T>>::iter(){
			log::info!("{:?}", i);
		}

		log::info!("Reveal:");
		for i in <RevealResult<T>>::iter(){
			log::info!("{:?}", i);
		}

		log::info!("Accept random:");
		for i in random_vec.iter(){
			log::info!("{}", i);
		}
		log::info!("consensus random: {}", random_number);

		log::info!("Committee members:");
		let members = <NextMembers<T>>::get();
		for i in members.iter(){
			log::info!("{:?}", i);
		}
	}
}
