#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

#![allow(unused_variables)]
#![allow(dead_code)]
//! A pallet to demonstrate usage of a simple storage map
//!
//! Storage maps map a key type to a value type. The hasher used to hash the key can be customized.
//! This pallet uses the `blake2_128_concat` hasher. This is a good default hasher.
//! 

use frame_support::{
	traits::{
		Get,  ValidatorSet,
		ValidatorSetWithIdentification, 
	},
};
use frame_system::offchain::{SendTransactionTypes, SubmitTransaction};
pub use pallet::*;
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;

use sp_application_crypto::RuntimeAppPublic;
use sp_std::{convert::TryInto, prelude::*, vec, collections::btree_map::BTreeMap};
use sp_runtime::{
	offchain::storage::{StorageValueRef},
	traits::{Hash},
};

// pub(crate) const CURRENT_STAGE: &[u8] = b"parity/commit-reveal-stage";
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

pub(crate) const MEMBER_COUNT: u64 = 5;

// use sp_std::{convert::TryInto, prelude::*, vec, collections::btree_map::BTreeMap};

/// Error which may occur while executing the off-chain code.
#[cfg_attr(test, derive(PartialEq))]
enum OffchainErr<BlockNumber> {
	// TooEarly,
	WaitingForInclusion(BlockNumber),
	// AlreadyOnline(u32),
	// FailedSigning,
	// FailedToAcquireLock,
	// NetworkState,
	SubmitTransaction,
	GetLocalStorage,
	VerifyCommitHash,
}

impl<BlockNumber: sp_std::fmt::Debug> sp_std::fmt::Debug for OffchainErr<BlockNumber> {
	fn fmt(&self, fmt: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		match *self {
			// OffchainErr::TooEarly => write!(fmt, "Too early to send heartbeat."),
			OffchainErr::WaitingForInclusion(ref block) =>
				write!(fmt, "Heartbeat already sent at {:?}. Waiting for inclusion.", block),
			// OffchainErr::AlreadyOnline(auth_idx) =>
			// 	write!(fmt, "Authority {} is already online", auth_idx),
			// OffchainErr::FailedSigning => write!(fmt, "Failed to sign heartbeat"),
			// OffchainErr::FailedToAcquireLock => write!(fmt, "Failed to acquire lock"),
			// OffchainErr::NetworkState => write!(fmt, "Failed to fetch network state"),
			OffchainErr::SubmitTransaction => write!(fmt, "Failed to submit transaction"),
			OffchainErr::GetLocalStorage => write!(fmt, "Failed to get local storage"),
			OffchainErr::VerifyCommitHash => write!(fmt, "Commit hash verify failed"),
		}
	}
}

