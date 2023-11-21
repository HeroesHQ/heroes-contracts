require('dotenv').config();
const Big = require("big.js");
const homedir = require("os").homedir();
const { MongoClient } = require('mongodb');
const nearAPI = require("near-api-js");
const path = require("path");

const client = new MongoClient(process.env.MONGO_URL || 'mongodb://localhost:27017/heroes');

const credentialsPath = path.join(homedir, ".near-credentials");
const keyStore = new nearAPI.keyStores.UnencryptedFileSystemKeyStore(
  credentialsPath
);

const Gas = Big(10).pow(12).mul(300).toFixed(0);

const testnet = {
  networkId: "testnet",
  nodeUrl: "https://rpc.testnet.near.org",
};

const mainnet = {
  networkId: "mainnet",
  nodeUrl: "https://rpc.mainnet.near.org",
};

const nearConfigs = {
  testnet,
  mainnet,
};

const getNearConfig = () => {
  const network = process.env.NEAR_NETWORK_ENV || 'testnet';
  const config = nearConfigs[network];
  return {
    keyStore,
    ...config,
  };
};

const config = JSON.parse(process.env.CONTRACT_CONFIG || "{}");

const convertDate = (date) => {
  return String(new Date(date).getTime() * 1000000);
};

async function main () {
  const near = await nearAPI.connect(getNearConfig());

  await client.connect();
  console.log("Connected successfully to MongoDB server");
  const db = client.db();

  const heroesConfig = await db
    .collection("configurations")
    .find()
    .limit(1)
    .toArray();
  const contractAccountId = heroesConfig?.[0]?.config?.kyc_whitelist_contract_id;
  console.log(`Whitelist contract account: ${contractAccountId}`);
  const account = await near.account(contractAccountId);

  const applicants = await db
    .collection("kycapplicants")
    .find(
      {
        $and: [
          {"finished": {$eq: true}},
          {
            $or: [
              {
                $and: [
                  { "provider": { $eq: 'fractal' } },
                  { "fractalAnswer": { $eq: 'approved' } }
                ]
              },
              {
                $and: [
                  { "provider": { $eq: 'sumsub' } },
                  { "reviewAnswer": { $eq: 'GREEN' } }
                ]
              }
            ]
          }
        ]
      }
    )
    .sort({"accountId": 1, "finishedAt": 1})
    .toArray();

  const accounts = applicants.reduce(
    (result, applicant) => {
      result[applicant.accountId] = result[applicant.accountId] || [];
      result[applicant.accountId].push(applicant);
      return result;
    },
    {}
  );
  const entries = [];

  for (const accountId of Object.keys(accounts)) {
    const whitelistEntries = [];
    for (const whitelistEntry of accounts[accountId]) {
      whitelistEntries.push({
        verification_date: convertDate(
          whitelistEntry.finishedAt || whitelistEntry.updatedAt || new Date()
        ),
        provider: whitelistEntry.provider,
        verification_type: whitelistEntry.verificationType?.toUpperCase(),
        verification_level: whitelistEntry.levelName,
        ident_document_date_of_expiry: null,
      });
    }
    entries.push([accountId, whitelistEntries]);
  }

  console.log(JSON.stringify(entries));

  await account.functionCall({
    contractId: contractAccountId,
    methodName: "migrate",
    args: { config, entries },
    gas: Gas,
  });

  return "Done.";
}

main()
  .then(console.log)
  .catch(console.error)
  .finally(() => client.close());
