use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::collections::Vector;
use near_sdk::{
    env, ext_contract, init, near_bindgen, AccountId, BorshStorageKey, PanicOnDefault, Promise,
    PromiseResult,
};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};
//#Description Stroage key enum for NEAR Protocol persistent storage
#[derive(BorshStorageKey, BorshSerialize)]
pub enum StorageKeys {
    Policies,
}

///#Description
///
///  `Quote` is an offer from an insurer that has been accepted by a client
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Debug, PartialEq, Clone)]
pub struct Quote {
    ///unique to each client
    client: String,
    ///unique to each quote
    id: String,
    ///Trigger contracts define the logic and oracle data that validates policy triggers
    events_contract: AccountId,
    ///an `and` gate for parametric payout. all conditions must be must for payout condition to be true
    triggers: HashMap<String, i32>,
    ///maxmim value total value of the policy
    max_payout: u32,
    ///a representation of client location. Can be Z curve or geohash or something else
    location: String,
    ///the period that a policy will be valid.
    coverage_period: [u64; 2],
}

///#Description
///
/// defines a tracked weather occurence that has been recorded by an `Oracle`
///
#[derive(BorshDeserialize, BorshSerialize, Debug, PartialEq, Clone)]
pub struct Event {
    ///unique identification for an event
    id: String,
    ///the trust oracle that notifies of the event
    oracle: AccountId,
    ///the date of the event as told by the oracle
    date: u64,
}

///#Description
///
/// `Payout` represent ana amount paid from insurer to client based on predetermined trigger thresholds
/// payout amounts are determined offchain to protect pricing algorithms
#[derive(BorshDeserialize, BorshSerialize, Debug, PartialEq, Clone)]
pub struct Payout {
    ///unique identifier can be one that links the event to the payout such has a hash
    id: String,
    ///amount paid out to client is determined off chain
    amount: u32,
    ///the associated event that caused the payout to be executed
    event: Event,
}

///#Description
///
///  an insurer uses a `Quote` to create a policy
#[derive(BorshDeserialize, BorshSerialize, Debug)]
pub struct Policy {
    ///unique identifier can be deterministic.
    id: String,
    ///client unique identifier
    client: String,
    ///vector of all payouts done for this policy
    completed_payouts: Vector<Payout>,
    ///the total balance of the account. can be at most equal to the `max_payout` in the `Policy`'s `Quote`
    balance: u32,
    ///the `Quote`, originally issued from the QuoteManager. the `Quote` defines theh `Policy`
    quote: Quote,
}

///#Definition
/// 
/// PolicyManagerError
#[derive(Debug)]
pub enum PolicyManagerError {
    PolicyNotFound,
}

impl Error for PolicyManagerError {}

impl fmt::Display for PolicyManagerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            PolicyManagerError::PolicyNotFound => write!(f, "No records of that policy."),
        }
    }
}

//accept events that are older than 72 hours
///#Description
///
/// a contract that maintains the state of policies
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct PolicyManager {
    policies: UnorderedMap<String, Policy>,
    privileged_parties: Vec<AccountId>,
    owner: AccountId,
}

///#Description
///
/// a smart contract for the NEAR protocoll that maintains parametric insurance policy state
/// receives `Quotes` from a `QuoteManager` Contract
#[near_bindgen]
impl PolicyManager {
    #[init]
    pub fn new() -> Self {
        assert!(!near_sdk::env::state_exists(), "Already initialized");
        Self {
            policies: near_sdk::collections::UnorderedMap::new(StorageKeys::Policies),
            owner: near_sdk::env::predecessor_account_id(),
            privileged_parties: Vec::new(),
        }
    }
    ///#Description
    ///
    /// activate a policy through this method
    ///
    /// #Parameters
    /// *`client` this is a unique identifier for a client (insuranace buyer)
    /// *`id` unique identifier for the policy
    /// *`events_contract` a contract that maintains the logic that validates a policy's threshold value
    /// *`triggers` threshold values that dictate if a payout is due based on a weather `Event`
    /// *`max_payout` the maximum total value of a policy
    /// *`location` a string representation of location (e.g. geohash)
    /// *`coverage_period` the period in which the poilcy is valid
    ///
    /// #Returns
    ///
    /// `String` function returns the new policy id
    pub fn activate_policy(
        &mut self,
        client: String,
        id: String,
        events_contract: AccountId,
        triggers: HashMap<String, i32>,
        max_payout: u32,
        location: String,
        coverage_period: [u64; 2],
    ) -> String {
        let quote = Quote {
            client,
            id,
            events_contract,
            triggers,
            max_payout,
            location,
            coverage_period,
        };
        let policy_id = PolicyManager::calculate_hash(&(&quote.client, &quote.id));
        let policy_id_clone = policy_id.clone();
        let vector_id = policy_id.clone().into_bytes();
        let policy = Policy {
            id: policy_id,
            client: quote.client.clone(),
            completed_payouts: Vector::new(vector_id),
            balance: quote.max_payout.clone(),
            quote: quote,
        };
        self.policies.insert(&policy.id, &policy);
        policy_id_clone
    }

