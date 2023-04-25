use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::{ext_contract, init, near_bindgen, AccountId, BorshStorageKey, PanicOnDefault};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::panic;

//#Description Stroage key enum for NEAR Protocoll persistent storage
#[derive(BorshStorageKey, BorshSerialize)]
pub enum StorageKeys {
    UndecidedQuotes,
    DaysValid,
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
    triggers_contract: AccountId,
    ///Triggers are the threshold value that executes payment for a poilcy
    triggers: HashMap<String, i32>,
    ///maxmim value total value of the policy
    max_payout: u32,
    ///a representation of client location. Can be Z curve or geohash or something else
    location: String,
    ///the period that a policy will be valid.
    coverage_period: [u64; 2],
}

//implement data valildation

///#Description
///
/// `UndecidedQuote` holds a `Quote` and the period of time that it is valid
#[derive(BorshDeserialize, BorshSerialize, Debug, PartialEq, Clone)]
pub struct UndecidedQuote {
    ///a quote presented by an issuer
    quote: Quote,
    ///the deadline in nanoseconds before the Quote becomes invalid
    accept_deadline: u64,
}

///#Description
///
/// a smart contract that holds approved yet unaccepted quotes and issues policies
/// through cross contract call to poilcy manager
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct QuoteManager {
    ///clientId is the key
    undecided_quotes: UnorderedMap<String, UndecidedQuote>,
    ///the owner of this Contract
    owner: AccountId,
    ///who are the valid parties that can issue quotes
    quote_issuers: Vec<AccountId>,
    ///what days have the quote issuers determined to be the period of time that a quote remains valid
    standard_days_valid: UnorderedMap<AccountId, u64>,
}

#[near_bindgen]
impl QuoteManager {
    #[init]
    pub fn new() -> Self {
        assert!(!near_sdk::env::state_exists(), "Already initialized");
        Self {
            owner: near_sdk::env::predecessor_account_id(),
            undecided_quotes: near_sdk::collections::UnorderedMap::new(
                StorageKeys::UndecidedQuotes,
            ),
            quote_issuers: Vec::new(),
            standard_days_valid: HashMap::new(Storage::Keys::DaysValid),
        }
    }

    ///a quote is issued with a set , predetermined, valid time period.
    /// a quote does not become a policy until accepted by client
    pub fn issue_quote(
        &mut self,
        client: String,
        id: String,
        triggers_contract: AccountId,
        triggers: HashMap<String, i32>,
        max_payout: u32,
        location: String,
        coverage_period: [u64; 2],
    ) {
        assert!(
            self.quote_issuers
                .contains(&near_sdk::env::predecessor_account_id()),
            "Not permitted."
        );
        let quote = Quote {
            client,
            id,
            triggers_contract,
            triggers,
            max_payout,
            location,
            coverage_period,
        };
        let undecided_quote = UndecidedQuote {
            accept_deadline: self.get_valid_period(near_sdk::env::predecessor_account_id()),
            quote,
        };

        self.undecided_quotes
            .insert(&undecided_quote.quote.id, &undecided_quote);
    }

    ///remove single invalid quote
    pub fn remove_invalid_quote(&mut self, quote_id: &String) {
        assert!(*&near_sdk::env::predecessor_account_id() == self.owner);
        if let Some(quote) = self.undecided_quotes.get(quote_id) {
            assert!(self.is_valid_quote(&quote) != true, "quote is still valid");
            self.undecided_quotes.remove(quote_id);
        }
    }

    ///when a client accepts a quote through an offchain process the issuer
    /// creates a valid policy by calling this function
    pub fn issue_policy(&mut self, quote_id: String) {
        assert!(
            self.quote_issuers
                .contains(&near_sdk::env::predecessor_account_id()),
            "Not permitted."
        );
        if let Some(undecided_quote) = self.undecided_quotes.get(&quote_id) {
            if self.is_valid_quote(&undecided_quote) {
                let accepted_quote = undecided_quote.quote;
                policy_manager::activate_policy(
                    accepted_quote.client,
                    accepted_quote.id,
                    accepted_quote.triggers,
                    accepted_quote.max_payout,
                    accepted_quote.location,
                    accepted_quote.coverage_period,
                    &"policymanager.accountid",
                    0,
                    5_000_000_000_000,
                );
            }
        }
    }

    ///get  quote by its id
    pub fn get_quote(&self, quote_id: String) -> Option<UndecidedQuote> {
        self.undecided_quotes.get(&quote_id)
    }

    ///a quote issuer can change the number of days that a quote is valid for all potential quotes
    /// this value is constant
    pub fn change_days_valid(&mut self, days_valid: u64) {
        assert!(
            self.quote_issuers
                .contains(&near_sdk::env::predecessor_account_id()),
            "only valid issuer."
        );
        self.standard_days_valid
            .insert(near_sdk::env::predecessor_account_id(), days_valid);
    }

