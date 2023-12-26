use std::string::ToString;
use near_sdk::json_types::{U128, U64};
use serde_json::json;
use workspaces::Account;
use bounties::{Bounty, BountyAction, BountyMetadata, BountyStatus, BountyUpdate, ClaimerApproval,
               ClaimStatus, ContactDetails, ContactType, Deadline, DefermentOfKYC, Experience,
               KycConfig, KycVerificationMethod, Multitasking, Postpaid, Reviewers, ReviewersParams,
               StartConditions, Subtask, TokenDetails, WhitelistType};
use disputes::DisputeStatus;

mod utils;
use crate::utils::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let e = Env::init().await?;
  // begin tests
  test_create_bounty(&e).await?;
  test_bounty_claim(&e).await?;
  test_claim_done(&e).await?;
  test_claim_result_approve_by_validators_dao(&e).await?;
  test_claimer_give_up(&e).await?;
  test_claim_result_reject_by_project_owner(&e).await?;
  test_claim_result_approve_by_project_owner(&e).await?;
  test_claim_result_reject_by_validators_dao(&e).await?;
  test_bounty_claim_deadline_has_expired(&e).await?;
  test_open_dispute_and_reject_by_dispute_dao(&e).await?;
  test_cancel_dispute_by_claimer(&e).await?;
  test_cancel_dispute_by_project_owner(&e).await?;
  test_open_dispute_and_approve_by_dispute_dao(&e).await?;
  test_statistics_for_bounty_claim_approval(&e).await?;
  test_rejection_and_approval_of_claimers_by_project_owner(&e).await?;
  test_rejection_and_approval_of_claimers_by_validators_dao(&e).await?;
  test_using_more_reviewers(&e).await?;
  test_bounty_cancel(&e).await?;
  test_bounty_update(&e).await?;
  test_kyc_whitelist_flow(&e).await?;
  test_use_commissions(&e).await?;
  test_creating_bounty_for_disabled_token(&e).await?;
  test_approval_by_whitelist_flow(&e).await?;
  test_postpaid_flow(&e).await?;
  test_competition_flow(&e).await?;
  test_one_bounty_for_many(&e).await?;
  test_different_tasks_flow(&e).await?;
  test_withdraw_non_refunded_bonds(&e).await?;
  test_flow_with_stretch_claim_deadline(&e).await?;
  test_owners_whitelist_flow(&e).await?;
  Ok(())
}

async fn test_create_bounty(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.bounties).await?;
  assert_eq!(last_bounty_id, 0);
  let owner_balance = e.get_token_balance(e.project_owner.id()).await?;
  assert_eq!(e.get_token_balance(e.bounties.id()).await?, 0);

  let _ = e.add_bounty(
    &e.bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    Some(e.reviewer_is_dao().await?),
    None, None, None, None, None, None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.bounties).await?;
  assert_eq!(last_bounty_id, 1);
  assert_eq!(
    e.get_token_balance(e.project_owner.id()).await?,
    owner_balance - BOUNTY_AMOUNT.0 - PLATFORM_FEE.0 - DAO_FEE.0
  );
  assert_eq!(
    e.get_token_balance(e.bounties.id()).await?,
    BOUNTY_AMOUNT.0 + PLATFORM_FEE.0 + DAO_FEE.0
  );

  let bounty_id = 0;
  let bounties = e.get_account_bounties(&e.bounties).await?;
  assert_eq!(bounties.len(), 1);
  assert_eq!(bounties[0].0, bounty_id);

  let bounty = get_bounty(&e.bounties, bounty_id).await?;
  assert_eq!(bounties[0].1, bounty);
  let bounty_set = get_bounties_by_ids(&e.bounties, vec![bounty_id]).await?;
  assert_eq!(bounty_set.len(), 1);
  assert_eq!(bounty_set[0].0, bounty_id);
  assert_eq!(bounty_set[0].1, bounty);

  assert_eq!(
    bounty,
    Bounty {
      token: Some(e.test_token.id().to_string().parse().unwrap()),
      amount: BOUNTY_AMOUNT,
      platform_fee: PLATFORM_FEE,
      dao_fee: DAO_FEE,
      metadata: BountyMetadata {
        title: "Test bounty title".to_string(),
        description: "Test bounty description".to_string(),
        category: "Marketing".to_string(),
        attachments: Some(vec!["http://ipfs-url/1".to_string(), "http://ipfs-url/2".to_string()]),
        experience: Some(Experience::Intermediate),
        tags: Some(vec!["Community".to_string(), "NFT".to_string()]),
        acceptance_criteria: Some("test acceptance criteria".to_string()),
        contact_details: Some(
          ContactDetails {
            contact: "example@domain.net".to_string(),
            contact_type: ContactType::Email
          }
        ),
      },
      deadline: Deadline::MaxDeadline {
        max_deadline: MAX_DEADLINE,
      },
      claimer_approval: ClaimerApproval::WithoutApproval,
      reviewers: Some(e.reviewer_is_dao().await?.to_reviewers()),
      owner: e.project_owner.id().to_string().parse().unwrap(),
      status: BountyStatus::New,
      created_at: bounty.created_at,
      kyc_config: KycConfig::KycNotRequired,
      postpaid: None,
      multitasking: None,
      allow_deadline_stretch: false,
    }
  );

  println!("      Passed ✅ Test create a bounty");
  Ok(())
}

async fn test_bounty_claim(e: &Env) -> anyhow::Result<()> {
  let bounty_id = 0;
  let deadline = Some(U64(1_000_000_000 * 60 * 60 * 24 * 2));
  e.bounty_claim(
    &e.bounties,
    bounty_id,
    deadline,
    "Test claim".to_string(),
    None,
    None, // by default freelancer
    None
  ).await?;

  let (bounty_claim, _) = e.assert_statuses(
    &e.bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::InProgress,
    BountyStatus::Claimed
  ).await?;

  let freelancer_claims = e.get_bounty_claims(&e.bounties).await?;
  assert_eq!(freelancer_claims.len(), 1);
  let freelancer_claim = freelancer_claims[0].clone();
  assert_eq!(freelancer_claim, bounty_claim);
  assert_eq!(freelancer_claim.bounty_id, bounty_id);
  assert_eq!(freelancer_claim.deadline, deadline);
  assert!(freelancer_claim.bounty_payout_proposal_id.is_none());

  println!("      Passed ✅ Test create a bounty claim");
  Ok(())
}

async fn test_claim_done(e: &Env) -> anyhow::Result<()> {
  let bounty_id = 0;
  e.bounty_done(&e.bounties, bounty_id, "test description".to_string(), None, None).await?;

  e.assert_statuses(
    &e.bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::Completed,
    BountyStatus::Claimed
  ).await?;

  println!("      Passed ✅ Test of claim completion");
  Ok(())
}

async fn test_claim_result_approve_by_validators_dao(e: &Env) -> anyhow::Result<()> {
  let token_balance = e.get_token_balance(e.bounties.id()).await?;
  assert_eq!(e.get_token_balance(e.freelancer.id()).await?, 0);

  let bounty_id = 0;
  e.bounty_action_by_validators_dao(
    &e.bounties,
    bounty_id,
    "VoteApprove".to_string(),
    0,
    true,
  ).await?;

  e.assert_statuses(
    &e.bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::Approved,
    BountyStatus::Completed,
  ).await?;

  assert_eq!(
    e.get_token_balance(e.bounties.id()).await?,
    token_balance - BOUNTY_AMOUNT.0
  );
  assert_eq!(e.get_token_balance(e.freelancer.id()).await?, BOUNTY_AMOUNT.0);

  println!("      Passed ✅ Test - claim result approve by DAO");
  Ok(())
}

async fn test_claimer_give_up(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.bounties).await?;
  assert_eq!(last_bounty_id, 1);

  let _ = e.add_bounty(
    &e.bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None, None, None, None, None, None, None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.bounties).await?;
  assert_eq!(last_bounty_id, 2);

  let bounty_id = 1;
  e.bounty_claim(
    &e.bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    None, // by default freelancer
    None
  ).await?;
  e.bounty_give_up(&e.bounties, bounty_id, None).await?;

  e.assert_statuses(
    &e.bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::Canceled,
    BountyStatus::New,
  ).await?;

  println!("      Passed ✅ Test - claimer give up");
  Ok(())
}

async fn test_claim_result_reject_by_project_owner(e: &Env) -> anyhow::Result<()> {
  let bounty_id = 1;
  e.bounty_claim(
    &e.bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    None, // by default freelancer
    None
  ).await?;
  e.bounty_done(&e.bounties, bounty_id, "test description".to_string(), None, None).await?;

  Env::bounty_action_by_user(
    &e.bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimRejected {
      receiver_id: e.freelancer.id().as_str().parse().unwrap()
    },
    None,
  ).await?;

  e.assert_statuses(
    &e.bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::NotCompleted,
    BountyStatus::New,
  ).await?;

  println!("      Passed ✅ Test - claim result reject by project owner");
  Ok(())
}

async fn test_claim_result_approve_by_project_owner(e: &Env) -> anyhow::Result<()> {
  let token_balance = e.get_token_balance(e.bounties.id()).await?;
  let freelancer_balance = e.get_token_balance(e.freelancer.id()).await?;

  let bounty_id = 1;
  e.bounty_claim(
    &e.bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    None, // by default freelancer
    None
  ).await?;
  e.bounty_done(&e.bounties, bounty_id, "test description".to_string(), None, None).await?;

  Env::bounty_action_by_user(
    &e.bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimApproved {
      receiver_id: e.freelancer.id().as_str().parse().unwrap()
    },
    None,
  ).await?;

  e.assert_statuses(
    &e.bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::Approved,
    BountyStatus::Completed,
  ).await?;

  assert_eq!(
    e.get_token_balance(e.bounties.id()).await?,
    token_balance - BOUNTY_AMOUNT.0
  );
  assert_eq!(
    e.get_token_balance(e.freelancer.id()).await?,
    freelancer_balance + BOUNTY_AMOUNT.0
  );

  println!("      Passed ✅ Test - claim result approve by project owner");
  Ok(())
}

async fn test_claim_result_reject_by_validators_dao(e: &Env) -> anyhow::Result<()> {
  e.assert_reputation_stat_values_eq(None, None, None).await?;
  // New bounty contract, numbering reset
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 0);

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    Some(e.reviewer_is_dao().await?),
    None, None, None, None, None, None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 1);
  e.assert_reputation_stat_values_eq(None, Some([1, 0, 0, 0, 0, 0, 0, 0, 0]), None).await?;

  let bounty_id = 0;
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    None, // by default freelancer
    None
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 1, 0, 0, 0, 0, 0, 0]),
    Some([1, 0, 0, 1, 1, 0, 0, 0, 0]),
    None,
  ).await?;

  e.bounty_done(
    &e.disputed_bounties,
    bounty_id,
    "test description".to_string(),
    None,
    None,
  ).await?;
  e.bounty_action_by_validators_dao(
    &e.disputed_bounties,
    bounty_id,
    "VoteReject".to_string(),
    0,
    true,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::Completed,
    BountyStatus::Claimed,
  ).await?;

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.freelancer,
    &BountyAction::Finalize { receiver_id: None },
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::Rejected,
    BountyStatus::Claimed,
  ).await?;

  // Period for opening a dispute: 10 min, wait for 500 blocks
  e.worker.fast_forward(500).await?;
  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::Finalize { receiver_id: None },
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::NotCompleted,
    BountyStatus::New,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 1, 0, 1, 0, 0, 0, 0]),
    Some([1, 0, 0, 1, 1, 0, 1, 0, 0]),
    None,
  ).await?;

  println!("      Passed ✅ Test - claim result reject by DAO");
  Ok(())
}

async fn test_bounty_claim_deadline_has_expired(e: &Env) -> anyhow::Result<()> {
  let bounty_id = 0;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    None, // by default freelancer
    None
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([2, 2, 0, 1, 0, 0, 0, 0]),
    Some([1, 0, 0, 2, 2, 0, 1, 0, 0]),
    None,
  ).await?;

  e.bounty_give_up(&e.disputed_bounties, bounty_id, None).await?;
  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::Canceled,
    BountyStatus::New,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([2, 2, 0, 1, 0, 1, 0, 0]),
    Some([1, 0, 0, 2, 2, 0, 1, 0, 0]),
    None,
  ).await?;

  // Deadline 2 min
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 2)),
    "Test claim".to_string(),
    None,
    None, // by default freelancer
    None
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([3, 3, 0, 1, 0, 1, 0, 0]),
    Some([1, 0, 0, 3, 3, 0, 1, 0, 0]),
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::InProgress,
    BountyStatus::Claimed,
  ).await?;

  // Wait for 200 blocks
  e.worker.fast_forward(200).await?;
  Env::bounty_action_by_user(
    &e.disputed_bounties, bounty_id,
    &e.project_owner,
    &BountyAction::Finalize { receiver_id: None },
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::Expired,
    BountyStatus::New,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([3, 3, 0, 1, 1, 1, 0, 0]),
    Some([1, 0, 0, 3, 3, 0, 1, 0, 0]),
    None,
  ).await?;

  println!("      Passed ✅ Test - bounty claim deadline has expired");
  Ok(())
}

