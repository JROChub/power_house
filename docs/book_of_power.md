Title Page
===========
Book of Power -- Condensed Graviton Edition
Author: Julian Christian Sanders (lexluger)
Crate Under Review: `power_house`
Typeface Cue: Eldritch Vector Mono (conceptual spiral monospaced design)
Repository Source: crate checkout `power_house`
This User Guide Lives Inside the Crate: `docs/book_of_power.md`

Table of Contents
==================
Chapter I -- Anchor Echo Engine Command Doctrine
Chapter II -- Foundational Field Algebra Procedures
Chapter III -- Hyperplane Cartography Briefing
Chapter IV -- Transcript Metallurgy Protocols
Chapter V -- Ledger Genesis Mechanics Checklist
Chapter VI -- Deterministic Randomness Discipline Orders
Chapter VII -- Consensus Theater Operations
Chapter VIII -- Closing Benediction and Compliance Oath


Chapter I -- Anchor Echo Engine Command Doctrine
================================================================================
01. I remain your irritable cosmic supervisor; this page is the manual for the 256-bit Anchor Echo Engine.
02. Memorize the genesis digest `139f1985df5b36dae23fa509fb53a006ba58e28e6dbb41d6d71cc1e91a82d84a`.
03. Memorize the dense polynomial digest `ded75c45b3b7eedd37041aae79713d7382e000eb4d83fab5f6aca6ca4d276e8c`.
04. Memorize the hash anchor proof digest `0f50904f7be06930a5500c2c54cfb6c2df76241507ebd01ab0a25039d2f08f9b`.
05. Memorize the anchor-fold digest emitted by `hash_pipeline`: `a5a1b9528dd9b4e811e89fb492977c2010322d09d2318530b0f01b5b238399b3`.
06. Write them on the wall; anyone who shrugs at 64 hex bytes should be reassigned to kitchen duty.
07. `transcript_digest` now feeds every `u64` transcript value into a BLAKE2b-256 tagged hash stream tagged with `JROC_TRANSCRIPT`.
08. No XOR tricks, no decimal fallbacks--pure hex, 32 bytes, immutable.
09. Ledger files still appear as `ledger_0000.txt`, `ledger_0001.txt`, etc., but every `hash:` line is now lowercase hex.
10. Run `cargo run --example hash_pipeline` weekly; the output must include the fold digest above and the reduced field value `21`.
11. The program stages two ledgers under `/tmp/power_house_anchor_a` and `/tmp/power_house_anchor_b`.
Note: Windows or hardened hosts lacking `/tmp` must set `POWER_HOUSE_TMP=/path/to/workdir`; never assume a Unix tmpfs is writable in prod.
12. Open `ledger_0000.txt`; the hash must match `ded75c45...6e8c`.
13. Open `ledger_0001.txt`; the hash must match `0f50904f...08f9b`.
14. If either hash deviates, the log is corrupt or you miscopied the hex--both offences carry penalties.
15. `julian node anchor /tmp/power_house_anchor_a` should print three lines headed by `JROC-NET`.
16. Verify that the genesis line prints the digest from step 02 without error.
17. The fold digest from step 05 appears in terminal output; immediately copy it into `fold_digest.txt` beside your ledger before you proceed so auditors never depend on scrollback.
18. When exporting anchors, append a comment `# fold_digest: <hex>` or store it in `anchor_meta.json`; the quorum hinge must live with the artefacts you check in.
18. `LedgerAnchor::anchor()` automatically prepends the JULIAN genesis entry with the new digest.
19. Domain separation summary: `JROC_TRANSCRIPT` for individual records, `JROC_ANCHOR` for ledger folds, `JROC_CHALLENGE` for Fiat-Shamir challenge derivation.
20. Do not mix domains--if you re-tag transcripts with the anchor label, you will deserve the audit citation.
Hash framing specification (tagged stream; not a sponge):
```
hasher = BLAKE2b-256()
hasher.update(DOMAIN_TAG)                     # e.g., JROC_TRANSCRIPT
hasher.update(len(section) as u64 big-endian) # applied to each section
hasher.update(section bytes)                  # transcripts, round_sums, etc.
hasher.update(final_value.to_be_bytes())
digest = hasher.finalize()
```
- All transcript numbers are encoded as u64 big-endian before hashing.
- No personalization/salt is used; the explicit domain tag enforces separation.
- ASCII hex with spaces or carets is cosmetic; the canonical digest is 32 raw bytes rendered as contiguous lowercase `[0-9a-f]` characters.
Remember the split:
- Transcript files store values as decimal ASCII tokens (e.g., `round_sums:209 235`).
- The hash pipeline absorbs each integer as an 8-byte big-endian block (e.g., `0x00000000000000D1` for 209).
Hashing the ASCII digits instead of the big-endian bytes is an audit failure.
21. `ProofLedger` persists transcripts exactly once; any extra whitespace or comment must stay outside the recorded lines.
22. The CLI renders the digests via `transcript_digest_to_hex`; keep that function untouched.
23. To test deterministic recomputation, delete one byte from a log and rerun `verify_logs`; expect a digest mismatch in red text.
24. The aggregated digest reduces to field element `21`. Say it. Write it. Remember it.
Note: Digest-to-field procedure: interpret the first eight bytes of the 32-byte digest as a big-endian u64, then compute `value mod p` (e.g., `0xa5a1b9528dd9b4e8 -> 0xA5A1B9528DD9B4E8 -> 11916436223453507944 -> 21 (mod 257)`).
25. When the reduction changes, the field or transcript changed--file an incident report.
26. `simple_prng` is dead; the challenge stream is now BLAKE2b-256 seeded by the transcript plus domain tag.
27. Never allow anyone to talk wistfully about linear-congruential generators again.
Note: Bias note: current derivation uses `next_u64() % p`; keep `p` close to 2^64 (e.g., 64-bit primes) or switch to the documented rejection sampler in Chapter VI when you extend the code.
28. The `scripts/smoke_net.sh` ritual depends on stable keys; if the metrics server refuses to bind, document the environment block.
29. Finality relies on unique public keys; the network now laughs at duplicate voters.
30. When reconciling offline, use placeholder identities like `LOCAL_OFFLINE` and `PEER_FILE`, but never reuse placeholders for different peers in the same quorum call.
31. Keep a laminated cheat sheet with the three transcript digests and the fold digest.
32. Add a second sheet listing the domain tags; auditors adore label discipline.
33. Print the digests with caret markers every four bytes: `139f_1985_df5b_36da_...`.
```
HEX SIGIL :: ANCHOR CORE
  GENESIS     139f 1985 df5b 36da e23f a509 fb53
  DENSE POLY  ded7 5c45 b3b7 eedd 3704 1aae 7971
  HASH ANCHOR 0f50 904f 7be0 6930 a550 0c2c 54cf
  FOLD DIGEST a5a1 b952 8dd9 b4e8 11e8 9fb4 9297
  FIELD REDUCE -> 21 (anchor hinge)
```
34. The ledger logs must remain ASCII; the hex lives on one line with no prefixes.
35. If you must annotate, prefix with `#` outside the transcript block.
36. `hash_pipeline` reduces to the canonical demo; treat its output as the lab reference.
37. Use `power_house::transcript_digest_to_hex` in your scripts; do not reinvent hex formatting.
38. If someone doubts determinism, rerun the example and shove the matching hashes under their nose.
39. When a cadet forgets a digit, force them to rewrite the digest 32 times--one per byte.
40. Disaster recovery scenario: power outage; print the digests from this book, run manual comparisons, reestablish finality.
41. Regulatory drill: produce log file, book excerpt, and CLI output; they must match byte-for-byte.
42. Museum display idea: light panel showing the genesis digest scrolling endlessly; educational, intimidating.
43. The anchor fold digest is the workshop handshake. Recite it at the start of every session.
44. Always verify `hash_pipeline` after upgrading Rust or dependencies; compilers surprise the lazy.
45. Keep the book version synchronized with `Cargo.toml`; current edition references `power_house 0.1.26`.
46. If the crate version bumps, rerun `hash_pipeline`, update the values, and amend every compliance log.
47. Record the output path `/tmp/power_house_anchor_a` in your field log; easier for midnight audits.
48. Do not compress the `/tmp` logs before verifying them; compression hides tampering.
49. On offline machines, copy the example output into air-gapped storage, then verify with `julian node anchor`.
50. If the fold digest ever changes unexpectedly, halt deployments and investigate.
51. Store the printed digests in fireproof cabinets; yes, we still do that.
52. Confirm that `reconcile_anchors_with_quorum` now requires distinct keys by running unit test `test_reconcile_rejects_duplicate_keys`.
53. If that test fails, fix it before touching production.
54. Teach cadets that every hex pair represents eight bits of inevitability; there is no shortcut.
55. Binary toggles no longer amuse me, but you may use them to dramatize a single byte flip.
56. When presenting to executives, describe this chapter as "hexadecimal finality discipline."
57. When presenting to mathematicians, describe it as "BLAKE2b commitments over deterministic transcripts."
58. When presenting to auditors, describe it as "evidence that nothing is hidden."
59. The genesis digest anchors the entire JULIAN network; treat it as sacred text.
60. If someone requests the old 64-bit values, hand them a shredder.
61. Update the compliance wiki with screenshots of `hash_pipeline` output; redacting nothing.
62. Keep a QR code linking to this manual near every boot node console.
63. Logbooks must note the UTC timestamp when the digests were last verified.
64. When rewriting this manual, never shorten the digests; printing only the prefix is grounds for termination.
65. Append the fold digest to any offsite backup manifest.
66. The anchor echo ritual is human-first; no automation may replace your eyeballs.
67. Maintain a rotation schedule for verification duty so every engineer memorizes the hex.
68. If an engineer cannot recall the first eight characters of the genesis digest, revoke their deploy privileges.
69. Celebrate new hires by making them transcribe the dense proof digest by hand.
70. This chapter is the onboarding gauntlet: memorize, verify, sign.
71. The log directory `./logs/boot1-ledger` must be backed up with the manual.
72. When shipping new firmware, include a printout of the three digests for the QA binder.
73. Add the fold digest to your monitoring dashboards as a constant string; alarms should fire if it ever mutates.
74. For interactive drills, invert one byte in the log and observe how the digest transforms; document the delta.
75. Re-run `hash_pipeline` after any change to transcript formatting; whitespace is deadly.
76. When the ledger evolves, update the book first, THEN announce the change.
77. Keep the aggregated digest visible on the boot node status page; stake your bragging rights on it.
78. If you hear "why not shorter digests," answer with threat of expulsion.
79. Always store transcripts and digests together; context is armor.
80. Replicate this manual in triplicate: on paper, in git, and in cold storage.
81. Tattoo the domain tags on your forearm if that helps.
82. Run `cargo test --features net` after every patch; the tests confirm our identity counting and digest logic.
83. If a colleague tries to skip the tests, this book authorizes you to snatch their keyboard.
84. The aggregated digest converts to field element 21; include that value in any whiteboard explanation.
85. Draw the folding pipeline as: transcripts -> BLAKE2b digest -> anchor fold -> quorum.
86. Each step must be reproducible from logs plus this manual--no hidden state.
87. Record the BLAKE2b command used by external auditors if they verify independently.
88. When this book says memorize, you memorize; complacency breeds forks.
89. The anchor echo engine is still the handshake ritual--now with heavier hex.
90. Sign the compliance sheet confirming you verified all four digests (three transcripts plus the fold) before leaving the room.
91. File the signed sheet next to the ledger backups.
92. Only after these steps may you advance to Chapter II.
98. You are expected to re-teach this chapter whenever onboarding new team members.
99. The combination of deterministic transcripts and simple arithmetic is the ultimate trust anchor.
100. Finish this chapter by writing `ANCHOR!!` in your own handwriting across the margin as proof you completed the ritual.