    ///#Description
    ///
    /// get a `Policy`
    ///
    /// #Parameters
    ///
    /// *`policy_id` the unique identifier for a policy
    ///
    /// #Returns
    ///
    /// `Option(Policy)` a policy if found
    pub fn get_policy(&self, policy_id: &String) -> Option<Policy> {
        self.policies.get(policy_id)
    }

    ///#Description
    ///
    /// update policy state with information related to off chain payout
    ///
    /// #Parameters
    ///
    /// *`policy_id` the policy's identifier
    /// *`event` a tuple with the relevant information of the event that triggered the payout
    /// *`amount` the amount paid
    ///
    /// #Returns
    ///
    /// `Result<String, PolicyManagerError>` returns a `String` representing the new
    /// payout id or an error if no policy was found
    pub fn save_completed_payout(
        &mut self,
        policy_id: &String,
        event: (String, AccountId, u64),
        amount: u32,
    ) -> Result<String, PolicyManagerError> {
        //amount will probably be aa string or something else
        assert!(
            self.privileged_parties
                .contains(&near_sdk::env::predecessor_account_id()),
            "Not permitted."
        );

        if let Some(mut policy) = self.policies.get(policy_id) {
            assert!(policy.balance > amount, "amount mistake.");
            let payout_event = Event {
                id: event.0,
                oracle: event.1,
                date: event.2,
            };
            let payout_id = PolicyManager::calculate_hash(&(&policy_id, &payout_event.id));
            let completed_payout = Payout {
                id: payout_id,
                amount,
                event: payout_event,
            };
            policy.completed_payouts.push(&completed_payout);
            policy.balance = policy.balance - completed_payout.amount;
            self.policies.insert(&policy.id, &policy);
            let send_id = completed_payout.id.clone();
            Ok(send_id)
        } else {
            Err(PolicyManagerError::PolicyNotFound)
        }
    }

    ///#Description
    ///
    /// retrieve `Payout` information
    ///
    /// #Parameters
    /// *`policy_id` unique policy id
    /// *`payout_id` unique payout id
    ///
    /// #Returns
    ///
    /// `Result<Option<Payout>, PolicyManagerError>` returns  a `Option(Policy)`
    /// if the policy exists and a `PolicyManagerError` if no policy was found
    pub fn get_completed_payout(
        &self,
        policy_id: &String,
        payout_id: &String,
    ) -> Result<Option<Payout>, PolicyManagerError> {
        if let Some(policy) = self.policies.get(policy_id) {
            let payout_option: Option<Payout> = policy
                .completed_payouts
                .iter()
                .filter_map(|payout_ref| {
                    *payout_ref.id.to_string() == *payout_id;
                    Some(payout_ref)
                })
                .next();

            Ok(payout_option)
        } else {
            Err(PolicyManagerError::PolicyNotFound)
        }
    }

    ///#Description
    ///
    /// checks to see if there is a triggering `Event`
    ///
    /// #Parameters
    /// *`policy_id` the unique id of the policy
    pub fn check_for_events(&mut self, policy_id: &String) -> Option<Promise> {
        if let Some(policy) = self.policies.get(policy_id) {
            if near_sdk::env::block_timestamp() < policy.quote.coverage_period[1] {
                Some(
                    events_contract::check_for_events(
                        policy.quote.triggers,
                        &policy.quote.events_contract.to_string(),
                        0,
                        5_000_000_000_000,
                    )
                    .then(self_contract::events_callback(
                        &env::current_account_id(),
                        0,                 // yocto NEAR to attach to the callback
                        5_000_000_000_000, // gas to attach to the callback
                    )),
                )
            } else {
                None
            }
        } else {
            None
        }
    }