type OffchainResult<T, A> = Result<A, OffchainErr<<T as frame_system::Config>::BlockNumber>>;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

	#[pallet::config]
	pub trait Config: SendTransactionTypes<Call<Self>>+frame_system::Config {
		/// The identifier type for an authority.
		type AuthorityId: Member
			+ Parameter
			+ RuntimeAppPublic
			+ Default
			+ Ord
			+ MaybeSerializeDeserialize;

		type ValidatorSet: ValidatorSetWithIdentification<Self::AccountId>;

		#[pallet::constant]
		type UnsignedPriority: Get<TransactionPriority>;
	}


	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(PhantomData<T>);

    #[pallet::storage]
    pub(crate) type CommitResult<T: Config> =
        StorageMap<_, Twox64Concat, T::AuthorityId, T::Hash, ValueQuery>;

    #[pallet::storage]
    pub(crate) type RevealResult<T: Config> =
        StorageMap<_, Twox64Concat, T::AuthorityId, u64, ValueQuery>;
	
    #[pallet::storage]
    pub(crate) type TestMap<T: Config> =
        StorageMap<_, Twox64Concat, Vec<u8>, u64, ValueQuery>;

    #[pallet::storage]
    pub(crate) type NextMembers<T: Config> =
        StorageValue<_, Vec<T::AuthorityId>, ValueQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Increase the value associated with a particular key
		#[pallet::weight(10_000)]
		pub fn say_hello(origin: OriginFor<T>, now: T::BlockNumber) -> DispatchResult{
			ensure_none(origin)?;
			log::info!("commit-reveal: say hello: {:?}", now);

			Ok(().into())
		}

        #[pallet::weight(0)]
        pub fn proposal_commit(
			origin: OriginFor<T>, 
			_now: T::BlockNumber,
			authority_id: T::AuthorityId, 
			hash: T::Hash
		) -> DispatchResult{
			ensure_none(origin)?;
			log::info!("[before]:");
			for (a, h) in <CommitResult<T>>::iter(){
				let raw_vec = a.to_raw_vec();
				log::info!("> [{:x}..{:x}]: {:?}", raw_vec[0], raw_vec.last().unwrap(), h);
			}

			let raw_vec = authority_id.to_raw_vec();
			log::info!("[after >>>([{:x}..{:x}]: {:?})]:", raw_vec[0], raw_vec.last().unwrap(), hash);
			<CommitResult<T>>::insert(&authority_id, hash);

			for (a, h) in <CommitResult<T>>::iter(){
				let raw_vec = a.to_raw_vec();
				log::info!("[{:x}..{:x}]: {:?}", raw_vec[0], raw_vec.last().unwrap(), h);
			}
			log::info!("=================================================");
			log::info!("");
			Ok(())
        }

        #[pallet::weight(0)]
        pub fn proposal_reveal(
			origin: OriginFor<T>, 
			_now: T::BlockNumber,
			authority_id: T::AuthorityId, 
			reveal_num: u64
		) -> DispatchResult{
			ensure_none(origin)?;
			log::info!("[before]:");
			for (a, n) in <RevealResult<T>>::iter(){
				let raw_vec = a.to_raw_vec();
				log::info!(">[{:x}..{:x}]: {}", raw_vec[0], raw_vec.last().unwrap(), n);
			}

			let raw_vec = authority_id.to_raw_vec();
			log::info!("[after >>>([{:x}..{:x}]: {})]:", raw_vec[0], raw_vec.last().unwrap(), reveal_num);
			<RevealResult<T>>::insert(&authority_id, reveal_num);

			for (a, n) in <RevealResult<T>>::iter(){
				let raw_vec = a.to_raw_vec();
				log::info!("[{:x}..{:x}]: {}", raw_vec[0], raw_vec.last().unwrap(), n);
			}
			log::info!("=================================================");
			log::info!("");
			Ok(())
        }

		#[pallet::weight(0)]
		pub fn insert_testmap(origin: OriginFor<T>, authority_vec: Vec<u8>, n: u64)->DispatchResult{
			ensure_none(origin)?;
			<TestMap<T>>::insert(&authority_vec, n);
			log::info!("testmap insert: {:?}, {:?}",authority_vec,  n);
			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(now: T::BlockNumber)->Weight{
			log::info!("initialize: {:?}", now);
			// log::info!("block number: {:?}", frame_system::Pallet::<T>::block_number());
			// log::info!("block hash: {:?}", frame_system::Pallet::<T>::block_hash());
			// log::info!("parent hash: {:?}", frame_system::Pallet::<T>::parent_hash());

			// if let Some(a) = T::AuthorityId::all().iter().next(){
			// 	log::info!("{:?}", a);
			// }
			
			// let r = sp_io::offchain::randomness();
			// log::info!("{:?}", r);

			0
		}

		// fn on_finalize(now: T::BlockNumber){
		// 	let count = <RevealResult<T>>::iter().count();
		// 	log::info!("finalize: {:?}:{:?}", now, count);
		// }

        fn offchain_worker(now: T::BlockNumber){
			// if let Err(e) = Self::select_members_3(now){
			// 	log::info!("select member error: {:?}", e);
			// }
			// if let Err(e) = Self::_map_test(now){
			// 	log::info!("map test error: {:?}", e);
			// }
        }
    }

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			match call{
				Call::say_hello(_)|
				Call::proposal_commit(_,_,_)|
				Call::proposal_reveal(_,_,_)|
				Call::insert_testmap(_,_)
				=> {
				// Call::proposal_commit(_,_,_)|Call::proposal_reveal(_,_,_) => {
					let current_session = T::ValidatorSet::session_index();

					ValidTransaction::with_tag_prefix("CommitReveal")
						.priority(T::UnsignedPriority::get())
						.and_provides((current_session, 0))
						.longevity(2)
						.propagate(false)
						.build()
				},
				_ =>{
					InvalidTransaction::Call.into()
				}
			}
		}
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub authorities: Vec<T::AuthorityId>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			log::info!("default");
			Self { authorities: Vec::new() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			log::info!("build");
			Pallet::<T>::initialize_authorities(&self.authorities);
		}
	}
}

