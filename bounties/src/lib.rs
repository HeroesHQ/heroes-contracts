use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedSet};
use near_sdk::json_types::{Base64VecU8, U128, U64};
use near_sdk::serde_json::json;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{assert_one_yocto, env, is_promise_success, log, near_bindgen, serde_json, AccountId,
               Balance, Gas, ONE_YOCTO, PanicOnDefault, Promise, PromiseError, PromiseOrValue};

pub use crate::types::*;

pub mod callbacks;
pub mod internal;
pub mod receiver;
pub mod types;
pub mod view;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct BountiesContract {
  /// Token types allowed for use
  pub token_account_ids: UnorderedSet<AccountId>,

  /// Last available id for the bounty.
  pub last_bounty_id: BountyIndex,

  /// Bounties map from ID to bounty information.
  pub bounties: LookupMap<BountyIndex, Bounty>,

  /// Bounty indexes map per owner account.
  pub account_bounties: LookupMap<AccountId, Vec<BountyIndex>>,

  /// Bounty claimers map per user. Allows quickly to query for each users their claims.
  pub bounty_claimers: LookupMap<AccountId, Vec<BountyClaim>>,

  /// Bounty claimer accounts map per bounty ID.
  pub bounty_claimer_accounts: LookupMap<BountyIndex, Vec<AccountId>>,

  /// Amount of $NEAR locked for bonds.
  pub locked_amount: Balance,

  /// account ids that can perform all actions:
  /// - manage admin_whitelist
  /// - add new ft-token types
  /// - change contract config
  /// - etc.
  pub admin_whitelist: UnorderedSet<AccountId>,

  /// Bounty contract configuration.
  pub config: Config,

  /// Reputation contract (optional)
  pub reputation_contract: Option<AccountId>,

  /// Dispute contract (optional)
  pub dispute_contract: Option<AccountId>,
}

