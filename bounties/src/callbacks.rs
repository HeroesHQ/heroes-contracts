use crate::*;

#[near_bindgen]
impl BountiesContract {
  #[private]
  pub fn on_added_proposal_callback(
    &mut self,
    #[callback_unwrap] proposal_id: u64,
    id: BountyIndex,
    sender_id: &AccountId,
  ) -> bool {
    if !is_promise_success() {
      env::log_str("Error creating a bounty payout proposal");
      false
    } else {
      let (mut claims, claim_idx) = self.internal_get_claims(id, sender_id);
      claims[claim_idx].status = ClaimStatus::Completed;
      claims[claim_idx].proposal_id = Some(proposal_id);
      self.internal_save_claims(sender_id, &claims);
      true
    }
  }
}
