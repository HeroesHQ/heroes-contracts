use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap, UnorderedSet};
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
  pub tokens: UnorderedMap<AccountId, TokenDetails>,

  /// Last available id for the bounty.
  pub last_bounty_id: BountyIndex,

  /// Bounties map from ID to bounty information.
  pub bounties: LookupMap<BountyIndex, VersionedBounty>,

  /// Bounty indexes map per owner account.
  pub account_bounties: LookupMap<AccountId, Vec<BountyIndex>>,

  /// Bounty claimers map per user. Allows quickly to query for each users their claims.
  pub bounty_claimers: LookupMap<AccountId, Vec<VersionedBountyClaim>>,

  /// Bounty claimer accounts map per bounty ID.
  pub bounty_claimer_accounts: LookupMap<BountyIndex, Vec<AccountId>>,

  /// Amount of $NEAR locked for bonds.
  pub locked_amount: Balance,

  /// account ids that can perform all actions:
  /// - manage admins_whitelist
  /// - add new ft-token types
  /// - change contract config
  /// - etc.
  pub admins_whitelist: UnorderedSet<AccountId>,

  /// Bounty contract configuration.
  pub config: Config,

  /// Reputation contract (optional)
  pub reputation_contract: Option<AccountId>,

  /// Dispute contract (optional)
  pub dispute_contract: Option<AccountId>,

  /// KYC whitelist contract (optional)
  pub kyc_whitelist_contract: Option<AccountId>,

  /// Whitelist accounts for which claims do not require approval.
  /// Map of whitelists for each bounty owner account.
  pub claimers_whitelist: LookupMap<AccountId, Vec<AccountId>>,

  /// Total platform fees map per token ID
  pub total_fees: LookupMap<AccountId, FeeStats>,

  /// Total validators DAO fees per DAO account ID
  pub total_validators_dao_fees: LookupMap<AccountId, Vec<DaoFeeStats>>,

  /// Recipient of platform fee (optional)
  pub recipient_of_platform_fee: Option<AccountId>,
}

#[near_bindgen]
impl BountiesContract {
  #[init]
  pub fn new(
    admins_whitelist: Vec<AccountId>,
    config: Option<Config>,
    reputation_contract: Option<AccountId>,
    dispute_contract: Option<AccountId>,
    kyc_whitelist_contract: Option<AccountId>,
    recipient_of_platform_fee: Option<AccountId>,
  ) -> Self {
    assert!(
      admins_whitelist.len() > 0,
      "Admin whitelist must contain at least one account"
    );
    let mut admins_whitelist_set = UnorderedSet::new(StorageKey::AdminWhitelist);
    admins_whitelist_set.extend(admins_whitelist.into_iter().map(|a| a.into()));

    Self {
      tokens: UnorderedMap::new(StorageKey::Tokens),
      last_bounty_id: 0,
      bounties: LookupMap::new(StorageKey::Bounties),
      account_bounties: LookupMap::new(StorageKey::AccountBounties),
      bounty_claimers: LookupMap::new(StorageKey::BountyClaimers),
      bounty_claimer_accounts: LookupMap::new(StorageKey::BountyClaimerAccounts),
      locked_amount: 0,
      admins_whitelist: admins_whitelist_set,
      config: config.unwrap_or_default(),
      reputation_contract,
      dispute_contract,
      kyc_whitelist_contract,
      claimers_whitelist: LookupMap::new(StorageKey::ClaimersWhitelist),
      total_fees: LookupMap::new(StorageKey::TotalFees),
      total_validators_dao_fees: LookupMap::new(StorageKey::TotalValidatorsDaoFees),
      recipient_of_platform_fee,
    }
  }

  #[payable]
  pub fn add_to_admins_whitelist(
    &mut self,
    account_id: Option<AccountId>,
    account_ids: Option<Vec<AccountId>>,
  ) {
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    let account_ids = if let Some(account_ids) = account_ids {
      account_ids
    } else {
      vec![account_id.expect("Expected either account_id or account_ids")]
    };
    for account_id in &account_ids {
      self.admins_whitelist.insert(account_id);
    }
  }

  #[payable]
  pub fn remove_from_admins_whitelist(
    &mut self,
    account_id: Option<AccountId>,
    account_ids: Option<Vec<AccountId>>,
  ) {
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    let account_ids = if let Some(account_ids) = account_ids {
      account_ids
    } else {
      vec![account_id.expect("Expected either account_id or account_ids")]
    };
    for account_id in &account_ids {
      self.admins_whitelist.remove(&account_id);
    }
    assert!(
      !self.admins_whitelist.is_empty(),
      "Cannot remove all accounts from admin whitelist",
    );
  }

  #[payable]
  pub fn add_to_claimers_whitelist(
    &mut self,
    claimer: AccountId,
  ) {
    assert_one_yocto();
    let owner = env::predecessor_account_id();
    self.account_bounties.get(&owner).expect("No bounties found");
    let mut whitelist = self.claimers_whitelist.get(&owner).unwrap_or_default();
    if !whitelist.contains(&claimer) {
      whitelist.push(claimer);
      self.claimers_whitelist.insert(&owner, &whitelist);
    }
  }

