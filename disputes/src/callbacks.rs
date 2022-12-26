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
}
