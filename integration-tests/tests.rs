use std::string::ToString;
use near_sdk::json_types::U64;
use serde_json::json;
use bounties::{Bounty, BountyAction, BountyMetadata, BountyStatus, ClaimerApproval, ClaimStatus,
               ContactDetails, ContactType, Deadline, Experience};
use disputes::DisputeStatus;

mod utils;
use crate::utils::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let e = Env::init().await?;
  // begin tests
  test_create_bounty(&e).await?;
  test_bounty_claim(&e).await?;
  test_bounty_done(&e).await?;
  test_bounty_approve_by_validators_dao(&e).await?;
  test_bounty_give_up(&e).await?;
  test_bounty_reject_by_project_owner(&e).await?;
  test_bounty_approve_by_project_owner(&e).await?;
  test_bounty_reject_by_validators_dao(&e).await?;
  test_bounty_claim_deadline_that_has_expired(&e).await?;
  test_bounty_open_and_reject_dispute(&e).await?;
  test_cancel_dispute_by_claimer(&e).await?;
  test_cancel_dispute_by_project_owner(&e).await?;
  test_bounty_open_and_approve_dispute(&e).await?;
  test_bounty_claim_approved(&e).await?;
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
    }
  );

  println!("      Passed ✅ Create a bounty");
  Ok(())
}

async fn test_bounty_claim(e: &Env) -> anyhow::Result<()> {
  let bounty_id = 0;
  let deadline = U64(1_000_000_000 * 60 * 60 * 24 * 2);
  e.bounty_claim(&e.bounties, bounty_id, deadline, "Test claim".to_string()).await?;

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

  println!("      Passed ✅ Bounty claim");
  Ok(())
}

async fn test_bounty_done(e: &Env) -> anyhow::Result<()> {
  let bounty_id = 0;
  e.bounty_done(&e.bounties, bounty_id, "test description".to_string()).await?;

  e.assert_statuses(
    &e.bounties,
    bounty_id.clone(),
    ClaimStatus::Completed,
    BountyStatus::Claimed
  ).await?;

  println!("      Passed ✅ Bounty done");
  Ok(())
}

async fn test_bounty_approve_by_validators_dao(e: &Env) -> anyhow::Result<()> {
  let token_balance = e.get_token_balance(e.bounties.id()).await?;
  assert_eq!(e.get_token_balance(e.freelancer.id()).await?, 0);

  let bounty_id = 0;
  e.bounty_action_by_validators_dao(&e.bounties, bounty_id, "VoteApprove".to_string()).await?;

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

  println!("      Passed ✅ Bounty approve by dao");
  Ok(())
}

async fn test_bounty_give_up(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.bounties).await?;
  assert_eq!(last_bounty_id, 1);

  e.add_bounty(
    &e.bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    "WithoutApproval".to_string(),
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
  ).await?;
  e.bounty_give_up(&e.bounties, bounty_id).await?;

  e.assert_statuses(
    &e.bounties,
    bounty_id.clone(),
    ClaimStatus::Canceled,
    BountyStatus::New,
  ).await?;

  println!("      Passed ✅ Bounty give up");
  Ok(())
}

async fn test_bounty_reject_by_project_owner(e: &Env) -> anyhow::Result<()> {
  let bounty_id = 1;
  e.bounty_claim(
    &e.bounties,
    bounty_id,
    U64(1_000_000_000 * 60 * 60 * 24 * 2),
    "Test claim".to_string(),
  ).await?;
  e.bounty_done(&e.bounties, bounty_id, "test description".to_string()).await?;

  Env::bounty_action_by_user(
    &e.bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimRejected {
      receiver_id: e.freelancer.id().as_str().parse().unwrap()
    }
  ).await?;

  e.assert_statuses(
    &e.bounties,
    bounty_id.clone(),
    ClaimStatus::NotCompleted,
    BountyStatus::New,
  ).await?;

  println!("      Passed ✅ Bounty reject by project owner");
  Ok(())
}

