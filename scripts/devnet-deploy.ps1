# Devnet deploy — run on the Windows machine that has Agave/Solana 2.2.20.
# Usage:  powershell -ExecutionPolicy Bypass -File scripts\devnet-deploy.ps1
$ErrorActionPreference = "Stop"

$sol = "C:\Users\vudat\.local\share\solana\install\releases\2.2.20\solana-release\bin"
$payer = "target\deploy\devnet-payer.json"
$rpc = "https://api.devnet.solana.com"

$payerPubkey = & "$sol\solana.exe" address -k $payer
Write-Host "Devnet payer: $payerPubkey"

# 1) Ensure the payer has SOL. Faucet is rate-limited; retry or use the web faucet
#    https://faucet.solana.com (paste the pubkey above) until balance >= 6 SOL.
$balance = & "$sol\solana.exe" balance $payerPubkey --url $rpc
Write-Host "Balance: $balance"
# Optional CLI airdrop (often rate-limited):
# & "$sol\solana.exe" airdrop 2 $payerPubkey --url $rpc

# 2) Deploy both programs.
& "$sol\solana.exe" program deploy `
  target\sbf-solana-solana\release\yield_adapter_dispatcher.so `
  --program-id target\deploy\yield_adapter_dispatcher-keypair.json `
  --keypair $payer --url $rpc

& "$sol\solana.exe" program deploy `
  target\sbf-solana-solana\release\reference_yield_adapter.so `
  --program-id target\deploy\reference_yield_adapter-keypair.json `
  --keypair $payer --url $rpc

# 3) Confirm both program ids are executable on devnet.
& "$sol\solana.exe" program show 37fdMFG3eh91i7WYk4MgwYBGqoXK4dbpV73UUh6uxvtY --url $rpc
& "$sol\solana.exe" program show BCvRj9JakpU1mpo67yt7WjknSAcTqAJMWCSyurcRhBb1 --url $rpc

# 4) Print the registry init + 5-adapter registration payloads to record in submission.md.
npm run adapters:print

Write-Host ""
Write-Host "Done. Paste the deploy tx signatures + 'program show' output into docs/submission.md."
