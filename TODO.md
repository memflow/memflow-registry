- support signature for binaries via: https://docs.rs/signature/latest/signature/
- allow specifying tag on upload -> add fallback tag for sha?

- collect upload timestamp in metadata
- sort by upload timestamp to retrieve newest version of a connector with a specific tag

- store list of associated tags in metadata, too

- if hash of a connector already exists add tags to tag list (+ write new tag to file)
