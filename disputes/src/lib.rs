use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedSet};
use near_sdk::json_types::U64;
use near_sdk::{assert_one_yocto, env, is_promise_success, AccountId, near_bindgen, PanicOnDefault,
               PromiseError, PromiseOrValue};

pub use crate::bounties::*;
pub use crate::types::*;

pub mod bounties;
pub mod callbacks;
pub mod internal;
pub mod types;
pub mod view;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct DisputesContract {
  /// Bounties contract.
  pub bounties_contract: AccountId,
  /// Dispute resolution DAO.
  pub dispute_dao: AccountId,
  /// Last available id for the dispute.
  pub last_dispute_id: DisputeIndex,
  /// Disputes map from ID to dispute information.
  pub disputes: LookupMap<DisputeIndex, Dispute>,
  /// Dispute indexes map per bounty ID.
  pub bounty_disputes: LookupMap<u64, Vec<DisputeIndex>>,
  /// account ids that can perform all actions:
  /// - manage admin_whitelist
  /// - change contract config
  /// - etc.
  pub admin_whitelist: UnorderedSet<AccountId>,
  /// Dispute contract configuration.
  pub config: Config,
}

#[near_bindgen]
impl DisputesContract {
  #[init]
  pub fn new(
    bounties_contract: AccountId,
    dispute_dao: AccountId,
    admin_whitelist: Vec<AccountId>,
    config: Option<Config>,
  ) -> Self {
    assert!(
      admin_whitelist.len() > 0,
      "Admin whitelist must contain at least one account"
    );
    let mut admin_whitelist_set = UnorderedSet::new(StorageKey::AdminWhitelist);
    admin_whitelist_set.extend(admin_whitelist.into_iter().map(|a| a.into()));

    Self {
      bounties_contract,
      dispute_dao,
      last_dispute_id: 0,
      disputes: LookupMap::new(StorageKey::Disputes),
      bounty_disputes: LookupMap::new(StorageKey::BountyDisputes),
      admin_whitelist: admin_whitelist_set,
      config: config.unwrap_or_default(),
    }
  }

  #[payable]
  pub fn add_to_admin_whitelist(
    &mut self,
    account_id: Option<AccountId>,
    account_ids: Option<Vec<AccountId>>,
  ) {
    assert_one_yocto();
    self.assert_admin_whitelist(&env::predecessor_account_id());
    let account_ids = if let Some(account_ids) = account_ids {
      account_ids
    } else {
      vec![account_id.expect("Expected either account_id or account_ids")]
    };
    for account_id in &account_ids {
      self.admin_whitelist.insert(account_id);
    }
  }

  #[payable]
  pub fn remove_from_admin_whitelist(
    &mut self,
    account_id: Option<AccountId>,
    account_ids: Option<Vec<AccountId>>,
  ) {
    assert_one_yocto();
    self.assert_admin_whitelist(&env::predecessor_account_id());
    let account_ids = if let Some(account_ids) = account_ids {
      account_ids
    } else {
      vec![account_id.expect("Expected either account_id or account_ids")]
    };
    for account_id in &account_ids {
      self.admin_whitelist.remove(&account_id);
    }
    assert!(
      !self.admin_whitelist.is_empty(),
      "Cannot remove all accounts from admin whitelist",
    );
  }

  #[payable]
  pub fn change_config(&mut self, config: Config) {
    assert_one_yocto();
    self.assert_admin_whitelist(&env::predecessor_account_id());
    self.config = config;
  }

  #[payable]
  pub fn create_dispute(&mut self, bounty_id: u64, description: String) -> PromiseOrValue<()> {
    assert_one_yocto();
    assert_eq!(
      self.bounties_contract,
      env::predecessor_account_id(),
      "This method can only invoke a bounty contract"
    );
    assert!(
      !description.is_empty(),
      "The description cannot be empty"
    );
    self.internal_get_bounty_info(bounty_id, description)
  }
}
