use crate::*;

#[near_bindgen]
impl DisputesContract {
  #[private]
  pub fn on_added_proposal_callback(
    &mut self,
    #[callback_result] result: Result<u64, PromiseError>,
    id: DisputeIndex,
    dispute: &mut Dispute,
  ) -> bool {
    if !is_promise_success() || result.is_err() {
      env::log_str("Could not create a dispute resolution proposal");
      false
    } else {
      let proposal_id = result.unwrap();
      dispute.proposal_id = Some(U64::from(proposal_id));
      dispute.proposal_timestamp = Some(U64::from(env::block_timestamp()));
      dispute.status = DisputeStatus::DecisionPending;
      self.disputes.insert(&id, &dispute);
      true
    }
  }

  #[private]
  pub fn after_claim_approval(
    &mut self,
    id: DisputeIndex,
    dispute: &mut Dispute,
    success: bool,
    canceled: bool,
  ) -> bool {
    if !is_promise_success() {
      env::log_str("Bounty finalization error");
      false
    } else {
      dispute.status = if success {
        if canceled {
          DisputeStatus::CanceledByProjectOwner
        } else {
          DisputeStatus::InFavorOfClaimer
        }
      } else {
        if canceled {
          DisputeStatus::CanceledByClaimer
        } else {
          DisputeStatus::InFavorOfProjectOwner
        }
      };
      self.disputes.insert(&id, &dispute);
      true
    }
  }

  #[private]
  pub fn after_get_proposal(
    &mut self,
    #[callback_result] result: Result<Proposal, PromiseError>,
    id: DisputeIndex,
    dispute: &mut Dispute,
  ) -> PromiseOrValue<()> {
    if !is_promise_success() || result.is_err() {
      env::panic_str("Error checking proposal status");
    } else {
      let proposal = result.unwrap();
      assert_eq!(proposal.id, dispute.proposal_id.unwrap().0);
      assert_eq!(proposal.proposer, env::current_account_id());
      let success = if proposal.status == "Approved" { true }
      else if proposal.status == "Rejected" { false }
      else {
        env::panic_str("The proposal status is not being processed");
      };
      self.internal_send_result_of_dispute(id, dispute, success, false)
    }
  }
}
