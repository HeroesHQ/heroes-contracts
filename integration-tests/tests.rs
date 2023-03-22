use std::string::ToString;
use near_sdk::json_types::{U128, U64};
use near_sdk::serde::Deserialize;
use near_sdk::ONE_YOCTO;
use near_units::parse_near;
use serde_json::json;
use workspaces::{Account, AccountId, Contract, Worker};
use workspaces::network::Sandbox;
use workspaces::result::ExecutionFinalResult;
use bounties::{Bounty, BountyAction, BountyClaim, BountyStatus, BountyType, ClaimStatus, GAS_FOR_ADD_PROPOSAL, GAS_FOR_CLAIM_APPROVAL, GAS_FOR_CLAIMER_APPROVAL, ValidatorsDao};
use disputes::{Dispute, DisputeStatus, Proposal};
use reputation::{ClaimerMetrics, BountyOwnerMetrics};

#[derive(Deserialize)]
#[serde(crate = "near_sdk::serde")]
struct DaoPolicy {
  proposal_bond: U128,
}

pub const TEST_TOKEN_WASM: &str = "./integration-tests/res/test_token.wasm";
pub const BOUNTIES_WASM: &str = "./bounties/res/bounties.wasm";
pub const SPUTNIK_DAO2_WASM: &str = "./integration-tests/res/sputnikdao2.wasm";
pub const DISPUTES_WASM: &str = "./disputes/res/disputes.wasm";
pub const REPUTATION_WASM: &str = "./reputation/res/reputation.wasm";

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

  let test_token = worker.dev_deploy(&std::fs::read(TEST_TOKEN_WASM)?).await?;
  let mut res = test_token
    .call("new")
    .max_gas()
    .transact()
    .await?;
  assert_contract_call_result(res).await?;

  res = test_token
    .call("mint")
    .args_json((project_owner.id(), "100000000000000000000".to_string()))
    .max_gas()
    .transact()
    .await?;
  assert_contract_call_result(res).await?;
  register_user(&test_token, freelancer.id()).await?;

  let bounties = worker.dev_deploy(&std::fs::read(BOUNTIES_WASM)?).await?;
  res = bounties
    .call("new")
    .args_json(json!({
      "token_account_ids": vec![test_token.id()],
      "admin_whitelist": vec![bounties_contract_admin.id()],
    }))
    .max_gas()
    .transact()
    .await?;
  assert_contract_call_result(res).await?;
  register_user(&test_token, bounties.id()).await?;

  let validators_dao = worker.dev_deploy(&std::fs::read(SPUTNIK_DAO2_WASM)?).await?;
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
  assert_contract_call_result(res).await?;

  let (
    disputed_bounties,
    dispute_contract,
    reputation_contract,
    dispute_dao,
    arbitrator
  ) = init_bounty_contract_with_dispute_and_reputation(
    &worker,
    &root,
    &test_token,
    &bounties_contract_admin
  ).await?;

  // begin tests
  test_create_bounty(&test_token, &bounties, &validators_dao, &project_owner).await?;
  test_bounty_claim(&bounties, &freelancer).await?;
  test_bounty_done(&bounties, &freelancer).await?;
  test_bounty_approve_by_validators_dao(&test_token, &bounties, &dao_council_member, &validators_dao,
                                        &freelancer).await?;
  test_bounty_give_up(&test_token, &bounties, &project_owner, &freelancer).await?;
  test_bounty_reject_by_project_owner(&bounties, &project_owner, &freelancer).await?;
  test_bounty_approve_by_project_owner(&test_token, &bounties, &project_owner, &freelancer).await?;
  test_bounty_reject_by_validators_dao(&worker, &test_token, &disputed_bounties, &dao_council_member,
                                       &validators_dao, &project_owner, &freelancer,
                                       &reputation_contract).await?;
  test_bounty_claim_deadline_that_has_expired(&worker, &disputed_bounties, &project_owner,
                                              &freelancer, &reputation_contract).await?;
  test_bounty_open_and_reject_dispute(&worker, &disputed_bounties, &dao_council_member,
                                      &validators_dao, &project_owner, &freelancer,
                                      &dispute_contract, &dispute_dao, &arbitrator,
                                      &reputation_contract).await?;
  test_cancel_dispute_by_claimer(&disputed_bounties, &dao_council_member, &validators_dao,
                                 &project_owner, &freelancer, &dispute_contract,
                                 &reputation_contract).await?;
  test_cancel_dispute_by_project_owner(&test_token, &disputed_bounties, &dao_council_member,
                                       &validators_dao, &project_owner, &freelancer,
                                       &dispute_contract, &reputation_contract).await?;
  test_bounty_open_and_approve_dispute(&worker, &test_token, &disputed_bounties, &dao_council_member,
                                       &validators_dao, &project_owner, &freelancer,
                                       &dispute_contract, &dispute_dao, &arbitrator,
                                       &reputation_contract).await?;
  test_bounty_claim_approved(&test_token, &disputed_bounties, &dao_council_member, &validators_dao,
                             &project_owner, &freelancer, &reputation_contract).await?;
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
  assert_contract_call_result(res).await?;
  Ok(())
}

