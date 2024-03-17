# memflow-registry

docker-style registry to store memflow plugin artifacts.

## Running

```
$env:RUST_LOG="info"
cargo run --release
```

## Usage:

### Uploading a plugin:
```
curl -F 'file=@.storage.bak/libmemflow_coredump.aarch64.so' http://localhost:3000/
curl -F 'file=@.storage.bak/libmemflow_coredump.aarch64.so' http://$(hostname).local:3000/

curl -v http://$(hostname).local:3000/find\?plugin_name\=coredump\&plugin_version\=1 | jq

curl -v http://$(hostname).local:3000/aa8067150b14bee6ee9d4edb0d51472601531437da43cfbc672ddded43641b5d --output file.dll
```

https://crates.io/crates/openapi-tui
