use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, ext_contract, near_bindgen, AccountId, BorshStorageKey, PanicOnDefault, Promise, PromiseResult};
use std::collections::HashMap;
//use rust_elgamal::CipherText;

#[derive(BorshStorageKey, BorshSerialize)]
pub enum StorageKeys {
	Policies,
	RegulationAdmins,
	PolicyAdmins,
	ValidQuoteIssuers,
	LossConfirmationRequests,
	ObligationsAwaitingPayment,
	Clients,
}



///# description
///
/// a smart contract for the NEAR protocol that maintains parametric insurance policy state
/// receives `Quotes` from a `QuoteManager` Contract
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Quote {
	///unique identifier
	id: String,
	///the party issuing the `Quote`
	issuer: User,
	///an arbitrary unique identifier
	client: User,
	///the oracle that can provide data to validate a policy's triggers
	claims_manager: AccountId,
	///the protection option that this quote represents
	policy_type: u8,
	///maximum value total value of the policy
	max_payout: f64,
	///the period that a policy will be valid.
	coverage_period: [u64; 2],
	/// the policy manager that activated this quote
	policy_manager: AccountId,
	///location under policy protection
	location: Location,
}

/// a location being protected under a **Policy**
///
/// # Fields
///
/// * `latitude` [u32]
/// * `longitude` [u32]
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct ResolveObligation{
	identity:LossIdentity,
	payment_proof:String,
}
///a `PayoutContext` is what the `ClaimsManager` needs to determine if a `PayoutObligation` is to be due
/// a `PayoutContextRequest` is a construct sent to the `PolicyManager` so it can construct a `PayoutContext` from
/// its internal data to subsequently send to the `ClaimsManager`
///
/// # fields
///
/// * `id` a unique id. As of current implementation is created from hash from `Oracle` data
/// * `oracle_data` the triggering values from the `Oracle` that will be used to create a `PayoutObligation` if necessary
/// * `policy_id` policy id
/// * `event_id` event id
/// * `policy_manager` where to send this `PayoutContextRequest` for processing
/// * `oracle` the source of the off chain data.
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct User {
	user_type: UserType,
	id: String,
	authorized_administrator: AccountId,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum UserType {
	PayoutAuthority,
	PaymentProcessor,
	Client,
	Issuer,
}


///#Description
///
/// a confirmation object that gives the caller relevant information about the
/// the transaction
///
/// #Fields
/// `client_id` the unique client id
#[derive(BorshDeserialize, BorshSerialize, Serialize,Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct EventContext {
	percent_due: u8,
	balance_snapshot: f64,
	payments: Vec<Payment>,
	obligations: Vec<Obligation>,
	rejected_losses: Vec<ComputedLoss>,
	computed_losses: Vec<ComputedLoss>,
	claims_manager: AccountId,
}

//TODO payments decrease balance, obligations decrease pending balance and computed_loss as well
/// a construct that is sent to a `PolicyManager` so it can update its internal state relative to a particular `Event`
/// in the context of a particular `Policy`
///
/// # fields
/// * `policy_manager` which `PolicyManager` retains the `Policy` data
/// * `policy_id` policy id
/// * `event_id` event id
/// * `max_payout_percent` the current percent payout for an `Event`
/// * `max_possible_amount` the real value amount that is to be paid to the `Client` and is to be deducted from `Policy`
/// balance. Is calculated from `max_payout_percent`
/// * `payout_obligation` a `PayoutObligation` that is calculated by the `ClaimsManager` and is subsequently processed
/// by the `PolicyManager`
/// # notes
/// - payout amounts do not stack. (not accumulative). if the `max_payout_percent` was 30 and circumstance for an
/// event dictate that the event payout is to be 50 percent. then the total payout against a policy balance would be
/// 50% and not 80% cumulatively.
/// example: category 3 hits at distance x and the payout percent is 30. the same storm has grown to a category 4
/// and the policy is to payout out at 50 percent. the previous `max_payout_percent` was 30 and has become 50.
///
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct EventContextUpdate {
	policy_manager: AccountId,
	policy_id: String,
	event_id: String,
	max_payout_percent: u8,
	max_possible_payout: f64,
	computed_loss: ComputedLoss,
}

