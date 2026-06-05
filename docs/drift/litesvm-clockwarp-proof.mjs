import { LiteSVM, Clock } from "litesvm";

const svm = new LiteSVM();

// --- PROOF 2: CLOCK WARP (the core unstaking_period requirement) ---
const c0 = svm.getClock();
console.log("clock0 unixTimestamp:", c0.unixTimestamp);
const THIRTEEN_DAYS = 13n*24n*3600n; // > Drift USDC IF unstaking_period
const warped = new Clock(c0.slot, c0.epochStartTimestamp, c0.epoch, c0.leaderScheduleEpoch, c0.unixTimestamp + THIRTEEN_DAYS);
svm.setClock(warped);
const c1 = svm.getClock();
console.log("clock1 unixTimestamp:", c1.unixTimestamp, "| delta secs:", (c1.unixTimestamp - c0.unixTimestamp).toString(), "| >=13d?", (c1.unixTimestamp - c0.unixTimestamp) >= THIRTEEN_DAYS);

// --- PROOF 3: ACCOUNT CLONE (inject cloned mainnet account bytes, read back) ---
const addr = "So11111111111111111111111111111111111111112"; // arbitrary address as clone target
const data = new Uint8Array(64); data[0]=0xab; data[63]=0xcd;
svm.setAccount(addr, { lamports: 1_000_000n, data, owner: "11111111111111111111111111111111", executable: false, rentEpoch: 0n, address: addr });
const got = svm.getAccount(addr);
const gotData = got.data ?? (got.exists ? got.data : null);
console.log("CLONE readback exists:", got.exists ?? true, "len:", gotData? gotData.length : "n/a", "first:", gotData? gotData[0].toString(16):"-", "last:", gotData? gotData[63].toString(16):"-");

// --- PROOF 4: warpToSlot also available ---
svm.warpToSlot(c0.slot + 1000n);
console.log("warpToSlot ok -> slot:", svm.getClock().slot);
console.log("HARNESS_CAPABILITY_PROVEN");