async fn test_open_dispute_and_reject_by_dispute_dao(e: &Env) -> anyhow::Result<()> {
  let bounty_id = 0;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 2)),
    "Test claim".to_string(),
    None,
    None, // by default freelancer
    None
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([4, 4, 0, 1, 1, 1, 0, 0]),
    Some([1, 0, 0, 4, 4, 0, 1, 0, 0]),
    None,
  ).await?;
  e.bounty_done(
    &e.disputed_bounties,
    bounty_id,
    "test description".to_string(),
    None,
    None,
  ).await?;
  e.bounty_action_by_validators_dao(
    &e.disputed_bounties,
    bounty_id,
    "VoteReject".to_string(),
    0,
    true,
  ).await?;
  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.freelancer,
    &BountyAction::Finalize { receiver_id: None },
    None,
  ).await?;

  e.open_dispute(&e.disputed_bounties, bounty_id, None).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::Disputed,
    BountyStatus::Claimed,
  ).await?;

  let dispute_id: u64 = 0;
  let dispute = e.get_dispute(dispute_id).await?;
  assert_eq!(dispute.bounty_id.0, bounty_id);
  assert_eq!(dispute.status, DisputeStatus::New);
  assert_eq!(dispute.claimer.to_string(), e.freelancer.id().to_string());
  assert_eq!(dispute.project_owner_delegate.to_string(), e.validators_dao.id().to_string());
  assert_eq!(dispute.description, "Dispute description".to_string());
  assert_eq!(dispute.proposal_id, None);
  assert_eq!(dispute.proposal_timestamp, None);

  e.provide_arguments(
    dispute_id,
    e.validators_dao.as_account(),
    "Arguments of the project owner".to_string()
  ).await?;

  e.provide_arguments(
    dispute_id,
    &e.freelancer,
    "Claimer's arguments".to_string()
  ).await?;

  // Argument period: 10 min, wait for 500 blocks
  e.worker.fast_forward(500).await?;

  e.escalation(
    dispute_id,
    &e.freelancer,
  ).await?;

  let dispute = e.get_dispute(dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::DecisionPending);
  let proposal_id = dispute.proposal_id.unwrap().0;
  assert_eq!(proposal_id, 0);

  let proposal = Env::get_proposal(&e.dispute_dao, proposal_id).await?;
  println!("Proposal: {}", json!(proposal).to_string());

  e.dispute_dao_action(
    proposal_id,
    "VoteReject".to_string(),
  ).await?;

  e.finalize_dispute(
    dispute_id,
    &e.project_owner,
  ).await?;

  let dispute = e.get_dispute(dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::InFavorOfProjectOwner);

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::NotCompleted,
    BountyStatus::New,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([4, 4, 0, 2, 1, 1, 1, 0]),
    Some([1, 0, 0, 4, 4, 0, 2, 1, 1]),
    None,
  ).await?;

  println!("      Passed ✅ Test - open the dispute and reject by dispute DAO");
  Ok(())
}

async fn test_cancel_dispute_by_claimer(e: &Env) -> anyhow::Result<()> {
  let bounty_id = 0;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 2)),
    "Test claim".to_string(),
    None,
    None, // by default freelancer
    None
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([5, 5, 0, 2, 1, 1, 1, 0]),
    Some([1, 0, 0, 5, 5, 0, 2, 1, 1]),
    None,
  ).await?;
  e.bounty_done(
    &e.disputed_bounties,
    bounty_id,
    "test description".to_string(),
    None,
    None,
  ).await?;
  e.bounty_action_by_validators_dao(
    &e.disputed_bounties,
    bounty_id,
    "VoteReject".to_string(),
    0,
    true,
  ).await?;
  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.freelancer,
    &BountyAction::Finalize { receiver_id: None },
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::Rejected,
    BountyStatus::Claimed,
  ).await?;

  e.open_dispute(&e.disputed_bounties, bounty_id, None).await?;
  let dispute_id: u64 = 1;
  let dispute = e.get_dispute(dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::New);

  e.cancel_dispute(dispute_id, &e.freelancer).await?;

  let dispute = e.get_dispute(dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::CanceledByClaimer);

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::NotCompleted,
    BountyStatus::New,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([5, 5, 0, 3, 1, 1, 2, 0]),
    Some([1, 0, 0, 5, 5, 0, 3, 2, 2]),
    None,
  ).await?;

  println!("      Passed ✅ Test - cancel the dispute by claimer");
  Ok(())
}

async fn test_cancel_dispute_by_project_owner(e: &Env) -> anyhow::Result<()> {
  let token_balance = e.get_token_balance(e.disputed_bounties.id()).await?;
  let freelancer_balance = e.get_token_balance(e.freelancer.id()).await?;

  let bounty_id = 0;
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 2)),
    "Test claim".to_string(),
    None,
    None, // by default freelancer
    None
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([6, 6, 0, 3, 1, 1, 2, 0]),
    Some([1, 0, 0, 6, 6, 0, 3, 2, 2]),
    None,
  ).await?;
  e.bounty_done(
    &e.disputed_bounties,
    bounty_id,
    "test description".to_string(),
    None,
    None,
  ).await?;
  e.bounty_action_by_validators_dao(
    &e.disputed_bounties,
    bounty_id,
    "VoteReject".to_string(),
    0,
    true,
  ).await?;
  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.freelancer,
    &BountyAction::Finalize { receiver_id: None },
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::Rejected,
    BountyStatus::Claimed,
  ).await?;

  e.open_dispute(&e.disputed_bounties, bounty_id, None).await?;
  let dispute_id: u64 = 2;
  let dispute = e.get_dispute(dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::New);

  e.cancel_dispute(dispute_id, e.validators_dao.as_account()).await?;

  let dispute = e.get_dispute(dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::CanceledByProjectOwner);

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::Approved,
    BountyStatus::Completed,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([6, 6, 1, 3, 1, 1, 3, 1]),
    Some([1, 1, 0, 6, 6, 0, 4, 3, 2]),
    None,
  ).await?;

  assert_eq!(
    e.get_token_balance(e.disputed_bounties.id()).await?,
    token_balance - BOUNTY_AMOUNT.0
  );
  assert_eq!(
    e.get_token_balance(e.freelancer.id()).await?,
    freelancer_balance + BOUNTY_AMOUNT.0
  );

  println!("      Passed ✅ Test - dispute was canceled by the project owner");
  Ok(())
}

async fn test_open_dispute_and_approve_by_dispute_dao(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 1);

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    Some(e.reviewer_is_dao().await?),
    None, None, None, None, None, None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 2);
  e.assert_reputation_stat_values_eq(
    Some([6, 6, 1, 3, 1, 1, 3, 1]),
    Some([2, 1, 0, 6, 6, 0, 4, 3, 2]),
    None,
  ).await?;

  let token_balance = e.get_token_balance(e.disputed_bounties.id()).await?;
  let freelancer_balance = e.get_token_balance(e.freelancer.id()).await?;

  let bounty_id = 1;
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    None, // by default freelancer
    None
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([7, 7, 1, 3, 1, 1, 3, 1]),
    Some([2, 1, 0, 7, 7, 0, 4, 3, 2]),
    None,
  ).await?;
  e.bounty_done(
    &e.disputed_bounties,
    bounty_id,
    "test description".to_string(),
    None,
    None,
  ).await?;
  e.bounty_action_by_validators_dao(
    &e.disputed_bounties,
    bounty_id,
    "VoteReject".to_string(),
    0,
    true,
  ).await?;
  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.freelancer,
    &BountyAction::Finalize { receiver_id: None },
    None,
  ).await?;

  e.open_dispute(&e.disputed_bounties, bounty_id, None).await?;
  let dispute_id: u64 = 3;
  let dispute = e.get_dispute(dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::New);

  // Argument period: 10 min, wait for 500 blocks
  e.worker.fast_forward(500).await?;

  e.escalation(
    dispute_id,
    &e.freelancer,
  ).await?;

  let dispute = e.get_dispute(dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::DecisionPending);
  let proposal_id = dispute.proposal_id.unwrap().0;
  assert_eq!(proposal_id, 1);

  e.dispute_dao_action(proposal_id, "VoteApprove".to_string()).await?;

  let dispute = e.get_dispute(dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::InFavorOfClaimer);

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::Approved,
    BountyStatus::Completed,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([7, 7, 2, 3, 1, 1, 4, 2]),
    Some([2, 2, 0, 7, 7, 0, 5, 4, 2]),
    None,
  ).await?;

  assert_eq!(
    e.get_token_balance(e.disputed_bounties.id()).await?,
    token_balance - BOUNTY_AMOUNT.0
  );
  assert_eq!(
    e.get_token_balance(e.freelancer.id()).await?,
    freelancer_balance + BOUNTY_AMOUNT.0
  );

  println!("      Passed ✅ Test - open and approve the dispute by dispute DAO");
  Ok(())
}

async fn test_statistics_for_bounty_claim_approval(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 2);

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    Some(e.reviewer_is_dao().await?),
    None, None, None, None, None, None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 3);
  e.assert_reputation_stat_values_eq(
    Some([7, 7, 2, 3, 1, 1, 4, 2]),
    Some([3, 2, 0, 7, 7, 0, 5, 4, 2]),
    None,
  ).await?;

  let token_balance = e.get_token_balance(e.disputed_bounties.id()).await?;
  let freelancer_balance = e.get_token_balance(e.freelancer.id()).await?;

  let bounty_id = 2;
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    None, // by default freelancer
    None
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([8, 8, 2, 3, 1, 1, 4, 2]),
    Some([3, 2, 0, 8, 8, 0, 5, 4, 2]),
    None,
  ).await?;
  e.bounty_done(
    &e.disputed_bounties,
    bounty_id,
    "test description".to_string(),
    None,
    None,
  ).await?;
  e.bounty_action_by_validators_dao(
    &e.disputed_bounties,
    bounty_id,
    "VoteApprove".to_string(),
    0,
    true,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::Approved,
    BountyStatus::Completed,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([8, 8, 3, 3, 1, 1, 4, 2]),
    Some([3, 3, 0, 8, 8, 1, 5, 4, 2]),
    None,
  ).await?;

  assert_eq!(
    e.get_token_balance(e.disputed_bounties.id()).await?,
    token_balance - BOUNTY_AMOUNT.0
  );
  assert_eq!(
    e.get_token_balance(e.freelancer.id()).await?,
    freelancer_balance + BOUNTY_AMOUNT.0
  );

  println!("      Passed ✅ Test statistics for bounty claim approval");
  Ok(())
}

