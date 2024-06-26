# memflow-registry

Docker-style registry to store memflow plugin artifacts.

The memflow-registry project aims to provide a centralized solution for managing memflow binary plugins. This server facilitates the distribution and updating of memflow plugins, ensuring that users can easily access the appropriate plugin versions for their memflow version.

All plugins in the official registry are cryptographically signed and verified at the time memflowup or memflow downloads them.

## Features

- Artifact Management: Users can download and explore existing plugins.

- Version Control: The registry maintains version information for plugins, allowing users to access specific versions as needed.

- ABI Compatibility: Ensures that the proper plugin version is provided to end users, minimizing compatibility issues.

- Ease of Distribution: Simplifies the process of installing and updating memflow plugins across different environments.

- Security: Plugins are cryptographically signed before uploading and validated on the client by the end-user.

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
MEMFLOW_BEARER_TOKEN=token
```

In case you are using the default example configuration you also have to create the `.storage` directory first.

If you want to enable and test the file signature verification you have to generate a public-private key-pair that is being used in the upload and verification process.
The keys have to be an ecdsa p256k1 key in pkcs8 format. You can generate the key-pair via:
```bash
$ openssl ecparam -name secp256k1 -genkey | openssl pkcs8 -topk8 -nocrypt -out ec-secp256k1-priv-key.pem
$ openssl ec -in ec-secp256k1-priv-key.pem -pubout > ec-secp256k1-pub-key.pem
```

## Deploying your own instance

Official pre-built images are available in the docker registry [here](https://hub.docker.com/r/ko1n/memflow-registry).

### Configuration / Persistence

The image can be configured via environment variables. For a full list of all available variables take a look at the [.env.example](.env.example) file. To get started, you will most likely just want to override the `MEMFLOW_PUBLIC_KEY_FILE` and `MEMFLOW_BEARER_TOKEN` variables.

The image is also configured to store all artifacts in `/var/lib/memflow-registry/data/mfdata`. To ensure the database survives container restarts, create a volume binding for the folder.

### Scalability

Currently, the image does not support horizontal scaling. Please ensure to only allow one instance to access the storage volume.

## Testing via cURL

### Uploading a plugin artifact

As a first step you might want to upload a plugin. For example you can upload one of the provided sample binaries in the `assets` folder:
```bash
# Generate the file signature:
$ openssl dgst -sha256 -hex -sign ec-secp256k1-priv-key.pem memflow-registry-shared/assets/libmemflow_coredump.aarch64.so
EC-SHA256(memflow-registry-shared/assets/libmemflow_coredump.aarch64.so)= 304402200e45acb16e3f01b6f2f04df06eab1a40f8da90cfaa49a8ad987d013a41c7e647022065ff4ab45e543e5c068e4398c0703cf3142ffa6aee31892672dc6937f624e10a

# Set the file signature:
$ export SIGNATURE="304402200e45acb16e3f01b6f2f04df06eab1a40f8da90cfaa49a8ad987d013a41c7e647022065ff4ab45e543e5c068e4398c0703cf3142ffa6aee31892672dc6937f624e10a"

# Run curl with the appropriate Bearer Token and file signature:
$ curl -H "Authorization: Bearer token" -F 'file=@memflow-registry-shared/assets/libmemflow_coredump.aarch64.so' -F "signature=$SIGNATURE" http://localhost:3000/files
```

To sign and upload all binaries in a folder you can call the above function for all files:
```bash
$ cd assets
$ for i in *; do curl -F "file=@$i" -F "signature=$(openssl dgst -sha256 -hex -sign ../ec-secp256k1-priv-key.pem $i | cut -d' ' -f2)" http://localhost:3000/files; done
```

### Query all available plugins

```bash
$ curl -v http://localhost:3000/plugins
```
```json
{
  "plugins": [
    {
      "name": "coredump",
      "description": "win32 coredump connector for the memflow physical memory introspection framework"
    }
  ]
}
```

### Find specific plugin variants

```bash
$ curl -v http://localhost:3000/plugins/coredump\?memflow_plugin_version\=1\&file_type\=pe\&architecture\=x86_64
```
```json
{
  "plugins": [
    {
      "digest": "880e0e255146016e820a5890137599936232ea9bf26053697541f2c579921065",
      "signature": "30440220240ee8af828d459358882eabb7dbb0405a03c2f7bedd9504ec32190267ce9b27022057bdcef318eb0616fc030d028d4d6a1a802f4e6b95567e66a290380e2702f78d",
      "created_at": "2024-03-22T18:14:53.402258600",
      "descriptor": {
        "file_type": "pe",
        "architecture": "x86_64",
        "plugin_version": 1,
        "name": "coredump",
        "version": "0.2.0",
        "description": "win32 coredump connector for the memflow physical memory introspection framework"
      }
    }
  ],
  "skip": 0
}
```

All filtering is optional. The following filters are currently available:
- version - specific plugin version, think of it like a version tag
- memflow_plugin_version - the memflow abi version
- file_type - either pe, elf or mach
- architecture - either x86, x86_64, arm or arm64
- digest - sha256 digest of the plugin binary
- digest_short - sha256 of the plugin binary but cropped to 7 digits

Additionally, this api supports pagination by providing the following parameters:
- skip - skip the first `skip` elements
- limit - only show `limit` items

All plugins are sorted by upload date. So the latest version of a specific variant is always the first one in the list.

### Download a plugin

```bash
$ curl -v http://localhost:3000/files/880e0e255146016e820a5890137599936232ea9bf26053697541f2c579921065 --output file.dll
```

### Retrieve metadata for a specific plugin

```bash
$ curl -v http://localhost:3000/files/880e0e255146016e820a5890137599936232ea9bf26053697541f2c579921065/metadata
```
```json
{
  "digest": "30d5292c0acf87cf7a4ebddfcd108230f864609c882a0f7ca26ecbff4b2f5ee6",
  "signature": "3044022030E3E6C64269B29FA2FDE1E62774EAD6EF756C61C31E0A1287C9D55F53951AB302205CEC067CDE046A874701DCA2CD22346E6FD6D86B1ACCCC4CD7FC366E441556F8",
  "created_at": "2024-04-06T20:49:56.440891100",
  "descriptors": [
    {
      "plugin_kind": "connector",
      "export_name": "MEMFLOW_CONNECTOR_COREDUMP",
      "file_type": "elf",
      "architecture": "x86_64",
      "plugin_version": 1,
      "name": "coredump",
      "version": "0.2.0",
      "description": "win32 coredump connector for the memflow physical memory introspection framework"
    }
  ]
}
```

### Delete a plugin binary

```bash
$ curl -v -X DELETE -H "Authorization: Bearer token" http://localhost:3000/files/880e0e255146016e820a5890137599936232ea9bf26053697541f2c579921065
```

Since a plugin binary can contain multiple plugins, this call ensures all plugin variants are removed from the database.

## Roadmap

- Add pull-through capabilities to registry so end-users can setup their own registries more easily
- Add support for multiple signing keys
- Web UI for browsing the plugin database

## Contributing

Contributions to the memflow-registry server are welcome! If you would like to contribute, please follow these guidelines:

- Fork the repository and create a new branch for your feature or bug fix.
- Make your changes and ensure all tests pass.
- Submit a pull request with a clear description of your changes.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