async fn test_bounty_approve_by_project_owner(e: &Env) -> anyhow::Result<()> {
  let token_balance = e.get_token_balance(e.bounties.id()).await?;
  let freelancer_balance = e.get_token_balance(e.freelancer.id()).await?;

  let bounty_id = 1;
  e.bounty_claim(
    &e.bounties,
    bounty_id,
    U64(1_000_000_000 * 60 * 60 * 24 * 2),
    "Test claim".to_string(),
  ).await?;
  e.bounty_done(&e.bounties, bounty_id, "test description".to_string()).await?;

  Env::bounty_action_by_user(
    &e.bounties,
    bounty_id,
    &e.project_owner,
    &BountyAction::ClaimApproved {
      receiver_id: e.freelancer.id().as_str().parse().unwrap()
    }
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

  println!("      Passed ✅ Bounty approve by project owner");
  Ok(())
}

async fn test_bounty_reject_by_validators_dao(e: &Env) -> anyhow::Result<()> {
  e.assert_reputation_stat_values_eq(None, None).await?;
  // New bounty contract, numbering reset
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 0);

  e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    "WithoutApproval".to_string(),
    Some(e.reviewer_is_dao().await?),
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 1);
  e.assert_reputation_stat_values_eq(None, Some([1, 0, 0, 0, 0, 0, 0, 0])).await?;

  let bounty_id = 0;
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    U64(1_000_000_000 * 60 * 60 * 24 * 2),
    "Test claim".to_string(),
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 0, 0, 0, 0, 0, 0]),
    Some([1, 0, 0, 1, 0, 0, 0, 0])
  ).await?;

  e.bounty_done(&e.disputed_bounties, bounty_id, "test description".to_string()).await?;
  e.bounty_action_by_validators_dao(
    &e.disputed_bounties,
    bounty_id,
    "VoteReject".to_string()
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
    &BountyAction::Finalize
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
    &BountyAction::Finalize
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    ClaimStatus::NotCompleted,
    BountyStatus::New,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([1, 0, 1, 0, 0, 0, 0]),
    Some([1, 0, 0, 1, 0, 1, 0, 0])
  ).await?;

  println!("      Passed ✅ Bounty reject by dao");
  Ok(())
}

async fn test_bounty_claim_deadline_that_has_expired(e: &Env) -> anyhow::Result<()> {
  let bounty_id = 0;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    U64(1_000_000_000 * 60 * 60 * 24 * 2),
    "Test claim".to_string(),
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([2, 0, 1, 0, 0, 0, 0]),
    Some([1, 0, 0, 2, 0, 1, 0, 0])
  ).await?;

  e.bounty_give_up(&e.disputed_bounties, bounty_id).await?;
  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    ClaimStatus::Canceled,
    BountyStatus::New,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([2, 0, 1, 0, 1, 0, 0]),
    Some([1, 0, 0, 2, 0, 1, 0, 0])
  ).await?;

  // Deadline 2 min
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    U64(1_000_000_000 * 60 * 2),
    "Test claim".to_string(),
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([3, 0, 1, 0, 1, 0, 0]),
    Some([1, 0, 0, 3, 0, 1, 0, 0])
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
    &BountyAction::Finalize
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    ClaimStatus::Expired,
    BountyStatus::New,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([3, 0, 1, 1, 1, 0, 0]),
    Some([1, 0, 0, 3, 0, 1, 0, 0])
  ).await?;

  println!("      Passed ✅ Bounty claim deadline that has expired");
  Ok(())
}

async fn test_bounty_open_and_reject_dispute(e: &Env) -> anyhow::Result<()> {
  let bounty_id = 0;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    U64(1_000_000_000 * 60 * 2),
    "Test claim".to_string(),
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([4, 0, 1, 1, 1, 0, 0]),
    Some([1, 0, 0, 4, 0, 1, 0, 0])
  ).await?;
  e.bounty_done(&e.disputed_bounties, bounty_id, "test description".to_string()).await?;
  e.bounty_action_by_validators_dao(
    &e.disputed_bounties,
    bounty_id,
    "VoteReject".to_string()
  ).await?;
  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.freelancer,
    &BountyAction::Finalize
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
    Some([4, 0, 2, 1, 1, 1, 0]),
    Some([1, 0, 0, 4, 0, 2, 1, 1])
  ).await?;

  println!("      Passed ✅ Bounty open and reject the dispute");
  Ok(())
}

async fn test_cancel_dispute_by_claimer(e: &Env) -> anyhow::Result<()> {
  let bounty_id = 0;

  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    U64(1_000_000_000 * 60 * 2),
    "Test claim".to_string(),
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([5, 0, 2, 1, 1, 1, 0]),
    Some([1, 0, 0, 5, 0, 2, 1, 1])
  ).await?;
  e.bounty_done(&e.disputed_bounties, bounty_id, "test description".to_string()).await?;
  e.bounty_action_by_validators_dao(
    &e.disputed_bounties,
    bounty_id,
    "VoteReject".to_string()
  ).await?;
  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.freelancer,
    &BountyAction::Finalize
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
    Some([5, 0, 3, 1, 1, 2, 0]),
    Some([1, 0, 0, 5, 0, 3, 2, 2])
  ).await?;

  println!("      Passed ✅ Dispute was canceled by the claimer");
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
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([6, 0, 3, 1, 1, 2, 0]),
    Some([1, 0, 0, 6, 0, 3, 2, 2])
  ).await?;
  e.bounty_done(&e.disputed_bounties, bounty_id, "test description".to_string()).await?;
  e.bounty_action_by_validators_dao(
    &e.disputed_bounties,
    bounty_id,
    "VoteReject".to_string()
  ).await?;
  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.freelancer,
    &BountyAction::Finalize
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
    Some([6, 1, 3, 1, 1, 3, 1]),
    Some([1, 1, 0, 6, 0, 4, 3, 2])
  ).await?;

  assert_eq!(
    e.get_token_balance(e.disputed_bounties.id()).await?,
    token_balance - BOUNTY_AMOUNT.0
  );
  assert_eq!(
    e.get_token_balance(e.freelancer.id()).await?,
    freelancer_balance + BOUNTY_AMOUNT.0
  );

  println!("      Passed ✅ Dispute was canceled by the project owner");
  Ok(())
}

