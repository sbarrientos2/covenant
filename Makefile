.PHONY: build test deploy clean

# Build the program with the correct platform-tools version
build:
	cargo-build-sbf --tools-version v1.51

# Run tests
test: build
	anchor test --skip-build

# Deploy to devnet
deploy: build
	anchor deploy --provider.cluster devnet

# Clean build artifacts
clean:
	cargo clean
	rm -rf target/deploy/*.so

# Generate IDL
idl:
	anchor idl init --filepath target/idl/covenant.json $(shell solana address -k target/deploy/covenant-keypair.json)