  #[payable]
  pub fn remove_from_claimers_whitelist(
    &mut self,
    claimer: AccountId,
  ) {
    assert_one_yocto();
    let owner = env::predecessor_account_id();
    let mut whitelist = self.claimers_whitelist.get(&owner).expect("Not a bounty owner");
    let index = whitelist.iter().position(|c| c.clone() == claimer);
    if index.is_some() {
      whitelist.remove(index.unwrap());
      if whitelist.is_empty() {
        self.claimers_whitelist.remove(&owner);
      } else {
        self.claimers_whitelist.insert(&owner, &whitelist);
      }
    }
  }

  #[payable]
  pub fn add_token_id(
    &mut self,
    token_id: AccountId,
    min_amount_for_kyc: Option<U128>
  ) -> PromiseOrValue<()> {
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    assert!(
      self.tokens.get(&token_id).is_none(),
      "The token already exists"
    );
    self.internal_get_ft_metadata(token_id, min_amount_for_kyc)
  }

  #[payable]
  pub fn update_token(&mut self, token_id: AccountId, token_details: TokenDetails) {
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    assert!(
      self.tokens.get(&token_id).is_some(),
      "No token found"
    );
    self.tokens.insert(&token_id, &token_details);
  }

  #[payable]
  pub fn update_kyc_whitelist_contract(&mut self, kyc_whitelist_contract: Option<AccountId>) {
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    self.kyc_whitelist_contract = kyc_whitelist_contract;
  }

  #[payable]
  pub fn change_config(&mut self, config_create: ConfigCreate) {
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    self.config = config_create.to_config(self.config.clone());
  }

  #[payable]
  pub fn update_configuration_dictionary_entries(
    &mut self,
    dict: ReferenceType,
    entry: Option<String>,
    entries: Option<Vec<String>>
  ) {
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    let (reference, entries) = self.get_configuration_dictionary(dict, entry, entries);

    for entry in entries {
      reference.push(entry);
    }
  }

  #[payable]
  pub fn remove_configuration_dictionary_entries(
    &mut self,
    dict: ReferenceType,
    entry: Option<String>,
    entries: Option<Vec<String>>
  ) {
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    let (reference, entries) = self.get_configuration_dictionary(dict, entry, entries);

    for entry in entries {
      let index = reference.iter().position(|e| e.clone() == entry);
      if index.is_some() {
        reference.remove(index.unwrap());
      }
    }
  }

  #[payable]
  pub fn change_recipient_of_platform_fee(&mut self, recipient_of_platform_fee: AccountId) {
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    self.recipient_of_platform_fee = Some(recipient_of_platform_fee);
  }

  /// Claim given bounty by caller with given expected duration to execute.
  /// Bond must be attached to the claim.
  #[payable]
  pub fn bounty_claim(
    &mut self,
    id: BountyIndex,
    deadline: U64,
    description: String
  ) -> PromiseOrValue<()> {
    let bounty = self.get_bounty(id.clone());

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
      bounty.is_claim_deadline_correct(deadline),
      "Bounty wrong deadline"
    );
    assert!(
      !description.is_empty(),
      "The description cannot be empty"
    );

    let sender_id = env::predecessor_account_id();
    bounty.assert_account_is_not_owner_or_reviewer(sender_id.clone());

    let mut claims = self.get_bounty_claims(sender_id.clone());
    let index = Self::internal_find_claim(id, &claims);
    assert!(
      index.is_none() || !matches!(claims[index.unwrap()].status, ClaimStatus::New),
      "You already have a claim with the status 'New'"
    );

