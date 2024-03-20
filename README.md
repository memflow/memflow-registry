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

$ compute signature:
openssl dgst -sha256 -hex -sign ../ec-secp256k1-priv-key.pem libmemflow_coredump.aarch64.so

$ upload with signature:
curl -F 'file=@libmemflow_coredump.aarch64.so' -F "signature=$(openssl dgst -sha256 -hex -sign ../ec-secp256k1-priv-key.pem libmemflow_coredump.aarch64.so | cut -d' ' -f2)" http://$(hostname).local:3000/

$ upload all files from folder:
for i in *; do curl -F "file=@$i" http://localhost:3000/; done

$ upload all files with signatures
for i in *; do curl -F "file=@$i" -F "signature=$(openssl dgst -sha256 -hex -sign ../ec-secp256k1-priv-key.pem $i | cut -d' ' -f2)" http://localhost:3000/; done

curl -v http://$(hostname).local:3000/find\?plugin_name\=coredump\&plugin_version\=1 | jq

curl -v http://$(hostname).local:3000/aa8067150b14bee6ee9d4edb0d51472601531437da43cfbc672ddded43641b5d --output file.dll
```

https://crates.io/crates/openapi-tui

Generate a signing keypair:
```
openssl ecparam -name secp256k1 -genkey | openssl pkcs8 -topk8 -nocrypt -out ec-secp256k1-priv-key.pem
openssl ec -in ec-secp256k1-priv-key.pem -pubout > ec-secp256k1-pub-key.pem
```

Sign a connector with the newly generated private key:
```
cargo run --release --example sign_file assets\memflow_coredump.x86_64.dll ec-secp256k1-priv-key.pem
```


$env:RUST_LOG="info"
$env:MEMFLOW_PUBLIC_KEY="ec-secp256k1-pub-key.pem"