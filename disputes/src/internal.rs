use crate::*;

impl DisputesContract {
  pub(crate) fn assert_admin_whitelist(&self, account_id: &AccountId) {
    assert!(
      self.admin_whitelist.contains(account_id),
      "Not in admin whitelist"
    );
  }

  pub(crate) fn internal_save_bounty_disputes(
    &mut self,
    bounty_id: &u64,
    indices: Vec<DisputeIndex>,
  ) {
    if indices.is_empty() {
      self.bounty_disputes.remove(bounty_id);
    } else {
      self.bounty_disputes.insert(bounty_id, &indices);
    }
  }

  pub(crate) fn internal_add_dispute(&mut self, dispute: Dispute) -> DisputeIndex {
    let id = self.last_dispute_id;
    self.disputes.insert(&id, &dispute.clone().into());
    let bounty_id = dispute.bounty_id.0;
    let mut indices = self
      .bounty_disputes
      .get(&bounty_id)
      .unwrap_or_default();
    indices.push(id);
    self.internal_save_bounty_disputes(&bounty_id, indices);
    self.last_dispute_id += 1;
    id
  }

  pub(crate) fn internal_send_result_of_dispute(
    &self,
    id: DisputeIndex,
    bounty_id: U64,
    claimer: AccountId,
    claim_number: Option<u8>,
    success: bool,
    canceled: bool,
  ) -> PromiseOrValue<()> {
    ext_bounty_contract::ext(self.bounties_contract.clone())
      .with_static_gas(GAS_FOR_SEND_RESULT_OF_DISPUTE)
      .with_attached_deposit(1)
      .dispute_result(bounty_id.0, claimer, claim_number, success)
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_CLAIM_APPROVAL)
          .after_claim_approval(id, success, canceled)
      )
      .into()
  }

  pub(crate) fn is_argument_period_expired(&self, dispute: &Dispute) -> bool {
    env::block_timestamp() >
      dispute.start_time.0 + self.config.clone().to_config().argument_period.0
  }

  pub(crate) fn is_decision_period_expired(&self, dispute: &Dispute) -> bool {
    env::block_timestamp() >
      dispute.proposal_timestamp.unwrap().0 + self.config.clone().to_config().decision_period.0
  }

  pub(crate) fn internal_add_proposal(
    &self,
    id: DisputeIndex,
    dispute: Dispute,
  ) -> PromiseOrValue<()> {
    Promise::new(self.dispute_dao.clone())
      .function_call(
        "add_proposal".to_string(),
        json!({
          "proposal": {
            "description": self.internal_get_proposal_description(&id, &dispute),
            "kind": {
              "FunctionCall" : {
                "receiver_id": env::current_account_id(),
                "actions": [
                  {
                    "method_name": "result_of_dispute",
                    "args": Base64VecU8::from(json!({
                      "id": id.clone(),
                      "success": true,
                    })
                      .to_string()
                      .into_bytes()
                      .to_vec()),
                    "deposit": "1",
                    "gas": U64(GAS_FOR_CLAIM_APPROVAL.0),
                  }
                ],
              }
            }
          }
        })
          .to_string()
          .into_bytes(),
        self.config.clone().to_config().add_proposal_bond.0,
        GAS_FOR_ADD_PROPOSAL,
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_ON_ADDED_PROPOSAL_CALLBACK)
          .on_added_proposal_callback(id)
      )
      .into()
  }

  pub(crate) fn internal_get_proposal(
    &mut self,
    id: DisputeIndex,
    proposal_id: U64,
  ) -> PromiseOrValue<()> {
    Promise::new(self.dispute_dao.clone())
      .function_call(
        "get_proposal".to_string(),
        json!({"id": proposal_id.0}).to_string().into_bytes(),
        NO_DEPOSIT,
        GAS_FOR_CHECK_PROPOSAL,
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_CHECK_PROPOSAL)
          .after_get_proposal(id)
      )
      .into()
  }

  pub(crate) fn internal_add_argument(
    &mut self,
    id: &DisputeIndex,
    new_reason: Reason,
  ) -> usize {
    let mut reasons = self.arguments.get(id).unwrap_or_default();
    reasons.push(new_reason.into());
    self.arguments.insert(id, &reasons);
    reasons.len() - 1
  }

  pub(crate) fn internal_get_proposal_description(
    &self,
    id: &DisputeIndex,
    dispute: &Dispute
  ) -> String {
    let reasons = self.arguments.get(id).unwrap_or_default();
    let mut full_description = Self::chunk_of_description(
      dispute,
      &Reason {
        side: Side::Claimer,
        argument_timestamp: dispute.start_time,
        description: dispute.clone().description,
      }
    ).to_owned();
    for i in 0..reasons.len() {
      let reason = reasons[i].clone();
      full_description.push_str(Self::chunk_of_description(dispute, &reason.into()).as_str());
    }
    full_description
  }

  pub(crate) fn assert_live(&self) {
    assert!(
      matches!(self.status, ContractStatus::Live),
      "The contract status is not Live"
    );
  }
}
