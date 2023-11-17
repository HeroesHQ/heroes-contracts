require('dotenv').config();
const Big = require("big.js");
const homedir = require("os").homedir();
const { MongoClient } = require('mongodb');
const nearAPI = require("near-api-js");
const path = require("path");

const client = new MongoClient(process.env.MONGO_URL || 'mongodb://localhost:27017');
const databaseName = process.env.DB_NAME || "heroes";

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
const contractAccountId = process.env.KYC_WHITELIST_CONTRACT_ID;

const convertDate = (date) => {
  return String(new Date(date).getTime() * 1000000);
};

async function main () {
  const near = await nearAPI.connect(getNearConfig());
  const account = await near.account(contractAccountId);

  await client.connect();
  console.log("Connected successfully to MongoDB server");
  const db = client.db(databaseName);

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
        verification_type: whitelistEntry.verificationType?.toUpperCase(),
        provider: whitelistEntry.provider,
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
