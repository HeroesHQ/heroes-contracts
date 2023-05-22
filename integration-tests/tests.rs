use std::string::ToString;
use near_sdk::json_types::{U128, U64};
use serde_json::json;
use bounties::{Bounty, BountyAction, BountyMetadata, BountyStatus, BountyUpdate, ClaimerApproval,
               ClaimStatus, ContactDetails, ContactType, Deadline, DefermentOfKYC, Experience,
               KycConfig, KycVerificationMethod, Reviewers, ReviewersParams, TokenDetails};
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
  test_claimers_whitelist_flow(&e).await?;
  test_kyc_whitelist_flow(&e).await?;
  test_use_commissions(&e).await?;
  test_creating_bounty_for_disabled_token(&e).await?;
  Ok(())
}

async fn test_create_bounty(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.bounties).await?;
  assert_eq!(last_bounty_id, 0);
  let owner_balance = e.get_token_balance(e.project_owner.id()).await?;
  assert_eq!(e.get_token_balance(e.bounties.id()).await?, 0);

  e.add_bounty(
    &e.bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    "WithoutApproval".to_string(),
    Some(e.reviewer_is_dao().await?),
    None,
    None,
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
      token: e.test_token.id().to_string().parse().unwrap(),
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
    }
  );

  println!("      Passed ✅ Test create a bounty");
  Ok(())
}

async fn test_bounty_claim(e: &Env) -> anyhow::Result<()> {
  let bounty_id = 0;
  let deadline = U64(1_000_000_000 * 60 * 60 * 24 * 2);
  e.bounty_claim(
    &e.bounties,
    bounty_id,
    deadline,
    "Test claim".to_string(),
    None, // by default freelancer
    None
  ).await?;

  let (bounty_claim, _) = e.assert_statuses(
    &e.bounties,
    bounty_id.clone(),
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

  e.add_bounty(
    &e.bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    "WithoutApproval".to_string(),
    None,
    None,
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.bounties).await?;
  assert_eq!(last_bounty_id, 2);

  let bounty_id = 1;
  e.bounty_claim(
    &e.bounties,
    bounty_id,
    U64(1_000_000_000 * 60 * 60 * 24 * 2),
    "Test claim".to_string(),
    None, // by default freelancer
    None
  ).await?;
  e.bounty_give_up(&e.bounties, bounty_id, None).await?;

  e.assert_statuses(
    &e.bounties,
    bounty_id.clone(),
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
    U64(1_000_000_000 * 60 * 60 * 24 * 2),
    "Test claim".to_string(),
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
  ).await?;

  e.assert_statuses(
    &e.bounties,
    bounty_id.clone(),
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
    U64(1_000_000_000 * 60 * 60 * 24 * 2),
    "Test claim".to_string(),
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
  ).await?;

  e.assert_statuses(
    &e.bounties,
    bounty_id.clone(),
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

  e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    "WithoutApproval".to_string(),
    Some(e.reviewer_is_dao().await?),
    None,
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 1);
  e.assert_reputation_stat_values_eq(None, Some([1, 0, 0, 0, 0, 0, 0, 0, 0]), None).await?;

  let bounty_id = 0;
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    U64(1_000_000_000 * 60 * 60 * 24 * 2),
    "Test claim".to_string(),
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
    ClaimStatus::Completed,
    BountyStatus::Claimed,
  ).await?;

  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.freelancer,
    &BountyAction::Finalize { receiver_id: None },
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    ClaimStatus::Rejected,
    BountyStatus::Claimed,
  ).await?;

  // Period for opening a dispute: 5 min, wait for 500 blocks
  e.worker.fast_forward(500).await?;
  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::Finalize { receiver_id: None },
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
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
    U64(1_000_000_000 * 60 * 60 * 24 * 2),
    "Test claim".to_string(),
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
    U64(1_000_000_000 * 60 * 2),
    "Test claim".to_string(),
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
    ClaimStatus::InProgress,
    BountyStatus::Claimed,
  ).await?;

  // Wait for 200 blocks
  e.worker.fast_forward(200).await?;
  Env::bounty_action_by_user(
    &e.disputed_bounties, bounty_id,
    &e.project_owner,
    &BountyAction::Finalize { receiver_id: None },
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
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
    U64(1_000_000_000 * 60 * 2),
    "Test claim".to_string(),
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
  ).await?;

  e.open_dispute(&e.disputed_bounties, bounty_id).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
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

  // Argument period: 5 min, wait for 500 blocks
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
    U64(1_000_000_000 * 60 * 2),
    "Test claim".to_string(),
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
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    ClaimStatus::Rejected,
    BountyStatus::Claimed,
  ).await?;

  e.open_dispute(&e.disputed_bounties, bounty_id).await?;
  let dispute_id: u64 = 1;
  let dispute = e.get_dispute(dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::New);

  e.cancel_dispute(dispute_id, &e.freelancer).await?;

  let dispute = e.get_dispute(dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::CanceledByClaimer);

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
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
    U64(1_000_000_000 * 60 * 2),
    "Test claim".to_string(),
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
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    ClaimStatus::Rejected,
    BountyStatus::Claimed,
  ).await?;

  e.open_dispute(&e.disputed_bounties, bounty_id).await?;
  let dispute_id: u64 = 2;
  let dispute = e.get_dispute(dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::New);

  e.cancel_dispute(dispute_id, e.validators_dao.as_account()).await?;

  let dispute = e.get_dispute(dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::CanceledByProjectOwner);

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
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

  e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    "WithoutApproval".to_string(),
    Some(e.reviewer_is_dao().await?),
    None,
    None,
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
    U64(1_000_000_000 * 60 * 60 * 24 * 2),
    "Test claim".to_string(),
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
  ).await?;

  e.open_dispute(&e.disputed_bounties, bounty_id).await?;
  let dispute_id: u64 = 3;
  let dispute = e.get_dispute(dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::New);

  // Argument period: 5 min, wait for 500 blocks
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

  e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    "WithoutApproval".to_string(),
    Some(e.reviewer_is_dao().await?),
    None,
    None,
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
    U64(1_000_000_000 * 60 * 60 * 24 * 2),
    "Test claim".to_string(),
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

  e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    "MultipleClaims".to_string(),
    None,
    None,
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([8, 8, 3, 3, 1, 1, 4, 2]),
    Some([4, 3, 0, 8, 8, 1, 5, 4, 2]),
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 4);

  let bounty_id = 3;
  let deadline = U64(1_000_000_000 * 60 * 60 * 24 * 2);
  // Four freelancers expressed their desire to do the task and receive the bounty
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    deadline,
    "Test claim 1".to_string(),
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

  e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    "MultipleClaims".to_string(),
    Some(e.reviewer_is_dao().await?),
    None,
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([9, 8, 3, 3, 1, 1, 4, 2]),
    Some([5, 3, 0, 12, 10, 1, 5, 4, 2]),
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 5);

  let bounty_id = 4;
  let deadline = U64(1_000_000_000 * 60 * 60 * 24 * 2);
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    deadline,
    "Test claim 1".to_string(),
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
  e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    "MultipleClaims".to_string(),
    Some(e.more_reviewers(&reviewer).await?),
    None,
    None,
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
    U64(1_000_000_000 * 60 * 60 * 24 * 2),
    "Test claim".to_string(),
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
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([11, 9, 4, 3, 1, 1, 4, 2]),
    Some([6, 4, 0, 15, 12, 2, 5, 4, 2]),
    None,
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    ClaimStatus::Approved,
    BountyStatus::Completed
  ).await?;

  println!("      Passed ✅ Test - using more reviewers");
  Ok(())
}