async fn test_bounty_open_and_approve_dispute(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 1);

  e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    "WithoutApproval".to_string(),
    Some(e.reviewer_is_dao().await?),
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 2);
  e.assert_reputation_stat_values_eq(
    Some([6, 1, 3, 1, 1, 3, 1]),
    Some([2, 1, 0, 6, 0, 4, 3, 2])
  ).await?;

  let token_balance = e.get_token_balance(e.disputed_bounties.id()).await?;
  let freelancer_balance = e.get_token_balance(e.freelancer.id()).await?;

  let bounty_id = 1;
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    U64(1_000_000_000 * 60 * 60 * 24 * 2),
    "Test claim".to_string(),
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([7, 1, 3, 1, 1, 3, 1]),
    Some([2, 1, 0, 7, 0, 4, 3, 2])
  ).await?;
  e.bounty_done(&e.disputed_bounties, bounty_id, "test description".to_string()).await?;
  e.bounty_action_by_validators_dao(
    &e.disputed_bounties,
    bounty_id,
    "VoteReject".to_string()
  ).await?;
  Env::bounty_action_by_user(
    &e.disputed_bounties,
    bounty_id,
    &e.freelancer,
    &BountyAction::Finalize
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
    Some([7, 2, 3, 1, 1, 4, 2]),
    Some([2, 2, 0, 7, 0, 5, 4, 2])
  ).await?;

  assert_eq!(
    e.get_token_balance(e.disputed_bounties.id()).await?,
    token_balance - BOUNTY_AMOUNT.0
  );
  assert_eq!(
    e.get_token_balance(e.freelancer.id()).await?,
    freelancer_balance + BOUNTY_AMOUNT.0
  );

  println!("      Passed ✅ Bounty open and approve the dispute");
  Ok(())
}

async fn test_bounty_claim_approved(e: &Env) -> anyhow::Result<()> {
  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 2);

  e.add_bounty(
    &e.disputed_bounties,
    json!({ "MaxDeadline": json!({ "max_deadline": MAX_DEADLINE }) }),
    "WithoutApproval".to_string(),
    Some(e.reviewer_is_dao().await?),
  ).await?;

  let last_bounty_id = get_last_bounty_id(&e.disputed_bounties).await?;
  assert_eq!(last_bounty_id, 3);
  e.assert_reputation_stat_values_eq(
    Some([7, 2, 3, 1, 1, 4, 2]),
    Some([3, 2, 0, 7, 0, 5, 4, 2])
  ).await?;

  let token_balance = e.get_token_balance(e.disputed_bounties.id()).await?;
  let freelancer_balance = e.get_token_balance(e.freelancer.id()).await?;

  let bounty_id = 2;
  e.bounty_claim(
    &e.disputed_bounties,
    bounty_id,
    U64(1_000_000_000 * 60 * 60 * 24 * 2),
    "Test claim".to_string(),
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([8, 2, 3, 1, 1, 4, 2]),
    Some([3, 2, 0, 8, 0, 5, 4, 2])
  ).await?;
  e.bounty_done(&e.disputed_bounties, bounty_id, "test description".to_string()).await?;
  e.bounty_action_by_validators_dao(
    &e.disputed_bounties,
    bounty_id,
    "VoteApprove".to_string()
  ).await?;

  e.assert_statuses(
    &e.disputed_bounties,
    bounty_id.clone(),
    ClaimStatus::Approved,
    BountyStatus::Completed,
  ).await?;
  e.assert_reputation_stat_values_eq(
    Some([8, 3, 3, 1, 1, 4, 2]),
    Some([3, 3, 0, 8, 1, 5, 4, 2])
  ).await?;

  assert_eq!(
    e.get_token_balance(e.disputed_bounties.id()).await?,
    token_balance - BOUNTY_AMOUNT.0
  );
  assert_eq!(
    e.get_token_balance(e.freelancer.id()).await?,
    freelancer_balance + BOUNTY_AMOUNT.0
  );

  println!("      Passed ✅ Bounty claim approved");
  Ok(())
}