async fn test_rejection_and_approval_of_claimers_by_project_owner(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 3);

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("MultipleClaims"),
    None, None, None, None, None, None, None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([8, 8, 3, 3, 1, 1, 4, 2]),
    Some([4, 3, 0, 8, 8, 1, 5, 4, 2]),
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 4);

  let bounty_id = 3;
  let deadline = Some(U64(1_000_000_000 * 60 * 60 * 24 * 2));
  // Four freelancers expressed their desire to do the task and receive the bounty
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    deadline,
    "Test claim 1".to_string(),
    None,
    None, // by default freelancer
    None
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([9, 8, 3, 3, 1, 1, 4, 2]),
    Some([4, 3, 0, 9, 8, 1, 5, 4, 2]),
    None,
  ).await?;

  let freelancer2 = e.add_account("freelancer2").await?;
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    deadline,
    "Test claim 2".to_string(),
    None,
    Some(&freelancer2),
    None
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 0, 0, 0, 0, 0, 0, 0]),
    Some([4, 3, 0, 10, 8, 1, 5, 4, 2]),
    Some(freelancer2.id()),
  ).await?;

  let freelancer3 = e.add_account("freelancer3").await?;
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    deadline,
    "Test claim 3".to_string(),
    None,
    Some(&freelancer3),
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 0, 0, 0, 0, 0, 0, 0]),
    Some([4, 3, 0, 11, 8, 1, 5, 4, 2]),
    Some(freelancer3.id()),
  ).await?;

  let freelancer4 = e.add_account("freelancer4").await?;
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    deadline,
    "Test claim 4".to_string(),
    None,
    Some(&freelancer4),
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 0, 0, 0, 0, 0, 0, 0]),
    Some([4, 3, 0, 12, 8, 1, 5, 4, 2]),
    Some(freelancer4.id()),
  ).await?;

  e.decision_on_claim(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    false,
    None,
    None,
    None
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([9, 8, 3, 3, 1, 1, 4, 2]),
    Some([4, 3, 0, 12, 8, 1, 5, 4, 2]),
    None,
  ).await?;

  e.decision_on_claim(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    true,
    Some(&freelancer2),
    None,
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 1, 0, 0, 0, 0, 0, 0]),
    Some([4, 3, 0, 12, 9, 1, 5, 4, 2]),
    Some(freelancer2.id()),
  ).await?;

  e.bounty_give_up(&e.disputed_bounties, bounty_id, Some(&freelancer3)).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 0, 0, 0, 0, 0, 0, 0]),
    Some([4, 3, 0, 12, 9, 1, 5, 4, 2]),
    Some(freelancer3.id()),
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 4);
  // The first freelancer is rejected
  assert_eq!(bounty_claims[0].0.to_string(), e.freelancer.id().to_string());
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::NotHired);
  // The second freelancer is selected
  assert_eq!(bounty_claims[1].0.to_string(), freelancer2.id().to_string());
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::InProgress);
  // The third freelancer surrendered and returned his bond
  assert_eq!(bounty_claims[2].0.to_string(), freelancer3.id().to_string());
  assert_eq!(bounty_claims[2].1.status, ClaimStatus::Canceled);
  // The fourth freelancer's claim remains with status "New"
  assert_eq!(bounty_claims[3].0.to_string(), freelancer4.id().to_string());
  assert_eq!(bounty_claims[3].1.status, ClaimStatus::New);

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::Claimed);

  // The second freelancer failed the task
  e.bounty_give_up(&e.disputed_bounties, bounty_id, Some(&freelancer2)).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 1, 0, 0, 0, 1, 0, 0]),
    Some([4, 3, 0, 12, 9, 1, 5, 4, 2]),
    Some(freelancer2.id()),
  ).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);

  // The fourth freelancer is selected
  e.decision_on_claim(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    true,
    Some(&freelancer4),
    None,
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 1, 0, 0, 0, 0, 0, 0]),
    Some([4, 3, 0, 12, 10, 1, 5, 4, 2]),
    Some(freelancer4.id()),
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 4);
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Canceled);
  assert_eq!(bounty_claims[3].1.status, ClaimStatus::InProgress);

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::Claimed);

  println!("      Passed ✅ Test - rejection and approval of claimers by project owner");
  Ok(())
}

async fn test_rejection_and_approval_of_claimers_by_validators_dao(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 4);

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("MultipleClaims"),
    Some(e.reviewer_is_dao().await?),
    None, None, None, None, None, None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([9, 8, 3, 3, 1, 1, 4, 2]),
    Some([5, 3, 0, 12, 10, 1, 5, 4, 2]),
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 5);

  let bounty_id = 4;
  let deadline = Some(U64(1_000_000_000 * 60 * 60 * 24 * 2));
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    deadline,
    "Test claim 1".to_string(),
    None,
    None, // by default freelancer
    None
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([10, 8, 3, 3, 1, 1, 4, 2]),
    Some([5, 3, 0, 13, 10, 1, 5, 4, 2]),
    None,
  ).await?;

  let freelancer2 = e.add_account("freelancer5").await?;
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    deadline,
    "Test claim 2".to_string(),
    None,
    Some(&freelancer2),
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 0, 0, 0, 0, 0, 0, 0]),
    Some([5, 3, 0, 14, 10, 1, 5, 4, 2]),
    Some(freelancer2.id()),
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 2);
  assert_eq!(bounty_claims[0].0.to_string(), e.freelancer.id().to_string());
  assert_eq!(bounty_claims[0].1.description, "Test claim 1".to_string());
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::New);
  assert_eq!(bounty_claims[1].0.to_string(), freelancer2.id().to_string());
  assert_eq!(bounty_claims[1].1.description, "Test claim 2".to_string());
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::New);
  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);

  e.bounty_action_by_validators_dao(
    &e.disputed_bounties,
    bounty_id,
    "VoteReject".to_string(),
    0,
    false,
  ).await?;

  // The bounty contract does not yet know that the claimer has been rejected in the DAO
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::New);

  // Finalization for this claim
  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.freelancer,
    &BountyAction::Finalize {
      receiver_id: Some(e.freelancer.id().to_string().parse().unwrap())
    },
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([10, 8, 3, 3, 1, 1, 4, 2]),
    Some([5, 3, 0, 14, 10, 1, 5, 4, 2]),
    None,
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::NotHired);
  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);

  e.bounty_action_by_validators_dao(
    &e.disputed_bounties,
    bounty_id,
    "VoteApprove".to_string(),
    1,
    false,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 1, 0, 0, 0, 0, 0, 0]),
    Some([5, 3, 0, 14, 11, 1, 5, 4, 2]),
    Some(freelancer2.id()),
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::InProgress);
  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::Claimed);

  println!("      Passed ✅ Test - rejection and approval of claimers by DAO");
  Ok(())
}

async fn test_using_more_reviewers(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 5);

  let reviewer = e.add_account("reviewer").await?;
  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("MultipleClaims"),
    Some(e.more_reviewers(&reviewer).await?),
    None, None, None, None, None, None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([10, 8, 3, 3, 1, 1, 4, 2]),
    Some([6, 3, 0, 14, 11, 1, 5, 4, 2]),
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 6);

  let bounty_id = 5;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    None, // by default freelancer
    None
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([11, 8, 3, 3, 1, 1, 4, 2]),
    Some([6, 3, 0, 15, 11, 1, 5, 4, 2]),
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::New,
    BountyStatus::New
  ).await?;

  e.decision_on_claim(
    &e.disputed_bounties,
    bounty_id,
    &reviewer,
    true,
    None,
    None,
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([11, 9, 3, 3, 1, 1, 4, 2]),
    Some([6, 3, 0, 15, 12, 1, 5, 4, 2]),
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::InProgress,
    BountyStatus::Claimed
  ).await?;

  e.bounty_done(
    &e.disputed_bounties,
    bounty_id,
    "test description".to_string(),
    None,
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::Completed,
    BountyStatus::Claimed
  ).await?;

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id.clone(),
    &reviewer,
    &BountyAction::ClaimApproved {
      receiver_id: e.freelancer.id().as_str().parse().unwrap()
    },
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([11, 9, 4, 3, 1, 1, 4, 2]),
    Some([6, 4, 0, 15, 12, 2, 5, 4, 2]),
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::Approved,
    BountyStatus::Completed
  ).await?;

  println!("      Passed ✅ Test - using more reviewers");
  Ok(())
}

async fn test_bounty_cancel(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 6);

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("MultipleClaims"),
    None, None, None, None, None, None, None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([11, 9, 4, 3, 1, 1, 4, 2]),
    Some([7, 4, 0, 15, 12, 2, 5, 4, 2]),
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 7);

  let bounty_id = 6;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    None, // by default freelancer
    None
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([12, 9, 4, 3, 1, 1, 4, 2]),
    Some([7, 4, 0, 16, 12, 2, 5, 4, 2]),
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::New,
    BountyStatus::New,
  ).await?;

  // Bounty cancellation is possible only with 'New' status. Regardless of the presence of new claims
  e.bounty_cancel(&e.disputed_bounties, bounty_id).await?;
  e.assert_reputation_stat_values_eq(
    Some([12, 9, 4, 3, 1, 1, 4, 2]),
    Some([7, 4, 1, 16, 12, 2, 5, 4, 2]),
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::New,
    BountyStatus::Canceled,
  ).await?;

  // To return the bonus, the freelancer must call 'bounty_give_up'
  e.bounty_give_up(&e.disputed_bounties, bounty_id, None).await?;
  e.assert_reputation_stat_values_eq(
    Some([12, 9, 4, 3, 1, 1, 4, 2]),
    Some([7, 4, 1, 16, 12, 2, 5, 4, 2]),
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    None,
    ClaimStatus::Canceled,
    BountyStatus::Canceled,
  ).await?;

  println!("      Passed ✅ Test - cancellation of the bounty by project owner");
  Ok(())
}

async fn test_bounty_update(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 7);

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None, None, None, None, None, None, None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([12, 9, 4, 3, 1, 1, 4, 2]),
    Some([8, 4, 1, 16, 12, 2, 5, 4, 2]),
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 8);

  let bounty_id = 7;

  let new_metadata = BountyMetadata {
    title: "New bounty title".to_string(),
    description: "New bounty description".to_string(),
    category: "Design".to_string(),
    attachments: Some(vec!["http://ipfs-url/3".to_string()]),
    experience: Some(Experience::Advanced),
    tags: Some(vec!["Community".to_string(), "web3".to_string()]),
    acceptance_criteria: Some("New acceptance criteria".to_string()),
    contact_details: Some(
      ContactDetails {
        contact: "username".to_string(),
        contact_type: ContactType::Twitter
      }
    ),
  };
  let reviewer2 = e.add_account("reviewer2").await?;
  let mut bounty_update = BountyUpdate {
    metadata: Some(new_metadata.clone()),
    deadline: Some(Deadline::MaxDeadline {
      max_deadline: U64(MAX_DEADLINE.0 / 2),
    }),
    claimer_approval: Some(ClaimerApproval::MultipleClaims),
    reviewers: Some(ReviewersParams::MoreReviewers {
      more_reviewers: vec![reviewer2.id().to_string().parse().unwrap()],
    }),
    amount: None,
  };

  e.bounty_update(&e.disputed_bounties, bounty_id, bounty_update).await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    None, // by default freelancer
    None
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([13, 9, 4, 3, 1, 1, 4, 2]),
    Some([8, 4, 1, 17, 12, 2, 5, 4, 2]),
    None,
  ).await?;

  let reviewer3 = e.add_account("reviewer3").await?;
  bounty_update = BountyUpdate {
    metadata: None, // Can't be changed if there are new claims
    deadline: Some(Deadline::MaxDeadline {
      // The deadline can only be extended if there are new claims
      max_deadline: U64(MAX_DEADLINE.0 + 1),
    }),
    claimer_approval: None, // Can't be changed if there are new claims
    reviewers: Some(ReviewersParams::MoreReviewers {
      more_reviewers: vec![
        reviewer2.id().to_string().parse().unwrap(),
        reviewer3.id().to_string().parse().unwrap(),
      ],
    }),
    amount: None,
  };

  e.bounty_update(&e.disputed_bounties, bounty_id, bounty_update).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(
    bounty,
    Bounty {
      token: Some(e.test_token.id().to_string().parse().unwrap()),
      amount: BOUNTY_AMOUNT,
      platform_fee: PLATFORM_FEE,
      dao_fee: U128(0),
      metadata: new_metadata,
      deadline: Deadline::MaxDeadline {
        max_deadline: U64(MAX_DEADLINE.0 + 1),
      },
      claimer_approval: ClaimerApproval::MultipleClaims,
      reviewers: Some(Reviewers::MoreReviewers {
        more_reviewers: vec![
          reviewer2.id().to_string().parse().unwrap(),
          reviewer3.id().to_string().parse().unwrap(),
        ],
      }),
      owner: e.project_owner.id().to_string().parse().unwrap(),
      status: BountyStatus::New,
      created_at: bounty.created_at,
      kyc_config: KycConfig::KycNotRequired,
      postpaid: None,
      multitasking: None,
      allow_deadline_stretch: false,
    }
  );

  println!("      Passed ✅ Test - updating the bounty by project owner");
  Ok(())
}

async fn test_kyc_whitelist_flow(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 8);

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None,
    Some(U128((BOUNTY_AMOUNT.0 + PLATFORM_FEE.0) * 2)),
    Some(KycConfig::default()),
    None, None, None, None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([13, 9, 4, 3, 1, 1, 4, 2]),
    Some([9, 4, 1, 17, 12, 2, 5, 4, 2]),
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 9);

  let bounty_id = 8;
  let mut freelancer = e.add_account("freelancer8").await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    Some(&freelancer),
    Some("The claimer is not whitelisted")
  ).await?;
  e.assert_reputation_stat_values_eq(
    None,
    Some([9, 4, 1, 17, 12, 2, 5, 4, 2]),
    Some(freelancer.id()),
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 0);

  e.add_account_to_kyc_whitelist_by_service(&freelancer).await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    Some(&freelancer),
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 1, 0, 0, 0, 0, 0, 0]),
    Some([9, 4, 1, 18, 13, 2, 5, 4, 2]),
    Some(freelancer.id()),
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 1);
  assert_eq!(bounty_claims[0].0.to_string(), freelancer.id().to_string());
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::InProgress);

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::Claimed);

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("MultipleClaims"),
    None, None,
    Some(KycConfig::KycRequired {
      kyc_verification_method: KycVerificationMethod::DuringClaimApproval
    }),
    None, None, None, None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([13, 9, 4, 3, 1, 1, 4, 2]),
    Some([10, 4, 1, 18, 13, 2, 5, 4, 2]),
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 10);

  let bounty_id = 9;
  freelancer = e.add_account("freelancer9").await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    Some(&freelancer),
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 0, 0, 0, 0, 0, 0, 0]),
    Some([10, 4, 1, 19, 13, 2, 5, 4, 2]),
    Some(freelancer.id()),
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 1);
  assert_eq!(bounty_claims[0].0.to_string(), freelancer.id().to_string());
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::New);

  e.decision_on_claim(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    true,
    Some(&freelancer),
    None,
    Some("The claimer is not whitelisted"),
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 0, 0, 0, 0, 0, 0, 0]),
    Some([10, 4, 1, 19, 13, 2, 5, 4, 2]),
    Some(freelancer.id()),
  ).await?;

  e.decision_on_claim(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    true,
    Some(&freelancer),
    Some(DefermentOfKYC::BeforeDeadline),
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 1, 0, 0, 0, 0, 0, 0]),
    Some([10, 4, 1, 19, 14, 2, 5, 4, 2]),
    Some(freelancer.id()),
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 1);
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::InProgress);

  e.bounty_done(
    &e.disputed_bounties,
    bounty_id,
    "test description".to_string(),
    Some(&freelancer),
    Some("The claimer is not whitelisted"),
  ).await?;

  e.add_account_to_kyc_whitelist_by_service(&freelancer).await?;

  e.bounty_done(
    &e.disputed_bounties,
    bounty_id,
    "test description".to_string(),
    Some(&freelancer),
    None,
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 1);
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::Completed);

  println!("      Passed ✅ Test - KYC whitelist flow");
  Ok(())
}

