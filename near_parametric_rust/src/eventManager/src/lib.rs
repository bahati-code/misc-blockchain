use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::collections::Vector;
use near_sdk::{
    env, ext_contract, init, near_bindgen, AccountId, BorshStorageKey, PanicOnDefault, Promise,
    PromiseResult,
};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
//#Description Stroage key enum for NEAR Protocol persistent storage
#[derive(BorshStorageKey, BorshSerialize)]
pub enum StorageKeys {
    Requests,
    AuthorizedNodes,
    Admins,
}

///#Description
///
/// this is a weather even as defined by an `Oracle`
#[derive(BorshDeserialize, BorshSerialize, Debug, PartialEq)]
pub struct Event {
    ///an events unique identification.
    id: String,
    ///the Oracle contract that notified the trigger contract of this event
    oracle: AccountId,
    ///date of occurence
    date: u64,
}

///#Description
///
/// a request for data from a `PolicyManager`
#[derive(BorshDeserialize, BorshSerialize, Debug, PartialEq)]
pub struct Request {
    ///the unique policy id
    policy_id: String,
    ///the `PolicyManager`
    policy_manager: AccountId,
    ///data to be checked
    triggers: (u8, Vec<i32>),
}

///#Definition
///
/// PolicyManagerError
#[derive(Debug)]
pub enum HurricaneOracleError {
    TriggerDataError,
    RequestNotFound,
}

impl Error for HurricaneOracleError {}

impl fmt::Display for HurricaneOracleError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            HurricaneOracleError::TriggerDataError => {
                write!(f, "trigger data improperly formatted.")
            }
            HurricaneOracleError::RequestNotFound => {
                write!(f, "the request was not found")
            }
        }
    }
}

///#Description
///
/// this is aa sense an abstract contract that will be define per Event type
/// (e.g hurrricane, tornado, drought, etc)
/// and/or valid a `PolicyManager` state changes
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct HurricaneOracle {
    ///accounts authorized to interactive with this contract
    authorized_accounts: Vector<AccountId>,
    ///requests pending processing
    requests: UnorderedMap<String, Request>,
    ///admins of this oracle
    admins: Vector<AccountId>,
    /// master admin
    master_admin: AccountId,
}

impl HurricaneOracle {
    #[init]
    pub fn new() -> Self {
        assert!(!near_sdk::env::state_exists(), "Already initialized");
        Self {
            master_admin: env::predecessor_account_id(),
            admins: Vector::new(StorageKeys::Admins),
            authorized_accounts: Vector::new(StorageKeys::AuthorizedNodes),
            requests: UnorderedMap::new(StorageKeys::Requests),
        }
    }

    ///#Description
    ///
    /// save a request for policy triggering `Event`s
    ///
    /// #Parameters
    ///
    /// *`policy_id` the policy unique identifier
    /// *`triggers` the triggers of the respective policy
    pub fn check_for_events(
        &mut self,
        policy_id: String,
        triggers: HashMap<String, Vec<i32>>,
    ) -> Result<(), HurricaneOracleError> {
        let category_option: Option<u8> = {
            if let Some(category_vec) = triggers.get("category") {
                assert!(category_vec.len() > 0);
                let category: u8 = category_vec[0] as u8;
                Some(category)
            } else {
                None
            }
        };
        let location_option: Option<Vec<i32>> = {
            if let Some(location_vec) = triggers.get("location") {
                assert!(location_vec.len() > 0);
                Some(location_vec.to_vec())
            } else {
                None
            }
        };

        if (category_option.clone(), location_option.clone()) != (None, None) {
            let request: Request = Request {
                policy_id: policy_id.clone(),
                policy_manager: env::predecessor_account_id(),
                triggers: (category_option.unwrap(), location_option.unwrap()),
            };
            self.requests.insert(&policy_id, &request);
            Ok(())
        } else {
            Err(HurricaneOracleError::TriggerDataError)
        }
    }

    ///#Description
    ///
    /// authorized node gets a request to processs
    ///
    /// #Parameter
    ///
    /// *`policy_id` the policy to retrieve
    pub fn get_request(&self, policy_id: String) -> Option<Request> {
        assert!(
            self.authorized_accounts
                .to_vec()
                .contains(&env::predecessor_account_id()),
            "not authorized."
        );
        self.requests.get(&policy_id)
    }

    pub fn get_all_requests(&self) -> Vec<Request> {
        assert!(
            self.authorized_accounts
                .to_vec()
                .contains(&env::predecessor_account_id()),
            "not authorized."
        );
        self.requests.values().collect()
    }
    ///#Description
    ///
    /// authorized node calls this function to return data
    ///
    /// #Parameters
    ///
    /// *`policy_id` unique policy id
    /// *`event`
    ///   *`event_id` a `String` of the event's unique id
    ///   *`date` the date of the event in nanoseconds
    pub fn fulfill_request(
        &mut self,
        policy_id: String,
        event_data: (String, u64),
    ) -> Result<Promise, HurricaneOracleError> {
        assert!(self
            .authorized_accounts
            .to_vec()
            .contains(&env::predecessor_account_id()));
        let oracle_account = env::predecessor_account_id();
        if let Some(request) = self.requests.get(&policy_id) {
            let promise = policy_manager::event_callback(
                (event_data.0, oracle_account, policy_id, event_data.1),
                &request.policy_manager,
                0,
                5_000_000_000_000,
            );
            Ok(promise)
        } else {
            Err(HurricaneOracleError::RequestNotFound)
        }
    }
    //administrative functions

    pub fn add_authorized_account(&mut self, auth_account: AccountId) {
        assert!(self
            .admins
            .to_vec()
            .contains(&env::predecessor_account_id()));
        self.authorized_accounts.push(&auth_account);
    }

    pub fn remove_authorized_account(&mut self, auth_account: AccountId) {
        assert!(self
            .admins
            .to_vec()
            .contains(&env::predecessor_account_id()));
        let index = self
            .authorized_accounts
            .iter()
            .position(|account| account == auth_account)
            .unwrap() as u64;

        self.authorized_accounts.swap_remove(index);
    }

    pub fn add_admin(&mut self, admin: AccountId) {
        assert!(self.master_admin == env::predecessor_account_id());
        self.admins.push(&admin);
    }

    pub fn remove_admin(&mut self, old_admin: AccountId) {
        assert!(self.master_admin == env::predecessor_account_id());
        let index = self
            .admins
            .iter()
            .position(|account| account == old_admin)
            .unwrap() as u64;

        self.admins.swap_remove(index);
    }

    pub fn change_master_admin(&mut self, new_admin: AccountId) {
        assert!(self.master_admin == env::predecessor_account_id());
        self.master_admin = new_admin;
    }
}

#[ext_contract(policy_manager)]
trait PolicyManager {
    //(event_data.0, oracle_account, policy_id, event_data.1)
    fn event_callback(event: (String, AccountId, String, u64)) -> Option<Event>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, VMContext};
}
