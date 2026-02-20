#!/usr/bin/env python3
import argparse
import json
import sys
from pathlib import Path


def parse_hex32(value: str, label: str) -> bytes:
    raw = value[2:] if value.startswith("0x") else value
    if len(raw) != 64:
        raise ValueError(f"{label} must be 32 bytes hex")
    try:
        return bytes.fromhex(raw)
    except ValueError as exc:
        raise ValueError(f"invalid {label}: {exc}") from exc


def gwei_to_wei(value: float) -> int:
    return int(value * 1_000_000_000)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Deploy PowerHouseToken using compiled artifact + JSON-RPC"
    )
    parser.add_argument("--rpc-url")
    parser.add_argument("--private-key", required=True)
    parser.add_argument("--artifact", required=True)
    parser.add_argument("--owner", required=True)
    parser.add_argument("--snapshot-height", required=True, type=int)
    parser.add_argument("--conversion-ratio", type=int, default=1)
    parser.add_argument("--treasury-mint", type=int, default=0)
    parser.add_argument("--migration-root", required=True)
    parser.add_argument("--nonce", type=int)
    parser.add_argument("--chain-id", type=int)
    parser.add_argument("--gas", type=int)
    parser.add_argument("--gas-price-gwei", type=float)
    parser.add_argument("--max-fee-gwei", type=float)
    parser.add_argument("--max-priority-fee-gwei", type=float)
    parser.add_argument("--wait-timeout", type=int, default=180)
    parser.add_argument("--output")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    try:
        from web3 import Web3
    except Exception as exc:  # pragma: no cover
        print(f"web3 import failed: {exc}", file=sys.stderr)
        print("Install dependency: pip install web3", file=sys.stderr)
        return 1

    artifact_path = Path(args.artifact)
    artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
    abi = artifact.get("abi")
    bytecode = artifact.get("bytecode")
    if not abi or not bytecode:
        raise ValueError("artifact must include abi and bytecode")

    migration_root = parse_hex32(args.migration_root, "--migration-root")

    connected = False
    if args.rpc_url:
        w3 = Web3(Web3.HTTPProvider(args.rpc_url, request_kwargs={"timeout": 30}))
        connected = w3.is_connected()
    else:
        w3 = Web3()
    if not connected and not args.dry_run:
        raise RuntimeError("failed to connect to rpc (provide --rpc-url)")

    deployer = w3.eth.account.from_key(args.private_key)
    owner = Web3.to_checksum_address(args.owner)
    chain_id = (
        args.chain_id
        if args.chain_id is not None
        else (int(w3.eth.chain_id) if connected else 1)
    )
    nonce = (
        args.nonce
        if args.nonce is not None
        else (int(w3.eth.get_transaction_count(deployer.address)) if connected else 0)
    )

    contract = w3.eth.contract(abi=abi, bytecode=bytecode)
    constructor = contract.constructor(
        owner,
        int(args.snapshot_height),
        int(args.conversion_ratio),
        int(args.treasury_mint),
        migration_root,
    )

    base_tx = {
        "from": deployer.address,
        "nonce": nonce,
        "chainId": chain_id,
    }
    if connected:
        tx = constructor.build_transaction(base_tx)
    else:
        tx = dict(base_tx)
        tx["data"] = constructor.data_in_transaction

    if args.gas is not None:
        tx["gas"] = int(args.gas)
    elif not connected:
        tx["gas"] = 2_500_000
    else:
        estimated = int(constructor.estimate_gas({"from": deployer.address}))
        tx["gas"] = max(estimated + 50_000, int(estimated * 1.2))

    latest = w3.eth.get_block("latest") if connected else {}
    if args.gas_price_gwei is not None:
        tx["gasPrice"] = gwei_to_wei(args.gas_price_gwei)
    elif connected and "baseFeePerGas" in latest and latest["baseFeePerGas"] is not None:
        priority = (
            gwei_to_wei(args.max_priority_fee_gwei)
            if args.max_priority_fee_gwei is not None
            else int(getattr(w3.eth, "max_priority_fee", w3.to_wei(1, "gwei")))
        )
        max_fee = (
            gwei_to_wei(args.max_fee_gwei)
            if args.max_fee_gwei is not None
            else int(latest["baseFeePerGas"] * 2 + priority)
        )
        tx["maxPriorityFeePerGas"] = priority
        tx["maxFeePerGas"] = max_fee
    elif not connected:
        tx["gasPrice"] = gwei_to_wei(20.0)
    else:
        tx["gasPrice"] = int(w3.eth.gas_price)

    preview = {
        "rpc_url": args.rpc_url,
        "deployer": deployer.address,
        "owner": owner,
        "chain_id": chain_id,
        "nonce": nonce,
        "snapshot_height": int(args.snapshot_height),
        "conversion_ratio": int(args.conversion_ratio),
        "treasury_mint": int(args.treasury_mint),
        "migration_root": "0x" + migration_root.hex(),
        "tx": {
            k: (hex(v) if isinstance(v, int) else v)
            for k, v in tx.items()
            if k in {"gas", "gasPrice", "maxFeePerGas", "maxPriorityFeePerGas", "nonce", "chainId"}
        },
    }

    if args.dry_run:
        print(json.dumps(preview, indent=2))
        return 0

    signed = deployer.sign_transaction(tx)
    tx_hash = w3.eth.send_raw_transaction(signed.raw_transaction)
    receipt = w3.eth.wait_for_transaction_receipt(tx_hash, timeout=args.wait_timeout)

    if receipt.status != 1:
        raise RuntimeError(f"deployment reverted: tx={tx_hash.hex()}")

    result = {
        "schema": "mfenx.powerhouse.deployment-receipt.v1",
        "tx_hash": tx_hash.hex(),
        "contract_address": receipt.contractAddress,
        "chain_id": chain_id,
        "deployer": deployer.address,
        "owner": owner,
        "snapshot_height": int(args.snapshot_height),
        "conversion_ratio": int(args.conversion_ratio),
        "treasury_mint": int(args.treasury_mint),
        "migration_root": "0x" + migration_root.hex(),
        "block_number": int(receipt.blockNumber),
        "gas_used": int(receipt.gasUsed),
    }

    if args.output:
        Path(args.output).write_text(json.dumps(result, indent=2), encoding="utf-8")
        print(f"wrote deployment receipt: {args.output}")
    else:
        print(json.dumps(result, indent=2))

    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"error: {exc}", file=sys.stderr)
        raise SystemExit(1)