async fn test_use_commissions(e: &Env) -> anyhow::Result<()> {
  let contract_balance = e.get_token_balance(e.disputed_bounties.id()).await?;
  let bounty_owner_balance = e.get_token_balance(e.project_owner.id()).await?;
  let start_fee_recipient_balance = e.get_token_balance(e.bounties_contract_admin.id()).await?;
  let start_validator_dao_balance = e.get_token_balance(e.validators_dao.id()).await?;
  let start_platform_fees = e.get_platform_fees().await?;
  let start_dao_fees = e.get_dao_fees().await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 10);

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    Some(e.reviewer_is_dao().await?),
    None, None, None, None, None, None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([13, 9, 4, 3, 1, 1, 4, 2]),
    Some([11, 4, 1, 19, 14, 2, 5, 4, 2]),
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 11);
  let bounty_id = 10;

  let contract_balance_after_bounty_creating = e.get_token_balance(
    e.disputed_bounties.id()
  ).await?;
  assert_eq!(
    contract_balance + BOUNTY_AMOUNT.0 + PLATFORM_FEE.0 + DAO_FEE.0,
    contract_balance_after_bounty_creating
  );
  assert_eq!(
    bounty_owner_balance - BOUNTY_AMOUNT.0 - PLATFORM_FEE.0 - DAO_FEE.0,
    e.get_token_balance(e.project_owner.id()).await?
  );
  let last_platform_fees = e.get_platform_fees().await?;
  assert_eq!(
    start_platform_fees.balance.0 + PLATFORM_FEE.0,
    last_platform_fees.balance.0
  );
  assert_eq!(
    start_platform_fees.locked_balance.0 + PLATFORM_FEE.0,
    last_platform_fees.locked_balance.0
  );
  let last_dao_fees = e.get_dao_fees().await?;
  assert_eq!(
    start_dao_fees.balance.0 + DAO_FEE.0,
    last_dao_fees.balance.0
  );
  assert_eq!(
    start_dao_fees.locked_balance.0 + DAO_FEE.0,
    last_dao_fees.locked_balance.0
  );

  // Completion of the bounty
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 2)),
    "Test claim".to_string(),
    None,
    None, // by default freelancer
    None
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([14, 10, 4, 3, 1, 1, 4, 2]),
    Some([11, 4, 1, 20, 15, 2, 5, 4, 2]),
    None,
  ).await?;
  e.bounty_done(
    &e.disputed_bounties,
    bounty_id,
    "test description".to_string(),
    None,
    None,
  ).await?;
  e.bounty_action_by_validators_dao(
    &e.disputed_bounties,
    bounty_id,
    "VoteApprove".to_string(),
    0,
    true,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([14, 10, 5, 3, 1, 1, 4, 2]),
    Some([11, 5, 1, 20, 15, 3, 5, 4, 2]),
    None,
  ).await?;

  let contract_balance_after_approval = e.get_token_balance(
    e.disputed_bounties.id()
  ).await?;
  assert_eq!(
    contract_balance_after_bounty_creating - BOUNTY_AMOUNT.0,
    contract_balance_after_approval
  );
  let last_platform_fees = e.get_platform_fees().await?;
  assert_eq!(
    start_platform_fees.balance.0 + PLATFORM_FEE.0,
    last_platform_fees.balance.0
  );

  // The commission amount is unlocked and can be withdrawn
  assert_eq!(
    start_platform_fees.locked_balance.0,
    last_platform_fees.locked_balance.0
  );
  let platform_fee_amount_available = last_platform_fees.balance.0 - last_platform_fees.locked_balance.0;

  let last_dao_fees = e.get_dao_fees().await?;
  assert_eq!(
    start_dao_fees.balance.0 + DAO_FEE.0,
    last_dao_fees.balance.0
  );

  // The commission amount is unlocked and can be withdrawn
  assert_eq!(
    start_dao_fees.locked_balance.0,
    last_dao_fees.locked_balance.0
  );
  let dao_fee_amount_available = last_dao_fees.balance.0 - last_dao_fees.locked_balance.0;

  e.withdraw_platform_fee().await?;

  let contract_balance_after_platform_fee_withdrawal = e.get_token_balance(
    e.disputed_bounties.id()
  ).await?;
  assert_eq!(
    contract_balance_after_approval - platform_fee_amount_available,
    contract_balance_after_platform_fee_withdrawal
  );
  let fee_recipient_balance = e.get_token_balance(e.bounties_contract_admin.id()).await?;
  assert_eq!(
    start_fee_recipient_balance + platform_fee_amount_available,
    fee_recipient_balance
  );
  let final_platform_fees = e.get_platform_fees().await?;
  assert_eq!(
    last_platform_fees.locked_balance.0,
    final_platform_fees.balance.0
  );
  assert_eq!(
    last_platform_fees.locked_balance.0,
    final_platform_fees.locked_balance.0
  );

  e.withdraw_validators_dao_fee().await?;

  let contract_balance_after_dao_fee_withdrawal = e.get_token_balance(
    e.disputed_bounties.id()
  ).await?;
  assert_eq!(
    contract_balance_after_platform_fee_withdrawal - dao_fee_amount_available,
    contract_balance_after_dao_fee_withdrawal
  );
  let validator_dao_balance = e.get_token_balance(e.validators_dao.id()).await?;
  assert_eq!(
    start_validator_dao_balance + dao_fee_amount_available,
    validator_dao_balance
  );
  let final_dao_fees = e.get_dao_fees().await?;
  assert_eq!(
    last_dao_fees.locked_balance.0,
    final_dao_fees.balance.0
  );
  assert_eq!(
    last_dao_fees.locked_balance.0,
    final_dao_fees.locked_balance.0
  );

  println!("      Passed ✅ Test - use commissions");
  Ok(())
}

async fn test_creating_bounty_for_disabled_token(e: &Env) -> anyhow::Result<()> {
  e.update_token(
    &e.disputed_bounties,
    &TokenDetails {
      enabled: false,
      min_amount_for_kyc: Some(MIN_AMOUNT_FOR_KYC),
    },
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 11);

  e.assert_reputation_stat_values_eq(
    Some([14, 10, 5, 3, 1, 1, 4, 2]),
    Some([11, 5, 1, 20, 15, 3, 5, 4, 2]),
    None,
  ).await?;
  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None, None, None, None, None, None, None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([14, 10, 5, 3, 1, 1, 4, 2]),
    Some([11, 5, 1, 20, 15, 3, 5, 4, 2]),
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  // Bounty has not been created
  assert_eq!(last_bounty_id, 11);

  e.update_token(
    &e.disputed_bounties,
    &TokenDetails {
      enabled: true,
      min_amount_for_kyc: Some(MIN_AMOUNT_FOR_KYC),
    },
  ).await?;

  println!("      Passed ✅ Test - creating a bounty for a disabled token");
  Ok(())
}

async fn test_approval_by_whitelist_flow(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 11);

  let freelancer = e.add_account("freelancer10").await?;
  let freelancer2 = e.add_account("freelancer11").await?;

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!(
      {
        "ApprovalByWhitelist": json!(
          {
            "claimers_whitelist": vec![
              freelancer2.id().to_string()
            ]
          }
        )
      }
    ),
    None, None, None, None, None, None, None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([14, 10, 5, 3, 1, 1, 4, 2]),
    Some([12, 5, 1, 20, 15, 3, 5, 4, 2]),
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 12);

  let mut bounty_id = 11;
  let deadline = Some(U64(1_000_000_000 * 60 * 60 * 24 * 2));

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    deadline,
    "Test claim".to_string(),
    None,
    Some(&freelancer),
    Some("freelancer10.test.near is not whitelisted"),
  ).await?;
  e.assert_reputation_stat_values_eq(
    None,
    Some([12, 5, 1, 20, 15, 3, 5, 4, 2]),
    Some(freelancer.id()),
  ).await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    deadline,
    "Test claim 2".to_string(),
    None,
    Some(&freelancer2),
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 1, 0, 0, 0, 0, 0, 0]),
    Some([12, 5, 1, 21, 16, 3, 5, 4, 2]),
    Some(freelancer2.id()),
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 1);
  assert_eq!(bounty_claims[0].0.to_string(), freelancer2.id().to_string());
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::InProgress);

  let freelancer3 = e.add_account("freelancer12").await?;
  let freelancer4 = e.add_account("freelancer13").await?;
  let freelancer5 = e.add_account("freelancer14").await?;

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!(
      {
        "WhitelistWithApprovals": json!(
          {
            "claimers_whitelist": vec![
              freelancer4.id().to_string(),
              freelancer5.id().to_string()
            ]
          }
        )
      }
    ),
    None, None, None, None, None, None, None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([14, 10, 5, 3, 1, 1, 4, 2]),
    Some([13, 5, 1, 21, 16, 3, 5, 4, 2]),
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 13);

  bounty_id = 12;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    deadline,
    "Test claim".to_string(),
    None,
    Some(&freelancer3),
    Some("freelancer12.test.near is not whitelisted"),
  ).await?;
  e.assert_reputation_stat_values_eq(
    None,
    Some([13, 5, 1, 21, 16, 3, 5, 4, 2]),
    Some(freelancer3.id()),
  ).await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    deadline,
    "Test claim 2".to_string(),
    None,
    Some(&freelancer4),
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 0, 0, 0, 0, 0, 0, 0]),
    Some([13, 5, 1, 22, 16, 3, 5, 4, 2]),
    Some(freelancer4.id()),
  ).await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    deadline,
    "Test claim 3".to_string(),
    None,
    Some(&freelancer5),
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 0, 0, 0, 0, 0, 0, 0]),
    Some([13, 5, 1, 23, 16, 3, 5, 4, 2]),
    Some(freelancer5.id()),
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 2);
  assert_eq!(bounty_claims[0].0.to_string(), freelancer4.id().to_string());
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::New);
  assert_eq!(bounty_claims[1].0.to_string(), freelancer5.id().to_string());
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::New);

  e.decision_on_claim(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    true,
    Some(&freelancer5),
    None,
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 1, 0, 0, 0, 0, 0, 0]),
    Some([13, 5, 1, 23, 17, 3, 5, 4, 2]),
    Some(freelancer5.id()),
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 2);
  assert_eq!(bounty_claims[0].0.to_string(), freelancer4.id().to_string());
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::New);
  assert_eq!(bounty_claims[1].0.to_string(), freelancer5.id().to_string());
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::InProgress);

  println!("      Passed ✅ Test - approval by whitelist flow");
  Ok(())
}