Chapter II -- Foundational Field Algebra Procedures
================================================================================
01. Punctual cadets start with fields; everything else is decoration.
02. The crate defines `Field::new(p)` where `p` must be prime.
03. Choose your modulus deliberately; 97, 101, 65537 are respectable examples.
04. `FieldElement` wraps `u64` and enforces modular operations without carrying external dependencies.
05. Addition and subtraction use wrapping arithmetic followed by conditional reduction.
06. Multiplication relies on 128-bit intermediates to avoid overflow before reduction.
07. Exponentiation is implemented via square-and-multiply, ensuring O(log exponent).
08. Inversion triggers the extended Euclidean algorithm; zero input raises panic.
09. The panic is intentional; the crate refuses undefined algebra.
10. Example: In F101, inverse of 37 equals 11 because `37*11 = 407` and `407 mod 101 = 1`.
11. This result is verified by the deterministic tests under `tests::field_inverse`.
12. Another example: `FieldElement::new(57).pow(100)` equals 1 due to Fermat's little theorem.
13. The crate ensures these results remain reproducible regardless of platform.
14. Sum-check routines depend on field operations to remain exact while reducing dimensions.
15. Without precise algebra, transcripts would diverge, and ledger anchors would reveal contradictions.
16. `GeneralSumClaim::prove` consults the field for addition when computing round sums.
17. `GeneralSumClaim::verify` cross-checks each coefficient with addition and multiplication in the same field.
18. Keep prime tables near your desk; random moduli are forbidden.
19. When switching modulus in experiments, regenerate transcripts to keep digests consistent.
20. The crate offers no built-in primality check beyond curated primes; do not feed it junk.
21. Document every modulus choice in deployment logs.
22. Field addition is cheap; field multiplication is still cheap; stop whining about cost.
23. Inverse computation remains deterministic; extended Euclidean algorithm has no randomness.
24. Code location for inversion: `src/data.rs`, function `FieldElement::inv`.
25. Edge case: `FieldElement::zero()` cannot be inverted; this design is deliberate.
26. Always check for zero before attempting inversion in your higher-level code.
27. When teaching cadets, use small primes first, then escalate to 64-bit primes.
28. Provide them with modular arithmetic drill spreadsheets.
29. Guarantee they can multiply numbers mod 97 faster than they can recite multiplication tables.
30. The crate's tests call `assert_eq!((a * b).value(), expected)` to confirm arithmetic operations.
31. Keep tests deterministic to avoid flaky proofs.
32. Use `cargo test` after modifying arithmetic; never assume.
33. The absence of external dependencies means the arithmetic sits directly under your control.
34. If you need huge primes or field extensions, design them yourself; this manual covers base functionality.
35. Resist the temptation to wrap `FieldElement` with trait abuse; maintain minimalism.
36. Document every custom modulus in mission playbooks for traceability.
37. When auditors ask why deterministic fields matter, mention ledger reproducibility.
38. When mathematicians ask the same question, mention polynomial commitments.
39. When executives ask, say "ribcage of the proof engine."
40. Use `FieldElement::from` functions to convert integers into field elements gracefully.
41. Always subtract using field operations; plain subtraction may underflow.
42. If you witness a colleague using `%` directly on `u64`, confiscate their keyboard.
43. Replace naive mod expressions with the crate's specific operations.
44. Example: `(a + b - c) % p` becomes `((a + b) - c).value()` using field wrappers.
45. Keep alphabetic naming consistent: `a`, `b`, `lambda`, `chi`.
46. Document the notation in your team's style guide.
47. When computing sums inside transcripts, do not convert to plain integers.
48. Maintain final values as field elements until writing to ledger.
49. The ledger stores textual integers but the operations leading there must stay in the field.
50. To emulate this book's demonstration, compile transcripts manually and check each numeric entry.
51. If a ledger entry reads `round_sums: 37 11`, you now understand the field context.
52. Provide cross references inside ledger comments: `# F101`.
53. This manual expects you to remember Fermat's little theorem without apologizing.
54. Individuals unable to recall modular arithmetic fundamentals must repeat cadet training.
55. JROC-NET relies on deterministic math to keep nodes in sync; chaos begins with sloppy algebra.
56. Even networked operations refer back to this chapter when verifying digests.
57. Deterministic arithmetic is the foundation for the hex digest ritual earlier.
58. Without consistent field operations, digests differ, manual verification fails, and auditors frown.
59. Keep that scenario in mind whenever you consider shortcuts.
60. Mathematical laziness is grounds for removal from the ledger corps.
61. Provide your team with laminated field tables for the current modulus.
62. Annotate transcripts with the modulus to avoid misinterpretation.
63. Document the reason behind each modulus choice in your change log.
64. `FieldElement` provides `NEG_ONE` constant for convenience; use it to compute subtractions clearly.
65. Resist the urge to implement floating-point approximations for anything in this crate.
66. This manual forbids it; the crate forbids it; the ledger forbids it.
67. Example: verifying polynomial evaluations only requires field arithmetic; everything stays integral.
68. When dealing with odd primes, confirm they exceed the number of constraints.
69. power_house intentionally chooses primes that fit in 64 bits to maintain compatibility with the book.
70. If you attempt to feed a composite modulus, transcripts will break instantly.
71. Document such errors and treat them as sabotage attempts.
72. In training, compute `a^p` for several elements and check results equal `a`.
73. Use `FieldElement::pow` to enforce correct behavior.
74. Write exercises requiring cadets to rewrite polynomials into Lagrange form.
75. They learn why sum-check reductions are safe.
76. The manual expects you to maintain mental agility with GF(p) logic.
77. Provide calculators but never allow them to replace manual reasoning.
78. Install mental guardrails: if exponent exceeds modulus, reduce modulo `p-1` when appropriate.
79. Keep message logs that reference the field used for each transcript.
80. The manual does not repeat this chapter; this is your single warning.
81. Field arithmetic is the base layer; nothing above it is negotiable.
82. If asked "why not floats," respond "because floats mutate logs and ruin consensus."
83. Power-house deliberately uses integer arithmetic to keep transcripts identical across machines.
84. There is no tolerance window; errors are binary: correct or fraudulent.
85. Expect your ledger to throw errors the moment you deviate from deterministic field behavior.
86. The crate governs you; respect it; there is no escape.
87. Compose polynomial commitments only after verifying your field operations.
88. Keep raw integer backups to confirm conversions were correct.
89. Document all calculations in field notebooks for future audits.
90. The unstoppable combination of field arithmetic and transcripts is why Chapter I works.
91. Without it, `ANCHOR!!` would dissolve into meaningless noise.
92. Understanding this chapter is mandatory before entering cross-node reconciliation.
93. Archive this manual with the crate version to maintain legal compliance.
94. When lawyers ask, show them this chapter and back it with code references.
95. The book's authority stems from the code; cross-check each statement; nothing is marketing filler.
96. When you finish reading, annotate the margin with the prime currently deployed.
97. Your signature below indicates you can reproduce every example manually.
98. Sign here: ____________________.
99. Date: ____________________.
100. Proceed to the next chapter only if you completed the exercises honestly.


