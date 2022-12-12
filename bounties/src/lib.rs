use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedSet};
use near_sdk::json_types::{U128, U64};
use near_sdk::serde_json::json;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{assert_one_yocto, env, is_promise_success, log, near_bindgen, serde_json, AccountId,
               Balance, Gas, PanicOnDefault, Promise, PromiseOrValue};

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
    let mut admin_whitelist_set = UnorderedSet::new(StorageKey::DepositWhitelist);
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

    if env::block_timestamp() > claims[claim_idx].start_time.0 + claims[claim_idx].deadline.0 {
      // Expired
      claims[claim_idx].status = ClaimStatus::Expired;
      self.internal_save_claims(sender_id, &claims);
      bounty.status = BountyStatus::New;
      self.bounties.insert(&id, &bounty);

      PromiseOrValue::Value(())

    } else {
      if bounty.validators_dao.is_some() {
        let dao = bounty.validators_dao.clone().unwrap();
        Promise::new(dao.account_id)
          .function_call(
            "add_proposal".to_string(),
            json!({
              "proposal": {
                "description": description,
                "kind": {
                  "FunctionCall" : {
                    "receiver_id": env::current_account_id(),
                    "actions": [
                      {
                        "method_name": "bounty_action",
                        "args": json!({
                          "id": id.to_string(),
                          "action": {
                            "ClaimApproved": {
                              "receiver_id": sender_id.to_string(),
                            }
                          }
                        })
                          .to_string()
                          .into_bytes(),
                        "deposit": "0",
                        "gas": dao.gas_for_claim_approval,
                      }
                    ],
                  }
                }
              }
            })
              .to_string()
              .into_bytes(),
            dao.add_proposal_bond.0,
            Gas(dao.gas_for_add_proposal.0),
          )
          .then(
            Self::ext(env::current_account_id())
              .on_added_proposal_callback(id.clone(), sender_id)
          ).into()

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
      self.internal_return_bonds(sender_id.clone())
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
          // TODO: call ft_transfer
          claims[claim_idx].status = ClaimStatus::Approved;
          self.internal_save_claims(&receiver_id, &claims);
          bounty.status = BountyStatus::Completed;
          self.bounties.insert(&id, &bounty);
          // TODO: update reputation
          self.internal_return_bonds(sender_id.clone())

        } else {
          claims[claim_idx].status = ClaimStatus::Rejected;
          self.internal_save_claims(&receiver_id, &claims);

          if self.dispute_contract.is_none() {
            // If the creation of a dispute is not foreseen,
            // then the bounty reset to initial state
            bounty.status = BountyStatus::New;
            self.bounties.insert(&id, &bounty);
            // TODO: update reputation
            self.internal_return_bonds(sender_id.clone())
          } else {
            // You need to wait until the recipient creates a dispute,
            // or the deadline for creating a dispute expires
            PromiseOrValue::Value(())
          }
        };

        result
      }
      BountyAction::CreateDispute => env::panic_str("Not yet implemented"),
      BountyAction::Finalize => env::panic_str("Not yet implemented"),
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