async fn test_postpaid_flow(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  let freelancer = e.add_account("freelancer15").await?;
  let token_balance = e.get_token_balance(e.disputed_bounties.id()).await?;
  let freelancer_balance = e.get_token_balance(freelancer.id()).await?;
  Env::register_user(&e.test_token, freelancer.id()).await?;

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None,
    Some(BOUNTY_AMOUNT),
    None,
    Some(
      Postpaid::PaymentOutsideContract { currency: "USD".to_string(), payment_timestamps: None }
    ),
    None,
    Some("You are not allowed to create postpaid bounties"),
    None,
  ).await?;

  e.add_to_some_whitelist(
    e.project_owner.id(),
    WhitelistType::PostpaidSubscribersWhitelist
  ).await?;

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None,
    Some(BOUNTY_AMOUNT),
    None,
    Some(
      Postpaid::PaymentOutsideContract { currency: "EUR".to_string(), payment_timestamps: None }
    ),
    None,
    Some("Invalid currency EUR"),
    None,
  ).await?;

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None,
    Some(BOUNTY_AMOUNT),
    None,
    Some(
      Postpaid::PaymentOutsideContract { currency: "USD".to_string(), payment_timestamps: None }
    ),
    None, None, None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    None,
    Some([14, 5, 1, 23, 17, 3, 5, 4, 2]),
    Some(freelancer.id()),
  ).await?;

  let next_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(next_bounty_id, last_bounty_id + 1);
  let bounty_id = last_bounty_id;
  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.amount, BOUNTY_AMOUNT);
  assert_eq!(bounty.platform_fee, U128(0));
  assert_eq!(bounty.dao_fee, U128(0));

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    Some(&freelancer),
    None
  ).await?;
  e.bounty_done(
    &e.disputed_bounties,
    bounty_id,
    "test description".to_string(),
    Some(&freelancer),
    None
  ).await?;

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimApproved {
      receiver_id: freelancer.id().as_str().parse().unwrap()
    },
    Some("No payment confirmation"),
  ).await?;

  e.external_ft_transfer(&e.project_owner, &freelancer, BOUNTY_AMOUNT).await?;
  assert_eq!(
    e.get_token_balance(freelancer.id()).await?,
    freelancer_balance + BOUNTY_AMOUNT.0
  );

  e.mark_as_paid(&e.disputed_bounties, bounty_id, None).await?;

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimApproved {
      receiver_id: freelancer.id().as_str().parse().unwrap()
    },
    Some("No payment confirmation"),
  ).await?;

  e.confirm_payment(&e.disputed_bounties, bounty_id, Some(&freelancer)).await?;

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimApproved {
      receiver_id: freelancer.id().as_str().parse().unwrap()
    },
    None,
  ).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::Completed);
  // Since then, the escrow and freelancer balances have not changed
  assert_eq!(e.get_token_balance(e.disputed_bounties.id()).await?, token_balance);
  e.assert_reputation_stat_values_eq(
    Some([1, 1, 1, 0, 0, 0, 0, 0]),
    Some([14, 6, 1, 24, 18, 4, 5, 4, 2]),
    Some(freelancer.id()),
  ).await?;

  e.remove_from_some_whitelist(
    e.project_owner.id(),
    WhitelistType::PostpaidSubscribersWhitelist
  ).await?;

  println!("      Passed ✅ Test - postpaid flow");
  Ok(())
}

async fn test_competition_flow(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None, None, None, None,
    Some(Multitasking::ContestOrHackathon {
      allowed_create_claim_to: None,
      successful_claims_for_result: None,
      start_conditions: None,
      runtime_env: None,
    }),
    None, None,
  ).await?;

  let next_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(next_bounty_id, last_bounty_id + 1);
  let bounty_id = last_bounty_id;
  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);

  let freelancer = e.add_account("freelancer16").await?;
  Env::register_user(&e.test_token, freelancer.id()).await?;
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    Some(&freelancer),
    None
  ).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::ManyClaimed);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 1);
  assert_eq!(bounty_claims[0].0.to_string(), freelancer.id().to_string());
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::Competes);

  e.bounty_give_up(&e.disputed_bounties, bounty_id, Some(&freelancer)).await?;
  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 1);
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::Canceled);

  let freelancer = e.add_account("freelancer17").await?;
  Env::register_user(&e.test_token, freelancer.id()).await?;
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    Some(&freelancer),
    None
  ).await?;
  e.bounty_done(
    &e.disputed_bounties,
    bounty_id,
    "test description".to_string(),
    Some(&freelancer),
    None
  ).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::ManyClaimed);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 2);
  assert_eq!(bounty_claims[1].0.to_string(), freelancer.id().to_string());
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Completed);

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimRejected { receiver_id: freelancer.id().as_str().parse().unwrap() },
    None,
  ).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 2);
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::NotCompleted);

  let mut freelancers: Vec<Account> = vec![];
  for x in 0..2 {
    let freelancer_name = format!("freelancer{}", x + 18);
    let freelancer = e.add_account(freelancer_name.as_str()).await?;
    //println!("{}", freelancer.id());

    Env::register_user(&e.test_token, freelancer.id()).await?;

    e.bounty_claim(
      &e.disputed_bounties,
      bounty_id,
      Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
      "Test claim".to_string(),
      None,
      Some(&freelancer),
      None
    ).await?;

    e.bounty_done(
      &e.disputed_bounties,
      bounty_id,
      "test description".to_string(),
      Some(&freelancer),
      None
    ).await?;

    freelancers.push(freelancer)
  }

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimApproved {
      receiver_id: freelancers[0].id().to_string().parse().unwrap()
    },
    None,
  ).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::Completed);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims.len(), 4);
  assert_eq!(bounty_claims[2].0.to_string(), freelancers[0].id().to_string());
  assert_eq!(bounty_claims[2].1.status, ClaimStatus::Approved);
  assert_eq!(bounty_claims[3].0.to_string(), freelancers[1].id().to_string());
  assert_eq!(bounty_claims[3].1.status, ClaimStatus::Completed);

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &freelancers[1],
    &BountyAction::Finalize {
      receiver_id: Some(freelancers[1].id().to_string().parse().unwrap())
    },
    None,
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims[3].1.status, ClaimStatus::NotCompleted);

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None, None, None, None,
    Some(Multitasking::ContestOrHackathon {
      allowed_create_claim_to: None,
      successful_claims_for_result: None,
      start_conditions: Some(StartConditions::ManuallyStart),
      runtime_env: None,
    }),
    None, None,
  ).await?;

  let next_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(next_bounty_id, last_bounty_id + 2);
  let bounty_id = last_bounty_id + 1;
  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);
  //println!("{:#?}", bounty);

  let mut freelancers: Vec<Account> = vec![];
  for x in 0..3 {
    let freelancer_name = format!("freelancer{}", x + 20);
    let freelancer = e.add_account(freelancer_name.as_str()).await?;
    //println!("{}", freelancer.id());

    Env::register_user(&e.test_token, freelancer.id()).await?;

    e.bounty_claim(
      &e.disputed_bounties,
      bounty_id,
      Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
      "Test claim".to_string(),
      None,
      Some(&freelancer),
      None
    ).await?;

    freelancers.push(freelancer)
  }

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims.len(), 3);
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::Competes);
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Competes);
  assert_eq!(bounty_claims[2].1.status, ClaimStatus::Competes);

  e.start_competition(bounty_id).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::ManyClaimed);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims.len(), 3);
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::Competes);
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Competes);
  assert_eq!(bounty_claims[2].1.status, ClaimStatus::Competes);

  for x in 0..3 {
    if x == 2 {
      continue;
    }

    e.bounty_done(
      &e.disputed_bounties,
      bounty_id,
      "test description".to_string(),
      Some(&freelancers[x]),
      None
    ).await?;

    //println!("{}", freelancers[x].id());
  }

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimApproved {
      receiver_id: freelancers[1].id().to_string().parse().unwrap()
    },
    None,
  ).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::Completed);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims.len(), 3);
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::Completed);
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Approved);
  assert_eq!(bounty_claims[2].1.status, ClaimStatus::Competes);

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &freelancers[0],
    &BountyAction::Finalize {
      receiver_id: Some(freelancers[0].id().to_string().parse().unwrap())
    },
    None,
  ).await?;

  e.bounty_done(
    &e.disputed_bounties,
    bounty_id,
    "test description".to_string(),
    Some(&freelancers[2]),
    None
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::NotCompleted);
  assert_eq!(bounty_claims[2].1.status, ClaimStatus::NotCompleted);

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None, None, None, None,
    Some(Multitasking::ContestOrHackathon {
      allowed_create_claim_to: None,
      successful_claims_for_result: None,
      start_conditions: Some(StartConditions::MinAmountToStart { amount: 3 }),
      runtime_env: None,
    }),
    None, None,
  ).await?;

  let next_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(next_bounty_id, last_bounty_id + 3);
  let bounty_id = last_bounty_id + 2;
  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);
  //println!("{:#?}", bounty);

  let mut freelancers: Vec<Account> = vec![];
  for x in 0..2 {
    let freelancer_name = format!("freelancer{}", x + 23);
    let freelancer = e.add_account(freelancer_name.as_str()).await?;
    //println!("{}", freelancer.id());

    Env::register_user(&e.test_token, freelancer.id()).await?;

    e.bounty_claim(
      &e.disputed_bounties,
      bounty_id,
      Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
      "Test claim".to_string(),
      None,
      Some(&freelancer),
      None
    ).await?;

    freelancers.push(freelancer)
  }

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims.len(), 2);
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::Competes);
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Competes);

  for x in 0..2 {
    let freelancer_name = format!("freelancer{}", x + 25);
    let freelancer = e.add_account(freelancer_name.as_str()).await?;
    //println!("{}", freelancer.id());

    Env::register_user(&e.test_token, freelancer.id()).await?;

    e.bounty_claim(
      &e.disputed_bounties,
      bounty_id,
      Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
      "Test claim".to_string(),
      None,
      Some(&freelancer),
      None
    ).await?;

    freelancers.push(freelancer)
  }

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::ManyClaimed);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims.len(), 4);
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::Competes);
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Competes);
  assert_eq!(bounty_claims[2].1.status, ClaimStatus::Competes);
  assert_eq!(bounty_claims[3].1.status, ClaimStatus::Competes);

  for x in 0..4 {
    if x == 0 {
      continue;
    }

    e.bounty_done(
      &e.disputed_bounties,
      bounty_id,
      "test description".to_string(),
      Some(&freelancers[x]),
      None
    ).await?;

    //println!("{}", freelancers[x].id());
  }

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimApproved {
      receiver_id: freelancers[1].id().to_string().parse().unwrap()
    },
    None,
  ).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::Completed);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims.len(), 4);
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::Competes);
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Approved);
  assert_eq!(bounty_claims[2].1.status, ClaimStatus::Completed);
  assert_eq!(bounty_claims[3].1.status, ClaimStatus::Completed);

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &freelancers[0],
    &BountyAction::Finalize {
      receiver_id: Some(freelancers[0].id().to_string().parse().unwrap())
    },
    None,
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::NotCompleted);

  e.add_to_some_whitelist(
    e.project_owner.id(),
    WhitelistType::PostpaidSubscribersWhitelist
  ).await?;

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None,
    Some(BOUNTY_AMOUNT),
    None,
    Some(
      Postpaid::PaymentOutsideContract { currency: "USD".to_string(), payment_timestamps: None }
    ),
    Some(Multitasking::ContestOrHackathon {
      allowed_create_claim_to: None,
      successful_claims_for_result: None,
      start_conditions: None,
      runtime_env: None,
    }),
    None, None,
  ).await?;

  e.remove_from_some_whitelist(
    e.project_owner.id(),
    WhitelistType::PostpaidSubscribersWhitelist
  ).await?;

  let next_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(next_bounty_id, last_bounty_id + 4);
  let bounty_id = last_bounty_id + 3;
  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);
  //println!("{:#?}", bounty);

  let mut freelancers: Vec<Account> = vec![];
  for x in 0..2 {
    let freelancer_name = format!("freelancer{}", x + 27);
    let freelancer = e.add_account(freelancer_name.as_str()).await?;
    //println!("{}", freelancer.id());

    Env::register_user(&e.test_token, freelancer.id()).await?;

    e.bounty_claim(
      &e.disputed_bounties,
      bounty_id,
      Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
      "Test claim".to_string(),
      None,
      Some(&freelancer),
      None
    ).await?;

    e.bounty_done(
      &e.disputed_bounties,
      bounty_id,
      "test description".to_string(),
      Some(&freelancer),
      None
    ).await?;

    freelancers.push(freelancer)
  }

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert!(bounty.postpaid.clone().unwrap().get_payment_timestamps().payment_at.is_none());
  assert!(bounty.postpaid.clone().unwrap().get_payment_timestamps().payment_confirmed_at.is_none());

  e.mark_as_paid(&e.disputed_bounties, bounty_id, Some(freelancers[0].id())).await?;
  e.confirm_payment(&e.disputed_bounties, bounty_id, Some(&freelancers[0])).await?;

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimApproved {
      receiver_id: freelancers[0].id().to_string().parse().unwrap()
    },
    None,
  ).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::Completed);
  assert!(bounty.postpaid.clone().unwrap().get_payment_timestamps().payment_at.is_some());
  assert!(bounty.postpaid.clone().unwrap().get_payment_timestamps().payment_confirmed_at.is_some());
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims.len(), 2);
  assert_eq!(bounty_claims[0].0.to_string(), freelancers[0].id().to_string());
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::Approved);
  assert_eq!(bounty_claims[1].0.to_string(), freelancers[1].id().to_string());
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Completed);

  println!("      Passed ✅ Test - competition flow");
  Ok(())
}