Chapter III -- Hyperplane Cartography Briefing
================================================================================
Hypercube Holo-map (dim=3 reference):
```
            z
            ^
            |
        *---* (0,1,1)
       /|  /|
      *-+-* |
     /| |/| |
    *-+-*-|-*  -> y
    | *-+-* /
    |/  | / 
    *---*---* -> x
 (0,0,0)   (1,0,0)
```
01. Hyperplane cartography means navigating the Boolean hypercube with precision.
02. `MultilinearPolynomial::from_evaluations(dim, values)` enforces `values.len() == 1 << dim`.
03. Suppose `dim = 3`; values correspond to vertices `(0,0,0)` through `(1,1,1)`.
04. `GeneralSumClaim::prove` iteratively halves dimension, exposing per-round polynomials.
05. Round transcripts store coefficients `a_i`, `b_i`.
06. Verifier samples random challenge `r_i` using deterministic PRNG.
07. Consistency check: `S_i(0) + S_i(1) == previous_value`.
08. If integer arithmetic fails, digest diverges, anchor falls apart.
09. Example dataset: `[0, 1, 4, 5, 7, 8, 11, 23]`.
10. The sum over the cube equals 59; the proof verifies this without recomputing every vertex during verification.
11. `transcript_digest` ensures coefficients align with commitments.
12. The manual expects you to read `src/sumcheck.rs` while consuming this chapter.
13. When verifying transcripts, check each round sum line first.
14. If you see `round_sums: 37 11`, confirm that `37 + 11` equals previous accumulator.
15. Document every challenge `r_i` to maintain traceability.
16. Challenge values come from deterministic PRNG seeded with transcript context.
17. This ensures identical transcripts produce identical digests across nodes.
18. The manual forbids pushing transcripts with omitted challenges.
19. Ensure ledger logs list challenges in order; do not shuffle lines.
20. An example transcript snippet:
21. `statement: Dense polynomial proof`.
22. `challenge: 37`.
23. `round_sums: 12 47`.
24. `final_eval: 19`.
25. `hash: 999B55116F6AFC2F`.
26. The hash matches `digest_A` from Chapter I; cross-reference completed.
27. Each round multiplies dimension by the challenge; watch arithmetic carefully.
28. If you miscompute, fix the code before writing ledger lines.
29. Institutional policy: transcripts must be generated by code, never typed manually; but you must understand them manually.
30. Inspect the ledger log to confirm there are no extraneous blanks.
31. When verifying transcripts without code, check the invariants sequentially.
32. Should something fail, the digest mismatch reveals the culprit.
33. Document failure cases; they make excellent case studies for new recruits.
34. When verifying aggregated proofs, expect longer ledger entries with multiple statements.
35. Each aggregated proof includes multiple hashes; `LedgerAnchor` stores them as vectors.
36. The manual demands you know how to interpret anchor entries with multiple hashes.
37. For each additional proof, expect transcripts to list statements sequentially.
38. Maintain ledger logs sorted lexically; deterministic iteration is easier that way.
39. `AnchorJson` ensures the sequence of statements and hashes is preserved in JSON.
40. When exporting anchors, confirm the JSON matches the ledger log order.
41. If you modify transcript formatting, update this book accordingly.
42. The hyperplane cartography chapter is your blueprint for reading raw transcripts.
43. Example: verifying dimension 10 proofs; expect 10 challenge lines and 10 round sum lines.
44. The final evaluation line ties everything together; it equals the polynomial evaluated at random point determined by challenges.
45. The ledger digest ensures no one can swap final evaluation without detection.
46. For high dimension proofs, consider memory-friendly streaming proofs (Chapter VI).
47. Always cross-reference transcripts with field modulus selection recorded in Chapter II.
48. Document any mixture of dims within a proof bundle.
49. Keep transcripts in chronological order by proof generation date.
50. If transcripts from multiple proofs share ledger file, ensure they remain separated by blank lines.
51. The crate uses ASCII text precisely so you can audit in plain editors.
52. Resist any request to encode transcripts in binary without justification.
53. Provide training on reading transcripts to all on-call engineers.
54. Without comprehension, verifying anchors becomes guesswork.
55. Guesswork is unacceptable.
56. The hypercube is unforgiving; errors multiply quickly.
57. Keep polynomial evaluation functions documented in your lab notes.
58. The manual expects you to reconstruct at least one proof by hand.
59. Provide annotated transcripts in training material to reduce onboarding friction.
60. Explain to management that deterministic transcripts are the reason `ANCHOR!!` is reliable.
61. Use this chapter as a cross-check list when debugging failing proofs.
62. If the digest does not match, inspect challenge order first, round sums second, final evaluation third.
63. Most errors arise from misordered lines or incorrectly reduced field arithmetic.
64. Document the fix in your change log.
65. For aggregated proofs, ensure each statement has matching digest entry in anchor.
66. When verifying aggregated anchor, treat each hash individually, then compare entire sequence.
67. That is what `reconcile_anchors_with_quorum` does internally.
68. Maintain strict naming conventions for statements to keep ledger tidy.
69. For example: `Dense polynomial proof`, `Scaling benchmark`, `Hash anchor proof`.
70. Resist newlines inside statements; the parser expects single-line entries.
71. If you must use multi-line descriptions, embed them in additional metadata fields, not statement line.
72. Keep transcript formats consistent across versions of the crate.
73. Document version updates in ledger file header.
74. Provide `.md` or `.txt` explanation for each log to accompany the ledger.
75. Archive ledger logs after every major proof run.
76. Build a habit: after verifying a proof, compute the digest manually and compare to ledger.
77. Use `cargo run --example verify_logs` to cross-check your manual calculations later.
78. The CLI example is descriptive, but this manual expects you to operate on paper first.
79. Provide a copy of this chapter to auditors ahead of their visit.
80. They will appreciate clear instructions.
81. Remember: transcripts are immutable once logged; append new entries instead.
82. Deleting old transcripts is a firing offense.
83. The crate ensures logs sit in their own directory; keep the directory read-only after generation.
84. For offline review sessions, print transcripts and highlight round sums.
85. Use colored pens to differentiate challenge values, sums, and final evaluation.
86. Encourage cadets to verify calculations using both mental arithmetic and calculators.
87. Cross-check calculators for deterministic behavior; some add rounding artifacts.
88. Use plain integer calculators or spreadsheets set to integer mode.
89. Document each manual verification session; compliance loves logs.
90. Provide transcripts to external reviewers in zipped packages with checksums.
91. Example: `hash_pipeline` example writes to `/tmp/power_house_anchor_a` and `/tmp/power_house_anchor_b`.
92. After running, copy files from `/tmp` into your node directories for anchor generation.
93. Then run `julian node run nodeA ./logs/nodeA nodeA.anchor` to produce human-readable anchor.
94. Compare `nodeA.anchor` to the Chapter I hex digests; they must align byte-for-byte.
95. If they do not, your ledger logs may be outdated; rerun `hash_pipeline`.
96. Keep version numbers in anchor files for traceability.
97. Admission to advanced training requires presenting a hand-written transcript analysis.
98. You now understand why the hypercube matters for consensus.
99. Sign the ledger: ____________________.
100. Today's date: ____________________.


