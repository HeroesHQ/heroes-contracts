use std::string::ToString;
use near_sdk::json_types::{U128, U64};
use near_sdk::ONE_YOCTO;
use near_units::parse_near;
use serde_json::json;
use workspaces::{Account, Contract};
use bounties::{Bounty, BountyClaim, BountyStatus, BountyType, ClaimStatus, Config, GAS_FOR_ADD_PROPOSAL, GAS_FOR_CLAIM_APPROVAL, ValidatorsDao};

pub const BOUNTY_AMOUNT: U128 = U128(10u128.pow(18));
pub const ADD_PROPOSAL_BOND: U128 = U128(10u128.pow(24));
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

  res = test_token
    .call("storage_deposit")
    .args_json((bounties.id(), Option::<bool>::None))
    .deposit(parse_near!("90 N"))
    .max_gas()
    .transact()
    .await?;
  assert!(res.is_success());

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
  test_bounty_claim(&test_token, &bounties, &validators_dao, &project_owner, &freelancer).await?;
  Ok(())
}

async fn add_bounty(
  test_token: &Contract,
  bounties: &Contract,
  validators_dao: &Contract,
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
        "validators_dao": {
          "account_id": validators_dao.id(),
          "add_proposal_bond": ADD_PROPOSAL_BOND
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

async fn test_create_bounty(
  test_token: &Contract,
  bounties: &Contract,
  validators_dao: &Contract,
  project_owner: &Account,
) -> anyhow::Result<()> {
  let last_bounty_id = bounties.call("get_last_bounty_id").view().await?.json::<u64>()?;
  assert_eq!(last_bounty_id, 0);

  add_bounty(test_token, bounties, validators_dao, project_owner).await?;

  let last_bounty_id = bounties.call("get_last_bounty_id").view().await?.json::<u64>()?;
  assert_eq!(last_bounty_id, 1);

  let bounty_indexes: Vec<(u64, Bounty)> = bounties
    .call("get_account_bounties")
    .args_json((project_owner.id(),))
    .view()
    .await?
    .json()?;
  assert_eq!(bounty_indexes.len(), 1);

  let bounty: Bounty = bounties
    .call("get_bounty")
    .args_json((bounty_indexes[0].0,))
    .view()
    .await?
    .json()?;
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
          add_proposal_bond: ADD_PROPOSAL_BOND,
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
  test_token: &Contract,
  bounties: &Contract,
  validators_dao: &Contract,
  project_owner: &Account,
  freelancer: &Account,
) -> anyhow::Result<()> {
  add_bounty(test_token, bounties, validators_dao, project_owner).await?;
  bounty_claim(&bounties, 0, freelancer, U64(1_000_000_000 * 60 * 60 * 24 * 2)).await?;

  let freelancer_claims: Vec<BountyClaim> = bounties
    .call("get_bounty_claims")
    .args_json((freelancer.id(),))
    .view()
    .await?
    .json()?;

  assert_eq!(freelancer_claims.len(), 1);
  let bounty_claim = freelancer_claims[0].clone();

  let bounty_claims: Vec<(near_sdk::AccountId, BountyClaim)> = bounties
    .call("get_bounty_claims_by_id")
    .args_json((bounty_claim.bounty_id,))
    .view()
    .await?
    .json()?;

  assert_eq!(bounty_claims.len(), 1);
  assert_eq!(freelancer.id().to_string(), bounty_claims[0].0.to_string());
  assert_eq!(bounty_claim, bounty_claims[0].1);
  assert_eq!(bounty_claim.deadline, U64(1_000_000_000 * 60 * 60 * 24 * 2));
  assert_eq!(bounty_claim.status, ClaimStatus::New);
  assert!(bounty_claim.proposal_id.is_none());

  let bounty: Bounty = bounties
    .call("get_bounty")
    .args_json((bounty_claim.bounty_id,))
    .view()
    .await?
    .json()?;

  assert_eq!(bounty.status, BountyStatus::Claimed);

  println!("      Passed ✅ Bounty claim");
  Ok(())
}