#[near_bindgen]
impl BountiesContract {
  #[init]
  pub fn new(
    token_account_ids: Vec<AccountId>,
    admin_whitelist: Vec<AccountId>,
    config: Option<Config>,
    reputation_contract: Option<AccountId>,
    dispute_contract: Option<AccountId>,
  ) -> Self {
    assert!(
      admin_whitelist.len() > 0,
      "Admin whitelist must contain at least one account"
    );
    let mut token_account_ids_set = UnorderedSet::new(StorageKey::TokenAccountIds);
    token_account_ids_set.extend(token_account_ids.into_iter().map(|a| a.into()));
    let mut admin_whitelist_set = UnorderedSet::new(StorageKey::AdminWhitelist);
    admin_whitelist_set.extend(admin_whitelist.into_iter().map(|a| a.into()));

    Self {
      token_account_ids: token_account_ids_set,
      last_bounty_id: 0,
      bounties: LookupMap::new(StorageKey::Bounties),
      account_bounties: LookupMap::new(StorageKey::AccountBounties),
      bounty_claimers: LookupMap::new(StorageKey::BountyClaimers),
      bounty_claimer_accounts: LookupMap::new(StorageKey::BountyClaimerAccounts),
      locked_amount: 0,
      admin_whitelist: admin_whitelist_set,
      config: config.unwrap_or_default(),
      reputation_contract,
      dispute_contract,
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
  pub fn add_token_id(&mut self, token_id: AccountId) {
    assert_one_yocto();
    self.assert_admin_whitelist(&env::predecessor_account_id());
    // TODO: Check if the account contains a ft contract
    self.token_account_ids.insert(&token_id);
  }

  #[payable]
  pub fn change_config(&mut self, config: Config) {
    assert_one_yocto();
    self.assert_admin_whitelist(&env::predecessor_account_id());
    self.config = config;
  }

  /// Claim given bounty by caller with given expected duration to execute.
  /// Bond must be attached to the claim.
  #[payable]
  pub fn bounty_claim(&mut self, id: BountyIndex, deadline: U64) {
    let mut bounty = self.get_bounty(id.clone());

    assert_eq!(
      env::attached_deposit(),
      self.config.bounty_claim_bond.0,
      "Bounty wrong bond"
    );
    assert!(
      matches!(bounty.status, BountyStatus::New),
      "Bounty status does not allow to submit a claim"
    );
    assert!(
      deadline.0 <= bounty.max_deadline.0,
      "Bounty wrong deadline"
    );

    bounty.status = BountyStatus::Claimed;
    self.bounties.insert(&id, &bounty);

    let sender_id = env::predecessor_account_id();
    let mut claims = self.bounty_claimers.get(&sender_id).unwrap_or_default();
    let bounty_claim = BountyClaim {
      bounty_id: id,
      start_time: U64::from(env::block_timestamp()),
      deadline,
      status: ClaimStatus::New,
      proposal_id: None,
      rejected_timestamp: None,
    };
    self.internal_add_claim(id, claims.as_mut(), bounty_claim);
    self.internal_save_claims(&sender_id, &claims);
    self.internal_add_bounty_claimer_account(id, sender_id.clone());

    self.locked_amount += env::attached_deposit();
  }

  /// Report that bounty is done. Creates a proposal to vote for paying out the bounty,
  /// if validators DAO are assigned.
  /// Only creator of the claim can call `done`.
  #[payable]
  pub fn bounty_done(&mut self, id: BountyIndex, description: String) -> PromiseOrValue<()> {
    assert_one_yocto();

    let mut bounty = self.get_bounty(id.clone());
    assert!(
      matches!(bounty.status, BountyStatus::Claimed),
      "Bounty status does not allow to completion"
    );

    let sender_id = &env::predecessor_account_id();
    let (mut claims, claim_idx) = self.internal_get_claims(id.clone(), sender_id);
    assert!(
      matches!(claims[claim_idx].status, ClaimStatus::New),
      "The claim status does not allow to complete the bounty"
    );

    if claims[claim_idx].is_claim_expired() {
      self.internal_set_claim_expiry_status(id, sender_id, &mut bounty, claim_idx, &mut claims);
      PromiseOrValue::Value(())

    } else {
      if bounty.validators_dao.is_some() {
        self.internal_add_proposal(id, sender_id, &mut bounty, claim_idx, &mut claims, description)
      } else {
        claims[claim_idx].status = ClaimStatus::Completed;
        self.internal_save_claims(sender_id, &claims);

        PromiseOrValue::Value(())
      }
    }
  }

  /// Give up working on the bounty.
  /// Only the claimant can call this method.
  #[payable]
  pub fn bounty_give_up(&mut self, id: u64) -> PromiseOrValue<()> {
    assert_one_yocto();

    let mut bounty = self.get_bounty(id.clone());
    assert!(
      matches!(bounty.status, BountyStatus::Claimed),
      "Bounty status does not allow to give up"
    );

    let sender_id = env::predecessor_account_id();
    let (mut claims, claim_idx) = self.internal_get_claims(id.clone(), &sender_id);
    assert!(
      matches!(claims[claim_idx].status, ClaimStatus::New),
      "The claim status does not allow to give up the bounty"
    );

    let result = if env::block_timestamp() - claims[claim_idx].start_time.0
      > self.config.bounty_forgiveness_period.0
    {
      // If user over the forgiveness period.
      PromiseOrValue::Value(())
    } else {
      // Within forgiveness period. Return bond.
      self.internal_return_bonds(&sender_id)
    };

    claims[claim_idx].status = ClaimStatus::Canceled;
    self.internal_save_claims(&sender_id, &claims);
    bounty.status = BountyStatus::New;
    self.bounties.insert(&id, &bounty);

    result
  }

  #[payable]
  pub fn bounty_action(&mut self, id: BountyIndex, action: BountyAction) -> PromiseOrValue<()> {
    assert_one_yocto();

    let mut bounty = self.get_bounty(id.clone());
    assert!(
      matches!(bounty.status, BountyStatus::Claimed),
      "Bounty status does not allow approval of the execution result"
    );

    let sender_id = env::predecessor_account_id();

    match action.clone() {
      BountyAction::ClaimApproved { receiver_id } |
      BountyAction::ClaimRejected { receiver_id } => {
        if bounty.validators_dao.is_some() {
          assert_eq!(
            bounty.validators_dao.clone().unwrap().account_id,
            sender_id,
            "This method can only call DAO validators"
          );
        } else {
          assert_eq!(
            bounty.owner,
            sender_id,
            "Only the owner of the bounty can call this method"
          );
        }

        let (mut claims, claim_idx) = self.internal_get_claims(id.clone(), &receiver_id);
        assert!(
          matches!(claims[claim_idx].status, ClaimStatus::Completed),
          "The claim status does not allow approval of the execution result"
        );

        let result = if matches!(action, BountyAction::ClaimApproved { .. }) {
          self.internal_bounty_payout(id, receiver_id, &mut bounty, claim_idx, &mut claims)
        } else {
          self.internal_reject_claim(id, receiver_id, &mut bounty, claim_idx, &mut claims);
          PromiseOrValue::Value(())
        };

        result
      }
      BountyAction::Finalize => {
        let (receiver_id, mut claims, claim_idx) = self.internal_find_active_claim(id.clone());

        let result = if matches!(claims[claim_idx].status, ClaimStatus::New) &&
          claims[claim_idx].is_claim_expired()
        {
          self.internal_set_claim_expiry_status(id, &receiver_id, &mut bounty, claim_idx, &mut claims);
          PromiseOrValue::Value(())
        }
        else if matches!(claims[claim_idx].status, ClaimStatus::Completed) &&
          bounty.validators_dao.is_some()
        {
          self.internal_get_proposal(id, receiver_id, &mut bounty, claim_idx, &mut claims)
        }
        else if matches!(claims[claim_idx].status, ClaimStatus::Rejected) &&
          self.is_deadline_for_opening_dispute_expired(&claims[claim_idx])
        {
          self.internal_reset_bounty_to_initial_state(id, &receiver_id, &mut bounty, claim_idx, &mut claims);
          PromiseOrValue::Value(())
        } else {
          // TODO: Finalization of the bounty during the dispute
          env::panic_str("This bounty is not subject to finalization")
        };

        result
      }
      BountyAction::CreateDispute => env::panic_str("Not yet implemented"),
    }
  }

  #[payable]
  pub fn update_validators_dao_params(&mut self, id: BountyIndex, dao_params: ValidatorsDaoParams) {
    assert_one_yocto();
    let mut bounty = self.get_bounty(id.clone());

    assert_eq!(
      bounty.owner,
      env::predecessor_account_id(),
      "Only the owner of the bounty can call this method"
    );
    assert!(
      bounty.validators_dao.is_some(),
      "DAO validators are not used for this bounty"
    );
    assert_eq!(
      bounty.validators_dao.unwrap().account_id,
      dao_params.account_id,
      "DAO account ID does not match"
    );

    bounty.validators_dao = Some(dao_params.to_validators_dao());
    self.bounties.insert(&id, &bounty);
  }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
  use near_sdk::test_utils::{accounts, VMContextBuilder};
  use near_sdk::json_types::{U128, U64};
  use near_sdk::{testing_env, AccountId, Balance};
  use crate::{BountiesContract, Bounty, BountyAction, BountyClaim, BountyIndex, BountyStatus,
              BountyType, ClaimStatus, Config, ValidatorsDao, ValidatorsDaoParams};

  pub const TOKEN_DECIMALS: u8 = 18;
  pub const MAX_DEADLINE: U64 = U64(1_000_000_000 * 60 * 60 * 24 * 7);

  fn get_token_id() -> AccountId {
    "token_id".parse().unwrap()
  }

  pub const fn d(value: Balance, decimals: u8) -> Balance {
    value * 10u128.pow(decimals as _)
  }

  fn add_bounty(
    contract: &mut BountiesContract,
    owner: &AccountId,
    validators_dao: Option<ValidatorsDao>
  ) -> BountyIndex {
    let bounty_index: BountyIndex = 0;
    contract.bounties.insert(&bounty_index, &Bounty {
      description: "test".to_string(),
      token: get_token_id(),
      amount: U128(d(2_000, TOKEN_DECIMALS)),
      bounty_type: BountyType::Other,
      max_deadline: MAX_DEADLINE,
      validators_dao,
      owner: owner.clone(),
      status: BountyStatus::New,
    });
    contract.account_bounties.insert(owner, &vec![bounty_index]);
    contract.last_bounty_id = bounty_index.clone() + 1;
    bounty_index
  }

  fn bounty_claim(
    context: &mut VMContextBuilder,
    contract: &mut BountiesContract,
    id: BountyIndex,
    claimer: &AccountId
  ) {
    let bond = Config::default().bounty_claim_bond.0;
    testing_env!(context
      .predecessor_account_id(claimer.clone())
      .attached_deposit(bond.clone())
      .block_timestamp(0)
      .build());
    contract.bounty_claim(id, U64(1_000_000_000 * 60 * 60 * 24 * 2));
  }

  fn bounty_done(
    context: &mut VMContextBuilder,
    contract: &mut BountiesContract,
    id: BountyIndex,
    claimer: &AccountId
  ) {
    testing_env!(context
      .predecessor_account_id(claimer.clone())
      .attached_deposit(1)
      .build());
    contract.bounty_done(id, "test description".to_string());
  }

  fn bounty_give_up(
    context: &mut VMContextBuilder,
    contract: &mut BountiesContract,
    id: BountyIndex,
    claimer: &AccountId,
    current_block_timestamp: u64
  ) {
    testing_env!(context
      .predecessor_account_id(claimer.clone())
      .attached_deposit(1)
      .block_timestamp(current_block_timestamp)
      .build());
    contract.bounty_give_up(id);
  }

  fn bounty_action(
    context: &mut VMContextBuilder,
    contract: &mut BountiesContract,
    id: BountyIndex,
    project_owner: &AccountId,
    action: BountyAction
  ) {
    testing_env!(context
      .predecessor_account_id(project_owner.clone())
      .attached_deposit(1)
      .build());
    contract.bounty_action(id, action);
  }

  #[test]
  #[should_panic(expected = "Admin whitelist must contain at least one account")]
  fn test_failed_to_init_contract() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.predecessor_account_id(accounts(0)).build());
    BountiesContract::new(
      vec![get_token_id()],
      vec![],
      None,
      None,
      None
    );
  }