Chapter IV -- Transcript Metallurgy Protocols
================================================================================
01. Transcript metallurgy is my term for shaping ledger entries with surgical precision.
02. Each transcript is a composite of lines: statements, challenges, round sums, final evaluation, hash.
03. Lines are plain ASCII; no binary, no compression.
Transcript grammar (ABNF; ASCII 0x20-0x7E only):
```
record      = statement LF transcript LF round-sums LF final LF hash LF
statement   = "statement:" text
transcript  = "transcript:" numbers
round-sums  = "round_sums:" numbers
final       = "final:" number
hash        = "hash:" hexdigits
text        = 1*(%x20-7E)
numbers     = *(SP number)
number      = 1*DIGIT
hexdigits   = 64*(%x30-39 / %x61-66) ; lowercase only
```
Canonicalization checklist:
- Encode every integer in base-10 ASCII with no separators.
- Emit lowercase hex, exactly two chars per byte, no spacing.
- Enforce LF line endings and append a terminal newline.
- Reject tab characters; comments must be standalone `#` lines outside the hashed block.
04. Example statement: `statement: Dense polynomial proof`.
05. Example challenge line: `challenge: 37`.
06. Example round sums: `round_sums: 12 47`.
07. Example final evaluation: `final_eval: 19`.
08. Example digest: `hash: ded75c45b3b7eedd37041aae79713d7382e000eb4d83fab5f6aca6ca4d276e8c`.
09. Digest is produced by `transcript_digest` using BLAKE2b.
10. The digest ensures tamper evidence; any change mutates the number.
11. Never reorder lines; the digest includes ordering.
12. Keep transcripts under version control in your ledger directory.
13. Comments must begin with `#`; they are not included.
14. Do not mix proof transcripts with metadata in the same file without comment prefix.
15. Provide final evaluation after all round sums.
16. Each round sum line corresponds to a dimension reduction.
17. Provide challenge values before their respective round sums.
18. Resist creating multi-variable round sum outputs; keep them pairwise.
19. Append newline at the end of each transcript file; some tools require it.
20. Use UNIX line endings for consistency.
21. Document field modulus in comments: `# field: 101`.
22. Preserve chronological order of transcripts.
23. Time-stamp files if needed, but keep stamps outside hashed content.
Note: When you log a timestamp, prefer ISO 8601 `YYYY-MM-DDThh:mm:ssZ`; if the clock is suspect, add a monotonic counter (`counter=42`) alongside the UTC stamp.
24. Provide absolute path to ledger files in your audit log.
25. Use `verify_logs` example to cross-check transcript digest; treat it as after-action audit.
26. For aggregated transcripts, label each segment with comment headers.
27. Example comment: `# proof 1 start`.
28. When creating aggregated anchor, ensure each segment has unique statement line.
29. The manual forbids blank statements; entries must be descriptive.
30. Hash line must appear exactly once per transcript segment.
31. Failing to log hash line triggers immediate investigation.
32. Use high-quality editors that do not inject BOM markers.
33. Avoid `nano` default settings that insert extra trailing spaces.
34. Confirm your editor does not rewrap lines automatically.
35. Archive transcripts in read-only directories after anchor generation.
36. Provide zipped backups with SHA256 checksums for long-term storage.
37. Offsite storage should include this manual for context.
38. When verifying transcripts manually, check each line for formatting errors.
39. Example: double-check there are no tab characters.
40. Use a script to detect trailing spaces; remove them before digest generation.
Helper recipe:
```
julian tools canonicalize-transcript ledger_0000.txt > ledger_0000.clean
julian tools digest-transcript ledger_0000.clean --domain JROC_TRANSCRIPT
```
Even if you implement these helpers as shell scripts today, bake them into CI tomorrow.
41. Understand that transcripts are not logs--they are proof artifacts.
42. Do not mix general logging messages within transcript files.
43. Use separate log for CLI output.
44. This manual enforces the rule: transcripts must be pristine.
45. The simpler the format, the easier auditors can follow the data.
46. Anyone requesting JSON transcripts is missing the point; JSON anchors exist separately.
47. Use the CLI to produce JSON anchors for cross-node sharing.
48. Example command: `julian node anchor ./logs/nodeA`.
49. The JSON includes statement array and hash array.
50. Serialize anchor output to share with remote nodes in offline settings.
Note: Rule of thumb: transcripts stay US-ASCII forever; anchors/JSON use UTF-8, and any non-ASCII glyph must be escaped JSON-style.
51. Do not share raw transcript files without encryption; treat them as sensitive.
52. When archived, transcripts serve as legal evidence of proof execution.
53. Document the chain-of-custody for ledger directories.
54. Keep physical copies stored in tamper-evident envelopes.
55. Each envelope should include summary sheet referencing this manual.
56. When editing transcripts (rare), compute new digests and document the change.
57. Strict procedure requires dual signatures on any modification.
58. Provide reason for modification in an adjacent comment line.
59. Example comment: `# corrected round sums due to prior arithmetic slip`.
60. Resist writing transcripts by hand; rely on the crate to produce them, then audit manually.
61. Understand the difference between transcripts (per proof) and anchors (per ledger).
62. Anchor is the digest summary; transcript is the detailed dataset.
63. The combination is bulletproof when used properly.
64. Provide training on reading transcripts before giving trainees access to ledger directories.
65. They should be able to detect missing lines or anomalies instantly.
66. Provide printed transcripts alongside calculators for training.
67. Encourage trainees to compute the digest manually by re-implementing BLAKE2b in simple terms.
68. Good luck with that; still, the exercise teaches respect for determinism.
69. The manual expects you to know the digest algorithm, not treat it as magic.
70. Document your understanding in your training report.
71. Provide reproduction steps for each transcript in your documentation.
72. Example reproduction log: problem definition, polynomial settings, field modulus, final evaluation, digest.
73. Tie transcripts to specific crate version.
74. When crate updates digest algorithm, update this manual immediately.
75. Provide compatibility tables mapping version to digest method.
76. Resist quoting digest values out of context; always include statement and proof details.
77. Mention in your compliance log that you validated each transcript using this manual's checklists.
78. Keep transcript directories accessible but immutable for all operations except append.
79. Provide read-only mounts for network nodes to avoid accidental changes.
80. Confirm backup scripts treat transcripts as static files.
81. Backups should run after each proof batch.
82. Document the backup strategy inside operational manuals.
83. Provide script for auditors to compare transcripts with anchors.
84. Example pseudocode: `for each anchor statement, confirm hash matches transcript digest`.
85. Acceptable difference is zero; any mismatch is unacceptable.
86. When multiple nodes provide transcripts, compare them bit-for-bit.
87. Identify mismatches before running reconciliation.
88. Document mismatch investigation in incident log.
89. Provide closing summary for each transcript file indicating the number of rounds, final evaluation, and digest.
90. The manual expects you to maintain perfect discipline; transcripts are the heartbeat of the ledger.
91. Sign the transcript compliance ledger after each audit cycle.
92. Provide cross-references to the Anchor Echo Engine demonstration to show interplay between transcripts and anchors.
93. When verifying aggregated logs, list each statement and digest explicitly.
94. Keep aggregated logs segmented clearly to prevent confusion.
95. Provide training for new auditors using sanitized transcripts.
96. After reading this chapter, cadets should be able to parse transcripts faster than reading this sentence.
97. Anyone showing signs of confusion must revisit Chapters II and III.
98. When satisfied, annotate your training binder with the day you mastered transcript metallurgy.
99. Signature: ____________________.
100. Date: ____________________.


