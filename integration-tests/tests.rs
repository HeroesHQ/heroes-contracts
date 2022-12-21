use std::string::ToString;
use near_sdk::json_types::{U128, U64};
use near_sdk::serde::Deserialize;
use near_sdk::ONE_YOCTO;
use near_units::parse_near;
use serde_json::json;
use workspaces::{Account, AccountId, Contract};
use bounties::{Bounty, BountyAction, BountyClaim, BountyStatus, BountyType, ClaimStatus, Config,
               GAS_FOR_ADD_PROPOSAL, GAS_FOR_CLAIM_APPROVAL, ValidatorsDao};

#[derive(Deserialize)]
#[serde(crate = "near_sdk::serde")]
struct DaoPolicy {
  proposal_bond: U128,
}

pub const BOUNTY_AMOUNT: U128 = U128(10u128.pow(18));
pub const MAX_DEADLINE: U64 = U64(604_800_000_000_000);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let worker = workspaces::sandbox().await?;
  let root = worker.root_account().unwrap();

  let bounties_contract_admin = root
    .create_subaccount("admin")
    .initial_balance(parse_near!("30 N"))
    .transact()
    .await?
    .into_result()?;

  let project_owner = root
    .create_subaccount("owner")
    .initial_balance(parse_near!("30 N"))
    .transact()
    .await?
    .into_result()?;

  let dao_council_member = root
    .create_subaccount("expert")
    .initial_balance(parse_near!("30 N"))
    .transact()
    .await?
    .into_result()?;

  let freelancer = root
    .create_subaccount("freelancer")
    .initial_balance(parse_near!("30 N"))
    .transact()
    .await?
    .into_result()?;

  let test_token = worker.dev_deploy(include_bytes!("res/test_token.wasm")).await?;
  let mut res = test_token
    .call("new")
    .max_gas()
    .transact()
    .await?;
  assert!(res.is_success());

  res = test_token
    .call("mint")
    .args_json((project_owner.id(), "100000000000000000000".to_string()))
    .max_gas()
    .transact()
    .await?;
  assert!(res.is_success());
  register_user(&test_token, freelancer.id()).await?;

  let bounties = worker.dev_deploy(include_bytes!("../bounties/res/bounties.wasm")).await?;
  res = bounties
    .call("new")
    .args_json(json!({
      "token_account_ids": vec![test_token.id()],
      "admin_whitelist": vec![bounties_contract_admin.id()],
    }))
    .max_gas()
    .transact()
    .await?;
  assert!(res.is_success());
  register_user(&test_token, bounties.id()).await?;

  let validators_dao = worker.dev_deploy(include_bytes!("res/sputnikdao2.wasm")).await?;
  res = validators_dao
    .call("new")
    .args_json(json!({
      "config": {
        "name": "genesis2",
        "purpose": "test",
        "metadata": "",
      },
      "policy": vec![dao_council_member.id()],
    }))
    .max_gas()
    .transact()
    .await?;
  assert!(res.is_success());

  // begin tests
  test_create_bounty(&test_token, &bounties, &validators_dao, &project_owner).await?;
  test_bounty_claim(&bounties, &freelancer).await?;
  test_bounty_done(&bounties, &freelancer).await?;
  test_bounty_approve_by_validators_dao(&bounties, &dao_council_member, &validators_dao).await?;
  test_bounty_give_up(&test_token, &bounties, &project_owner, &freelancer).await?;
  test_bounty_reject_by_project_owner(&bounties, &project_owner, &freelancer).await?;
  test_bounty_approve_by_project_owner(&bounties, &project_owner, &freelancer).await?;
  Ok(())
}

async fn register_user(
  test_token: &Contract,
  user: &AccountId,
) -> anyhow::Result<()> {
  let res = test_token
    .call("storage_deposit")
    .args_json((user, Option::<bool>::None))
    .deposit(parse_near!("90 N"))
    .max_gas()
    .transact()
    .await?;
  assert!(res.is_success());
  Ok(())
}

