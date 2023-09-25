use crate::*;

#[near_bindgen]
impl DisputesContract {
  #[private]
  pub fn on_added_proposal_callback(
    &mut self,
    #[callback_result] result: Result<u64, PromiseError>,
    id: DisputeIndex,
  ) -> bool {
    if !is_promise_success() || result.is_err() {
      env::log_str("Could not create a dispute resolution proposal");
      false
    } else {
      let proposal_id = result.unwrap();
      let mut dispute = self.get_dispute(id);
      if dispute.status != DisputeStatus::New {
        env::panic_str(MESSAGE_DISPUTE_IS_NOT_NEW);
      }
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
    success: bool,
    canceled: bool,
  ) -> bool {
    if !is_promise_success() {
      env::log_str("Bounty finalization error");
      false
    } else {
      let mut dispute = self.get_dispute(id);
      assert!(
        matches!(dispute.status, DisputeStatus::New) ||
          matches!(dispute.status, DisputeStatus::DecisionPending),
        "The dispute has already completed status",
      );
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
  ) -> PromiseOrValue<()> {
    if !is_promise_success() || result.is_err() {
      env::panic_str("Error checking proposal status");
    } else {
      let proposal = result.unwrap();
      let dispute = self.get_dispute(id);
      assert_eq!(proposal.id, dispute.proposal_id.unwrap().0);
      assert_eq!(proposal.proposer, env::current_account_id());
      if dispute.status != DisputeStatus::DecisionPending {
        env::panic_str(MESSAGE_DISPUTE_IS_NOT_PENDING);
      }
      let success = if proposal.status == "Approved" { true }
      else if proposal.status == "Rejected" { false }
      else {
        if self.is_decision_period_expired(&dispute) {
          true
        } else {
          env::panic_str("The proposal status is not being processed");
        }
      };
      self.internal_send_result_of_dispute(id, dispute.bounty_id, dispute.claimer, success, false)
    }
  }
}