Chapter V -- Ledger Genesis Mechanics Checklist
================================================================================
Anchor Cross-Section (ledger strata):
```
[Transcript Lines]  --BLAKE2b-->  [Digest Row]
        |                            |
        +--> per-entry stack --------+
                v (merkle mix)
          [Merkle Root Capsule]
                v
          [Ledger Anchor]
                v (quorum pass)
          [Finality Ring]
```
01. Ledger anchors are the commitments stored across sessions.
02. `julian_genesis_anchor()` returns baseline anchor containing `JULIAN::GENESIS`.
03. `LedgerAnchor` struct has `entries: Vec<EntryAnchor>`.
Merkle capsule specification:
```
leaf(i)   = BLAKE2b-256("LEAF" || i_u64_be || transcript_hash_i)
node(a,b) = BLAKE2b-256("NODE" || a || b) ; left/right preserved
root      = fold(node, leaves) duplicating the last leaf if count is odd
```
- `i_u64_be` is the 8-byte big-endian encoding of the leaf index (padding with zeros).
- Leaves consume the 32-byte transcript digests; no additional domain tag is needed.
- Internal nodes always hash `(left || right)`; never sort or swap siblings.
- Render the resulting root as lowercase hex and store alongside the statement.
Worked example (2 leaves):
```
leaf0 = BLAKE2b-256("LEAF" || 0x0000000000000000 || ded7...6e8c)
leaf1 = BLAKE2b-256("LEAF" || 0x0000000000000001 || 0f50...08f9b)
root  = BLAKE2b-256("NODE" || leaf0 || leaf1) = 80e7...44f4
```
04. `EntryAnchor` holds `statement` and `hashes`.
05. Anchor entries remain append-only.
06. `LedgerAnchor::push` appends new statement and hash; duplicates rejected.
07. Anchor order matters; maintain it consistently.
08. Reconciliation compares statement text and associated hash vectors.
09. `reconcile_anchors_with_quorum` requires at least `quorum` anchors to match exactly.
10. Quorum is typically 2 for simple demonstrations.
11. Mismatch yields errors describing diverging statements or hash values.
12. Anchor JSON representation includes `schema`, `network`, `node_id`, `entries`.
JSON schema sketch (`jrocnet.anchor.v1`):
```
{
  "schema": "jrocnet.anchor.v1",
  "network": "JROC-NET",
  "node_id": "nodeA",
  "fold_digest": "a5a1...99b3",   // optional but recommended
  "entries": [
     {"statement":"JULIAN::GENESIS","hashes":["139f...84a"],"merkle_root":"09c0...995a"},
     {"statement":"Dense polynomial proof","hashes":["ded7...6e8c"],"merkle_root":"80e7...44f4"}
  ],
  "crate_version": "0.1.37"
}
```
- Strings are UTF-8; digests remain lowercase hex strings.
- `fold_digest` joins the anchor so remote auditors see the quorum hinge without reading stdout.
13. Node anchor generation command: `julian node run <node_id> <log_dir> <output_file>`.
14. Example: `julian node run nodeA ./logs/nodeA nodeA.anchor`.
15. Output file lists anchor statements and hash numbers.
16. Validate anchor by comparing to the hex digests listed in Chapter I.
17. Boot nodes produce identical anchors when reading identical transcripts.
18. Example summary in anchor file (hex digests):
19. `JROC-NET :: JULIAN::GENESIS -> [139f1985df5b36dae23fa509fb53a006ba58e28e6dbb41d6d71cc1e91a82d84a]`.
20. `JROC-NET :: Dense polynomial proof -> [ded75c45b3b7eedd37041aae79713d7382e000eb4d83fab5f6aca6ca4d276e8c]`.
21. `JROC-NET :: Hash anchor proof -> [0f50904f7be06930a5500c2c54cfb6c2df76241507ebd01ab0a25039d2f08f9b]`.
Golden test vector (book edition `v0.1.37`, field 257):
```
ledger_0000.txt
statement:Dense polynomial proof
transcript:247 246 144 68 105 92 243 202 72 124
round_sums:209 235 57 13 205 8 245 122 72 159
final:9
hash:ded75c45b3b7eedd37041aae79713d7382e000eb4d83fab5f6aca6ca4d276e8c

ledger_0001.txt
statement:Hash anchor proof
transcript:17 230 192 174 226 171
round_sums:21 139 198 99 178 89
final:173
hash:0f50904f7be06930a5500c2c54cfb6c2df76241507ebd01ab0a25039d2f08f9b

fold_digest:a5a1b9528dd9b4e811e89fb492977c2010322d09d2318530b0f01b5b238399b3
anchor_root:80e7cb9d1721ce47f6f908f9ac01098d9c035f1225fff84083a6e1d0828144f4
```
22. Maintain a single numeric representation (hex in this manual); record the chosen format with the ledger and ensure every anchor reproduces the Chapter I digests.
CI guardrail: `cargo run --example hash_pipeline` must emit the golden digests above; fail the build if the field reduction or fold digest drifts.
CI also checks that `Cargo.toml`'s `version` equals the version string printed in this book's title page; no silent mismatches.
23. Document anchors with version numbers and node descriptors.
24. Store anchor files in node-specific directories: `./logs/nodeA`, `./logs/nodeB`.
25. After generating anchors, run `julian node reconcile ./logs/nodeA nodeB.anchor 2`.
26. Expect output: `Finality reached with quorum 2.`
27. Manual verification ensures zero dependency on runtime if necessary.
28. For offline consensus, print anchors and share via secure routes.
29. When reading anchor file, confirm statements align with transcripts.
30. Provide cross-reference from anchor to transcript file names.
31. If anchor contains multiple hash entries per statement, list them clearly.
32. Document aggregated anchor structure when bundling multiple proofs.
33. Provide training on reading and interpreting anchor output.
34. Anchors include network identity string, e.g., `JROC-NET`.
35. Do not change network identifier without updating entire environment.
36. Anchor metadata includes genesis statement and final evaluation statements.
37. Archive old anchors for historical audit; do not delete them.
38. Provide anchor signatures if required; this manual focuses on deterministic digests.
39. When integrating with other systems, keep anchor format stable.
40. Example anchor file uses colon and explanatory notation.
41. Resist customizing the format beyond what crate outputs; uniformity aids audits.
42. Document anchor storage location in operational runbooks.
43. Provide script to package anchors with transcripts for distribution.
44. For cross-node verification, exchange anchor files and transcripts, then confirm matching digests.
45. Write compliance memo summarizing anchor generation procedure.
46. When nodes disagree, inspect ledger logs; anchor mismatch identifies offending node.
47. Provide translation table for anchor digests; keep decimal and hex forms.
48. Anchor digests may include leading zeros; preserve them in output.
49. Remember to update this manual when anchor format changes in future release.
50. Provide `--quorum` parameter when reconciling; default may not match policy.
51. Document the policy for quorum thresholds in governance documentation.
52. Keep a ledger of each reconciliation event, including timestamp and result.
53. For training, simulate mismatched anchors to show error reporting.
54. Example error message: `Quorum check failed: mismatch at statement Dense polynomial proof.`
55. When error occurs, examine transcripts for that statement; find discrepancy.
56. If transcripts match, check digest computation or ledger logs for corruption.
57. Maintain top-level log describing anchor events: generated, reconciled, archived.
58. Provide cross audit between nodes to verify they share the same anchor set.
59. Encourage cross-team reviews to maintain vigilance.
60. When generating anchors, ensure log directory is up to date; stale logs cause mismatches.
61. Example workflow: run `cargo run --example hash_pipeline`, copy outputs, generate anchors, reconcile.
62. Document each step and store log output for evidence.
63. Provide metrics instrumentation to track frequency of anchor generation.
64. `anchors_verified_total` increments each time a peer anchor matches local anchor.
65. Monitor `finality_events_total` for proof that reconciliation reached quorum.
66. Provide board-level summary indicating number of anchors stored, last reconciliation date.
67. Keep anchor files under version control or dedicated storage to detect unauthorized changes.
68. Resist storing anchor files in volatile directories.
69. Keep separate directories per node to avoid confusion.
70. Provide compass headings like `Left ledger`, `Right ledger` to help humans interpret.
71. Clarify that anchor digests represent hashed transcripts, not raw data.
72. This manual expects you to recite anchor generation commands from memory.
73. `julian node anchor` prints JSON to stdout; redirect to file as needed.
74. Example JSON snippet: `{"schema":"jrocnet.anchor.v1","node_id":"nodeA","entries":[...]}`.
75. Validate JSON with offline tools to ensure integrity.
76. Document JSON schema version; update manual if schema evolves.
77. Provide offline procedure to verify JSON anchor by recomputing digests.
78. When verifying anchor, cross-check digests against the Chapter I hex table to confirm base statements.
79. For aggregated anchors, create manual table mapping statement to hash for clarity.
80. Maintain list of anchor files with descriptive names: `nodeA_anchor_2025-10-31.txt`.
81. Keep chain-of-custody log for anchor files, just like transcripts.
82. Provide physical safe for storing printed anchor copies.
83. Book ensures you can continue operations during complete systems outage.
84. All instructions rely on deterministic outputs from the crate.
85. New recruits must reproduce anchor file by hand to graduate.
86. During tabletop exercises, simulate anchor divergence and remediate using manual.
87. For network scale-out, replicate anchor files to new nodes as baseline.
88. Document security classification of anchors; treat them as sensitive since they confirm ledger contents.
89. Provide encryption for anchors when transferring across insecure channels.
90. After reading this chapter, create anchor file for your own transcripts and compare manually.
91. Sign the ledger verifying you completed the procedure.
92. signature: ____________________.
93. date: ____________________.
94. To proceed, confirm you performed the Chapter I hex verification ritual using the digests listed earlier.
95. If not, go back to Chapter I.
96. This chapter is the beating heart of ledger maintenance.
97. Without anchored digests, consensus reduces to gossip.
98. Our manual forbids gossip.
99. Only deterministic anchors keep the federation honest.
100. Proceed to Chapter VI with discipline intact.