    let token_details = self.tokens.get(&bounty.token).unwrap();
    if self.kyc_whitelist_contract.is_some() &&
      (
        token_details.min_amount_for_kyc.is_none() ||
          bounty.amount.0 >= token_details.min_amount_for_kyc.unwrap().0
      )
    {
      self.is_claimer_in_kyc_whitelist(id, bounty, &mut claims, sender_id, deadline, description)

    } else if bounty.is_validators_dao_used() {
      Self::internal_add_proposal_to_approve_claimer(
        id,
        bounty,
        &mut claims,
        sender_id,
        deadline,
        description
      )

    } else {
      self.internal_create_claim(id, bounty, &mut claims, sender_id, deadline, description, None);
      PromiseOrValue::Value(())
    }
  }

  #[payable]
  pub fn decision_on_claim(&mut self, id: BountyIndex, claimer: AccountId, approve: bool) {
    assert_one_yocto();
    let bounty = self.get_bounty(id.clone());
    assert!(
      matches!(bounty.status, BountyStatus::New),
      "Bounty status does not allow to make a decision on a claim"
    );
    bounty.check_access_rights();
    let (mut claims, claim_idx) = self.internal_get_claims(id, &claimer);
    let mut bounty_claim= claims[claim_idx].clone();
    assert!(
      matches!(bounty_claim.status, ClaimStatus::New),
      "Claim status does not allow a decision to be made"
    );
    if approve {
      self.internal_claimer_approval(id, bounty, &mut bounty_claim);
    } else {
      bounty_claim.status = ClaimStatus::NotHired;
      self.internal_return_bonds(&claimer);
    }
    claims.insert(claim_idx, bounty_claim);
    self.internal_save_claims(&claimer, &claims);
  }

  /// Report that bounty is done. Creates a proposal to vote for paying out the bounty,
  /// if validators DAO are assigned.
  /// Only creator of the claim can call `done`.
  #[payable]
  pub fn bounty_done(&mut self, id: BountyIndex, description: String) -> PromiseOrValue<()> {
    assert_one_yocto();

    let bounty = self.get_bounty(id.clone());
    assert!(
      matches!(bounty.status, BountyStatus::Claimed),
      "Bounty status does not allow to completion"
    );

    let sender_id = &env::predecessor_account_id();
    let (mut claims, claim_idx) = self.internal_get_claims(id.clone(), sender_id);
    assert!(
      matches!(claims[claim_idx].status, ClaimStatus::InProgress),
      "The claim status does not allow to complete the bounty"
    );

    if claims[claim_idx].is_claim_expired() {
      self.internal_set_claim_expiry_status(id, sender_id, bounty, claim_idx, &mut claims);
      PromiseOrValue::Value(())

    } else {
      if bounty.is_validators_dao_used() {
        Self::internal_add_proposal_to_finish_claim(
          id,
          sender_id,
          &bounty,
          claim_idx,
          &mut claims,
          description
        )
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
  pub fn bounty_give_up(&mut self, id: BountyIndex) -> PromiseOrValue<()> {
    assert_one_yocto();

    let bounty = self.get_bounty(id.clone());
    let sender_id = env::predecessor_account_id();
    let (mut claims, claim_idx) = self.internal_get_claims(id.clone(), &sender_id);
    assert!(
      matches!(claims[claim_idx].status, ClaimStatus::New)
        || matches!(claims[claim_idx].status, ClaimStatus::InProgress),
      "The claim status does not allow to give up the bounty"
    );

    let was_status_in_progress = matches!(claims[claim_idx].status, ClaimStatus::InProgress);
    let result = if was_status_in_progress &&
      env::block_timestamp() - claims[claim_idx].start_time.unwrap().0 >
        self.config.bounty_forgiveness_period.0
    {
      // If user over the forgiveness period.
      PromiseOrValue::Value(())
    } else {
      // Within forgiveness period. Return bond.
      self.internal_return_bonds(&sender_id)
    };

    claims[claim_idx].status = ClaimStatus::Canceled;
    self.internal_save_claims(&sender_id, &claims);
    if was_status_in_progress && matches!(bounty.status, BountyStatus::Claimed) {
      self.internal_change_status_and_save_bounty(&id, bounty, BountyStatus::New);
    }
    self.internal_update_statistic(
      Some(sender_id),
      None,
      ReputationActionKind::ClaimCancelled
    );

    result
  }

  /// Cancel the bounty and return the funds to the owner.
  /// Only the owner of the bounty can call this method.
  #[payable]
  pub fn bounty_cancel(&mut self, id: BountyIndex) -> PromiseOrValue<()> {
    assert_one_yocto();

    let bounty = self.get_bounty(id.clone());
    assert!(
      matches!(bounty.status, BountyStatus::New),
      "Bounty status does not allow cancellation"
    );

    let sender_id = env::predecessor_account_id();
    assert_eq!(bounty.owner, sender_id, "Only the owner of the bounty can call this method");

    self.internal_refund_bounty_amount(id, bounty)
  }

  #[payable]
  pub fn bounty_action(&mut self, id: BountyIndex, action: BountyAction) -> PromiseOrValue<()> {
    assert_one_yocto();
    let mut bounty = self.get_bounty(id.clone());

    if !action.need_to_finalize_claim() {
      assert!(
        matches!(bounty.status, BountyStatus::Claimed),
        "Bounty status does not allow approval of the execution result"
      );

      match action.clone() {
        BountyAction::ClaimApproved { receiver_id } |
        BountyAction::ClaimRejected { receiver_id } => {
          bounty.check_access_rights();

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
        BountyAction::Finalize { .. } => {
          let (receiver_id, mut claims, claim_idx) = self.internal_find_active_claim(id.clone());

          let result = if matches!(claims[claim_idx].status, ClaimStatus::InProgress) &&
            claims[claim_idx].is_claim_expired()
          {
            self.internal_set_claim_expiry_status(id, &receiver_id, bounty, claim_idx, &mut claims);
            PromiseOrValue::Value(())
          }

          else if matches!(claims[claim_idx].status, ClaimStatus::Completed) &&
            bounty.is_validators_dao_used()
          {
            Self::internal_check_bounty_payout_proposal(
              id,
              receiver_id,
              &mut bounty,
              claim_idx,
              &mut claims
            )
          }

          else if matches!(claims[claim_idx].status, ClaimStatus::Rejected) &&
            self.is_deadline_for_opening_dispute_expired(&claims[claim_idx])
          {
            self.internal_reset_bounty_to_initial_state(id, &receiver_id, &mut bounty, claim_idx, &mut claims);
            PromiseOrValue::Value(())
          }

          else if matches!(claims[claim_idx].status, ClaimStatus::Disputed) {
            self.internal_get_dispute(id, receiver_id, &mut bounty, claim_idx, &mut claims)
          }

          else {
            env::panic_str("This bounty is not subject to finalization");
          };

          result
        }
      }
    } else {
      let receiver_id = action.get_finalize_action_receiver().unwrap();
      let (mut claims, claim_idx) = self.internal_get_claims(id.clone(), &receiver_id);

      if matches!(claims[claim_idx].status, ClaimStatus::New) &&
        bounty.is_validators_dao_used()
      {
        return Self::internal_check_approve_claimer_proposal(
          id,
          receiver_id,
          bounty,
          claim_idx,
          &mut claims
        )
      }

      env::panic_str("This bounty claim is not subject to finalization");
    }
  }

  #[payable]
  pub fn bounty_update(&mut self, id: BountyIndex, bounty_update: BountyUpdate) {
    assert_one_yocto();

    let mut bounty = self.get_bounty(id.clone());
    assert!(
      matches!(bounty.status, BountyStatus::New),
      "Bounty status does not allow updating"
    );

    let sender_id = env::predecessor_account_id();
    assert_eq!(bounty.owner, sender_id, "Only the owner of the bounty can call this method");

    let claims = self.get_bounty_claims_by_id(id.clone());
    let found_claim = claims
      .into_iter()
      .find(|c| matches!(c.1.status, ClaimStatus::New))
      .is_some();
    let mut changed = false;

    if bounty_update.metadata.is_some() {
      assert!(
        !found_claim,
        "Metadata cannot be changed if there are already claims"
      );
      bounty.metadata = bounty_update.metadata.unwrap();
      changed = true;
    }
    if bounty_update.claimer_approval.is_some() {
      assert!(
        !found_claim,
        "The approval setting cannot be changed if there are already claims"
      );
      bounty.claimer_approval = bounty_update.claimer_approval.unwrap();
      changed = true;
    }
    if bounty_update.deadline.is_some() {
      if found_claim {
        assert_ne!(
          bounty.deadline.get_deadline_type(),
          3,
          "The deadline cannot be limited if there are already claims"
        );
        if bounty_update.deadline.clone().unwrap().get_deadline_type() != 3 {
          assert_eq!(
            bounty.deadline.clone().get_deadline_type(),
            bounty_update.deadline.clone().unwrap().get_deadline_type(),
            "The type of deadline cannot be changed if there are already claims"
          );
          assert!(
            bounty_update.deadline.clone().unwrap().get_deadline_value().0 >
              bounty.deadline.clone().get_deadline_value().0,
            "The deadline can be increased only in the upward direction"
          );
        }
      }
      bounty.deadline = bounty_update.deadline.unwrap();
      changed = true;
    }
    if bounty_update.reviewers.is_some() {
      assert!(
        bounty.reviewers.is_none() ||
          match bounty.reviewers.clone().unwrap() {
            Reviewers::MoreReviewers { .. } => true,
            _ => false
          },
        "Validators DAO settings cannot be changed"
      );
      assert!(
        match bounty_update.reviewers.clone().unwrap() {
          ReviewersParams::MoreReviewers { .. } => true,
          _ => false
        },
        "It is not possible to start using Validators DAO after creating a bounty"
      );
      let more_reviewers = bounty_update.reviewers.clone().unwrap().get_more_reviewers();
      if bounty.reviewers.is_some() {
        let prev_more_reviewers = bounty.reviewers.clone().unwrap().get_more_reviewers();
        assert_ne!(
          more_reviewers,
          prev_more_reviewers,
          "The list of reviewers has not changed"
        );
      }
      bounty.reviewers = if more_reviewers.len() > 0 {
        Some(bounty_update.reviewers.unwrap().to_reviewers())
      } else {
        None
      };
      changed = true;
    }

    assert!(changed, "No changes found");
    self.bounties.insert(&id, &bounty.into());
  }

  #[payable]
  pub fn withdraw_platform_fee(&mut self, token_id: AccountId) -> PromiseOrValue<()> {
    assert_one_yocto();
    assert_eq!(
      self.recipient_of_platform_fee
        .clone().expect("The recipient of the platform fee is not specified"),
      env::predecessor_account_id(),
      "This account does not have permission to perform this action"
    );
    let balance = self.internal_get_unlocked_platform_fee_amount(token_id.clone());
    assert!(balance.0 > 0, "The available balance of commission is zero");

    Self::internal_fees_payout(
      token_id,
      balance,
      env::predecessor_account_id(),
      "Bounties platform fee transfer",
      true,
    )
  }

  #[payable]
  pub fn withdraw_validators_dao_fee(&mut self, token_id: AccountId) -> PromiseOrValue<()> {
    assert_one_yocto();
    let dao_account_id = env::predecessor_account_id();
    let balance = self.internal_get_unlocked_validators_dao_fee_amount(
      dao_account_id.clone(),
      token_id.clone()
    );
    assert!(balance.0 > 0, "The available balance of commission is zero");

    Self::internal_fees_payout(
      token_id,
      balance,
      dao_account_id,
      "Bounties validators DAO fee transfer",
      false,
    )
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
      bounty.is_validators_dao_used(),
      "DAO validators are not used for this bounty"
    );
    bounty.assert_validators_dao_account_id(dao_params.clone().account_id);

    bounty.reviewers = Some(Reviewers::ValidatorsDao {validators_dao: dao_params.to_validators_dao()});
    self.bounties.insert(&id, &bounty.into());
  }

  #[payable]
  pub fn open_dispute(&mut self, id: BountyIndex, description: String) -> PromiseOrValue<()> {
    assert_one_yocto();

    assert!(
      self.dispute_contract.is_some(),
      "Opening a dispute is not supported by this contract"
    );

    let bounty = self.get_bounty(id.clone());
    assert!(
      matches!(bounty.status, BountyStatus::Claimed),
      "Bounty status does not allow opening a dispute"
    );

    let sender_id = env::predecessor_account_id();
    let (mut claims, claim_idx) = self.internal_get_claims(id.clone(), &sender_id);
    assert!(
      matches!(claims[claim_idx].status, ClaimStatus::Rejected),
      "The claim status does not allow opening a dispute"
    );

    assert!(
      !self.is_deadline_for_opening_dispute_expired(&claims[claim_idx]),
      "The period for opening a dispute has expired"
    );

    self.internal_create_dispute(id, &sender_id, bounty, claim_idx, &mut claims, description)
  }

  #[payable]
  pub fn dispute_result(&mut self, id: BountyIndex, success: bool) -> PromiseOrValue<()> {
    assert_one_yocto();

    assert!(
      self.dispute_contract.is_some(),
      "The use of dispute is not supported by this contract"
    );

    assert_eq!(
      self.dispute_contract.clone().unwrap(),
      env::predecessor_account_id(),
      "This method can only be called by a dispute contract"
    );

    let mut bounty = self.get_bounty(id.clone());
    assert!(
      matches!(bounty.status, BountyStatus::Claimed),
      "Bounty status does not allow sending the result of the dispute"
    );

    let claimer = self.internal_find_claimer(id.clone());
    assert!(
      claimer.is_some(),
      "The claim status does not allow sending the result of the dispute",
    );
    let sender_id = claimer.unwrap();
    let (mut claims, claim_idx) = self.internal_get_claims(id.clone(), &sender_id);

    let result = if success {
      self.internal_bounty_payout(id, sender_id, &mut bounty, claim_idx, &mut claims)
    } else {
      self.internal_reset_bounty_to_initial_state(id, &sender_id, &mut bounty, claim_idx, &mut claims);
      PromiseOrValue::Value(())
    };

    result
  }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
  use near_sdk::test_utils::{accounts, VMContextBuilder};
  use near_sdk::json_types::{U128, U64};
  use near_sdk::{testing_env, AccountId, Balance};
  use crate::{BountiesContract, Bounty, BountyAction, BountyClaim, BountyIndex, BountyMetadata,
              BountyStatus, ClaimerApproval, ClaimStatus, Config, ConfigCreate, Deadline, FeeStats,
              Reviewers, TokenDetails, ValidatorsDao, ValidatorsDaoParams};

  pub const TOKEN_DECIMALS: u8 = 18;
  pub const MAX_DEADLINE: U64 = U64(1_000_000_000 * 60 * 60 * 24 * 7);

  fn get_token_id() -> AccountId {
    "token_id".parse().unwrap()
  }

  fn get_disputes_contract() -> AccountId {
    "disputes".parse().unwrap()
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
    let bounty = Bounty {
      token: get_token_id(),
      amount: U128(d(2_000, TOKEN_DECIMALS)),
      platform_fee: U128(d(200, TOKEN_DECIMALS)),
      dao_fee: if validators_dao.is_some() {
        U128(d(200, TOKEN_DECIMALS))
      } else { U128(0) },
      metadata: BountyMetadata {
        title: "test".to_string(),
        description: "test".to_string(),
        category: "Other".to_string(),
        attachments: None,
        experience: None,
        tags: None,
        acceptance_criteria: None,
        contact_details: None,
      },
      deadline: Deadline::MaxDeadline {max_deadline: MAX_DEADLINE},
      claimer_approval: ClaimerApproval::WithoutApproval,
      reviewers: if validators_dao.is_some() {
        Some(Reviewers::ValidatorsDao {validators_dao: validators_dao.unwrap()})
      } else {
        None
      },
      owner: owner.clone(),
      status: BountyStatus::New,
      created_at: U64::from(0),
    };
    contract.bounties.insert(&bounty_index, &bounty.clone().into());
    contract.account_bounties.insert(owner, &vec![bounty_index]);
    contract.last_bounty_id = bounty_index.clone() + 1;
    add_token(contract);
    contract.internal_total_fees_receiving_funds(&bounty);

    bounty_index
  }

  fn add_token(
    contract: &mut BountiesContract,
  ) {
    contract.total_fees.insert(&get_token_id(), &FeeStats::new());
    contract.tokens.insert(&get_token_id(), &TokenDetails {
      enabled: true,
      min_amount_for_kyc: None,
    });
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
    let deadline = U64(1_000_000_000 * 60 * 60 * 24 * 2);
    let description = "Test description".to_string();

    contract.bounty_claim(id, deadline, description.clone());

    let bounty = contract.bounties.get(&id).unwrap().to_bounty();
    if bounty.is_validators_dao_used() {
      let mut claims = contract.get_bounty_claims(claimer.clone());
      contract.internal_create_claim(
        id,
        bounty,
        &mut claims,
        claimer.clone(),
        deadline,
        description,
        Some(U64(1))
      );
    }
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
    action: BountyAction,
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
      vec![],
      None,
      None,
      None,
      None,
      None
    );
  }

  #[test]
  fn test_add_to_admins_whitelist() {
    let mut context = VMContextBuilder::new();
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut contract = BountiesContract::new(
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );
    let account_ids: Vec<AccountId> = vec![accounts(1), accounts(2)];
    contract.add_to_admins_whitelist(None, Some(account_ids.clone()));
    assert_eq!(contract.get_admins_whitelist(), vec![accounts(0), accounts(1), accounts(2)]);

    contract.add_to_admins_whitelist(Some(accounts(3)), None);
    assert_eq!(
      contract.get_admins_whitelist(),
      vec![accounts(0), accounts(1), accounts(2), accounts(3)]
    );
  }

  #[test]
  fn test_remove_from_admins_whitelist() {
    let mut context = VMContextBuilder::new();
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut contract = BountiesContract::new(
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );
    contract.add_to_admins_whitelist(
      None,
      Some(vec![accounts(1), accounts(2), accounts(3)])
    );

    contract.remove_from_admins_whitelist(Some(accounts(3)), None);
    assert_eq!(contract.get_admins_whitelist(), vec![accounts(0), accounts(1), accounts(2)]);
    contract.remove_from_admins_whitelist(None, Some(vec![accounts(1), accounts(2)]));
    assert_eq!(contract.get_admins_whitelist(), vec![accounts(0)]);
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
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );
    contract.add_to_admins_whitelist(None, None);
  }

  #[test]
  #[should_panic(expected = "Requires attached deposit of exactly 1 yoctoNEAR")]
  fn test_add_to_admins_whitelist_without_deposit() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.predecessor_account_id(accounts(0)).build());
    let mut contract = BountiesContract::new(
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );
    contract.add_to_admins_whitelist(Some(accounts(1)), None);
  }

  #[test]
  #[should_panic(expected = "Not in admin whitelist")]
  fn test_add_to_admins_whitelist_by_other_user() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.predecessor_account_id(accounts(0)).build());
    let mut contract = BountiesContract::new(
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );
    testing_env!(context.predecessor_account_id(accounts(1)).attached_deposit(1).build());
    contract.add_to_admins_whitelist(Some(accounts(1)), None);
  }

  #[test]
  #[should_panic(expected = "Cannot remove all accounts from admin whitelist")]
  fn test_remove_from_admins_whitelist_all_members() {
    let mut context = VMContextBuilder::new();
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut contract = BountiesContract::new(
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );
    contract.remove_from_admins_whitelist(Some(accounts(0)), None);
  }

  #[test]
  fn test_add_token() {
    let mut context = VMContextBuilder::new();
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut contract = BountiesContract::new(
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );

    contract.add_token_id(get_token_id(), None);
    // For the unit test, the list of tokens has not changed.
    // The action is performed in the promise callback function (see simulation test).
  }

  #[test]
  #[should_panic(expected = "The token already exists")]
  fn test_token_already_exists() {
    let mut context = VMContextBuilder::new();
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut contract = BountiesContract::new(
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );
    add_token(&mut contract);

    contract.add_token_id(get_token_id(), None);
  }

  #[test]
  fn test_get_tokens() {
    let mut context = VMContextBuilder::new();
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut contract = BountiesContract::new(
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );
    add_token(&mut contract);

    assert_eq!(
      contract.get_tokens(),
      [
        (
          get_token_id(),
          TokenDetails {
            enabled: true,
            min_amount_for_kyc: None
          }
        )
      ]
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
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );

    let config_create = ConfigCreate {
      bounty_claim_bond: U128::from(2_000_000_000_000_000_000_000_000),
      bounty_forgiveness_period: U64::from(1_000_000_000 * 60 * 60 * 24 * 7),
      period_for_opening_dispute: U64::from(1_000_000_000 * 60 * 60 * 24 * 20),
      platform_fee_percentage: 7_000,
      validators_dao_fee_percentage: 5_000,
      penalty_platform_fee_percentage: 500,
      penalty_validators_dao_fee_percentage: 500,
    };
    contract.change_config(config_create.clone());
    let config = contract.get_config();
    assert_eq!(config_create, ConfigCreate {
      bounty_claim_bond: config.bounty_claim_bond,
      bounty_forgiveness_period: config.bounty_forgiveness_period,
      period_for_opening_dispute: config.period_for_opening_dispute,
      platform_fee_percentage: config.platform_fee_percentage,
      validators_dao_fee_percentage: config.validators_dao_fee_percentage,
      penalty_platform_fee_percentage: config.penalty_platform_fee_percentage,
      penalty_validators_dao_fee_percentage: config.penalty_validators_dao_fee_percentage,
    });
  }

  #[test]
  fn test_bounty_claim() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      None,
      None,
      None,
      None,
      None
    );
    let id = add_bounty(&mut contract, &accounts(1), None);

    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    assert_eq!(
      contract.bounty_claimers.get(&claimer).unwrap()[0].clone().to_bounty_claim(),
      BountyClaim {
        bounty_id: id.clone(),
        created_at: U64(0),
        start_time: Some(U64(0)),
        deadline: U64(1_000_000_000 * 60 * 60 * 24 * 2),
        status: ClaimStatus::InProgress,
        bounty_payout_proposal_id: None,
        approve_claimer_proposal_id: None,
        rejected_timestamp: None,
        dispute_id: None,
        description: "Test description".to_string(),
      }
    );
    assert_eq!(contract.bounty_claimer_accounts.get(&id).unwrap()[0], claimer);
    assert_eq!(contract.locked_amount, Config::default().bounty_claim_bond.0);
    assert_eq!(contract.bounties.get(&id).unwrap().to_bounty().status, BountyStatus::Claimed);
  }

  #[test]
  #[should_panic(expected = "Bounty wrong bond")]
  fn test_bounty_claim_without_bond() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      None,
      None,
      None
    );
    let id = add_bounty(&mut contract, &accounts(1), None);

    testing_env!(context
      .predecessor_account_id(accounts(2))
      .build());
    contract.bounty_claim(
      id,
      U64(1_000_000_000 * 60 * 60 * 24 * 2),
      "Test description".to_string()
    );
  }

  #[test]
  #[should_panic(expected = "Bounty status does not allow to submit a claim")]
  fn test_bounty_claim_with_incorrect_status() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
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
    contract.bounty_claim(
      id,
      U64(1_000_000_000 * 60 * 60 * 24 * 2),
      "Test description".to_string()
    );
  }

  #[test]
  #[should_panic(expected = "Bounty wrong deadline")]
  fn test_bounty_claim_with_incorrect_deadline() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      None,
      None,
      None
    );
    let id = add_bounty(&mut contract, &accounts(1), None);

    testing_env!(context
      .predecessor_account_id(accounts(2))
      .attached_deposit(Config::default().bounty_claim_bond.0)
      .build());
    contract.bounty_claim(
      id,
      U64(MAX_DEADLINE.0 + 1),
      "Test description".to_string()
    );
  }

  #[test]
  #[should_panic(expected = "Bounty not found")]
  fn test_claim_for_non_existent_bounty() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      None,
      None,
      None
    );
    let id = add_bounty(&mut contract, &accounts(1), None);

    testing_env!(context
      .predecessor_account_id(accounts(2))
      .attached_deposit(Config::default().bounty_claim_bond.0)
      .build());
    contract.bounty_claim(
      id + 1,
      MAX_DEADLINE,
      "Test description".to_string()
    );
  }

  #[test]
  fn test_bounty_done() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      None,
      None,
      None
    );
    let id = add_bounty(&mut contract, &accounts(1), None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);

    bounty_done(&mut context, &mut contract, id.clone(), &claimer);
    assert_eq!(
      contract.bounty_claimers.get(&claimer).unwrap()[0].clone().to_bounty_claim().status,
      ClaimStatus::Completed
    );
    assert_eq!(contract.bounties.get(&id).unwrap().to_bounty().status, BountyStatus::Claimed);
  }

  #[test]
  #[should_panic(expected = "Bounty status does not allow to completion")]
  fn test_bounty_done_with_incorrect_bounty_status() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
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
      vec![accounts(0)],
      None,
      None,
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
      vec![accounts(0)],
      None,
      None,
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
    assert_eq!(
      contract.bounty_claimers.get(&claimer).unwrap()[0].clone().to_bounty_claim().status,
      ClaimStatus::Expired
    );
    assert_eq!(contract.bounties.get(&id).unwrap().to_bounty().status, BountyStatus::New);
  }

  #[test]
  fn test_bounty_give_up() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
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
    assert_eq!(
      contract.bounty_claimers.get(&claimer).unwrap()[0].clone().to_bounty_claim().status,
      ClaimStatus::Canceled
    );
    assert_eq!(contract.bounties.get(&id).unwrap().to_bounty().status, BountyStatus::New);
    assert_eq!(contract.locked_amount, 0);

    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);

    bounty_give_up(
      &mut context,
      &mut contract,
      id.clone(),
      &claimer,
      bounty_forgiveness_period.clone() + 1
    );
    assert_eq!(
      contract.bounty_claimers.get(&claimer).unwrap()[0].clone().to_bounty_claim().status,
      ClaimStatus::Canceled
    );
    assert_eq!(contract.bounties.get(&id).unwrap().to_bounty().status, BountyStatus::New);
    assert_eq!(contract.locked_amount, Config::default().bounty_claim_bond.0);
  }

  #[test]
  #[should_panic(expected = "No claimer found")]
  fn test_bounty_give_up_with_unknown_claimer() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
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
      vec![accounts(0)],
      None,
      None,
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
      vec![accounts(0)],
      None,
      None,
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
      BountyAction::ClaimRejected { receiver_id: claimer.clone() },
    );
    assert_eq!(
      contract.bounty_claimers.get(&claimer).unwrap()[0].clone().to_bounty_claim().status,
      ClaimStatus::NotCompleted
    );
    assert_eq!(contract.bounties.get(&id).unwrap().to_bounty().status, BountyStatus::New);
    assert_eq!(contract.locked_amount, 0);

    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    bounty_done(&mut context, &mut contract, id.clone(), &claimer);

    bounty_action(
      &mut context,
      &mut contract,
      id.clone(),
      &project_owner,
      BountyAction::ClaimApproved { receiver_id: claimer },
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
      vec![accounts(0)],
      None,
      None,
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
      gas_for_claimer_approval: None,
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
      BountyAction::ClaimApproved { receiver_id: claimer },
    );
  }

  #[test]
  #[should_panic(expected = "Only the owner of the bounty can call this method")]
  fn test_bounty_action_by_dao() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
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
      vec![accounts(0)],
      None,
      None,
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
          gas_for_claimer_approval: None,
        }
          .to_validators_dao()
      )
    );

    let new_dao_params = ValidatorsDaoParams {
      account_id: "dao".parse().unwrap(),
      add_proposal_bond: U128(2 * 10u128.pow(24)),
      gas_for_add_proposal: Some(U64(50_000_000_000_000)),
      gas_for_claim_approval: Some(U64(50_000_000_000_000)),
      gas_for_claimer_approval: Some(U64(25_000_000_000_000)),
    };
    testing_env!(context
      .predecessor_account_id(project_owner)
      .attached_deposit(1)
      .build());
    contract.update_validators_dao_params(id, new_dao_params.clone());
    assert_eq!(
      contract.get_bounty(id.clone()).reviewers.unwrap(),
      Reviewers::ValidatorsDao {validators_dao: new_dao_params.to_validators_dao()}
    );
  }

  #[test]
  #[should_panic(expected = "Opening a dispute is not supported by this contract")]
  fn test_open_dispute_without_dispute_support() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
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
      BountyAction::ClaimRejected { receiver_id: claimer.clone() },
    );

    testing_env!(context
      .predecessor_account_id(claimer)
      .attached_deposit(1)
      .build());
    contract.open_dispute(id, "Test description".to_string());
  }

  #[test]
  #[should_panic(expected = "Bounty status does not allow opening a dispute")]
  fn test_open_dispute_with_incorrect_bounty_status() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      Some(get_disputes_contract()),
      None,
      None
    );
    let project_owner = accounts(1);
    let id = add_bounty(&mut contract, &project_owner, None);

    testing_env!(context
      .predecessor_account_id(accounts(2))
      .attached_deposit(1)
      .build());
    contract.open_dispute(id, "Test description".to_string());
  }

  #[test]
  #[should_panic(expected = "No claimer found")]
  fn test_open_dispute_by_other_user() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      Some(get_disputes_contract()),
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
      BountyAction::ClaimRejected { receiver_id: claimer.clone() },
    );
    assert_eq!(
      contract.bounty_claimers.get(&claimer).unwrap()[0].clone().to_bounty_claim().status,
      ClaimStatus::Rejected
    );

    testing_env!(context
      .predecessor_account_id(accounts(3))
      .attached_deposit(1)
      .build());
    contract.open_dispute(id, "Test description".to_string());
  }

  #[test]
  #[should_panic(expected = "The claim status does not allow opening a dispute")]
  fn test_open_dispute_with_incorrect_claim_status() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      Some(get_disputes_contract()),
      None,
      None
    );
    let project_owner = accounts(1);
    let id = add_bounty(&mut contract, &project_owner, None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    bounty_done(&mut context, &mut contract, id.clone(), &claimer);

    testing_env!(context
      .predecessor_account_id(claimer.clone())
      .attached_deposit(1)
      .build());
    contract.open_dispute(id, "Test description".to_string());
  }

  #[test]
  #[should_panic(expected = "The period for opening a dispute has expired")]
  fn test_open_dispute_with_expired_opening_period() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      Some(get_disputes_contract()),
      None,
      None
    );

    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let config = Config::default();
    contract.change_config(ConfigCreate {
      bounty_claim_bond: config.bounty_claim_bond,
      period_for_opening_dispute: U64(10),
      bounty_forgiveness_period: config.bounty_forgiveness_period,
      platform_fee_percentage: config.platform_fee_percentage,
      validators_dao_fee_percentage: config.validators_dao_fee_percentage,
      penalty_platform_fee_percentage: config.penalty_platform_fee_percentage,
      penalty_validators_dao_fee_percentage: config.penalty_validators_dao_fee_percentage,
    });

    let project_owner = accounts(1);
    let id = add_bounty(&mut contract, &project_owner, None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    bounty_done(&mut context, &mut contract, id.clone(), &claimer);

    testing_env!(context
      .block_timestamp(0)
      .build());
    bounty_action(
      &mut context,
      &mut contract,
      id.clone(),
      &project_owner,
      BountyAction::ClaimRejected { receiver_id: claimer.clone() },
    );

    testing_env!(context
      .predecessor_account_id(claimer.clone())
      .attached_deposit(1)
      .block_timestamp(20)
      .build());
    contract.open_dispute(id, "Test description".to_string());
  }
}
