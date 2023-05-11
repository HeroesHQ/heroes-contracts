use crate::*;

#[near_bindgen]
impl BountiesContract {
  #[private]
  pub fn on_added_proposal_callback(
    &mut self,
    #[callback_result] result: Result<u64, PromiseError>,
    id: BountyIndex,
    claimer: AccountId,
  ) -> bool {
    if !is_promise_success() || result.is_err() {
      env::log_str("Could not create bounty proposal");
      false
    } else {
      let proposal_id = result.unwrap();
      self.internal_claim_done(id, claimer, Some(proposal_id.into()));
      true
    }
  }

  #[private]
  pub fn after_add_proposal(
    &mut self,
    #[callback_result] result: Result<u64, PromiseError>,
    id: BountyIndex,
    claimer: AccountId,
    deadline: U64,
    description: String,
  ) -> bool {
    if !is_promise_success() || result.is_err() {
      env::log_str("Could not create claimer proposal");
      false
    } else {
      self.internal_create_claim(
        id,
        claimer,
        deadline,
        description,
        Some(U64(result.unwrap()))
      );
      true
    }
  }

  #[private]
  pub fn after_ft_transfer(
    &mut self,
    id: BountyIndex,
    receiver_id: AccountId,
    bounty: &mut Bounty,
    claims: &mut Vec<BountyClaim>,
    claim_idx: usize,
  ) -> bool {
    if !is_promise_success() {
      env::log_str("Bounty payout failed");
      false
    } else {
      let with_dispute = matches!(claims[claim_idx].status, ClaimStatus::Disputed);
      claims[claim_idx].status = ClaimStatus::Approved;
      self.internal_total_fees_unlocking_funds(&bounty);
      self.internal_save_claims(&receiver_id, &claims);
      self.internal_change_status_and_save_bounty(&id, bounty.clone(), BountyStatus::Completed);
      self.internal_update_statistic(
        Some(receiver_id.clone()),
        Some(bounty.owner.clone()),
        ReputationActionKind::SuccessfulClaim {with_dispute},
      );
      self.internal_return_bonds(&receiver_id);
      true
    }
  }

  #[private]
  pub fn after_refund_bounty_amount(
    &mut self,
    id: BountyIndex,
    bounty: Bounty,
  ) -> bool {
    if !is_promise_success() {
      env::log_str("Bounty refund failed");
      false
    } else {
      self.internal_total_fees_refunding_funds(&bounty);
      self.internal_change_status_and_save_bounty(&id, bounty, BountyStatus::Canceled);
      true
    }
  }

  #[private]
  pub fn after_fees_payout(
    &mut self,
    token_id: AccountId,
    amount: U128,
    is_platform_fee: bool,
    account_id: AccountId,
  ) -> bool {
    if !is_promise_success() {
      env::log_str("Fees payout failed");
      false
    } else {
      if is_platform_fee {
        self.internal_platform_fee_withdraw(token_id, amount);
      } else {
        self.internal_validators_dao_fee_withdraw(account_id, token_id, amount);
      }
      true
    }
  }

  #[private]
  pub fn after_check_bounty_payout_proposal(
    &mut self,
    #[callback_result] result: Result<Proposal, PromiseError>,
    id: BountyIndex,
    receiver_id: AccountId,
    bounty: &mut Bounty,
    claims: &mut Vec<BountyClaim>,
    claim_idx: usize,
  ) -> PromiseOrValue<()> {
    if !is_promise_success() || result.is_err() {
      env::panic_str("Error checking proposal status");
    } else {
      let proposal = result.unwrap();
      assert_eq!(proposal.id, claims[claim_idx].bounty_payout_proposal_id.unwrap().0);
      assert_eq!(proposal.proposer, env::current_account_id());
      if proposal.status == "Approved" {
        self.internal_bounty_payout(id, receiver_id, bounty, claim_idx, claims)
      } else if proposal.status == "Rejected" {
        self.internal_reject_claim(id, receiver_id, bounty, claim_idx, claims);
        PromiseOrValue::Value(())
      } else {
        env::panic_str("The proposal status is not being processed");
      }
    }
  }