    ///#Description
    ///
    /// this callback is executed by the triggers contract
    /// 
    /// #Return
    /// 
    /// *`String` (event_id)
    /// *`AccountId (oracle_account)
    /// *`String` (policy_id)
    /// *`u64` (date)
    pub fn events_callback(&mut self) -> Option<(String,AccountId,String, u64)> {
        match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Failed => None,
            PromiseResult::Successful(result) => {
                let data =
                    near_sdk::serde_json::from_slice::<HashMap<String, Vec<i32>>>(&result).unwrap();
                Some(data)
            }
        }
    }

    ///#Description
    ///
    /// get all the completed payouts to a cilent
    ///
    /// #Parameters
    ///
    /// *`policy_id` policy unique id
    fn get_all_completed_payouts(
        &self,
        policy_id: &String,
    ) -> Result<Vector<Payout>, PolicyManagerError> {
        if let Some(policy) = self.policies.get(policy_id) {
            Ok(policy.completed_payouts)
        } else {
            Err(PolicyManagerError::PolicyNotFound)
        }
    }

    ///#Description
    ///
    /// get a policy's remaining balance
    ///
    /// #Parameters
    ///
    /// *`policy_id` unique id of policy
    pub fn get_policy_balance(&self, policy_id: &String) -> Result<u32, PolicyManagerError> {
        if let Some(policy) = self.policies.get(policy_id) {
            Ok(policy.balance)
        } else {
            Err(PolicyManagerError::PolicyNotFound)
        }
    }

    ///#Description
    ///
    /// change the owner of this contract. certain administrative functions can only be
    /// executed by an owner
    ///
    /// #Parameters
    ///
    /// *`new_owner` the `AccountId` of the new owner. can only be set by the old owner
    pub fn change_owner(&mut self, new_owner: AccountId) {
        assert!(
            near_sdk::env::predecessor_account_id() == self.owner,
            "you cant."
        );
        self.owner = new_owner;
    }

    ///#Description
    ///
    /// a privileged user (policy issuer) is able to create policies. this function adds an `AccountId` to
    /// that vector
    ///
    /// #Parameters
    /// *`user` an `AccountId` of a policy issuer
    pub fn add_privileged_user(&mut self, user: AccountId) {
        assert!(
            near_sdk::env::predecessor_account_id() == self.owner,
            "you cant."
        );
        self.privileged_parties.push(user);
    }
    ///#Description
    ///
    /// checks if account is privileged user (issuer)
    ///
    /// #Parameters
    ///
    /// *`account_id` account id of user that wants to issue policies
    pub fn is_privileged_user(&self, account_id: AccountId) -> bool {
        self.privileged_parties.contains(&account_id)
    }

    ///#Description
    ///
    /// internal function to create unique ids
    fn calculate_hash<T: Hash>(t: &T) -> String {
        let mut s = DefaultHasher::new();
        t.hash(&mut s);
        s.finish().to_string()
    }
}

#[ext_contract(events_contract)]
trait EventManager {
    fn check_for_events(triggers: HashMap<String, i32>) -> Option<(String, AccountId, u64)>;
}

