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
curl http://$(hostname).local:3000/\?plugin_name\=coredump\&plugin_version\=1\&file_type\=Pe\&architecture\=X86_64\&tag\=880e0e2 --output file.dll
```

https://crates.io/crates/openapi-tui
