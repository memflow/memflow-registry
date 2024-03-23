# memflow-registry

Docker-style registry to store memflow plugin artifacts.

The memflow-registry project aims to provide a centralized solution for managing memflow binary plugins. This server facilitates the distribution and updating of memflow plugins, ensuring that users can easily access the appropriate plugin versions for their memflow version.

## Features

- Artifact Management: Users can download and explore existing plugins.

- Version Control: The registry maintains version information for plugins, allowing users to access specific versions as needed.

- ABI Compatibility: Ensures that the proper plugin version is provided to end users, minimizing compatibility issues.

- Ease of Distribution: Simplifies the process of installing and updating memflow plugins across different environments.

- Security: Plugins can be cryptographically signed before the upload and validated on the client that is downloading the plugin.

## Getting started

To start development you can simply start the server instance:
```
cargo run --release
```

All configuration is stored in the `.env` file in the root folder.
To customize settings copy the `.env.example` file to `.env` and edit it.
The default `.env` values are:
```bash
# enable `info` and higher logs
RUST_LOG=info

# Serve the http api on all interfaces on port 3000
MEMFLOW_ADDR=0.0.0.0:3000

# Store plugin artifacts in `.storage`
MEMFLOW_STORAGE_ROOT=.storage

# Enable file signature verification support in the backend
#MEMFLOW_PUBLIC_KEY_FILE=ec-secp256k1-pub-key.pem

# Enable and set the bearer token which is required to upload and delete artifacts
MEMFLOW_BEARER_TOKEN=1234
```

In case you are using the default example configuration you also have to create the `.storage` directory first.

If you want to enable and test the file signature verification you have to generate a public-private key-pair that is being used in the upload and verification process.
The keys have to be an ecdsa p256k1 key in pkcs8 format. You can generate the keypair via:
```bash
$ openssl ecparam -name secp256k1 -genkey | openssl pkcs8 -topk8 -nocrypt -out ec-secp256k1-priv-key.pem
$ openssl ec -in ec-secp256k1-priv-key.pem -pubout > ec-secp256k1-pub-key.pem
```

## Deploying your own instance

## Testing via cURL / OpenAPI

...

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
for i in *; do curl -F "file=@$i" -F "signature=$(openssl dgst -sha256 -hex -sign ../ec-secp256k1-priv-key.pem $i | cut -d' ' -f2)" http://localhost:3000/files; done

$ list all available plugins
curl -v http://$(hostname).local:3000/plugins| jq

$ list specific artifacts
curl -v http://$(hostname).local:3000/plugins/coredump\?memflow_plugin_version\=1 | jq

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

## Contributing

Contributions to the memflow-registry server are welcome! If you would like to contribute, please follow these guidelines:

- Fork the repository and create a new branch for your feature or bug fix.
- Make your changes and ensure all tests pass.
- Submit a pull request with a clear description of your changes.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
