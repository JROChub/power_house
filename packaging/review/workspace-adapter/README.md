# Analysis workspace adapter

`Cargo.lock` is a commit-bound input for the validation automated security
workflow. It resolves only the four Rust crates whose source is present in the
signed validation-candidate archive: the three local-executor crates and the
standalone reference verifier.

The workflow copies those signed crates into a separate writable analysis
directory, generates a four-member `Cargo.toml`, and uses this lock with
`--locked`. The overlay manifest records both the original signed lock digest
and this adapter lock digest. Neither this directory nor the generated overlay
is validation-candidate release source.
