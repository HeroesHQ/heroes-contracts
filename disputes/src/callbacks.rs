use crate::*;

#[near_bindgen]
impl DisputesContract {
  #[private]
  pub fn after_get_bounty_info(
    &mut self,
    #[callback_result] bounty_result: Result<Bounty, PromiseError>,
    #[callback_result] bounty_claim_result: Result<Vec<(AccountId, BountyClaim)>, PromiseError>,
    bounty_id: u64,
    description: String,
  ) -> Option<DisputeIndex> {
    if !is_promise_success() || bounty_result.is_err() || bounty_claim_result.is_err() {
      env::log_str("Error getting bounty information");
      None
    } else {
      let bounty = bounty_result.unwrap();
      assert_eq!(
        bounty.status,
        "Claimed".to_string(),
        "Bounty status does not allow you to open a dispute",
      );
      let claimer = Self::internal_find_claimer(bounty_claim_result.unwrap());
      assert!(
        claimer.is_some(),
        "There are no claims with rejected status in the bounty",
      );

      let project_owner_delegate = if bounty.validators_dao.is_some() {
        bounty.validators_dao.unwrap().account_id
      } else {
        bounty.owner
      };

      let id = self.internal_add_dispute(Dispute {
        start_time: U64::from(env::block_timestamp()),
        description,
        bounty_id: U64::from(bounty_id),
        claimer: claimer.unwrap(),
        project_owner_delegate,
        status: DisputeStatus::New,
        arguments_of_participants: vec![],
        proposal_id: None,
        proposal_timestamp: None,
      });
      Some(id)
    }
  }

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
  ) -> bool {
    if !is_promise_success() {
      env::log_str("Bounty finalization error");
      false
    } else {
      dispute.status = if success {
        DisputeStatus::InFavorOfClaimer
      } else {
        DisputeStatus::InFavorOfProjectOwner
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
      self.internal_send_result_of_dispute(id, dispute, success)
    }
  }
}