    ///change the owner of the Quote Manager contract
    pub fn change_owner(&mut self, new_owner: AccountId) {
        assert!(
            near_sdk::env::predecessor_account_id() == self.owner,
            "only owner"
        );
        self.owner = new_owner;
    }

    ///add an valid quote issuer (insurer) to the white list
    pub fn add_issuer(&mut self, new_issuer: AccountId, deadline_length: u64) {
        assert!(
            near_sdk::env::predecessor_account_id() == self.owner,
            "only owner"
        );
        self.quote_issuers.push(new_issuer.clone());
        self.standard_days_valid.insert(new_issuer, deadline_length);
    }

    ///remove a quote issuer from the white list
    pub fn remove_issuer(&mut self, old_issuer: AccountId) {
        assert!(
            near_sdk::env::predecessor_account_id() == self.owner,
            "only owner"
        );
        self.standard_days_valid.remove(&old_issuer);
        let index = self
            .quote_issuers
            .iter()
            .position(|x| *x == old_issuer)
            .unwrap();
        self.quote_issuers.remove(index);
    }

    ///get the standard number of days that a quote issuers quotes are valid
    fn get_valid_period(&self, issuer: AccountId) -> u64 {
        if let Some(valid_days) = self.standard_days_valid.get(&issuer) {
            const NANO_SECONDS_IN_DAY: u64 = 86400000000000;
            valid_days * NANO_SECONDS_IN_DAY + near_sdk::env::block_timestamp()
        } else {
            panic!("valid quote issuer not found");
        }
    }

    //perhaps remove this function if the other one is implemented.
    fn is_valid_quote(&self, quote: &UndecidedQuote) -> bool {
        quote.accept_deadline > near_sdk::env::block_timestamp()
    }


}

#[ext_contract(policy_manager)]
pub trait PolicyManager {
    //borsh serialization not implemented for Quote
    fn activate_policy(
        client: String,
        id: String,
        triggers: HashMap<String, i32>,
        max_payout: u32,
        location: String,
        coverage_period: [u64; 2],
    );
}

#[ext_contract(event_manager)]
pub trait EventManager{
  
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, VMContext};

    #[test]
    #[should_panic]
    ///Unauthorized quote issuance
    fn unauthorized_issue_quote() {
        let mut triggers = HashMap::new();
        triggers.insert("hurricane_category".to_string(), 34);
        triggers.insert("hurricane_distance".to_string(), 100);
        let context = get_context("hillridge.near".to_string(), 1000000, 0);
        testing_env!(context);
        let mut quote_manager = QuoteManager::new();
        quote_manager.issue_quote(
            "some.client.id".to_string(),
            "some_id".to_string(),
            "trigger.contract".to_string(),
            triggers,
            1000000000,
            "someGeohash".to_string(),
            [123123123, 1231023123],
        );
    }

    #[test]
    fn authorized_issue_quote() {
        let context = get_context("hillridge.near".to_string(), 1000000, 0);
        testing_env!(context);
        let mut triggers = HashMap::new();
        triggers.insert("hurricane_category".to_string(), 34);
        triggers.insert("hurricane_distance".to_string(), 100);

        let mut quote_manager = QuoteManager::new();
        quote_manager.add_issuer("hillridge.near".to_string(), 7);
        quote_manager.issue_quote(
            "some.client.id".to_string(),
            "some_id".to_string(),
            "trigger.contract".to_string(),
            triggers,
            1000000000,
            "someGeohash".to_string(),
            [123123123, 1231023123],
        );
    }

    #[test]
    fn issue_policy() {
        let context = get_context("hillridge.near".to_string(), 1000000, 0);
        testing_env!(context);
        let mut triggers = HashMap::new();
        triggers.insert("hurricane_category".to_string(), 34);
        triggers.insert("hurricane_distance".to_string(), 100);
        let mut quote_manager = QuoteManager::new();
        quote_manager.add_issuer("hillridge.near".to_string(), 7);
        quote_manager.issue_quote(
            "some.client.id".to_string(),
            "some_id".to_string(),
            "trigger.contract".to_string(),
            triggers,
            1000000000,
            "someGeohash".to_string(),
            [123123123, 1231023123],
        );
        quote_manager.issue_policy("some_quote_id".to_string());
    }

    #[test]
    fn remove_invalid_quote() {
        let context = get_context("hillridge.near".to_string(), 1000000, 0);
        testing_env!(context);
        let mut triggers = HashMap::new();
        triggers.insert("hurricane_category".to_string(), 34);
        triggers.insert("hurricane_distance".to_string(), 100);
        let mut quote_manager = QuoteManager::new();
        quote_manager.add_issuer("hillridge.near".to_string(), 7);
        quote_manager.issue_quote(
            "some.client.id".to_string(),
            "some_id".to_string(),
            "trigger.contract".to_string(),
            triggers,
            1000000000,
            "someGeohash".to_string(),
            [123123123, 1231023123],
        );
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
