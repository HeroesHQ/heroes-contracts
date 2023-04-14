use crate::*;

impl BountiesContract {
  pub(crate) fn assert_that_caller_is_allowed(&self, account_id: &AccountId) {
    let token_details = self.tokens.get(account_id);
    assert!(
      token_details.is_some(),
      "Predecessor account ID is not an allowed FT contract"
    );
    assert!(
      token_details.unwrap().enabled,
      "Creating a bounty using this token is not currently supported"
    );
  }

  pub(crate) fn assert_admins_whitelist(&self, account_id: &AccountId) {
    assert!(
      self.admins_whitelist.contains(account_id),
      "Not in admin whitelist"
    );
  }

  pub(crate) fn is_claimer_whitelisted(
    &self,
    bounty_owner: AccountId,
    claimer: &AccountId
  ) -> bool {
    let claimers_whitelist = self.claimers_whitelist.get(&bounty_owner);
    claimers_whitelist.is_some() && claimers_whitelist.unwrap().contains(claimer)
  }

  pub(crate) fn assert_bounty_category_is_correct(&self, category: String) {
    assert!(
      self.config.clone().to_config().categories.contains(&category),
      "Invalid bounty type {}",
      category
    );
  }

  pub(crate) fn assert_bounty_tags_are_correct(&self, tags: Vec<String>) {
    tags.into_iter().for_each(|t| {
      assert!(self.config.clone().to_config().tags.contains(&t), "Invalid bounty tag {}", t);
    });
  }

  pub(crate) fn internal_add_bounty(&mut self, bounty: Bounty) -> BountyIndex {
    let id = self.last_bounty_id;
    self.bounties.insert(&id, &bounty.clone().into());
    let mut indices = self
      .account_bounties
      .get(&bounty.owner)
      .unwrap_or_default();
    indices.push(id);
    self.internal_save_account_bounties(&bounty.owner, indices);
    self.last_bounty_id += 1;
    self.internal_total_fees_receiving_funds(&bounty);
    self.internal_update_statistic(
      None,
      Some(bounty.owner.clone()),
      ReputationActionKind::BountyCreated
    );
    id
  }

  pub(crate) fn internal_total_fees_receiving_funds(
    &mut self,
    bounty: &Bounty,
  ) {
    let mut total_fees = self.total_fees.get(&bounty.token).unwrap();
    total_fees.receiving_commission(&bounty.platform_fee);
    self.total_fees.insert(&bounty.token, &total_fees);

    let dao_fee_stats = self.internal_get_dao_fee_stats(bounty);
    if dao_fee_stats.is_some() {
      let (dao_account_id, mut stats, stats_idx) = dao_fee_stats.unwrap();
      stats[stats_idx].fee_stats.receiving_commission(&bounty.dao_fee);
      self.total_validators_dao_fees.insert(&dao_account_id, &stats);
    }
  }

  pub(crate) fn internal_get_dao_fee_stats(
    &self,
    bounty: &Bounty,
  ) -> Option<(AccountId, Vec<DaoFeeStats>, usize)> {
    if bounty.reviewers.is_some() {
      match bounty.reviewers.clone().unwrap() {
        Reviewers::ValidatorsDao { validators_dao } => {
          let mut stats = self.get_total_validators_dao_fees(validators_dao.clone().account_id);
          let stats_idx = Self::internal_find_dao_fee_stats(bounty.clone().token, &mut stats);
          return Some((validators_dao.account_id, stats, stats_idx));
        },
        _ => {}
      }
    };
    None
  }

  pub(crate) fn internal_find_dao_fee_stats(
    token_id: AccountId,
    stats: &mut Vec<DaoFeeStats>
  ) -> usize {
    let stats_size = stats.len();
    for i in 0..stats_size {
      if stats[i].token_id == token_id {
        return i;
      }
    }
    stats.push(DaoFeeStats::new(token_id));
    stats_size
  }

  pub(crate) fn internal_get_unlocked_platform_fee_amount(&self, token_id: AccountId) -> U128 {
    self.total_fees
      .get(&token_id)
      .expect("No platform fees found")
      .get_available_balance()
  }