Chapter VI -- Deterministic Randomness Discipline Orders
================================================================================
Critical warning:
- If your field modulus satisfies `p <= 2^64`, the simple `next_u64() % p` reduction is acceptable but still document the choice.
- If `p` approaches or exceeds 2^64, switch to the rejection-sampling variant (see below) to avoid bias.
- In every transcript metadata block, write `challenge_mode: mod` or `challenge_mode: rejection` so auditors know which derivation to replay.
01. Fiat-Shamir challenges must be reproducible.
02. power_house now derives Fiat-Shamir challenges with domain-separated BLAKE2b-256.
03. Each invocation absorbs the transcript words, the domain tag `JROC_CHALLENGE`, and an ever-increasing counter.
04. The output block is 32 bytes; the first eight bytes reduce modulo the field to produce the challenge value.
05. Deterministic hashing replaces the old LCG while retaining reproducibility.
06. The seed material is the transcript itself; identical transcripts yield identical challenge streams.
07. The crate avoids ambient randomness so auditors can replay transcripts without external state.
08. When verifying transcripts, recompute challenges using the same hashing steps presented in `prng.rs`.
Challenge derivation pseudocode (current implementation):
```
seed = BLAKE2b-256("JROC_CHALLENGE" || len(tag) || tag || len(transcript) || transcript_words)
prng = SimplePrng::from_seed_bytes(seed)
for i in 0..k {
    r = prng.next_u64()
    challenge_i = r mod p        # biased if p << 2^64
}
```
Bias-mitigated variant (recommended for larger fields):
```
threshold = 2^64 - ((2^64) mod p)
loop {
    r = prng.next_u64()
    if r < threshold {
        return r mod p
    }
}
```
Declare in your transcript metadata which variant you used so verifiers can reproduce the same stream.
Challenge Waterfall (Fiat-Shamir stream):
```
counter 0 -- digest[f7bc...0d4d] --> challenge 247
    |
counter 1 -- digest[f65f...4b64] --> challenge 246
    |
counter 2 -- digest[908b...c51f] --> challenge 144
    |
counter 3 -- digest[44f5...13e4] --> challenge 68
    |
counter 4 -- digest[9669...7ced] --> challenge 105
    |
counter 5 -- digest[5c00...4b66] --> challenge 92
```
09. Document the counter sequence in your audit notes: counter starts at zero and increments per challenge.
10. Provide challenge logs listing counter, digest block, and reduced value.
11. Resist mixing in OS entropy; you would break determinism and fail compliance instantly.
12. Should a protocol require additional entropy, layer it outside the core transcript and record the reasoning.
13. The hash-based generator is not a VRF but is collision-resistant and tamper-evident.
14. Record that assessment in your compliance statement.
15. When computing challenge values manually, replicate the hashing pipeline using trusted tooling.
16. Keep a small script (audited) that prints per-round digests for cross-checking.
17. Validate that challenge values in the ledger appear in the exact order emitted by the hash generator.
18. Document `r_i` values alongside transcripts so auditors can confirm the sequence.
19. Example challenge list for the demo ledger: r1 = 247, r2 = 246, r3 = 144 (exact values from the transcript).
20. Any deviation indicates transcript tampering; the digest exposes it immediately.
21. Provide training exercises where cadets recompute a challenge block by hand using BLAKE2b references.
22. No randomness enters from the environment; everything comes from transcript words and counter.
23. Record this fact in every security review.
24. For cryptographic upgrades, you can wrap the hash derivation with key agreement, but keep the transcript digest identical.
25. Maintain a chain-of-custody document for transcript snapshots used in challenge derivation.
26. When verifying, recompute the first two challenges; mismatch means halt the process.
27. This manual expects silent verification every time you touch a transcript.
28. Provide reproducibility logs showing counter, digest hex, and reduced challenge.
29. Example log line: `counter=2 digest=a8e7... challenge=11`.
30. Store such logs with your audit package.
31. Cross-check scripts must use `derive_many_mod_p` from the crate to avoid inconsistencies.
32. Document script output in your operational runbook for future teams.
33. Avoid customizing domain tags without updating this manual and all tooling.
34. Deterministic hashing ensures anchor digests and challenge streams stay synchronized over time.
35. Without it, Chapter I's hex ritual would drift and finality would crumble.
36. Because transcripts embed the challenge lines and the digest covers the entire transcript, the chain remains immutable.
37. When verifying aggregated proofs, ensure each component proof uses the same domain-tagged hash derivation.
38. Commit changes to randomness code with explicit review; include diff snippets in change logs.
39. Maintain unit tests that verify the hash-based generator yields stable sequences for known transcripts.
40. Include test results in compliance reports to prove nothing regressed.
41. In training, have cadets inspect the updated `prng.rs` and explain each hashing step.
42. Confirm they understand why the counter is reabsorbed into the transcript after emission.
43. Document instructions for customizing challenge derivation if protocol extensions demand it, and update transcripts accordingly.
44. Distinguish between the base deterministic hash generator and any optional enhancements layered on top.
63. For now, base generator serves all official operations.
64. Provide mental model: LCG acts as crank; each transcript line turns the crank once.
65. The crank's clicking ensures no hidden state.
66. Every node manipulates identical crank and obtains identical output.
67. Document unstoppable nature of this process.
68. Should LCG constants change, recompute transcripts and anchors.
69. Update this manual's Chapter I digests after crate upgrade.
70. Provide guidance on migrating anchored logs when constants change.
71. Keep old transcripts archived with documentation describing old constants.
72. For new deployments, record generator constants in configuration baseline.
73. Ensure admin consoles highlight deterministic randomness as key feature.
74. Do not let marketing rename this concept to "AI spontaneity."
75. This is not random; it is deliberate reproducibility.
76. Provide simple pseudocode in training manuals for clarity.
77. Example:
78. ```
79. state = seed
80. for round in rounds:
81.     state = (state * A + C) mod modulus
82.     emit state
83. ```
84. Document modulus as field modulus or selected range.
85. Encourage trainees to simulate generator using spreadsheets.
86. Provide columns for state, challenge, next state.
87. Check results against transcripts.
88. The exercise builds internal trust in the deterministic design.
89. After training, evaluate trainees with challenge reconstruction quiz.
90. Pass or fail; no partial credit.
91. If they cannot reproduce challenge sequence, they cannot audit transcripts.
92. Without auditors, consensus decays; we cannot allow that.
93. Therefore, deterministic randomness discipline remains mandatory reading.
94. Keep personal stash of seeds for quick reference.
95. If you do not know the current seed, you lost control of the ledger.
96. Document the moment you regained control.
97. Have each cadet sign off that they understand this chapter.
98. signature: ____________________.
99. date: ____________________.
100. Only now may you proceed to network operations.