async fn test_one_bounty_for_many(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  let token_balance = e.get_token_balance(e.disputed_bounties.id()).await?;
  let owner_balance = e.get_token_balance(e.project_owner.id()).await?;

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None, None, None, None,
    Some(Multitasking::OneForAll {
      number_of_slots: 2,
      amount_per_slot: U128((BOUNTY_AMOUNT.0 + PLATFORM_FEE.0) / 2),
      min_slots_to_start: None,
      runtime_env: None,
    }),
    None, None,
  ).await?;

  assert_eq!(
    token_balance + BOUNTY_AMOUNT.0 + PLATFORM_FEE.0,
    e.get_token_balance(e.disputed_bounties.id()).await?
  );
  assert_eq!(
    owner_balance - BOUNTY_AMOUNT.0 - PLATFORM_FEE.0,
    e.get_token_balance(e.project_owner.id()).await?
  );

  let next_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(next_bounty_id, last_bounty_id + 1);
  let bounty_id = last_bounty_id;
  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);

  let mut freelancers: Vec<Account> = vec![];
  for x in 0..2 {
    let freelancer_name = format!("freelancer{}", x + 29);
    let freelancer = e.add_account(freelancer_name.as_str()).await?;
    //println!("{}", freelancer.id());
    freelancers.push(freelancer.clone());
    Env::register_user(&e.test_token, freelancer.id()).await?;

    e.bounty_claim(
      &e.disputed_bounties,
      bounty_id,
      Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
      "Test claim".to_string(),
      None,
      Some(&freelancer),
      None
    ).await?;

    if x == 1 {
      continue;
    }

    e.bounty_done(
      &e.disputed_bounties,
      bounty_id,
      "test description".to_string(),
      Some(&freelancer),
      None
    ).await?;
  }

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::ManyClaimed);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims.len(), 2);
  assert_eq!(bounty_claims[0].0.to_string(), freelancers[0].id().to_string());
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::Completed);
  assert_eq!(bounty_claims[1].0.to_string(), freelancers[1].id().to_string());
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::InProgress);

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimRejected { receiver_id: freelancers[0].id().as_str().parse().unwrap() },
    None,
  ).await?;
  e.bounty_give_up(&e.disputed_bounties, bounty_id, Some(&freelancers[1])).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::ManyClaimed);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::Rejected);
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Canceled);

  // Period for opening a dispute: 10 min, wait for 500 blocks
  e.worker.fast_forward(500).await?;
  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::Finalize { receiver_id: Some(freelancers[0].id().to_string().parse().unwrap()) },
    None,
  ).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::NotCompleted);
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Canceled);

  for x in 0..2 {
    let freelancer_name = format!("freelancer{}", x + 31);
    let freelancer = e.add_account(freelancer_name.as_str()).await?;
    //println!("{}", freelancer.id());
    freelancers.push(freelancer.clone());
    Env::register_user(&e.test_token, freelancer.id()).await?;

    e.bounty_claim(
      &e.disputed_bounties,
      bounty_id,
      Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
      "Test claim".to_string(),
      None,
      Some(&freelancer),
      None
    ).await?;

    if x == 1 {
      continue;
    }

    e.bounty_done(
      &e.disputed_bounties,
      bounty_id,
      "test description".to_string(),
      Some(&freelancer),
      None
    ).await?;
  }

  let freelancer_balance = e.get_token_balance(freelancers[2].id()).await?;
  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::ManyClaimed);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims.len(), 4);
  assert_eq!(bounty_claims[2].0.to_string(), freelancers[2].id().to_string());
  assert_eq!(bounty_claims[2].1.status, ClaimStatus::Completed);
  assert_eq!(bounty_claims[3].0.to_string(), freelancers[3].id().to_string());
  assert_eq!(bounty_claims[3].1.status, ClaimStatus::InProgress);

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimApproved {
      receiver_id: freelancers[2].id().to_string().parse().unwrap()
    },
    None,
  ).await?;
  e.bounty_give_up(&e.disputed_bounties, bounty_id, Some(&freelancers[3])).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::AwaitingClaims);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims[2].1.status, ClaimStatus::Approved);
  assert_eq!(bounty_claims[3].1.status, ClaimStatus::Canceled);

  e.bounty_cancel(&e.disputed_bounties, bounty_id).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::PartiallyCompleted);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::NotCompleted);
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Canceled);
  assert_eq!(bounty_claims[2].1.status, ClaimStatus::Approved);
  assert_eq!(bounty_claims[3].1.status, ClaimStatus::Canceled);

  assert_eq!(
    freelancer_balance + BOUNTY_AMOUNT.0 / 2,
    e.get_token_balance(freelancers[2].id()).await?
  );
  assert_eq!(
    token_balance + PLATFORM_FEE.0 / 2,
    e.get_token_balance(e.disputed_bounties.id()).await?
  );
  assert_eq!(
    owner_balance - BOUNTY_AMOUNT.0 / 2 - PLATFORM_FEE.0 / 2,
    e.get_token_balance(e.project_owner.id()).await?
  );

  let token_balance = e.get_token_balance(e.disputed_bounties.id()).await?;
  let owner_balance = e.get_token_balance(e.project_owner.id()).await?;

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None, None, None, None,
    Some(Multitasking::OneForAll {
      number_of_slots: 4,
      amount_per_slot: U128((BOUNTY_AMOUNT.0 + PLATFORM_FEE.0) / 4),
      min_slots_to_start: Some(3),
      runtime_env: None,
    }),
    None, None,
  ).await?;

  assert_eq!(
    token_balance + BOUNTY_AMOUNT.0 + PLATFORM_FEE.0,
    e.get_token_balance(e.disputed_bounties.id()).await?
  );
  assert_eq!(
    owner_balance - BOUNTY_AMOUNT.0 - PLATFORM_FEE.0,
    e.get_token_balance(e.project_owner.id()).await?
  );

  let next_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(next_bounty_id, last_bounty_id + 2);
  let bounty_id = last_bounty_id + 1;
  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);
  //println!("{:#?}", bounty);

  let mut freelancers: Vec<Account> = vec![];
  let mut freelancers_balances: Vec<u128> = vec![];
  for x in 0..2 {
    let freelancer_name = format!("freelancer{}", x + 33);
    let freelancer = e.add_account(freelancer_name.as_str()).await?;
    //println!("{}", freelancer.id());
    Env::register_user(&e.test_token, freelancer.id()).await?;
    freelancers_balances.push(e.get_token_balance(freelancer.id()).await?);

    e.bounty_claim(
      &e.disputed_bounties,
      bounty_id,
      Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
      "Test claim".to_string(),
      None,
      Some(&freelancer),
      None
    ).await?;

    freelancers.push(freelancer);
  }

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims.len(), 2);
  assert_eq!(bounty_claims[0].0.to_string(), freelancers[0].id().to_string());
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::ReadyToStart);
  assert_eq!(bounty_claims[1].0.to_string(), freelancers[1].id().to_string());
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::ReadyToStart);

  let freelancer = e.add_account("freelancer35").await?;
  Env::register_user(&e.test_token, freelancer.id()).await?;
  freelancers_balances.push(e.get_token_balance(freelancer.id()).await?);
  freelancers.push(freelancer.clone());

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    Some(&freelancer),
    None
  ).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::ManyClaimed);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims.len(), 3);
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::InProgress);
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::InProgress);
  assert_eq!(bounty_claims[2].0.to_string(), freelancers[2].id().to_string());
  assert_eq!(bounty_claims[2].1.status, ClaimStatus::InProgress);

  for x in 0..3 {
    e.bounty_done(
      &e.disputed_bounties,
      bounty_id,
      "test description".to_string(),
      Some(&freelancers[x]),
      None
    ).await?;

    Env::bounty_action_by_user(
      &e.disputed_bounties,
      bounty_id,
      &e.project_owner,
      &BountyAction::ClaimApproved {
        receiver_id: freelancers[x].id().to_string().parse().unwrap()
      },
      None,
    ).await?;
  }

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::AwaitingClaims);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::Approved);
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Approved);
  assert_eq!(bounty_claims[2].1.status, ClaimStatus::Approved);

  let freelancer = e.add_account("freelancer36").await?;
  Env::register_user(&e.test_token, freelancer.id()).await?;
  freelancers_balances.push(e.get_token_balance(freelancer.id()).await?);
  freelancers.push(freelancer.clone());

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    Some(&freelancer),
    None
  ).await?;

  e.bounty_done(
    &e.disputed_bounties,
    bounty_id,
    "test description".to_string(),
    Some(&freelancer),
    None
  ).await?;

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimApproved {
      receiver_id: freelancer.id().to_string().parse().unwrap()
    },
    None,
  ).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::Completed);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims.len(), 4);
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::Approved);
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Approved);
  assert_eq!(bounty_claims[2].1.status, ClaimStatus::Approved);
  assert_eq!(bounty_claims[3].0.to_string(), freelancer.id().to_string());
  assert_eq!(bounty_claims[3].1.status, ClaimStatus::Approved);

  assert_eq!(
    token_balance + PLATFORM_FEE.0,
    e.get_token_balance(e.disputed_bounties.id()).await?
  );
  assert_eq!(
    freelancers_balances[0] + BOUNTY_AMOUNT.0 / 4,
    e.get_token_balance(freelancers[0].id()).await?
  );
  assert_eq!(
    freelancers_balances[1] + BOUNTY_AMOUNT.0 / 4,
    e.get_token_balance(freelancers[1].id()).await?
  );
  assert_eq!(
    freelancers_balances[2] + BOUNTY_AMOUNT.0 / 4,
    e.get_token_balance(freelancers[2].id()).await?
  );
  assert_eq!(
    freelancers_balances[3] + BOUNTY_AMOUNT.0 / 4,
    e.get_token_balance(freelancers[3].id()).await?
  );

  e.add_to_some_whitelist(
    e.project_owner.id(),
    WhitelistType::PostpaidSubscribersWhitelist
  ).await?;

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None,
    Some(BOUNTY_AMOUNT),
    None,
    Some(
      Postpaid::PaymentOutsideContract { currency: "USD".to_string(), payment_timestamps: None }
    ),
    Some(Multitasking::OneForAll {
      number_of_slots: 2,
      amount_per_slot: U128(BOUNTY_AMOUNT.0 / 2),
      min_slots_to_start: None,
      runtime_env: None,
    }),
    None, None,
  ).await?;

  e.remove_from_some_whitelist(
    e.project_owner.id(),
    WhitelistType::PostpaidSubscribersWhitelist
  ).await?;

  let next_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(next_bounty_id, last_bounty_id + 3);
  let bounty_id = last_bounty_id + 2;
  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);
  //println!("{:#?}", bounty);

  let mut freelancers: Vec<Account> = vec![];
  for x in 0..2 {
    let freelancer_name = format!("freelancer{}", x + 37);
    let freelancer = e.add_account(freelancer_name.as_str()).await?;
    //println!("{}", freelancer.id());
    freelancers.push(freelancer.clone());
    Env::register_user(&e.test_token, freelancer.id()).await?;

    e.bounty_claim(
      &e.disputed_bounties,
      bounty_id,
      Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
      "Test claim".to_string(),
      None,
      Some(&freelancers[x]),
      None
    ).await?;

    e.bounty_done(
      &e.disputed_bounties,
      bounty_id,
      "test description".to_string(),
      Some(&freelancers[x]),
      None
    ).await?;

    e.mark_as_paid(&e.disputed_bounties, bounty_id, Some(freelancers[x].id())).await?;
    e.confirm_payment(&e.disputed_bounties, bounty_id, Some(&freelancers[x])).await?;

    Env::bounty_action_by_user(
      &e.disputed_bounties,
      bounty_id,
      &e.project_owner,
      &BountyAction::ClaimApproved {
        receiver_id: freelancers[x].id().to_string().parse().unwrap()
      },
      None,
    ).await?;
  }

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::Completed);
  assert!(bounty.postpaid.clone().unwrap().get_payment_timestamps().payment_at.is_none());
  assert!(bounty.postpaid.clone().unwrap().get_payment_timestamps().payment_confirmed_at.is_none());
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims.len(), 2);
  assert_eq!(bounty_claims[0].0.to_string(), freelancers[0].id().to_string());
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::Approved);
  assert!(bounty_claims[0].1.payment_timestamps.clone().unwrap().payment_at.is_some());
  assert!(bounty_claims[0].1.payment_timestamps.clone().unwrap().payment_confirmed_at.is_some());
  assert_eq!(bounty_claims[1].0.to_string(), freelancers[1].id().to_string());
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Approved);
  assert!(bounty_claims[1].1.payment_timestamps.clone().unwrap().payment_at.is_some());
  assert!(bounty_claims[1].1.payment_timestamps.clone().unwrap().payment_confirmed_at.is_some());

  println!("      Passed ✅ Test - one bounty for all claimants");
  Ok(())
}

