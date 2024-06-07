use crate::*;

#[near_bindgen]
impl BountiesContract {
  #[private]
  pub fn on_added_proposal_callback(
    &mut self,
    #[callback_result] result: Result<u64, PromiseError>,
    id: BountyIndex,
    receiver_id: AccountId,
    claim_number: Option<u8>,
  ) -> bool {
    if !is_promise_success() || result.is_err() {
      env::log_str("Could not create bounty proposal");
      false
    } else {
      let proposal_id = result.unwrap();
      self.internal_claim_done(id, receiver_id, claim_number, Some(proposal_id.into()));
      true
    }
  }

  #[private]
  pub fn after_add_proposal(
    &mut self,
    #[callback_result] result: Result<u64, PromiseError>,
    id: BountyIndex,
    receiver_id: AccountId,
    deadline: Option<U64>,
    description: String,
    slot: Option<usize>,
  ) -> bool {
    if !is_promise_success() || result.is_err() {
      env::log_str("Could not create claimant proposal");
      false
    } else {
      self.internal_create_claim(
        id,
        receiver_id,
        None,
        deadline,
        description,
        Some(U64(result.unwrap())),
        slot,
        None,
      );
      true
    }
  }

  #[private]
  pub fn after_ft_transfer(
    &mut self,
    id: BountyIndex,
    claimant: Option<(AccountId, Option<u8>)>,
    amounts: (U128, U128, U128),
    place: Option<usize>,
  ) -> bool {
    if !is_promise_success() {
      env::log_str("Bounty payout failed");
      false
    } else {
      let bounty = self.get_bounty(id);
      self.internal_bounty_completion(id, bounty, claimant, amounts, place);
      true
    }
  }

  #[private]
  pub fn after_bounty_withdraw(
    &mut self,
    id: BountyIndex,
    receiver_id: AccountId,
    claim_number: Option<u8>,
    slot: usize,
  ) -> bool {
    if !is_promise_success() {
      env::log_str("Bounty payout failed");
      false
    } else {
      let bounty = self.get_bounty(id);
      self.internal_slot_finalize(id, bounty, receiver_id, claim_number, slot);
      true
    }
  }

