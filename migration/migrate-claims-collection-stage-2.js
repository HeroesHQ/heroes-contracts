require('dotenv').config();
const Big = require("big.js");
const { MongoClient } = require('mongodb');

const { nearInit } = require("./near");

const client = new MongoClient(process.env.MONGO_URL || "mongodb://localhost:27017/heroes_claims_collection_migration");
const bountyContractId = process.env.BOUNTY_CONTRACT_ID || "bounties.heroes.near";
const limit = Number(process.env.CLAIMS_FOR_ONE_ITERATION) || 20;

const Gas = Big(10).pow(12).mul(300).toFixed(0);

// Deleting data from previous claims collections
async function clearingPreviousCollections(near, db) {
  let skip = 0;
  let bountyClaimers = await db
    .collection("bounty_claimers")
    .find()
    .sort({ "_id": 1 })
    .skip(skip)
    .limit(limit)
    .toArray();
  while (bountyClaimers.length > 0) {
    const entries = bountyClaimers.map(c => c.ownerId);
    console.log(JSON.stringify(entries));

    await near.account.functionCall({
      contractId: bountyContractId,
      methodName: "clean_bounty_claimers",
      args: { claimers: entries },
      gas: Gas,
    });

    skip += bountyClaimers.length;
    bountyClaimers = await db
      .collection("bounty_claimers")
      .find()
      .sort({ "_id": 1 })
      .skip(skip)
      .limit(limit)
      .toArray();
  }

  skip = 0;
  let bountyClaimerAccounts = await db
    .collection("bounty_claims")
    .find()
    .sort({ "_id": 1 })
    .skip(skip)
    .limit(limit)
    .toArray();
  while (bountyClaimerAccounts.length > 0) {
    const entries = bountyClaimerAccounts.map(c => c.id);
    console.log(JSON.stringify(entries));

    await near.account.functionCall({
      contractId: bountyContractId,
      methodName: "clean_bounty_claimer_accounts",
      args: { bounties: entries },
      gas: Gas,
    });

    skip += bountyClaimerAccounts.length;
    bountyClaimerAccounts = await db
      .collection("bounty_claims")
      .find()
      .sort({ "_id": 1 })
      .skip(skip)
      .limit(limit)
      .toArray();
  }
}

// Populating new collections with claims data
async function fillingNewCollections(near, db) {
  let skip = 0;
  let claims = await db
    .collection("claims")
    .find()
    .sort({ "id": 1 })
    .skip(skip)
    .limit(limit)
    .toArray();

  while (claims.length > 0) {
    const entries = claims.map(c => ({
      owner: c.owner || null,
      bounty_id: c.bounty_id || c.bounty_id === 0 ? c.bounty_id : null,
      created_at: c.created_at || null,
      start_time: c.start_time || null,
      deadline: c.deadline || null,
      description: c.description || null,
      status: c.status || null,
      bounty_payout_proposal_id: c.bounty_payout_proposal_id || null,
      approve_claimer_proposal_id: c.approve_claimer_proposal_id || null,
      rejected_timestamp: c.rejected_timestamp || null,
      dispute_id: c.dispute_id || null,
      is_kyc_delayed: c.is_kyc_delayed || null,
      payment_timestamps: c.payment_timestamps || null,
      slot: c.slot || c.slot === 0 ? c.slot : null,
      bond: c.bond || null,
      claim_number: c.claim_number || c.claim_number === 0 ? c.claim_number : null,
    }));

    console.log(JSON.stringify(entries));

    await near.account.functionCall({
      contractId: bountyContractId,
      methodName: "migrate_claims",
      args: { claims: entries },
      gas: Gas,
    });

    skip += claims.length;
    claims = await db
      .collection("claims")
      .find()
      .sort({ "id": 1 })
      .skip(skip)
      .limit(limit)
      .toArray();
  }
}

async function main () {
  const near = await nearInit();

  await client.connect();
  console.log("Connected successfully to MongoDB server");
  const db = client.db();

  await clearingPreviousCollections(near, db);
  await fillingNewCollections(near, db);

  return "Done.";
}

main()
  .then(console.log)
  .catch(console.error)
  .finally(() => client.close());