#[ext_contract(self_contract)]
trait SelfContract {
    fn events_callback(&self) -> Option<(String, AccountId, u64)>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, VMContext};
    //policy_id and payout_id were pregenerated can be seen in the output with cargo test -- --nocapture

    #[test]
    fn get_policy() {
        let policy_id: String = "8614638452391330545".to_string();
        let context = get_context("hillridge.near".to_string(), 1000000, 0);
        testing_env!(context);
        let policy_manager = policy_manager_constructor();
        let policy = policy_manager.get_policy(&policy_id);
        assert!(policy.unwrap().id == policy_id);
    }

    #[test]
    fn save_get_completed_payout() {
        let policy_id: String = "8614638452391330545".to_string();
        let payout_id: String = "1724167287788456284".to_string();
        let context = get_context("hillridge.near".to_string(), 1000000, 0);
        testing_env!(context);
        let mut policy_manager = policy_manager_constructor();
        let event = (
            "some_event_id".to_string(),
            "some.account.id".to_string(),
            12312312312323,
        );
        let saved_payout = policy_manager.save_completed_payout(&policy_id, event, 100000);
        let payout = policy_manager.get_completed_payout(&policy_id, &payout_id);
        let retrieved_payout = policy_manager.get_completed_payout(&policy_id, &payout_id);
        let payout_option = retrieved_payout.unwrap();
        assert!(payout_option != None, "payout not found");
    }

    #[test]
    fn get_all_payouts() {
        let policy_id: String = "8614638452391330545".to_string();
        let context = get_context("hillridge.near".to_string(), 1000000, 0);
        testing_env!(context);
        let mut policy_manager = policy_manager_constructor();
        let event = (
            "some_event_id".to_string(),
            "some.account.id".to_string(),
            12312312312323,
        );
        let saved_payout = policy_manager.save_completed_payout(&policy_id, event, 100000);

        let all_payouts = policy_manager.get_all_completed_payouts(&policy_id);

        assert!(all_payouts.unwrap().len() > 0);
    }

    #[test]
    fn get_policy_balance() {
        let policy_id: String = "8614638452391330545".to_string();
        let policy_payout: u32 = 100000;
        let context = get_context("hillridge.near".to_string(), 1000000, 0);
        testing_env!(context);
        let mut policy_manager = policy_manager_constructor();
        let event = (
            "some_event_id".to_string(),
            "some.account.id".to_string(),
            12312312312323,
        );
        let policy: Option<Policy> = policy_manager.get_policy(&policy_id);
        let saved_payout = policy_manager.save_completed_payout(&policy_id, event, policy_payout);
        let policy_balance = policy_manager.get_policy_balance(&policy_id);
        println!("policy_balance {:?}", policy_balance);
        assert!(policy_balance.unwrap() == (policy.unwrap().balance - policy_payout));
    }

    #[test]
    #[should_panic]
    fn change_owner() {
        let context = get_context("hillridge.near".to_string(), 1000000, 0);
        testing_env!(context);
        let mut policy_manager = policy_manager_constructor();
        policy_manager.change_owner("someone.else".to_string());
        policy_manager.add_privileged_user("another.one".to_string());
    }

    #[test]
    fn add_privileged_user() {
        let test_user = "someone.near".to_string();

        let context = get_context("hillridge.near".to_string(), 1000000, 0);
        testing_env!(context);
        let mut policy_manager = policy_manager_constructor();
        assert!(policy_manager.is_privileged_user(test_user.clone()) == false);
        policy_manager.add_privileged_user(test_user.clone());
        assert!(policy_manager.is_privileged_user(test_user) == true);
    }

    #[test]
    fn is_privileged_user() {
        let test_user = "someone.near".to_string();
        let context = get_context("hillridge.near".to_string(), 1000000, 0);
        testing_env!(context);
        let mut policy_manager = policy_manager_constructor();
        let is_privileged = policy_manager.is_privileged_user("hillridge.near".to_string());
        let not_privileged = policy_manager.is_privileged_user(test_user);

        assert!(is_privileged == true);
        assert!(not_privileged == false);
    }

    fn policy_manager_constructor() -> PolicyManager {
        let mut triggers = HashMap::new();
        triggers.insert("hurricane_category".to_string(), 34);
        triggers.insert("hurricane_distance".to_string(), 100);
        let quote = Quote {
            client: "some_id".to_string(),
            id: "some_quote_id".to_string(),
            events_contract: "some.valid.address".to_string(),
            triggers,
            max_payout: 1000000000,
            location: "someGeohash".to_string(),
            coverage_period: [123123123, 1231023123],
        };

        let mut policy_manager = PolicyManager::new();
        policy_manager.add_privileged_user(near_sdk::env::predecessor_account_id().to_string());

        let policy_id = policy_manager.activate_policy(
            quote.client,
            quote.id,
            quote.events_contract,
            quote.triggers,
            quote.max_payout,
            quote.location,
            quote.coverage_period,
        );
        policy_manager
    }

    fn get_context(
        predecessor_account_id: String,
        storage_usage: u64,
        blocktime: u64,
    ) -> VMContext {
        VMContext {
            current_account_id: "alice.testnet".to_string(),
            signer_account_id: "jane.testnet".to_string(),
            signer_account_pk: vec![0, 1, 2],
            predecessor_account_id,
            input: vec![],
            block_index: 0,
            block_timestamp: blocktime,
            account_balance: 0,
            account_locked_balance: 0,
            storage_usage,
            attached_deposit: 0,
            prepaid_gas: 10u64.pow(18),
            random_seed: vec![0, 1, 2],
            is_view: false,
            output_data_receivers: vec![],
            epoch_height: 19,
        }
    }
}