  pub(crate) fn internal_platform_fee_withdraw(
    &mut self,
    token_id: AccountId,
    amount: U128,
  ) {
    let mut total_fees = self.total_fees.get(&token_id).unwrap();
    total_fees.withdrawal_of_commission(&amount);
    self.total_fees.insert(&token_id, &total_fees);
  }

  pub(crate) fn internal_get_unlocked_validators_dao_fee_amount(
    &self,
    dao_account_id: AccountId,
    token_id: AccountId
  ) -> U128 {
    let mut stats = self.total_validators_dao_fees
      .get(&dao_account_id)
      .expect("No validators DAO fees found");
    let stats_idx = Self::internal_find_dao_fee_stats(token_id, &mut stats);
    stats[stats_idx].fee_stats.get_available_balance()
  }

  pub(crate) fn internal_validators_dao_fee_withdraw(
    &mut self,
    dao_account_id: AccountId,
    token_id: AccountId,
    amount: U128,
  ) {
    let mut stats = self.total_validators_dao_fees
      .get(&dao_account_id)
      .expect("No validators DAO fees found");
    let stats_idx = Self::internal_find_dao_fee_stats(token_id, &mut stats);
    stats[stats_idx].fee_stats.withdrawal_of_commission(&amount);
    self.total_validators_dao_fees.insert(&dao_account_id, &stats);
  }

  pub(crate) fn assert_locked_amount_greater_than_or_equal_transaction_amount(
    &self,
    bounty: &Bounty,
  ) {
    let total_fees = self.total_fees.get(&bounty.token).unwrap();
    total_fees.assert_locked_amount_greater_than_or_equal_transaction_amount(&bounty.platform_fee);
    let dao_fee_stats = self.internal_get_dao_fee_stats(bounty);
    if dao_fee_stats.is_some() {
      let (_, stats, stats_idx) = dao_fee_stats.unwrap();
      stats[stats_idx].fee_stats
        .assert_locked_amount_greater_than_or_equal_transaction_amount(&bounty.dao_fee);
    }
  }

  pub(crate) fn internal_total_fees_refunding_funds(
    &mut self,
    bounty: &Bounty,
  ) {
    let mut total_fees = self.total_fees.get(&bounty.token).unwrap();
    let config = self.config.clone().to_config();
    total_fees.refund_commission(
      &bounty.platform_fee,
      config.platform_fee_percentage,
      config.penalty_platform_fee_percentage
    );
    self.total_fees.insert(&bounty.token, &total_fees);

    let dao_fee_stats = self.internal_get_dao_fee_stats(bounty);
    if dao_fee_stats.is_some() {
      let (dao_account_id, mut stats, stats_idx) = dao_fee_stats.unwrap();
      stats[stats_idx].fee_stats.refund_commission(
        &bounty.dao_fee,
        config.validators_dao_fee_percentage,
        config.penalty_validators_dao_fee_percentage
      );
      self.total_validators_dao_fees.insert(&dao_account_id, &stats);
    }
  }

  pub(crate) fn internal_total_fees_unlocking_funds(
    &mut self,
    bounty: &Bounty,
  ) {
    let mut total_fees = self.total_fees.get(&bounty.token).unwrap();
    total_fees.commission_unlocking(&bounty.platform_fee);
    self.total_fees.insert(&bounty.token, &total_fees);

    let dao_fee_stats = self.internal_get_dao_fee_stats(bounty);
    if dao_fee_stats.is_some() {
      let (dao_account_id, mut stats, stats_idx) = dao_fee_stats.unwrap();
      stats[stats_idx].fee_stats.commission_unlocking(&bounty.dao_fee);
      self.total_validators_dao_fees.insert(&dao_account_id, &stats);
    }
  }

  pub(crate) fn internal_get_full_bounty_amount(
    &self,
    bounty: &Bounty,
  ) -> U128 {
    let config = self.config.clone().to_config();
    let penalty_platform_fee: u128 = if config.platform_fee_percentage != 0 {
      bounty.platform_fee.0 *
        config.penalty_platform_fee_percentage as u128 /
        config.platform_fee_percentage as u128
    } else { 0 };
    let penalty_validators_dao_fee: u128 = if config.validators_dao_fee_percentage != 0 {
      bounty.dao_fee.0 *
        config.penalty_validators_dao_fee_percentage as u128 /
        config.platform_fee_percentage as u128
    } else { 0 };
    let amount = bounty.amount.0 + bounty.platform_fee.0 - penalty_platform_fee +
      bounty.dao_fee.0 - penalty_validators_dao_fee;
    U128(amount)
  }

