# memflow-registry

docker-style registry to store memflow plugin artifacts.

## Running

```
cargo run --release
```

## Usage:

### Uploading a plugin:
```
curl -F 'file=@.storage.bak/libmemflow_coredump.aarch64.so' http://localhost:3000/
```

https://crates.io/crates/openapi-tui