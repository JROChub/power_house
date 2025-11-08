# Training Binder — Power House Cadet Packet

This binder distills the mandatory drills called out across `book_of_power.md`. Work through each section, initial in the signature block, and archive the completed packet with your ledger logs.

## 1. Field Prime Drills *(Book refs: Chapter II, §§01–41)*

| Drill | Prime | Task | Scratchpad |
| --- | --- | --- | --- |
| F-1 | 97 | Compute inverses for 12, 37, 81. Show that `a * a⁻¹ ≡ 1 (mod p)` for each. | |
| F-2 | 101 | Verify `FieldElement::new(57).pow(100) == 1`. Note each intermediate square-and-multiply step. | |
| F-3 | 257 | Reduce the aggregated digest `0xa5a1…99b3` to the field element `21` by taking the first 8 bytes → `u64::from_be_bytes` → `mod 257`. | |
| F-4 | 65537 | Demonstrate extended Euclidean inversion by hand for 37. Record quotient steps and bezout coefficients. | |

*Completion checklist:* □ documented modulus choice □ noted failure cases □ reran `cargo test field_inverse`.

## 2. Transcript Printouts *(Book refs: Chapter IV, §§01–45)*

These are direct copies from `/tmp/power_house_anchor_a` after running `cargo run --example hash_pipeline` (2025-??-?? UTC).

```
statement: Dense polynomial proof
transcript: 247 246 144 68 105 92 243 202 72 124
round_sums: 209 235 57 13 205 8 245 122 72 159
final: 9
hash: ded75c45b3b7eedd37041aae79713d7382e000eb4d83fab5f6aca6ca4d276e8c
```

```
statement: Hash anchor proof
transcript: 17 230 192 174 226 171
round_sums: 21 139 198 99 178 89
final: 173
hash: 0f50904f7be06930a5500c2c54cfb6c2df76241507ebd01ab0a25039d2f08f9b
```

Instructions: highlight challenge lines, check that each `round_sums` pair collapses to the previous accumulator, and confirm hashes match Chapter I (`book_of_power.md:24-34`).

## 3. Challenge Logs *(Book refs: Chapter VI, §§01–40)*

| Ledger | Counter | Digest (first 16 hex chars) | Challenge (mod 257) |
| --- | --- | --- | --- |
| Dense polynomial proof | 0 | `f7bcf2e0e9860d4d` | 247 |
|  | 1 | `f65f6ef933dc4b64` | 246 |
|  | 2 | `908bda4ae594c51f` | 144 |
|  | 3 | `44f55f17796313e4` | 68 |
|  | 4 | `9669d58a27cc7ced` | 105 |
|  | 5 | `5c00555f915f4b66` | 92 |
|  | 6 | `b49abee5b352329a` | 243 |
|  | 7 | `b28760025f1fa09d` | 202 |
|  | 8 | `25163b3e11ef8670` | 72 |
|  | 9 | `5f61c2142fb120f8` | 124 |
| Hash anchor proof | 0 | `8bf810b1384df5f4` | 17 |
|  | 1 | `fd5f6e43f43d61fc` | 230 |
|  | 2 | `8c441a0862b041b0` | 192 |
|  | 3 | `e0fa4c7657898e52` | 174 |
|  | 4 | `7e4f2b81f54654ad` | 226 |
|  | 5 | `2b026c77c9244f33` | 171 |

*Exercise:* recompute the BLAKE2b-256 state with domain tag `JROC_CHALLENGE`, transcript words, and counter; confirm the reduced challenges match.

## 4. Signature Blocks *(Book refs: Chapter III §97, Chapter V §91, Chapter VIII §§30–35)*

1. **Field Algebra Mastery**
   - Name: ______________________
   - Date: ______________________
   - Statement: “I can reproduce every prime-field example in Chapter II without aid.” Signature: ______________________

2. **Transcript Metallurgy**
   - Verified ledgers (`hash_pipeline` date/time): ______________________
   - Hashes checked: □ `ded75c45…6e8c` □ `0f50904f…08f9b`
   - Signature: ______________________

3. **Challenge Reconstruction**
   - Dense proof challenges recomputed? □ Yes □ No
   - Hash anchor challenges recomputed? □ Yes □ No
   - Signature: ______________________

4. **Consensus Drill**
   - Boot nodes started and reconciled per Chapter VII? □ Yes □ No
   - Metrics snapshot archived at: ______________________
   - Signature: ______________________

Store completed sheets in the compliance ledger alongside the latest anchor files. Attach any calculator printouts or spreadsheets used during the drills for future audits.