async fn test_different_tasks_flow(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  let token_balance = e.get_token_balance(e.disputed_bounties.id()).await?;
  let owner_balance = e.get_token_balance(e.project_owner.id()).await?;

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None, None, None, None,
    Some(Multitasking::DifferentTasks {
      subtasks: vec![
        Subtask {
          subtask_description: "Subtask 1".to_string(),
          subtask_percent: 20_000, // 20% slot 0
        },
        Subtask {
          subtask_description: "Subtask 2".to_string(),
          subtask_percent: 80_000, // 80% slot 1
        },
      ],
      runtime_env: None,
    }),
    None, None,
  ).await?;

  assert_eq!(
    token_balance + BOUNTY_AMOUNT.0 + PLATFORM_FEE.0,
    e.get_token_balance(e.disputed_bounties.id()).await?
  );
  assert_eq!(
    owner_balance - BOUNTY_AMOUNT.0 - PLATFORM_FEE.0,
    e.get_token_balance(e.project_owner.id()).await?
  );

  let next_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(next_bounty_id, last_bounty_id + 1);
  let bounty_id = last_bounty_id;
  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);

  let mut freelancers: Vec<Account> = vec![];
  let mut freelancers_balances: Vec<u128> = vec![];

  for x in 0..2 {
    let freelancer_name = format!("freelancer{}", x + 39);
    let freelancer = e.add_account(freelancer_name.as_str()).await?;
    //println!("{}", freelancer.id());
    freelancers.push(freelancer.clone());
    Env::register_user(&e.test_token, freelancer.id()).await?;
    freelancers_balances.push(e.get_token_balance(freelancer.id()).await?);

    e.bounty_claim(
      &e.disputed_bounties,
      bounty_id,
      Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
      "Test claim".to_string(),
      Some(x),
      Some(&freelancer),
      None
    ).await?;

    if x == 0 {
      let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
      assert_eq!(bounty.status, BountyStatus::New);
      let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
      assert_eq!(bounty_claims.len(), 1);
      assert_eq!(bounty_claims[0].1.status, ClaimStatus::ReadyToStart);

      e.bounty_done(
        &e.disputed_bounties,
        bounty_id,
        "test description".to_string(),
        Some(&freelancers[0]),
        Some("Bounty status does not allow to completion")
      ).await?;
    }
  }

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::ManyClaimed);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims.len(), 2);
  assert_eq!(bounty_claims[0].0.to_string(), freelancers[0].id().to_string());
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::InProgress);
  assert_eq!(bounty_claims[0].1.slot, Some(0));
  assert_eq!(bounty_claims[1].0.to_string(), freelancers[1].id().to_string());
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::InProgress);
  assert_eq!(bounty_claims[1].1.slot, Some(1));

  e.bounty_done(
    &e.disputed_bounties,
    bounty_id,
    "test description".to_string(),
    Some(&freelancers[0]),
    None
  ).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::ManyClaimed);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::Completed);
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::InProgress);

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimApproved {
      receiver_id: freelancers[0].id().to_string().parse().unwrap()
    },
    Some("This action is not available for DifferentTasks mode"),
  ).await?;

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::SeveralClaimsApproved,
    Some("Not all tasks have already been completed"),
  ).await?;

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimRejected { receiver_id: freelancers[0].id().as_str().parse().unwrap() },
    None,
  ).await?;
  e.bounty_give_up(&e.disputed_bounties, bounty_id, Some(&freelancers[1])).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::ManyClaimed);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::Rejected);
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Canceled);

  // Period for opening a dispute: 10 min, wait for 500 blocks
  e.worker.fast_forward(500).await?;
  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::Finalize { receiver_id: Some(freelancers[0].id().to_string().parse().unwrap()) },
    None,
  ).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::NotCompleted);
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Canceled);

  for x in 0..2 {
    let freelancer_name = format!("freelancer{}", x + 41);
    let freelancer = e.add_account(freelancer_name.as_str()).await?;
    //println!("{}", freelancer.id());
    freelancers.push(freelancer.clone());
    Env::register_user(&e.test_token, freelancer.id()).await?;
    freelancers_balances.push(e.get_token_balance(freelancer.id()).await?);

    e.bounty_claim(
      &e.disputed_bounties,
      bounty_id,
      Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
      "Test claim".to_string(),
      Some(x),
      Some(&freelancer),
      None
    ).await?;
  }

  for x in 0..2 {
    e.bounty_done(
      &e.disputed_bounties,
      bounty_id,
      "test description".to_string(),
      Some(&freelancers[x + 2]),
      None
    ).await?;
  }

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimRejected { receiver_id: freelancers[3].id().as_str().parse().unwrap() },
    None,
  ).await?;

  e.open_dispute(&e.disputed_bounties, bounty_id, Some(&freelancers[3])).await?;

  let dispute_id = e.get_last_dispute_id().await? - 1;
  let dispute = e.get_dispute(dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::New);

  // Argument period: 10 min, wait for 500 blocks
  e.worker.fast_forward(500).await?;

  e.escalation(
    dispute_id,
    &freelancers[3],
  ).await?;

  let dispute = e.get_dispute(dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::DecisionPending);
  let proposal_id = dispute.proposal_id.unwrap().0;

  e.dispute_dao_action(proposal_id, "VoteApprove".to_string()).await?;

  let dispute = e.get_dispute(dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::InFavorOfClaimer);

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::ManyClaimed);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims.len(), 4);
  assert_eq!(bounty_claims[2].0.to_string(), freelancers[2].id().to_string());
  assert_eq!(bounty_claims[2].1.status, ClaimStatus::Completed);
  assert_eq!(bounty_claims[2].1.slot, Some(0));
  assert_eq!(bounty_claims[3].0.to_string(), freelancers[3].id().to_string());
  assert_eq!(bounty_claims[3].1.status, ClaimStatus::CompletedWithDispute);
  assert_eq!(bounty_claims[3].1.slot, Some(1));

  e.withdraw(
    bounty_id,
    Some(&freelancers[2]),
    Some("Bounty status does not allow this action")
  ).await?;

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::SeveralClaimsApproved,
    None,
  ).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::Completed);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims.len(), 4);
  assert_eq!(bounty_claims[2].1.status, ClaimStatus::Completed);
  assert_eq!(bounty_claims[3].1.status, ClaimStatus::CompletedWithDispute);

  e.withdraw(bounty_id, Some(&freelancers[2]), None).await?;
  e.withdraw(bounty_id, Some(&freelancers[3]), None).await?;
  e.withdraw(
    bounty_id,
    Some(&freelancers[2]),
    Some("The claim status does not allow this action")
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims.len(), 4);
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::NotCompleted);
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Canceled);
  assert_eq!(bounty_claims[2].1.status, ClaimStatus::Approved);
  assert_eq!(bounty_claims[3].1.status, ClaimStatus::Approved);

  assert_eq!(
    token_balance + PLATFORM_FEE.0,
    e.get_token_balance(e.disputed_bounties.id()).await?
  );
  assert_eq!(
    freelancers_balances[2] + BOUNTY_AMOUNT.0 * 20_000 / 100_000, // 20% slot 0
    e.get_token_balance(freelancers[2].id()).await?
  );
  assert_eq!(
    freelancers_balances[3] + BOUNTY_AMOUNT.0 * 80_000 / 100_000, // 80% slot 1
    e.get_token_balance(freelancers[3].id()).await?
  );

  e.add_to_some_whitelist(
    e.project_owner.id(),
    WhitelistType::PostpaidSubscribersWhitelist
  ).await?;
  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None,
    Some(BOUNTY_AMOUNT),
    None,
    Some(
      Postpaid::PaymentOutsideContract { currency: "USD".to_string(), payment_timestamps: None }
    ),
    Some(Multitasking::DifferentTasks {
      subtasks: vec![
        Subtask {
          subtask_description: "Subtask 1".to_string(),
          subtask_percent: 20_000, // 20% slot 0
        },
        Subtask {
          subtask_description: "Subtask 2".to_string(),
          subtask_percent: 80_000, // 80% slot 1
        },
      ],
      runtime_env: None,
    }),
    None, None,
  ).await?;

  e.remove_from_some_whitelist(
    e.project_owner.id(),
    WhitelistType::PostpaidSubscribersWhitelist
  ).await?;

  let next_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(next_bounty_id, last_bounty_id + 2);
  let bounty_id = last_bounty_id + 1;
  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);
  //println!("{:#?}", bounty);

  let mut freelancers: Vec<Account> = vec![];
  for x in 0..2 {
    let freelancer_name = format!("freelancer{}", x + 43);
    let freelancer = e.add_account(freelancer_name.as_str()).await?;
    //println!("{}", freelancer.id());
    freelancers.push(freelancer.clone());
    Env::register_user(&e.test_token, freelancer.id()).await?;

    e.bounty_claim(
      &e.disputed_bounties,
      bounty_id,
      Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
      "Test claim".to_string(),
      Some(x),
      Some(&freelancers[x]),
      None
    ).await?;
  }

  for x in 0..2 {
    e.bounty_done(
      &e.disputed_bounties,
      bounty_id,
      "test description".to_string(),
      Some(&freelancers[x]),
      None
    ).await?;

    e.mark_as_paid(&e.disputed_bounties, bounty_id, Some(freelancers[x].id())).await?;

    if x == 1 {
      continue;
    }

    e.confirm_payment(&e.disputed_bounties, bounty_id, Some(&freelancers[x])).await?;
  }

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::ManyClaimed);
  assert!(bounty.postpaid.clone().unwrap().get_payment_timestamps().payment_at.is_none());
  assert!(bounty.postpaid.clone().unwrap().get_payment_timestamps().payment_confirmed_at.is_none());
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims.len(), 2);
  assert_eq!(bounty_claims[0].0.to_string(), freelancers[0].id().to_string());
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::Completed);
  assert!(bounty_claims[0].1.payment_timestamps.clone().unwrap().payment_at.is_some());
  assert!(bounty_claims[0].1.payment_timestamps.clone().unwrap().payment_confirmed_at.is_some());
  assert_eq!(bounty_claims[1].0.to_string(), freelancers[1].id().to_string());
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Completed);
  assert!(bounty_claims[1].1.payment_timestamps.clone().unwrap().payment_at.is_some());
  assert!(bounty_claims[1].1.payment_timestamps.clone().unwrap().payment_confirmed_at.is_none());

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::SeveralClaimsApproved,
    Some("Not all tasks are confirmed to be paid"),
  ).await?;

  e.confirm_payment(&e.disputed_bounties, bounty_id, Some(&freelancers[1])).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert!(bounty_claims[1].1.payment_timestamps.clone().unwrap().payment_confirmed_at.is_some());

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::SeveralClaimsApproved,
    None,
  ).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::Completed);
  //println!("{:#?}", bounty);
  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  //println!("{:#?}", bounty_claims);
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::Completed);
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Completed);

  e.withdraw(bounty_id, Some(&freelancers[0]), None).await?;
  e.withdraw(bounty_id, Some(&freelancers[1]), None).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::Approved);
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::Approved);

  println!("      Passed ✅ Test - different tasks flow");
  Ok(())
}

