# Published evidence subsets

`release/` contains the selected, browser-verified files from the current MFENX Local release capture.

`release-v1/` preserves the exact selected payload that was published for the accepted MFENX Local v1 capture. Its files are retained as an immutable comparator; future releases must use a new versioned directory rather than changing `release-v1/`.

Each directory includes the complete capture `SHA256SUMS` file, but only a selected subset of the files named by that manifest is published here. The browser verifies every published selected file against the corresponding full-capture manifest entry. These are checksum records, not digital signatures or third-party attestations.