  pub(crate) fn internal_save_account_bounties(
    &mut self,
    account_id: &AccountId,
    indices: Vec<BountyIndex>,
  ) {
    if indices.is_empty() {
      self.account_bounties.remove(account_id);
    } else {
      self.account_bounties.insert(account_id, &indices);
    }
  }

  pub(crate) fn internal_find_claim(
    id: BountyIndex,
    claims: &[BountyClaim]
  ) -> Option<usize> {
    for i in 0..claims.len() {
      if claims[i].bounty_id == id {
        return Some(i);
      }
    }
    None
  }

  pub(crate) fn internal_add_claim(
    id: BountyIndex,
    claims: &mut Vec<BountyClaim>,
    new_claim: BountyClaim
  ) {
    let claim_idx = Self::internal_find_claim(id, claims);
    if claim_idx.is_some() {
      claims.insert(claim_idx.unwrap(), new_claim);
    } else {
      claims.push(new_claim);
    }
  }

  pub(crate) fn internal_get_claims(
    &self,
    id: BountyIndex,
    sender_id: &AccountId
  ) -> (Vec<BountyClaim>, usize) {
    let versioned_claims= self
      .bounty_claimers
      .get(&sender_id)
      .expect("No claimer found");
    let claims: Vec<BountyClaim> = versioned_claims
      .into_iter()
      .map(|c| c.into())
      .collect();
    let claim_idx = Self::internal_find_claim(id, &claims)
      .expect("No bounty claim found");
    (claims, claim_idx)
  }

  pub(crate) fn internal_save_claims(
    &mut self,
    account_id: &AccountId,
    claims: &Vec<BountyClaim>,
  ) {
    if claims.is_empty() {
      self.bounty_claimers.remove(account_id);
    } else {
      let versioned_claims = claims
        .into_iter()
        .map(|c| c.clone().into())
        .collect();
      self.bounty_claimers.insert(account_id, &versioned_claims);
    }
  }

  pub(crate) fn internal_add_bounty_claimer_account(
    &mut self,
    id: BountyIndex,
    account_id: AccountId,
  ) {
    let mut claimer_accounts = self.bounty_claimer_accounts.get(&id).unwrap_or_default();
    if !claimer_accounts.contains(&account_id) {
      claimer_accounts.push(account_id);
      self.bounty_claimer_accounts.insert(&id, &claimer_accounts);
    }
  }

  pub(crate) fn internal_return_bonds(&mut self, receiver_id: &AccountId) -> PromiseOrValue<()> {
    let config = self.config.clone().to_config();
    self.locked_amount -= config.bounty_claim_bond.0;
    Promise::new(receiver_id.clone()).transfer(config.bounty_claim_bond.0).into()
  }

  pub(crate) fn internal_reset_bounty_to_initial_state(
    &mut self,
    id: BountyIndex,
    receiver_id: &AccountId,
    bounty: &mut Bounty,
    claim_idx: usize,
    claims: &mut Vec<BountyClaim>
  ) {
    self.internal_change_status_and_save_bounty(&id, bounty.clone(), BountyStatus::New);
    let with_dispute = matches!(claims[claim_idx].status, ClaimStatus::Disputed);
    claims[claim_idx].status = ClaimStatus::NotCompleted;
    self.internal_save_claims(receiver_id, &claims);
    self.internal_update_statistic(
      Some(receiver_id.clone()),
      Some(bounty.owner.clone()),
      ReputationActionKind::UnsuccessfulClaim {with_dispute},
    );
    self.internal_return_bonds(receiver_id);
  }

  pub(crate) fn is_deadline_for_opening_dispute_expired(&self, claim: &BountyClaim) -> bool {
    env::block_timestamp() >
      claim.rejected_timestamp.unwrap().0 +
        self.config.clone().to_config().period_for_opening_dispute.0
  }

