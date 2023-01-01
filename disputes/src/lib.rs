use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedSet};
use near_sdk::json_types::{Base64VecU8, U64};
use near_sdk::serde_json::json;
use near_sdk::{assert_one_yocto, env, is_promise_success, near_bindgen, AccountId, PanicOnDefault,
               Promise, PromiseError, PromiseOrValue, Timestamp};

pub use crate::types::*;

pub mod callbacks;
pub mod internal;
pub mod types;
pub mod utils;
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
  /// Arguments of the parties
  pub arguments: LookupMap<DisputeIndex, Vec<Reason>>,
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
      arguments: LookupMap::new(StorageKey::Reasons),
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
  pub fn create_dispute(&mut self, dispute_create: DisputeCreate) -> DisputeIndex {
    assert_one_yocto();
    assert_eq!(
      self.bounties_contract,
      env::predecessor_account_id(),
      "This method can only invoke a bounty contract"
    );
    assert!(
      !dispute_create.description.is_empty(),
      "The description cannot be empty"
    );

    let id = self.internal_add_dispute(dispute_create.to_dispute());
    id
  }

  #[payable]
  pub fn provide_arguments(&mut self, id: DisputeIndex, description: String) -> usize {
    assert_one_yocto();
    let dispute = self.get_dispute(id);
    assert!(
      matches!(dispute.status, DisputeStatus::New),
      "This action can be performed only for a dispute with the status 'New'",
    );
    assert!(
      !self.is_argument_period_expired(&dispute),
      "The period for providing arguments has expired, now you need to perform an 'escalation' action or cancel the dispute",
    );

    let side = dispute.get_side_of_dispute();
    let reason_idx = self.internal_add_argument(
      &id,
      Reason {
        side,
        argument_timestamp: U64::from(env::block_timestamp()),
        description,
      },
    );
    reason_idx
  }

  #[payable]
  pub fn cancel_dispute(&mut self, id: DisputeIndex) -> PromiseOrValue<()> {
    assert_one_yocto();
    let mut dispute = self.get_dispute(id);
    assert!(
      matches!(dispute.status, DisputeStatus::New),
      "This action can be performed only for a dispute with the status 'New'",
    );

    let success = matches!(dispute.get_side_of_dispute(), Side::ProjectOwner);
    self.internal_send_result_of_dispute(id, &mut dispute, success, true)
  }

  #[payable]
  pub fn escalation(&mut self, id: DisputeIndex) -> PromiseOrValue<()> {
    assert_one_yocto();
    let mut dispute = self.get_dispute(id);
    assert!(
      matches!(dispute.status, DisputeStatus::New),
      "This action can be performed only for a dispute with the status 'New'",
    );
    assert!(
      self.is_argument_period_expired(&dispute),
      "Escalation of the dispute is possible only after the end of the argumentation period",
    );

    self.internal_add_proposal(id, &mut dispute)
  }

  #[payable]
  pub fn result_of_dispute(
    &mut self,
    id: DisputeIndex,
    success: bool,
  ) -> PromiseOrValue<()> {
    assert_one_yocto();
    let mut dispute = self.get_dispute(id);
    assert_eq!(
      self.dispute_dao,
      env::predecessor_account_id(),
      "This method can only invoke a dispute resolution DAO contract"
    );
    assert!(
      matches!(dispute.status, DisputeStatus::DecisionPending),
      "This action can be performed only for a dispute with the status 'DecisionPending'",
    );
    assert!(
      !self.is_decision_period_expired(&dispute),
      "The decision period is over, now you need to perform the 'finalize' action",
    );

    self.internal_send_result_of_dispute(id, &mut dispute, success, false)
  }

  #[payable]
  pub fn finalize_dispute(
    &mut self,
    id: DisputeIndex,
  ) -> PromiseOrValue<()> {
    assert_one_yocto();
    let mut dispute = self.get_dispute(id);
    assert!(
      matches!(dispute.status, DisputeStatus::DecisionPending),
      "This action can be performed only for a dispute with the status 'DecisionPending'",
    );

    self.internal_get_proposal(id, &mut dispute)
  }
}