///a quote is issued to a client. Quotes come after estimates. Estimates are not represented in this contract
///
/// # Fields
///
/// * `id` unique identifier
/// * `issuer` an issuer is the party that is permitted to issue `Quotes` to `Clients`
/// * `client` represents the potential purchaser of insurance
/// * `claims_contract` The contract that maintains the logic that determines if a `PayoutObligation` is made
///by an Oracle account
/// * `policy_type` one of `N` different choices for `Policies`
///* `max_payout` the maximum payout value of the [`Policy`]
///* `coverage_period` the period of time that a [`Policy`] would be active and valid
///* `policy_manager` Which policy manager will be responsible for maintaining an issued [`Policy`]
///*  `payment_processor` is a party responsible for receiving `Client` payment off chain and subsequently
/// binding a [`Policy`] by calling `issue_policy` on a `QuoteManager` contract
/// * `location` the location of the asset that is being insured.
///* `payout_authority` the party that is responsible for fulfilling `PayoutObligations`
///* `premium` the amount that the `Client` must pay to bind a [`Policy`]
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Location {
	latitude: f64,
	longitude: f64,
}


/// # parameters
/// * accept {`bool`} client's decision related to the loss
/// * loss_ident {`LossIdentity`} relevant data  to query Loss
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct LossDecision {
	accept: bool,
	identity: LossIdentity,
}