async fn add_bounty(
  test_token: &Contract,
  bounties: &Contract,
  validators_dao: Option<&Contract>,
  project_owner: &Account,
) -> anyhow::Result<()> {
  let res = project_owner
    .call(test_token.id(), "ft_transfer_call")
    .args_json(json!({
      "receiver_id": bounties.id(),
      "amount": BOUNTY_AMOUNT,
      "msg": json!({
        "description": "Test bounty description",
        "bounty_type": "MarketingServices",
        "max_deadline": MAX_DEADLINE,
        "validators_dao": match validators_dao {
          Some(d) => json!({
            "account_id": d.id(),
            "add_proposal_bond": get_proposal_bond(d).await?,
          }),
          _ => json!(null),
        }
      })
        .to_string(),
    }))
    .max_gas()
    .deposit(ONE_YOCTO)
    .transact()
    .await?;
  assert!(res.is_success());
  Ok(())
}

async fn bounty_claim(
  bounties: &Contract,
  bounty_id: u64,
  freelancer: &Account,
  deadline: U64,
) -> anyhow::Result<()> {
  let config: Config = bounties.call("get_config").view().await?.json()?;
  let res = freelancer
    .call(bounties.id(), "bounty_claim")
    .args_json((bounty_id, deadline))
    .max_gas()
    .deposit(config.bounty_claim_bond.0)
    .transact()
    .await?;
  assert!(res.is_success());
  Ok(())
}

async fn bounty_done(
  bounties: &Contract,
  bounty_id: u64,
  claimer: &Account,
  description: String,
) -> anyhow::Result<()> {
  let res = claimer
    .call(bounties.id(), "bounty_done")
    .args_json((bounty_id, description))
    .max_gas()
    .deposit(ONE_YOCTO)
    .transact()
    .await?;
  assert!(res.is_success());
  Ok(())
}

async fn bounty_approve_by_validators_dao(
  bounties: &Contract,
  bounty_id: u64,
  dao_council_member: &Account,
  validators_dao: &Contract,
) -> anyhow::Result<()> {
  let proposal_id = get_claim_proposal_id(bounties, bounty_id, 0).await?;
  let res = dao_council_member
    .call(validators_dao.id(), "act_proposal")
    .args_json((proposal_id.0, "VoteApprove".to_string(), Option::<bool>::None))
    .max_gas()
    .transact()
    .await?;
  assert!(res.is_success());
  Ok(())
}

async fn bounty_action_by_project_owner(
  bounties: &Contract,
  bounty_id: u64,
  project_owner: &Account,
  action: &BountyAction,
) -> anyhow::Result<()> {
  let res = project_owner
    .call(bounties.id(), "bounty_action")
    .args_json((bounty_id, action))
    .max_gas()
    .deposit(ONE_YOCTO)
    .transact()
    .await?;
  assert!(res.is_success());
  Ok(())
}

async fn bounty_give_up(
  bounties: &Contract,
  bounty_id: u64,
  claimer: &Account,
) -> anyhow::Result<()> {
  let res = claimer
    .call(bounties.id(), "bounty_give_up")
    .args_json((bounty_id,))
    .max_gas()
    .deposit(ONE_YOCTO)
    .transact()
    .await?;
  assert!(res.is_success());
  Ok(())
}

async fn get_proposal_bond(
  validators_dao: &Contract,
) -> anyhow::Result<U128> {
  let policy: DaoPolicy = validators_dao
    .call("get_policy")
    .view()
    .await?
    .json()?;
  Ok(policy.proposal_bond)
}

async fn get_claim_proposal_id(
  bounties: &Contract,
  bounty_id: u64,
  claim_index: usize,
) -> anyhow::Result<U64> {
  let bounty_claims = get_bounty_claims_by_id(bounties, bounty_id).await?;
  assert!(bounty_claims.len() > claim_index);
  let bounty_claim = bounty_claims[claim_index].clone().1;
  Ok(bounty_claim.proposal_id.unwrap())
}

async fn get_bounty(
  bounties: &Contract,
  bounty_id: u64,
) -> anyhow::Result<Bounty> {
  let bounty = bounties
    .call("get_bounty")
    .args_json((bounty_id,))
    .view()
    .await?
    .json()?;
  Ok(bounty)
}

async fn get_bounty_claims(
  bounties: &Contract,
  freelancer: &Account,
) -> anyhow::Result<Vec<BountyClaim>> {
  let bounty_claims = bounties
    .call("get_bounty_claims")
    .args_json((freelancer.id(),))
    .view()
    .await?
    .json()?;
  Ok(bounty_claims)
}

