use std::collections::HashSet;

use crate::*;

impl BountiesContract {
  pub(crate) fn assert_that_token_is_allowed(&self, account_id: &AccountId) {
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

  pub(crate) fn assert_bounty_currency_is_correct(&self, currency: String) {
    assert!(
      self.config.clone().to_config().currencies.contains(&currency),
      "Invalid currency {}",
      currency
    );
  }

  pub(crate) fn assert_postpaid_is_ready(
    bounty: &Bounty,
    claim: &BountyClaim,
    approve: bool
  ) {
    if bounty.is_payment_outside_contract() {
      let payment_timestamps = Self::internal_get_payment_timestamps(bounty, claim);
      assert!(
        payment_timestamps.payment_at.is_none() || approve,
        "The result cannot be rejected after the payment has been confirmed"
      );
      assert!(
        !approve ||
          payment_timestamps.payment_at.is_some() &&
          payment_timestamps.payment_confirmed_at.is_some(),
        "No payment confirmation"
      );
    }
  }

  pub(crate) fn assert_multitasking_requirements(
    &self,
    id: BountyIndex,
    bounty: &Bounty,
    receiver_id: &AccountId,
    claim_number: Option<u8>,
  ) {
    if bounty.multitasking.is_some() {
      let multitasking = bounty.multitasking.clone().unwrap();
      match multitasking {
        Multitasking::ContestOrHackathon { successful_claims_for_result, .. } => {
          let deadline = bounty.deadline.clone();
          assert!(
            !matches!(deadline, Deadline::DueDate { .. }) ||
              env::block_timestamp() > deadline.get_deadline_value().0,
            "The winner of the competition can be determined only after the deadline."
          );
          assert!(
            multitasking.has_competition_started(),
            "The competition does not continue"
          );
          if successful_claims_for_result.is_some() {
            let claims = self.get_claims_with_statuses(id, vec![ClaimStatus::Completed], None);
            assert!(
              claims.len() as u16 >= successful_claims_for_result.unwrap(),
              "Not enough participants finished"
            );
          }
          if bounty.is_payment_outside_contract() {
            let competition_winner = multitasking.get_competition_winner();
            assert!(
              competition_winner.is_some() &&
                competition_winner.clone().unwrap().0 == receiver_id.clone() &&
                competition_winner.clone().unwrap().1 == claim_number,
              "Only the winner of the competition can be approved"
            );
          }
        },
        Multitasking::OneForAll { .. } => {},
        Multitasking::DifferentTasks { .. } =>
          env::panic_str("This action is not available for DifferentTasks mode"),
        _ => unreachable!(),
      }
    }
  }

  pub(crate) fn internal_add_bounty(&mut self, bounty: Bounty) -> BountyIndex {
    let id = self.last_bounty_id;
    self.internal_update_bounty(&id, bounty.clone());
    let mut indices = self
      .account_bounties
      .get(&bounty.owner)
      .unwrap_or_default();
    indices.push(id);
    self.internal_save_account_bounties(&bounty.owner, indices);
    self.last_bounty_id += 1;
    self.internal_total_fees_receiving_funds(&bounty, bounty.platform_fee, bounty.dao_fee);
    self.internal_update_statistic(
      None,
      Some(bounty.owner.clone()),
      ReputationActionKind::BountyCreated
    );
    id
  }

  pub(crate) fn internal_tag_filter(&self, bounty: &mut Bounty) {
    if bounty.metadata.tags.is_some() {
      let tags: Vec<String> = bounty.metadata
        .tags
        .clone()
        .unwrap()
        .into_iter()
        .filter(|t| self.config.clone().to_config().tags.contains(&t))
        .collect();
      bounty.metadata.tags = if tags.len() > 0 { Some(tags) } else { None };
    }
  }

  pub(crate) fn internal_create_bounty(
    &mut self,
    bounty_create: BountyCreate,
    payer_id: &AccountId,
    token_id: Option<AccountId>,
    amount: U128
  ) {
    let mut bounty = bounty_create.to_bounty(
      payer_id,
      token_id,
      amount,
      self.config.clone().to_config()
    );
    self.internal_tag_filter(&mut bounty);
    self.check_bounty(&bounty);
    let index = self.internal_add_bounty(bounty);
    log!(
          "Created new bounty for {} with index {}",
          payer_id,
          index
        );
  }

  pub(crate) fn internal_total_fees_receiving_funds(
    &mut self,
    bounty: &Bounty,
    platform_fee: U128,
    dao_fee: U128,
  ) {
    if bounty.token.is_some() {
      let token_id = &bounty.token.clone().unwrap();
      let mut total_fees = self.total_fees.get(token_id).unwrap();
      total_fees.receiving_commission(&platform_fee);
      self.total_fees.insert(token_id, &total_fees);
    }

    let dao_fee_stats = self.internal_get_dao_fee_stats(bounty);
    if dao_fee_stats.is_some() {
      let (dao_account_id, mut stats, stats_idx) = dao_fee_stats.unwrap();
      stats[stats_idx].fee_stats.receiving_commission(&dao_fee);
      self.total_validators_dao_fees.insert(&dao_account_id, &stats);
    }
  }

  pub(crate) fn internal_get_dao_fee_stats(
    &self,
    bounty: &Bounty,
  ) -> Option<(AccountId, Vec<DaoFeeStats>, usize)> {
    if bounty.reviewers.is_some() && bounty.token.is_some() {
      match bounty.reviewers.clone().unwrap() {
        Reviewers::ValidatorsDao { validators_dao } => {
          let mut stats = self.get_total_validators_dao_fees(validators_dao.clone().account_id);
          let stats_idx = Self::internal_find_dao_fee_stats(bounty.token.clone().unwrap(), &mut stats);
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
    platform_fee: Option<U128>,
    dao_fee: Option<U128>,
  ) {
    if bounty.token.is_some() {
      let token_id = &bounty.token.clone().unwrap();
      let total_fees = self.total_fees.get(token_id).unwrap();
      total_fees.assert_locked_amount_greater_than_or_equal_transaction_amount(
        &platform_fee.unwrap_or(bounty.platform_fee)
      );
    }
    let dao_fee_stats = self.internal_get_dao_fee_stats(bounty);
    if dao_fee_stats.is_some() {
      let (_, stats, stats_idx) = dao_fee_stats.unwrap();
      stats[stats_idx].fee_stats.assert_locked_amount_greater_than_or_equal_transaction_amount(
          &dao_fee.unwrap_or(bounty.dao_fee)
      );
    }
  }

  pub(crate) fn internal_total_fees_refunding_funds(&mut self, bounty: &Bounty) {
    let amounts = self.internal_get_bounty_amount_to_return(bounty);

    if bounty.token.is_some() {
      let token_id = &bounty.token.clone().unwrap();
      let mut total_fees = self.total_fees.get(token_id).unwrap();
      total_fees.refund_commission(&amounts.1, &amounts.2);
      self.total_fees.insert(token_id, &total_fees);
    }

    let dao_fee_stats = self.internal_get_dao_fee_stats(bounty);
    if dao_fee_stats.is_some() {
      let (dao_account_id, mut stats, stats_idx) = dao_fee_stats.unwrap();
      stats[stats_idx].fee_stats.refund_commission(&amounts.3, &amounts.4);
      self.total_validators_dao_fees.insert(&dao_account_id, &stats);
    }
  }

  pub(crate) fn internal_total_fees_unlocking_funds(
    &mut self,
    bounty: &Bounty,
    platform_fee: Option<U128>,
    dao_fee: Option<U128>,
  ) {
    if bounty.token.is_some() {
      let token_id = &bounty.token.clone().unwrap();
      let mut total_fees = self.total_fees.get(token_id).unwrap();
      total_fees.commission_unlocking(&platform_fee.unwrap_or(bounty.platform_fee));
      self.total_fees.insert(token_id, &total_fees);
    }

    let dao_fee_stats = self.internal_get_dao_fee_stats(bounty);
    if dao_fee_stats.is_some() {
      let (dao_account_id, mut stats, stats_idx) = dao_fee_stats.unwrap();
      stats[stats_idx].fee_stats.commission_unlocking(&dao_fee.unwrap_or(bounty.dao_fee));
      self.total_validators_dao_fees.insert(&dao_account_id, &stats);
    }
  }

  pub(crate) fn internal_get_bounty_amount_to_return(
    &self,
    bounty: &Bounty
  ) -> (U128, U128, U128, U128, U128) {
    if bounty.is_payment_outside_contract() {
      return (U128(0), U128(0), U128(0), U128(0), U128(0));
    }

    let (amount, platform_fee, dao_fee) = if bounty.is_one_bounty_for_many_claimants() &&
      bounty.status == BountyStatus::AwaitingClaims
    {
      let multitasking = bounty.multitasking.clone().unwrap();
      match multitasking {
        Multitasking::OneForAll { number_of_slots, .. } => {
          let (_, paid_slots) = multitasking.get_one_for_all_env();
          let remainder = number_of_slots - paid_slots;
          (
            bounty.amount.0 / number_of_slots as u128 * remainder as u128,
            bounty.platform_fee.0 / number_of_slots as u128 * remainder as u128,
            bounty.dao_fee.0 / number_of_slots as u128 * remainder as u128
          )
        }
        _ => unreachable!(),
      }
    } else {
      (bounty.amount.0, bounty.platform_fee.0, bounty.dao_fee.0)
    };

    let config = self.config.clone().to_config();
    let penalty_platform_fee: u128 = if config.platform_fee_percentage != 0 {
      platform_fee *
        config.penalty_platform_fee_percentage as u128 /
        config.platform_fee_percentage as u128
    } else { 0 };
    let penalty_validators_dao_fee: u128 = if config.validators_dao_fee_percentage != 0 {
      dao_fee *
        config.penalty_validators_dao_fee_percentage as u128 /
        config.validators_dao_fee_percentage as u128
    } else { 0 };
    let amount_to_return = amount + platform_fee - penalty_platform_fee +
      dao_fee - penalty_validators_dao_fee;
    (
      U128(amount_to_return),
      U128(platform_fee),
      U128(penalty_platform_fee),
      U128(dao_fee),
      U128(penalty_validators_dao_fee)
    )
  }

  pub(crate) fn internal_get_bounty_amount_for_payment(
    bounty: &Bounty
  ) -> (U128, U128, U128) {
    if bounty.is_payment_outside_contract() {
      return (U128(0), U128(0), U128(0));
    }
    if bounty.is_one_bounty_for_many_claimants() {
      let multitasking = bounty.multitasking.clone().unwrap();
      match multitasking {
        Multitasking::OneForAll { number_of_slots, .. } => {
          let (occupied_slots, paid_slots) = multitasking.get_one_for_all_env();

          let one_slot_amount = bounty.amount.0 / number_of_slots as u128;
          let one_slot_platform_fee = bounty.platform_fee.0 / number_of_slots as u128;
          let one_slot_dao_fee = bounty.dao_fee.0 / number_of_slots as u128;
          let second_to_last = number_of_slots - 1;

          // There may be rounding errors for the last slot
          if occupied_slots == 1 && paid_slots == second_to_last {
            (
              U128(bounty.amount.0 - one_slot_amount * second_to_last as u128),
              U128(bounty.platform_fee.0 - one_slot_platform_fee * second_to_last as u128),
              U128(bounty.dao_fee.0 - one_slot_dao_fee * second_to_last as u128)
            )
          } else {
            (U128(one_slot_amount), U128(one_slot_platform_fee), U128(one_slot_dao_fee))
          }
        },
        _ => unreachable!(),
      }
    } else {
      (bounty.amount, bounty.platform_fee, bounty.dao_fee)
    }
  }

  pub(crate) fn internal_get_partial_amount(
    &self,
    id: BountyIndex,
    bounty: &Bounty,
    receiver_id: AccountId,
    claim_number: Option<u8>,
    slot: usize,
  ) -> U128 {
    let multitasking = bounty.multitasking.clone().unwrap();
    match multitasking.clone() {
      Multitasking::DifferentTasks { subtasks, .. } => {
        let incomplete_slots = self.get_claims_with_statuses(
          id,
          vec![ClaimStatus::Completed, ClaimStatus::CompletedWithDispute],
          Some((receiver_id, claim_number))
        );
        let participants = multitasking.get_different_tasks_env().participants;
        let one_slot_amount = bounty.amount.0 * subtasks[slot].subtask_percent as u128 / 100_000;

        let amount = if incomplete_slots.len() == 0 {
          let mut other_slots: u128 = 0;
          (0..subtasks.len()).into_iter().for_each(|s| {
            if participants[s].is_none() {
              other_slots += bounty.amount.0 * subtasks[s].subtask_percent as u128 / 100_000;
            }
          });
          bounty.amount.0 - other_slots
        } else {
          one_slot_amount
        };
        U128(amount)
      },
      _ => unreachable!()
    }
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

  pub(crate) fn internal_get_claims_by_account_id_an_bounty_id(
    &self,
    id: &BountyIndex,
    receiver_id: &AccountId,
    no_panic: bool,
  ) -> Vec<ClaimIndex> {
    let account_claims: HashSet<ClaimIndex> = self.bounty_claimants
      .get(receiver_id)
      .unwrap_or_default()
      .into_iter()
      .collect();
    assert!(no_panic || account_claims.len() > 0, "No claimant found");

    let bounty_claims: HashSet<ClaimIndex> = self.bounty_claims
      .get(id)
      .unwrap_or_default()
      .into_iter()
      .collect();

    let mut claims = account_claims
      .intersection(&bounty_claims)
      .map(|c| c.clone())
      .collect::<Vec<_>>();
    claims.sort();

    claims
  }

  pub(crate) fn internal_find_claim(
    &self,
    claims: &Vec<ClaimIndex>,
    id: BountyIndex,
    receiver_id: AccountId,
    claim_number: Option<u8>
  ) -> Option<(ClaimIndex, BountyClaim)> {
    let claim_index: ClaimIndex;

    if claim_number.is_some() {
      let some_number = claim_number.unwrap() as usize;
      if claims.len() <= some_number {
        return None;
      }
      claim_index = claims[some_number];
    } else {
      if claims.len() != 1 {
        return None;
      }
      claim_index = claims[0];
    }

    let claim = self.claims.get(&claim_index);
    if claim.is_none() {
      return None;
    }

    let bounty_claim: BountyClaim = claim.unwrap().into();
    if bounty_claim.owner != receiver_id ||
      bounty_claim.claim_number != claim_number ||
      bounty_claim.bounty_id != id
    {
      return None;
    }
    Some((claim_index, bounty_claim))
  }

  pub(crate) fn internal_get_claim(
    &self,
    id: BountyIndex,
    receiver_id: AccountId,
    claim_number: Option<u8>
  ) -> (ClaimIndex, BountyClaim) {
    let claims = self.internal_get_claims_by_account_id_an_bounty_id(&id, &receiver_id, false);
    let claim = self.internal_find_claim(&claims, id, receiver_id, claim_number);
    claim.expect("No bounty claim found")
  }

  pub(crate) fn internal_get_payment_timestamps(
    bounty: &Bounty,
    claim: &BountyClaim,
  ) -> PaymentTimestamps {
    if bounty.is_one_bounty_for_many_claimants() || bounty.is_different_tasks() {
      claim.payment_timestamps.clone().unwrap_or_default()
    } else {
      bounty.postpaid.clone().unwrap().get_payment_timestamps()
    }
  }

  pub(crate) fn internal_return_bonds(
    &mut self,
    receiver_id: &AccountId,
    bond: Option<U128>
  ) -> PromiseOrValue<()> {
    let bond = bond.unwrap_or(DEFAULT_BOUNTY_CLAIM_BOND);
    if bond.0 == 0 {
      PromiseOrValue::Value(())
    } else {
      self.locked_amount -= bond.0;
      Promise::new(receiver_id.clone()).transfer(bond.0).into()
    }
  }

  pub(crate) fn internal_unlock_non_refunded_bond(&mut self, bond: Option<U128>) {
    let bond = bond.unwrap_or(DEFAULT_BOUNTY_CLAIM_BOND);
    if bond.0 != 0 {
      self.locked_amount -= bond.0;
      self.unlocked_amount += bond.0;
    }
  }

  pub(crate) fn internal_reset_bounty_to_initial_state(
    &mut self,
    id: BountyIndex,
    receiver_id: &AccountId,
    bounty: &mut Bounty,
    claim_id: ClaimIndex,
    claim: &mut BountyClaim,
    claim_status: Option<ClaimStatus>,
    return_bond: bool,
  ) -> PromiseOrValue<()> {
    let old_status = bounty.status.clone();
    if bounty.multitasking.is_none() {
      bounty.status = BountyStatus::New;
    } else {
      match bounty.multitasking.clone().unwrap() {
        Multitasking::ContestOrHackathon { .. } => {
          self.internal_participants_decrement(bounty);
          if bounty.status == BountyStatus::ManyClaimed &&
            bounty.multitasking.clone().unwrap().get_participants() == 0
          {
            self.internal_finish_competition(bounty, None);
            bounty.status = BountyStatus::New;
          }
        },
        Multitasking::OneForAll { .. } => {
          self.internal_occupied_slots_decrement(bounty);
          let env = bounty.multitasking.clone().unwrap().get_one_for_all_env();
          if bounty.status == BountyStatus::ManyClaimed && env.0 == 0 {
            bounty.status = if env.1 == 0 {
              BountyStatus::New
            } else {
              BountyStatus::AwaitingClaims
            };
          }
        },
        Multitasking::DifferentTasks { .. } => {
          let slot = claim.slot.clone().unwrap();
          self.internal_reset_slot(bounty, slot);
          if bounty.multitasking.clone().unwrap().are_all_slots_available() {
            bounty.status = BountyStatus::New;
          }
        },
        _ => unreachable!(),
      }
    }

    self.internal_update_bounty(&id, bounty.clone());
    self.internal_claim_closure(
      receiver_id,
      bounty.owner.clone(),
      old_status,
      claim_id,
      claim,
      claim_status,
      return_bond
    )
  }

  pub(crate) fn internal_claim_closure(
    &mut self,
    receiver_id: &AccountId,
    owner: AccountId,
    old_status: BountyStatus,
    claim_id: ClaimIndex,
    claim: &mut BountyClaim,
    claim_status: Option<ClaimStatus>,
    return_bond: bool,
  ) -> PromiseOrValue<()> {
    let with_dispute = claim.status == ClaimStatus::Disputed;
    let is_new = claim.status == ClaimStatus::New ||
      (claim.status == ClaimStatus::Competes ||
        claim.status == ClaimStatus::ReadyToStart) &&
        [BountyStatus::New, BountyStatus::Canceled].contains(&old_status);

    let new_status = if claim_status.is_none() {
      ClaimStatus::NotCompleted
    } else {
      claim_status.unwrap()
    };

    let action_kind = match new_status {
      ClaimStatus::NotCompleted => ReputationActionKind::UnsuccessfulClaim { with_dispute },
      ClaimStatus::Expired => ReputationActionKind::ClaimExpired,
      ClaimStatus::Canceled => ReputationActionKind::ClaimCancelled,
      _ => unreachable!(),
    };

    let bounty_owner = if new_status == ClaimStatus::NotCompleted {
      Some(owner)
    } else {
      None
    };

    claim.status = new_status;
    self.claims.insert(&claim_id, &claim.clone().into());
    if !is_new || new_status != ClaimStatus::Canceled {
      self.internal_update_statistic(Some(receiver_id.clone()), bounty_owner, action_kind);
    }

    if return_bond {
      self.internal_return_bonds(receiver_id, claim.bond)
    } else {
      self.internal_unlock_non_refunded_bond(claim.bond);
      PromiseOrValue::Value(())
    }
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
    bounty: &mut Bounty,
    claim_id: ClaimIndex,
    claim: &mut BountyClaim
  ) -> PromiseOrValue<()> {
    self.internal_reset_bounty_to_initial_state(
      id,
      receiver_id,
      bounty,
      claim_id,
      claim,
      Some(ClaimStatus::Expired),
      false
    )
  }

  pub(crate) fn internal_update_bounty(
    &mut self,
    id: &BountyIndex,
    bounty: Bounty,
  ) {
    self.bounties.insert(&id, &bounty.clone().into());
  }

  pub(crate) fn internal_change_status_and_save_bounty(
    &mut self,
    id: &BountyIndex,
    bounty: &mut Bounty,
    status: BountyStatus,
  ) {
    bounty.status = status;
    self.internal_update_bounty(&id, bounty.clone());
  }

  pub(crate) fn is_claim_active(claim: &BountyClaim) -> bool {
    claim.status == ClaimStatus::InProgress
      || claim.status == ClaimStatus::Completed
      || claim.status == ClaimStatus::Rejected
      || claim.status == ClaimStatus::Disputed
  }

  pub(crate) fn internal_find_active_claim(
    &self,
    id: BountyIndex,
  ) -> (ClaimIndex, BountyClaim) {
    let claim_ids = self.bounty_claims.get(&id).unwrap_or_default();

    for claim_id in claim_ids {
      let bounty_claim = self.get_bounty_claim(claim_id);
      if Self::is_claim_active(&bounty_claim) {
        return (claim_id, bounty_claim);
      }
    }

    env::panic_str("No active claims");
  }

  pub(crate) fn internal_bounty_payout(
    &mut self,
    id: BountyIndex,
    claimant: Option<(AccountId, Option<u8>)>,
  ) -> PromiseOrValue<()> {
    let bounty = self.get_bounty(id);
    let amounts = Self::internal_get_bounty_amount_for_payment(&bounty);
    self.assert_locked_amount_greater_than_or_equal_transaction_amount(
      &bounty,
      Some(amounts.1),
      Some(amounts.2)
    );

    if bounty.is_payment_outside_contract() || bounty.is_different_tasks() {
      self.internal_bounty_completion(id, bounty, claimant);
      return PromiseOrValue::Value(())
    }

    let (receiver_id, _) = claimant.clone().expect("Incorrect claimant parameter");
    ext_ft_contract::ext(bounty.token.clone().unwrap())
      .with_attached_deposit(ONE_YOCTO)
      .with_static_gas(GAS_FOR_FT_TRANSFER)
      .ft_transfer(
        receiver_id,
        amounts.0,
        Some(format!("Bounty {} payout", id)),
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_FT_TRANSFER)
          .after_ft_transfer(id, claimant)
      )
      .into()
  }

  pub(crate) fn internal_bounty_withdraw(
    &mut self,
    id: BountyIndex,
    bounty: Bounty,
    receiver_id: AccountId,
    claim_number: Option<u8>,
    slot: usize,
  ) -> PromiseOrValue<()> {
    let amount = self.internal_get_partial_amount(
      id,
      &bounty,
      receiver_id.clone(),
      claim_number,
      slot
    );

    if bounty.is_payment_outside_contract() {
      self.internal_slot_finalize(id, bounty, receiver_id, claim_number, slot);
      return PromiseOrValue::Value(())
    }

    ext_ft_contract::ext(bounty.token.clone().unwrap())
      .with_attached_deposit(ONE_YOCTO)
      .with_static_gas(GAS_FOR_FT_TRANSFER)
      .ft_transfer(
        receiver_id.clone(),
        amount,
        Some(format!("Bounty {} payment for {}", id, receiver_id)),
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_FT_TRANSFER)
          .after_bounty_withdraw(id, receiver_id, claim_number, slot)
      )
      .into()
  }

  pub(crate) fn internal_claim_return_after_dispute(
    &mut self,
    claim_id: ClaimIndex,
    claim: &mut BountyClaim
  ) -> PromiseOrValue<()> {
    claim.status = ClaimStatus::CompletedWithDispute;
    self.claims.insert(&claim_id, &claim.clone().into());
    PromiseOrValue::Value(())
  }

  pub(crate) fn internal_slot_finalize(
    &mut self,
    id: BountyIndex,
    mut bounty: Bounty,
    receiver_id: AccountId,
    claim_number: Option<u8>,
    slot: usize,
  ) {
    let (claim_id, mut bounty_claim) = self.internal_get_claim(id, receiver_id.clone(), claim_number);
    let with_dispute = bounty_claim.status == ClaimStatus::CompletedWithDispute;
    assert!(
      bounty_claim.status == ClaimStatus::Completed || with_dispute,
      "The claim status does not allow this action"
    );

    bounty_claim.status = ClaimStatus::Approved;
    self.claims.insert(&claim_id, &bounty_claim.clone().into());

    self.internal_reset_slot(&mut bounty, slot);
    self.internal_update_bounty(&id, bounty.clone());

    self.internal_update_statistic(
      Some(receiver_id.clone()),
      Some(bounty.owner),
      ReputationActionKind::SuccessfulClaim { with_dispute },
    );
    self.internal_return_bonds(&receiver_id, bounty_claim.bond);
  }

  pub(crate) fn internal_bounty_cancellation(
    &mut self,
    id: BountyIndex,
    mut bounty: Bounty,
  ) {
    self.internal_total_fees_refunding_funds(&bounty);
    let new_status = if bounty.status == BountyStatus::AwaitingClaims {
      BountyStatus::PartiallyCompleted
    } else {
      BountyStatus::Canceled
    };
    self.internal_change_status_and_save_bounty(&id, &mut bounty, new_status.clone());
    self.internal_update_statistic(
      None,
      Some(bounty.owner),
      if new_status == BountyStatus::Canceled {
        ReputationActionKind::BountyCancelled
      } else {
        ReputationActionKind::SuccessfulBounty
      },
    );
  }

  pub(crate) fn internal_refund_bounty_amount(
    &mut self,
    id: BountyIndex,
    bounty: Bounty,
  ) -> PromiseOrValue<()> {
    let amounts = self.internal_get_bounty_amount_to_return(&bounty);
    self.assert_locked_amount_greater_than_or_equal_transaction_amount(
      &bounty,
      Some(amounts.1),
      Some(amounts.3)
    );

    if amounts.0.0 == 0 {
      self.internal_bounty_cancellation(id, bounty);
      return PromiseOrValue::Value(())
    }

    ext_ft_contract::ext(bounty.token.clone().unwrap())
      .with_attached_deposit(ONE_YOCTO)
      .with_static_gas(GAS_FOR_FT_TRANSFER)
      .ft_transfer(
        bounty.owner.clone(),
        amounts.0,
        Some(format!("Returning amount of bounty {} to {}", id, bounty.owner)),
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_FT_TRANSACT)
          .after_refund_bounty_amount(id)
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
    claim_id: ClaimIndex,
    claim: &mut BountyClaim,
  ) -> PromiseOrValue<()> {
    if self.dispute_contract.is_some() &&
      !bounty.is_payment_outside_contract() &&
      !bounty.is_contest_or_hackathon()
    {
      claim.status = ClaimStatus::Rejected;
      claim.rejected_timestamp = Some(env::block_timestamp().into());
      self.claims.insert(&claim_id, &claim.clone().into());
      PromiseOrValue::Value(())
    } else {
      // If the creation of a dispute is not foreseen,
      // then the bounty reset to initial state
      self.internal_reset_bounty_to_initial_state(
        id,
        &receiver_id,
        bounty,
        claim_id,
        claim,
        None,
        true
      )
    }
  }

  pub(crate) fn internal_add_proposal_to_finish_claim(
    &self,
    id: BountyIndex,
    receiver_id: AccountId,
    claim_number: Option<u8>,
    bounty: Bounty,
    place_of_check: PlaceOfCheckKYC,
  ) -> PromiseOrValue<()> {
    if let Reviewers::ValidatorsDao {validators_dao} = bounty.reviewers.clone().unwrap() {
      let description = match place_of_check {
        PlaceOfCheckKYC::ClaimDone { description } => description,
        _ => unreachable!(),
      };
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
                    "method_name": "bounty_approve",
                    "args": Base64VecU8::from(json!({
                      "id": id.clone(),
                      "receiver_id": receiver_id.to_string(),
                      "claim_number": claim_number,
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
          .on_added_proposal_callback(id, receiver_id, claim_number),
        validators_dao.add_proposal_bond,
        validators_dao.gas_for_add_proposal,
      )
    } else {
      unreachable!();
    }
  }

  pub(crate) fn internal_add_proposal_to_finish_several_claims(
    &self,
    id: BountyIndex,
    receiver_id: AccountId,
    claim_number: Option<u8>,
    bounty: Bounty,
    place_of_check: PlaceOfCheckKYC,
  ) -> PromiseOrValue<()> {
    if let Reviewers::ValidatorsDao {validators_dao} = bounty.reviewers.clone().unwrap() {
      let description = match place_of_check {
        PlaceOfCheckKYC::ClaimDone { description } => description,
        _ => unreachable!(),
      };
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
                    "method_name": "bounty_approve_of_several",
                    "args": Base64VecU8::from(json!({
                      "id": id.clone(),
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
          .on_added_proposal_callback(id, receiver_id, claim_number),
        validators_dao.add_proposal_bond,
        validators_dao.gas_for_add_proposal,
      )
    } else {
      unreachable!();
    }
  }

  pub(crate) fn internal_add_proposal_to_approve_claimant(
    &self,
    id: BountyIndex,
    receiver_id: AccountId,
    claim_number: Option<u8>,
    bounty: Bounty,
    deadline: Option<U64>,
    description: String,
    slot: Option<usize>,
  ) -> PromiseOrValue<()> {
    if let Reviewers::ValidatorsDao { validators_dao } = bounty.reviewers.clone().unwrap() {
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
                    "method_name": "accept_claimant",
                    "args": Base64VecU8::from(json!({
                      "id": id.clone(),
                      "receiver_id": receiver_id.clone(),
                      "claim_number": claim_number,
                    })
                      .to_string()
                      .into_bytes()
                      .to_vec()),
                    "deposit": "1",
                    "gas": validators_dao.gas_for_claimant_approval,
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
          .after_add_proposal(id, receiver_id, deadline, description, slot),
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
    &self,
    id: BountyIndex,
    bounty: Bounty,
    claimant: Option<(AccountId, Option<u8>)>,
    proposal_id: U64,
  ) -> PromiseOrValue<()> {
    if let Reviewers::ValidatorsDao {validators_dao} = bounty.reviewers.clone().unwrap() {
      Self::internal_check_proposal(
        validators_dao.account_id,
        proposal_id,
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_CHECK_BOUNTY_PAYOUT_PROPOSAL)
          .after_check_bounty_payout_proposal(id, claimant, proposal_id)
      )
    } else {
      unreachable!();
    }
  }

  pub(crate) fn internal_check_approve_claimant_proposal(
    &self,
    id: BountyIndex,
    bounty: &Bounty,
    receiver_id: AccountId,
    claim_number: Option<u8>,
    proposal_id: U64,
  ) -> PromiseOrValue<()> {
    if let Reviewers::ValidatorsDao {validators_dao} = bounty.reviewers.clone().unwrap() {
      Self::internal_check_proposal(
        validators_dao.account_id,
        proposal_id,
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_CHECK_APPROVE_CLAIMANT_PROPOSAL)
          .after_check_approve_claimant_proposal(id, receiver_id, claim_number, proposal_id)
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
    claim_number: Option<u8>,
    bounty: Bounty,
    description: String,
  ) -> PromiseOrValue<()> {
    let project_owner_delegate = bounty.get_bounty_owner_delegate();
    let dispute_create = DisputeCreate {
      bounty_id: id.clone(),
      description,
      receiver_id: receiver_id.clone(),
      project_owner_delegate,
      claim_number,
    };

    Promise::new(self.dispute_contract.clone().unwrap())
      .function_call(
        "create_dispute".to_string(),
        json!({
          "dispute_create": dispute_create,
        })
          .to_string()
          .into_bytes(),
        NO_DEPOSIT,
        GAS_FOR_CREATE_DISPUTE,
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_CREATE_DISPUTE)
          .after_create_dispute(id, receiver_id, claim_number)
      )
      .into()
  }

  pub(crate) fn internal_get_dispute(
    &self,
    id: BountyIndex,
    receiver_id: AccountId,
    claim_number: Option<u8>,
    dispute_id: U64,
  ) -> PromiseOrValue<()> {
    Promise::new(self.dispute_contract.clone().unwrap())
      .function_call(
        "get_dispute".to_string(),
        json!({
          "id": dispute_id.0,
        })
          .to_string()
          .into_bytes(),
        NO_DEPOSIT,
        GAS_FOR_CHECK_DISPUTE,
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_CHECK_DISPUTE)
          .after_get_dispute(id, receiver_id, claim_number)
      )
      .into()
  }

  pub(crate) fn internal_update_statistic(
    &self,
    receiver_id: Option<AccountId>,
    bounty_owner: Option<AccountId>,
    action_kind: ReputationActionKind,
  ) -> PromiseOrValue<()> {
    if self.reputation_contract.is_some() {
      Promise::new(self.reputation_contract.clone().unwrap())
        .function_call(
          "emit".to_string(),
          json!({
            "receiver_id": receiver_id,
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
      ReferenceType::Tags => { ("tag", "tags", config.tags.as_mut()) },
      _ => { ("currency", "currencies", config.currencies.as_mut()) },
    };

    let entries = if let Some(entries) = entries {
      entries
    } else {
      assert!(entry.is_some(), "Expected either {} or {}", name_entry, name_entries);
      vec![entry.unwrap()]
    };

    (reference, entries)
  }

  pub(crate) fn internal_get_one_page_of_claims(
    &self,
    claims: Vec<ClaimIndex>,
    from_index: usize,
    limit: usize
  ) -> Vec<(ClaimIndex, BountyClaim)> {
    (from_index..std::cmp::min(from_index + limit, claims.len()))
      .map(|c| (claims[c], self.get_bounty_claim(claims[c])))
      .collect()
  }

  pub(crate) fn internal_get_claims_by_bounty_id(&self, id: BountyIndex) -> Vec<(ClaimIndex, BountyClaim)> {
    self.bounty_claims
      .get(&id)
      .unwrap_or_default()
      .into_iter()
      .map(|c| (c, self.get_bounty_claim(c)))
      .collect()
  }

  pub(crate) fn get_claims_with_statuses(
    &self,
    id: BountyIndex,
    statuses: Vec<ClaimStatus>,
    except: Option<(AccountId, Option<u8>)>,
  ) -> Vec<(ClaimIndex, BountyClaim)> {
    self.internal_get_claims_by_bounty_id(id)
      .into_iter()
      .filter(
        |entry|
          statuses.contains(&entry.1.status) &&
            (
              except.is_none() ||
                entry.1.owner != except.clone().unwrap().0 ||
                  entry.1.claim_number != except.clone().unwrap().1
            )
      )
      .collect()
  }

  pub(crate) fn internal_start_competition(
    &mut self,
    bounty: &mut Bounty,
    start_time: Option<U64>
  ) {
    bounty.multitasking = Some(
      bounty.multitasking.clone().unwrap().set_competition_timestamp(start_time)
    );
  }

  pub(crate) fn internal_finish_competition(
    &mut self,
    bounty:
    &mut Bounty,
    winner: Option<(AccountId, Option<u8>)>
  ) {
    bounty.multitasking = Some(
      bounty.multitasking.clone().unwrap().set_competition_timestamp(None)
    );
    if winner.is_some() {
      bounty.multitasking = Some(
        bounty.multitasking.clone().unwrap().set_competition_winner(winner)
      );
    }
  }

  pub(crate) fn internal_participants_increment(&mut self, bounty: &mut Bounty) {
    let multitasking = bounty.multitasking.clone().unwrap();
    bounty.multitasking = Some(
      multitasking.clone().set_competition_participants(multitasking.clone().get_participants() + 1)
    );
  }

  pub(crate) fn internal_participants_decrement(&mut self, bounty: &mut Bounty) {
    let multitasking = bounty.multitasking.clone().unwrap();
    bounty.multitasking = Some(
      multitasking.clone().set_competition_participants(multitasking.clone().get_participants() - 1)
    );
  }

  pub(crate) fn internal_occupied_slots_increment(&mut self, bounty: &mut Bounty) {
    let multitasking = bounty.multitasking.clone().unwrap();
    let (occupied_slots, paid_slots) = multitasking.clone().get_one_for_all_env();
    bounty.multitasking = Some(
      multitasking.clone().set_one_for_all_env(occupied_slots + 1, paid_slots)
    );
  }

  pub(crate) fn internal_occupied_slots_decrement(&mut self, bounty: &mut Bounty) {
    let multitasking = bounty.multitasking.clone().unwrap();
    let (occupied_slots, paid_slots) = multitasking.clone().get_one_for_all_env();
    bounty.multitasking = Some(
      multitasking.clone().set_one_for_all_env(occupied_slots - 1, paid_slots)
    );
  }

  pub(crate) fn internal_paid_slots_increment(&mut self, bounty: &mut Bounty) {
    let multitasking = bounty.multitasking.clone().unwrap();
    let (occupied_slots, paid_slots) = multitasking.clone().get_one_for_all_env();
    bounty.multitasking = Some(
      multitasking.clone().set_one_for_all_env(occupied_slots - 1, paid_slots + 1)
    );
  }

  pub(crate) fn internal_set_slot_account(
    &mut self,
    bounty: &mut Bounty,
    slot: usize,
    account_id: AccountId,
    claim_number: Option<u8>
  ) {
    let mut multitasking = bounty.multitasking.clone().unwrap();
    multitasking.set_slot_env(
      slot,
      Some(SubtaskEnv {
        participant: account_id,
        claim_number,
        completed: false,
        confirmed: false
      })
    );
    bounty.multitasking = Some(multitasking);
  }

  pub(crate) fn internal_complete_slot(&mut self, bounty: &mut Bounty, slot: usize) {
    let mut multitasking = bounty.multitasking.clone().unwrap();
    let mut slot_env = multitasking.clone().get_slot_env(slot).expect("The slot is not occupied");
    slot_env.completed = true;
    multitasking.set_slot_env(slot, Some(slot_env));
    bounty.multitasking = Some(multitasking);
  }

  pub(crate) fn internal_confirm_slot(&mut self, bounty: &mut Bounty, slot: usize) {
    let mut multitasking = bounty.multitasking.clone().unwrap();
    let mut slot_env = multitasking.clone().get_slot_env(slot).expect("The slot is not occupied");
    slot_env.confirmed = true;
    multitasking.set_slot_env(slot, Some(slot_env));
    bounty.multitasking = Some(multitasking);
  }

  pub(crate) fn internal_reset_slot(&mut self, bounty: &mut Bounty, slot: usize) {
    let mut multitasking = bounty.multitasking.clone().unwrap();
    multitasking.set_slot_env(slot, None);
    bounty.multitasking = Some(multitasking);
  }

  pub(crate) fn internal_set_bounty_payout_proposal_id(
    &mut self,
    bounty: &mut Bounty,
    proposal_id: Option<U64>
  ) {
    if Self::internal_get_bounty_payout_proposal_id(bounty).is_none() {
      bounty.multitasking = Some(
        bounty.multitasking.clone().unwrap().set_bounty_payout_proposal_id(proposal_id)
      );
    }
  }

  pub(crate) fn internal_are_all_slots_complete(
    bounty: &Bounty,
    except: Option<(AccountId, Option<u8>)>
  ) -> bool {
    bounty.multitasking.clone().unwrap().are_all_slots_complete(except)
  }

  pub(crate) fn internal_get_bounty_payout_proposal_id(bounty: &Bounty) -> Option<U64> {
    bounty.multitasking.clone().unwrap().get_different_tasks_env().bounty_payout_proposal_id
  }

  pub(crate) fn internal_claimant_approval(
    &mut self,
    id: BountyIndex,
    bounty: &mut Bounty,
    claim: &mut BountyClaim,
    is_kyc_delayed: Option<DefermentOfKYC>,
  ) {
    let approved_status = if bounty.bounty_flow == BountyFlow::SimpleBounty {
      ClaimStatus::Completed
    } else {
      if bounty.is_contest_or_hackathon() {
        ClaimStatus::Competes
      } else {
        ClaimStatus::InProgress
      }
    };
    let start_time = Some(U64::from(env::block_timestamp()));

    if bounty.multitasking.is_none() {
      bounty.status = BountyStatus::Claimed;
      claim.status = approved_status;

    } else {
      let multitasking = bounty.multitasking.clone().unwrap();
      assert!(multitasking.is_allowed_to_create_or_approve_claims(claim.slot.clone()),
        "It is no longer possible to approve the claim"
      );

      match multitasking.clone() {
        Multitasking::ContestOrHackathon { start_conditions, .. } => {
          let started = multitasking.clone().has_competition_started();

          if start_conditions.is_none() {
            if !started {
              bounty.status = BountyStatus::ManyClaimed;
            }
          } else {
            match start_conditions.clone().unwrap() {
              StartConditions::MinAmountToStart { amount } => {
                if !started && multitasking.clone().get_participants() == amount as u32 - 1 {
                  bounty.status = BountyStatus::ManyClaimed;
                }
              },
              _ => {},
            }
          }

          self.internal_participants_increment(bounty);
          claim.status = approved_status;
          if !started && bounty.status == BountyStatus::ManyClaimed {
            self.internal_start_competition(bounty, start_time);
          }
        },

        Multitasking::OneForAll { min_slots_to_start, .. } => {
          let started = bounty.status == BountyStatus::ManyClaimed;
          let paused = bounty.status == BountyStatus::AwaitingClaims;

          if min_slots_to_start.is_none() {
            if !started {
              bounty.status = BountyStatus::ManyClaimed;
            }
            claim.status = approved_status;

          } else {
            if started || paused {
              if paused {
                bounty.status = BountyStatus::ManyClaimed;
              }
              claim.status = approved_status;

            } else {
              let claims = self.get_claims_with_statuses(
                id,
                vec![ClaimStatus::ReadyToStart],
                Some((claim.owner.clone(), claim.claim_number))
              );

              if claims.len() as u16 == min_slots_to_start.unwrap() - 1 {
                bounty.status = BountyStatus::ManyClaimed;
                claim.status = approved_status;
                self.internal_update_status_of_many_claims(
                  bounty,
                  claims,
                  vec![ClaimStatus::ReadyToStart],
                  approved_status,
                  start_time,
                );

              } else {
                claim.status = ClaimStatus::ReadyToStart;
              }
            }
          }

          self.internal_occupied_slots_increment(bounty);
        },

        Multitasking::DifferentTasks { .. } => {
          self.internal_set_slot_account(
            bounty,
            claim.slot.clone().unwrap(),
            claim.owner.clone(),
            claim.claim_number
          );
          let started = bounty.status == BountyStatus::ManyClaimed;

          if started {
            claim.status = approved_status;

          } else {
            if bounty.multitasking.clone().unwrap().are_all_slots_taken() {
              bounty.status = BountyStatus::ManyClaimed;
              claim.status = approved_status;

              let claims = self.get_claims_with_statuses(
                id,
                vec![ClaimStatus::ReadyToStart],
                Some((claim.owner.clone(), claim.claim_number))
              );
              self.internal_update_status_of_many_claims(
                bounty,
                claims,
                vec![ClaimStatus::ReadyToStart],
                approved_status,
                start_time,
              );
            } else {
              claim.status = ClaimStatus::ReadyToStart;
            }
          }

          if bounty.bounty_flow == BountyFlow::SimpleBounty && claim.status == approved_status {
            self.internal_complete_slot(bounty, claim.slot.clone().unwrap());
          }
        },
        _ => unreachable!(),
      }
    }

    self.internal_update_bounty(&id, bounty.clone());
    if claim.status == approved_status &&
      (bounty.status == BountyStatus::ManyClaimed || bounty.status == BountyStatus::Claimed)
    {
      claim.start_time = start_time;
    }
    claim.is_kyc_delayed = is_kyc_delayed;

    self.internal_update_statistic(
      Some(claim.owner.clone()),
      Some(bounty.clone().owner),
      ReputationActionKind::ClaimantApproved
    );
  }

  pub(crate) fn internal_update_status_of_many_claims(
    &mut self,
    bounty: &mut Bounty,
    claims: Vec<(ClaimIndex, BountyClaim)>,
    old_statuses: Vec<ClaimStatus>,
    new_status: ClaimStatus,
    start_time: Option<U64>,
  ) {
    claims
      .into_iter()
      .for_each(|entry| {
        let claim_id = entry.0;
        let mut claim = entry.1;
        if old_statuses.contains(&claim.status) {
          claim.status = new_status;
          if start_time.is_some() {
            claim.start_time = start_time;
          }
          self.claims.insert(&claim_id, &claim.clone().into());
          if bounty.bounty_flow == BountyFlow::SimpleBounty &&
            bounty.is_different_tasks() &&
            new_status == ClaimStatus::Completed
          {
            self.internal_complete_slot(bounty, claim.slot.clone().unwrap());
          }
        }
      });
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

  pub(crate) fn is_kyc_check_required(
    &self,
    bounty: Bounty,
    claim: Option<&BountyClaim>,
    receiver_id: Option<AccountId>,
    place_of_check: PlaceOfCheckKYC,
  ) -> bool {
    match bounty.kyc_config {
      KycConfig::KycRequired { kyc_verification_method } => {
        assert!(
          self.kyc_whitelist_contract.is_some(),
          "KYC whitelist contract is not set"
        );
        match place_of_check {
          PlaceOfCheckKYC::CreatingClaim { .. } => {
            kyc_verification_method == KycVerificationMethod::WhenCreatingClaim
          },
          PlaceOfCheckKYC::DecisionOnClaim { is_kyc_delayed } => {
            kyc_verification_method == KycVerificationMethod::DuringClaimApproval &&
              is_kyc_delayed.is_none()
          },
          _ => {
            kyc_verification_method == KycVerificationMethod::DuringClaimApproval &&
              (
                claim.is_some()
                  && claim.unwrap().is_kyc_delayed.is_some()
                  && matches!(
                   claim.unwrap().is_kyc_delayed.clone().unwrap(),
                   DefermentOfKYC::BeforeDeadline
                 )
                  || receiver_id.is_some()
                  && !self.is_approval_required(&bounty, &receiver_id.unwrap())
              )
          }
        }
      },
      _ => false,
    }
  }

  pub(crate) fn is_approval_required(&self, bounty: &Bounty, receiver_id: &AccountId) -> bool {
    match bounty.claimant_approval.clone() {
      ClaimantApproval::MultipleClaims => true,
      ClaimantApproval::ApprovalByWhitelist { claimants_whitelist } =>
        !claimants_whitelist.contains(receiver_id),
      ClaimantApproval::WhitelistWithApprovals { .. } => true,
      ClaimantApproval::ApprovalWithWhitelist =>
        env::panic_str("ApprovalWithWhitelist is not supported"),
      _ => false
    }
  }

  pub(crate) fn internal_add_claim(&mut self, bounty_claim: &BountyClaim) {
    let claim_id = self.last_claim_id;
    let mut bounty_claimants = self.bounty_claimants.get(&bounty_claim.owner).unwrap_or_default();
    bounty_claimants.push(claim_id);
    self.bounty_claimants.insert(&bounty_claim.owner, &bounty_claimants);
    let mut bounty_claims = self.bounty_claims.get(&bounty_claim.bounty_id).unwrap_or_default();
    bounty_claims.push(claim_id);
    self.bounty_claims.insert(&bounty_claim.bounty_id, &bounty_claims);
    self.last_claim_id += 1;
    self.claims.insert(&claim_id, &bounty_claim.clone().into());
  }

  pub(crate) fn internal_create_claim(
    &mut self,
    id: BountyIndex,
    receiver_id: AccountId,
    claim_number: Option<u8>,
    deadline: Option<U64>,
    description: String,
    proposal_id: Option<U64>,
    slot: Option<usize>,
    bounty_and_claim: Option<(Bounty, Option<(ClaimIndex, BountyClaim)>)>,
  ) {
    let (mut bounty, claim, claim_number) = if bounty_and_claim.is_some() {
      let (bounty, claim) = bounty_and_claim.unwrap();
      (bounty, claim, claim_number)
    } else {
      self.check_if_allowed_to_create_claim_by_status(
        id,
        receiver_id.clone(),
        slot.clone(),
      )
    };

    let created_at = U64::from(env::block_timestamp());
    let bond = self.config.clone().to_config().bounty_claim_bond;
    let mut bounty_claim = BountyClaim {
      owner: receiver_id.clone(),
      bounty_id: id,
      created_at: created_at.clone(),
      start_time: None,
      deadline,
      description,
      status: ClaimStatus::New,
      bounty_payout_proposal_id: None,
      approve_claimant_proposal_id: proposal_id,
      rejected_timestamp: None,
      dispute_id: None,
      is_kyc_delayed: None,
      payment_timestamps: if bounty.is_payment_outside_contract() &&
        bounty.is_one_bounty_for_many_claimants() || bounty.is_different_tasks()
      {
        Some(PaymentTimestamps::default())
      } else {
        None
      },
      slot,
      bond: Some(bond),
      claim_number,
    };

    if !self.is_approval_required(&bounty, &receiver_id) {
      self.internal_claimant_approval(id, &mut bounty, &mut bounty_claim, None);
    }

    if claim.is_none() {
      self.internal_add_claim(&bounty_claim);
    } else {
      let claim_id = claim.unwrap().0;
      self.claims.insert(&claim_id, &bounty_claim.into());
    }
    self.locked_amount += bond.0;

    self.internal_update_statistic(
      Some(receiver_id.clone()),
      Some(bounty.owner.clone()),
      ReputationActionKind::ClaimCreated
    );

    log!("Created new claim for bounty {} by applicant {} #{:?}", id, receiver_id, claim_number);
  }

  pub(crate) fn check_if_allowed_to_create_claim_by_status(
    &self,
    id: BountyIndex,
    receiver_id: AccountId,
    slot: Option<usize>,
  ) -> (Bounty, Option<(ClaimIndex, BountyClaim)>, Option<u8>) {
    let bounty = self.get_bounty(id.clone());
    let bounty_statuses: Vec<BountyStatus>;
    let claim_statuses: Vec<ClaimStatus>;
    let claim_message: &str;
    let mut claim_number: Option<u8> = None;

    if bounty.multitasking.is_none() {
      assert!(slot.is_none(), "The slot parameter is not used for this mode");

      bounty_statuses = vec![BountyStatus::New];
      claim_statuses = vec![ClaimStatus::New];
      claim_message = "You already have a claim with the status 'New'";

    } else {
      if bounty.multitasking.clone().unwrap().is_different_tasks_mode() {
        assert!(slot.is_some(), "The slot parameter is required for this mode");
      } else {
        assert!(slot.is_none(), "The slot parameter is not used for this mode");
      }

      match bounty.multitasking.clone().unwrap() {
        Multitasking::ContestOrHackathon { .. } => {
          bounty_statuses = vec![BountyStatus::New, BountyStatus::ManyClaimed];
          claim_statuses = vec![
            ClaimStatus::New,
            ClaimStatus::Competes,
            ClaimStatus::Completed,
            ClaimStatus::NotHired,
          ];
          claim_message = "You already have a claim to participate in the competition";
        },

        Multitasking::OneForAll { .. } => {
          bounty_statuses = vec![
            BountyStatus::New,
            BountyStatus::ManyClaimed,
            BountyStatus::AwaitingClaims
          ];
          claim_statuses = vec![
            ClaimStatus::New,
            ClaimStatus::ReadyToStart,
            ClaimStatus::InProgress,
            ClaimStatus::Completed,
            ClaimStatus::Rejected,
            ClaimStatus::Disputed,
          ];
          claim_message = "You already have an active claim";
        },

        Multitasking::DifferentTasks { subtasks, .. } => {
          assert!(
            slot.unwrap() < subtasks.len(),
            "The slot parameter cannot exceed the number of bounty tasks"
          );

          bounty_statuses = vec![BountyStatus::New, BountyStatus::ManyClaimed];
          claim_statuses = vec![
            ClaimStatus::New,
            ClaimStatus::ReadyToStart,
            ClaimStatus::InProgress,
            ClaimStatus::Completed,
            ClaimStatus::Rejected,
            ClaimStatus::Disputed,
            ClaimStatus::CompletedWithDispute,
          ];
          claim_message = "You already have an active claim";
        },
        _ => unreachable!(),
      }
    }

    let (_, claim, claims) = self.internal_get_and_check_bounty_and_claim(
      id.clone(),
      receiver_id.clone(),
      None,
      bounty_statuses,
      claim_statuses,
      true,
      "Bounty status does not allow to submit a claim",
      claim_message
    );

    if bounty.multitasking.is_some() {
      assert!(
        bounty.multitasking.clone().unwrap().is_allowed_to_create_or_approve_claims(slot),
        "It is no longer possible to create new claims"
      );
      if bounty.allow_creating_many_claims {
        let claims_amount = claims.len();
        assert!(claims_amount < 255, "One account can create no more than 255 claims");
        claim_number = Some(claims_amount as u8);
      }
    }

    (bounty, claim, claim_number)
  }

  pub(crate) fn check_if_allowed_to_approve_claim_by_status(
    &self,
    id: BountyIndex,
    receiver_id: AccountId,
    claim_number: Option<u8>,
  ) -> (Bounty, ClaimIndex, BountyClaim) {
    let bounty = self.get_bounty(id.clone());
    let bounty_statuses: Vec<BountyStatus>;

    assert_eq!(
      claim_number.is_some(),
      bounty.allow_creating_many_claims,
      "Invalid claim_number value"
    );

    if bounty.multitasking.is_none() {
      bounty_statuses = vec![BountyStatus::New];
    } else {
      match bounty.multitasking.clone().unwrap() {
        Multitasking::ContestOrHackathon { .. } => {
          bounty_statuses = vec![BountyStatus::New, BountyStatus::ManyClaimed];
        },
        Multitasking::OneForAll { .. } => {
          bounty_statuses = vec![
            BountyStatus::New,
            BountyStatus::ManyClaimed,
            BountyStatus::AwaitingClaims
          ];
        },
        Multitasking::DifferentTasks { .. } => {
          bounty_statuses = vec![BountyStatus::New, BountyStatus::ManyClaimed];
        },
        _ => unreachable!(),
      }
    }

    let (_, claim, _) = self.internal_get_and_check_bounty_and_claim(
      id.clone(),
      receiver_id.clone(),
      claim_number,
      bounty_statuses,
      vec![ClaimStatus::New],
      false,
      "Bounty status does not allow to make a decision on a claim",
      "Claim status does not allow a decision to be made"
    );
    let (claim_id, bounty_claim) = claim.unwrap();

    if bounty.multitasking.is_some() {
      assert!(
        bounty.multitasking
          .clone()
          .unwrap()
          .is_allowed_to_create_or_approve_claims(bounty_claim.slot.clone()),
        "It is no longer possible to create new claims"
      );
    }

    (bounty, claim_id, bounty_claim)
  }

  pub(crate) fn check_if_claimant_in_kyc_whitelist(
    &self,
    id: BountyIndex,
    receiver_id: AccountId,
    claim_number: Option<u8>,
    place_of_check: PlaceOfCheckKYC,
    slot: Option<usize>,
  ) -> PromiseOrValue<()> {
    Promise::new(self.kyc_whitelist_contract.clone().unwrap())
      .function_call(
        "is_whitelisted".to_string(),
        json!({
          "account_id": receiver_id.clone(),
        })
          .to_string()
          .into_bytes(),
        NO_DEPOSIT,
        GAS_FOR_CHECK_IF_WHITELISTED,
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_CHECK_IF_WHITELISTED)
          .after_check_if_whitelisted(id, receiver_id, claim_number, place_of_check, slot)
      )
      .into()
  }

  pub(crate) fn internal_add_proposal_and_create_claim(
    &mut self,
    id: BountyIndex,
    receiver_id: AccountId,
    claim_number: Option<u8>,
    place_of_check: PlaceOfCheckKYC,
    slot: Option<usize>,
    bounty_and_claim: Option<(Bounty, Option<(ClaimIndex, BountyClaim)>)>,
  ) -> PromiseOrValue<()> {
    let ( deadline, description ) = match place_of_check {
      PlaceOfCheckKYC::CreatingClaim { deadline, description } => (deadline, description),
      _ => unreachable!(),
    };
    let bounty = self.get_bounty(id.clone());
    if bounty.is_validators_dao_used() &&
      self.is_approval_required(&bounty, &receiver_id)
    {
      self.internal_add_proposal_to_approve_claimant(
        id,
        receiver_id,
        claim_number,
        bounty,
        deadline,
        description,
        slot,
      )
    } else {
      self.internal_create_claim(
        id,
        receiver_id,
        claim_number,
        deadline,
        description,
        None,
        slot,
        bounty_and_claim
      );
      PromiseOrValue::Value(())
    }
  }

  pub(crate) fn internal_approval_and_save_claim(
    &mut self,
    id: BountyIndex,
    claimant: Option<(AccountId, Option<u8>)>,
    bounty_and_claim: Option<(Bounty, ClaimIndex, BountyClaim)>,
    is_kyc_delayed: Option<DefermentOfKYC>,
  ) -> PromiseOrValue<()> {
    let (
      mut bounty,
      claim_id,
      mut bounty_claim
    ) = if claimant.is_some() {
      let (receiver_id, claim_number) = claimant.unwrap();
      self.check_if_allowed_to_approve_claim_by_status(id, receiver_id.clone(), claim_number)
    } else {
      bounty_and_claim.expect("Incorrect approval parameters")
    };

    self.internal_claimant_approval(id, &mut bounty, &mut bounty_claim, is_kyc_delayed);
    self.claims.insert(&claim_id, &bounty_claim.into());
    PromiseOrValue::Value(())
  }

  pub(crate) fn internal_rejection_and_save_claim(
    &mut self,
    claimant: Option<(BountyIndex, AccountId, Option<u8>)>,
    bounty_and_claim: Option<(Bounty, ClaimIndex, BountyClaim)>,
  ) -> PromiseOrValue<()> {
    let (
      _,
      claim_id,
      mut bounty_claim
    ) = if claimant.is_some() {
      let (id, receiver_id, claim_number) = claimant.unwrap();
      self.check_if_allowed_to_approve_claim_by_status(id, receiver_id.clone(), claim_number)
    } else {
      bounty_and_claim.expect("Incorrect rejection parameters")
    };

    bounty_claim.status = ClaimStatus::NotHired;
    self.claims.insert(&claim_id, &bounty_claim.clone().into());
    self.internal_return_bonds(&bounty_claim.owner, bounty_claim.bond)
  }

  pub(crate) fn internal_add_proposal_and_update_claim(
    &mut self,
    id: BountyIndex,
    receiver_id: AccountId,
    claim_number: Option<u8>,
    place_of_check: PlaceOfCheckKYC,
  ) -> PromiseOrValue<()> {
    let bounty = self.get_bounty(id.clone());
    if bounty.is_validators_dao_used() && !bounty.is_different_tasks() {
      self.internal_add_proposal_to_finish_claim(
        id,
        receiver_id,
        claim_number,
        bounty,
        place_of_check
      )
    } else if bounty.is_validators_dao_used() && bounty.is_different_tasks() &&
      Self::internal_are_all_slots_complete(&bounty, Some((receiver_id.clone(), claim_number))) &&
      Self::internal_get_bounty_payout_proposal_id(&bounty).is_none()
    {
      self.internal_add_proposal_to_finish_several_claims(
        id,
        receiver_id,
        claim_number,
        bounty,
        place_of_check
      )
    } else {
      self.internal_claim_done(id, receiver_id, claim_number, None);
      PromiseOrValue::Value(())
    }
  }

  pub(crate) fn internal_claim_done(
    &mut self,
    id: BountyIndex,
    receiver_id: AccountId,
    claim_number: Option<u8>,
    proposal_id: Option<U64>
  ) {
    let (mut bounty, claim, _) = self.internal_get_and_check_bounty_and_claim(
      id.clone(),
      receiver_id.clone(),
      claim_number,
      vec![BountyStatus::Claimed, BountyStatus::ManyClaimed],
      vec![ClaimStatus::InProgress, ClaimStatus::Competes],
      false,
      "Bounty status does not allow to completion",
      "The claim status does not allow to complete the bounty"
    );
    let (claim_id, mut bounty_claim) = claim.unwrap();

    bounty_claim.status = ClaimStatus::Completed;
    if bounty.is_different_tasks() {
      self.internal_complete_slot(&mut bounty, bounty_claim.slot.clone().unwrap());
      if proposal_id.is_some() {
        self.internal_set_bounty_payout_proposal_id(&mut bounty, proposal_id);
      }
      self.internal_update_bounty(&id, bounty);
    } else {
      bounty_claim.bounty_payout_proposal_id = proposal_id;
    }
    self.claims.insert(&claim_id, &bounty_claim.into());
  }

  pub(crate) fn internal_bounty_completion(
    &mut self,
    id: BountyIndex,
    mut bounty: Bounty,
    claimant: Option<(AccountId, Option<u8>)>,
  ) {
    let action_kind;
    let bond: Option<U128>;
    let (receiver_id, claim_number) = if claimant.is_some() {
      let claimant = claimant.clone().unwrap();
      (Some(claimant.0), claimant.1)
    } else {
      (None, None)
    };

    if bounty.is_different_tasks() {
      assert!(claimant.is_none(), "The claimant must not be established");
      assert!(
        matches!(bounty.status, BountyStatus::ManyClaimed),
        "Bounty status does not allow to payout"
      );
      assert!(
        Self::internal_are_all_slots_complete(&bounty, None),
        "Not all tasks have already been completed"
      );

      action_kind = ReputationActionKind::SuccessfulBounty;
      bond = None;

    } else {
      assert!(claimant.is_some(), "The claimant must be established");
      let (_, claim, _) = self.internal_get_and_check_bounty_and_claim(
        id.clone(),
        receiver_id.clone().unwrap(),
        claim_number,
        vec![BountyStatus::Claimed, BountyStatus::ManyClaimed],
        vec![ClaimStatus::Completed, ClaimStatus::Disputed],
        false,
        "Bounty status does not allow to payout",
        "The claim status does not allow to payout"
      );
      let (claim_id, mut bounty_claim) = claim.unwrap();

      bond = bounty_claim.bond;
      let with_dispute = bounty_claim.status == ClaimStatus::Disputed;
      bounty_claim.status = ClaimStatus::Approved;
      self.claims.insert(&claim_id, &bounty_claim.into());

      action_kind = if bounty.is_one_bounty_for_many_claimants() {
        ReputationActionKind::SuccessfulClaim { with_dispute }
      } else {
        ReputationActionKind::SuccessfulBountyAndClaim { with_dispute }
      };
    }

    let amounts = Self::internal_get_bounty_amount_for_payment(&bounty);
    self.internal_total_fees_unlocking_funds(&bounty, Some(amounts.1), Some(amounts.2));

    if bounty.multitasking.is_none() {
      bounty.status = BountyStatus::Completed;
    } else {
      match bounty.multitasking.clone().unwrap() {
        Multitasking::ContestOrHackathon { .. } => {
          self.internal_participants_decrement(&mut bounty);
          self.internal_finish_competition(
            &mut bounty,
            Some((receiver_id.clone().unwrap(), claim_number))
          );
          bounty.status = BountyStatus::Completed;
        },
        Multitasking::OneForAll { number_of_slots, .. } => {
          self.internal_paid_slots_increment(&mut bounty);
          let (
            occupied_slots,
            paid_slots
          ) = bounty.multitasking.clone().unwrap().get_one_for_all_env();
          if occupied_slots == 0 {
            if paid_slots == number_of_slots {
              bounty.status = BountyStatus::Completed;
            } else {
              bounty.status = BountyStatus::AwaitingClaims;
            }
          }
        },
        Multitasking::DifferentTasks { .. } => {
          bounty.status = BountyStatus::Completed;
        },
        _ => unreachable!(),
      }
    }

    self.internal_update_bounty(&id, bounty.clone());
    self.internal_update_statistic(
      receiver_id.clone(),
      Some(bounty.owner.clone()),
      action_kind,
    );
    if receiver_id.is_some() {
      self.internal_return_bonds(&receiver_id.unwrap(), bond);
    }
  }

  pub(crate) fn internal_get_and_check_bounty_and_claim(
    &self,
    id: BountyIndex,
    receiver_id: AccountId,
    claim_number: Option<u8>,
    bounty_statuses: Vec<BountyStatus>,
    claim_statuses: Vec<ClaimStatus>,
    no_claim_found: bool,
    bounty_message: &str,
    claim_message: &str,
  ) -> (Bounty, Option<(ClaimIndex, BountyClaim)>, Vec<ClaimIndex>) {
    let bounty = self.get_bounty(id.clone());
    if !bounty_statuses.contains(&bounty.status) {
      env::panic_str(bounty_message);
    }

    let claims = self.internal_get_claims_by_account_id_an_bounty_id(&id, &receiver_id, true);

    if !bounty.allow_creating_many_claims || !no_claim_found {
      let claim = self.internal_find_claim(&claims, id, receiver_id, claim_number);
      assert!(no_claim_found || claim.is_some(), "No bounty claim found");

      let claim_found = claim.is_some() &&
        claim_statuses
          .into_iter()
          .find(|s| claim.clone().unwrap().1.status == s.clone())
          .is_some();
      if no_claim_found == claim_found {
        env::panic_str(claim_message);
      }

      (bounty, claim, claims)

    } else {
      (bounty, None, claims)
    }
  }

  pub(crate) fn check_bounty(&self, bounty: &Bounty) {
    bounty.assert_valid();
    self.assert_bounty_category_is_correct(bounty.metadata.category.clone());
    if bounty.metadata.tags.is_some() {
      self.assert_bounty_tags_are_correct(bounty.metadata.tags.clone().unwrap());
    }
    if bounty.postpaid.is_some() {
      match bounty.postpaid.clone().unwrap() {
        Postpaid::PaymentOutsideContract { currency, .. } =>
          self.assert_bounty_currency_is_correct(currency),
        _ => unreachable!()
      }
    }
    match bounty.kyc_config {
      KycConfig::KycRequired { .. } => {
        assert!(
          self.kyc_whitelist_contract.is_some(),
          "KYC whitelist contract is not set"
        );
      },
      _ => {}
    }
    if self.config.clone().to_config().max_due_date.is_some() &&
      bounty.deadline.get_deadline_type() == 1
    {
      assert!(
        bounty.deadline.get_deadline_value().0 <=
          env::block_timestamp() + self.config.clone().to_config().max_due_date.unwrap().0,
        "Incorrect due date"
      );
    }
  }

  pub(crate) fn internal_finalize_active_claim(
    &mut self,
    id: BountyIndex,
    mut bounty: Bounty,
    active_claim: Option<(ClaimIndex, BountyClaim)>,
  ) -> Option<PromiseOrValue<()>> {
    let different_task = bounty.is_different_tasks();

    if active_claim.is_none() {
      if bounty.status == BountyStatus::ManyClaimed &&
        different_task &&
        Self::internal_are_all_slots_complete(&bounty, None) &&
        Self::internal_get_bounty_payout_proposal_id(&bounty).is_some() &&
        bounty.is_validators_dao_used()
      {
        let proposal_id = Self::internal_get_bounty_payout_proposal_id(&bounty).unwrap();

        Some(self.internal_check_bounty_payout_proposal(
          id,
          bounty,
          None,
          proposal_id
        ))
      } else {
        None
      }

    } else {
      let (claim_id, mut bounty_claim) = active_claim.unwrap();

      if (bounty_claim.status == ClaimStatus::InProgress ||
        bounty_claim.status == ClaimStatus::Competes) &&
        (bounty.status == BountyStatus::Claimed ||
          bounty.status == BountyStatus::ManyClaimed ||
          bounty.status == BountyStatus::Completed && bounty.is_contest_or_hackathon()) &&
        bounty_claim.is_claim_expired(&bounty)
      {
        Some(self.internal_set_claim_expiry_status(
          id,
          &bounty_claim.owner.clone(),
          &mut bounty,
          claim_id,
          &mut bounty_claim
        ))
      }

      else if
        (bounty_claim.status == ClaimStatus::Completed && !different_task ||
          different_task &&
          Self::internal_are_all_slots_complete(&bounty, None) &&
          Self::internal_get_bounty_payout_proposal_id(&bounty).is_some()) &&
        (bounty.status == BountyStatus::Claimed ||
          bounty.status == BountyStatus::ManyClaimed) &&
        bounty.is_validators_dao_used()
      {
        let proposal_id = if different_task {
          Self::internal_get_bounty_payout_proposal_id(&bounty).unwrap()
        } else {
          bounty_claim.bounty_payout_proposal_id.unwrap()
        };

        Some(self.internal_check_bounty_payout_proposal(
          id,
          bounty,
          if different_task { None } else { Some((bounty_claim.owner, bounty_claim.claim_number)) },
          proposal_id
        ))
      }

      else if bounty_claim.status == ClaimStatus::Rejected &&
        (bounty.status == BountyStatus::Claimed ||
          bounty.status == BountyStatus::ManyClaimed) &&
        self.is_deadline_for_opening_dispute_expired(&bounty_claim)
      {
        Some(self.internal_reset_bounty_to_initial_state(
          id,
          &bounty_claim.owner.clone(),
          &mut bounty,
          claim_id,
          &mut bounty_claim,
          None,
          true
        ))
      }

      else if bounty_claim.status == ClaimStatus::Disputed &&
        (bounty.status == BountyStatus::Claimed ||
          bounty.status == BountyStatus::ManyClaimed)
      {
        Some(
          self.internal_get_dispute(
            id,
            bounty_claim.owner,
            bounty_claim.claim_number,
            bounty_claim.dispute_id.unwrap()
          )
        )
      }

      else {
        None
      }
    }
  }

  pub(crate) fn internal_finalize_some_claim(
    &mut self,
    id: BountyIndex,
    receiver_id: AccountId,
    claim_number: Option<u8>,
    bounty: &mut Bounty,
  ) -> PromiseOrValue<()> {
    let (claim_id, mut bounty_claim) = self.internal_get_claim(
      id,
      receiver_id.clone(),
      claim_number
    );

    if bounty.multitasking.is_some() {
      let result = self.internal_finalize_active_claim(
        id,
        bounty.clone(),
        Some((claim_id, bounty_claim.clone())),
      );
      if result.is_some() {
        return result.unwrap();
      }
    }

    if bounty_claim.status == ClaimStatus::New &&
      bounty.status == BountyStatus::New &&
      bounty.is_validators_dao_used()
    {
      self.internal_check_approve_claimant_proposal(
        id,
        &bounty,
        receiver_id,
        claim_number,
        bounty_claim.approve_claimant_proposal_id.unwrap(),
      )
    }

    else if bounty_claim.status == ClaimStatus::New &&
      (bounty.status == BountyStatus::Completed ||
        bounty.status == BountyStatus::Canceled ||
        bounty.status == BountyStatus::PartiallyCompleted)
    {
      self.internal_claim_closure(
        &receiver_id,
        bounty.owner.clone(),
        bounty.status.clone(),
        claim_id,
        &mut bounty_claim,
        Some(ClaimStatus::Canceled),
        true
      )
    }

    else if (bounty.status == BountyStatus::Completed ||
      bounty.status == BountyStatus::Canceled) &&
      (bounty_claim.status == ClaimStatus::Competes ||
        bounty_claim.status == ClaimStatus::Completed) &&
      bounty.is_contest_or_hackathon() ||
      bounty.status == BountyStatus::Canceled &&
        bounty_claim.status == ClaimStatus::ReadyToStart &&
        (bounty.is_one_bounty_for_many_claimants() || bounty.is_different_tasks())
    {
      match bounty.multitasking.clone().unwrap() {
        Multitasking::ContestOrHackathon { .. } => {
          self.internal_participants_decrement(bounty);
        },
        Multitasking::OneForAll { .. } => {
          self.internal_occupied_slots_decrement(bounty);
        },
        Multitasking::DifferentTasks { .. } => {
          let slot = bounty_claim.slot.clone().unwrap();
          self.internal_reset_slot(bounty, slot);
        },
        _ => unreachable!(),
      }
      self.internal_update_bounty(&id, bounty.clone());
      let new_status = if bounty.status == BountyStatus::Canceled {
        Some(ClaimStatus::Canceled)
      } else {
        None
      };
      self.internal_claim_closure(
        &receiver_id,
        bounty.owner.clone(),
        bounty.status.clone(),
        claim_id,
        &mut bounty_claim,
        new_status,
        true
      )
    }

    else {
      env::panic_str("This bounty claim is not subject to finalization");
    }
  }

  pub(crate) fn assert_live(&self) {
    assert!(
      matches!(self.status, ContractStatus::Live),
      "The contract status is not Live"
    );
  }
}