  #[private]
  pub fn after_check_approve_claimer_proposal(
    &mut self,
    #[callback_result] result: Result<Proposal, PromiseError>,
    id: BountyIndex,
    claimer: AccountId,
    bounty: Bounty,
    claims: &mut Vec<BountyClaim>,
    claim_idx: usize,
  ) -> bool {
    if !is_promise_success() || result.is_err() {
      env::log_str("Error checking proposal status");
      false
    } else {
      let proposal = result.unwrap();
      let mut bounty_claim= claims[claim_idx].clone();
      assert_eq!(proposal.id, bounty_claim.approve_claimer_proposal_id.unwrap().0);
      assert_eq!(proposal.proposer, env::current_account_id());
      if proposal.status == "Approved" {
        self.internal_claimer_approval(id, bounty, &mut bounty_claim, &claimer, None);
      } else if proposal.status == "Rejected" {
        bounty_claim.status = ClaimStatus::NotHired;
        self.internal_return_bonds(&claimer);
      } else {
        env::panic_str("The proposal status is not being processed");
      }
      claims.insert(claim_idx, bounty_claim);
      self.internal_save_claims(&claimer, &claims);
      true
    }
  }

  #[private]
  pub fn after_create_dispute(
    &mut self,
    #[callback_result] result: Result<u64, PromiseError>,
    receiver_id: &AccountId,
    claims: &mut Vec<BountyClaim>,
    claim_idx: usize,
  ) -> bool {
    if !is_promise_success() || result.is_err() {
      env::log_str("Error create a dispute");
      false
    } else {
      let dispute_id = result.unwrap();
      claims[claim_idx].status = ClaimStatus::Disputed;
      claims[claim_idx].dispute_id = Some(dispute_id.into());
      self.internal_save_claims(receiver_id, &claims);
      true
    }
  }

  #[private]
  pub fn after_get_dispute(
    &mut self,
    #[callback_result] result: Result<Dispute, PromiseError>,
    id: BountyIndex,
    receiver_id: AccountId,
    bounty: &mut Bounty,
    claims: &mut Vec<BountyClaim>,
    claim_idx: usize,
  ) -> PromiseOrValue<()> {
    if !is_promise_success() || result.is_err() {
      env::panic_str("Error checking dispute status");
    } else {
      let dispute = result.unwrap();
      if dispute.status == "InFavorOfClaimer" || dispute.status == "CanceledByProjectOwner" {
        self.internal_bounty_payout(id, receiver_id, bounty, claim_idx, claims)
      } else if dispute.status == "InFavorOfProjectOwner" || dispute.status == "CanceledByClaimer" {
        self.internal_reset_bounty_to_initial_state(id, &receiver_id, bounty, claim_idx, claims);
        PromiseOrValue::Value(())
      } else {
        env::panic_str("The dispute status is not being processed");
      }
    }
  }

  #[private]
  pub fn after_get_ft_metadata(
    &mut self,
    #[callback_result] result: Result<FungibleTokenMetadata, PromiseError>,
    token_id: AccountId,
    min_amount_for_kyc: Option<U128>,
  ) -> bool {
    if !is_promise_success() || result.is_err() {
      env::log_str("Error getting token metadata");
      false
    } else {
      self.total_fees.insert(&token_id, &FeeStats::new());
      self.tokens.insert(&token_id, &TokenDetails {
        enabled: true,
        min_amount_for_kyc,
      });
      true
    }
  }

  #[private]
  pub fn after_check_if_whitelisted(
    &mut self,
    #[callback_result] result: Result<bool, PromiseError>,
    id: BountyIndex,
    claimer: AccountId,
    place_of_check: PlaceOfCheckKYC,
  ) -> PromiseOrValue<()> {
    if !is_promise_success() || result.is_err() {
      env::panic_str("Error determining the claimer's KYC status");
    } else {
      let is_whitelisted = result.unwrap();
      if is_whitelisted {
        match place_of_check {
          PlaceOfCheckKYC::CreatingClaim { .. } => {
            self.internal_add_proposal_and_create_claim(id, claimer, place_of_check)
          },
          PlaceOfCheckKYC::DecisionOnClaim { .. } => {
            self.internal_approval_and_save_claim(id, claimer, place_of_check)
          },
          _ => {
            self.internal_add_proposal_and_update_claim(id, claimer, place_of_check)
          },
        }
      } else {
        env::panic_str("The claimer is not whitelisted");
      }
    }
  }
}