async fn init_bounty_contract_with_dispute_and_reputation(
  worker: &Worker<Sandbox>,
  root: &Account,
  test_token: &Contract,
  bounties_contract_admin: &Account,
) -> anyhow::Result<(Contract, Contract, Contract, Contract, Account)> {
  let dispute_dao_council_member = root
    .create_subaccount("arbitrator")
    .initial_balance(parse_near!("30 N"))
    .transact()
    .await?
    .into_result()?;

  let dispute_dao = worker.dev_deploy(&std::fs::read(SPUTNIK_DAO2_WASM)?).await?;
  let mut res = dispute_dao
    .call("new")
    .args_json(json!({
      "config": {
        "name": "genesis2",
        "purpose": "test",
        "metadata": "",
      },
      "policy": vec![dispute_dao_council_member.id()],
    }))
    .max_gas()
    .transact()
    .await?;
  assert_contract_call_result(res).await?;

  let bounties = worker.dev_deploy(&std::fs::read(BOUNTIES_WASM)?).await?;

  let mut disputes_config = disputes::Config::default();
  disputes_config.argument_period = U64(1_000_000_000 * 60 * 5); // 5 min
  disputes_config.decision_period = U64(1_000_000_000 * 60 * 5); // 5 min

  let dispute_contract = worker.dev_deploy(&std::fs::read(DISPUTES_WASM)?).await?;
  res = dispute_contract
    .call("new")
    .args_json(json!({
      "bounties_contract": bounties.id(),
      "dispute_dao": dispute_dao.id(),
      "admin_whitelist": vec![bounties_contract_admin.id()],
      "config": disputes_config,
    }))
    .max_gas()
    .transact()
    .await?;
  assert_contract_call_result(res).await?;

  let reputation_contract = worker.dev_deploy(&std::fs::read(REPUTATION_WASM)?).await?;
  res = reputation_contract
    .call("new")
    .args_json(json!({
      "bounties_contract": bounties.id(),
    }))
    .max_gas()
    .transact()
    .await?;
  assert_contract_call_result(res).await?;

  let mut bounties_config = bounties::Config::default();
  bounties_config.period_for_opening_dispute = U64(1_000_000_000 * 60 * 10); // 5 min

  res = bounties
    .call("new")
    .args_json(json!({
      "token_account_ids": vec![test_token.id()],
      "admin_whitelist": vec![bounties_contract_admin.id()],
      "config": bounties_config,
      "reputation_contract": reputation_contract.id(),
      "dispute_contract": dispute_contract.id(),
    }))
    .max_gas()
    .transact()
    .await?;
  assert_contract_call_result(res).await?;
  register_user(&test_token, bounties.id()).await?;

  Ok((bounties, dispute_contract, reputation_contract, dispute_dao, dispute_dao_council_member))
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
  assert_contract_call_result(res).await?;
  Ok(())
}

async fn bounty_claim(
  bounties: &Contract,
  bounty_id: u64,
  freelancer: &Account,
  deadline: U64,
) -> anyhow::Result<()> {
  let config: bounties::Config = bounties.call("get_config").view().await?.json()?;
  let res = freelancer
    .call(bounties.id(), "bounty_claim")
    .args_json((bounty_id, deadline))
    .max_gas()
    .deposit(config.bounty_claim_bond.0)
    .transact()
    .await?;
  assert_contract_call_result(res).await?;
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
  assert_contract_call_result(res).await?;
  Ok(())
}

async fn bounty_action_by_validators_dao(
  bounties: &Contract,
  bounty_id: u64,
  dao_council_member: &Account,
  validators_dao: &Contract,
  proposal_action: String,
) -> anyhow::Result<()> {
  let proposal_id = get_claim_proposal_id(bounties, bounty_id, 0).await?;
  let res = dao_council_member
    .call(validators_dao.id(), "act_proposal")
    .args_json((proposal_id.0, proposal_action, Option::<bool>::None))
    .max_gas()
    .transact()
    .await?;
  assert_contract_call_result(res).await?;
  Ok(())
}

async fn bounty_action_by_user(
  bounties: &Contract,
  bounty_id: u64,
  user: &Account,
  action: &BountyAction,
) -> anyhow::Result<()> {
  let res = user
    .call(bounties.id(), "bounty_action")
    .args_json((bounty_id, action))
    .max_gas()
    .deposit(ONE_YOCTO)
    .transact()
    .await?;
  assert_contract_call_result(res).await?;
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
  assert_contract_call_result(res).await?;
  Ok(())
}

async fn open_dispute(
  bounties: &Contract,
  bounty_id: u64,
  claimer: &Account,
) -> anyhow::Result<()> {
  let res = claimer
    .call(bounties.id(), "open_dispute")
    .args_json((bounty_id, "Dispute description"))
    .max_gas()
    .deposit(ONE_YOCTO)
    .transact()
    .await?;
  assert_contract_call_result(res).await?;
  Ok(())
}

async fn provide_arguments(
  dispute_contract: &Contract,
  dispute_id: u64,
  user: &Account,
  description: String,
) -> anyhow::Result<()> {
  let res = user
    .call(dispute_contract.id(), "provide_arguments")
    .args_json((dispute_id, description))
    .max_gas()
    .deposit(ONE_YOCTO)
    .transact()
    .await?;
  assert_contract_call_result(res).await?;
  Ok(())
}

async fn escalation(
  dispute_contract: &Contract,
  dispute_id: u64,
  user: &Account,
) -> anyhow::Result<()> {
  let res = user
    .call(dispute_contract.id(), "escalation")
    .args_json((dispute_id,))
    .max_gas()
    .deposit(ONE_YOCTO)
    .transact()
    .await?;
  assert_contract_call_result(res).await?;
  Ok(())
}

