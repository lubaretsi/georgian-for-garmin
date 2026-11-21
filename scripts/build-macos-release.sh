#!/usr/bin/env bash
# Builds the macOS release: a universal (arm64 + x86_64) command-line tool and desktop app, built
# with Apple's toolchain and rustup's Rust, embedding the fonts from `nix build .#fonts`.
#
# Run it outside `nix develop`: binaries from the dev shell link Nix store libraries and need the
# macOS version of Nix's SDK. Nix only builds the fonts and supplies the Tauri CLI, which runs here
# but is not linked into anything.
#
# Signing is ad hoc only: no identity, no notarization. The linker signs only arm64 code, so the
# universal binaries are re-signed ad hoc to cover both slices (the app by Tauri, through
# `"signingIdentity": "-"`, which also seals the bundle so Gatekeeper doesn't call it damaged).
#
# Needs: Xcode or its command-line tools, rustup, Nix with flakes. Writes to target/macos-release/;
# the files to publish end up in target/macos-release/dist/. If RELEASE_TAG is set (CI on a tag),
# it must be v<workspace version>.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

die() {
  echo "error: $*" >&2
  exit 1
}
step() { printf '\n==> %s\n' "$*"; }

[[ $(uname -s) == Darwin ]] || die "macOS releases are built on macOS"
[[ -z ${IN_NIX_SHELL:-} ]] || die "run this outside \`nix develop\`; its toolchain links Nix store libraries"
nix=$(command -v nix) || die "Nix is needed to build the fonts"
rustup=$(command -v rustup) || die "rustup is needed (https://rustup.rs)"

toolchain=$(sed -n 's/^channel = "\(.*\)"$/\1/p' rust-toolchain.toml)
targets=(aarch64-apple-darwin x86_64-apple-darwin)
min_macos=$(plutil -extract bundle.macOS.minimumSystemVersion raw -o - crates/app/tauri.conf.json)
work=$root/target/macos-release
dist=$work/dist

step "Checking that rust-toolchain.toml matches nixpkgs"
scripts/check-rust-toolchain.sh

