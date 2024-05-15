use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedSet};
use near_sdk::json_types::{Base64VecU8, U64};
use near_sdk::serde_json::json;
use near_sdk::{env, is_promise_success, near_bindgen, AccountId, PanicOnDefault, Promise,
               PromiseError, PromiseOrValue, Timestamp};

pub use crate::types::*;

pub mod callbacks;
pub mod internal;
pub mod types;
pub mod utils;
pub mod view;
mod upgrade;

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
  pub disputes: LookupMap<DisputeIndex, VersionedDispute>,
  /// Arguments of the parties
  pub arguments: LookupMap<DisputeIndex, Vec<VersionedReason>>,
  /// Dispute indexes map per bounty ID.
  pub bounty_disputes: LookupMap<u64, Vec<DisputeIndex>>,
  /// account ids that can perform all actions:
  /// - manage admin_whitelist
  /// - change contract config
  /// - etc.
  pub admin_whitelist: UnorderedSet<AccountId>,
  /// Dispute contract configuration.
  pub config: VersionedConfig,
  /// Contract status
  pub status: ContractStatus,
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
      config: config.unwrap_or_default().into(),
      status: ContractStatus::Genesis,
    }
  }

  pub fn add_to_admin_whitelist(
    &mut self,
    account_id: Option<AccountId>,
    account_ids: Option<Vec<AccountId>>,
  ) {
    self.assert_live();
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

  pub fn remove_from_admin_whitelist(
    &mut self,
    account_id: Option<AccountId>,
    account_ids: Option<Vec<AccountId>>,
  ) {
    self.assert_live();
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

  pub fn change_config(&mut self, config: Config) {
    self.assert_live();
    self.assert_admin_whitelist(&env::predecessor_account_id());
    self.config = config.into();
  }

  pub fn create_dispute(&mut self, dispute_create: DisputeCreate) -> DisputeIndex {
    self.assert_live();
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

  pub fn provide_arguments(&mut self, id: DisputeIndex, description: String) -> usize {
    self.assert_live();
    let dispute = self.get_dispute(id);
    if dispute.status != DisputeStatus::New {
      env::panic_str(MESSAGE_DISPUTE_IS_NOT_NEW);
    }
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

  pub fn cancel_dispute(&mut self, id: DisputeIndex) -> PromiseOrValue<()> {
    self.assert_live();
    let dispute = self.get_dispute(id);
    if dispute.status != DisputeStatus::New {
      env::panic_str(MESSAGE_DISPUTE_IS_NOT_NEW);
    }

    let success = matches!(dispute.get_side_of_dispute(), Side::ProjectOwner);
    self.internal_send_result_of_dispute(
      id,
      dispute.bounty_id,
      dispute.receiver_id,
      dispute.claim_number,
      success,
      true
    )
  }

  pub fn escalation(&mut self, id: DisputeIndex) -> PromiseOrValue<()> {
    self.assert_live();
    let dispute = self.get_dispute(id);
    if dispute.status != DisputeStatus::New {
      env::panic_str(MESSAGE_DISPUTE_IS_NOT_NEW);
    }
    assert!(
      self.is_argument_period_expired(&dispute),
      "Escalation of the dispute is possible only after the end of the argumentation period",
    );

    self.internal_add_proposal(id, dispute)
  }

  pub fn result_of_dispute(
    &mut self,
    id: DisputeIndex,
    success: bool,
  ) -> PromiseOrValue<()> {
    self.assert_live();
    let dispute = self.get_dispute(id);
    assert_eq!(
      self.dispute_dao,
      env::predecessor_account_id(),
      "This method can only invoke a dispute resolution DAO contract"
    );
    if dispute.status != DisputeStatus::DecisionPending {
      env::panic_str(MESSAGE_DISPUTE_IS_NOT_PENDING);
    }
    assert!(
      !self.is_decision_period_expired(&dispute),
      "The decision period is over, now you need to perform the 'finalize' action",
    );

    self.internal_send_result_of_dispute(
      id,
      dispute.bounty_id,
      dispute.receiver_id,
      dispute.claim_number,
      success,
      false
    )
  }

  pub fn finalize_dispute(
    &mut self,
    id: DisputeIndex,
  ) -> PromiseOrValue<()> {
    self.assert_live();
    let dispute = self.get_dispute(id);
    if dispute.status != DisputeStatus::DecisionPending {
      env::panic_str(MESSAGE_DISPUTE_IS_NOT_PENDING);
    }

    self.internal_get_proposal(id, dispute.proposal_id.unwrap())
  }

  /// Can be used only during migrations when updating contract versions
  pub fn update_bounties_contract(&mut self, bounties_contract: AccountId) {
    self.assert_live();
    self.assert_admin_whitelist(&env::predecessor_account_id());
    self.bounties_contract = bounties_contract;
  }

  #[private]
  pub fn set_status(&mut self, status: ContractStatus) {
    assert!(
      !matches!(status, ContractStatus::Genesis),
      "The status can't be set to Genesis"
    );
    self.status = status;
  }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
  use near_sdk::{AccountId, ONE_NEAR, testing_env};
  use near_sdk::json_types::{U128, U64};
  use near_sdk::test_utils::{accounts, VMContextBuilder};
  use crate::{Config, ContractStatus, Dispute, DisputeCreate, DisputeIndex, DisputesContract,
              DisputeStatus, Reason, Side};

  fn create_dispute(
    context: &mut VMContextBuilder,
    contract: &mut DisputesContract,
    bounty_id: u64,
    description: String,
    receiver_id: AccountId,
    claim_number: Option<u8>,
    project_owner_delegate: AccountId,
  ) -> DisputeIndex {
    testing_env!(context
      .attached_deposit(1)
      .build());
    let dispute_create = DisputeCreate {
      bounty_id,
      description,
      receiver_id,
      project_owner_delegate,
      claim_number,
    };
    contract.create_dispute(dispute_create)
  }

  fn change_status(
    contract: &mut DisputesContract,
    dispute_id: DisputeIndex,
    new_status: DisputeStatus,
  ) {
    let mut dispute = contract.disputes.get(&dispute_id).unwrap().to_dispute();
    dispute.status = new_status;
    contract.disputes.insert(&dispute_id, &dispute.into());
  }

  fn escalation(
    contract: &mut DisputesContract,
    dispute_id: DisputeIndex,
  ) {
    let mut dispute = contract.disputes.get(&dispute_id).unwrap().to_dispute();
    dispute.status = DisputeStatus::DecisionPending;
    dispute.proposal_id = Some(1.into());
    dispute.proposal_timestamp = Some(0.into());
    contract.disputes.insert(&dispute_id, &dispute.into());
  }

  fn get_bounties_contract() -> AccountId {
    "bounties".parse().unwrap()
  }

  fn get_dispute_dao() -> AccountId {
    "dao".parse().unwrap()
  }

  #[test]
  fn test_create_dispute() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);

    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .block_timestamp(0)
      .build());
    let dispute_id = create_dispute(
      &mut context,
      &mut contract,
      1,
      "Test description".to_string(),
      accounts(1),
      None,
      accounts(2)
    );

    let new_dispute = Dispute {
      start_time: U64(0),
      description: "Test description".to_string(),
      bounty_id: U64(1),
      receiver_id: accounts(1),
      claim_number: None,
      project_owner_delegate: accounts(2),
      status: DisputeStatus::New,
      proposal_id: None,
      proposal_timestamp: None,
    };

    assert_eq!(contract.get_dispute(dispute_id), new_dispute);
    assert_eq!(contract.get_disputes(Some(0), Some(100)), vec![(dispute_id, new_dispute.clone())]);
    assert_eq!(contract.get_bounty_disputes(1), vec![(dispute_id, new_dispute.clone())]);
    assert_eq!(contract.get_disputes_by_ids(vec![dispute_id]), vec![(dispute_id, new_dispute)]);
    assert_eq!(contract.get_last_dispute_id(), dispute_id + 1);
    assert_eq!(contract.get_bounties_contract_account_id(), get_bounties_contract());
    assert_eq!(contract.get_dispute_dao_account_id(), get_dispute_dao());
  }

  #[test]
  #[should_panic(expected = "This method can only invoke a bounty contract")]
  fn test_create_dispute_by_other_user() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);

    testing_env!(context
      .predecessor_account_id(accounts(1))
      .block_timestamp(0)
      .build());
    create_dispute(
      &mut context,
      &mut contract,
      1,
      "Test description".to_string(),
      accounts(1),
      None,
      accounts(2)
    );
  }

  #[test]
  #[should_panic(expected = "The description cannot be empty")]
  fn test_create_dispute_with_empty_description() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);

    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .block_timestamp(0)
      .build());
    create_dispute(
      &mut context,
      &mut contract,
      1,
      "".to_string(),
      accounts(1),
      None,
      accounts(2)
    );
  }

  #[test]
  #[should_panic(expected = "Admin whitelist must contain at least one account")]
  fn test_failed_to_init_contract() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.predecessor_account_id(accounts(0)).build());
    DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![],
      None,
    );
  }

  #[test]
  fn test_add_to_admin_whitelist() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let account_ids: Vec<AccountId> = vec![accounts(1), accounts(2)];
    contract.add_to_admin_whitelist(None, Some(account_ids.clone()));
    assert_eq!(contract.get_admin_whitelist(), vec![accounts(0), accounts(1), accounts(2)]);

    contract.add_to_admin_whitelist(Some(accounts(3)), None);
    assert_eq!(
      contract.get_admin_whitelist(),
      vec![accounts(0), accounts(1), accounts(2), accounts(3)]
    );
  }

  #[test]
  fn test_remove_from_admin_whitelist() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    contract.add_to_admin_whitelist(
      None,
      Some(vec![accounts(1), accounts(2), accounts(3)])
    );

    contract.remove_from_admin_whitelist(Some(accounts(3)), None);
    assert_eq!(contract.get_admin_whitelist(), vec![accounts(0), accounts(1), accounts(2)]);
    contract.remove_from_admin_whitelist(None, Some(vec![accounts(1), accounts(2)]));
    assert_eq!(contract.get_admin_whitelist(), vec![accounts(0)]);
  }

  #[test]
  #[should_panic(expected = "Expected either account_id or account_ids")]
  fn test_failed_to_whitelist_admin() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    contract.add_to_admin_whitelist(None, None);
  }

  #[test]
  #[should_panic(expected = "Not in admin whitelist")]
  fn test_add_to_admin_whitelist_by_other_user() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);
    testing_env!(context.predecessor_account_id(accounts(1)).attached_deposit(1).build());
    contract.add_to_admin_whitelist(Some(accounts(1)), None);
  }

  #[test]
  #[should_panic(expected = "Cannot remove all accounts from admin whitelist")]
  fn test_remove_from_admin_whitelist_all_members() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    contract.remove_from_admin_whitelist(Some(accounts(0)), None);
  }

  #[test]
  fn test_change_config() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);

    assert_eq!(contract.get_config(), Config::default());

    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());

    let config = Config {
      argument_period: U64(1_000_000_000 * 60 * 60 * 24 * 12),
      decision_period: U64(1_000_000_000 * 60 * 60 * 24 * 10),
      add_proposal_bond: U128(ONE_NEAR * 2),
    };
    contract.change_config(config.clone());
    assert_eq!(contract.get_config(), config);
  }

  #[test]
  fn test_provide_arguments() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);

    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .block_timestamp(0)
      .build());
    let dispute_id = create_dispute(
      &mut context,
      &mut contract,
      1,
      "Test description".to_string(),
      accounts(1),
      None,
      accounts(2)
    );

    testing_env!(context
      .predecessor_account_id(accounts(2))
      .block_timestamp(1)
      .attached_deposit(1)
      .build());
    contract.provide_arguments(dispute_id, "The first argument".to_string());
    testing_env!(context
      .predecessor_account_id(accounts(1))
      .block_timestamp(2)
      .attached_deposit(1)
      .build());
    contract.provide_arguments(dispute_id, "The second argument".to_string());

    assert_eq!(
      contract.get_arguments(dispute_id),
      vec![
        Reason {
          side: Side::ProjectOwner,
          argument_timestamp: U64(1),
          description: "The first argument".to_string()
        },
        Reason {
          side: Side::Claimant,
          argument_timestamp: U64(2),
          description: "The second argument".to_string()
        },
      ],
    );
  }

  #[test]
  #[should_panic(expected = "You do not have access rights to perform this action")]
  fn test_provide_arguments_by_other_user() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);

    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .block_timestamp(0)
      .build());
    let dispute_id = create_dispute(
      &mut context,
      &mut contract,
      1,
      "Test description".to_string(),
      accounts(1),
      None,
      accounts(2)
    );

    testing_env!(context
      .predecessor_account_id(accounts(3))
      .block_timestamp(1)
      .attached_deposit(1)
      .build());
    contract.provide_arguments(dispute_id, "The test argument".to_string());
  }

  #[test]
  #[should_panic(expected = "This action can be performed only for a dispute with the status 'New'")]
  fn test_provide_arguments_with_incorrect_status() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);

    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());
    let dispute_id = create_dispute(
      &mut context,
      &mut contract,
      1,
      "Test description".to_string(),
      accounts(1),
      None,
      accounts(2)
    );

    change_status(&mut contract, dispute_id, DisputeStatus::DecisionPending);
    testing_env!(context
      .predecessor_account_id(accounts(2))
      .attached_deposit(1)
      .build());
    contract.provide_arguments(dispute_id, "The test argument".to_string());
  }

  #[test]
  #[should_panic(expected = "The period for providing arguments has expired, now you need to perform an 'escalation' action or cancel the dispute")]
  fn test_provide_arguments_with_expired_period_of_argumentation() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut config = Config::default();
    config.argument_period = U64(10);
    contract.change_config(config.clone());

    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .block_timestamp(0)
      .build());
    let dispute_id = create_dispute(
      &mut context,
      &mut contract,
      1,
      "Test description".to_string(),
      accounts(1),
      None,
      accounts(2)
    );

    testing_env!(context
      .predecessor_account_id(accounts(2))
      .attached_deposit(1)
      .block_timestamp(20)
      .build());
    contract.provide_arguments(dispute_id, "The test argument".to_string());
  }

  #[test]
  #[should_panic(expected = "Dispute not found")]
  fn test_provide_arguments_with_dispute_id() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);

    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());
    create_dispute(
      &mut context,
      &mut contract,
      1,
      "Test description".to_string(),
      accounts(1),
      None,
      accounts(2)
    );

    testing_env!(context
      .predecessor_account_id(accounts(2))
      .attached_deposit(1)
      .build());
    contract.provide_arguments(1, "The test argument".to_string());
  }

  #[test]
  #[should_panic(expected = "This action can be performed only for a dispute with the status 'New'")]
  fn test_cancel_dispute_with_incorrect_status() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);

    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());
    let dispute_id = create_dispute(
      &mut context,
      &mut contract,
      1,
      "Test description".to_string(),
      accounts(1),
      None,
      accounts(2)
    );

    change_status(&mut contract, dispute_id, DisputeStatus::DecisionPending);
    testing_env!(context
      .predecessor_account_id(accounts(2))
      .attached_deposit(1)
      .build());
    contract.cancel_dispute(dispute_id);
  }

  #[test]
  #[should_panic(expected = "You do not have access rights to perform this action")]
  fn test_cancel_dispute_by_other_user() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);

    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());
    let dispute_id = create_dispute(
      &mut context,
      &mut contract,
      1,
      "Test description".to_string(),
      accounts(1),
      None,
      accounts(2)
    );

    testing_env!(context
      .predecessor_account_id(accounts(3))
      .attached_deposit(1)
      .build());
    contract.cancel_dispute(dispute_id);
  }

  #[test]
  #[should_panic(expected = "This action can be performed only for a dispute with the status 'New'")]
  fn test_escalation_with_incorrect_status() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut config = Config::default();
    config.argument_period = U64(10);
    contract.change_config(config.clone());

    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .block_timestamp(0)
      .build());
    let dispute_id = create_dispute(
      &mut context,
      &mut contract,
      1,
      "Test description".to_string(),
      accounts(1),
      None,
      accounts(2)
    );

    change_status(&mut contract, dispute_id, DisputeStatus::DecisionPending);
    testing_env!(context
      .predecessor_account_id(accounts(3))
      .attached_deposit(1)
      .block_timestamp(20)
      .build());
    contract.escalation(dispute_id);
  }

  #[test]
  #[should_panic(expected = "Escalation of the dispute is possible only after the end of the argumentation period")]
  fn test_escalation_with_not_expired_period_of_argumentation() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);

    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());
    let dispute_id = create_dispute(
      &mut context,
      &mut contract,
      1,
      "Test description".to_string(),
      accounts(1),
      None,
      accounts(2)
    );

    testing_env!(context
      .predecessor_account_id(accounts(3))
      .attached_deposit(1)
      .build());
    contract.escalation(dispute_id);
  }

  #[test]
  #[should_panic(expected = "This method can only invoke a dispute resolution DAO contract")]
  fn test_result_of_dispute_by_other_user() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);

    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());
    let dispute_id = create_dispute(
      &mut context,
      &mut contract,
      1,
      "Test description".to_string(),
      accounts(1),
      None,
      accounts(2)
    );

    escalation(&mut contract, dispute_id);
    testing_env!(context
      .predecessor_account_id(accounts(3))
      .attached_deposit(1)
      .build());
    contract.result_of_dispute(dispute_id, true);
  }

  #[test]
  #[should_panic(expected = "This action can be performed only for a dispute with the status 'DecisionPending'")]
  fn test_result_of_dispute_with_incorrect_status() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);

    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());
    let dispute_id = create_dispute(
      &mut context,
      &mut contract,
      1,
      "Test description".to_string(),
      accounts(1),
      None,
      accounts(2)
    );

    testing_env!(context
      .predecessor_account_id(get_dispute_dao())
      .attached_deposit(1)
      .build());
    contract.result_of_dispute(dispute_id, true);
  }

  #[test]
  #[should_panic(expected = "The decision period is over, now you need to perform the 'finalize' action")]
  fn test_result_of_dispute_with_expired_period_of_decision() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut config = Config::default();
    config.decision_period = U64(10);
    contract.change_config(config.clone());

    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());
    let dispute_id = create_dispute(
      &mut context,
      &mut contract,
      1,
      "Test description".to_string(),
      accounts(1),
      None,
      accounts(2)
    );

    escalation(&mut contract, dispute_id);
    testing_env!(context
      .predecessor_account_id(get_dispute_dao())
      .attached_deposit(1)
      .block_timestamp(20)
      .build());
    contract.result_of_dispute(dispute_id, true);
  }

  #[test]
  #[should_panic(expected = "This action can be performed only for a dispute with the status 'DecisionPending'")]
  fn test_finalize_dispute_with_incorrect_status() {
    let mut context = VMContextBuilder::new();
    let mut contract = DisputesContract::new(
      get_bounties_contract(),
      get_dispute_dao(),
      vec![accounts(0).into()],
      None,
    );
    contract.set_status(ContractStatus::Live);

    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());
    let dispute_id = create_dispute(
      &mut context,
      &mut contract,
      1,
      "Test description".to_string(),
      accounts(1),
      None,
      accounts(2)
    );

    testing_env!(context
      .predecessor_account_id(accounts(3))
      .attached_deposit(1)
      .build());
    contract.finalize_dispute(dispute_id);
  }
}
