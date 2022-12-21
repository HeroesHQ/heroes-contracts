use crate::*;

#[near_bindgen]
impl BountiesContract {
  #[private]
  pub fn on_added_proposal_callback(
    &mut self,
    #[callback_result] result: Result<u64, PromiseError>,
    sender_id: &AccountId,
    claims: &mut Vec<BountyClaim>,
    claim_idx: usize,
  ) -> bool {
    if !is_promise_success() || result.is_err() {
      env::log_str("Could not create bounty proposal");
      false
    } else {
      let proposal_id = result.unwrap();
      claims[claim_idx].status = ClaimStatus::Completed;
      claims[claim_idx].proposal_id = Some(U64(proposal_id));
      self.internal_save_claims(sender_id, &claims);
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
      claims[claim_idx].status = ClaimStatus::Approved;
      self.internal_save_claims(&receiver_id, &claims);
      bounty.status = BountyStatus::Completed;
      self.bounties.insert(&id, &bounty);
      // TODO: update reputation
      self.internal_return_bonds(&receiver_id);
      true
    }
  }

  #[private]
  pub fn after_get_proposal(
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
      assert_eq!(proposal.id, claims[claim_idx].proposal_id.unwrap().0);
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
}