Chapter VII -- Consensus Theater Operations
================================================================================
01. Time to stage consensus on the big network.
02. Feature `net` in `Cargo.toml` activates libp2p integration.
03. Build with `cargo run --features net`.
04. CLI entrypoint: `julian`.
05. Primary command: `julian net start`.
06. Example boot node command:
07. `julian net start \`
08. `  --node-id boot1 \`
09. `  --log-dir ./logs/boot1 \`
10. `  --listen /ip4/0.0.0.0/tcp/7001 \`
11. `  --broadcast-interval 2000 \`
12. `  --quorum 2 \`
13. `  --metrics :9100 \`
14. `  --key ed25519://boot1-seed`.
15. Boot node prints metrics server location and peer ID.
16. Bootstrap second node:
17. `julian net start \`
18. `  --node-id boot2 \`
19. `  --log-dir ./logs/boot2 \`
20. `  --listen /ip4/0.0.0.0/tcp/7002 \`
21. `  --bootstrap /dns4/boot1.jrocnet.com/tcp/7001/p2p/<peerID> \`
22. `  --broadcast-interval 2000 \`
23. `  --quorum 2 \`
24. `  --metrics :9101 \`
25. `  --key ed25519://boot2-seed`.
26. Node logs show finality events and anchor broadcasts.
27. Use deterministic seeds so Peer IDs remain stable.
Note: Deterministic `ed25519://` seeds are for demos only; production nodes must source keys from HSMs or encrypted keyfiles and document the derivation/rotation ritual.
28. Prometheus metrics accessible at `http://127.0.0.1:9100/metrics`.
29. Metrics include `anchors_verified_total`, `finality_events_total`, `anchors_received_total`, `invalid_envelopes_total`.
Metrics crib sheet:
```
anchors_verified_total    Counter, monotonic, increments per matching anchor from peers.
anchors_received_total    Counter, counts every envelope before validation.
finality_events_total     Counter, increments when quorum satisfied.
invalid_envelopes_total   Counter, increments when signature or digest fails.
```
- All counters are unit-less but monotonic; alert if they reset outside planned restarts.
30. Monitor metrics to confirm network health.
31. Anchor broadcast happens at set interval or when anchor changes.
32. Node anchor generation uses same transcripts described earlier.
33. This manual expects you to start network manually before automating.
34. Use `julian net anchor --log-dir ./logs/nodeA` to print JSON anchor.
35. Example JSON snippet:
36. `{"schema":"jrocnet.anchor.v1","node_id":"nodeA","entries":[{"statement":"JULIAN::GENESIS","hashes":["139f1985df5b36dae23fa509fb53a006ba58e28e6dbb41d6d71cc1e91a82d84a"]}]}`.
37. `julian net verify-envelope --file peer.envelope.json --log-dir ./logs/nodeA` verifies envelope before acceptance.
38. Envelope includes base64 payload, signature, public key.
39. If verification fails, metrics increment `invalid_envelopes_total`.
40. `reconcile_anchors_with_quorum` runs for local log and incoming anchor.
41. Quorum success increments `finality_events_total`.
42. Log output includes `Finality reached with quorum 2.`
43. Should mismatch occur, log prints error detailing divergence.
44. Use manual anchors from Chapter V to double-check network results.
45. Record the governance descriptor path (`--policy governance.json`) alongside boot credentials.
46. Static deployments may keep a simple allowlist JSON, but publish it so auditors can diff the membership set.
47. Multisig deployments must guard the state file: document the threshold, authorised signer keys, and the ritual for collecting signatures.
48. Only rotate membership after verifying at least `threshold` signatures on the `GovernanceUpdate` payload; archive every rotation in `logs/policy/`.
49. Legacy clusters can still pass `--policy-allowlist allow.json`, but note that it is read-only and unsuitable for automated rotation.
50. Stake-backed deployments require bond postings recorded in the staking registry; no bond, no quorum rights.
51. Conflicting anchors automatically trigger slashing--investigate the incident, keep the evidence, and reissue the staking registry with the slashed flag preserved.
52. Provide step-by-step onboarding instructions for new nodes, including where to fetch the current policy descriptor.
51. Example: copy ledger logs to new node directory, place the current `governance.json`, run `julian net start` with bootstrap peers.
53. Ensure firewall rules allow incoming connections on chosen ports.
54. Document firewall configuration in operational manual.
55. Provide DNS entries like `boot1.jrocnet.com`, `boot2.jrocnet.com`.
56. Keep DNS records up to date; stale addresses break bootstrap.
57. Use deterministic seeds so restarting nodes retains same Peer IDs.
57. Include metrics snapshots in compliance reports.
58. Provide script to export metrics to CSV for analysis.
59. Example script `curl http://127.0.0.1:9100/metrics`.
60. When network load increases, consider adjusting broadcast interval or quorum threshold.
61. Document any changes to configuration.
62. Encourage running `./scripts/smoke_net.sh` for local two-node test.
63. Script creates temporary logs, spins two nodes, confirms finality.
64. Script output `smoke_net: PASS` indicates success.
65. Keep script updated if CLI changes.
66. For grid deployments, replicate playbook across nodes.
67. Provide operations manual referencing this chapter.
68. When issues occur, inspect logs for `broadcast error: libp2p error`.
69. Confirm log directories exist and contain transcripts; missing logs cause errors.
70. `hash_pipeline` example generates sample logs; copy them to node directories.
71. After copying, rerun nodes to eliminate `InsufficientPeers` warnings.
72. If nodes fail due to firewall, update firewall and restart process.
73. Provide runbook entries for diagnosing `invalid anchor line` errors (usually due to text anchors).
74. When verifying anchors from peers, ensure format matches expected JSON, not textual summary.
75. Document procedure for converting textual anchors to machine-readable format if necessary.
76. For long-running networks, rotate logs and archive older ones.
77. Provide summary reports to stakeholders including finality counts and anchor updates.
78. Keep config files under version control with limited access.
79. Train operators on safe shutdown procedures: `Ctrl+C` once, wait for `node shutting down`.
80. After shutdown, check logs for final summary lines of anchors.
81. Restart nodes carefully; use same seeds to maintain continuity.
82. Provide timeline for network maintenance windows.
83. Document backup plan for ledger logs before performing maintenance.
84. Use manual anchor verification post-maintenance to confirm consistency.
85. Provide contact list for network emergencies.
86. For compliance, print this chapter and keep it on control-room clipboard.
87. The manual expects you to internalize every command.
88. Mistakes happen when people treat network operations lightly.
89. This manual will not tolerate casual attitudes.
90. Always cross-reference network results with ledger anchors and transcripts.
91. The combination of deterministic transcripts, anchors, and network metrics provides full observability.
92. When ready for advanced deployments, introduce additional nodes following same pattern.
93. Extend metrics dashboards in Grafana or equivalent using provided dashboards.
94. Always keep the Chapter I hex demonstration ready to reassure stakeholders.
95. Sign off that you completed network drills: ____________________.
96. Update the operations ledger with date and node IDs tested.
97. Document any anomalies encountered during drills.
98. Submit after-action report to compliance office.
99. File metrics snapshots in audit repository.
100. Proceed to closing chapter once operations ledger updated.