async fn test_bounty_cancel(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 6);

  e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    "MultipleClaims".to_string(),
    None,
    None,
    None,
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
    U64(1_000_000_000 * 60 * 60 * 24 * 2),
    "Test claim".to_string(),
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
    ClaimStatus::Canceled,
    BountyStatus::Canceled,
  ).await?;

  println!("      Passed ✅ Test - cancellation of the bounty by project owner");
  Ok(())
}

async fn test_bounty_update(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 7);

  e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    "WithoutApproval".to_string(),
    None,
    None,
    None,
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
  };

  e.bounty_update(&e.disputed_bounties, bounty_id, bounty_update).await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    U64(1_000_000_000 * 60 * 60 * 24 * 2),
    "Test claim".to_string(),
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
  };

  e.bounty_update(&e.disputed_bounties, bounty_id, bounty_update).await?;

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(
    bounty,
    Bounty {
      token: e.test_token.id().to_string().parse().unwrap(),
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
    }
  );

  println!("      Passed ✅ Test - updating the bounty by project owner");
  Ok(())
}

async fn test_claimers_whitelist_flow(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 8);

  e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    "ApprovalWithWhitelist".to_string(),
    None,
    None,
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([13, 9, 4, 3, 1, 1, 4, 2]),
    Some([9, 4, 1, 17, 12, 2, 5, 4, 2]),
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 9);

  let bounty_id = 8;
  let deadline = U64(1_000_000_000 * 60 * 60 * 24 * 2);
  let freelancer = e.add_account("freelancer6").await?;
  let freelancer2 = e.add_account("freelancer7").await?;

  e.add_to_claimers_whitelist(&freelancer2).await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    deadline,
    "Test claim".to_string(),
    Some(&freelancer),
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 0, 0, 0, 0, 0, 0, 0]),
    Some([9, 4, 1, 18, 12, 2, 5, 4, 2]),
    Some(freelancer.id()),
  ).await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    deadline,
    "Test claim 2".to_string(),
    Some(&freelancer2),
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 1, 0, 0, 0, 0, 0, 0]),
    Some([9, 4, 1, 19, 13, 2, 5, 4, 2]),
    Some(freelancer2.id()),
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 2);
  assert_eq!(bounty_claims[0].0.to_string(), freelancer.id().to_string());
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::New);
  assert_eq!(bounty_claims[1].0.to_string(), freelancer2.id().to_string());
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::InProgress);

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::Claimed);

  e.remove_from_claimers_whitelist(&freelancer2).await?;

  e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    "ApprovalWithWhitelist".to_string(),
    None,
    None,
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([13, 9, 4, 3, 1, 1, 4, 2]),
    Some([10, 4, 1, 19, 13, 2, 5, 4, 2]),
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 10);
  let bounty_id = 9;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    deadline,
    "Test claim".to_string(),
    Some(&freelancer),
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([2, 0, 0, 0, 0, 0, 0, 0]),
    Some([10, 4, 1, 20, 13, 2, 5, 4, 2]),
    Some(freelancer.id()),
  ).await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    deadline,
    "Test claim 2".to_string(),
    Some(&freelancer2),
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([2, 1, 0, 0, 0, 0, 0, 0]),
    Some([10, 4, 1, 21, 13, 2, 5, 4, 2]),
    Some(freelancer2.id()),
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 2);
  assert_eq!(bounty_claims[0].0.to_string(), freelancer.id().to_string());
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::New);
  assert_eq!(bounty_claims[1].0.to_string(), freelancer2.id().to_string());
  assert_eq!(bounty_claims[1].1.status, ClaimStatus::New);

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);

  println!("      Passed ✅ Test - claimers whitelist flow");
  Ok(())
}