  pub(crate) fn internal_set_claim_expiry_status(
    &mut self,
    id: BountyIndex,
    receiver_id: &AccountId,
    bounty: Bounty,
    claim_idx: usize,
    claims: &mut Vec<BountyClaim>
  ) {
    claims[claim_idx].status = ClaimStatus::Expired;
    self.internal_save_claims(receiver_id, &claims);
    self.internal_change_status_and_save_bounty(&id, bounty, BountyStatus::New);
    self.internal_update_statistic(
      Some(receiver_id.clone()),
      None,
      ReputationActionKind::ClaimExpired
    );
  }

  pub(crate) fn internal_change_status_and_save_bounty(
    &mut self,
    id: &BountyIndex,
    mut bounty: Bounty,
    status: BountyStatus,
  ) {
    bounty.status = status;
    self.bounties.insert(&id, &bounty.into());
  }

  pub(crate) fn is_claim_active(claim: &BountyClaim) -> bool {
    matches!(claim.status, ClaimStatus::InProgress)
      || matches!(claim.status, ClaimStatus::Completed)
      || matches!(claim.status, ClaimStatus::Rejected)
      || matches!(claim.status, ClaimStatus::Disputed)
  }

  pub(crate) fn internal_find_active_claim(
    &self,
    id: BountyIndex,
  ) -> (AccountId, Vec<BountyClaim>, usize) {
    let account_ids = self
      .bounty_claimer_accounts
      .get(&id)
      .expect("No claims found");

    for account_id in account_ids {
      let (claims, index) = self.internal_get_claims(id, &account_id);
      let claim = claims[index].clone();
      if Self::is_claim_active(&claim) {
        return (account_id, claims, index);
      }
    }

    env::panic_str("No active claims");
  }