Chapter VIII -- Closing Benediction and Compliance Oath
================================================================================
01. You have survived the Book of Power condensed edition.
02. You now understand the deterministic skeleton of `power_house`.
03. You have matched 256-bit digests and witnessed `ANCHOR!!` emerge from deterministic transcripts.
04. You can recite field arithmetic rules and enforce them ruthlessly.
05. You can read transcripts without blinking.
06. You can shepherd anchors from genesis to multi-node reconciliation.
07. You can uphold deterministic randomness and diagnose network operations.
08. That makes you a custodian of reproducible proofs.
09. Keep this manual within reach at all times.
10. Always cross-reference ledger outputs with this text before trusting them.
11. In compliance audits, this book is admissible evidence.
12. In regulatory hearings, the Anchor Echo ritual becomes showpiece.
13. In training, each cadet must swear the following oath:
14. "I will not permit entropy into my transcripts."
15. "I will not accept anchor mismatches."
16. "I will not fudge field arithmetic to save time."
17. "I will not allow network nodes to broadcast unverified envelopes."
18. The oath includes writing the three hex digests from Chapter I without error.
19. After taking the oath, sign the compliance ledger.
20. Provide version of this manual in organizational wiki.
21. Update the manual whenever crate version changes.
22. Document training schedule referencing each chapter.
23. For cross-team knowledge transfer, host reading groups.
24. Always bring calculators and transcript printouts to the meetups.
25. When new features arrive, extend this book by new chapters following same style.
26. Keep appended chapters in separate supplements to avoid confusion.
Compliance Seal (sign before dismissal):
```
+-----------------------------------------------------+
| ANCHOR!! COMPLIANCE SEAL                            |
| HEIR DIGESTS: genesis * dense * hash * fold         |
| OATH: no entropy, no mismatches, no excuses.        |
| LEDGER: __________________  DATE: ________________  |
+-----------------------------------------------------+
```
27. In closing, remember: a minimal dependency surface means zero excuses for reproducibility lapses.
28. When transcripts lie, anchoring fails; when anchors fail, consensus dies; when consensus dies, the grumpy alien returns.
29. Do not summon me unnecessarily.
30. Sign below to confirm comprehension.
31. ____________________
32. Date: ____________________
33. Keep this page on file with the compliance office.
34. Notify compliance if manual is updated.
35. Provide feedback to lexluger.dev@proton.me if corrections are needed.
36. Always cross-check manual with source code to maintain accuracy.
37. Lest you forget, the code still lives at `power_house`.
38. The book references version `0.1.x`; update references on release.
39. Keep `book_of_power.md` under version control.
40. Print physical copies for war room, secure them in binder.
41. Each copy should include summary of digests, transcripts, anchors, network commands.
42. Consider embossing `ANCHOR!!` on the cover.
43. Teach new hires to respect manual as they respect code.
44. Without literate operations, deterministic code is wasted.
45. The manual ends here; your vigilance continues.
46. May your transcripts stay immutable.
47. May your anchors remain synchronized.
48. May your challenges be deterministic.
49. May your regulators be impressed.
50. Dismissed.

Glossary (pin it inside your binder):
- Anchor: ordered list of statements plus transcript digests, optionally with fold digest metadata.
- Transcript: ASCII record of statement/challenges/round sums/final value/hash for a single proof.
- Fold digest: BLAKE2b-256 hash across transcript digests, used as quorum hinge.
- Domain tag: ASCII label (e.g., `JROC_TRANSCRIPT`) spliced into the hash input to prevent cross-protocol collisions.
- Quorum: minimum count of matching anchors needed for finality (`reconcile_anchors_with_quorum` enforces it).
