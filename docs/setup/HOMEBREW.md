# Homebrew Installation Guide for Valknut

This guide explains how to set up Valknut for distribution via Homebrew on macOS.

## Prerequisites

- Rust toolchain (for building from source)
- Homebrew installed on your system

## For Users

### Installing from Homebrew Tap

Once the tap is published:

```bash
brew tap sibyllinesoft/valknut
brew install valknut
```

### Building from Source

```bash
git clone https://github.com/sibyllinesoft/valknut
cd valknut
cargo build --release
cp target/release/valknut /usr/local/bin/
```

## For Maintainers

### Creating a New Release

1. Update version in `Cargo.toml`
2. Run the release script:
   ```bash
   ./scripts/release.sh 0.1.0
   ```

3. Push the tag to GitHub:
   ```bash
   git push origin v0.1.0
   ```

4. Create a GitHub release:
   - Go to https://github.com/sibyllinesoft/valknut/releases
   - Click "Create a new release"
   - Select the tag you just created
   - Upload the binary from `target/release/valknut`

### Updating the Homebrew Formula

1. Get the SHA256 of the release tarball:
   ```bash
   curl -L https://github.com/sibyllinesoft/valknut/archive/refs/tags/v0.1.0.tar.gz | shasum -a 256
   ```

2. Update the formula in `homebrew-valknut/Formula/valknut.rb`:
   - Update the `url` with the new release URL
   - Update the `sha256` with the calculated hash
   - Update the `tag` in the stable block

3. Test the formula locally:
   ```bash
   cd homebrew-valknut
   brew install --build-from-source Formula/valknut.rb
   brew test Formula/valknut.rb
   brew audit --strict Formula/valknut.rb
   ```

4. Push the updated formula to your tap repository

### Publishing the Tap

1. Create a new repository named `homebrew-valknut` under the `sibyllinesoft` organization
2. Push the tap contents:
   ```bash
   cd homebrew-valknut
   git init
   git add .
   git commit -m "Initial tap for Valknut"
   git remote add origin https://github.com/sibyllinesoft/homebrew-valknut
   git push -u origin main
   ```

## Testing

Test the installation:

```bash
brew tap sibyllinesoft/valknut
brew install valknut
valknut --version
valknut analyze .
```

## Bottling (Optional)

For faster installation, you can create bottles (pre-compiled binaries):

```bash
brew install --build-bottle valknut
brew bottle valknut
```

This will create bottle files that can be added to the formula for specific macOS versions.

## Troubleshooting

- If the build fails, ensure Rust is properly installed
- For M1/M2 Macs, the build will create an ARM64 binary
- For Intel Macs, the build will create an x86_64 binary

## Future Improvements

- Add bottles for faster installation
- Consider submitting to homebrew-core once the project is stable
- Add CI/CD for automatic formula updates