/// a `PayoutObligation` that has been fulfilled by a `payout_authority`
///
/// # fields
///
/// * `payout_obligation` the original `PayoutObligation`
/// * `time_of_contract_notification` this represents the blocktime when the `payout_authority` notified
///the contract of a successful payment (and provided a payment proof) to the `Client`
///This does not represent the time the the `payout_authority` issued the payment to the `Client`
/// * `payment_proof` a string that represents an on-chain record of some reference to an off-chain payment to a `Client`
///for auditing purposes
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct LossIdentity {
	id: String,
	event_id: String,
	policy_id: String,
	client_id: String,
	issuer_id: String,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct OracleMetadata {
	triggering_values: HashMap<String, u32>,
	claims_manager: AccountId,
	oracle: AccountId,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct LossContext {
	identity:LossIdentity,
	oracle_data:OracleMetadata,
	policy_type:u8,
	balance_snapshot:f64,
	current_percent:u8,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct LossCalculation {
	payout_percent: f64,
	amount_due: f64,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct ComputedLoss {
	identity: LossIdentity,
	oracle_data: OracleMetadata,
	calculations: LossCalculation,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Obligation {
	computed_loss:ComputedLoss,
	contract_update_time:u64,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Payment {
	contract_update_time: u64,
	payment_proof: String,
	obligation:Obligation
}


///# description
/// an object with relevant confirmation data meant to a caller outside of the chain.
///
///# fields
/// * `client_id` {`String`}
/// * `data_id` {`String`} the id of the relevant data type that has been changed
/// * `issuer_id` {`String`} the issuer of the `Policy`
/// * `from` {`String`} `near_sdk::env::predecessor_account_id()`
/// * `confirmation_type` {`ConfirmationType`} enum for what data change is being confirmed
/// * `success` {`bool`} was the action successful


/// the `PayoutContext` construct retains all necessary information to create `PayoutObligations` and update a
/// `Policy` `EventContext` in conjunction with `OracleData`. it is one part of a two part calculation
///
/// # fields
/// * `policy_id` policy id
/// * `policy_manager` the source of the requested `PayoutContext`
/// * `event_id` `PayoutObligations` are issued on a per `Event` basis
/// * `pending_balance` the current balance of the `Policy` under the consideration of all current `PayoutObligations`
/// that have been confirmed by a `Client` and those that and pending loss confirmation from a `Client`
/// * `event_context` construct that has all relevant information for a particular `Event`
/// * `policy_type`  the policy option that the client chose. this is used to calculate `PayoutObligation`
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Policy {
	///unique identifier can be deterministic.
	policy_id: String,
	///the total balance of the account. can be at most equal to the `max_payout` in the `Policy`'s `Quote`
	balance: f64,
	///an updated value for the balance
	pending_balance: f64,
	///the `Quote`, originally issued from the QuoteManager. the `Quote` defines then `Policy`
	quote: Quote,
	///`Policy` start date
	start_date: u64,
	///`Policy` end date
	end_date: u64,
	///is `Policy` active
	active: bool,
	///the party issuing the `Quote`
	issuer: User,
	///an arbitrary unique identifier
	client: User,
	///the oracle that can provide data to validate a policy's triggers
	claims_manager: AccountId,
	///the protection option that this quote represents
	policy_type: u8,
	///maximum value total value of the policy
	max_payout: f64,
	///location under policy protection
	location: Location,
	///payments that have been made to the client of the `Policy`
	payments: Vec<Payment>,
	///insurer payment obligations to client
	obligations: Vec<Obligation>,
	///calculated losses that the client has rejected
	rejected_losses: Vec<ComputedLoss>,
	///computed losses that have yet to receive a decision from client
	computed_losses: Vec<ComputedLoss>,
}


/// a construct that defines all the relevant data related to an `Event`
///
/// # fields
/// * `max_payout_percent` the current percentage of pending_balance being paid due to an event
/// * `max_possible_payout` the maximum amount that this `Event` can payout from the `Policy` balance
/// * `paid_obligations` a collection of obligations that have already been issued and paid
/// * `confirmed_obligations` obligations that have been 'loss confirmed' by a `Client` through a `PolicyManager`
/// * `rejected_obligations` obligations that have been rejected by a `Client` through a `PolicyManager`
/// * `pending_confirmation_obligations` obligations that are pending a `Client` decision (confirm loss or reject)
/// * `claims_manager` which `ClaimsManager` retains the information to calculate a `PayoutObligation`
///
/// # notes
/// `EventContext` retains PayoutObligations in their four possible states:
/// - confirmed and paid
/// - confirmed and unpaid
/// - rejected
/// - pending client decision (confirmation or rejection)
/// this context is needed to calculate any subsequent `PayoutObligation` because neither the `ClaimManager` nor
/// the `Oracle` have a view on the state of any `PayoutObligations` or `CompletedPayouts`. That is the purview of
/// the `PolicyManager`
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct PolicyManager {
	///indexed by policy id. since this is an
	policies: UnorderedMap<String, Policy>,
	/// the master admin of this contract
	master_admin: AccountId,
	/// the new master admin of this contract
	new_master_admin: Option<AccountId>,
	///AccountIds permitted to activate `Policy`
	policy_managers: Vec<AccountId>,
	///all the `Obligations` that an issuer must make payment key is issuer_id
	obligations: UnorderedMap<String, Vec<Obligation>>,
	///client_id as key that maps client to a collection of `Policy`
	///this makes it easier to get client data per Policy when
	/// interacting with client side app
	clients: UnorderedMap<String, Vec<String>>,
	///client_id is the key in `(key, value)` to make it easier for `Client`
	/// apps to receive this  data.
	loss_identities: UnorderedMap<String, Vec<LossIdentity>>,
}

//TODO accept events that are older than 72 hours
///# description
/// `PolicyManager` is the main contract. `QuoteManager` and `ClaimsManager` work as support.
/// all parties are to interact directly with the `PolicyManager` for getting data. it is the canonical source.
///# fields
/// * policies {UnorderedMap} according to NEAR documentations at the time of writing `UnorderedMaps` are cheaper
/// with respect to gas as compared to `HashMap`.
/// a contract that maintains the state of policies
/// * master_admin {AccountId} the account that administrates this contract. restricted to only defining
/// `valid_policy_activators`
// TODO complete documentation for PolicyManager
/// # note
/// key thing to remember is that `UnorderedMap` return a value and `HashMap` returns a reference to a value
/// that means any changes  to the value in a (key, value) pair must be saved by explicitly set.
#[near_bindgen]
impl PolicyManager {
	#[init]
	pub fn new() -> Self {
		assert!(!near_sdk::env::state_exists(), "Already initialized");
		Self {
			policies: near_sdk::collections::UnorderedMap::new(StorageKeys::Policies),
			master_admin: near_sdk::env::predecessor_account_id(),
			new_master_admin: None,
			policy_managers: Vec::new(),
			obligations: UnorderedMap::new(
				StorageKeys::ObligationsAwaitingPayment,
			),
			clients: UnorderedMap::new(StorageKeys::Clients),
			loss_identities: UnorderedMap::new(StorageKeys::LossConfirmationRequests),
		}
	}

	///#Description
	///
	/// activate a policy through this method
	///
	/// #Parameters
	///
	///`Quote`
	///
	/// #Returns
	///
	/// `Confirmation` function returns the new policy id
	pub fn save_policy(&mut self, policy: Policy) -> Policy {
		// assert!(self
		// 		.policy_managers
		// 		.to_vec()
		// 		.contains(&env::predecessor_account_id()));
		let response = policy.clone();
		self.policies.insert(&policy.policy_id, &policy);
		response
	}

	/// get a `Policy`'s information
	///
	/// # parameters
	///
	/// * `policy_id` the unique identifier for a policy
	///
	/// # Returns
	///
	/// `Option(Policy)` a policy if found
	pub fn get_policy(&self, policy_id: String) -> Option<Policy> {
		self.policies.get(&policy_id)
	}



	/// update contract when an off-chain payment is made to a client
	/// # parameters
	/// resolve_obligation {`ResolveObligation`}
	/// # returns
	/// payout id or an error if no policy was found
	pub fn post_payment_made(&mut self, resolve_obligation:ResolveObligation) ->Payment{
		assert!(
			self.policy_managers.to_vec().contains(&env::predecessor_account_id()),
			"POLICY_MANAGER_RESTRICTED"
		);
		let mut payment_response:Option<Payment> = None;
		if let Some(mut policy) = self.policies.get(&resolve_obligation.identity.policy_id) {
				let obligation_vec_index_option = policy
						.obligations
						.iter()
						.position(|obligation| *obligation.computed_loss.identity.id == resolve_obligation.identity.id);
				assert!(obligation_vec_index_option.is_some(),"OBLIGATION_NOT_FOUND");
				let  obligation: Obligation = policy.obligations.get(obligation_vec_index_option.unwrap()).unwrap().clone();
				let completed_payment:Payment = Payment {
					contract_update_time:env::block_timestamp(),
					obligation:obligation.clone(),
					payment_proof:resolve_obligation.payment_proof.clone()
				};
				payment_response = Option::from(completed_payment.clone());
				policy.payments.push(completed_payment.clone());
				policy
						.obligations
						.remove(obligation_vec_index_option.unwrap());
				policy.balance = policy.balance - obligation.computed_loss.calculations.amount_due;
				self.policies.insert(&resolve_obligation.identity.policy_id, &policy);
				let obligation_vec_option: Option<Vec<Obligation>> = self.obligations.get(&resolve_obligation.identity.issuer_id);
				assert!(obligation_vec_option.is_some(),"OBLIGATION_NOT_FOUND_IN_MANAGER");
				let mut obligation_vec = obligation_vec_option.unwrap();
				let obligation_index_option = obligation_vec
						.iter()
						.position(|obligation| *obligation.computed_loss.identity.id == resolve_obligation.identity.id);
				assert!(obligation_index_option.is_some(),"OBLIGATION_NOT_FOUND_IN_VEC");
				obligation_vec.remove(obligation_index_option.unwrap());
				self.obligations.insert(&resolve_obligation.identity.issuer_id, &obligation_vec);
				assert!(payment_response.is_some(),"ERROR_RESOLVING_PAYMENT_DATA");

		};
		payment_response.unwrap()
	}

	pub fn compute_loss(&self, loss_contexts:Vec<LossContext>)->Promise{
		let loss_context:LossContext = loss_contexts.last().unwrap().clone();
		let claims_manager = loss_context.oracle_data.claims_manager.clone();
		claims_contract::ext(claims_manager)
				.compute_loss(loss_contexts)
				.then(
					Self::ext(env::current_account_id())
							.compute_loss_callback()
				)
	}

	#[private]
	pub fn compute_loss_callback(&mut self) ->Vec<ComputedLoss>{
		assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
		match env::promise_result(0){
			PromiseResult::NotReady => unreachable!(),
			PromiseResult::Successful(returned_value) => {
				if let Ok(computed_losses) = near_sdk::serde_json::from_slice::<Vec<ComputedLoss>>(&returned_value) {
					for computed_loss in computed_losses.clone().into_iter() {
						if let Some(mut policy) = self.policies.get(&computed_loss.identity.policy_id){
							policy.computed_losses.push(computed_loss.clone());
							policy.pending_balance = policy.pending_balance - computed_loss.calculations.amount_due.clone();
							self.policies.insert(&policy.policy_id,&policy.clone());
						}
					}
					computed_losses
				} else {
					env::panic_str("ERR_WRONG_VAL_RECEIVED")
				}
			},
			PromiseResult::Failed => env::panic_str("ERR_CALL_FAILED")
		}
	}
	/// retrieve computed_loss data
	/// # parameters
	/// * loss_identity {`LossIdentity`} data to retrieve computed_loss
	/// # returns
	/// `ComputedLoss`
	pub fn get_computed_loss(&self, loss_identity: &LossIdentity) -> ComputedLoss {
		let policy_option:Option<Policy> = self.policies.get(&loss_identity.policy_id);
		assert!(policy_option.is_some(),"POLICY_NOT_FOUND");
		let policy:Policy = policy_option.unwrap();
		let computed_loss_vec_index_option:Option<usize> = policy.computed_losses.iter().position
		(|computed_loss:&ComputedLoss|*computed_loss.identity.id	== loss_identity.id);
		assert!(computed_loss_vec_index_option.is_some(),"COMPUTED_LOSS_NOT_FOUND");
		policy.computed_losses.get(computed_loss_vec_index_option.unwrap()).unwrap().clone()
	}
	/// # definition
	/// can only be called by the respective `ClaimsManager` contract of a `Policy` this function
	/// updates the `EventContext` of a particular `Event` with respect to a `Policy`
	/// it adds a `PayoutObligation`, creates a `ConfirmLossRequest` and updates the `Policy` balance
	///
	/// # parameters
	/// * event_context_update {EventContextUpdate} the object that defines the changes
	// pub fn post_event_update(&mut self, event_context_update: EventContextUpdate) {
	// 	if let Some(mut policy) = self.policies.get(&event_context_update.policy_id) {
	// 		assert_eq!(policy.claims_manager, env::predecessor_account_id(), "NOT_AUTHORIZED_TO_UPDATE_EVENT");
	// 		assert!(env::block_timestamp() <= policy.end_date,"POLICY_EXPIRED");
	// 		assert_eq!(policy.active, true, "POLICY_INACTIVE");
	// 		if let Some(event_context) = policy.events.get(&event_context_update.event_id) {
	// 			let mut computed_losses: Vec<ComputedLoss> = event_context.computed_losses.clone();
	// 			computed_losses.push(event_context_update.computed_loss.clone());
	// 			let new_event_context = EventContext{
	// 				percent_due: event_context_update.max_payout_percent,
	// 				balance_snapshot: event_context_update.max_possible_payout,
	// 				payments: event_context.payments.clone(),
	// 				obligations: event_context.obligations.clone(),
	// 				rejected_losses: event_context.rejected_losses.clone(),
	// 				computed_losses,
	// 				claims_manager: event_context.claims_manager.clone()
	// 			};
	// 			let new_balance = policy.pending_balance - event_context_update.computed_loss.calculations.amount_due.clone();
	// 			if new_balance < 0.0 {
	// 				policy.pending_balance = 0.0;
	// 			} else {
	// 				policy.pending_balance = new_balance;
	// 			}
	// 			let mut loss_identities_vec: Vec<LossIdentity> = self.loss_identities.get(&policy.quote.client.id).unwrap();
	// 			loss_identities_vec.push(event_context_update.computed_loss.identity);
	// 			self.loss_identities.insert(&policy.quote.client.id, &loss_identities_vec);
	// 			policy
	// 					.events
	// 					.insert(event_context_update.event_id, new_event_context);
	// 			self.policies.insert(&policy.policy_id, &policy);
	// 		} else {
	// 			panic!("Event not found in Policy data.");
	// 		}
	// 	} else {
	// 		panic!("No Policy found in PolicyManager");
	// 	}
	// }

	///# definition
	/// method called by `Client` to update to confirm/reject loss based on `LossConfirmationRequest`. will subsequently
	/// move a `PayoutObligation` from `pending_confirmation_obligations` to `loss_confirmed_obligations` in the `EventContext` of a
	/// particular event within a `Policy`
	///
	/// # parameters
	/// * `client_id` {`String`} unique identifier of client to find their loss_confirmation_requests
	/// *` payout_id` {`String`} every `PayoutObligation` has a one to one relationship with a
	/// * `LossConfirmationRequest` therefore a payout_id is a suitable unique identifier.
	/// * `accept` {`bool`} the `Client` decision to accept/reject the `PayoutObligation`
	///
	/// # returns
	/// ContractResponse -tentative- liable to change.

	pub fn post_loss_decision(&mut self, loss_decision: LossDecision) -> LossDecision {
		if let Some(mut policy) = self.policies.get(&loss_decision.identity.policy_id) {
			assert_eq!(
				policy.quote.client.authorized_administrator,
				env::predecessor_account_id(),
				"Not Authorized to Confirm loss for this client."
			);
				if loss_decision.accept {
					let computed_loss_vec_index_option:Option<usize> = policy.computed_losses
							.iter()
							.position(|computed_loss| *computed_loss.identity.id == *loss_decision.identity.id);
					assert!(computed_loss_vec_index_option.is_some(), "COMPUTED_LOSS_NOT_FOUND_IN_POLICY");
					let computed_loss_vec_index: usize = computed_loss_vec_index_option.unwrap();
					let computed_loss: ComputedLoss = policy.computed_losses.get(computed_loss_vec_index).unwrap().clone();
					policy.computed_losses.remove(computed_loss_vec_index);
					let new_obligation: Obligation = Obligation {
						computed_loss,
						contract_update_time: env::block_timestamp()
					};
					policy.obligations.push(new_obligation.clone());
					let issuer_obligations_option: Option<Vec<Obligation>> = self.obligations
					                                                             .get(&loss_decision.identity.issuer_id);
					assert!(issuer_obligations_option.is_some(), "ISSUER_OBLIGATIONS_NOT_FOUND");
					let mut issuer_obligations = issuer_obligations_option.unwrap();
					issuer_obligations.push(new_obligation);
					self.obligations.insert(
						&loss_decision.identity.issuer_id,
						&issuer_obligations,
					); } else {
					let computed_loss_vec_index_option = policy
							.computed_losses
							.iter()
							.position(|computed_loss| *computed_loss.identity.id == *loss_decision.identity.id);
					assert!(computed_loss_vec_index_option.is_some(), "COMPUTED_LOSS_NOT_FOUND_IN_POLICY");
					let computed_loss_vec_index: usize = computed_loss_vec_index_option.unwrap();
					let computed_loss: ComputedLoss = policy.computed_losses.get(computed_loss_vec_index).unwrap().clone();
					policy.computed_losses.remove(computed_loss_vec_index);
					policy.rejected_losses.push(computed_loss);
					let new_pending_balance = policy.pending_balance
							+ policy.rejected_losses.last().unwrap().calculations.amount_due;
					policy.pending_balance = new_pending_balance;
				};
				self.policies.insert(&policy.policy_id, &policy);
		};
		let mut loss_identities: Vec<LossIdentity> =
				self.loss_identities.get(&loss_decision.identity.client_id).unwrap();
		let loss_identity_vec_option = loss_identities
				.iter()
				.position(|vec_loss_identity| *vec_loss_identity.id == loss_decision.identity.id);
		if loss_identity_vec_option.is_some() {
			loss_identities.remove(loss_identity_vec_option.unwrap());
			self.loss_identities
			    .insert(&loss_decision.identity.client_id, &loss_identities);
		} else {
			panic!("LOSS_IDENTITY_NOT_FOUND");
		}
		loss_decision
	}



	///#Description
	///
	/// get a policy's remaining balance
	///
	/// #Parameters
	///
	/// *`policy_id` unique id of policy
	pub fn get_policy_balance(&self, policy_id: &String) -> f64 {
		let policy_option = self.policies.get(policy_id);
		assert!(policy_option.is_some(),"NO_POLICY_FOUND");
		let policy = policy_option.unwrap();
		policy.balance
	}

	pub fn add_policy_activator(&mut self, policy_activator: &AccountId) -> AccountId {
		assert_eq!(env::predecessor_account_id(), self.master_admin);
		self.policy_managers.push(policy_activator.clone());
		policy_activator.clone()
	}

	pub fn remove_policy_activator(&mut self, policy_activator: &AccountId) ->AccountId{
		assert_eq!(env::predecessor_account_id(), self.master_admin);
		let index = self
				.policy_managers
				.iter()
				.position(|manager| *manager == *policy_activator);
		assert!(index.is_some(), "That Policy Activator was not found");
		let removed_activator:AccountId = self.policy_managers.swap_remove(index.unwrap());
		removed_activator
	}

	///starts the two part process to abdicate an AccountId as master_admin
	///
	/// # Parameters
	///
	/// * `new_master_admin` **`AccountId`** who will be the new `master_admin`
	pub fn suspend_master_admin(&mut self, new_master_admin: AccountId) ->AccountId{
		assert_eq!(
			env::predecessor_account_id(),
			self.master_admin,
			"only master admin can start abdication process."
		);
		self.new_master_admin = Some(new_master_admin.clone());
		new_master_admin
	}

	///previous `master_admin` can cancel the two part abdication process
	pub fn cancel_master_admin_abdication(&mut self) -> bool{
		assert_eq!(
			env::predecessor_account_id(),
			self.master_admin,
			"only current master admin can do that"
		);
		self.new_master_admin = None;
		true
	}

	///new `master_admin` claims the role
	pub fn claim_master_admin(&mut self) -> AccountId {
		assert!(
			self.new_master_admin.is_some(),
			"No abdication process started"
		);
		let new_master_admin = self.new_master_admin.clone().unwrap();
		assert_eq!(
			env::predecessor_account_id(),
			new_master_admin,
			"you are not the new master admin"
		);
		self.master_admin = new_master_admin.clone();
		self.new_master_admin = None;
		new_master_admin.clone()
		}
}

#[ext_contract(claims_contract)]
trait ClaimsContract {
	fn compute_loss(loss_contexts:Vec<LossContext>)->Vec<ComputedLoss>;
}