  pub(crate) fn internal_bounty_payout(
    &self,
    id: BountyIndex,
    receiver_id: AccountId,
    bounty: &mut Bounty,
    claim_idx: usize,
    claims: &mut Vec<BountyClaim>,
  ) -> PromiseOrValue<()> {
    self.assert_locked_amount_greater_than_or_equal_transaction_amount(&bounty);
    ext_ft_contract::ext(bounty.token.clone())
      .with_attached_deposit(ONE_YOCTO)
      .with_static_gas(GAS_FOR_FT_TRANSFER)
      .ft_transfer(
        receiver_id.clone(),
        bounty.amount.clone(),
        Some(format!("Bounty {} payout", id)),
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_FT_TRANSFER)
          .after_ft_transfer(id, receiver_id, bounty, claims, claim_idx)
      )
      .into()
  }

  pub(crate) fn internal_refund_bounty_amount(
    &self,
    id: BountyIndex,
    bounty: Bounty,
  ) -> PromiseOrValue<()> {
    self.assert_locked_amount_greater_than_or_equal_transaction_amount(&bounty);
    let full_amount = self.internal_get_full_bounty_amount(&bounty);

    ext_ft_contract::ext(bounty.token.clone())
      .with_attached_deposit(ONE_YOCTO)
      .with_static_gas(GAS_FOR_FT_TRANSFER)
      .ft_transfer(
        bounty.owner.clone(),
        full_amount,
        Some(format!("Returning amount of bounty {} to {}", id, bounty.owner)),
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_FT_TRANSACT)
          .after_refund_bounty_amount(id, bounty)
      )
      .into()
  }

  pub(crate) fn internal_fees_payout(
    token_id: AccountId,
    amount: U128,
    receiver: AccountId,
    msg: &str,
    is_platform_fee: bool,
  ) -> PromiseOrValue<()> {
    ext_ft_contract::ext(token_id.clone())
      .with_attached_deposit(ONE_YOCTO)
      .with_static_gas(GAS_FOR_FT_TRANSFER)
      .ft_transfer(
        receiver.clone(),
        amount.clone(),
        Some(format!("{} to {}", msg, receiver.clone())),
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_FT_TRANSACT)
          .after_fees_payout(token_id, amount, is_platform_fee, receiver)
      )
      .into()
  }

  pub(crate) fn internal_reject_claim(
    &mut self,
    id: BountyIndex,
    receiver_id: AccountId,
    bounty: &mut Bounty,
    claim_idx: usize,
    claims: &mut Vec<BountyClaim>,
  ) {
    if self.dispute_contract.is_some() {
      claims[claim_idx].status = ClaimStatus::Rejected;
      claims[claim_idx].rejected_timestamp = Some(env::block_timestamp().into());
      self.internal_save_claims(&receiver_id, &claims);
    } else {
      // If the creation of a dispute is not foreseen,
      // then the bounty reset to initial state
      self.internal_reset_bounty_to_initial_state(id, &receiver_id, bounty, claim_idx, claims);
    }
  }

  pub(crate) fn internal_add_proposal_to_finish_claim(
    id: BountyIndex,
    sender_id: &AccountId,
    bounty: &Bounty,
    claim_idx: usize,
    claims: &mut Vec<BountyClaim>,
    description: String,
  ) -> PromiseOrValue<()> {
    if let Reviewers::ValidatorsDao {validators_dao} = bounty.reviewers.clone().unwrap() {
      Self::internal_add_proposal(
        validators_dao.account_id,
        json!({
          "proposal": {
            "description": description,
            "kind": {
              "FunctionCall" : {
                "receiver_id": env::current_account_id(),
                "actions": [
                  {
                    "method_name": "bounty_action",
                    "args": Base64VecU8::from(json!({
                      "id": id.clone(),
                      "action": {
                        "ClaimApproved": {
                          "receiver_id": sender_id.to_string(),
                        }
                      }
                    })
                      .to_string()
                      .into_bytes()
                      .to_vec()),
                    "deposit": "1",
                    "gas": validators_dao.gas_for_claim_approval,
                  }
                ],
              }
            }
          }
        })
          .to_string()
          .into_bytes(),
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_ON_ADDED_PROPOSAL_CALLBACK)
          .on_added_proposal_callback(sender_id, claims.as_mut(), claim_idx),
        validators_dao.add_proposal_bond,
        validators_dao.gas_for_add_proposal,
      )
    } else {
      unreachable!();
    }
  }

  pub(crate) fn internal_add_proposal_to_approve_claimer(
    id: BountyIndex,
    bounty: Bounty,
    claims: &mut Vec<BountyClaim>,
    claimer: AccountId,
    deadline: U64,
    description: String,
  ) -> PromiseOrValue<()> {
    if let Reviewers::ValidatorsDao {validators_dao} = bounty.reviewers.clone().unwrap() {
      Self::internal_add_proposal(
        validators_dao.account_id,
        json!({
          "proposal": {
            "description": description,
            "kind": {
              "FunctionCall" : {
                "receiver_id": env::current_account_id(),
                "actions": [
                  {
                    "method_name": "decision_on_claim",
                    "args": Base64VecU8::from(json!({
                      "id": id.clone(),
                      "claimer": claimer.clone(),
                      "approve": true,
                    })
                      .to_string()
                      .into_bytes()
                      .to_vec()),
                    "deposit": "1",
                    "gas": validators_dao.gas_for_claimer_approval,
                  }
                ],
              }
            }
          }
        })
          .to_string()
          .into_bytes(),
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_ADD_PROPOSAL)
          .after_add_proposal(id, bounty, claims, claimer, deadline, description),
        validators_dao.add_proposal_bond,
        validators_dao.gas_for_add_proposal,
      )
    } else {
      unreachable!();
    }
  }

  pub(crate) fn internal_add_proposal(
    receiver: AccountId,
    arguments: Vec<u8>,
    callback: Promise,
    amount: U128,
    gas: U64,
  ) -> PromiseOrValue<()> {
    Promise::new(receiver)
      .function_call(
        "add_proposal".to_string(),
        arguments,
        amount.0,
        Gas(gas.0),
      )
      .then(callback)
      .into()
  }

  pub(crate) fn internal_check_bounty_payout_proposal(
    id: BountyIndex,
    receiver_id: AccountId,
    bounty: &mut Bounty,
    claim_idx: usize,
    claims: &mut Vec<BountyClaim>,
  ) -> PromiseOrValue<()> {
    if let Reviewers::ValidatorsDao {validators_dao} = bounty.reviewers.clone().unwrap() {
      let proposal_id = claims[claim_idx].bounty_payout_proposal_id.unwrap();
      Self::internal_check_proposal(
        validators_dao.account_id,
        proposal_id,
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_CHECK_BOUNTY_PAYOUT_PROPOSAL)
          .after_check_bounty_payout_proposal(id, receiver_id, bounty, claims, claim_idx)
      )
    } else {
      unreachable!();
    }
  }

  pub(crate) fn internal_check_approve_claimer_proposal(
    id: BountyIndex,
    claimer: AccountId,
    bounty: Bounty,
    claim_idx: usize,
    claims: &mut Vec<BountyClaim>,
  ) -> PromiseOrValue<()> {
    if let Reviewers::ValidatorsDao {validators_dao} = bounty.reviewers.clone().unwrap() {
      let proposal_id = claims[claim_idx].approve_claimer_proposal_id.unwrap();
      Self::internal_check_proposal(
        validators_dao.account_id,
        proposal_id,
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_CHECK_APPROVE_CLAIMER_PROPOSAL)
          .after_check_approve_claimer_proposal(id, claimer, bounty, claims, claim_idx)
      )
    } else {
      unreachable!();
    }
  }

  pub(crate) fn internal_check_proposal(
    receiver: AccountId,
    proposal_id: U64,
    callback: Promise,
  ) -> PromiseOrValue<()> {
    Promise::new(receiver)
      .function_call(
        "get_proposal".to_string(),
        json!({"id": proposal_id.0}).to_string().into_bytes(),
        NO_DEPOSIT,
        GAS_FOR_CHECK_PROPOSAL,
      )
      .then(callback)
      .into()
  }

  pub(crate) fn internal_create_dispute(
    &self,
    id: BountyIndex,
    receiver_id: &AccountId,
    bounty: Bounty,
    claim_idx: usize,
    claims: &mut Vec<BountyClaim>,
    description: String,
  ) -> PromiseOrValue<()> {
    let project_owner_delegate = bounty.get_bounty_owner_delegate();
    let dispute_create = DisputeCreate {
      bounty_id: id.clone(),
      description,
      claimer: receiver_id.clone(),
      project_owner_delegate,
    };

    Promise::new(self.dispute_contract.clone().unwrap())
      .function_call(
        "create_dispute".to_string(),
        json!({
          "dispute_create": dispute_create,
        })
          .to_string()
          .into_bytes(),
        ONE_YOCTO,
        GAS_FOR_CREATE_DISPUTE,
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_CREATE_DISPUTE)
          .after_create_dispute(receiver_id, claims, claim_idx)
      )
      .into()
  }

  pub(crate) fn internal_find_claimer(
    &self,
    id: BountyIndex,
  ) -> Option<AccountId> {
    let claims = self.get_bounty_claims_by_id(id.clone());
    for claim in claims {
      if matches!(claim.1.status, ClaimStatus::Disputed) {
        return Some(claim.0.clone())
      }
    }
    None
  }

  pub(crate) fn internal_get_dispute(
    &self,
    id: BountyIndex,
    receiver_id: AccountId,
    bounty: &mut Bounty,
    claim_idx: usize,
    claims: &mut Vec<BountyClaim>,
  ) -> PromiseOrValue<()> {
    Promise::new(self.dispute_contract.clone().unwrap())
      .function_call(
        "get_dispute".to_string(),
        json!({
          "id": claims[claim_idx].dispute_id.unwrap().0,
        })
          .to_string()
          .into_bytes(),
        NO_DEPOSIT,
        GAS_FOR_CHECK_DISPUTE,
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_CHECK_DISPUTE)
          .after_get_dispute(id, receiver_id, bounty, claims, claim_idx)
      )
      .into()
  }

  pub(crate) fn internal_update_statistic(
    &self,
    claimer: Option<AccountId>,
    bounty_owner: Option<AccountId>,
    action_kind: ReputationActionKind,
  ) -> PromiseOrValue<()> {
    if self.reputation_contract.is_some() {
      Promise::new(self.reputation_contract.clone().unwrap())
        .function_call(
          "emit".to_string(),
          json!({
            "claimer": claimer,
            "bounty_owner": bounty_owner,
            "action_kind": action_kind,
          })
            .to_string()
            .into_bytes(),
          NO_DEPOSIT,
          GAS_FOR_UPDATE_STATISTIC,
        )
        .into()
    } else {
      PromiseOrValue::Value(())
    }
  }

  pub(crate) fn get_configuration_dictionary(
    &mut self,
    dict: ReferenceType,
    entry: Option<String>,
    entries: Option<Vec<String>>
  ) -> (&mut Vec<String>, Vec<String>) {
    let config = self.config.to_config_mut();
    let (name_entry, name_entries, reference) = match dict {
      ReferenceType::Categories => { ("category", "categories", config.categories.as_mut()) },
      _ => { ("tag", "tags", config.tags.as_mut()) },
    };

    let entries = if let Some(entries) = entries {
      entries
    } else {
      assert!(entry.is_some(), "Expected either {} or {}", name_entry, name_entries);
      vec![entry.unwrap()]
    };

    (reference, entries)
  }

  pub(crate) fn internal_claimer_approval(
    &mut self,
    id: BountyIndex,
    bounty: Bounty,
    claim: &mut BountyClaim,
    claimer: &AccountId,
  ) {
    self.internal_change_status_and_save_bounty(&id, bounty.clone(), BountyStatus::Claimed);
    claim.start_time = Some(U64::from(env::block_timestamp()));
    claim.status = ClaimStatus::InProgress;
    self.internal_update_statistic(
      Some(claimer.clone()),
      Some(bounty.owner),
      ReputationActionKind::ClaimerApproved
    );
  }

  pub(crate) fn internal_get_ft_metadata(
    &self,
    token_id: AccountId,
    min_amount_for_kyc: Option<U128>,
  ) -> PromiseOrValue<()> {
    ext_ft_contract::ext(token_id.clone())
      .with_static_gas(GAS_FOR_GET_FT_METADATA)
      .ft_metadata()
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_GET_FT_METADATA)
          .after_get_ft_metadata(token_id, min_amount_for_kyc)
      )
      .into()
  }

  pub(crate) fn is_approval_required(&self, bounty: Bounty, claimer: &AccountId) -> bool {
    match bounty.claimer_approval {
      ClaimerApproval::MultipleClaims => true,
      ClaimerApproval::ApprovalWithWhitelist =>
        !self.is_claimer_whitelisted(bounty.owner, claimer),
      _ => false
    }
  }

  pub(crate) fn internal_create_claim(
    &mut self,
    id: BountyIndex,
    bounty: Bounty,
    claims: &mut Vec<BountyClaim>,
    claimer: AccountId,
    deadline: U64,
    description: String,
    proposal_id: Option<U64>,
  ) {
    let created_at = U64::from(env::block_timestamp());
    let mut bounty_claim = BountyClaim {
      bounty_id: id,
      created_at: created_at.clone(),
      start_time: None,
      deadline,
      description,
      status: ClaimStatus::New,
      bounty_payout_proposal_id: None,
      approve_claimer_proposal_id: proposal_id,
      rejected_timestamp: None,
      dispute_id: None,
    };

    if !self.is_approval_required(bounty.clone(), &claimer) {
      self.internal_claimer_approval(id, bounty.clone(), &mut bounty_claim, &claimer);
    }
    Self::internal_add_claim(id, claims, bounty_claim);
    self.internal_save_claims(&claimer, &claims);
    self.internal_add_bounty_claimer_account(id, claimer.clone());
    self.locked_amount += self.config.clone().to_config().bounty_claim_bond.0;
    self.internal_update_statistic(
      Some(claimer.clone()),
      Some(bounty.owner),
      ReputationActionKind::ClaimCreated
    );

    log!("Created new claim for bounty {} by applicant {}", id, claimer);
  }

  pub(crate) fn check_if_claimer_in_kyc_whitelist(
    &self,
    id: BountyIndex,
    bounty: Bounty,
    claims: &mut Vec<BountyClaim>,
    claimer: AccountId,
    deadline: U64,
    description: String,
  ) -> PromiseOrValue<()> {
    Promise::new(self.kyc_whitelist_contract.clone().unwrap())
      .function_call(
        "is_whitelisted".to_string(),
        json!({
          "account_id": claimer.clone(),
        })
          .to_string()
          .into_bytes(),
        NO_DEPOSIT,
        GAS_FOR_CHECK_IF_WHITELISTED,
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_CHECK_IF_WHITELISTED)
          .after_check_if_whitelisted(id, bounty, claims, claimer, deadline, description)
      )
      .into()
  }
}