step "Building the fonts and checking them against the pinned hashes"
system=$("$nix" eval --raw --impure --expr builtins.currentSystem)
"$nix" build --no-link --print-build-logs ".#fonts" ".#checks.$system.builder"
fonts=$("$nix" build --no-link --print-out-paths ".#fonts")
cargo_tauri=$("$nix" build --no-link --print-out-paths --inputs-from . nixpkgs#cargo-tauri)/bin/cargo-tauri

step "Installing Rust $toolchain for ${targets[*]}"
"$rustup" toolchain install "$toolchain" --profile minimal --no-self-update \
  --target "$(IFS=,; echo "${targets[*]}")"
rust_bin=$(dirname "$("$rustup" which --toolchain "$toolchain" cargo)")

# Apple's toolchain only: nothing from Nix, Homebrew or the caller's compiler settings.
export PATH="$rust_bin:/usr/bin:/bin:/usr/sbin:/sbin"
unset CC CXX AR LD CFLAGS CXXFLAGS LDFLAGS RUSTC RUSTC_WRAPPER RUSTFLAGS \
  CARGO_ENCODED_RUSTFLAGS CARGO_BUILD_RUSTFLAGS CARGO_BUILD_TARGET NIX_CFLAGS_COMPILE NIX_LDFLAGS
DEVELOPER_DIR=$(xcode-select -p)
SDKROOT=$(xcrun --sdk macosx --show-sdk-path)
export DEVELOPER_DIR SDKROOT
export MACOSX_DEPLOYMENT_TARGET=$min_macos
export CARGO_TARGET_DIR=$work/cargo
export GEORGIAN_FOR_GARMIN_FONTS=$fonts

pkgid=$(cargo pkgid --locked -p georgian-for-garmin)
version=${pkgid##*[#@]}
if [[ -n ${RELEASE_TAG:-} && $RELEASE_TAG != "v$version" ]]; then
  die "tag $RELEASE_TAG does not match the workspace version $version"
fi
echo "Georgian for Garmin $version, macOS $min_macos+, $(rustc --version), SDK $(xcrun --sdk macosx --show-sdk-version)"

step "Building the command-line tool"
target_args=() cli_slices=()
for target in "${targets[@]}"; do
  target_args+=(--target "$target")
  cli_slices+=("$CARGO_TARGET_DIR/$target/release/georgian-for-garmin")
done
cargo build --locked --release -p georgian-for-garmin "${target_args[@]}"
rm -rf "$work/stage" "$dist"
mkdir -p "$work/stage" "$dist"
cli=$work/stage/georgian-for-garmin
lipo -create -output "$cli" "${cli_slices[@]}"
codesign --force --sign - "$cli"

step "Building the app"
(cd crates/app && "$cargo_tauri" build --ci --target universal-apple-darwin --bundles app -- --locked)
product=$(plutil -extract productName raw -o - crates/app/tauri.conf.json)
app=$CARGO_TARGET_DIR/universal-apple-darwin/release/bundle/macos/$product.app
[[ -d $app ]] || die "tauri build made no $app"
app_bin=$app/Contents/MacOS/$(plutil -extract CFBundleExecutable raw -o - "$app/Contents/Info.plist")

step "Checking the binaries"
check_binary() {
  local bin=$1 arch libs minos signature
  for arch in arm64 x86_64; do
    lipo "$bin" -verify_arch "$arch" || die "$bin has no $arch slice"
    libs=$(otool -L -arch "$arch" "$bin" | tail -n +2 | awk '{print $1}')
    if grep -Ev '^(/usr/lib/|/System/Library/)' <<<"$libs"; then
      die "$bin ($arch) links libraries outside /usr/lib and /System (above)"
    fi
    minos=$(vtool -arch "$arch" -show-build "$bin" | awk '$1 == "minos" {print $2}')
    [[ $minos == "$min_macos" ]] || die "$bin ($arch) needs macOS $minos, not $min_macos"
    signature=$(codesign -dv --arch "$arch" "$bin" 2>&1)
    grep -qx 'Signature=adhoc' <<<"$signature" || die "$bin ($arch) is not ad-hoc signed"
  done
  codesign --verify --strict "$bin" || die "$bin has no valid signature"
}
check_binary "$cli"
check_binary "$app_bin"
codesign --verify --deep --strict "$app" || die "$app has no valid signature"
plist_min=$(plutil -extract LSMinimumSystemVersion raw -o - "$app/Contents/Info.plist")
[[ $plist_min == "$min_macos" ]] || die "the app's Info.plist asks for macOS $plist_min, not $min_macos"

# Each slice of both binaries must carry every font byte for byte.
for bin in "$cli" "$app_bin"; do
  for arch in arm64 x86_64; do
    lipo "$bin" -thin "$arch" -output "$work/stage/slice"
    /usr/bin/python3 - "$fonts" "$work/stage/slice" "$(basename "$bin") $arch" <<'EOF'
import hashlib, json, pathlib, sys

fonts, binary, label = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2]), sys.argv[3]
data = binary.read_bytes()
for font in json.loads((fonts / "manifest.json").read_text())["fonts"]:
    for kind in ("stock", "georgian"):
        blob = (fonts / font[kind]).read_bytes()
        offset = data.find(blob)
        if offset < 0:
            sys.exit(f"error: {label} does not embed {font[kind]}")
        embedded = hashlib.sha256(data[offset:offset + len(blob)]).hexdigest()
        if kind == "georgian":
            print(f"{label}: embeds {font[kind]} ({font['size_px']} px) sha256 {embedded}")
EOF
  done
done
rm -f "$work/stage/slice"

step "Packaging"
# No version in the file names, so the README's releases/latest/download/<name> links keep working.
zip_name=Georgian-for-Garmin-macos.zip
cli_name=georgian-for-garmin-macos-universal
ditto -c -k --sequesterRsrc --keepParent "$app" "$dist/$zip_name"
mkdir "$work/stage/$cli_name"
install -m 755 "$cli" "$work/stage/$cli_name/georgian-for-garmin"
install -m 644 LICENSE "$work/stage/$cli_name/LICENSE"
install -m 644 "$fonts/LICENSES/OFL.txt" "$work/stage/$cli_name/OFL.txt"
COPYFILE_DISABLE=1 tar -czf "$dist/$cli_name.tar.gz" --uid 0 --gid 0 --uname root --gname wheel \
  -C "$work/stage" "$cli_name"
install -m 644 "$fonts/manifest.json" "$dist/manifest.json"
(cd "$dist" && shasum -a 256 "$zip_name" "$cli_name.tar.gz" manifest.json >SHA256SUMS)

step "Done: $dist"
ls -l "$dist"
cat "$dist/SHA256SUMS"