async fn get_bounty_claims_by_id(
  bounties: &Contract,
  bounty_id: u64,
) -> anyhow::Result<Vec<(near_sdk::AccountId, BountyClaim)>> {
  let bounty_claims = bounties
    .call("get_bounty_claims_by_id")
    .args_json((bounty_id,))
    .view()
    .await?
    .json()?;
  Ok(bounty_claims)
}

async fn test_create_bounty(
  test_token: &Contract,
  bounties: &Contract,
  validators_dao: &Contract,
  project_owner: &Account,
) -> anyhow::Result<()> {
  let last_bounty_id = bounties.call("get_last_bounty_id").view().await?.json::<u64>()?;
  assert_eq!(last_bounty_id, 0);

  add_bounty(test_token, bounties, Some(validators_dao), project_owner).await?;

  let last_bounty_id = bounties.call("get_last_bounty_id").view().await?.json::<u64>()?;
  assert_eq!(last_bounty_id, 1);

  let bounty_id = 0;
  let bounty_indexes: Vec<(u64, Bounty)> = bounties
    .call("get_account_bounties")
    .args_json((project_owner.id(),))
    .view()
    .await?
    .json()?;
  assert_eq!(bounty_indexes.len(), 1);
  assert_eq!(bounty_indexes[0].0, bounty_id);

  let bounty = get_bounty(bounties, bounty_id).await?;
  assert_eq!(bounty_indexes[0].1, bounty);

  assert_eq!(
    bounty,
    Bounty {
      description: "Test bounty description".to_string(),
      token: test_token.id().to_string().parse().unwrap(),
      amount: BOUNTY_AMOUNT,
      bounty_type: BountyType::MarketingServices,
      max_deadline: MAX_DEADLINE,
      validators_dao: Some(
        ValidatorsDao {
          account_id: validators_dao.id().to_string().parse().unwrap(),
          add_proposal_bond: get_proposal_bond(validators_dao).await?,
          gas_for_add_proposal: U64(GAS_FOR_ADD_PROPOSAL.0),
          gas_for_claim_approval: U64(GAS_FOR_CLAIM_APPROVAL.0),
        }
      ),
      owner: project_owner.id().to_string().parse().unwrap(),
      status: BountyStatus::New,
    }
  );

  println!("      Passed ✅ Create a bounty");
  Ok(())
}

async fn test_bounty_claim(
  bounties: &Contract,
  freelancer: &Account,
) -> anyhow::Result<()> {
  let bounty_id = 0;
  bounty_claim(bounties, bounty_id, freelancer, U64(1_000_000_000 * 60 * 60 * 24 * 2)).await?;

  let freelancer_claims = get_bounty_claims(bounties, freelancer).await?;
  assert_eq!(freelancer_claims.len(), 1);
  let bounty_claim = freelancer_claims[0].clone();
  assert_eq!(bounty_claim.bounty_id, bounty_id);

  let bounty_claims = get_bounty_claims_by_id(bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 1);
  assert_eq!(freelancer.id().to_string(), bounty_claims[0].0.to_string());
  assert_eq!(bounty_claim, bounty_claims[0].1);
  assert_eq!(bounty_claim.deadline, U64(1_000_000_000 * 60 * 60 * 24 * 2));
  assert_eq!(bounty_claim.status, ClaimStatus::New);
  assert!(bounty_claim.proposal_id.is_none());

  let bounty = get_bounty(bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::Claimed);

  println!("      Passed ✅ Bounty claim");
  Ok(())
}

async fn test_bounty_done(
  bounties: &Contract,
  freelancer: &Account,
) -> anyhow::Result<()> {
  let bounty_id = 0;
  bounty_done(bounties, bounty_id, freelancer, "test description".to_string()).await?;

  let bounty_claims = get_bounty_claims(bounties, freelancer).await?;
  assert_eq!(bounty_claims[0].status, ClaimStatus::Completed);
  let bounty = get_bounty(bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::Claimed);

  println!("      Passed ✅ Bounty done");
  Ok(())
}