  #[test]
  fn test_add_to_admin_whitelist() {
    let mut context = VMContextBuilder::new();
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0).into()],
      None,
      None,
      None
    );
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
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0).into()],
      None,
      None,
      None
    );
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
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0).into()],
      None,
      None,
      None
    );
    contract.add_to_admin_whitelist(None, None);
  }

  #[test]
  #[should_panic(expected = "Requires attached deposit of exactly 1 yoctoNEAR")]
  fn test_add_to_admin_whitelist_without_deposit() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.predecessor_account_id(accounts(0)).build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0).into()],
      None,
      None,
      None
    );
    contract.add_to_admin_whitelist(Some(accounts(1)), None);
  }

  #[test]
  #[should_panic(expected = "Not in admin whitelist")]
  fn test_add_to_admin_whitelist_by_other_user() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.predecessor_account_id(accounts(0)).build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0).into()],
      None,
      None,
      None
    );
    testing_env!(context.predecessor_account_id(accounts(1)).attached_deposit(1).build());
    contract.add_to_admin_whitelist(Some(accounts(1)), None);
  }

  #[test]
  #[should_panic(expected = "Cannot remove all accounts from admin whitelist")]
  fn test_remove_from_admin_whitelist_all_members() {
    let mut context = VMContextBuilder::new();
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0).into()],
      None,
      None,
      None
    );
    contract.remove_from_admin_whitelist(Some(accounts(0)), None);
  }

  #[test]
  fn test_add_token() {
    let mut context = VMContextBuilder::new();
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0).into()],
      None,
      None,
      None
    );

    contract.add_token_id("new_token".parse().unwrap());
    assert_eq!(
      contract.get_token_account_ids(),
      vec!["token_id".parse().unwrap(), "new_token".parse().unwrap()]
    );
  }

  #[test]
  fn test_change_config() {
    let mut context = VMContextBuilder::new();
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0).into()],
      None,
      None,
      None
    );

    let config = Config {
      bounty_claim_bond: U128::from(2_000_000_000_000_000_000_000_000),
      bounty_forgiveness_period: U64::from(1_000_000_000 * 60 * 60 * 24 * 7),
      period_for_opening_dispute: U64::from(1_000_000_000 * 60 * 60 * 24 * 20),
    };
    contract.change_config(config.clone());
    assert_eq!(contract.get_config(), config);
  }

  #[test]
  fn test_bounty_claim() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0)],
      None,
      None,
      None
    );
    let id = add_bounty(&mut contract, &accounts(1), None);

    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    assert_eq!(
      contract.bounty_claimers.get(&claimer).unwrap()[0],
      BountyClaim {
        bounty_id: id.clone(),
        start_time: U64(0),
        deadline: U64(1_000_000_000 * 60 * 60 * 24 * 2),
        status: ClaimStatus::New,
        proposal_id: None,
        rejected_timestamp: None,
      }
    );
    assert_eq!(contract.bounty_claimer_accounts.get(&id).unwrap()[0], claimer);
    assert_eq!(contract.locked_amount, Config::default().bounty_claim_bond.0);
    assert_eq!(contract.bounties.get(&id).unwrap().status, BountyStatus::Claimed);
  }

  #[test]
  #[should_panic(expected = "Bounty wrong bond")]
  fn test_bounty_claim_without_bond() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0)],
      None,
      None,
      None
    );
    let id = add_bounty(&mut contract, &accounts(1), None);

    testing_env!(context
      .predecessor_account_id(accounts(2))
      .build());
    contract.bounty_claim(id, U64(1_000_000_000 * 60 * 60 * 24 * 2));
  }

  #[test]
  #[should_panic(expected = "Bounty status does not allow to submit a claim")]
  fn test_bounty_claim_with_incorrect_status() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0)],
      None,
      None,
      None
    );
    let id = add_bounty(&mut contract, &accounts(1), None);
    bounty_claim(&mut context, &mut contract, id.clone(), &accounts(2));

    testing_env!(context
      .predecessor_account_id(accounts(3))
      .attached_deposit(Config::default().bounty_claim_bond.0)
      .build());
    contract.bounty_claim(id, U64(1_000_000_000 * 60 * 60 * 24 * 2));
  }

  #[test]
  #[should_panic(expected = "Bounty wrong deadline")]
  fn test_bounty_claim_with_incorrect_deadline() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0)],
      None,
      None,
      None
    );
    let id = add_bounty(&mut contract, &accounts(1), None);

    testing_env!(context
      .predecessor_account_id(accounts(2))
      .attached_deposit(Config::default().bounty_claim_bond.0)
      .build());
    contract.bounty_claim(id, U64(MAX_DEADLINE.0 + 1));
  }

  #[test]
  #[should_panic(expected = "Bounty not found")]
  fn test_claim_for_non_existent_bounty() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0)],
      None,
      None,
      None
    );
    let id = add_bounty(&mut contract, &accounts(1), None);

    testing_env!(context
      .predecessor_account_id(accounts(2))
      .attached_deposit(Config::default().bounty_claim_bond.0)
      .build());
    contract.bounty_claim(id + 1, MAX_DEADLINE);
  }

  #[test]
  fn test_bounty_done() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0)],
      None,
      None,
      None
    );
    let id = add_bounty(&mut contract, &accounts(1), None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);

    bounty_done(&mut context, &mut contract, id.clone(), &claimer);
    assert_eq!(contract.bounty_claimers.get(&claimer).unwrap()[0].status, ClaimStatus::Completed);
    assert_eq!(contract.bounties.get(&id).unwrap().status, BountyStatus::Claimed);
  }

  #[test]
  #[should_panic(expected = "Bounty status does not allow to completion")]
  fn test_bounty_done_with_incorrect_bounty_status() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0)],
      None,
      None,
      None
    );
    let id = add_bounty(&mut contract, &accounts(1), None);
    let claimer = accounts(2);

    bounty_done(&mut context, &mut contract, id.clone(), &claimer);
  }

  #[test]
  #[should_panic(expected = "The claim status does not allow to complete the bounty")]
  fn test_bounty_done_with_incorrect_claim_status() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0)],
      None,
      None,
      None
    );
    let id = add_bounty(&mut contract, &accounts(1), None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    bounty_done(&mut context, &mut contract, id.clone(), &claimer);

    bounty_done(&mut context, &mut contract, id.clone(), &claimer);
  }

  #[test]
  fn test_bounty_done_on_claim_that_has_expired() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0)],
      None,
      None,
      None
    );
    let id = add_bounty(&mut contract, &accounts(1), None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);

    testing_env!(context
      .predecessor_account_id(claimer.clone())
      .attached_deposit(1)
      .block_timestamp(1_000_000_000 * 60 * 60 * 24 * 2 + 1)
      .build());
    contract.bounty_done(id, "test description".to_string());
    assert_eq!(contract.bounty_claimers.get(&claimer).unwrap()[0].status, ClaimStatus::Expired);
    assert_eq!(contract.bounties.get(&id).unwrap().status, BountyStatus::New);
  }

  #[test]
  fn test_bounty_give_up() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0)],
      None,
      None,
      None
    );
    let id = add_bounty(&mut contract, &accounts(1), None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);

    let bounty_forgiveness_period = Config::default().bounty_forgiveness_period.0;
    bounty_give_up(
      &mut context,
      &mut contract,
      id.clone(),
      &claimer,
      bounty_forgiveness_period.clone() - 1
    );
    assert_eq!(contract.bounty_claimers.get(&claimer).unwrap()[0].status, ClaimStatus::Canceled);
    assert_eq!(contract.bounties.get(&id).unwrap().status, BountyStatus::New);
    assert_eq!(contract.locked_amount, 0);

    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);

    bounty_give_up(
      &mut context,
      &mut contract,
      id.clone(),
      &claimer,
      bounty_forgiveness_period.clone() + 1
    );
    assert_eq!(contract.bounty_claimers.get(&claimer).unwrap()[0].status, ClaimStatus::Canceled);
    assert_eq!(contract.bounties.get(&id).unwrap().status, BountyStatus::New);
    assert_eq!(contract.locked_amount, Config::default().bounty_claim_bond.0);
  }

  #[test]
  #[should_panic(expected = "Bounty status does not allow to give up")]
  fn test_bounty_give_up_with_incorrect_bounty_status() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0)],
      None,
      None,
      None
    );
    let id = add_bounty(&mut contract, &accounts(1), None);
    let claimer = accounts(2);

    bounty_give_up(
      &mut context,
      &mut contract,
      id.clone(),
      &claimer,
      0,
    );
  }

  #[test]
  #[should_panic(expected = "The claim status does not allow to give up the bounty")]
  fn test_bounty_give_up_with_incorrect_claim_status() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0)],
      None,
      None,
      None
    );
    let id = add_bounty(&mut contract, &accounts(1), None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    bounty_done(&mut context, &mut contract, id.clone(), &claimer);

    bounty_give_up(
      &mut context,
      &mut contract,
      id.clone(),
      &claimer,
      0,
    );
  }

  #[test]
  fn test_bounty_action() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0)],
      None,
      None,
      None
    );
    let project_owner = accounts(1);
    let id = add_bounty(&mut contract, &project_owner, None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    bounty_done(&mut context, &mut contract, id.clone(), &claimer);

    bounty_action(
      &mut context,
      &mut contract,
      id.clone(),
      &project_owner,
      BountyAction::ClaimRejected { receiver_id: claimer.clone() }
    );
    assert_eq!(contract.bounty_claimers.get(&claimer).unwrap()[0].status, ClaimStatus::NotCompleted);
    assert_eq!(contract.bounties.get(&id).unwrap().status, BountyStatus::New);
    assert_eq!(contract.locked_amount, 0);

    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    bounty_done(&mut context, &mut contract, id.clone(), &claimer);

    bounty_action(
      &mut context,
      &mut contract,
      id.clone(),
      &project_owner,
      BountyAction::ClaimApproved { receiver_id: claimer }
    );
    // For the unit test, the object statuses have not changed.
    // The action is performed in the promise callback function (see simulation test).
  }

  #[test]
  #[should_panic(expected = "This method can only call DAO validators")]
  fn test_bounty_action_by_other_user() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0)],
      None,
      None,
      None
    );
    let project_owner = accounts(1);
    let dao_params = ValidatorsDaoParams {
      account_id: "dao".parse().unwrap(),
      add_proposal_bond: U128(10u128.pow(24)),
      gas_for_add_proposal: None,
      gas_for_claim_approval: None,
    };
    let id = add_bounty(&mut contract, &project_owner, Some(dao_params.to_validators_dao()));
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    bounty_done(&mut context, &mut contract, id.clone(), &claimer);

    bounty_action(
      &mut context,
      &mut contract,
      id.clone(),
      &project_owner,
      BountyAction::ClaimApproved { receiver_id: claimer }
    );
  }

  #[test]
  #[should_panic(expected = "Only the owner of the bounty can call this method")]
  fn test_bounty_action_by_dao() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0)],
      None,
      None,
      None
    );
    let project_owner = accounts(1);
    let id = add_bounty(&mut contract, &project_owner, None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    bounty_done(&mut context, &mut contract, id.clone(), &claimer);

    testing_env!(context
      .predecessor_account_id("dao".parse().unwrap())
      .attached_deposit(1)
      .build());
    let action = BountyAction::ClaimApproved { receiver_id: claimer };
    contract.bounty_action(id, action);
  }

  #[test]
  fn test_update_validators_dao_params() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      vec![accounts(0)],
      None,
      None,
      None
    );
    let project_owner = accounts(1);
    let id = add_bounty(
      &mut contract,
      &project_owner,
      Some(
        ValidatorsDaoParams {
          account_id: "dao".parse().unwrap(),
          add_proposal_bond: U128(10u128.pow(24)),
          gas_for_add_proposal: None,
          gas_for_claim_approval: None,
        }
          .to_validators_dao()
      )
    );

    let new_dao_params = ValidatorsDaoParams {
      account_id: "dao".parse().unwrap(),
      add_proposal_bond: U128(2 * 10u128.pow(24)),
      gas_for_add_proposal: Some(U64(50_000_000_000_000)),
      gas_for_claim_approval: Some(U64(50_000_000_000_000)),
    };
    testing_env!(context
      .predecessor_account_id(project_owner)
      .attached_deposit(1)
      .build());
    contract.update_validators_dao_params(id, new_dao_params.clone());
    assert_eq!(
      contract.get_bounty(id.clone()).validators_dao.unwrap(),
      new_dao_params.to_validators_dao()
    );
  }
}