async fn dispute_dao_action(
  dispute_dao: &Contract,
  dao_council_member: &Account,
  proposal_id: u64,
  proposal_action: String,
) -> anyhow::Result<()> {
  let res = dao_council_member
    .call(dispute_dao.id(), "act_proposal")
    .args_json((proposal_id, proposal_action, Option::<bool>::None))
    .max_gas()
    .transact()
    .await?;
  assert_contract_call_result(res).await?;
  Ok(())
}

async fn finalize_dispute(
  dispute_contract: &Contract,
  dispute_id: u64,
  user: &Account,
) -> anyhow::Result<()> {
  let res = user
    .call(dispute_contract.id(), "finalize_dispute")
    .args_json((dispute_id,))
    .max_gas()
    .deposit(ONE_YOCTO)
    .transact()
    .await?;
  assert_contract_call_result(res).await?;
  Ok(())
}

async fn cancel_dispute(
  dispute_contract: &Contract,
  dispute_id: u64,
  user: &Account,
) -> anyhow::Result<()> {
  let res = user
    .call(dispute_contract.id(), "cancel_dispute")
    .args_json((dispute_id,))
    .max_gas()
    .deposit(ONE_YOCTO)
    .transact()
    .await?;
  assert_contract_call_result(res).await?;
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
  Ok(bounty_claim.bounty_payout_proposal_id.unwrap())
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

async fn get_bounties_by_ids(
  bounties: &Contract,
  bounty_ids: Vec<u64>,
) -> anyhow::Result<Vec<(u64, Bounty)>> {
  let result = bounties
    .call("get_bounties_by_ids")
    .args_json((bounty_ids,))
    .view()
    .await?
    .json()?;
  Ok(result)
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

async fn get_token_balance(
  test_token: &Contract,
  account_id: &AccountId,
) -> anyhow::Result<u128> {
  let token_balance: U128 = test_token
    .call("ft_balance_of")
    .args_json((account_id,))
    .view()
    .await?
    .json()?;
  Ok(token_balance.0)
}

async fn get_account_bounties(
  bounties: &Contract,
  project_owner: &Account,
) -> anyhow::Result<Vec<(u64, Bounty)>> {
  let bounty_indexes: Vec<(u64, Bounty)> = bounties
    .call("get_account_bounties")
    .args_json((project_owner.id(),))
    .view()
    .await?
    .json()?;
  Ok(bounty_indexes)
}

async fn get_dispute(
  dispute_contract: &Contract,
  dispute_id: u64,
) -> anyhow::Result<Dispute> {
  let dispute = dispute_contract
    .call("get_dispute")
    .args_json((dispute_id,))
    .view()
    .await?
    .json::<Dispute>()?;
  Ok(dispute)
}

async fn get_reputation_stats(
  reputation_contract: &Contract,
  project_owner: &Account,
  freelancer: &Account,
) -> anyhow::Result<(Option<ClaimerMetrics>, Option<BountyOwnerMetrics>)> {
  let bounty_owner_stats: (Option<ClaimerMetrics>, Option<BountyOwnerMetrics>) =
    reputation_contract
      .call("get_statistics")
      .args_json((project_owner.id(),))
      .view()
      .await?
      .json()?;
  let claimer_stats: (Option<ClaimerMetrics>, Option<BountyOwnerMetrics>) =
    reputation_contract
      .call("get_statistics")
      .args_json((freelancer.id(),))
      .view()
      .await?
      .json()?;
  Ok((claimer_stats.0, bounty_owner_stats.1))
}

async fn claimer_metrics_value_of(values: [u64; 7]) -> anyhow::Result<ClaimerMetrics> {
  let stats = ClaimerMetrics {
    number_of_claims: values[0],
    number_of_successful_claims: values[1],
    number_of_unsuccessful_claims: values[2],
    number_of_overdue_claims: values[3],
    number_of_canceled_claims: values[4],
    number_of_open_disputes: values[5],
    number_of_disputes_won: values[6],
  };
  Ok(stats)
}

async fn bounty_owner_metrics_value_of(values: [u64; 8]) -> anyhow::Result<BountyOwnerMetrics> {
  let stats = BountyOwnerMetrics {
    number_of_bounties: values[0],
    number_of_successful_bounties: values[1],
    number_of_canceled_bounties: values[2],
    number_of_claims: values[3],
    number_of_approved_claims: values[4],
    number_of_rejected_claims: values[5],
    number_of_open_disputes: values[6],
    number_of_disputes_won: values[7],
  };
  Ok(stats)
}

async fn assert_statuses(
  bounties: &Contract,
  bounty_id: u64,
  freelancer: &Account,
  claim_status: ClaimStatus,
  bounty_status: BountyStatus,
) -> anyhow::Result<(BountyClaim, Bounty)> {
  let bounty_claims = get_bounty_claims_by_id(bounties, bounty_id).await?;
  assert_eq!(bounty_claims.len(), 1);
  assert_eq!(bounty_claims[0].0.to_string(), freelancer.id().to_string());
  let bounty_claim = bounty_claims[0].clone().1;
  assert_eq!(bounty_claim.bounty_id, bounty_id);
  assert_eq!(bounty_claim.status, claim_status);
  let bounty = get_bounty(bounties, bounty_id).await?;
  assert_eq!(bounty.status, bounty_status);
  Ok((bounty_claim, bounty))
}

async fn assert_contract_call_result(result: ExecutionFinalResult) -> anyhow::Result<()> {
  if result.is_failure() {
    println!("{:#?}", result.clone().into_result().err());
  }
  assert!(result.is_success());
  Ok(())
}

async fn assert_reputation_stat_values_eq(
  reputation_contract: &Contract,
  project_owner: &Account,
  freelancer: &Account,
  claimer_stats: Option<[u64; 7]>,
  bounty_owner_stats: Option<[u64; 8]>,
) -> anyhow::Result<()> {
  let stats = get_reputation_stats(reputation_contract, project_owner, freelancer).await?;
  assert_eq!(stats.0.is_some(), claimer_stats.is_some());
  if claimer_stats.is_some() {
    assert_eq!(

      stats.0.unwrap(),
      claimer_metrics_value_of(claimer_stats.unwrap()).await?
    );
  }
  assert_eq!(stats.1.is_some(), bounty_owner_stats.is_some());
  if bounty_owner_stats.is_some() {
    assert_eq!(
      stats.1.unwrap(),
      bounty_owner_metrics_value_of(bounty_owner_stats.unwrap()).await?
    );
  }
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
  let owner_balance = get_token_balance(test_token, project_owner.id()).await?;
  assert_eq!(get_token_balance(test_token, bounties.id()).await?, 0);

  add_bounty(test_token, bounties, Some(validators_dao), project_owner).await?;

  let last_bounty_id = bounties.call("get_last_bounty_id").view().await?.json::<u64>()?;
  assert_eq!(last_bounty_id, 1);
  assert_eq!(
    get_token_balance(test_token, project_owner.id()).await?,
    owner_balance - BOUNTY_AMOUNT.0
  );
  assert_eq!(get_token_balance(test_token, bounties.id()).await?, BOUNTY_AMOUNT.0);

  let bounty_id = 0;
  let bounty_indexes = get_account_bounties(bounties, project_owner).await?;
  assert_eq!(bounty_indexes.len(), 1);
  assert_eq!(bounty_indexes[0].0, bounty_id);

  let bounty = get_bounty(bounties, bounty_id).await?;
  assert_eq!(bounty_indexes[0].1, bounty);
  let bounty_set = get_bounties_by_ids(bounties, vec![bounty_id]).await?;
  assert_eq!(bounty_set.len(), 1);
  assert_eq!(bounty_set[0].0, bounty_id);
  assert_eq!(bounty_set[0].1, bounty);

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
          gas_for_add_proposal: GAS_FOR_ADD_PROPOSAL.0.into(),
          gas_for_claim_approval: GAS_FOR_CLAIM_APPROVAL.0.into(),
          gas_for_claimer_approval: GAS_FOR_CLAIMER_APPROVAL.0.into(),
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

  let (bounty_claim, _) = assert_statuses(
    bounties,
    bounty_id.clone(),
    freelancer,
    ClaimStatus::New,
    BountyStatus::Claimed,
  ).await?;

  let freelancer_claims = get_bounty_claims(bounties, freelancer).await?;
  assert_eq!(freelancer_claims.len(), 1);
  let freelancer_claim = freelancer_claims[0].clone();
  assert_eq!(freelancer_claim, bounty_claim);
  assert_eq!(freelancer_claim.bounty_id, bounty_id);
  assert_eq!(freelancer_claim.deadline, U64(1_000_000_000 * 60 * 60 * 24 * 2));
  assert!(freelancer_claim.bounty_payout_proposal_id.is_none());

  println!("      Passed ✅ Bounty claim");
  Ok(())
}

async fn test_bounty_done(
  bounties: &Contract,
  freelancer: &Account,
) -> anyhow::Result<()> {
  let bounty_id = 0;
  bounty_done(bounties, bounty_id, freelancer, "test description".to_string()).await?;

  assert_statuses(
    bounties,
    bounty_id.clone(),
    freelancer,
    ClaimStatus::Completed,
    BountyStatus::Claimed,
  ).await?;

  println!("      Passed ✅ Bounty done");
  Ok(())
}

async fn test_bounty_approve_by_validators_dao(
  test_token: &Contract,
  bounties: &Contract,
  dao_council_member: &Account,
  validators_dao: &Contract,
  freelancer: &Account,
) -> anyhow::Result<()> {
  let token_balance = get_token_balance(test_token, bounties.id()).await?;
  assert_eq!(get_token_balance(test_token, freelancer.id()).await?, 0);

  let bounty_id = 0;
  bounty_action_by_validators_dao(
    bounties,
    bounty_id,
    dao_council_member,
    validators_dao,
    "VoteApprove".to_string()
  ).await?;

  assert_statuses(
    bounties,
    bounty_id.clone(),
    freelancer,
    ClaimStatus::Approved,
    BountyStatus::Completed,
  ).await?;

  assert_eq!(
    get_token_balance(test_token, bounties.id()).await?,
    token_balance - BOUNTY_AMOUNT.0
  );
  assert_eq!(get_token_balance(test_token, freelancer.id()).await?, BOUNTY_AMOUNT.0);

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

  assert_statuses(
    bounties,
    bounty_id.clone(),
    freelancer,
    ClaimStatus::Canceled,
    BountyStatus::New,
  ).await?;

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
  bounty_action_by_user(
    bounties,
    bounty_id,
    project_owner,
    &BountyAction::ClaimRejected {
      receiver_id: freelancer.id().as_str().parse().unwrap()
    }
  ).await?;

  assert_statuses(
    bounties,
    bounty_id.clone(),
    freelancer,
    ClaimStatus::NotCompleted,
    BountyStatus::New,
  ).await?;

  println!("      Passed ✅ Bounty reject by project owner");
  Ok(())
}

async fn test_bounty_approve_by_project_owner(
  test_token: &Contract,
  bounties: &Contract,
  project_owner: &Account,
  freelancer: &Account,
) -> anyhow::Result<()> {
  let token_balance = get_token_balance(test_token, bounties.id()).await?;
  let freelancer_balance = get_token_balance(test_token, freelancer.id()).await?;

  let bounty_id = 1;
  bounty_claim(bounties, bounty_id, freelancer, U64(1_000_000_000 * 60 * 60 * 24 * 2)).await?;
  bounty_done(bounties, bounty_id, freelancer, "test description".to_string()).await?;
  bounty_action_by_user(
    bounties,
    bounty_id,
    project_owner,
    &BountyAction::ClaimApproved {
      receiver_id: freelancer.id().as_str().parse().unwrap()
    }
  ).await?;

  assert_statuses(
    bounties,
    bounty_id.clone(),
    freelancer,
    ClaimStatus::Approved,
    BountyStatus::Completed,
  ).await?;

  assert_eq!(
    get_token_balance(test_token, bounties.id()).await?,
    token_balance - BOUNTY_AMOUNT.0
  );
  assert_eq!(
    get_token_balance(test_token, freelancer.id()).await?,
    freelancer_balance + BOUNTY_AMOUNT.0
  );

  println!("      Passed ✅ Bounty approve by project owner");
  Ok(())
}

async fn test_bounty_reject_by_validators_dao(
  worker: &Worker<Sandbox>,
  test_token: &Contract,
  bounties: &Contract,
  dao_council_member: &Account,
  validators_dao: &Contract,
  project_owner: &Account,
  freelancer: &Account,
  reputation_contract: &Contract,
) -> anyhow::Result<()> {
  assert_reputation_stat_values_eq(
    reputation_contract, project_owner, freelancer, None, None
  ).await?;
  // New bounty contract, numbering reset
  let last_bounty_id = bounties.call("get_last_bounty_id").view().await?.json::<u64>()?;
  assert_eq!(last_bounty_id, 0);

  add_bounty(test_token, bounties, Some(validators_dao), project_owner).await?;

  let last_bounty_id = bounties.call("get_last_bounty_id").view().await?.json::<u64>()?;
  assert_eq!(last_bounty_id, 1);
  assert_reputation_stat_values_eq(
    reputation_contract, project_owner, freelancer,
    None, Some([1, 0, 0, 0, 0, 0, 0, 0])
  ).await?;

  let bounty_id = 0;
  bounty_claim(bounties, bounty_id, freelancer, U64(1_000_000_000 * 60 * 60 * 24 * 2)).await?;
  assert_reputation_stat_values_eq(
    reputation_contract, project_owner, freelancer,
    Some([1, 0, 0, 0, 0, 0, 0]), Some([1, 0, 0, 1, 0, 0, 0, 0])
  ).await?;

  bounty_done(bounties, bounty_id, freelancer, "test description".to_string()).await?;

  bounty_action_by_validators_dao(
    bounties,
    bounty_id,
    dao_council_member,
    validators_dao,
    "VoteReject".to_string()
  ).await?;

  assert_statuses(
    bounties,
    bounty_id.clone(),
    freelancer,
    ClaimStatus::Completed,
    BountyStatus::Claimed,
  ).await?;

  bounty_action_by_user(bounties, bounty_id, freelancer, &BountyAction::Finalize).await?;

  assert_statuses(
    bounties,
    bounty_id.clone(),
    freelancer,
    ClaimStatus::Rejected,
    BountyStatus::Claimed,
  ).await?;

  // Period for opening a dispute: 5 min, wait for 500 blocks
  worker.fast_forward(500).await?;
  bounty_action_by_user(bounties, bounty_id, project_owner, &BountyAction::Finalize).await?;

  assert_statuses(
    bounties,
    bounty_id.clone(),
    freelancer,
    ClaimStatus::NotCompleted,
    BountyStatus::New,
  ).await?;
  assert_reputation_stat_values_eq(
    reputation_contract, project_owner, freelancer,
    Some([1, 0, 1, 0, 0, 0, 0]), Some([1, 0, 0, 1, 0, 1, 0, 0])
  ).await?;

  println!("      Passed ✅ Bounty reject by dao");
  Ok(())
}

async fn test_bounty_claim_deadline_that_has_expired(
  worker: &Worker<Sandbox>,
  bounties: &Contract,
  project_owner: &Account,
  freelancer: &Account,
  reputation_contract: &Contract,
) -> anyhow::Result<()> {
  let bounty_id = 0;

  bounty_claim(bounties, bounty_id, freelancer, U64(1_000_000_000 * 60 * 60 * 24 * 2)).await?;
  assert_reputation_stat_values_eq(
    reputation_contract, project_owner, freelancer,
    Some([2, 0, 1, 0, 0, 0, 0]), Some([1, 0, 0, 2, 0, 1, 0, 0])
  ).await?;

  bounty_give_up(bounties, bounty_id, freelancer).await?;
  assert_statuses(
    bounties,
    bounty_id.clone(),
    freelancer,
    ClaimStatus::Canceled,
    BountyStatus::New,
  ).await?;
  assert_reputation_stat_values_eq(
    reputation_contract, project_owner, freelancer,
    Some([2, 0, 1, 0, 1, 0, 0]), Some([1, 0, 0, 2, 0, 1, 0, 0])
  ).await?;

  // Deadline 2 min
  bounty_claim(bounties, bounty_id, freelancer, U64(1_000_000_000 * 60 * 2)).await?;
  assert_reputation_stat_values_eq(
    reputation_contract, project_owner, freelancer,
    Some([3, 0, 1, 0, 1, 0, 0]), Some([1, 0, 0, 3, 0, 1, 0, 0])
  ).await?;

  assert_statuses(
    bounties,
    bounty_id.clone(),
    freelancer,
    ClaimStatus::New,
    BountyStatus::Claimed,
  ).await?;

  // Wait for 200 blocks
  worker.fast_forward(200).await?;
  bounty_action_by_user(bounties, bounty_id, project_owner, &BountyAction::Finalize).await?;

  assert_statuses(
    bounties,
    bounty_id.clone(),
    freelancer,
    ClaimStatus::Expired,
    BountyStatus::New,
  ).await?;
  assert_reputation_stat_values_eq(
    reputation_contract, project_owner, freelancer,
    Some([3, 0, 1, 1, 1, 0, 0]), Some([1, 0, 0, 3, 0, 1, 0, 0])
  ).await?;

  println!("      Passed ✅ Bounty claim deadline that has expired");
  Ok(())
}

async fn test_bounty_open_and_reject_dispute(
  worker: &Worker<Sandbox>,
  bounties: &Contract,
  dao_council_member: &Account,
  validators_dao: &Contract,
  project_owner: &Account,
  freelancer: &Account,
  dispute_contract: &Contract,
  dispute_dao: &Contract,
  arbitrator: &Account,
  reputation_contract: &Contract,
) -> anyhow::Result<()> {
  let bounty_id = 0;

  bounty_claim(bounties, bounty_id, freelancer, U64(1_000_000_000 * 60 * 2)).await?;
  assert_reputation_stat_values_eq(
    reputation_contract, project_owner, freelancer,
    Some([4, 0, 1, 1, 1, 0, 0]), Some([1, 0, 0, 4, 0, 1, 0, 0])
  ).await?;
  bounty_done(bounties, bounty_id, freelancer, "test description".to_string()).await?;
  bounty_action_by_validators_dao(
    bounties,
    bounty_id,
    dao_council_member,
    validators_dao,
    "VoteReject".to_string()
  ).await?;
  bounty_action_by_user(bounties, bounty_id, freelancer, &BountyAction::Finalize).await?;

  open_dispute(bounties, bounty_id, freelancer).await?;

  assert_statuses(
    bounties,
    bounty_id.clone(),
    freelancer,
    ClaimStatus::Disputed,
    BountyStatus::Claimed,
  ).await?;

  let dispute_id: u64 = 0;
  let dispute = get_dispute(dispute_contract, dispute_id).await?;
  assert_eq!(dispute.bounty_id.0, bounty_id);
  assert_eq!(dispute.status, DisputeStatus::New);
  assert_eq!(dispute.claimer.to_string(), freelancer.id().to_string());
  assert_eq!(dispute.project_owner_delegate.to_string(), validators_dao.id().to_string());
  assert_eq!(dispute.description, "Dispute description".to_string());
  assert_eq!(dispute.proposal_id, None);
  assert_eq!(dispute.proposal_timestamp, None);

  provide_arguments(
    dispute_contract,
    dispute_id,
    validators_dao.as_account(),
    "Arguments of the project owner".to_string()
  ).await?;

  provide_arguments(
    dispute_contract,
    dispute_id,
    freelancer,
    "Claimer's arguments".to_string()
  ).await?;

  // Argument period: 5 min, wait for 500 blocks
  worker.fast_forward(500).await?;

  escalation(
    dispute_contract,
    dispute_id,
    freelancer,
  ).await?;

  let dispute = get_dispute(dispute_contract, dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::DecisionPending);
  let proposal_id = dispute.proposal_id.unwrap().0;
  assert_eq!(proposal_id, 0);

  let proposal = dispute_dao
    .call("get_proposal")
    .args_json((proposal_id,))
    .view()
    .await?
    .json::<Proposal>()?;
  println!("Proposal: {}", json!(proposal).to_string());

  dispute_dao_action(
    dispute_dao,
    arbitrator,
    proposal_id,
    "VoteReject".to_string(),
  ).await?;

  finalize_dispute(
    dispute_contract,
    dispute_id,
    project_owner,
  ).await?;

  let dispute = get_dispute(dispute_contract, dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::InFavorOfProjectOwner);

  assert_statuses(
    bounties,
    bounty_id.clone(),
    freelancer,
    ClaimStatus::NotCompleted,
    BountyStatus::New,
  ).await?;
  assert_reputation_stat_values_eq(
    reputation_contract, project_owner, freelancer,
    Some([4, 0, 2, 1, 1, 1, 0]), Some([1, 0, 0, 4, 0, 2, 1, 1])
  ).await?;

  println!("      Passed ✅ Bounty open and reject the dispute");
  Ok(())
}

async fn test_cancel_dispute_by_claimer(
  bounties: &Contract,
  dao_council_member: &Account,
  validators_dao: &Contract,
  project_owner: &Account,
  freelancer: &Account,
  dispute_contract: &Contract,
  reputation_contract: &Contract,
) -> anyhow::Result<()> {
  let bounty_id = 0;

  bounty_claim(bounties, bounty_id, freelancer, U64(1_000_000_000 * 60 * 2)).await?;
  assert_reputation_stat_values_eq(
    reputation_contract, project_owner, freelancer,
    Some([5, 0, 2, 1, 1, 1, 0]), Some([1, 0, 0, 5, 0, 2, 1, 1])
  ).await?;
  bounty_done(bounties, bounty_id, freelancer, "test description".to_string()).await?;
  bounty_action_by_validators_dao(
    bounties,
    bounty_id,
    dao_council_member,
    validators_dao,
    "VoteReject".to_string()
  ).await?;
  bounty_action_by_user(bounties, bounty_id, freelancer, &BountyAction::Finalize).await?;

  assert_statuses(
    bounties,
    bounty_id.clone(),
    freelancer,
    ClaimStatus::Rejected,
    BountyStatus::Claimed,
  ).await?;

  open_dispute(bounties, bounty_id, freelancer).await?;
  let dispute_id: u64 = 1;
  let dispute = get_dispute(dispute_contract, dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::New);

  cancel_dispute(dispute_contract, dispute_id, freelancer).await?;

  let dispute = get_dispute(dispute_contract, dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::CanceledByClaimer);

  assert_statuses(
    bounties,
    bounty_id.clone(),
    freelancer,
    ClaimStatus::NotCompleted,
    BountyStatus::New,
  ).await?;
  assert_reputation_stat_values_eq(
    reputation_contract, project_owner, freelancer,
    Some([5, 0, 3, 1, 1, 2, 0]), Some([1, 0, 0, 5, 0, 3, 2, 2])
  ).await?;

  println!("      Passed ✅ Dispute was canceled by the claimer");
  Ok(())
}

async fn test_cancel_dispute_by_project_owner(
  test_token: &Contract,
  bounties: &Contract,
  dao_council_member: &Account,
  validators_dao: &Contract,
  project_owner: &Account,
  freelancer: &Account,
  dispute_contract: &Contract,
  reputation_contract: &Contract,
) -> anyhow::Result<()> {
  let token_balance = get_token_balance(test_token, bounties.id()).await?;
  let freelancer_balance = get_token_balance(test_token, freelancer.id()).await?;

  let bounty_id = 0;
  bounty_claim(bounties, bounty_id, freelancer, U64(1_000_000_000 * 60 * 2)).await?;
  assert_reputation_stat_values_eq(
    reputation_contract, project_owner, freelancer,
    Some([6, 0, 3, 1, 1, 2, 0]), Some([1, 0, 0, 6, 0, 3, 2, 2])
  ).await?;
  bounty_done(bounties, bounty_id, freelancer, "test description".to_string()).await?;
  bounty_action_by_validators_dao(
    bounties,
    bounty_id,
    dao_council_member,
    validators_dao,
    "VoteReject".to_string()
  ).await?;
  bounty_action_by_user(bounties, bounty_id, freelancer, &BountyAction::Finalize).await?;

  assert_statuses(
    bounties,
    bounty_id.clone(),
    freelancer,
    ClaimStatus::Rejected,
    BountyStatus::Claimed,
  ).await?;

  open_dispute(bounties, bounty_id, freelancer).await?;
  let dispute_id: u64 = 2;
  let dispute = get_dispute(dispute_contract, dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::New);

  cancel_dispute(dispute_contract, dispute_id, validators_dao.as_account()).await?;

  let dispute = get_dispute(dispute_contract, dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::CanceledByProjectOwner);

  assert_statuses(
    bounties,
    bounty_id.clone(),
    freelancer,
    ClaimStatus::Approved,
    BountyStatus::Completed,
  ).await?;
  assert_reputation_stat_values_eq(
    reputation_contract, project_owner, freelancer,
    Some([6, 1, 3, 1, 1, 3, 1]), Some([1, 1, 0, 6, 0, 4, 3, 2])
  ).await?;

  assert_eq!(
    get_token_balance(test_token, bounties.id()).await?,
    token_balance - BOUNTY_AMOUNT.0
  );
  assert_eq!(
    get_token_balance(test_token, freelancer.id()).await?,
    freelancer_balance + BOUNTY_AMOUNT.0
  );

  println!("      Passed ✅ Dispute was canceled by the project owner");
  Ok(())
}

async fn test_bounty_open_and_approve_dispute(
  worker: &Worker<Sandbox>,
  test_token: &Contract,
  bounties: &Contract,
  dao_council_member: &Account,
  validators_dao: &Contract,
  project_owner: &Account,
  freelancer: &Account,
  dispute_contract: &Contract,
  dispute_dao: &Contract,
  arbitrator: &Account,
  reputation_contract: &Contract,
) -> anyhow::Result<()> {
  let last_bounty_id = bounties.call("get_last_bounty_id").view().await?.json::<u64>()?;
  assert_eq!(last_bounty_id, 1);

  add_bounty(test_token, bounties, Some(validators_dao), project_owner).await?;

  let last_bounty_id = bounties.call("get_last_bounty_id").view().await?.json::<u64>()?;
  assert_eq!(last_bounty_id, 2);
  assert_reputation_stat_values_eq(
    reputation_contract, project_owner, freelancer,
    Some([6, 1, 3, 1, 1, 3, 1]), Some([2, 1, 0, 6, 0, 4, 3, 2])
  ).await?;

  let token_balance = get_token_balance(test_token, bounties.id()).await?;
  let freelancer_balance = get_token_balance(test_token, freelancer.id()).await?;

  let bounty_id = 1;
  bounty_claim(bounties, bounty_id, freelancer, U64(1_000_000_000 * 60 * 60 * 24 * 2)).await?;
  assert_reputation_stat_values_eq(
    reputation_contract, project_owner, freelancer,
    Some([7, 1, 3, 1, 1, 3, 1]), Some([2, 1, 0, 7, 0, 4, 3, 2])
  ).await?;
  bounty_done(bounties, bounty_id, freelancer, "test description".to_string()).await?;
  bounty_action_by_validators_dao(
    bounties,
    bounty_id,
    dao_council_member,
    validators_dao,
    "VoteReject".to_string()
  ).await?;
  bounty_action_by_user(bounties, bounty_id, freelancer, &BountyAction::Finalize).await?;

  open_dispute(bounties, bounty_id, freelancer).await?;
  let dispute_id: u64 = 3;
  let dispute = get_dispute(dispute_contract, dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::New);

  // Argument period: 5 min, wait for 500 blocks
  worker.fast_forward(500).await?;

  escalation(
    dispute_contract,
    dispute_id,
    freelancer,
  ).await?;

  let dispute = get_dispute(dispute_contract, dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::DecisionPending);
  let proposal_id = dispute.proposal_id.unwrap().0;
  assert_eq!(proposal_id, 1);

  dispute_dao_action(
    dispute_dao,
    arbitrator,
    proposal_id,
    "VoteApprove".to_string(),
  ).await?;

  let dispute = get_dispute(dispute_contract, dispute_id).await?;
  assert_eq!(dispute.status, DisputeStatus::InFavorOfClaimer);

  assert_statuses(
    bounties,
    bounty_id.clone(),
    freelancer,
    ClaimStatus::Approved,
    BountyStatus::Completed,
  ).await?;
  assert_reputation_stat_values_eq(
    reputation_contract, project_owner, freelancer,
    Some([7, 2, 3, 1, 1, 4, 2]), Some([2, 2, 0, 7, 0, 5, 4, 2])
  ).await?;

  assert_eq!(
    get_token_balance(test_token, bounties.id()).await?,
    token_balance - BOUNTY_AMOUNT.0
  );
  assert_eq!(
    get_token_balance(test_token, freelancer.id()).await?,
    freelancer_balance + BOUNTY_AMOUNT.0
  );

  println!("      Passed ✅ Bounty open and approve the dispute");
  Ok(())
}

async fn test_bounty_claim_approved(
  test_token: &Contract,
  bounties: &Contract,
  dao_council_member: &Account,
  validators_dao: &Contract,
  project_owner: &Account,
  freelancer: &Account,
  reputation_contract: &Contract,
) -> anyhow::Result<()> {
  let last_bounty_id = bounties.call("get_last_bounty_id").view().await?.json::<u64>()?;
  assert_eq!(last_bounty_id, 2);

  add_bounty(test_token, bounties, Some(validators_dao), project_owner).await?;

  let last_bounty_id = bounties.call("get_last_bounty_id").view().await?.json::<u64>()?;
  assert_eq!(last_bounty_id, 3);
  assert_reputation_stat_values_eq(
    reputation_contract, project_owner, freelancer,
    Some([7, 2, 3, 1, 1, 4, 2]), Some([3, 2, 0, 7, 0, 5, 4, 2])
  ).await?;

  let token_balance = get_token_balance(test_token, bounties.id()).await?;
  let freelancer_balance = get_token_balance(test_token, freelancer.id()).await?;

  let bounty_id = 2;
  bounty_claim(bounties, bounty_id, freelancer, U64(1_000_000_000 * 60 * 60 * 24 * 2)).await?;
  assert_reputation_stat_values_eq(
    reputation_contract, project_owner, freelancer,
    Some([8, 2, 3, 1, 1, 4, 2]), Some([3, 2, 0, 8, 0, 5, 4, 2])
  ).await?;
  bounty_done(bounties, bounty_id, freelancer, "test description".to_string()).await?;
  bounty_action_by_validators_dao(
    bounties,
    bounty_id,
    dao_council_member,
    validators_dao,
    "VoteApprove".to_string()
  ).await?;

  assert_statuses(
    bounties,
    bounty_id.clone(),
    freelancer,
    ClaimStatus::Approved,
    BountyStatus::Completed,
  ).await?;
  assert_reputation_stat_values_eq(
    reputation_contract, project_owner, freelancer,
    Some([8, 3, 3, 1, 1, 4, 2]), Some([3, 3, 0, 8, 1, 5, 4, 2])
  ).await?;

  assert_eq!(
    get_token_balance(test_token, bounties.id()).await?,
    token_balance - BOUNTY_AMOUNT.0
  );
  assert_eq!(
    get_token_balance(test_token, freelancer.id()).await?,
    freelancer_balance + BOUNTY_AMOUNT.0
  );

  println!("      Passed ✅ Bounty claim approved");
  Ok(())
}
