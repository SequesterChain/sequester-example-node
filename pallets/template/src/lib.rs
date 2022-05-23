#![cfg_attr(not(feature = "std"), no_std)]
#![feature(more_qualified_paths)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_runtime::{
		offchain::{
			storage::{StorageValueRef},
			storage_lock::{StorageLock, Time},
		},
		traits::{
			AtLeast32BitUnsigned, Saturating, Zero
		}
	};
	use frame_system::offchain::{SubmitTransaction, SendTransactionTypes};
	use codec::{Codec};
	use sp_std::{fmt::Debug};
	use scale_info::TypeInfo;

	const DB_KEY_SUM: &[u8] = b"donations/txn-fee-sum";
	const DB_LOCK: &[u8] = b"donations/txn-sum-lock";
	const SEND_INTERVAL: u32 = 10;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_balances::Config + SendTransactionTypes<Call<Self>>{
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type BalancesEvent: From<<Self as frame_system::Config>::Event> + TryInto<pallet_balances::Event<Self>>;
		type Balance: AtLeast32BitUnsigned + Saturating + Codec + TypeInfo + Default + Debug + Copy + From<<Self as pallet_balances::Config>::Balance>;
	}

	// `SignedSubmissions` items end here.

	/// The minimum score that each 'untrusted' solution must attain in order to be considered
	/// feasible.
	///
	/// Can be set via `set_minimum_untrusted_score`.
	#[pallet::type_value]
	pub(super) fn DefaultNextUnsigned<T: Config>() -> T::BlockNumber { T::BlockNumber::from(0u32) }
	#[pallet::storage]
	#[pallet::getter(fn next_unsigned_at)]
	pub(super) type NextUnsignedAt<T: Config> = StorageValue<_, T::BlockNumber, ValueQuery, DefaultNextUnsigned<T>>;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/v3/runtime/events-and-errors
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event documentation should end with an array that provides descriptive names for event
		/// parameters. [something, who]
		TxnFeeSent(<T as Config>::Balance),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Error names should be descriptive.
		NoneValue,
		/// Errors should have helpful documentation associated with them.
		InvalidOffchainStorageRead,
	}

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		// Each block, initiate an offchain worker to summarize the txn fees for that block,
		// and append that amount to a counter in local storage, which we will empty
		// when it is time to send the txn fees to Sequester.
		fn offchain_worker(block_number: T::BlockNumber){
			Self::calculate_fees_and_update_storage();

			// send fees to sequester
			if (block_number % T::BlockNumber::from(SEND_INTERVAL)).is_zero() {
				Self::send_fees_to_sequester(block_number);
			}
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;
		fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			log::info!("validating unsigned transaction");
			if let Call::submit_unsigned { amount, block_num, .. } = call {
				// Discard solution not coming from the local OCW.
				match source {
					TransactionSource::Local | TransactionSource::InBlock => { /* allowed */ },
					_ => return InvalidTransaction::Call.into(),
				}

				// Reject outdated txns
				let next_unsigned_at = Self::next_unsigned_at();
				if &next_unsigned_at > block_num {
					return InvalidTransaction::Stale.into();
				}
				// Reject txns from the future
				let current_block = <frame_system::Pallet<T>>::block_number();
				if &current_block < block_num {
					return InvalidTransaction::Future.into();
				}

				log::info!("unsigned transaction -- sending {:?} to sequester", amount);

				ValidTransaction::with_tag_prefix("Donations")
					// The higher the amount, the higher urgency to send.
					.priority(9999999u64)
					// We don't propagate this. This can never be validated at a remote node.
					.propagate(false)
					.build()
			} else {
				InvalidTransaction::Call.into()
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		// TODO: calculate weight
		#[pallet::weight(10_000)]
		pub fn submit_unsigned(
			origin: OriginFor<T>,
			amount: <T as Config>::Balance,
			block_num: T::BlockNumber,
		) -> DispatchResultWithPostInfo {
			ensure_none(origin)?;
			// TODO (once chain is live): send XCM transfer to Sequester here
			Self::deposit_event(Event::TxnFeeSent(amount));
			<NextUnsignedAt<T>>::put(block_num + T::BlockNumber::from(SEND_INTERVAL));
			Ok(None.into())
		}
	}
	
	impl<T: Config> Pallet<T> {
		fn calculate_fees_and_update_storage(){
			let block_fee_sum = Self::calculate_fees_for_block();

			log::info!("total fees for block!!: {:?}", block_fee_sum);

			Self::update_storage(block_fee_sum);
		}

		fn calculate_fees_for_block() -> <T as Config>::Balance {
			let events = <frame_system::Pallet<T>>::read_events_no_consensus();

			let mut curr_block_fee_sum = Zero::zero(); 
			let mut withdrawee: Option<T::AccountId> = None;

			let filtered_events = events.into_iter().filter_map(|event_record| {
				let balances_event = <T as Config>::BalancesEvent::from(event_record.event);
				balances_event.try_into().ok()
			});

			for event in filtered_events {
				Self::match_event(event, & mut withdrawee, & mut curr_block_fee_sum);
			}

			curr_block_fee_sum
		}

		fn match_event(event: pallet_balances::Event<T>, withdrawee: & mut Option<T::AccountId>, curr_block_fee_sum: & mut<T as Config>::Balance) {
			match event {
				<pallet_balances::Event<T>>::Withdraw{who, amount} => {
					*withdrawee = who.into();
					log::info!("withdraw event!!: {:?}", amount);
					*curr_block_fee_sum = (*curr_block_fee_sum).saturating_add(<T as Config>::Balance::from(amount));
				},
				<pallet_balances::Event<T>>::Deposit{who, amount} => {
					// If amount is deposited back into the account that paid for the transaction fees
					// during the same transaction, then deduct it from the txn fee counter as a refund
					if Some(who) == *withdrawee {
						log::info!("deposit refunded!!: {:?}", amount);
						*curr_block_fee_sum = (*curr_block_fee_sum).saturating_sub(<T as Config>::Balance::from(amount));
					}
				},
				_ => {}
			}
		}

		fn update_storage(block_fee_sum: <T as Config>::Balance){
			// Use get/set instead of mutation to guarantee that we don't
			// hit any MutateStorageError::ConcurrentModification errors
			let mut lock = StorageLock::<Time>::new(&DB_LOCK);
			{
				let _guard = lock.lock();
				let val = StorageValueRef::persistent(&DB_KEY_SUM);
				match val.get::<<T as Config>::Balance>() {
					// initialize value
					Ok(None) => {
						log::info!("initializing storage val and setting it to: {:?}", block_fee_sum);
						val.set(&block_fee_sum);
					},
					// update value
					Ok(Some(fetched_txn_fee_sum)) => {
						log::info!("retrieved storage val: {:?} and adding : {:?}", fetched_txn_fee_sum, block_fee_sum);
						val.set(&fetched_txn_fee_sum.saturating_add(block_fee_sum));
					},
					_ => {}
				};
			}
		}

		fn send_fees_to_sequester(block_num: T::BlockNumber){
			// get lock so that another ocw doesn't modify the value mid-send
			let mut lock = StorageLock::<Time>::new(&DB_LOCK);
			{
				let _guard = lock.lock();
				let val = StorageValueRef::persistent(&DB_KEY_SUM);
				let fees_to_send = val.get::<<T as Config>::Balance>();
				match fees_to_send {
					Ok(Some(fetched_fees)) => {
						let call = Call::<T>::submit_unsigned{amount: fetched_fees, block_num: block_num};
						SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
						.map_err(|_| {
							log::error!("Failed in offchain_unsigned_tx");
							return;
						});
						log::error!("resetting storage value");
						let zero_bal:<T as Config>::Balance = Zero::zero(); 
						val.set(&zero_bal);
					},
					_ => {},
				};
			}

		}

	}
}