// pub use sp_consensus_babe::{AuthorityId, PUBLIC_KEY_LENGTH, RANDOMNESS_LENGTH, VRF_OUTPUT_LENGTH};

// impl<T: Config> OneSessionHandler<T::AccountId> for Pallet<T> {
// 	type Key = AuthorityId;

// 	fn on_genesis_session<'a, I: 'a>(validators: I)
// 	where
// 		I: Iterator<Item = (&'a T::AccountId, AuthorityId)>,
// 	{
// 		let authorities = validators.map(|(_, k)| (k, 1)).collect::<Vec<_>>();
// 		for i in authorities.iter(){
// 			log::info!("genesis: {:?}", i);
// 		}
// 		Self::initialize_authorities(&authorities);
// 	}

// 	fn on_new_session<'a, I: 'a>(_changed: bool, validators: I, queued_validators: I)
// 	where
// 		I: Iterator<Item = (&'a T::AccountId, AuthorityId)>,
// 	{
// 		return
// 	}

// 	fn on_disabled(i: usize) {
// 	}
// }

impl<T: Config> Pallet<T>{
	// pub(crate) fn select_members_3(block_number: T::BlockNumber) -> OffchainResult<T, ()>{
	// 	let block_number_u64 = TryInto::<u64>::try_into(block_number).unwrap_or(0u64);


	// 	if let Some(signer) = T::AuthorityId::all().iter().next(){
	// 		if block_number_u64 % 20 == 3{
	// 			if signer.can_sign(){
	// 				log::info!("ok");
	// 			}
	// 			else{
	// 				log::info!("can't sign");
	// 			}
	// 		}
	// 	}

	// 	Ok(())
	// }