async fn test_bounty_approve_by_validators_dao(
  bounties: &Contract,
  dao_council_member: &Account,
  validators_dao: &Contract,
) -> anyhow::Result<()> {
  let bounty_id = 0;
  bounty_approve_by_validators_dao(bounties, bounty_id, dao_council_member, validators_dao).await?;

  let bounty_claims = get_bounty_claims_by_id(bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 1);
  let bounty_claim = bounty_claims[0].clone().1;
  assert_eq!(bounty_claim.bounty_id, bounty_id);
  assert_eq!(bounty_claim.status, ClaimStatus::Approved);
  let bounty = get_bounty(bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::Completed);

  println!("      Passed ✅ Bounty approve by dao");
  Ok(())
}

async fn test_bounty_give_up(
  test_token: &Contract,
  bounties: &Contract,
  project_owner: &Account,
  freelancer: &Account,
) -> anyhow::Result<()> {
  let last_bounty_id = bounties.call("get_last_bounty_id").view().await?.json::<u64>()?;
  assert_eq!(last_bounty_id, 1);

  add_bounty(test_token, bounties, None, project_owner).await?;

  let last_bounty_id = bounties.call("get_last_bounty_id").view().await?.json::<u64>()?;
  assert_eq!(last_bounty_id, 2);

  let bounty_id = 1;
  bounty_claim(bounties, bounty_id, freelancer, U64(1_000_000_000 * 60 * 60 * 24 * 2)).await?;
  bounty_give_up(bounties, bounty_id, freelancer).await?;

  let bounty_claims = get_bounty_claims_by_id(bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 1);
  assert_eq!(bounty_claims[0].0.to_string(), freelancer.id().to_string());
  let bounty_claim = bounty_claims[0].clone().1;
  assert_eq!(bounty_claim.bounty_id, bounty_id);
  assert_eq!(bounty_claim.status, ClaimStatus::Canceled);
  let bounty = get_bounty(bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);

  println!("      Passed ✅ Bounty give up");
  Ok(())
}

async fn test_bounty_reject_by_project_owner(
  bounties: &Contract,
  project_owner: &Account,
  freelancer: &Account,
) -> anyhow::Result<()> {
  let bounty_id = 1;
  bounty_claim(bounties, bounty_id, freelancer, U64(1_000_000_000 * 60 * 60 * 24 * 2)).await?;
  bounty_done(bounties, bounty_id, freelancer, "test description".to_string()).await?;
  bounty_action_by_project_owner(
    bounties,
    bounty_id,
    project_owner,
    &BountyAction::ClaimRejected {
      receiver_id: freelancer.id().as_str().parse().unwrap()
    }
  ).await?;

  let bounty_claims = get_bounty_claims_by_id(bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 1);
  assert_eq!(bounty_claims[0].0.to_string(), freelancer.id().to_string());
  let bounty_claim = bounty_claims[0].clone().1;
  assert_eq!(bounty_claim.bounty_id, bounty_id);
  assert_eq!(bounty_claim.status, ClaimStatus::NotCompleted);
  let bounty = get_bounty(bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::New);

  println!("      Passed ✅ Bounty reject by project owner");
  Ok(())
}

async fn test_bounty_approve_by_project_owner(
  bounties: &Contract,
  project_owner: &Account,
  freelancer: &Account,
) -> anyhow::Result<()> {
  let bounty_id = 1;
  bounty_claim(bounties, bounty_id, freelancer, U64(1_000_000_000 * 60 * 60 * 24 * 2)).await?;
  bounty_done(bounties, bounty_id, freelancer, "test description".to_string()).await?;
  bounty_action_by_project_owner(
    bounties,
    bounty_id,
    project_owner,
    &BountyAction::ClaimApproved {
      receiver_id: freelancer.id().as_str().parse().unwrap()
    }
  ).await?;

  let bounty_claims = get_bounty_claims_by_id(bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 1);
  assert_eq!(bounty_claims[0].0.to_string(), freelancer.id().to_string());
  let bounty_claim = bounty_claims[0].clone().1;
  assert_eq!(bounty_claim.bounty_id, bounty_id);
  assert_eq!(bounty_claim.status, ClaimStatus::Approved);
  let bounty = get_bounty(bounties, bounty_id).await?;
  assert_eq!(bounty.status, BountyStatus::Completed);

  println!("      Passed ✅ Bounty approve by project owner");
  Ok(())
}
