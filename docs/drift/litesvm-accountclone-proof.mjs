import { LiteSVM } from "litesvm";
const svm = new LiteSVM();
const addr = "So11111111111111111111111111111111111111112";
const data = new Uint8Array(64); data[0]=0xab; data[63]=0xcd;
// simulate a cloned mainnet account (e.g. Drift SpotMarket) injected wholesale
svm.setAccount({ address: addr, lamports: 1_000_000n, data, programAddress: "11111111111111111111111111111111", executable: false });
const got = svm.getAccount(addr);
console.log("CLONE exists:", got.exists, "len:", got.data.length, "first:", got.data[0].toString(16), "last:", got.data[63].toString(16), "owner:", got.programAddress);
console.log("ACCOUNT_CLONE_PROVEN");
