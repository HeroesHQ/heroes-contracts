require('dotenv').config();
// const Big = require("big.js");
const homedir = require("os").homedir();
const { MongoClient } = require('mongodb');
const nearAPI = require("near-api-js");
const path = require("path");

const client = new MongoClient(process.env.MONGO_URL || "mongodb://localhost:27017/heroes_claims_collection_migration");
const bountyContractId = process.env.BOUNTY_CONTRACT_ID || "bounties.heroes.near";
const network = process.env.NEAR_NETWORK_ENV || 'mainnet';

// const Gas = Big(10).pow(12).mul(300).toFixed(0);

const nearInit = async () => {
  const credentialsPath = path.join(homedir, ".near-credentials");
  const keyStore = new nearAPI.keyStores.UnencryptedFileSystemKeyStore(
    credentialsPath
  );
  const config = {
    networkId: network,
    nodeUrl: network === "mainnet" ? "https://rpc.mainnet.near.org" : "https://rpc.testnet.near.org",
    keyStore
  };

  const near = await nearAPI.connect(config);
  const account = await near.account(bountyContractId);

  return { account };
};

async function getOnePageBounties(near, fromIndex = 0, limit = 100) {
  return near.account.viewFunction({
    contractId: bountyContractId,
    methodName: "get_bounties",
    args: {
      from_index: fromIndex,
      limit,
    },
  });
}

async function getAllBounties(near) {
  const result = [];
  let fromIndex = 0;
  const limit = 100;
  let bounties = await getOnePageBounties(near, fromIndex, limit);

  while (bounties.length > 0) {
    Array.prototype.push.apply(result, bounties);
    if (bounties.length < limit) {
      break;
    }
    fromIndex += limit;
    bounties = await getOnePageBounties(near, fromIndex, limit);
  }

  return result;
}

async function getBountyClaimsById(near, id) {
  return near.account.viewFunction({
    contractId: bountyContractId,
    methodName: "get_bounty_claims_by_id",
    args: { id  },
  });
}

async function collectDataFromContract(near, db) {
  const bounties = await getAllBounties(near);

  if ((await db.listCollections({ name: "bounties" }).toArray())?.length > 0) {
    await db.collection("bounties").deleteMany({});
  }
  if ((await db.listCollections({ name: "claims" }).toArray())?.length > 0) {
    await db.collection("claims").deleteMany({});
  }
  if (
    (await db.listCollections({ name: "claims" }).toArray())?.length > 0 &&
    !(await db.collection("claims").indexes())?.find(i => i.name === "created_at_1")
  ) {
    await db.collection("claims").createIndex({ "created_at": 1 });
  }
  if (
    (await db.listCollections({ name: "claims" }).toArray())?.length > 0 &&
    !(await db.collection("claims").indexes())?.find(i => i.name === "id_1")
  ) {
    await db.collection("claims").createIndex({ "id": 1 });
  }

  for (const b of bounties) {
    const bounty = { id: b[0], ...b[1] };
    await db.collection("bounties").insertOne(bounty);

    const claims = await getBountyClaimsById(near, b[0]);

    for (const c of claims) {
      const claim = { ...c[1], owner: c[0] };
      await db.collection("claims").insertOne(claim);
    }
  }
}

async function setClaimIDs(db) {
  const claims = await db.collection("claims").find().sort({ "created_at": 1 }).toArray();
  let index = 0;

  for (const claim of claims) {
    await db.collection("claims").updateOne({ _id: claim._id }, { $set: { id: index } });
    index += 1;
  }
}

async function buildNewCollections(db) {
  // bounty_claims: LookupMap<BountyIndex, Vec<AccountId>>,
  if ((await db.listCollections({ name: "bounty_claimers" }).toArray())?.length > 0) {
    await db.collection("bounty_claimers").deleteMany({});
  }
  if ((await db.listCollections({ name: "bounty_claims" }).toArray())?.length > 0) {
    await db.collection("bounty_claims").deleteMany({});
  }
  if (
    (await db.listCollections({ name: "bounty_claimers" }).toArray())?.length > 0 &&
    !(await db.collection("bounty_claimers").indexes())?.find(i => i.name === "ownerId_1")
  ) {
    await db.collection("bounty_claimers").createIndex({ "ownerId": 1 });
  }
  if (
    (await db.listCollections({ name: "bounty_claims" }).toArray())?.length > 0 &&
    !(await db.collection("bounty_claims").indexes())?.find(i => i.name === "id_1")
  ) {
    await db.collection("bounty_claims").createIndex({ "id": 1 });
  }

  const claims = await db.collection("claims").find().sort({ "id": 1 }).toArray();

  for (const claim of claims) {
    const bountyClaimer = await db.collection("bounty_claimers").findOne({ ownerId: claim.owner });

    if (bountyClaimer) {
      await db.collection("bounty_claimers").updateOne(
        { ownerId: claim.owner },
        { $push: { claims: claim.id } }
      );
    } else {
      await db.collection("bounty_claimers").insertOne({
        ownerId: claim.owner,
        claims: [claim.id],
      });
    }

    const bountyClaims = await db.collection("bounty_claims").findOne({ id: claim.bounty_id });

    if (bountyClaims) {
      await db.collection("bounty_claims").updateOne(
        { id: claim.bounty_id },
        { $push: { claims: claim.id } }
      );
    } else {
      await db.collection("bounty_claims").insertOne({
        id: claim.bounty_id,
        claims: [claim.id],
      });
    }
  }
}

async function main () {
  const near = await nearInit();

  await client.connect();
  console.log("Connected successfully to MongoDB server");
  const db = client.db();

  await collectDataFromContract(near, db);
  await setClaimIDs(db);
  await buildNewCollections(db);

  // TODO
  return "Done.";
}

main()
  .then(console.log)
  .catch(console.error)
  .finally(() => client.close());