  #[private]
  pub fn after_refund_bounty_amount(
    &mut self,
    id: BountyIndex,
  ) -> bool {
    if !is_promise_success() {
      env::log_str("Bounty refund failed");
      false
    } else {
      let bounty = self.get_bounty(id);
      self.internal_bounty_cancellation(id, bounty);
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
    claimant: Option<(AccountId, Option<u8>)>,
    proposal_id: U64,
  ) -> PromiseOrValue<()> {
    if !is_promise_success() || result.is_err() {
      env::panic_str("Error checking proposal status");
    } else {
      let proposal = result.unwrap();
      assert_eq!(proposal.id, proposal_id.0);
      assert_eq!(proposal.proposer, env::current_account_id());

      if claimant.is_none() {
        let bounty = self.get_bounty(id);
        assert!(
          matches!(bounty.status, BountyStatus::ManyClaimed),
          "Bounty status does not allow completion"
        );

        if proposal.status == "Approved" {
          self.internal_bounty_payout(id, None, None)
        } else {
          env::panic_str("The proposal status is not being processed");
        }
      } else {
        let (receiver_id, claim_number) = claimant.clone().unwrap();
        let (mut bounty, claim, _) = self.internal_get_and_check_bounty_and_claim(
          id.clone(),
          receiver_id.clone(),
          claim_number,
          vec![BountyStatus::Claimed, BountyStatus::ManyClaimed],
          vec![ClaimStatus::Completed],
          false,
          "Bounty status does not allow completion",
          "The claim status does not allow you to complete the bounty"
        );
        let (claim_id, mut bounty_claim) = claim.unwrap();

        if proposal.status == "Approved" {
          self.internal_bounty_payout(id, claimant, None)
        } else if proposal.status == "Rejected" {
          self.internal_reject_claim(id, receiver_id, &mut bounty, claim_id, &mut bounty_claim)
        } else {
          env::panic_str("The proposal status is not being processed");
        }
      }
    }
  }

  #[private]
  pub fn after_check_approve_claimant_proposal(
    &mut self,
    #[callback_result] result: Result<Proposal, PromiseError>,
    id: BountyIndex,
    receiver_id: AccountId,
    claim_number: Option<u8>,
    proposal_id: U64,
  ) -> PromiseOrValue<()> {
    if !is_promise_success() || result.is_err() {
      env::panic_str("Error checking proposal status");
    } else {
      let proposal = result.unwrap();
      assert_eq!(proposal.id, proposal_id.0);
      assert_eq!(proposal.proposer, env::current_account_id());
      if proposal.status == "Approved" {
        self.internal_approval_and_save_claim(id, Some((receiver_id, claim_number)), None, None)
      } else if proposal.status == "Rejected" {
        self.internal_rejection_and_save_claim(Some((id, receiver_id, claim_number)), None)
      } else {
        env::panic_str("The proposal status is not being processed");
      }
    }
  }

  #[private]
  pub fn after_create_dispute(
    &mut self,
    #[callback_result] result: Result<u64, PromiseError>,
    id: BountyIndex,
    receiver_id: &AccountId,
    claim_number: Option<u8>,
  ) -> bool {
    if !is_promise_success() || result.is_err() {
      env::log_str("Error create a dispute");
      false
    } else {
      let dispute_id = result.unwrap();
      let (_, claim, _) = self.internal_get_and_check_bounty_and_claim(
        id.clone(),
        receiver_id.clone(),
        claim_number,
        vec![BountyStatus::Claimed, BountyStatus::ManyClaimed],
        vec![ClaimStatus::Rejected],
        false,
        "Bounty status does not allow opening a dispute",
        "The claim status does not allow opening a dispute"
      );
      let (claim_id, mut bounty_claim) = claim.unwrap();
      bounty_claim.status = ClaimStatus::Disputed;
      bounty_claim.dispute_id = Some(dispute_id.into());
      self.claims.insert(&claim_id, &bounty_claim.into());
      true
    }
  }

  #[private]
  pub fn after_get_dispute(
    &mut self,
    #[callback_result] result: Result<Dispute, PromiseError>,
    id: BountyIndex,
    receiver_id: AccountId,
    claim_number: Option<u8>,
  ) -> PromiseOrValue<()> {
    if !is_promise_success() || result.is_err() {
      env::panic_str("Error checking dispute status");
    } else {
      let dispute = result.unwrap();
      let (mut bounty, claim, _) = self.internal_get_and_check_bounty_and_claim(
        id.clone(),
        receiver_id.clone(),
        claim_number,
        vec![BountyStatus::Claimed, BountyStatus::ManyClaimed],
        vec![ClaimStatus::Disputed],
        false,
        "Bounty status does not allow to reject a claim as a result of a dispute",
        "Claim status does not allow rejection as a result of a dispute"
      );
      let (claim_id, mut bounty_claim) = claim.unwrap();

      if dispute.status == "InFavorOfClaimant" || dispute.status == "CanceledByProjectOwner" {
        if bounty.is_different_tasks() {
          self.internal_claim_return_after_dispute(claim_id, &mut bounty_claim)
        } else {
          self.internal_bounty_payout(id, Some((receiver_id, claim_number)), None)
        }
      } else if dispute.status == "InFavorOfProjectOwner" || dispute.status == "CanceledByClaimant" {
        self.internal_reset_bounty_to_initial_state(
          id,
          &receiver_id,
          &mut bounty,
          claim_id,
          &mut bounty_claim,
          None,
          true
        )
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
    receiver_id: AccountId,
    claim_number: Option<u8>,
    place_of_check: PlaceOfCheckKYC,
    slot: Option<usize>,
  ) -> PromiseOrValue<()> {
    if !is_promise_success() || result.is_err() {
      env::panic_str("Error determining the claimant's KYC status");
    } else {
      let is_whitelisted = result.unwrap();
      if is_whitelisted {
        match place_of_check {
          PlaceOfCheckKYC::CreatingClaim { .. } => {
            self.internal_add_proposal_and_create_claim(
              id,
              receiver_id,
              claim_number,
              place_of_check,
              slot,
              None,
            )
          },
          PlaceOfCheckKYC::DecisionOnClaim { is_kyc_delayed } => {
            self.internal_approval_and_save_claim(
              id,
              Some((receiver_id, claim_number)),
              None,
              is_kyc_delayed
            )
          },
          _ => {
            self.internal_add_proposal_and_update_claim(id, receiver_id, claim_number, place_of_check)
          },
        }
      } else {
        env::panic_str("The claimant is not whitelisted");
      }
    }
  }
}