	pub(crate) fn select_members_2(block_number: T::BlockNumber) -> OffchainResult<T, ()>{
		let block_number_u64 = TryInto::<u64>::try_into(block_number).unwrap_or(0u64);

		if block_number_u64 < 5 {
			if block_number_u64 == 1{
				let state_memory = StorageValueRef::persistent(&CURRENT_STATE);
				state_memory.set(&STATE_INIT);
				let index_memory = StorageValueRef::persistent(&STATE_SET_INDEX);
				index_memory.set(&block_number_u64);
			}
			return Ok(());
		}

		if let Some(authority_id) = T::AuthorityId::all().iter().next(){

			let state_memory = StorageValueRef::persistent(&CURRENT_STATE);
			let state_ret = state_memory.get::<u64>().map_err(|_|OffchainErr::GetLocalStorage)?;
			let state = state_ret.unwrap();

			let index_memory = StorageValueRef::persistent(&STATE_SET_INDEX);
			let index_ret = index_memory.get::<u64>().map_err(|_|OffchainErr::GetLocalStorage)?;
			let last_index = index_ret.unwrap();

			match state{
				STATE_INIT =>{
					log::info!("## state init");
					Self::clean_pallet_storage();
					state_memory.set(&STATE_COMMIT);
					index_memory.set(&block_number_u64);
				},
				STATE_COMMIT => {
					if block_number_u64 == last_index + 5{
						log::info!("## state commit");
						let ret = Self::send_commit_hash(block_number, authority_id.clone());
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
					if (<CommitResult<T>>::iter().count() as u64) == MEMBER_COUNT {
						if block_number_u64 > last_index + 5u64{
							state_memory.set(&STATE_REVEAL);
							index_memory.set(&block_number_u64);
						}
					}
				},
				STATE_REVEAL => {
					if block_number_u64 == last_index + 5u64{
						log::info!("## state reveal");
						let ret = Self::send_reveal_number(block_number, authority_id.clone());
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
					if (<RevealResult<T>>::iter().count() as u64) == MEMBER_COUNT {
						if block_number_u64 == last_index + 5{
							state_memory.set(&STATE_REVEAL_DONE);
							index_memory.set(&block_number_u64);
						}
					}
				},
				STATE_REVEAL_DONE => {
					log::info!("## state reveal done");
					let ret = Self::verify_vote_result(block_number);
					state_memory.set(&STATE_FINISH);
					index_memory.set(&block_number_u64);
					return ret;
				},
				STATE_FINISH => {
					log::info!("## state finish");
				}
				_ =>{},
			}
		}
		Ok(())
	}

	fn clean_pallet_storage(){
		log::info!("clean pallet sotrage");
		<CommitResult<T>>::remove_all(None);
		<RevealResult<T>>::remove_all(None);
		<NextMembers<T>>::kill();
	}

	fn send_commit_hash(block_number: T::BlockNumber, authority_id: T::AuthorityId)->OffchainResult<T, ()>{
		// log::info!("Commit Stage");
		let block_number_u64 = TryInto::<u64>::try_into(block_number).unwrap_or(0u64);

		let public_u64 = {
			let authority_vec = authority_id.to_raw_vec();
			let mut raw = [0u8;8];
			raw[0..8].copy_from_slice(&authority_vec[0..8]);
			u64::from_be_bytes(raw)
		};

		let block_u64 = block_number_u64;
		let random_number = public_u64 ^ block_u64;

		let local_mem = StorageValueRef::persistent(&LOCAL_RANDOM);
		local_mem.set(&random_number);

		let proposal_hash = T::Hashing::hash(&random_number.to_be_bytes()[..]);
		// log::info!("k: {:?}: hash: {:?}, number: {}", authority_id, proposal_hash, random_number);

		let raw_vec = authority_id.to_raw_vec();
		log::info!(">>>>>>commit: ([{:x}..{:x}]: {:?}, {})", raw_vec[0], raw_vec.last().unwrap(), proposal_hash, random_number);

		let prepare_call = || -> OffchainResult<T, Call<T>> {
			Ok(Call::proposal_commit(block_number, authority_id.clone(), proposal_hash))
		};

		let call = prepare_call()?;
		SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
			.map_err(|_| OffchainErr::SubmitTransaction)?;
		Ok(())
	}

	fn send_reveal_number(block_number: T::BlockNumber, authority_id: T::AuthorityId)->OffchainResult<T, ()>{
		// log::info!("Reveal Stage");

		let local_mem = StorageValueRef::persistent(&LOCAL_RANDOM);
		let random_ret = local_mem.get::<u64>().map_err(|_|OffchainErr::GetLocalStorage)?;

		if let Some(random_number) = random_ret{
			let proposal_number = random_number;

			let raw_vec = authority_id.to_raw_vec();
			log::info!(">>>>>>reveal: ([{:x}..{:x}]: {:?})", raw_vec[0], raw_vec.last().unwrap(), proposal_number);

			let prepare_call = || -> OffchainResult<T, Call<T>> {
				Ok(Call::proposal_reveal(block_number, authority_id.clone(), proposal_number))
			};
			let call = prepare_call()?;

			SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
				.map_err(|_| OffchainErr::SubmitTransaction)?;
		}
		Ok(())
	}

	fn verify_vote_result(block_number: T::BlockNumber)->OffchainResult<T, ()>{
		let mut random_vec: vec::Vec<u64> = vec![];
		for (authority_id, commit_hash) in <CommitResult<T>>::iter(){
			// ensure!(
			// 	<RevealResult<T>>::contains_key(&authority_id),
			// 	// Error::<T>::NoValueStored
			// 	Err(OffchainErr::TooEarly)
			// );

			let new_random = <RevealResult<T>>::get(authority_id);
			let verify_hash = T::Hashing::hash(&new_random.to_be_bytes()[..]);
			if verify_hash == commit_hash{
				// log::info!("accept random: {}", new_random);
				random_vec.push(new_random);
			}
			else{
				log::info!("================ Result: Failed (commit reveal failed");
				log::info!("verify failed: {:?}!={:?}", commit_hash, verify_hash);
				return Err(OffchainErr::VerifyCommitHash);
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
		for (authority_id, _) in <CommitResult<T>>::iter(){
			//TODO: public key to hash
			let user_public_hash = T::Hashing::hash(&authority_id.to_raw_vec()[..]);
			let distance = user_public_hash ^ random_number_hash;
			priority_map.insert(distance, authority_id);
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


	pub(crate) fn select_members(block_number: T::BlockNumber) -> OffchainResult<T, ()>{
		let block_number_u64 = TryInto::<u64>::try_into(block_number).unwrap_or(0u64);
		let base_number = 43;
		// if block_number_u64 < base_number {
		// 	return Ok(())
		// }

		let n = block_number_u64 % base_number;

		match n {
			3 => {
				log::info!("Clear CommitMap, RevealMap, NextMembers");
				<CommitResult<T>>::remove_all(None);
				<RevealResult<T>>::remove_all(None);
				<NextMembers<T>>::kill();

				// Self::show_commit();
				// Self::show_reveal();

				if let Some(authority_id) = T::AuthorityId::all().iter().next(){
					log::info!("Account: {:?}", authority_id);
				}
			},
			5 => {
				if let Some(authority_id) = T::AuthorityId::all().iter().next(){
					log::info!("Commit Stage");
	
					let public_u64 = {
						let authority_vec = authority_id.to_raw_vec();
						let mut raw = [0u8;8];
						raw[0..8].copy_from_slice(&authority_vec[0..8]);
						u64::from_be_bytes(raw)
					};
	
					let block_u64 = block_number_u64;
					let random_number = public_u64 ^ block_u64;
	
					let local_mem = StorageValueRef::persistent(&LOCAL_RANDOM);
					local_mem.set(&random_number);
	
					let proposal_hash = T::Hashing::hash(&random_number.to_be_bytes()[..]);
					// log::info!("k: {:?}: hash: {:?}, number: {}", authority_id, proposal_hash, random_number);

					let raw_vec = authority_id.to_raw_vec();
					log::info!("commit reqeust: ([{:x}..{:x}]: {:?})", raw_vec[0], raw_vec.last().unwrap(), proposal_hash);
	
					let prepare_call = || -> OffchainResult<T, Call<T>> {
						Ok(Call::proposal_commit(block_number, authority_id.clone(), proposal_hash))
					};
	
					let call = prepare_call()?;
					SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
						.map_err(|_| OffchainErr::SubmitTransaction)?;
				}
			},
			// 19 => {
			// 	Self::show_commit();
			// },
			20 => {
				if let Some(authority_id) = T::AuthorityId::all().iter().next(){
					log::info!("Reveal Stage");

					let local_mem = StorageValueRef::persistent(&LOCAL_RANDOM);
					let random_ret = local_mem.get::<u64>().map_err(|_|OffchainErr::GetLocalStorage)?;

					if let Some(random_number) = random_ret{
						let proposal_number = random_number;

						// log::info!("k: {:?}, number: {}", authority_id, random_number);
						let prepare_call = || -> OffchainResult<T, Call<T>> {
							Ok(Call::proposal_reveal(block_number, authority_id.clone(), proposal_number))
						};
						let call = prepare_call()?;
	
						SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
							.map_err(|_| OffchainErr::SubmitTransaction)?;
					}
					else{
						log::info!("Load random number error");
					}
				}
			},
			// 36 => {
			// 	Self::show_reveal();
			// },
			37 => {
				log::info!("Final stage");

				// let mut commit_count = 0;
				// // log::info!("show commit reuslt");
				// for i in <CommitResult<T>>::iter(){
				// 	commit_count += 1;
				// 	// log::info!("{:?}", i);
				// }


				// if commit_count != reveal_count{
				// 	log::info!("================ Result: Failed (commit reveal failed");
				// 	return Err(OffchainErr::VerifyCommitHash);
				// }

				let mut random_vec: vec::Vec<u64> = vec![];
				for (authority_id, commit_hash) in <CommitResult<T>>::iter(){
					// ensure!(
					// 	<RevealResult<T>>::contains_key(&authority_id),
					// 	// Error::<T>::NoValueStored
					// 	Err(OffchainErr::TooEarly)
					// );
	
					let new_random = <RevealResult<T>>::get(authority_id);
					let verify_hash = T::Hashing::hash(&new_random.to_be_bytes()[..]);
					if verify_hash == commit_hash{
						// log::info!("accept random: {}", new_random);
						random_vec.push(new_random);
					}
					else{
						log::info!("================ Result: Failed (commit reveal failed");
						log::info!("verify failed: {:?}!={:?}", commit_hash, verify_hash);
						return Err(OffchainErr::VerifyCommitHash);
					}
				}
	
				// determinate consensus random number
				let mut random_number = 0u64;
				for i in random_vec.iter(){
					random_number ^= i;
				}
	
				// generate btreemap: (distance, authority_id)
				let random_number_hash = T::Hashing::hash(&random_number.to_be_bytes());
				let mut priority_map = BTreeMap::new();
				for (authority_id, _) in <CommitResult<T>>::iter(){
					//TODO: public key to hash
					let user_public_hash = T::Hashing::hash(&authority_id.to_raw_vec()[..]);
					let distance = user_public_hash ^ random_number_hash;
					priority_map.insert(distance, authority_id);
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
			},
			_=>{},
		}
		Ok(())
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

	pub(crate) fn _map_test(block_number: T::BlockNumber) -> OffchainResult<T, ()>{
		let block_number_u64 = TryInto::<u64>::try_into(block_number).unwrap_or(0u64);
		let base_number = 37;
		let step = 10;
		// if block_number_u64 < base_number {
		// 	return Ok(())
		// }

		let n = block_number_u64 % base_number;
		if n == 1{
			<RevealResult<T>>::remove_all(None);
			log::info!("after clear reveal");
			for i in <RevealResult<T>>::iter(){
				log::info!("{:?}", i);
			}
		}
		// if n == 3{
		// 	if let Some(authority_id) = T::AuthorityId::all().iter().next(){
		// 		let random_number = n + 20;
		// 		let prepare_call = || -> OffchainResult<T, Call<T>> {
		// 			Ok(Call::proposal_reveal(block_number, authority_id.clone(), random_number))
		// 		};
		// 		let call = prepare_call()?;
		// 		SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
		// 			.map_err(|_| OffchainErr::SubmitTransaction)?;
		// 	}
		// }
		else if n == 5{
			if let Some(authority_id) = T::AuthorityId::all().iter().next(){
				// log::info!("Commit Stage");

				let public_u64 = {
					let authority_vec = authority_id.to_raw_vec();
					let mut raw = [0u8;8];
					raw[0..8].copy_from_slice(&authority_vec[0..8]);
					u64::from_be_bytes(raw)
				};

				let block_u64 = block_number_u64;
				let random_number = public_u64 ^ block_u64;

				// let local_mem = StorageValueRef::persistent(&LOCAL_RANDOM);
				// local_mem.set(&random_number);
				// let proposal_hash = T::Hashing::hash(&random_number.to_be_bytes()[..]);

				let raw_vec = authority_id.to_raw_vec();
				log::info!("reveal insert: ([{:x}..{:x}]: {})", raw_vec[0], raw_vec.last().unwrap(), random_number);
				// log::info!("k: {:?}: hash: {:?}, number: {}", authority_id, proposal_hash, random_number);

				let prepare_call = || -> OffchainResult<T, Call<T>> {
					Ok(Call::proposal_reveal(block_number, authority_id.clone(), random_number))
				};

				let call = prepare_call()?;
				SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
					.map_err(|_| OffchainErr::SubmitTransaction)?;
			}
		}
		else if n == 36 {
			log::info!("show map");
			for i in <CommitResult<T>>::iter(){
				log::info!("{:?}", i);
			}
		}
		// else if n == 3{
		// 	if let Some(authority_id) = T::AuthorityId::all().iter().next(){
		// 		log::info!("insert testmap");

		// 		let authority_vec = authority_id.to_raw_vec();
		// 		let prepare_call = || -> OffchainResult<T, Call<T>> {
		// 			// Ok(Call::insert_testmap(authority_id, block_number))
		// 			Ok(Call::insert_testmap(authority_vec, n+8))
		// 		};

		// 		let call = prepare_call()?;
		// 		SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
		// 			.map_err(|_| OffchainErr::SubmitTransaction)?;
		// 	}
		// }
		Ok(())
	}


	pub(crate) fn _offchain_test_1(now: T::BlockNumber) -> OffchainResult<T, ()>{
		let n = TryInto::<u64>::try_into(now).unwrap_or(0u64);
		if n <10 {
			return Ok(());
		}

		if n%10 == 1{
			let all_ids = T::AuthorityId::all();
			for i in all_ids.iter(){
				log::info!("{:?}", i);
			}
		}

		Ok(())
	}

	pub(crate) fn _offchain_test_2(now: T::BlockNumber) -> OffchainResult<T, ()>{
		let prepare_call = || -> OffchainResult<T, Call<T>> {
			Ok(Call::say_hello(now))
		};

		let call = prepare_call()?;
		SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
			.map_err(|_| OffchainErr::SubmitTransaction)?;

		// let prepare_call = Call::say_hello();
		Ok(())
	}

}