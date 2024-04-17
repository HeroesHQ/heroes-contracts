import path from "path";
const homedir = require("os").homedir();
import nearAPI from "near-api-js";

const bountyContractId = process.env.BOUNTY_CONTRACT_ID || "bounties.heroes.near";
const network = process.env.NEAR_NETWORK_ENV || 'mainnet';

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

module.exports = { nearInit };