async fn test_kyc_whitelist_flow(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 10);

  e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    "WithoutApproval".to_string(),
    None,
    Some(U128((BOUNTY_AMOUNT.0 + PLATFORM_FEE.0) * 2)),
    Some(KycConfig::default())
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([13, 9, 4, 3, 1, 1, 4, 2]),
    Some([11, 4, 1, 21, 13, 2, 5, 4, 2]),
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 11);

  let bounty_id = 10;
  let mut freelancer = e.add_account("freelancer8").await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    U64(1_000_000_000 * 60 * 60 * 24 * 2),
    "Test claim".to_string(),
    Some(&freelancer),
    Some("The claimer is not whitelisted")
  ).await?;
  e.assert_reputation_stat_values_eq(
    None,
    Some([11, 4, 1, 21, 13, 2, 5, 4, 2]),
    Some(freelancer.id()),
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 0);

  e.add_account_to_kyc_whitelist_by_service(&freelancer).await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    U64(1_000_000_000 * 60 * 60 * 24 * 2),
    "Test claim".to_string(),
    Some(&freelancer),
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 1, 0, 0, 0, 0, 0, 0]),
    Some([11, 4, 1, 22, 14, 2, 5, 4, 2]),
    Some(freelancer.id()),
  ).await?;

  let bounty_claims = Env::get_bounty_claims_by_id(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 1);
  assert_eq!(bounty_claims[0].0.to_string(), freelancer.id().to_string());
  assert_eq!(bounty_claims[0].1.status, ClaimStatus::InProgress);

  let bounty = get_bounty(&e.disputed_bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::Claimed);

  e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    "MultipleClaims".to_string(),
    None,
    None,
    Some(KycConfig::KycRequired {
      kyc_verification_method: KycVerificationMethod::DuringClaimApproval
    })
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([13, 9, 4, 3, 1, 1, 4, 2]),
    Some([12, 4, 1, 22, 14, 2, 5, 4, 2]),
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 12);

  let bounty_id = 11;
  freelancer = e.add_account("freelancer9").await?;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    U64(1_000_000_000 * 60 * 60 * 24 * 2),
    "Test claim".to_string(),
    Some(&freelancer),
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 0, 0, 0, 0, 0, 0, 0]),
    Some([12, 4, 1, 23, 14, 2, 5, 4, 2]),
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
    Some([12, 4, 1, 23, 14, 2, 5, 4, 2]),
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
    Some([12, 4, 1, 23, 15, 2, 5, 4, 2]),
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
  assert_eq!(last_bounty_id, 12);

  e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    "WithoutApproval".to_string(),
    Some(e.reviewer_is_dao().await?),
    None,
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([13, 9, 4, 3, 1, 1, 4, 2]),
    Some([13, 4, 1, 23, 15, 2, 5, 4, 2]),
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 13);
  let bounty_id = 12;

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
    U64(1_000_000_000 * 60 * 2),
    "Test claim".to_string(),
    None, // by default freelancer
    None
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([14, 10, 4, 3, 1, 1, 4, 2]),
    Some([13, 4, 1, 24, 16, 2, 5, 4, 2]),
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
    Some([13, 5, 1, 24, 16, 3, 5, 4, 2]),
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
  assert_eq!(last_bounty_id, 13);

  e.assert_reputation_stat_values_eq(
    Some([14, 10, 5, 3, 1, 1, 4, 2]),
    Some([13, 5, 1, 24, 16, 3, 5, 4, 2]),
    None,
  ).await?;
  e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    "WithoutApproval".to_string(),
    None,
    None,
    None,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([14, 10, 5, 3, 1, 1, 4, 2]),
    Some([13, 5, 1, 24, 16, 3, 5, 4, 2]),
    None,
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  // Bounty has not been created
  assert_eq!(last_bounty_id, 13);

  println!("      Passed ✅ Test - creating a bounty for a disabled token");
  Ok(())
}