async fn test_withdraw_non_refunded_bonds(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  let contract_balance = e.disputed_bounties.view_account().await?.balance;
  let available_bonds = Env::get_non_refunded_bonds_amount(&e.disputed_bounties).await?.0;
  let bond_receiver_balance = e.bounties_contract_admin.view_account().await?.balance;

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None, None, None, None, None, None, None,
  ).await?;
  let bounty_id = last_bounty_id;

  let freelancer = e.add_account("freelancer45").await?;
  Env::register_user(&e.test_token, freelancer.id()).await?;
  let freelancer1_balance = freelancer.view_account().await?.balance;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    Some(&freelancer),
    None
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer.id()),
    ClaimStatus::InProgress,
    BountyStatus::Claimed,
  ).await?;

  Env::assert_eq_with_gas(
    contract_balance + BOND,
    e.disputed_bounties.view_account().await?.balance
  );
  Env::assert_eq_with_gas(
    freelancer1_balance - BOND,
    freelancer.view_account().await?.balance
  );

  e.bounty_give_up(&e.disputed_bounties, bounty_id, Some(&freelancer)).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer.id()),
    ClaimStatus::Canceled,
    BountyStatus::New,
  ).await?;

  // The bond is returned to the claimant 1
  Env::assert_eq_with_gas(
    contract_balance,
    e.disputed_bounties.view_account().await?.balance
  );
  Env::assert_eq_with_gas(
    freelancer1_balance,
    freelancer.view_account().await?.balance
  );

  let freelancer2 = e.add_account("freelancer46").await?;
  Env::register_user(&e.test_token, freelancer2.id()).await?;
  let freelancer2_balance = freelancer2.view_account().await?.balance;
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    Some(&freelancer2),
    None
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer2.id()),
    ClaimStatus::InProgress,
    BountyStatus::Claimed,
  ).await?;

  Env::assert_eq_with_gas(
    contract_balance + BOND,
    e.disputed_bounties.view_account().await?.balance
  );
  Env::assert_eq_with_gas(
    freelancer2_balance - BOND,
    freelancer2.view_account().await?.balance
  );

  // Forgiveness period: 10 min, wait for 500 blocks
  e.worker.fast_forward(500).await?;
  e.bounty_give_up(&e.disputed_bounties, bounty_id, Some(&freelancer2)).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer2.id()),
    ClaimStatus::Canceled,
    BountyStatus::New,
  ).await?;

  // The bond has not been returned to the claimant 2
  Env::assert_eq_with_gas(
    contract_balance + BOND,
    e.disputed_bounties.view_account().await?.balance
  );
  Env::assert_eq_with_gas(
    freelancer2_balance - BOND,
    freelancer2.view_account().await?.balance
  );

  let freelancer3 = e.add_account("freelancer47").await?;
  Env::register_user(&e.test_token, freelancer3.id()).await?;
  let freelancer3_balance = freelancer3.view_account().await?.balance;
  // Deadline 2 min
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 2)),
    "Test claim".to_string(),
    None,
    Some(&freelancer3),
    None
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer3.id()),
    ClaimStatus::InProgress,
    BountyStatus::Claimed,
  ).await?;

  Env::assert_eq_with_gas(
    contract_balance + 2 * BOND,
    e.disputed_bounties.view_account().await?.balance
  );
  Env::assert_eq_with_gas(
    freelancer3_balance - BOND,
    freelancer3.view_account().await?.balance
  );

  // Wait for 200 blocks
  e.worker.fast_forward(200).await?;
  Env::bounty_action_by_user(
    &e.disputed_bounties, bounty_id,
    &e.project_owner,
    &BountyAction::Finalize { receiver_id: None },
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer3.id()),
    ClaimStatus::Expired,
    BountyStatus::New,
  ).await?;

  // The bond has not been returned to the claimant 3
  Env::assert_eq_with_gas(
    contract_balance + 2 * BOND,
    e.disputed_bounties.view_account().await?.balance
  );
  Env::assert_eq_with_gas(
    freelancer3_balance - BOND,
    freelancer3.view_account().await?.balance
  );

  assert_eq!(
    available_bonds + 2 * BOND,
    Env::get_non_refunded_bonds_amount(&e.disputed_bounties).await?.0
  );

  // Withdrawal of unreturned bonds
  e.withdraw_non_refunded_bonds(&e.disputed_bounties).await?;

  assert_eq!(
    0,
    Env::get_non_refunded_bonds_amount(&e.disputed_bounties).await?.0
  );
  Env::assert_eq_with_gas(
    bond_receiver_balance + available_bonds + 2 * BOND,
    e.bounties_contract_admin.view_account().await?.balance
  );

  println!("      Passed ✅ Test - withdraw non refunded bonds");
  Ok(())
}

async fn test_flow_with_stretch_claim_deadline(e: &Env) -> anyhow::Result<()> {
  let result = e.add_bounty(
    &e.disputed_bounties,
    json!("WithoutDeadline"),
    json!("WithoutApproval"),
    None, None, None, None, None, None,
    Some(true),
  ).await?;
  Env::assert_exists_failed_receipts(
    result,
    "This bounty has no deadline, so the claimant must specify a deadline"
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  let _ = e.add_bounty(
    &e.disputed_bounties,
    // Max deadline: 10 min
    json!({ "MaxDeadline": json!({ "max_deadline": U64(10 * 60 * 1_000 * 1_000_000) }) }),
    json!("WithoutApproval"),
    None, None, None, None, None, None, None,
  ).await?;
  let bounty_id = last_bounty_id;

  let freelancer = e.add_account("freelancer48").await?;
  Env::register_user(&e.test_token, freelancer.id()).await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    None, // Without deadline
    "Test claim".to_string(),
    None,
    Some(&freelancer),
    Some("For this bounty, you need to specify a claim deadline"),
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  let _ = e.add_bounty(
    &e.disputed_bounties,
    // Max deadline: 10 min
    json!({ "MaxDeadline": json!({ "max_deadline": U64(10 * 60 * 1_000 * 1_000_000) }) }),
    json!("WithoutApproval"),
    None, None, None, None, None, None,
    Some(true),
  ).await?;
  let bounty_id = last_bounty_id;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    None, // Without deadline
    "Test claim".to_string(),
    None,
    Some(&freelancer),
    None
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer.id()),
    ClaimStatus::InProgress,
    BountyStatus::Claimed,
  ).await?;

  // MaxDeadline: 10 min, wait for 500 blocks
  e.worker.fast_forward(500).await?;

  e.bounty_done(
    &e.disputed_bounties,
    bounty_id.clone(),
    "test description".to_string(),
    Some(&freelancer),
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer.id()),
    ClaimStatus::Expired,
    BountyStatus::New
  ).await?;

  let freelancer2 = e.add_account("freelancer49").await?;
  Env::register_user(&e.test_token, freelancer2.id()).await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    None, // Without deadline
    "Test claim".to_string(),
    None,
    Some(&freelancer2),
    None
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer2.id()),
    ClaimStatus::InProgress,
    BountyStatus::Claimed,
  ).await?;

  e.bounty_done(
    &e.disputed_bounties,
    bounty_id.clone(),
    "test description".to_string(),
    Some(&freelancer2),
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer2.id()),
    ClaimStatus::Completed,
    BountyStatus::Claimed
  ).await?;

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimApproved {
      receiver_id: freelancer2.id().to_string().parse().unwrap()
    },
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer2.id()),
    ClaimStatus::Approved,
    BountyStatus::Completed
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None, None, None, None,
    Some(Multitasking::ContestOrHackathon {
      allowed_create_claim_to: None,
      successful_claims_for_result: None,
      start_conditions: None,
      runtime_env: None,
    }),
    None,
    Some(true),
  ).await?;
  let bounty_id = last_bounty_id;

  let freelancer = e.add_account("freelancer50").await?;
  Env::register_user(&e.test_token, freelancer.id()).await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    None, // Without deadline
    "Test claim".to_string(),
    None,
    Some(&freelancer),
    None
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer.id()),
    ClaimStatus::Competes,
    BountyStatus::ManyClaimed,
  ).await?;

  e.bounty_done(
    &e.disputed_bounties,
    bounty_id.clone(),
    "test description".to_string(),
    Some(&freelancer),
    None,
  ).await?;

  let freelancer2 = e.add_account("freelancer51").await?;
  Env::register_user(&e.test_token, freelancer2.id()).await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    // With deadline
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    Some(&freelancer2),
    None
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer2.id()),
    ClaimStatus::Competes,
    BountyStatus::ManyClaimed,
  ).await?;

  e.bounty_done(
    &e.disputed_bounties,
    bounty_id.clone(),
    "test description".to_string(),
    Some(&freelancer2),
    None,
  ).await?;

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimApproved {
      receiver_id: freelancer.id().to_string().parse().unwrap()
    },
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer.id()),
    ClaimStatus::Approved,
    BountyStatus::Completed,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer2.id()),
    ClaimStatus::Completed,
    BountyStatus::Completed,
  ).await?;

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::Finalize { receiver_id: Some(freelancer2.id().to_string().parse().unwrap()) },
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer2.id()),
    ClaimStatus::NotCompleted,
    BountyStatus::Completed,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None, None, None, None,
    Some(Multitasking::OneForAll {
      number_of_slots: 2,
      amount_per_slot: U128((BOUNTY_AMOUNT.0 + PLATFORM_FEE.0) / 2),
      min_slots_to_start: None,
      runtime_env: None,
    }),
    None,
    Some(true),
  ).await?;
  let bounty_id = last_bounty_id;

  let freelancer = e.add_account("freelancer52").await?;
  Env::register_user(&e.test_token, freelancer.id()).await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    None, // Without deadline
    "Test claim".to_string(),
    None,
    Some(&freelancer),
    None
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer.id()),
    ClaimStatus::InProgress,
    BountyStatus::ManyClaimed,
  ).await?;

  e.bounty_done(
    &e.disputed_bounties,
    bounty_id.clone(),
    "test description".to_string(),
    Some(&freelancer),
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer.id()),
    ClaimStatus::Completed,
    BountyStatus::ManyClaimed,
  ).await?;

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimApproved {
      receiver_id: freelancer.id().to_string().parse().unwrap()
    },
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer.id()),
    ClaimStatus::Approved,
    BountyStatus::AwaitingClaims,
  ).await?;

  let freelancer2 = e.add_account("freelancer53").await?;
  Env::register_user(&e.test_token, freelancer2.id()).await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    // With deadline
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    Some(&freelancer2),
    None
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer2.id()),
    ClaimStatus::InProgress,
    BountyStatus::ManyClaimed,
  ).await?;

  e.bounty_done(
    &e.disputed_bounties,
    bounty_id.clone(),
    "test description".to_string(),
    Some(&freelancer2),
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer2.id()),
    ClaimStatus::Completed,
    BountyStatus::ManyClaimed,
  ).await?;

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimApproved {
      receiver_id: freelancer2.id().to_string().parse().unwrap()
    },
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer2.id()),
    ClaimStatus::Approved,
    BountyStatus::Completed,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None, None, None, None,
    Some(Multitasking::DifferentTasks {
      subtasks: vec![
        Subtask {
          subtask_description: "Subtask 1".to_string(),
          subtask_percent: 20_000, // 20% slot 0
        },
        Subtask {
          subtask_description: "Subtask 2".to_string(),
          subtask_percent: 80_000, // 80% slot 1
        },
      ],
      runtime_env: None,
    }),
    None,
    Some(true),
  ).await?;
  let bounty_id = last_bounty_id;

  let freelancer = e.add_account("freelancer54").await?;
  Env::register_user(&e.test_token, freelancer.id()).await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    None, // Without deadline
    "Test claim".to_string(),
    Some(0),
    Some(&freelancer),
    None
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer.id()),
    ClaimStatus::ReadyToStart,
    BountyStatus::New,
  ).await?;

  let freelancer2 = e.add_account("freelancer55").await?;
  Env::register_user(&e.test_token, freelancer2.id()).await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    // With deadline
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    Some(1),
    Some(&freelancer2),
    None
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer.id()),
    ClaimStatus::InProgress,
    BountyStatus::ManyClaimed,
  ).await?;
  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer2.id()),
    ClaimStatus::InProgress,
    BountyStatus::ManyClaimed,
  ).await?;

  e.bounty_done(
    &e.disputed_bounties,
    bounty_id.clone(),
    "test description".to_string(),
    Some(&freelancer),
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer.id()),
    ClaimStatus::Completed,
    BountyStatus::ManyClaimed,
  ).await?;

  e.bounty_done(
    &e.disputed_bounties,
    bounty_id.clone(),
    "test description".to_string(),
    Some(&freelancer2),
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer2.id()),
    ClaimStatus::Completed,
    BountyStatus::ManyClaimed,
  ).await?;

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::SeveralClaimsApproved,
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer.id()),
    ClaimStatus::Completed,
    BountyStatus::Completed,
  ).await?;
  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer2.id()),
    ClaimStatus::Completed,
    BountyStatus::Completed,
  ).await?;

  e.withdraw(bounty_id, Some(&freelancer), None).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer.id()),
    ClaimStatus::Approved,
    BountyStatus::Completed,
  ).await?;
  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer2.id()),
    ClaimStatus::Completed,
    BountyStatus::Completed,
  ).await?;

  e.withdraw(bounty_id, Some(&freelancer2), None).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer2.id()),
    ClaimStatus::Approved,
    BountyStatus::Completed,
  ).await?;

  println!("      Passed ✅ Test - flow with stretch claim deadline");
  Ok(())
}

async fn test_owners_whitelist_flow(e: &Env) -> anyhow::Result<()> {
  e.start_using_owner_whitelist(&e.disputed_bounties).await?;
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;

  let result = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None, None, None, None, None, None, None,
  ).await?;
  Env::assert_exists_failed_receipts(
    result,
    "You are not allowed to create bounties"
  ).await?;

  e.add_to_some_whitelist(
    e.project_owner.id(),
    WhitelistType::OwnersWhitelist,
  ).await?;

  let _ = e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    json!("WithoutApproval"),
    None, None, None, None, None, None, None,
  ).await?;

  let bounty_id = last_bounty_id;

  let freelancer = e.add_account("freelancer56").await?;
  Env::register_user(&e.test_token, freelancer.id()).await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    Some(U64(1_000_000_000 * 60 * 60 * 24 * 2)),
    "Test claim".to_string(),
    None,
    Some(&freelancer),
    None
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    Some(&freelancer.id()),
    ClaimStatus::InProgress,
    BountyStatus::Claimed,
  ).await?;

  e.remove_from_some_whitelist(
    e.project_owner.id(),
    WhitelistType::OwnersWhitelist,
  ).await?;
  e.stop_using_owner_whitelist(&e.disputed_bounties).await?;

  println!("      Passed ✅ Test - owners whitelist flow");
  Ok(())
}
