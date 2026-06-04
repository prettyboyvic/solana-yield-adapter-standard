import {
  REFERENCE_ADAPTERS,
  REFERENCE_ADAPTER_PROGRAM_ID,
  bytesToHex,
} from "../packages/sdk/src/index.js";

for (const adapter of REFERENCE_ADAPTERS) {
  console.log(
    JSON.stringify(
      {
        label: adapter.label,
        adapterIdHex: bytesToHex(adapter.adapterId),
        adapterProgram: REFERENCE_ADAPTER_PROGRAM_ID,
        protocol: adapter.protocol,
        underlyingMint: adapter.underlyingMint,
        receiptMint: adapter.receiptMint,
        capabilities: adapter.capabilities,
        riskTier: adapter.riskTier,
        metadataUri: adapter.metadataUri,
      },
      null,
      2,
    ),
  );
}

