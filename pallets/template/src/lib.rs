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
			storage::{StorageValueRef}
		},
		traits::{
			AtLeast32BitUnsigned, Saturating, Zero
		}
	};
	use codec::{Codec};
	use sp_std::{fmt::Debug};

	const DB_KEY: &[u8] = b"donations/txn-fee-sum";

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_balances::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type BalancesEvent: From<<Self as frame_system::Config>::Event> + TryInto<pallet_balances::Event<Self>>;
		type Balance: AtLeast32BitUnsigned + Saturating + Codec + Default + Debug + Copy + From<<Self as pallet_balances::Config>::Balance>;
	}

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
		SomethingStored(u32, T::AccountId),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Error names should be descriptive.
		NoneValue,
		/// Errors should have helpful documentation associated with them.
		StorageOverflow,
	}

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		// Each block, initiate an offchain worker to summarize the txn fees for that block,
		// and append that amount to a counter in local storage, which we will empty
		// when it is time to send the txn fees to Sequester.
		fn offchain_worker(_block_number: T::BlockNumber){
			Self::calculate_fees_and_update_storage();
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {}
	
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
			let val = StorageValueRef::persistent(&DB_KEY);
			let result = val.mutate::<<T as Config>::Balance, (), _>(|fetched_txn_fee_sum|{
				match fetched_txn_fee_sum {
					// initalize value
					Ok(None) => {
						log::info!("initializing storage val and setting it to: {:?}", block_fee_sum);
						Ok(block_fee_sum)
					},
					// update value
					Ok(Some(fetched_txn_fee_sum)) => {
						log::info!("retrieved storage val: {:?} and adding : {:?}", fetched_txn_fee_sum, block_fee_sum);
						Ok(fetched_txn_fee_sum.saturating_add(block_fee_sum))
					},
					Err(e) => {
						log::info!("mutation err: {:?}", e);
						Err(())
					}
				}
			});
			log::info!("mutation result: {:?}", result);
		}
	}
}
